//! The drawing engine: the triangle batcher, model-building frames, the 3D
//! shader with its lighting and fog, textures and model files.
//!
//! The world, models and effects only draw through what this exports. If
//! they need something new from the engine, it's added here.

// Parts of the engine's API (some texture and batch helpers) are here for
// the graphics work that follows, and aren't all used yet.
#[allow(dead_code)]
mod batch;
mod color;
mod frame;
mod model_file;
mod shader;
#[allow(dead_code)]
pub mod texture;

pub use batch::{Batch, basis};
pub use color::{c, dark, mix, rgb};
pub use frame::Frame;
pub use model_file::{ModelFile, Picture, Transform};
pub use shader::{Fog, GroundPaint, Light, Shading};
