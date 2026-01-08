//! # comprehensive storage tests
//!
//! why: verify all storage scenarios work correctly
//! relations: tests raft-storage crate
//! what: persistence, crash recovery, concurrent access, edge cases

use raft_storage::{Storage, FileStorage, InMemoryStorage};
use raft_core::LogEntry;
use tempfile::tempdir;
use std::fs;

// =============================================================================
// SECTION 1: IN-MEMORY STORAGE TESTS
// =============================================================================

mod in_memory_basic {
    use super::*;

    #[test]
    fn new_storage_has_default_values() {
        let storage = InMemoryStorage::new();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 0);
        assert_eq!(voted_for, None);
    }

    #[test]
    fn save_and_load_term_and_vote() {
        let mut storage = InMemoryStorage::new();
        
        storage.save_term_and_vote(5, Some(3)).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 5);
        assert_eq!(voted_for, Some(3));
    }

    #[test]
    fn save_voted_for_none() {
        let mut storage = InMemoryStorage::new();
        
        storage.save_term_and_vote(10, None).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 10);
        assert_eq!(voted_for, None);
    }

    #[test]
    fn overwrite_term_and_vote() {
        let mut storage = InMemoryStorage::new();
        
        storage.save_term_and_vote(1, Some(1)).unwrap();
        storage.save_term_and_vote(5, Some(3)).unwrap();
        
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        assert_eq!(term, 5);
        assert_eq!(voted_for, Some(3));
    }
}

mod in_memory_log {
    use super::*;

    #[test]
    fn new_storage_has_empty_log() {
        let storage = InMemoryStorage::new();
        let log = storage.load_log().unwrap();
        
        assert!(log.is_empty());
    }

    #[test]
    fn append_single_entry() {
        let mut storage = InMemoryStorage::new();
        let entries = vec![LogEntry::new(1, 1, b"cmd1".to_vec())];
        
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].term, 1);
        assert_eq!(log[0].index, 1);
        assert_eq!(log[0].command, b"cmd1".to_vec());
    }

    #[test]
    fn append_multiple_entries() {
        let mut storage = InMemoryStorage::new();
        let entries = vec![
            LogEntry::new(1, 1, b"cmd1".to_vec()),
            LogEntry::new(1, 2, b"cmd2".to_vec()),
            LogEntry::new(2, 3, b"cmd3".to_vec()),
        ];
        
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn append_in_batches() {
        let mut storage = InMemoryStorage::new();
        
        storage.append_entries(&[LogEntry::new(1, 1, b"a".to_vec())]).unwrap();
        storage.append_entries(&[LogEntry::new(1, 2, b"b".to_vec())]).unwrap();
        storage.append_entries(&[LogEntry::new(1, 3, b"c".to_vec())]).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn append_empty_entries() {
        let mut storage = InMemoryStorage::new();
        
        storage.append_entries(&[]).unwrap();
        
        let log = storage.load_log().unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn truncate_log_from_index() {
        let mut storage = InMemoryStorage::new();
        let entries = vec![
            LogEntry::new(1, 1, b"a".to_vec()),
            LogEntry::new(1, 2, b"b".to_vec()),
            LogEntry::new(1, 3, b"c".to_vec()),
            LogEntry::new(1, 4, b"d".to_vec()),
        ];
        storage.append_entries(&entries).unwrap();
        
        storage.truncate_log_from(3).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].index, 1);
        assert_eq!(log[1].index, 2);
    }

    #[test]
    fn truncate_all_entries() {
        let mut storage = InMemoryStorage::new();
        let entries = vec![LogEntry::new(1, 1, b"a".to_vec())];
        storage.append_entries(&entries).unwrap();
        
        storage.truncate_log_from(1).unwrap();
        
        let log = storage.load_log().unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn truncate_empty_log() {
        let mut storage = InMemoryStorage::new();
        
        storage.truncate_log_from(5).unwrap();
        
        let log = storage.load_log().unwrap();
        assert!(log.is_empty());
    }

    #[test]
    fn clear_resets_all_state() {
        let mut storage = InMemoryStorage::new();
        storage.save_term_and_vote(10, Some(5)).unwrap();
        storage.append_entries(&[LogEntry::new(1, 1, b"cmd".to_vec())]).unwrap();
        
        storage.clear().unwrap();
        
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(term, 0);
        assert_eq!(voted_for, None);
        assert!(log.is_empty());
    }
}

// =============================================================================
// SECTION 2: FILE STORAGE TESTS
// =============================================================================

mod file_storage_basic {
    use super::*;

    #[test]
    fn create_storage_creates_directory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("raft_data");
        
        FileStorage::new(&path).unwrap();
        
        assert!(path.exists());
    }

    #[test]
    fn new_storage_has_default_values() {
        let dir = tempdir().unwrap();
        let storage = FileStorage::new(dir.path()).unwrap();
        
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 0);
        assert_eq!(voted_for, None);
    }

    #[test]
    fn save_and_load_term_and_vote() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(7, Some(3)).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, 7);
        assert_eq!(voted_for, Some(3));
    }

    #[test]
    fn save_creates_meta_file() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(5, Some(2)).unwrap();
        
        assert!(dir.path().join("meta.json").exists());
    }
}

mod file_storage_log {
    use super::*;

    #[test]
    fn append_creates_log_file() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        let entries = vec![LogEntry::new(1, 1, b"cmd".to_vec())];
        storage.append_entries(&entries).unwrap();
        
        assert!(dir.path().join("log.json").exists());
    }

    #[test]
    fn append_and_load_entries() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        let entries = vec![
            LogEntry::new(1, 1, b"SET key1 value1".to_vec()),
            LogEntry::new(1, 2, b"SET key2 value2".to_vec()),
        ];
        storage.append_entries(&entries).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].command, b"SET key1 value1".to_vec());
    }

    #[test]
    fn truncate_log() {
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

// =============================================================================
// SECTION 3: CRASH RECOVERY / PERSISTENCE TESTS
// =============================================================================

mod crash_recovery {
    use super::*;

    #[test]
    fn term_survives_restart() {
        let dir = tempdir().unwrap();
        
        // first "session"
        {
            let mut storage = FileStorage::new(dir.path()).unwrap();
            storage.save_term_and_vote(10, Some(5)).unwrap();
        }
        
        // "restart" - new storage instance
        {
            let storage = FileStorage::new(dir.path()).unwrap();
            let (term, voted_for) = storage.load_term_and_vote().unwrap();
            
            assert_eq!(term, 10);
            assert_eq!(voted_for, Some(5));
        }
    }

    #[test]
    fn log_survives_restart() {
        let dir = tempdir().unwrap();
        
        // first "session"
        {
            let mut storage = FileStorage::new(dir.path()).unwrap();
            let entries = vec![
                LogEntry::new(1, 1, b"cmd1".to_vec()),
                LogEntry::new(1, 2, b"cmd2".to_vec()),
            ];
            storage.append_entries(&entries).unwrap();
        }
        
        // "restart"
        {
            let storage = FileStorage::new(dir.path()).unwrap();
            let log = storage.load_log().unwrap();
            
            assert_eq!(log.len(), 2);
            assert_eq!(log[0].command, b"cmd1".to_vec());
            assert_eq!(log[1].command, b"cmd2".to_vec());
        }
    }

    #[test]
    fn multiple_restarts_preserve_state() {
        let dir = tempdir().unwrap();
        
        // session 1
        {
            let mut storage = FileStorage::new(dir.path()).unwrap();
            storage.save_term_and_vote(1, Some(1)).unwrap();
            storage.append_entries(&[LogEntry::new(1, 1, b"a".to_vec())]).unwrap();
        }
        
        // session 2
        {
            let mut storage = FileStorage::new(dir.path()).unwrap();
            storage.save_term_and_vote(2, Some(2)).unwrap();
            storage.append_entries(&[LogEntry::new(2, 2, b"b".to_vec())]).unwrap();
        }
        
        // session 3 - verify
        {
            let storage = FileStorage::new(dir.path()).unwrap();
            let (term, voted_for) = storage.load_term_and_vote().unwrap();
            let log = storage.load_log().unwrap();
            
            assert_eq!(term, 2);
            assert_eq!(voted_for, Some(2));
            assert_eq!(log.len(), 2);
        }
    }

    #[test]
    fn clear_removes_all_files() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(5, Some(3)).unwrap();
        storage.append_entries(&[LogEntry::new(1, 1, b"cmd".to_vec())]).unwrap();
        
        storage.clear().unwrap();
        
        // files should be gone
        assert!(!dir.path().join("meta.json").exists());
        assert!(!dir.path().join("log.json").exists());
    }

    #[test]
    fn load_after_clear_returns_defaults() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(5, Some(3)).unwrap();
        storage.append_entries(&[LogEntry::new(1, 1, b"cmd".to_vec())]).unwrap();
        storage.clear().unwrap();
        
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(term, 0);
        assert_eq!(voted_for, None);
        assert!(log.is_empty());
    }
}

// =============================================================================
// SECTION 4: ATOMIC WRITE TESTS
// =============================================================================

mod atomic_writes {
    use super::*;

    #[test]
    fn meta_file_is_valid_json() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(5, Some(2)).unwrap();
        
        let contents = fs::read_to_string(dir.path().join("meta.json")).unwrap();
        let _: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    }

    #[test]
    fn log_file_is_valid_json() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.append_entries(&[LogEntry::new(1, 1, b"cmd".to_vec())]).unwrap();
        
        let contents = fs::read_to_string(dir.path().join("log.json")).unwrap();
        let _: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
    }

    #[test]
    fn no_temp_files_remain() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        
        storage.save_term_and_vote(5, Some(2)).unwrap();
        storage.append_entries(&[LogEntry::new(1, 1, b"cmd".to_vec())]).unwrap();
        
        // temp files should be cleaned up
        assert!(!dir.path().join("meta.tmp").exists());
        assert!(!dir.path().join("log.tmp").exists());
    }
}

// =============================================================================
// SECTION 5: EDGE CASES
// =============================================================================

mod edge_cases {
    use super::*;

    #[test]
    fn large_log_entry() {
        let mut storage = InMemoryStorage::new();
        
        // 1MB command
        let large_command = vec![0u8; 1024 * 1024];
        let entries = vec![LogEntry::new(1, 1, large_command.clone())];
        
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(log[0].command.len(), 1024 * 1024);
    }

    #[test]
    fn many_log_entries() {
        let mut storage = InMemoryStorage::new();
        
        let entries: Vec<LogEntry> = (1..=1000)
            .map(|i| LogEntry::new(1, i, format!("cmd{}", i).into_bytes()))
            .collect();
        
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(log.len(), 1000);
    }

    #[test]
    fn binary_command_data() {
        let mut storage = InMemoryStorage::new();
        
        let binary_data = vec![0x00, 0xFF, 0x7F, 0x80, 0xFE];
        let entries = vec![LogEntry::new(1, 1, binary_data.clone())];
        
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(log[0].command, binary_data);
    }

    #[test]
    fn unicode_in_command() {
        let mut storage = InMemoryStorage::new();
        
        let unicode_cmd = "SET 键 值 🎉".as_bytes().to_vec();
        let entries = vec![LogEntry::new(1, 1, unicode_cmd.clone())];
        
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        
        assert_eq!(log[0].command, unicode_cmd);
    }

    #[test]
    fn very_high_term_number() {
        let mut storage = InMemoryStorage::new();
        
        storage.save_term_and_vote(u64::MAX, Some(u64::MAX)).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        
        assert_eq!(term, u64::MAX);
        assert_eq!(voted_for, Some(u64::MAX));
    }

    #[test]
    fn high_log_index() {
        let mut storage = InMemoryStorage::new();
        
        let entries = vec![LogEntry::new(1, u64::MAX, b"cmd".to_vec())];
        storage.append_entries(&entries).unwrap();
        
        let log = storage.load_log().unwrap();
        assert_eq!(log[0].index, u64::MAX);
    }
}

// =============================================================================
// SECTION 6: STORAGE TRAIT POLYMORPHISM
// =============================================================================

mod trait_polymorphism {
    use super::*;

    fn test_storage_impl<S: Storage>(storage: &mut S) {
        // save and load term
        storage.save_term_and_vote(5, Some(2)).unwrap();
        let (term, voted_for) = storage.load_term_and_vote().unwrap();
        assert_eq!(term, 5);
        assert_eq!(voted_for, Some(2));
        
        // append and load log
        let entries = vec![LogEntry::new(1, 1, b"cmd".to_vec())];
        storage.append_entries(&entries).unwrap();
        let log = storage.load_log().unwrap();
        assert_eq!(log.len(), 1);
        
        // truncate
        storage.truncate_log_from(1).unwrap();
        let log = storage.load_log().unwrap();
        assert!(log.is_empty());
        
        // clear
        storage.clear().unwrap();
        let (term, _) = storage.load_term_and_vote().unwrap();
        assert_eq!(term, 0);
    }

    #[test]
    fn in_memory_implements_trait() {
        let mut storage = InMemoryStorage::new();
        test_storage_impl(&mut storage);
    }

    #[test]
    fn file_storage_implements_trait() {
        let dir = tempdir().unwrap();
        let mut storage = FileStorage::new(dir.path()).unwrap();
        test_storage_impl(&mut storage);
    }
}
