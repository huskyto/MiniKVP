
use minikvp::engine::Engine;
use minikvp::engine::EngineError;


// Opens the store at `path`, runs `body`, then closes it.
// Returns the engine after `body` runs so the caller can make final assertions.
fn with_engine<F: FnOnce(&mut Engine)>(path: &str, body: F) -> Engine {
    let mut engine = Engine::open(path).unwrap();
    body(&mut engine);
    engine
}

fn temp_path(name: &str) -> String {
    let path = format!("/tmp/minikvp_inttest_{}.db", name);
    let _ = std::fs::remove_file(&path);
    path
}


// ── Persistence ──────────────────────────────────────────────────────────────

#[test]
fn written_values_are_readable_after_reopen() {
    let path = temp_path("persist_basic");

    let mut e = with_engine(&path, |e| {
        e.set("hello", b"world").unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert_eq!(e.get("hello").unwrap(), b"world");
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn deleted_key_stays_gone_after_reopen() {
    let path = temp_path("persist_delete");

    let mut e = with_engine(&path, |e| {
        e.set("gone", b"bye").unwrap();
        e.delete("gone").unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert!(matches!(e.get("gone"), Err(EngineError::NoSuchKey)));
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn only_the_latest_value_survives_reopen_after_multiple_updates() {
    let path = temp_path("persist_updates");

    let mut e = with_engine(&path, |e| {
        e.set("k", b"v1").unwrap();
        e.set("k", b"v2").unwrap();
        e.set("k", b"v3").unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert_eq!(e.get("k").unwrap(), b"v3");
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn all_live_keys_are_restored_after_reopen() {
    let path = temp_path("persist_all_keys");

    let mut e = with_engine(&path, |e| {
        e.set("a", b"1").unwrap();
        e.set("b", b"2").unwrap();
        e.set("c", b"3").unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    let mut keys = e.get_all_keys();
    keys.sort();
    assert_eq!(keys, vec!["a", "b", "c"]);
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn reset_store_and_reopen_gives_an_empty_store() {
    let path = temp_path("persist_reset");

    let mut e = with_engine(&path, |e| {
        e.set("a", b"1").unwrap();
        e.set("b", b"2").unwrap();
        e.reset_store().unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert!(e.get_all_keys().is_empty());
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}


// ── Complex log sequences ─────────────────────────────────────────────────────

#[test]
fn key_deleted_then_set_again_is_readable_after_reopen() {
    let path = temp_path("delete_then_set");

    let mut e = with_engine(&path, |e| {
        e.set("key", b"original").unwrap();
        e.delete("key").unwrap();
        e.set("key", b"renewed").unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert_eq!(e.get("key").unwrap(), b"renewed");
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn interleaved_sets_and_deletes_replay_to_correct_final_state() {
    let path = temp_path("interleaved");

    let mut e = with_engine(&path, |e| {
        e.set("a", b"1").unwrap();
        e.set("b", b"2").unwrap();
        e.delete("a").unwrap();      // a is gone
        e.set("c", b"3").unwrap();
        e.set("a", b"new").unwrap(); // a is back with new value
        e.delete("b").unwrap();      // b is gone
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert_eq!(e.get("a").unwrap(), b"new");
    assert!(matches!(e.get("b"), Err(EngineError::NoSuchKey)));
    assert_eq!(e.get("c").unwrap(), b"3");
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn all_values_readable_after_many_keys_survive_reopen() {
    let path = temp_path("many_keys");
    let n = 200usize;

    let mut e = with_engine(&path, |e| {
        for i in 0..n {
            e.set(&format!("key_{}", i), format!("value_{}", i).as_bytes()).unwrap();
        }
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    for i in 0..n {
        let expected = format!("value_{}", i);
        assert_eq!(e.get(&format!("key_{}", i)).unwrap(), expected.as_bytes());
    }
    assert_eq!(e.get_all_keys().len(), n);
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn store_opened_on_empty_file_contains_no_keys() {
    let path = temp_path("open_empty");

    let mut e = Engine::open(&path).unwrap();
    assert!(e.get_all_keys().is_empty());
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}

#[test]
fn binary_values_round_trip_through_reopen() {
    let path = temp_path("binary_persist");
    let value: Vec<u8> = (0u8..=255).collect();

    let mut e = with_engine(&path, |e| {
        e.set("bin", &value).unwrap();
    });
    e.close().unwrap();

    let mut e = Engine::open(&path).unwrap();
    assert_eq!(e.get("bin").unwrap(), value);
    e.close().unwrap();

    std::fs::remove_file(&path).unwrap();
}
