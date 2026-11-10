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
mod daylight;
mod debounce;
mod diagnostic;
mod general;
mod gradient;
mod keys;
mod menu;
mod paths;
mod rules;
#[cfg(feature = "schema")]
pub mod schema;
mod scheme;
mod store;
mod style;
mod theme;
mod transition;
mod value;
mod wallpaper;
mod watcher;

pub use action::{Action, Command, Half, MAX_WORKSPACES, Role, Started, WorkspaceNumber};
pub use bounded::Bounded;
pub use color::Color;
pub use config::{Config, FILES, MAX_FILE_SIZE};
pub use daylight::{Coordinates, Day, Location};
pub use debounce::Debouncer;
pub use diagnostic::{Diagnostic, Position};
pub use general::{
    Acceleration, Align, Appearance, Clipboard, Cursor, Focus, FocusModel, General, Icons, Idle,
    Kept, Keyboard, Launcher, Lock, Mouse, NamedOutput, Notifications, OfferedMode, Offset, Output,
    OutputScale, Outputs, Panel, Place, Placement, Portals, Rotation, ScreenMode, Screenshots,
    Speed, Touchpad, Update, Workspaces, X11Scaling, Xwayland,
};
pub use gradient::{Direction, Gradient, Run, Stop};
pub use keys::{Binding, KeyChain, KeyChord, Keys, Modifiers};
pub use menu::{Menu, MenuItem};
pub use paths::{cache_dir, config_dir};
pub use rules::{Match, Rule, Rules};
pub use scheme::{Accent, Scheme, SchemeChoice};
pub use store::{STEP, Store, TURNING};
pub use style::{Edges, Line, Part, Presence, Shadow, State, States, Style};
pub use theme::{Font, Theme, ThemeError};
pub use transition::{Easing, Transition};
pub use wallpaper::{
    Fit, Order, Picture, PicturePlace, ScreenWallpaper, Screens, Shown, Wallpaper,
};
pub use watcher::Watcher;
