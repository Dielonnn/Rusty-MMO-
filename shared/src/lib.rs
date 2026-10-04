//! Code shared by the client and the server: game data (classes, abilities,
//! mobs), the network protocol, message framing and the terrain.

pub mod data;
pub mod net;
pub mod protocol;
pub mod world;

pub use glam;

/// The game's version, as shown to players ("v1.0"). Set in the workspace `Cargo.toml`.
pub const VERSION: &str = concat!(
    "v",
    env!("CARGO_PKG_VERSION_MAJOR"),
    ".",
    env!("CARGO_PKG_VERSION_MINOR")
);
