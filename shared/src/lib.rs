//! Code shared by the client and the server: game data (classes, abilities,
//! mobs), the network protocol, message framing, the terrain and scenery.

pub mod data;
pub mod dungeon;
pub mod layout;
pub mod net;
pub mod props;
pub mod protocol;
pub mod quests;
pub mod talents;
pub mod world;

pub use glam;

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
