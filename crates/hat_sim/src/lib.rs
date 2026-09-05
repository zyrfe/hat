//! HAT simulation core.
//!
//! Pure Rust, deterministic, no engine dependency. Trains are contiguous arrays of cars
//! moving in one dimension along a path over a track graph. All quantities SI.
//! See docs/sim/train-dynamics.md and docs/architecture.md.

pub mod car;
pub mod events;
pub mod params;
pub mod track;
pub mod train;
pub mod world;

pub use car::*;
pub use events::*;
pub use track::*;
pub use train::*;
pub use world::*;

pub type TrainId = u32;

/// Fixed simulation step, seconds.
pub const DT: f64 = 1.0 / 120.0;
