//! The game server: an authoritative world simulation behind a TCP listener,
//! with characters saved to a JSON file.

pub mod character;
mod network;
pub mod password;
pub mod store;
pub mod world;

pub use network::{Server, spawn_local};

/// World updates per second.
pub const TICK_RATE: u32 = 20;
