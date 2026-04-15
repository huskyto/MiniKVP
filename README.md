# MiniKVP

A minimal key-value store written in Rust, built as a learning project.

MiniKVP takes inspiration from [Bitcask](https://riak.com/assets/bitcask-intro.pdf), a log-structured key-value store. It intentionally omits most of Bitcask's features in favour of simplicity, while still being genuinely useful for small, low-stakes storage needs.

## How it works

The store is backed by a single append-only log file on disk. Every write adds a new entry to the end of the file; updates do not overwrite old data. On startup, MiniKVP replays the log from the beginning to reconstruct an in-memory index that maps each live key to its offset in the file. Reads use the index to seek directly to the right position.

Deletion appends a tombstone entry rather than removing data. The tombstone is replayed on the next startup to drop the key from the index.

Because the log is never compacted, the file grows with every update. The only way to reclaim space is `reset`, which truncates the file entirely.

### On-disk entry format

All multi-byte fields are big-endian.

```
 0               4               12      13
 ├───────────────┼───────────────┼───────┼──────────────────┬─────────────────┐
 │    KeySize    │   ValueSize   │ Flags │       Key        │      Value      │
 │   (4b, u32)   │   (8b, u64)   │ (1b)  │  (KeySize bytes) │(ValueSize bytes)│
 └───────────────┴───────────────┴───────┴──────────────────┴─────────────────┘
```

Flags bit `0x01` marks a tombstone (deleted) entry, which carries no value.

## Building

```
cargo build --release
```

The binary will be at `target/release/minikvp`.

## Usage

All commands accept a `--store` / `-s` flag to specify the store file (default: `minikvp.db`).

```
minikvp [--store <path>] <command>
```

### Commands

**get** — print the value stored for a key
```
minikvp get <key>
```

**set** — write a value for a key
```
minikvp set <key> <value>               # UTF-8 string value
minikvp set <key> --hex <hex>           # raw bytes as hex (e.g. FF00AB)
minikvp set <key> --file <path>         # read value from a file
```

**delete** — remove a key
```
minikvp delete <key>
```

**keys** — list all live keys in the store
```
minikvp keys
```

**reset** — erase all data in the store file
```
minikvp reset
```

**inspect** — print the raw on-disk structure of every log entry, including tombstones
```
minikvp inspect
```

Example output:
```
  KeySz |        ValSz | Flags  | Key        | Value
─────── | ──────────── | ────── | ────────── | ──────────────────────
      5 |            5 | 0x00   | hello      | world
      5 |            0 | 0x01   | hello      | [deleted]
      5 |           11 | 0x00   | hello      | hello again
      3 |            4 | 0x00   | bin        | [bin]
```

## Non-goals

These are deliberately out of scope:

- Concurrency
- Crash recovery
- Log compaction
- Replication
- Transactions
- Namespaces / buckets
- Integrity checks (CRCs etc.)

## AI use

AI tooling was used to assist with parts of this project; specifically documentation and test generation.

The system design, on-disk format, and all production code were written by hand.
