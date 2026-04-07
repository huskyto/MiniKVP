
use std::io::Error;

use crate::model::OnDiskEntry;


pub fn read_store_str(path: &str) -> String {
    todo!()
}
pub fn reset_store(path: &str) -> Result<(), Error> {
    todo!()
}
pub fn read_at_offset(path: &str, offset: u64) -> Result<String, Error> {
    todo!()
}
pub fn append_to(path: &str) -> Result<(), Error> {
    todo!()
}

pub fn parse_entry(data: &str) -> Result<OnDiskEntry, ParseError> {
    todo!()
}
pub fn encode_entry(entry: &OnDiskEntry) -> String {
    todo!()
}

pub enum ParseError {
    SizeMismatch,
    KeyEncodeError,
    SizeParseError
}
