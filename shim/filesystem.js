/**
 * filesystem.js
 * 
 * why: provide persistent storage for wasm nodes using indexeddb
 * relations: used by host.js, intercepts wasi:filesystem calls
 * what: VirtualFilesystem class, per-node storage isolation
 */

/**
 * virtual filesystem that maps std::fs operations to indexeddb.
 * 
 * each node gets an isolated namespace in the database to prevent
 * state corruption between nodes. supports crash recovery via
 * persistent storage.
 */
export class VirtualFilesystem {
    constructor() {
        /** @type {IDBDatabase | null} */
        this.db = null;

        /** @type {string} */
        this.dbName = 'raft-storage';

        /** @type {number} */
        this.dbVersion = 1;
    }

    /**
     * initialize the indexeddb database
     * @returns {Promise<void>}
     */
    async init() {
        if (this.db) return;

        return new Promise((resolve, reject) => {
            const request = indexedDB.open(this.dbName, this.dbVersion);

            request.onerror = () => reject(request.error);

            request.onsuccess = () => {
                this.db = request.result;
                resolve();
            };

            request.onupgradeneeded = (event) => {
                const db = event.target.result;

                // files store: keyed by [nodeId, path]
                if (!db.objectStoreNames.contains('files')) {
                    db.createObjectStore('files', { keyPath: ['nodeId', 'path'] });
                }

                // metadata store: term and voted_for per node
                if (!db.objectStoreNames.contains('metadata')) {
                    db.createObjectStore('metadata', { keyPath: 'nodeId' });
                }
            };
        });
    }

    /**
     * write data to a file for a specific node
     * @param {number} nodeId 
     * @param {string} path 
     * @param {string|Uint8Array} data 
     * @returns {Promise<void>}
     */
    async write(nodeId, path, data) {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('files', 'readwrite');
            const store = tx.objectStore('files');

            const record = {
                nodeId,
                path,
                data: typeof data === 'string' ? data : Array.from(data),
                timestamp: Date.now()
            };

            const request = store.put(record);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
        });
    }

    /**
     * read data from a file for a specific node
     * @param {number} nodeId 
     * @param {string} path 
     * @returns {Promise<string|null>}
     */
    async read(nodeId, path) {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('files', 'readonly');
            const store = tx.objectStore('files');

            const request = store.get([nodeId, path]);
            request.onsuccess = () => {
                const record = request.result;
                if (record) {
                    resolve(typeof record.data === 'string' ? record.data : new Uint8Array(record.data));
                } else {
                    resolve(null);
                }
            };
            request.onerror = () => reject(request.error);
        });
    }

    /**
     * check if a file exists
     * @param {number} nodeId 
     * @param {string} path 
     * @returns {Promise<boolean>}
     */
    async exists(nodeId, path) {
        const data = await this.read(nodeId, path);
        return data !== null;
    }

    /**
     * delete a file
     * @param {number} nodeId 
     * @param {string} path 
     * @returns {Promise<void>}
     */
    async delete(nodeId, path) {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('files', 'readwrite');
            const store = tx.objectStore('files');

            const request = store.delete([nodeId, path]);
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
        });
    }

    /**
     * clear all files for a node (for testing/reset)
     * @param {number} nodeId 
     * @returns {Promise<void>}
     */
    async clearNode(nodeId) {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('files', 'readwrite');
            const store = tx.objectStore('files');

            const request = store.openCursor();
            request.onsuccess = (event) => {
                const cursor = event.target.result;
                if (cursor) {
                    if (cursor.value.nodeId === nodeId) {
                        cursor.delete();
                    }
                    cursor.continue();
                } else {
                    resolve();
                }
            };
            request.onerror = () => reject(request.error);
        });
    }

    /**
     * clear entire database (for demo reset)
     * @returns {Promise<void>}
     */
    async clearAll() {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction(['files', 'metadata'], 'readwrite');

            tx.objectStore('files').clear();
            tx.objectStore('metadata').clear();

            tx.oncomplete = () => resolve();
            tx.onerror = () => reject(tx.error);
        });
    }

    // -- raft-specific storage helpers --

    /**
     * save raft metadata (term and voted_for)
     * @param {number} nodeId 
     * @param {number} term 
     * @param {number|null} votedFor 
     * @returns {Promise<void>}
     */
    async saveMetadata(nodeId, term, votedFor) {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('metadata', 'readwrite');
            const store = tx.objectStore('metadata');

            const request = store.put({ nodeId, term, votedFor, timestamp: Date.now() });
            request.onsuccess = () => resolve();
            request.onerror = () => reject(request.error);
        });
    }

    /**
     * load raft metadata (term and voted_for)
     * @param {number} nodeId 
     * @returns {Promise<{term: number, votedFor: number|null}>}
     */
    async loadMetadata(nodeId) {
        await this.ensureDb();

        return new Promise((resolve, reject) => {
            const tx = this.db.transaction('metadata', 'readonly');
            const store = tx.objectStore('metadata');

            const request = store.get(nodeId);
            request.onsuccess = () => {
                const record = request.result;
                if (record) {
                    resolve({ term: record.term, votedFor: record.votedFor });
                } else {
                    resolve({ term: 0, votedFor: null }); // default for new nodes
                }
            };
            request.onerror = () => reject(request.error);
        });
    }

    /**
     * save log entries
     * @param {number} nodeId 
     * @param {Array} entries 
     * @returns {Promise<void>}
     */
    async saveLog(nodeId, entries) {
        await this.write(nodeId, 'log.json', JSON.stringify(entries));
    }

    /**
     * load log entries
     * @param {number} nodeId 
     * @returns {Promise<Array>}
     */
    async loadLog(nodeId) {
        const data = await this.read(nodeId, 'log.json');
        if (!data) return [];
        try {
            return JSON.parse(data);
        } catch {
            return [];
        }
    }

    // -- helpers --

    /**
     * ensure db is initialized
     * @returns {Promise<void>}
     */
    async ensureDb() {
        if (!this.db) {
            await this.init();
        }
    }
}

// singleton instance
export const filesystem = new VirtualFilesystem();
