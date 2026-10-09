//! Pure window-management domain model for the omorfos compositor.
//!
//! This crate performs no I/O and depends on no windowing library. Everything
//! in it is deterministic, so it can be tested exhaustively without a display.
//!
//! - [`Desktop`] is the state of the session: windows, tab groups, workspaces,
//!   stacking and focus. Adapters call its methods as things happen and read
//!   back what to show.
//! - [`Decoration`] lays out a frame's titlebar, tabs and borders.
//! - [`KeyChains`] recognises key bindings, including multi-key chains.
//! - [`Rule`] and [`Pattern`] describe per-application rules.
//! - [`place`] decides where a new window goes.

mod decoration;
mod desktop;
pub mod geometry;
mod keys;
mod placement;
mod rules;

pub use decoration::{CORNER, Decoration, Edges, GRIP, Metrics, Part, resize};
pub use desktop::{Desktop, Frame, Settings, Window, WindowId};
pub use keys::{KeyChains, KeyOutcome};
pub use placement::{CASCADE_STEP, Placement, place};
pub use rules::{Effects, Pattern, Rule, resolve};
