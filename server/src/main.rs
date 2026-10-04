//! Dedicated server.
//!
//! Usage: `server [--bind ADDRESS] [--port PORT]`

use server::Server;
use shared::protocol::DEFAULT_PORT;

fn main() {
    let mut bind = "0.0.0.0".to_string();
    let mut port = DEFAULT_PORT;
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
            "-h" | "--help" => {
                println!("Usage: server [--bind ADDRESS] [--port PORT]");
                println!("Defaults: --bind 0.0.0.0 --port {DEFAULT_PORT}");
                return;
            }
            other => {
                eprintln!("Unknown argument {other:?}; try --help");
                std::process::exit(2);
            }
        }
    }
    let server = match Server::bind((bind.as_str(), port)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Couldn't listen on {bind}:{port}: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "Rusty MMO server listening on {}",
        server.local_addr().unwrap()
    );
    server.run();
}
