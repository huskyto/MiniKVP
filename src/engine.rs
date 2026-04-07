
use std::fs::File;
use std::collections::HashMap;

use crate::model::InMemoryEntry;


pub struct Engine {
    path: String,
    file_handle: File,
    entries: HashMap<String, InMemoryEntry>,
}
impl Engine {
            // LIFECYCLE //
    pub fn open(path: &str) -> Result<Engine, EngineError> {
        todo!()
    }
    pub fn close(&mut self) -> Result<(), EngineError> {
        todo!()
    }
            // ACTIONS //
    pub fn get(&self, key: &str) -> Result<Vec<u8>, EngineError> {
        todo!()
    }

    pub fn set(&mut self, key: &str, value: &[u8]) -> Result<(), EngineError> {
        todo!()
    }

    pub fn delete(&mut self, key: &str) -> Result<(), EngineError> {
        todo!()
    }

    pub fn get_all_keys(&self) -> Result<Vec<String>, EngineError> {
        todo!()
    }

    pub fn reset_store(&mut self) -> Result<(), EngineError> {
        todo!()
    }
}

pub enum EngineError {
    NoSuchKey,
    IOError
}
