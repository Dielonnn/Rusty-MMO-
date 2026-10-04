//! Dedicated server.
//!
//! Usage: `server [--bind ADDRESS] [--port PORT] [--save FILE]`

use std::path::PathBuf;

use server::Server;
use server::store::Store;
use shared::protocol::DEFAULT_PORT;

fn main() {
    let mut bind = "0.0.0.0".to_string();
    let mut port = DEFAULT_PORT;
    let mut save = PathBuf::from("characters.json");
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bind" => bind = args.next().expect("--bind needs an address"),
            "--port" => {
                port = args
                    .next()
                    .and_then(|p| p.parse().ok())
                    .expect("--port needs a number")
            }
            "--save" => save = args.next().expect("--save needs a file name").into(),
            "-h" | "--help" => {
                println!("Usage: server [--bind ADDRESS] [--port PORT] [--save FILE]");
                println!("Defaults: --bind 0.0.0.0 --port {DEFAULT_PORT} --save characters.json");
                return;
            }
            other => {
                eprintln!("Unknown argument {other:?}; try --help");
                std::process::exit(2);
            }
        }
    }
    let store = match Store::open(save.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Couldn't load characters from {}: {e}", save.display());
            std::process::exit(1);
        }
    };
    let server = match Server::bind((bind.as_str(), port), store) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Couldn't listen on {bind}:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "Rusty MMO {} server listening on {}, saving characters to {}",
        shared::VERSION,
        server.local_addr().unwrap(),
        save.display()
    );
    server.run();
}
