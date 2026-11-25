//! Drawing for the omorfos compositor and its shell components.
//!
//! The compositor paints its titlebars and menus, and each shell component
//! paints itself. They share how: a [`Canvas`] described in units of
//! layout and drawn at a [`Scale`], with [`Text`] set in the fonts the
//! system has. Sharing it is what makes a panel look as if it belongs to
//! the window frames beside it.

pub mod blink;
mod blur;
mod button;
mod canvas;
pub mod field;
mod glide;
mod motion;
pub mod question;
mod scale;
mod shadow;
mod tooltip;

pub use blink::{Blink, Moment};
pub use blur::Blur;
pub use button::{button, button_look, button_size};
pub use canvas::{Area, Canvas, Image, Text, inset};
pub use field::{Edit, Field};
pub use glide::Glide;
pub use motion::{Motion, Seen};
pub use scale::Scale;
pub use shadow::{Cut, Piece, Shade, falls};
pub use tooltip::{ROW as TOOLTIP_ROW, tooltip};
