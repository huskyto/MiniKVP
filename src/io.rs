
use std::fs::File;
use std::io::Seek;
use std::io::Read;
use std::io::Error;
use std::io::Write;
use std::io::SeekFrom;

use crate::model::OnDiskEntry;


pub fn read_full_store(file: &mut File) -> Result<Vec<u8>, ParseError> {
    let mut buffer = Vec::new();
    file.seek(SeekFrom::Start(0)).map_err(|_| ParseError::SeekError)?;
    file.read_to_end(&mut buffer).map_err(|_| ParseError::ReadError)?;
    Ok(buffer)
}
pub fn reset_store(path: &str) -> Result<(), Error> {
    File::create(path).map(|_| ())
}
pub fn get_at_offset(file: &mut File, offset: u64) -> Result<OnDiskEntry, ParseError> {
    let mut head_buffer = [0; 13];
    file.seek(SeekFrom::Start(offset)).map_err(|_| ParseError::SeekError)?;
    file.read_exact(&mut head_buffer).map_err(|_| ParseError::ReadError)?;

    let ks_data: [u8; 4] = head_buffer[..4].try_into()
            .map_err(|_| ParseError::SliceCopyError)?;
    let ds_data: [u8; 8] = head_buffer[4..12].try_into()
            .map_err(|_| ParseError::SliceCopyError)?;

    let key_size = u32::from_be_bytes(ks_data);
    let value_size = u64::from_be_bytes(ds_data);
    let flags = head_buffer[12];

    let file_size = file.metadata().map_err(|_| ParseError::ReadError)?.len();
    let entry_size = (13 + key_size as u64)
            .checked_add(value_size)
            .ok_or(ParseError::SizeMismatch)?;
    if file_size < entry_size {
        return Err(ParseError::SizeMismatch)
    }

    let mut key_buffer = Vec::with_capacity(key_size as usize);
    file.take(key_size as u64).read_to_end(&mut key_buffer)
            .map_err(|_| ParseError::ReadError)?;
    let key = String::from_utf8(key_buffer)
            .map_err(|_| ParseError::StringParseError)?;

    let mut val_buffer = Vec::with_capacity(value_size as usize);
    file.take(value_size).read_to_end(&mut val_buffer)
            .map_err(|_| ParseError::ReadError)?;

    Ok(OnDiskEntry {
        key_size,
        value_size,
        flags,
        key,
        value: val_buffer,
    })
}
pub fn append_to(file: &mut File, data: &[u8]) -> Result<u64, Error> {
    let len = file.metadata()?.len();
    file.write_all(data)?;
    Ok(len)
}
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

#[derive(Debug)]
pub enum ParseError {
    SizeMismatch,
    KeyEncodeError,
    SizeParseError,
    SliceCopyError,
    SeekError,
    ReadError,
    StringParseError
}

