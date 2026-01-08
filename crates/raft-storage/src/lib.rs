//! # raft-storage
//!
//! why: provide durable persistence for raft state using standard rust fs apis
//! relations: used by raft-core for state persistence, mapped to indexeddb via wasi
//! what: Storage trait, FileStorage implementation, InMemoryStorage for testing

use raft_core::LogEntry;
use std::io::{self, Read, Write};
use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;

/// trait for durable storage of raft state
/// 
/// this abstraction allows the same code to work with:
/// - real filesystem (native)  
/// - indexeddb (browser via wasi)
/// - in-memory (testing)
pub trait Storage {
    /// persist the current term and voted_for
    fn save_term_and_vote(&mut self, term: u64, voted_for: Option<u64>) -> io::Result<()>;
    
    /// load the persisted term and voted_for
    fn load_term_and_vote(&self) -> io::Result<(u64, Option<u64>)>;
    
    /// append entries to the log
    fn append_entries(&mut self, entries: &[LogEntry]) -> io::Result<()>;
    
    /// load all log entries (for crash recovery)
    fn load_log(&self) -> io::Result<Vec<LogEntry>>;
    
    /// truncate log from given index (for conflict resolution)
    fn truncate_log_from(&mut self, from_index: u64) -> io::Result<()>;
    
    /// clear all persisted state (for testing)
    fn clear(&mut self) -> io::Result<()>;
}

// -- file storage implementation --

/// file-based storage implementation using std::fs
/// 
/// stores raft state in a directory with:
/// - meta.json: term and voted_for
/// - log.json: array of log entries
pub struct FileStorage {
    /// directory path for storing state files
    dir: PathBuf,
}

impl FileStorage {
    /// create a new filestorage at the given directory
    /// creates the directory if it doesn't exist
    pub fn new(dir: impl Into<PathBuf>) -> io::Result<Self> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }
    
    /// get the path to the metadata file
    fn meta_path(&self) -> PathBuf {
        self.dir.join("meta.json")
    }
    
    /// get the path to the log file
    fn log_path(&self) -> PathBuf {
        self.dir.join("log.json")
    }
}

/// metadata structure for term and vote
#[derive(serde::Serialize, serde::Deserialize, Default)]
struct MetaData {
    term: u64,
    voted_for: Option<u64>,
}

impl Storage for FileStorage {
    fn save_term_and_vote(&mut self, term: u64, voted_for: Option<u64>) -> io::Result<()> {
        let meta = MetaData { term, voted_for };
        let json = serde_json::to_string_pretty(&meta)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        // atomic write: write to temp file then rename
        let temp_path = self.dir.join("meta.tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp_path, self.meta_path())?;
        
        Ok(())
    }
    
    fn load_term_and_vote(&self) -> io::Result<(u64, Option<u64>)> {
        let path = self.meta_path();
        if !path.exists() {
            return Ok((0, None)); // default for new nodes
        }
        
        let mut file = File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let meta: MetaData = serde_json::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        Ok((meta.term, meta.voted_for))
    }
    
    fn append_entries(&mut self, entries: &[LogEntry]) -> io::Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        
        // load existing log
        let mut log = self.load_log()?;
        
        // append new entries
        log.extend(entries.iter().cloned());
        
        // write entire log (simple approach - could optimize with append-only file)
        let json = serde_json::to_string_pretty(&log)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        let temp_path = self.dir.join("log.tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp_path, self.log_path())?;
        
        Ok(())
    }
    
    fn load_log(&self) -> io::Result<Vec<LogEntry>> {
        let path = self.log_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        
        let mut file = File::open(&path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        let log: Vec<LogEntry> = serde_json::from_str(&contents)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        Ok(log)
    }
    
    fn truncate_log_from(&mut self, from_index: u64) -> io::Result<()> {
        let mut log = self.load_log()?;
        log.retain(|e| e.index < from_index);
        
        let json = serde_json::to_string_pretty(&log)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        
        let temp_path = self.dir.join("log.tmp");
        let mut file = File::create(&temp_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temp_path, self.log_path())?;
        
        Ok(())
    }
    
    fn clear(&mut self) -> io::Result<()> {
        let _ = fs::remove_file(self.meta_path());
        let _ = fs::remove_file(self.log_path());
        Ok(())
    }
}

// -- in-memory storage implementation --

/// in-memory storage for testing
/// 
/// stores all state in memory, no persistence across restarts
#[derive(Default)]
pub struct InMemoryStorage {
    term: u64,
    voted_for: Option<u64>,
    log: Vec<LogEntry>,
}

impl InMemoryStorage {
    /// create a new in-memory storage
    pub fn new() -> Self {
        Self::default()
    }
}

impl Storage for InMemoryStorage {
    fn save_term_and_vote(&mut self, term: u64, voted_for: Option<u64>) -> io::Result<()> {
        self.term = term;
        self.voted_for = voted_for;
        Ok(())
    }
    
    fn load_term_and_vote(&self) -> io::Result<(u64, Option<u64>)> {
        Ok((self.term, self.voted_for))
    }
    
    fn append_entries(&mut self, entries: &[LogEntry]) -> io::Result<()> {
        self.log.extend(entries.iter().cloned());
        Ok(())
    }
    
    fn load_log(&self) -> io::Result<Vec<LogEntry>> {
        Ok(self.log.clone())
    }
    
    fn truncate_log_from(&mut self, from_index: u64) -> io::Result<()> {
        self.log.retain(|e| e.index < from_index);
        Ok(())
    }
    
    fn clear(&mut self) -> io::Result<()> {
        self.term = 0;
        self.voted_for = None;
        self.log.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    #[test]
    fn in_memory_storage_persists_term_and_vote() {
        let mut storage = InMemoryStorage::new();
        
        storage.save_term_and_vote(5, Some(2)).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 5);
        assert_eq!(voted_for, Some(2));
    }
    
    #[test]
    fn in_memory_storage_appends_and_loads_log() {
        let mut storage = InMemoryStorage::new();
        
        let entries = vec![
            LogEntry::new(1, 1, vec![1, 2, 3]),
            LogEntry::new(1, 2, vec![4, 5, 6]),
        ];
        storage.append_entries(&entries).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].index, 1);
        assert_eq!(log[1].index, 2);
    }
    
    #[test]
    fn in_memory_storage_truncates_log() {
        let mut storage = InMemoryStorage::new();
        
        let entries = vec![
            LogEntry::new(1, 1, vec![1]),
            LogEntry::new(1, 2, vec![2]),
            LogEntry::new(1, 3, vec![3]),
        ];
        storage.append_entries(&entries).unwrap();
        
        storage.truncate_log_from(2).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].index, 1);
    }
    
    #[test]
    fn file_storage_persists_term_and_vote() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(7, Some(3)).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 7);
        assert_eq!(voted_for, Some(3));
    }
    
    #[test]
    fn file_storage_appends_and_loads_log() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        let entries = vec![
            LogEntry::new(1, 1, b"set key1 value1".to_vec()),
            LogEntry::new(1, 2, b"set key2 value2".to_vec()),
        ];
        storage.append_entries(&entries).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].command, b"set key1 value1".to_vec());
    }
    
    #[test]
    fn file_storage_survives_restart() {
        let dir = tempdir().unwrap();
        
        // first "session"
        {
            let mut storage = FileStorage::new(dir.path()).unwrap();
            storage.save_term_and_vote(10, Some(1)).unwrap();
            let entries = vec![LogEntry::new(10, 1, b"command".to_vec())];
            storage.append_entries(&entries).unwrap();
        }
        
        // "restart" - new storage instance
        {
            let storage = FileStorage::new(dir.path()).unwrap();
            let (term, voted_for) = storage.load_term_and_vote().unwrap();
            let log = storage.load_log().unwrap();
            
            assert_eq!(term, 10);
            assert_eq!(voted_for, Some(1));
            assert_eq!(log.len(), 1);
        }
    }
    
    #[test]
    fn file_storage_truncates_log() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        let entries = vec![
            LogEntry::new(1, 1, vec![1]),
            LogEntry::new(2, 2, vec![2]),
            LogEntry::new(3, 3, vec![3]),
        ];
        storage.append_entries(&entries).unwrap();
        
        storage.truncate_log_from(2).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].index, 1);
    }
}
