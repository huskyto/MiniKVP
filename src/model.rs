

pub struct InMemoryEntry {
    pub key: String,
    pub offset: u64
}

pub struct OnDiskEntry {
    pub key_size: u32,
    pub value_size: u64,
    pub flags: u8,
    pub key: String,
    pub value: Vec<u8>
}
