
use std::process;

use clap::Arg;
use clap::Command;

use minikvp::engine::Engine;
use minikvp::engine::EngineError;


fn main() {
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
                .about("Write VALUE for KEY")
                .arg(Arg::new("key").required(true).help("The key to write"))
                .arg(Arg::new("value").required(true).help("The value to store (UTF-8 string)")),
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
            let value = sub.get_one::<String>("value").unwrap();
            engine.set(key, value.as_bytes()).unwrap_or_else(|e| {
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
        _ => unreachable!(),
    }

    engine.close().unwrap_or_else(|e| {
        eprintln!("error: could not close store: {}", e);
        process::exit(1);
    });
}
