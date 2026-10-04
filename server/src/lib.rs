//! The game server: an authoritative world simulation behind a TCP listener.

mod network;
pub mod world;

pub use network::{Server, spawn_local};

/// World updates per second.
pub const TICK_RATE: u32 = 20;
