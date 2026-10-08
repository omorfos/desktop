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
//!
//! Every value in a [`Config`] is valid by construction: validation happens
//! while deserializing, so nothing downstream needs to re-check it.

mod action;
mod bounded;
mod color;
mod config;
mod diagnostic;
mod general;
mod keys;
mod menu;
mod paths;
mod rules;
#[cfg(feature = "schema")]
pub mod schema;
mod store;
mod theme;

pub use action::{Action, Command, MAX_WORKSPACES, WorkspaceNumber};
pub use bounded::Bounded;
pub use color::Color;
pub use config::{Config, FILES, MAX_FILE_SIZE};
pub use diagnostic::{Diagnostic, Position};
pub use general::{Focus, FocusModel, General, Keyboard, Placement, Touchpad, Workspaces};
pub use keys::{Binding, KeyChain, KeyChord, Keys, Modifiers};
pub use menu::{Menu, MenuItem};
pub use paths::config_dir;
pub use rules::{Match, Rule, Rules};
pub use store::Store;
pub use theme::{Animations, Border, Colors, Decoration, Font, Theme, Titlebar};
