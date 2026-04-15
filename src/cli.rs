
use std::process;

use clap::Arg;
use clap::Command;
use clap::ArgGroup;

use minikvp::engine::Engine;
use minikvp::engine::EngineError;
use minikvp::model::OnDiskEntry;


pub fn run() {
    let matches = Command::new("minikvp")
        .about("A very minimal key-value store")
        .arg_required_else_help(true)
        .arg(
            Arg::new("store")
                .long("store")
                .short('s')
                .default_value("minikvp.db")
                .global(true)
                .help("Path to the store file"),
        )
        .subcommand(
            Command::new("get")
                .about("Print the value stored for KEY")
                .arg(Arg::new("key").required(true).help("The key to look up")),
        )
        .subcommand(
            Command::new("set")
                .about("Write a value for KEY")
                .arg(Arg::new("key").required(true).help("The key to write"))
                .arg(
                    Arg::new("value")
                        .value_name("VALUE")
                        .help("The value as a UTF-8 string"),
                )
                .arg(
                    Arg::new("hex")
                        .long("hex")
                        .value_name("HEX")
                        .help("The value as a hex-encoded byte string (e.g. FF00AB)"),
                )
                .arg(
                    Arg::new("file")
                        .long("file")
                        .value_name("PATH")
                        .help("Read the value from a file"),
                )
                .group(
                    ArgGroup::new("value_source")
                        .args(["value", "hex", "file"])
                        .required(true),
                ),
        )
        .subcommand(
            Command::new("delete")
                .about("Remove KEY from the store")
                .arg(Arg::new("key").required(true).help("The key to delete")),
        )
        .subcommand(
            Command::new("keys")
                .about("List all keys present in the store"),
        )
        .subcommand(
            Command::new("reset")
                .about("Erase all data in the store file"),
        )
        .subcommand(
            Command::new("inspect")
                .about("Print the raw on-disk structure of every log entry"),
        )
        .get_matches();

    let store_path = matches.get_one::<String>("store").unwrap();

    let mut engine = Engine::open(store_path).unwrap_or_else(|e| {
        eprintln!("error: could not open store '{}': {}", store_path, e);
        process::exit(1);
    });

    match matches.subcommand() {
        Some(("get", sub)) => {
            let key = sub.get_one::<String>("key").unwrap();
            match engine.get(key) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(s) => println!("{}", s),
                    Err(e) => println!("{:?}", e.into_bytes()),
                },
                Err(EngineError::NoSuchKey) => {
                    eprintln!("error: key not found: '{}'", key);
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(("set", sub)) => {
            let key = sub.get_one::<String>("key").unwrap();
            let value: Vec<u8> = if let Some(s) = sub.get_one::<String>("value") {
                s.as_bytes().to_vec()
            } else if let Some(hex) = sub.get_one::<String>("hex") {
                decode_hex(hex).unwrap_or_else(|e| {
                    eprintln!("error: {}", e);
                    process::exit(1);
                })
            } else if let Some(path) = sub.get_one::<String>("file") {
                std::fs::read(path).unwrap_or_else(|e| {
                    eprintln!("error: could not read '{}': {}", path, e);
                    process::exit(1);
                })
            } else {
                unreachable!()
            };
            engine.set(key, &value).unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                process::exit(1);
            });
        }
        Some(("delete", sub)) => {
            let key = sub.get_one::<String>("key").unwrap();
            match engine.delete(key) {
                Ok(()) => {}
                Err(EngineError::NoSuchKey) => {
                    eprintln!("error: key not found: '{}'", key);
                    process::exit(1);
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    process::exit(1);
                }
            }
        }
        Some(("keys", _)) => {
            let mut keys = engine.get_all_keys().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                process::exit(1);
            });
            if keys.is_empty() {
                println!("(store is empty)");
            } else {
                keys.sort();
                for key in keys {
                    println!("{}", key);
                }
            }
        }
        Some(("reset", _)) => {
            engine.reset_store().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                process::exit(1);
            });
            println!("Store reset.");
        }
        Some(("inspect", _)) => {
            let entries = engine.scan_log().unwrap_or_else(|e| {
                eprintln!("error: {}", e);
                process::exit(1);
            });
            if entries.is_empty() {
                println!("(store is empty)");
            } else {
                print_inspect_table(&entries);
            }
        }
        _ => unreachable!(),
    }

    engine.close().unwrap_or_else(|e| {
        eprintln!("error: could not close store: {}", e);
        process::exit(1);
    });
}


fn terminal_width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120)
}

fn truncate(s: &str, max: usize) -> String {
    if max == 0 { return String::new(); }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else {
        let truncated: String = chars[..max - 1].iter().collect();
        format!("{}…", truncated)
    }
}

fn format_value(bytes: &[u8], max: usize) -> String {
    if bytes.is_empty() { return String::new(); }
    match std::str::from_utf8(bytes) {
        Ok(s) => truncate(s, max),
        Err(_) => "[bin]".to_string(),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("hex string must have an even number of characters".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte at position {}: '{}'", i, &s[i..i + 2]))
        })
        .collect()
}

fn print_inspect_table(entries: &[(u64, OnDiskEntry)]) {
    const KEY_SZ_W: usize = 7;
    const VAL_SZ_W: usize = 12;
    const FLAGS_W:  usize = 6;
    const SEP: &str = " | ";

    // Remaining space is split: key gets 1/3, value gets the rest
    let fixed = KEY_SZ_W + VAL_SZ_W + FLAGS_W + SEP.len() * 4;
    let remaining = terminal_width().saturating_sub(fixed);
    let key_w = (remaining / 3).max(10);
    let val_w = remaining.saturating_sub(key_w).max(10);

    // Header
    println!(
        "{:>KEY_SZ_W$}{SEP}{:>VAL_SZ_W$}{SEP}{:<FLAGS_W$}{SEP}{:<key_w$}{SEP}Value",
        "KeySz", "ValSz", "Flags", "Key",
    );

    // Separator
    println!(
        "{}{SEP}{}{SEP}{}{SEP}{}{SEP}{}",
        "─".repeat(KEY_SZ_W),
        "─".repeat(VAL_SZ_W),
        "─".repeat(FLAGS_W),
        "─".repeat(key_w),
        "─".repeat(val_w),
    );

    // Rows
    for (_, entry) in entries {
        let is_tombstone = entry.flags & 0x01 != 0;
        let flags_str = format!("0x{:02X}", entry.flags);
        let key_str   = truncate(&entry.key, key_w);
        let value_str = if is_tombstone {
            "[deleted]".to_string()
        } else {
            format_value(&entry.value, val_w)
        };

        println!(
            "{:>KEY_SZ_W$}{SEP}{:>VAL_SZ_W$}{SEP}{:<FLAGS_W$}{SEP}{:<key_w$}{SEP}{}",
            entry.key_size, entry.value_size, flags_str, key_str, value_str,
        );
    }
}
