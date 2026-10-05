//! Code shared by the client and the server: game data (classes, abilities,
//! mobs), the network protocol, message framing, the terrain and scenery.

pub mod data;
pub mod dungeon;
pub mod emote;
pub mod layout;
pub mod net;
pub mod props;
pub mod protocol;
pub mod quests;
pub mod spells;
pub mod talents;
pub mod world;

pub use glam;

use std::path::PathBuf;

/// Where saves and settings live: `%APPDATA%\RustyMMO` on Windows,
/// `~/Library/Application Support/RustyMMO` on macOS and
/// `~/.local/share/rusty-mmo` elsewhere. The same for every version, so
/// saves carry over when a new build is unzipped somewhere else.
pub fn data_dir() -> PathBuf {
    let env = |k: &str| std::env::var_os(k).map(PathBuf::from);
    if cfg!(windows) {
        if let Some(d) = env("APPDATA") {
            return d.join("RustyMMO");
        }
    } else if cfg!(target_os = "macos") {
        if let Some(h) = env("HOME") {
            return h.join("Library/Application Support/RustyMMO");
        }
    } else if let Some(d) = env("XDG_DATA_HOME") {
        return d.join("rusty-mmo");
    } else if let Some(h) = env("HOME") {
        return h.join(".local/share/rusty-mmo");
    }
    PathBuf::from(".")
}

/// The game's version, as shown to players ("v1.0", or "v7.9.5" for a patch).
/// Set in the workspace `Cargo.toml`.
pub fn version() -> String {
    let (major, minor, patch) = (
        env!("CARGO_PKG_VERSION_MAJOR"),
        env!("CARGO_PKG_VERSION_MINOR"),
        env!("CARGO_PKG_VERSION_PATCH"),
    );
    if patch == "0" {
        format!("v{major}.{minor}")
    } else {
        format!("v{major}.{minor}.{patch}")
    }
}
