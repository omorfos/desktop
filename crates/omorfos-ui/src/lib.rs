//! Drawing for the omorfos compositor and its shell components.
//!
//! The compositor paints its titlebars and menus, and each shell component
//! paints itself. They share how: a [`Canvas`] described in units of
//! layout and drawn at a [`Scale`], with [`Text`] set in the fonts the
//! system has. Sharing it is what makes a panel look as if it belongs to
//! the window frames beside it.

mod blur;
mod canvas;
mod glide;
mod motion;
mod scale;
mod shadow;

pub use blur::Blur;
pub use canvas::{Area, Canvas, Image, Text, inset};
pub use glide::Glide;
pub use motion::{Motion, Seen};
pub use scale::Scale;
pub use shadow::{Cut, Piece, Shade, falls};
