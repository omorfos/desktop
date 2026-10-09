//! The omorfos Wayland compositor.
//!
//! This crate is the adapter layer: it connects Smithay (Wayland protocols,
//! input, rendering) to the pure window-management model in `omorfos-core`,
//! configured by `omorfos-config`. Policy lives in those crates; code here
//! translates between them and the outside world.
//!
//! [`Compositor`] is the whole compositor except for where pixels go and
//! where input comes from. A backend owns the event loop: it feeds
//! [`Input`] in, calls [`Compositor::refresh`] after dispatching clients,
//! and draws with [`Compositor::render`].

pub mod bridge;
pub mod cli;
mod compositor;
mod handlers;
mod input;

pub use compositor::{ClientState, Compositor};
pub use input::Input;
