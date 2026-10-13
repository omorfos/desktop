//! The omorfos Wayland compositor.
//!
//! This crate is the adapter layer: it connects Smithay (Wayland protocols,
//! input, rendering) to the pure window-management model in `omorfos-core`,
//! configured by `omorfos-config`. Policy lives in those crates; code here
//! translates between them and the outside world.
//!
//! [`Compositor`] is the whole compositor except for where pixels go and
//! where input comes from. A [`Session`] pairs it with the event loop and
//! the Wayland display, and keeps it up to date as clients talk to it. A
//! backend adds its input to that loop as [`Input`] values and draws each
//! turn with [`Compositor::render`].

pub mod backend;
pub mod bridge;
pub mod cli;
mod compositor;
mod corners;
mod cursor;
mod ghost;
mod handlers;
mod input;
mod ipc;
mod lock;
mod outputs;
mod paint;
mod restarts;
mod reveal;
mod session;
mod tiles;
mod x11;

pub use compositor::{ClientState, Compositor, DESKTOP_NAME};
pub use cursor::Themes;
pub use input::Input;
pub use omorfos_ui::Scale;
pub use session::Session;
pub use x11::Focus;
