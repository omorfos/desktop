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
//! - [`Menu`] lays out pop-up menus and reacts to the pointer and to keys.
//! - [`Rule`] and [`Pattern`] describe per-application rules.
//! - [`place`] decides where a new window goes.
//! - [`Switcher`] and [`Used`] are the switcher on `Alt+Tab` and the order
//!   of use it goes by.

mod decoration;
mod desktop;
pub mod geometry;
mod keys;
mod menu;
mod placement;
mod rules;
mod switcher;

pub use decoration::{Button, CORNER, Decoration, Edges, GRIP, Metrics, Part, resize};
pub use desktop::{
    Arrangement, Desktop, Frame, Half, Home, Line, Listed, Offer, Opening, Place, Screen, Settings,
    Standing, Window, WindowId, WorkspaceId,
};
pub use keys::{KeyChains, KeyOutcome};
pub use menu::{Item, Menu, MenuKey, MenuMetrics, MenuOutcome, Panel};
pub use placement::{CASCADE_STEP, Placement, place};
pub use rules::{Effects, Pattern, Rule, resolve};
pub use switcher::{Switcher, SwitcherLayout, SwitcherMetrics, Used};
