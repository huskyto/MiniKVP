
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::collections::HashMap;

use crate::io;
use crate::model::NewEntry;
use crate::model::OnDiskEntry;
use crate::model::InMemoryEntry;


pub struct Engine {
    path: String,
    file_handle: File,
    entries: HashMap<String, InMemoryEntry>,
}
impl Engine {
            // LIFECYCLE //
    pub fn open(path: &str) -> Result<Engine, EngineError> {
        let p = Path::new(path);
        if !p.exists() {
            File::create(path)
                .map_err(|_| EngineError::IOError)?;
        }
        else if !p.is_file() {
            return Err(EngineError::IOError);
        }

        let mut file = OpenOptions::new()
                .read(true)
                .append(true)
                .open(path)
                .map_err(|_| EngineError::IOError)?;

        let contents = io::read_full_store(&mut file)
                .map_err(|_| EngineError::IOError)?;
        let entries = Self::replay_store(&contents)?;

        let engine = Engine {
            path: path.to_string(),
            file_handle: file,
            entries,
        };

        Ok(engine)
    }
    pub fn close(&mut self) -> Result<(), EngineError> {
        self.file_handle.unlock()
                .map_err(|_| EngineError::IOError)
    }
            // ACTIONS //
    pub fn get(&mut self, key: &str) -> Result<Vec<u8>, EngineError> {
        let offset = match self.entries.get(key) {
            Some(ime) => {
                ime.offset
            },
            None => return Err(EngineError::NoSuchKey),
        };

        let ode = io::get_at_offset(&mut self.file_handle, offset)
                .map_err(|_| EngineError::IOError)?;

        Ok(ode.value)
    }

    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), EngineError> {
        if let Ok(cv) = self.get(key) && cv == value {
            // Value has not changed. Avoid creating new entry.
            return Ok(())
        };

        let od_entry = OnDiskEntry::from_new_entry(
            NewEntry { key: key.to_string(), value: value.to_vec() }
        );
        let encoded = io::encode_entry(&od_entry);
        let offset = io::append_to(&mut self.file_handle, &encoded)
                .map_err(|_| EngineError::IOError)?;

        let im_entry = InMemoryEntry {
            key: key.to_string(), offset,
        };
        self.entries.insert(key.to_string(), im_entry);

        Ok(())
    }

    pub fn delete(&mut self, key: &str) -> Result<(), EngineError> {
        let removed = self.entries.remove(key);
        let ime = match removed {
            Some(entry) => entry,
            None => return Err(EngineError::NoSuchKey),
        };

        let tombstone = OnDiskEntry::tombstone_for(&ime);
        let encoded = io::encode_entry(&tombstone);
        io::append_to(&mut self.file_handle, &encoded)
                .map_err(|_| EngineError::IOError)?;

        Ok(())
    }

    pub fn get_all_keys(&self) -> Result<Vec<String>, EngineError> {
        let keys = self.entries.keys().cloned().collect();
        Ok(keys)
    }

    pub fn reset_store(&mut self) -> Result<(), EngineError> {
        self.file_handle.unlock()
                .map_err(|_| EngineError::IOError)?;
        io::reset_store(&self.path)
                .map_err(|_| EngineError::IOError)?;
        self.file_handle.lock()
                .map_err(|_| EngineError::IOError)?;

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
                    .map_err(|_| EngineError::StoreError)?;
            let ds_data: [u8; 8] = data[offset + 4.. offset + 12].try_into()
                    .map_err(|_| EngineError::StoreError)?;
            let key_size = u32::from_be_bytes(ks_data);
            let value_size = u64::from_be_bytes(ds_data);
            let flags = data[offset + 12];

            let entry_size = (13 + key_size as u64)
                    .checked_add(value_size)
                    .ok_or(EngineError::StoreError)?;
            if data.len() < entry_size as usize + offset {
                return Err(EngineError::TruncatedStore)
            }

            let key_data = &data[offset + 13..offset + 13 + key_size as usize];
            let key = String::from_utf8(key_data.to_vec())
                    .map_err(|_| EngineError::StoreError)?;

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

#[derive(Debug)]
pub enum EngineError {
    NoSuchKey,
    IOError,
    StoreError,
    TruncatedStore
}
