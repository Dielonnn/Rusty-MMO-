//! Code shared by the client and the server: game data (classes, abilities,
//! mobs), the network protocol, message framing and the terrain.

pub mod data;
pub mod net;
pub mod protocol;
pub mod world;

pub use glam;
