//! Pure window-management domain model for the omorfos compositor.
//!
//! This crate performs no I/O and depends on no windowing library. Everything
//! in it is deterministic, so it can be tested exhaustively without a display.

pub mod geometry;
