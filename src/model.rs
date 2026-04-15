
/// An entry stored in the in-memory index.
///
/// Holds just enough information to locate the full entry on disk.
pub struct InMemoryEntry {
    /// The entry key.
    pub key: String,
    /// Byte offset in the store file where the corresponding [`OnDiskEntry`] begins.
    pub offset: u64,
}

/// The full representation of an entry as laid out in the store file.
///
/// On-disk layout (big-endian):
/// ```text
/// [KeySize: 4 bytes][ValueSize: 8 bytes][Flags: 1 byte][Key: KeySize bytes][Value: ValueSize bytes]
/// ```
#[derive(Debug)]
pub struct OnDiskEntry {
    /// Length of the key in bytes.
    pub key_size: u32,
    /// Length of the value in bytes. Zero for tombstone entries.
    pub value_size: u64,
    /// Entry flags. Bit `0x01` is set for tombstone (deleted) entries.
    pub flags: u8,
    /// The entry key.
    pub key: String,
    /// The entry value. Empty for tombstone entries.
    pub value: Vec<u8>,
}

impl OnDiskEntry {
    /// Creates a new [`OnDiskEntry`] from a [`NewEntry`].
    pub fn from_new_entry(entry: NewEntry) -> Self {
        Self {
            key_size: entry.key.len() as u32,
            value_size: entry.value.len() as u64,
            flags: 0,
            key: entry.key,
            value: entry.value,
        }
    }

    /// Creates a tombstone entry for the given [`InMemoryEntry`].
    ///
    /// Tombstones have `flags & 0x01 == 1` and a zero-length value, signaling
    /// that the key has been deleted.
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

/// A key-value pair supplied by the caller for writing to the store.
pub struct NewEntry {
    /// The entry key.
    pub key: String,
    /// The entry value.
    pub value: Vec<u8>,
}
