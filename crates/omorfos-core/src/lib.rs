//! Pure window-management domain model for the omorfos compositor.
//!
//! This crate performs no I/O and depends on no windowing library. Everything
//! in it is deterministic, so it can be tested exhaustively without a display.
//!
//! - [`place`] decides where a new window goes.

pub mod geometry;
mod placement;

pub use placement::{CASCADE_STEP, Placement, place};
