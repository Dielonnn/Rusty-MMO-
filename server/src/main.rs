//! Dedicated server.
//!
//! Usage: `server [--bind ADDRESS] [--port PORT] [--save FILE] [--sandbox]`
//!
//! The host can type commands into the server's window: `level NAME LEVEL`
//! sets a character's level, `help` lists them.
//!
//! Characters are saved to `server_characters.json` in the game's data folder
//! (`%APPDATA%\RustyMMO` on Windows) unless `--save` names another file.

use std::fs;
use std::path::{Path, PathBuf};

use server::Server;
use server::store::Store;
use shared::protocol::DEFAULT_PORT;

fn main() {
    let mut bind = "0.0.0.0".to_string();
    let mut port = DEFAULT_PORT;
    let mut save = None;
    let mut sandbox = false;
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
            "--save" => {
                save = Some(PathBuf::from(
                    args.next().expect("--save needs a file name"),
                ))
            }
            "--sandbox" => sandbox = true,
            "-h" | "--help" => {
                println!("Usage: server [--bind ADDRESS] [--port PORT] [--save FILE] [--sandbox]");
                println!("--sandbox lets every player use the sandbox cheats.");
                println!(
                    "Defaults: --bind 0.0.0.0 --port {DEFAULT_PORT} --save {}",
                    default_save().display()
                );
                return;
            }
            other => {
                eprintln!("Unknown argument {other:?}; try --help");
                std::process::exit(2);
            }
        }
    }
    let save = match save {
        Some(path) => absolute(&path),
        None => {
            let path = default_save();
            import_old_save(&path);
            path
        }
    };
    let mut store = match Store::open(save.clone()) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Couldn't load characters from {}: {e}", save.display());
            std::process::exit(1);
        }
    };
    // Find out now, not after the first player logs out, if saving can't work.
    if let Err(e) = store.save() {
        eprintln!("Couldn't save characters: {e}");
        eprintln!("Pick a folder you can write to with --save FILE.");
        std::process::exit(1);
    }
    let mut server = match Server::bind((bind.as_str(), port), store) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Couldn't listen on {bind}:{port}: {e}");
            std::process::exit(1);
        }
    };
    server.world.sandbox = sandbox;
    server.read_console();
    if sandbox {
        println!("Sandbox mode: players can use cheats.");
    }
    println!(
        "Rusty MMO {} server listening on {}, saving characters to {}",
        shared::version(),
        server.local_addr().unwrap(),
        save.display()
    );
    println!("Type help for host commands, like level NAME LEVEL.");
    server.run();
}

/// Where characters are saved when `--save` isn't given. It doesn't depend on
/// where the server is run from, so every version finds the same save.
fn default_save() -> PathBuf {
    absolute(&shared::data_dir().join("server_characters.json"))
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Servers before v8.2.1 saved to `characters.json` in whatever folder they
/// were started from. If there's no save in the data folder yet, copy one of
/// those in (from the current folder or the server's own folder), leaving
/// the old file where it was.
fn import_old_save(save: &Path) {
    if save.exists() {
        return;
    }
    let mut candidates = vec![absolute(Path::new("characters.json"))];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        candidates.push(dir.join("characters.json"));
    }
    let Some(old) = candidates.into_iter().find(|p| p.is_file()) else {
        return;
    };
    if let Some(dir) = save.parent() {
        let _ = fs::create_dir_all(dir);
    }
    match fs::copy(&old, save) {
        Ok(_) => println!(
            "Copied the characters saved in {} to {}",
            old.display(),
            save.display()
        ),
        Err(e) => eprintln!("Couldn't copy {} to {}: {e}", old.display(), save.display()),
    }
}
