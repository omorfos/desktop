//! Integer geometry in logical pixels.
//!
//! The origin is the top-left corner of the global space; `x` grows to the
//! right and `y` grows downwards.

/// A position in the global logical space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Point {
    /// Horizontal offset from the origin.
    pub x: i32,
    /// Vertical offset from the origin.
    pub y: i32,
}

impl Point {
    /// Creates a point at `(x, y)`.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

/// The dimensions of a rectangular area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Size {
    /// Horizontal extent.
    pub width: u32,
    /// Vertical extent.
    pub height: u32,
}

impl Size {
    /// Creates a size of `width` by `height`.
    #[must_use]
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// Returns `true` if the size covers no area.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// An axis-aligned rectangle: an origin plus a size.
///
/// The left and top edges are inclusive, the right and bottom edges exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Rect {
    /// The top-left corner.
    pub origin: Point,
    /// The extent from the origin.
    pub size: Size,
}

impl Rect {
    /// Creates a rectangle from its top-left corner and size.
    #[must_use]
    pub const fn new(origin: Point, size: Size) -> Self {
        Self { origin, size }
    }

    /// The exclusive right edge.
    ///
    /// Widened to `i64` so a rectangle near the edge of the coordinate space
    /// cannot overflow.
    #[must_use]
    pub const fn right(self) -> i64 {
        self.origin.x as i64 + self.size.width as i64
    }

    /// The exclusive bottom edge, widened to `i64` like [`Rect::right`].
    #[must_use]
    pub const fn bottom(self) -> i64 {
        self.origin.y as i64 + self.size.height as i64
    }

    /// Returns `true` if `point` lies inside the rectangle.
    #[must_use]
    pub const fn contains(self, point: Point) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && (point.x as i64) < self.right()
            && (point.y as i64) < self.bottom()
    }

    /// Returns the overlapping area of two rectangles, or `None` if they do
    /// not overlap. Rectangles that merely share an edge do not overlap.
    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let x = self.origin.x.max(other.origin.x);
        let y = self.origin.y.max(other.origin.y);
        let width = u32::try_from(self.right().min(other.right()) - i64::from(x)).ok()?;
        let height = u32::try_from(self.bottom().min(other.bottom()) - i64::from(y)).ok()?;
        let size = Size::new(width, height);
        (!size.is_empty()).then_some(Self::new(Point::new(x, y), size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn size_is_empty_when_either_dimension_is_zero() {
        assert!(Size::new(0, 10).is_empty());
        assert!(Size::new(10, 0).is_empty());
        assert!(!Size::new(1, 1).is_empty());
    }

    #[test]
    fn edges_are_origin_plus_size() {
        let r = rect(-10, 20, 30, 40);
        assert_eq!(r.right(), 20);
        assert_eq!(r.bottom(), 60);
    }

    #[test]
    fn edges_do_not_overflow_at_the_limits_of_the_space() {
        let r = rect(i32::MAX, i32::MAX, u32::MAX, u32::MAX);
        assert_eq!(r.right(), i64::from(i32::MAX) + i64::from(u32::MAX));
        assert_eq!(r.bottom(), i64::from(i32::MAX) + i64::from(u32::MAX));
    }

    #[test]
    fn contains_includes_top_left_and_excludes_bottom_right() {
        let r = rect(10, 10, 5, 5);
        assert!(r.contains(Point::new(10, 10)));
        assert!(r.contains(Point::new(14, 14)));
        assert!(!r.contains(Point::new(9, 12)));
        assert!(!r.contains(Point::new(12, 9)));
        assert!(!r.contains(Point::new(15, 12)));
        assert!(!r.contains(Point::new(12, 15)));
    }

    #[test]
    fn intersection_of_overlapping_rects_is_the_shared_area() {
        assert_eq!(
            rect(0, 0, 10, 10).intersection(rect(5, 6, 10, 10)),
            Some(rect(5, 6, 5, 4))
        );
    }

    #[test]
    fn intersection_is_none_for_disjoint_rects() {
        assert_eq!(rect(0, 0, 10, 10).intersection(rect(20, 0, 10, 10)), None);
        assert_eq!(rect(0, 0, 10, 10).intersection(rect(0, 20, 10, 10)), None);
    }

    #[test]
    fn intersection_is_none_for_rects_sharing_only_an_edge() {
        assert_eq!(rect(0, 0, 10, 10).intersection(rect(10, 0, 10, 10)), None);
    }

    #[test]
    fn intersection_with_an_empty_rect_is_none() {
        assert_eq!(rect(0, 0, 10, 10).intersection(rect(5, 5, 0, 0)), None);
    }
}
