//! Pure window-management domain model for the omorfos compositor.
//!
//! This crate performs no I/O and depends on no windowing library. Everything
//! in it is deterministic, so it can be tested exhaustively without a display.
//!
//! - [`KeyChains`] recognises key bindings, including multi-key chains.
//! - [`Rule`] and [`Pattern`] describe per-application rules.
//! - [`place`] decides where a new window goes.

pub mod geometry;
mod keys;
mod placement;
mod rules;

pub use keys::{KeyChains, KeyOutcome};
pub use placement::{CASCADE_STEP, Placement, place};
pub use rules::{Effects, Pattern, Rule, resolve};
