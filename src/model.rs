

pub struct InMemoryEntry {
    pub key: String,
    pub offset: u64
}

#[derive(Debug)]
pub struct OnDiskEntry {
    pub key_size: u32,
    pub value_size: u64,
    pub flags: u8,
    pub key: String,
    pub value: Vec<u8>
}
impl OnDiskEntry {
    pub fn from_new_entry(entry: NewEntry) -> Self {
        Self {
            key_size: entry.key.len() as u32,
            value_size: entry.value.len() as u64,
            flags: 0,
            key: entry.key,
            value: entry.value,
        }
    }
    pub fn tombstone_for(ime: &InMemoryEntry) -> Self {
        Self {
            key_size: ime.key.len() as u32,
            value_size: 0,
            flags: 0x01,
            key: ime.key.clone(),
            value: vec![],
        }
    }
}

pub struct NewEntry {
    pub key: String,
    pub value: Vec<u8>
}
