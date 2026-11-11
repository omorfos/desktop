//! A window's frame on its way to where it was sent: maximized or
//! restored, tiled, or given the room of one that closed. Its program is
//! told at once how the window is to be, and the frame is drawn easing
//! there over the time `docs/DESIGN.md` gives something moving to where it
//! was sent, each edge as a notification glides. What is dragged by hand,
//! and what a program makes its own window, is where it is put at once.
//!
//! Time is whatever whoever asks says it is.

use std::time::Duration;

use omorfos_core::geometry::{Point, Rect, Size};
use omorfos_ui::Glide;

/// Where a frame is drawn: its left and top edges, its width and its
/// height, each where the frame is or on its way there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Way {
    left: Glide,
    top: Glide,
    width: Glide,
    height: Glide,
}

impl Way {
    /// A frame at `place`, going nowhere.
    pub(crate) fn at(place: Rect) -> Self {
        Self {
            left: Glide::at(place.origin.x),
            top: Glide::at(place.origin.y),
            width: Glide::at(length(place.size.width)),
            height: Glide::at(length(place.size.height)),
        }
    }

    /// Sends it to `place` from where it is at `now` if it `moves`, and
    /// otherwise puts it there at once. What is already going where it is
    /// sent goes on as it was.
    pub(crate) fn go(&mut self, place: Rect, now: Duration, moves: bool) {
        self.left.go(place.origin.x, now, moves);
        self.top.go(place.origin.y, now, moves);
        self.width.go(length(place.size.width), now, moves);
        self.height.go(length(place.size.height), now, moves);
    }

    /// Where it is at `now`.
    pub(crate) fn now(&self, now: Duration) -> Rect {
        Rect::new(
            Point::new(self.left.now(now), self.top.now(now)),
            Size::new(
                self.width.now(now).unsigned_abs(),
                self.height.now(now).unsigned_abs(),
            ),
        )
    }

    /// Where it is going, and will stay.
    pub(crate) fn to(&self) -> Rect {
        Rect::new(
            Point::new(self.left.to(), self.top.to()),
            Size::new(
                self.width.to().unsigned_abs(),
                self.height.to().unsigned_abs(),
            ),
        )
    }
}

/// A width or a height as a place along a line, which none on a screen is
/// too long to be.
fn length(of: u32) -> i32 {
    i32::try_from(of).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn ms(count: u64) -> Duration {
        Duration::from_millis(count)
    }

    const fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn a_frame_sent_elsewhere_eases_there_edge_by_edge() {
        let mut way = Way::at(rect(100, 100, 200, 100));
        way.go(rect(0, 0, 600, 500), ms(1000), true);
        assert_eq!(way.now(ms(1000)), rect(100, 100, 200, 100), "setting out");
        assert_eq!(way.now(ms(1120)), rect(50, 50, 400, 300), "half way");
        assert_eq!(way.now(ms(1240)), way.to());
        assert_eq!(way.to(), rect(0, 0, 600, 500));
    }

    #[test]
    fn a_frame_put_somewhere_is_there_at_once() {
        let mut way = Way::at(rect(100, 100, 200, 100));
        way.go(rect(0, 0, 600, 500), ms(1000), false);
        assert_eq!(way.now(ms(1000)), rect(0, 0, 600, 500));
        // However large it is.
        let huge = rect(0, 0, u32::MAX, 1);
        assert_eq!(Way::at(huge).to(), rect(0, 0, i32::MAX.unsigned_abs(), 1));
    }
}
