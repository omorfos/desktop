//! Configuration for the omorfos compositor.
//!
//! Configuration lives in a directory of optional YAML files, one per
//! concern (see [`FILES`]). This crate turns that directory into a validated
//! [`Config`] value and keeps it current:
//!
//! - [`Config::load`] reads and validates every file, reporting problems as
//!   [`Diagnostic`]s that carry the file, line and column.
//! - [`Store`] holds the configuration in effect. A file that fails to load
//!   keeps its previous value, so a typo never breaks a running session.
//! - [`Watcher`] and [`Debouncer`] tell the caller when to reload.
//!
//! Every value in a [`Config`] is valid by construction: validation happens
//! while deserializing, so nothing downstream needs to re-check it.

mod action;
mod bounded;
mod color;
mod config;
mod css;
mod debounce;
mod diagnostic;
mod general;
mod keys;
mod menu;
mod paths;
mod rules;
#[cfg(feature = "schema")]
pub mod schema;
mod store;
mod style;
mod theme;
mod transition;
mod value;
mod watcher;

pub use action::{Action, Command, MAX_WORKSPACES, WorkspaceNumber};
pub use bounded::Bounded;
pub use color::Color;
pub use config::{Config, FILES, MAX_FILE_SIZE};
pub use debounce::Debouncer;
pub use diagnostic::{Diagnostic, Position};
pub use general::{
    Cursor, Focus, FocusModel, General, Idle, Keyboard, Launcher, Mouse, NamedOutput,
    Notifications, Output, OutputScale, Outputs, Panel, Placement, Touchpad, Workspaces,
    X11Scaling, Xwayland,
};
pub use keys::{Binding, KeyChain, KeyChord, Keys, Modifiers};
pub use menu::{Menu, MenuItem};
pub use paths::config_dir;
pub use rules::{Match, Rule, Rules};
pub use store::Store;
pub use style::{Edges, Line, Part, State, States, Style};
pub use theme::{Font, Theme, ThemeError};
pub use transition::{Easing, Transition};
pub use watcher::Watcher;
