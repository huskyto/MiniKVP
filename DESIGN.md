
# MiniKVP

## What is MiniKVP.

MiniKVP is a basic implementation of a Key-Value Pair Store. It's designed to be simple to use and to understand, while still being useful in low-stakes use cases.

It is mainly created as a learning example.


## Design decisions.

### Persistent storage.

While volatile storage would be simpler, the project would then become simply a glorified HashMap. Adding persistence adds easily digestible complexity with considerable value.

### In-memory Index.

A compact index is kept in memory which includes the keys and an offset value, to quickly locate the entry in the actual store file.

This index is regenerated on startup by scanning through the log and replaying it until the end.

### Basic log store.

We will use a simplified version of log store for the data, keeping the entries minimal, and skipping housekeeping passes such as compaction.

Skipping compaction means that the file can grow very big if entries are updated a lot, since a new entry is appended with each update.

This can also slowdown startup, since the old logs have to be read to recreate the volatile representation. Startup time increases linearly with log size.

The only way to clear the store file is to call reset_store(), which will erase the file, and remove the entries.

### Single file.

All the store data will be kept in a single file, skipping partitioning entirely. While there are significant drawbacks to this approach, it will work well for small non-critical stores, and will keep complexity tame.

If partitioning is needed, multiple stores can be used, and each will live in its own file.

This also can cause issues if file size limits are reached.

### No Namespaces.

Namespaces/Buckets can be simulated in other ways. For example, multiple stores can be used, or keys can be prepended.

They are left out for implementation simplicity.

### Deletion.

For deletion, the in-memory entry is removed, and a tombstone is appended to the log.

For the store file; each entry includes a "deleted" flag. For deletions, a tombstone entry is added, which has the flag set to "true", and ValueSize is zero.


## Data model.

### Entries.

No explicit limit is set by MiniKVP on either keys or values.

- Key: String.
- Value: [u8].

### Log entries.

#### In memory.

- Key.
- Offset. (offset in the file to read the entry from)

#### In disk.

File is append-only.
Updates create a new log entry.

- KeySize: u64.
- ValueSize: u64.
- Deleted: bool.
- Key: String.
- Value: [u8].


## Operations.

All operations return a Result, which includes simple error details, if one occurred.

- get(key)
- set(key, value)
- delete(key)
- get_all_keys()
- reset_store()


## Non-Goals.

These are functions that are useful in a KVP Store, but will not be added to MiniKVP:

- Concurrency.
- Crash recovery.
- Log compaction.
- Replication.
- Transactions.
- Namespaces/Buckets.

