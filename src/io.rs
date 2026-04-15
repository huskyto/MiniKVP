
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::Write;
use std::io::Error;
use std::io::SeekFrom;
use std::string::FromUtf8Error;
use std::array::TryFromSliceError;

use thiserror::Error;

use crate::model::OnDiskEntry;


/// Reads the entire contents of the store file into a byte vector.
///
/// The file cursor is reset to the beginning before reading.
pub fn read_full_store(file: &mut File) -> Result<Vec<u8>, ParseError> {
    let mut buffer = Vec::new();
    file.seek(SeekFrom::Start(0)).map_err(ParseError::SeekError)?;
    file.read_to_end(&mut buffer).map_err(ParseError::ReadError)?;
    Ok(buffer)
}

/// Truncates the store file at `path`, effectively clearing all entries.
pub fn reset_store(path: &str) -> Result<(), Error> {
    File::create(path).map(|_| ())
}

/// Reads and decodes a single [`OnDiskEntry`] from `file` at the given byte `offset`.
pub fn get_at_offset(file: &mut File, offset: u64) -> Result<OnDiskEntry, ParseError> {
    let mut head_buffer = [0; 13];
    file.seek(SeekFrom::Start(offset)).map_err(ParseError::SeekError)?;
    file.read_exact(&mut head_buffer).map_err(ParseError::ReadError)?;

    let ks_data: [u8; 4] = head_buffer[..4].try_into()
            .map_err(ParseError::SliceCopyError)?;
    let ds_data: [u8; 8] = head_buffer[4..12].try_into()
            .map_err(ParseError::SliceCopyError)?;

    let key_size = u32::from_be_bytes(ks_data);
    let value_size = u64::from_be_bytes(ds_data);
    let flags = head_buffer[12];

    let file_size = file.metadata().map_err(ParseError::ReadError)?.len();
    let entry_size = (13 + key_size as u64)
            .checked_add(value_size)
            .ok_or(ParseError::SizeMismatch)?;
    if file_size < entry_size {
        return Err(ParseError::SizeMismatch)
    }

    let mut key_buffer = Vec::with_capacity(key_size as usize);
    file.take(key_size as u64).read_to_end(&mut key_buffer)
            .map_err(ParseError::ReadError)?;
    let key = String::from_utf8(key_buffer)?;

    let mut val_buffer = Vec::with_capacity(value_size as usize);
    file.take(value_size).read_to_end(&mut val_buffer)
            .map_err(ParseError::ReadError)?;

    Ok(OnDiskEntry {
        key_size,
        value_size,
        flags,
        key,
        value: val_buffer,
    })
}

/// Appends `data` to `file` and returns the byte offset where writing began.
pub fn append_to(file: &mut File, data: &[u8]) -> Result<u64, Error> {
    let len = file.metadata()?.len();
    file.write_all(data)?;
    Ok(len)
}

/// Encodes an [`OnDiskEntry`] into its binary on-disk representation.
///
/// The layout is (big-endian):
/// `[KeySize: 4 bytes][ValueSize: 8 bytes][Flags: 1 byte][Key bytes][Value bytes]`
pub fn encode_entry(entry: &OnDiskEntry) -> Vec<u8> {
    let ks_bytes = entry.key_size.to_be_bytes();
    let vs_bytes = entry.value_size.to_be_bytes();
    let flag_byte = entry.flags;
    let key_bytes = entry.key.as_bytes();
    let value_bytes = &entry.value;

    let size = ks_bytes.len() + vs_bytes.len() + 1 + key_bytes.len() + value_bytes.len();
    let mut entry = Vec::with_capacity(size);
    entry.extend_from_slice(&ks_bytes);
    entry.extend_from_slice(&vs_bytes);
    entry.push(flag_byte);
    entry.extend_from_slice(key_bytes);
    entry.extend_from_slice(value_bytes);

    entry
}

/// Errors that can occur while parsing or accessing the store file.
#[derive(Debug, Error)]
pub enum ParseError {
    /// The entry size declared in the header exceeds the actual file size.
    #[error("Entry size in header does not match the file size")]
    SizeMismatch,
    /// Failed to copy a byte slice into a fixed-size array.
    #[error("Failed to copy bytes into a fixed-size array: {0}")]
    SliceCopyError(TryFromSliceError),
    /// Failed to seek within the store file.
    #[error("Failed to seek within the store file: {0}")]
    SeekError(Error),
    /// Failed to read from the store file.
    #[error("Failed to read from the store file: {0}")]
    ReadError(Error),
    /// The key bytes are not valid UTF-8.
    #[error("Key contains invalid UTF-8: {0}")]
    StringParseError(#[from] FromUtf8Error),
}


#[cfg(test)]
mod tests {
    use std::fs::{File, OpenOptions};
    use std::io::Write;

    use super::*;
    use crate::model::{InMemoryEntry, NewEntry, OnDiskEntry};

    fn temp_path() -> String {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("unknown").replace("::", "_");
        format!("/tmp/minikvp_io_test_{}.db", name)
    }

    // Writes bytes to a fresh file at `path` and returns a readable handle to it.
    fn write_to_file(path: &str, data: &[u8]) -> File {
        File::create(path).unwrap().write_all(data).unwrap();
        OpenOptions::new().read(true).open(path).unwrap()
    }

    #[test]
    fn encode_entry_has_correct_byte_layout() {
        let entry = OnDiskEntry::from_new_entry(NewEntry {
            key: "ab".to_string(),
            value: vec![0xFF, 0x00],
        });

        let encoded = encode_entry(&entry);

        // KeySize: 2 as u32 big-endian
        assert_eq!(&encoded[0..4], &[0, 0, 0, 2]);
        // ValueSize: 2 as u64 big-endian
        assert_eq!(&encoded[4..12], &[0, 0, 0, 0, 0, 0, 0, 2]);
        // Flags: 0
        assert_eq!(encoded[12], 0);
        // Key bytes: "ab"
        assert_eq!(&encoded[13..15], b"ab");
        // Value bytes
        assert_eq!(&encoded[15..17], &[0xFF, 0x00]);
    }

    #[test]
    fn encode_tombstone_sets_deleted_flag() {
        let ime = InMemoryEntry { key: "k".to_string(), offset: 0 };
        let tombstone = OnDiskEntry::tombstone_for(&ime);

        let encoded = encode_entry(&tombstone);

        assert_eq!(encoded[12], 0x01, "flags byte should have deleted bit set");
        // ValueSize should be 0
        assert_eq!(&encoded[4..12], &[0u8; 8]);
    }

    #[test]
    fn encode_then_decode_round_trip() {
        let path = temp_path();
        let entry = OnDiskEntry::from_new_entry(NewEntry {
            key: "hello".to_string(),
            value: b"world".to_vec(),
        });

        let encoded = encode_entry(&entry);
        let mut file = write_to_file(&path, &encoded);
        let decoded = get_at_offset(&mut file, 0).unwrap();

        assert_eq!(decoded.key, "hello");
        assert_eq!(decoded.value, b"world");
        assert_eq!(decoded.flags, 0);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn encode_then_decode_tombstone_round_trip() {
        let path = temp_path();
        let ime = InMemoryEntry { key: "gone".to_string(), offset: 0 };
        let tombstone = OnDiskEntry::tombstone_for(&ime);

        let encoded = encode_entry(&tombstone);
        let mut file = write_to_file(&path, &encoded);
        let decoded = get_at_offset(&mut file, 0).unwrap();

        assert_eq!(decoded.key, "gone");
        assert!(decoded.value.is_empty());
        assert_eq!(decoded.flags & 0x01, 0x01);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn encode_then_decode_with_empty_value() {
        let path = temp_path();
        let entry = OnDiskEntry::from_new_entry(NewEntry {
            key: "empty".to_string(),
            value: vec![],
        });

        let encoded = encode_entry(&entry);
        let mut file = write_to_file(&path, &encoded);
        let decoded = get_at_offset(&mut file, 0).unwrap();

        assert_eq!(decoded.key, "empty");
        assert!(decoded.value.is_empty());

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn encode_then_decode_with_binary_value() {
        let path = temp_path();
        let value: Vec<u8> = (0u8..=255).collect();
        let entry = OnDiskEntry::from_new_entry(NewEntry {
            key: "bin".to_string(),
            value: value.clone(),
        });

        let encoded = encode_entry(&entry);
        let mut file = write_to_file(&path, &encoded);
        let decoded = get_at_offset(&mut file, 0).unwrap();

        assert_eq!(decoded.value, value);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn get_at_offset_reads_correct_entry_when_multiple_entries_exist() {
        let path = temp_path();
        let first = encode_entry(&OnDiskEntry::from_new_entry(NewEntry {
            key: "first".to_string(),
            value: b"one".to_vec(),
        }));
        let second = encode_entry(&OnDiskEntry::from_new_entry(NewEntry {
            key: "second".to_string(),
            value: b"two".to_vec(),
        }));

        let second_offset = first.len() as u64;
        let mut combined = first;
        combined.extend(second);

        let mut file = write_to_file(&path, &combined);

        let decoded_second = get_at_offset(&mut file, second_offset).unwrap();
        assert_eq!(decoded_second.key, "second");
        assert_eq!(decoded_second.value, b"two");

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn append_to_returns_offset_where_data_was_written() {
        let path = temp_path();
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)
            .unwrap();

        let offset_a = append_to(&mut file, b"hello").unwrap();
        let offset_b = append_to(&mut file, b"world").unwrap();

        assert_eq!(offset_a, 0);
        assert_eq!(offset_b, 5);

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn reset_store_produces_an_empty_file() {
        let path = temp_path();
        File::create(&path).unwrap().write_all(b"some data").unwrap();

        reset_store(&path).unwrap();

        let size = std::fs::metadata(&path).unwrap().len();
        assert_eq!(size, 0);

        std::fs::remove_file(&path).unwrap();
    }
}
