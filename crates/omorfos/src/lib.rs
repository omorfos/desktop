//! The omorfos Wayland compositor.
//!
//! This crate is the adapter layer: it connects Smithay (Wayland protocols,
//! input, rendering) to the pure window-management model in `omorfos-core`,
//! configured by `omorfos-config`. Policy lives in those crates; code here
//! translates between them and the outside world.

pub mod bridge;
pub mod cli;
