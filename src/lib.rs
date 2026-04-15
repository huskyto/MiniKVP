//! MiniKVP is a minimal key-value store backed by an append-only log file.
//!
//! The main entry point is [`engine::Engine`].

pub mod model;
pub mod engine;

mod io;
