//! Backends: where pixels go and where input comes from.
//!
//! A backend owns the event loop and the screen. Everything else is in
//! [`Compositor`](crate::Compositor), which a backend drives.

pub mod winit;
