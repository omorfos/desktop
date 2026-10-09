//! Backends: where pixels go and where input comes from.
//!
//! A backend owns the screen and the input devices. It adds its input to a
//! [`Session`](crate::Session)'s event loop and draws after each turn of
//! it. Everything else is in [`Compositor`](crate::Compositor).

pub mod winit;
