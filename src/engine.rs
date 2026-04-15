
use std::path::Path;
use std::fs::File;
use std::fs::OpenOptions;
use std::collections::HashMap;
use std::array::TryFromSliceError;

use thiserror::Error;

use crate::io;
use crate::model::InMemoryEntry;
use crate::model::NewEntry;
use crate::model::OnDiskEntry;


/// The core key-value store engine.
///
/// `Engine` manages an append-only log file on disk and a compact in-memory
/// index that maps each key to the byte offset of its latest entry in the log.
///
/// The index is reconstructed by replaying the log on [`Engine::open`].
///
/// # Example
///
/// ```no_run
/// use minikvp::engine::Engine;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut engine = Engine::open("my_store.kvp")?;
///     engine.set("hello", b"world")?;
///     let value = engine.get("hello")?;
///     assert_eq!(value, b"world");
///     engine.close()?;
///     Ok(())
/// }
/// ```
pub struct Engine {
    path: String,
    file_handle: File,
    entries: HashMap<String, InMemoryEntry>,
}

impl Engine {
            // LIFECYCLE //

    /// Opens the store at `path`, creating the file if it does not exist.
    ///
    /// The in-memory index is populated by replaying the on-disk log from the beginning.
    /// Startup time grows linearly with the size of the log file.
    ///
    /// Returns an [`EngineError`] if the file cannot be opened or the log is malformed.
    pub fn open(path: &str) -> Result<Engine, EngineError> {
        let p = Path::new(path);
        if !p.exists() {
            File::create(path)?;
        }
        else if !p.is_file() {
            return Err(EngineError::Io(std::io::Error::other("path exists but is not a file")));
        }

        let mut file = OpenOptions::new()
                .read(true)
                .append(true)
                .open(path)?;

        let contents = io::read_full_store(&mut file)
                .map_err(|e| EngineError::CorruptStore(e.to_string()))?;
        let entries = Self::replay_store(&contents)?;

        let engine = Engine {
            path: path.to_string(),
            file_handle: file,
            entries,
        };

        Ok(engine)
    }

    /// Releases the file lock held by this engine.
    ///
    /// Call this when you are done with the store to cleanly release the lock.
    pub fn close(&mut self) -> Result<(), EngineError> {
        self.file_handle.unlock()?;
        Ok(())
    }

            // ACTIONS //

    /// Returns the value stored for `key`.
    ///
    /// Returns [`EngineError::NoSuchKey`] if the key does not exist or has been deleted.
    pub fn get(&mut self, key: &str) -> Result<Vec<u8>, EngineError> {
        let offset = match self.entries.get(key) {
            Some(ime) => {
                ime.offset
            },
            None => return Err(EngineError::NoSuchKey),
        };

        let ode = io::get_at_offset(&mut self.file_handle, offset)
                .map_err(|e| EngineError::CorruptStore(e.to_string()))?;

        Ok(ode.value)
    }

    /// Writes `value` for `key`, appending a new log entry to the store file.
    ///
    /// If `key` already holds the same `value`, the write is skipped to avoid
    /// creating unnecessary log entries.
    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), EngineError> {
        if let Ok(cv) = self.get(key) && cv == value {
            // Value has not changed. Avoid creating new entry.
            return Ok(())
        };

        let od_entry = OnDiskEntry::from_new_entry(
            NewEntry { key: key.to_string(), value: value.to_vec() }
        );
        let encoded = io::encode_entry(&od_entry);
        let offset = io::append_to(&mut self.file_handle, &encoded)?;

        let im_entry = InMemoryEntry {
            key: key.to_string(), offset,
        };
        self.entries.insert(key.to_string(), im_entry);

        Ok(())
    }

    /// Removes `key` from the store.
    ///
    /// Deletion appends a tombstone entry to the log and removes the key from
    /// the in-memory index. Returns [`EngineError::NoSuchKey`] if the key does
    /// not exist.
    pub fn delete(&mut self, key: &str) -> Result<(), EngineError> {
        let removed = self.entries.remove(key);
        let ime = match removed {
            Some(entry) => entry,
            None => return Err(EngineError::NoSuchKey),
        };

        let tombstone = OnDiskEntry::tombstone_for(&ime);
        let encoded = io::encode_entry(&tombstone);
        io::append_to(&mut self.file_handle, &encoded)?;

        Ok(())
    }

    /// Returns a list of all keys currently present in the store.
    pub fn get_all_keys(&self) -> Result<Vec<String>, EngineError> {
        let keys = self.entries.keys().cloned().collect();
        Ok(keys)
    }

    /// Erases all data in the store file and clears the in-memory index.
    ///
    /// This is the only way to reclaim space from old log entries, since
    /// MiniKVP does not perform log compaction.
    pub fn reset_store(&mut self) -> Result<(), EngineError> {
        self.file_handle.unlock()?;
        io::reset_store(&self.path)?;
        self.file_handle.lock()?;

        self.entries.clear();

        Ok(())
    }

    fn replay_store(data: &[u8]) -> Result<HashMap<String, InMemoryEntry>, EngineError> {
        let mut res = HashMap::new();
        if data.is_empty() {
            return Ok(res);
        }
        else if data.len() < 13 {
            return Err(EngineError::TruncatedStore)
        };

        let mut offset = 0;
        while offset < data.len() {
            let ks_data: [u8; 4] = data[offset..offset + 4].try_into()
                    .map_err(|e: TryFromSliceError| EngineError::CorruptStore(e.to_string()))?;
            let ds_data: [u8; 8] = data[offset + 4.. offset + 12].try_into()
                    .map_err(|e: TryFromSliceError| EngineError::CorruptStore(e.to_string()))?;
            let key_size = u32::from_be_bytes(ks_data);
            let value_size = u64::from_be_bytes(ds_data);
            let flags = data[offset + 12];

            let entry_size = (13 + key_size as u64)
                    .checked_add(value_size)
                    .ok_or_else(|| EngineError::CorruptStore("integer overflow computing entry size".to_string()))?;
            if data.len() < entry_size as usize + offset {
                return Err(EngineError::TruncatedStore)
            }

            let key_data = &data[offset + 13..offset + 13 + key_size as usize];
            let key = String::from_utf8(key_data.to_vec())
                    .map_err(|e| EngineError::CorruptStore(e.to_string()))?;

            if (flags & 0x01) == 0x01 {
                res.remove(&key);
            }
            else {
                let ime = InMemoryEntry {
                    key: key.clone(), offset: offset as u64
                };
                res.insert(key, ime);
            }

            offset += 13 + key_size as usize + value_size as usize;
        }

        Ok(res)
    }
}

/// Errors that can be returned by [`Engine`] operations.
#[derive(Debug, Error)]
pub enum EngineError {
    /// No entry exists for the given key.
    #[error("No entry found for the given key")]
    NoSuchKey,
    /// An IO error occurred while accessing the store file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// The store file contains unexpected or invalid data.
    #[error("Store file is corrupted: {0}")]
    CorruptStore(String),
    /// The store file ends mid-entry.
    #[error("Store file is truncated")]
    TruncatedStore,
}


#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path() -> String {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unknown").replace("::", "_");
        let path = format!("/tmp/minikvp_engine_test_{}.db", name);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn get_nonexistent_key_returns_no_such_key() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        assert!(matches!(engine.get("missing"), Err(EngineError::NoSuchKey)));

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn set_then_get_returns_the_stored_value() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("key", b"value").unwrap();

        assert_eq!(engine.get("key").unwrap(), b"value");

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn setting_same_value_again_does_not_grow_the_file() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("key", b"value").unwrap();
        let size_after_first = std::fs::metadata(&path).unwrap().len();

        engine.set("key", b"value").unwrap();
        let size_after_second = std::fs::metadata(&path).unwrap().len();

        assert_eq!(size_after_first, size_after_second);

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn setting_different_value_for_same_key_returns_new_value() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("key", b"old").unwrap();
        engine.set("key", b"new").unwrap();

        assert_eq!(engine.get("key").unwrap(), b"new");

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn delete_removes_key_so_get_returns_no_such_key() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("key", b"value").unwrap();
        engine.delete("key").unwrap();

        assert!(matches!(engine.get("key"), Err(EngineError::NoSuchKey)));

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn delete_nonexistent_key_returns_no_such_key() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        assert!(matches!(engine.delete("missing"), Err(EngineError::NoSuchKey)));

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn deleted_key_does_not_appear_in_get_all_keys() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("keep", b"1").unwrap();
        engine.set("drop", b"2").unwrap();
        engine.delete("drop").unwrap();

        let keys = engine.get_all_keys().unwrap();
        assert_eq!(keys, vec!["keep"]);

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn get_all_keys_on_empty_store_returns_empty_vec() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        assert!(engine.get_all_keys().unwrap().is_empty());

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn get_all_keys_returns_every_set_key() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("a", b"1").unwrap();
        engine.set("b", b"2").unwrap();
        engine.set("c", b"3").unwrap();

        let mut keys = engine.get_all_keys().unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b", "c"]);

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn reset_store_clears_all_entries_from_memory_and_disk() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("a", b"1").unwrap();
        engine.set("b", b"2").unwrap();
        engine.reset_store().unwrap();

        assert!(engine.get_all_keys().unwrap().is_empty());
        assert!(matches!(engine.get("a"), Err(EngineError::NoSuchKey)));
        assert_eq!(std::fs::metadata(&path).unwrap().len(), 0);

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn set_and_get_full_byte_range_value() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        let value: Vec<u8> = (0u8..=255).collect();
        engine.set("bin", &value).unwrap();

        assert_eq!(engine.get("bin").unwrap(), value);

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn key_can_contain_unicode_characters() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("héllo wörld", b"value").unwrap();

        assert_eq!(engine.get("héllo wörld").unwrap(), b"value");

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn key_can_be_set_again_after_deletion() {
        let path = temp_path();
        let mut engine = Engine::open(&path).unwrap();

        engine.set("key", b"first").unwrap();
        engine.delete("key").unwrap();
        engine.set("key", b"second").unwrap();

        assert_eq!(engine.get("key").unwrap(), b"second");

        engine.close().unwrap();
        std::fs::remove_file(&path).unwrap();
    }
}
