//! Deciding where a new window goes.

use crate::geometry::{Point, Rect, Size};

/// The distance, in both directions, between successive cascaded windows.
pub const CASCADE_STEP: u32 = 32;

/// A strategy for positioning a new window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Placement {
    /// Offset each window from the previous one, so titlebars stay visible.
    #[default]
    Cascade,
    /// In the middle of the area.
    Center,
    /// Centered on the pointer.
    Pointer,
}

/// Returns the top-left corner for a new window of `size` within `area`.
///
/// `pointer` is used by [`Placement::Pointer`], and `index` (how many windows
/// are already there) by [`Placement::Cascade`]. The result always keeps the
/// window inside `area`; a window larger than `area` is aligned to its
/// top-left corner so the titlebar stays reachable.
#[must_use]
pub fn place(policy: Placement, size: Size, area: Rect, pointer: Point, index: usize) -> Point {
    let (width, height) = (i64::from(size.width), i64::from(size.height));
    let (left, top) = (i64::from(area.origin.x), i64::from(area.origin.y));
    let (x, y) = match policy {
        Placement::Cascade => {
            let free_width = area.size.width.saturating_sub(size.width);
            let free_height = area.size.height.saturating_sub(size.height);
            // Start again from the corner once a window would leave the area.
            let slots = i64::from(free_width.min(free_height) / CASCADE_STEP) + 1;
            let offset = i64::try_from(index).unwrap_or(i64::MAX) % slots * i64::from(CASCADE_STEP);
            (left + offset, top + offset)
        }
        Placement::Center => (
            left + (i64::from(area.size.width) - width) / 2,
            top + (i64::from(area.size.height) - height) / 2,
        ),
        Placement::Pointer => (
            i64::from(pointer.x) - width / 2,
            i64::from(pointer.y) - height / 2,
        ),
    };
    Point::new(
        fit(x, left, area.right() - width),
        fit(y, top, area.bottom() - height),
    )
}

/// Clamps `value` between `min` and `max`, preferring `min` if they cross.
fn fit(value: i64, min: i64, max: i64) -> i32 {
    i32::try_from(value.clamp(min, max.max(min))).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(Point::new(100, 50), Size::new(1000, 800));
    const SIZE: Size = Size::new(400, 300);
    const ORIGIN: Point = Point::new(0, 0);

    #[test]
    fn center_puts_the_window_in_the_middle_of_the_area() {
        assert_eq!(
            place(Placement::Center, SIZE, AREA, ORIGIN, 0),
            Point::new(400, 300)
        );
    }

    #[test]
    fn pointer_centers_the_window_on_the_pointer() {
        let pointer = Point::new(600, 450);
        assert_eq!(
            place(Placement::Pointer, SIZE, AREA, pointer, 0),
            Point::new(400, 300)
        );
    }

    #[test]
    fn pointer_placement_keeps_the_window_inside_the_area() {
        let top_left = place(Placement::Pointer, SIZE, AREA, Point::new(100, 50), 0);
        assert_eq!(top_left, Point::new(100, 50));
        let bottom_right = place(Placement::Pointer, SIZE, AREA, Point::new(1099, 849), 0);
        assert_eq!(bottom_right, Point::new(700, 550));
    }

    #[test]
    fn cascade_steps_diagonally_from_the_corner() {
        assert_eq!(
            place(Placement::Cascade, SIZE, AREA, ORIGIN, 0),
            Point::new(100, 50)
        );
        assert_eq!(
            place(Placement::Cascade, SIZE, AREA, ORIGIN, 1),
            Point::new(132, 82)
        );
        assert_eq!(
            place(Placement::Cascade, SIZE, AREA, ORIGIN, 2),
            Point::new(164, 114)
        );
    }

    #[test]
    fn cascade_starts_over_when_the_window_would_leave_the_area() {
        // 500 pixels are free vertically: 15 steps fit, the 16th does not.
        assert_eq!(
            place(Placement::Cascade, SIZE, AREA, ORIGIN, 15),
            Point::new(580, 530)
        );
        assert_eq!(
            place(Placement::Cascade, SIZE, AREA, ORIGIN, 16),
            Point::new(100, 50)
        );
    }

    #[test]
    fn a_window_larger_than_the_area_is_aligned_to_its_top_left_corner() {
        let huge = Size::new(5000, 5000);
        for policy in [Placement::Cascade, Placement::Center, Placement::Pointer] {
            assert_eq!(
                place(policy, huge, AREA, Point::new(600, 450), 7),
                AREA.origin
            );
        }
    }

    #[test]
    fn placement_at_the_limit_of_the_coordinate_space_does_not_overflow() {
        let area = Rect::new(Point::new(i32::MAX - 10, 0), Size::new(1000, 1000));
        let placed = place(Placement::Center, Size::new(10, 10), area, ORIGIN, 0);
        assert_eq!(placed, Point::new(i32::MAX, 495));
    }

    #[test]
    fn cascade_is_the_default() {
        assert_eq!(Placement::default(), Placement::Cascade);
    }
}
