//! Window decorations: the titlebar and borders that make up a frame.
//!
//! Everything here is geometry. What a decoration looks like is the
//! compositor's business; where its parts are, and which part a point falls
//! on, is decided here so that it can be tested without drawing anything.

use crate::geometry::{Point, Rect, Size};

/// How far outside a frame the pointer can still grab its edge to resize it.
///
/// Borders are often a single pixel wide, which is too thin to aim at.
pub const GRIP: u32 = 6;

/// How close to a corner, along an edge, a grab resizes in both directions.
pub const CORNER: u32 = 24;

/// A set of rectangle edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Edges(u8);

impl Edges {
    /// No edges.
    pub const NONE: Self = Self(0);
    /// The left edge.
    pub const LEFT: Self = Self(1);
    /// The right edge.
    pub const RIGHT: Self = Self(1 << 1);
    /// The top edge.
    pub const TOP: Self = Self(1 << 2);
    /// The bottom edge.
    pub const BOTTOM: Self = Self(1 << 3);

    /// Returns the union of `self` and `other`.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns `true` if every edge in `other` is also in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns `other` if `condition` holds, and no edges otherwise.
    const fn when(condition: bool, other: Self) -> Self {
        if condition { other } else { Self::NONE }
    }
}

/// The thickness of a decoration's parts, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Metrics {
    /// The width of the border on the left, right and bottom.
    pub border: u32,
    /// The height of the titlebar across the top.
    pub titlebar: u32,
}

impl Metrics {
    /// The size of a frame whose window is `client` pixels.
    #[must_use]
    pub const fn outer_size(self, client: Size) -> Size {
        Size::new(
            client.width.saturating_add(self.border.saturating_mul(2)),
            client
                .height
                .saturating_add(self.border)
                .saturating_add(self.titlebar),
        )
    }
}

/// The part of a frame under a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Part {
    /// The tab with this index, in the titlebar.
    Tab(usize),
    /// A border, or just outside one. Dragging resizes these edges.
    Border(Edges),
    /// The window itself.
    Client,
}

/// The layout of a decorated frame: a titlebar holding one tab per window
/// across the top, and a border around the other three sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Decoration {
    outer: Rect,
    metrics: Metrics,
    tabs: usize,
}

impl Decoration {
    /// Lays out a frame occupying `outer` with `tabs` tabs (at least one).
    #[must_use]
    pub const fn new(outer: Rect, metrics: Metrics, tabs: usize) -> Self {
        Self {
            outer,
            metrics,
            tabs: if tabs == 0 { 1 } else { tabs },
        }
    }

    /// The whole frame.
    #[must_use]
    pub const fn outer(&self) -> Rect {
        self.outer
    }

    /// The area left for the window.
    #[must_use]
    pub const fn client(&self) -> Rect {
        let Metrics { border, titlebar } = self.metrics;
        let size = self.outer.size;
        Rect::new(
            self.outer
                .origin
                .offset(border.cast_signed(), titlebar.cast_signed()),
            Size::new(
                size.width.saturating_sub(border.saturating_mul(2)),
                size.height.saturating_sub(border).saturating_sub(titlebar),
            ),
        )
    }

    /// The titlebar, across the full width of the frame.
    #[must_use]
    pub const fn titlebar(&self) -> Rect {
        let height = min(self.metrics.titlebar, self.outer.size.height);
        Rect::new(self.outer.origin, Size::new(self.outer.size.width, height))
    }

    /// The number of tabs.
    #[must_use]
    pub const fn tabs(&self) -> usize {
        self.tabs
    }

    /// The tab with this index. Tabs share the titlebar equally.
    #[must_use]
    pub fn tab(&self, index: usize) -> Rect {
        let titlebar = self.titlebar();
        let width = u64::from(titlebar.size.width);
        let count = u64::try_from(self.tabs).unwrap_or(u64::MAX);
        let edge = |index: usize| {
            let index = u64::try_from(index).unwrap_or(u64::MAX).min(count);
            u32::try_from(width * index / count).unwrap_or(u32::MAX)
        };
        let (start, end) = (edge(index), edge(index.saturating_add(1)));
        Rect::new(
            titlebar.origin.offset(start.cast_signed(), 0),
            Size::new(end - start, titlebar.size.height),
        )
    }

    /// The left, right and bottom borders, in that order.
    #[must_use]
    pub const fn borders(&self) -> [Rect; 3] {
        let client = self.client();
        let border = self.metrics.border;
        let side = Size::new(border, client.size.height.saturating_add(border));
        let right = self.outer.size.width.saturating_sub(border);
        let below = self.outer.size.height.saturating_sub(border);
        [
            Rect::new(Point::new(self.outer.origin.x, client.origin.y), side),
            Rect::new(
                Point::new(self.outer.origin.x, client.origin.y).offset(right.cast_signed(), 0),
                side,
            ),
            Rect::new(
                self.outer
                    .origin
                    .offset(border.cast_signed(), below.cast_signed()),
                Size::new(client.size.width, border),
            ),
        ]
    }

    /// The part of the frame at `point`, or `None` if the point is not on
    /// the frame or within [`GRIP`] pixels of it.
    #[must_use]
    pub fn hit(&self, point: Point) -> Option<Part> {
        let outer = self.outer;
        let grip = GRIP.cast_signed();
        let reach = Rect::new(
            outer.origin.offset(-grip, -grip),
            Size::new(
                outer.size.width.saturating_add(GRIP * 2),
                outer.size.height.saturating_add(GRIP * 2),
            ),
        );
        if !reach.contains(point) {
            return None;
        }
        let client = self.client();
        if client.contains(point) {
            return Some(Part::Client);
        }
        if self.titlebar().contains(point) {
            return (0..self.tabs)
                .find(|index| self.tab(*index).contains(point))
                .map(Part::Tab);
        }

        let (x, y) = (i64::from(point.x), i64::from(point.y));
        let left = x < i64::from(client.origin.x);
        let right = x >= client.right();
        let top = y < i64::from(outer.origin.y);
        let bottom = y >= client.bottom();
        // Near a corner, a grab on one edge also takes the adjacent one. The
        // corner zones never overlap, however small the frame.
        let across = i64::from(min(CORNER, outer.size.width / 2));
        let down = i64::from(min(CORNER, outer.size.height / 2));
        let near_left = x < i64::from(outer.origin.x) + across;
        let near_right = x >= outer.right() - across;
        let near_top = y < i64::from(outer.origin.y) + down;
        let near_bottom = y >= outer.bottom() - down;
        let edges = Edges::when(left || ((top || bottom) && near_left), Edges::LEFT)
            .with(Edges::when(
                right || ((top || bottom) && near_right),
                Edges::RIGHT,
            ))
            .with(Edges::when(
                top || ((left || right) && near_top),
                Edges::TOP,
            ))
            .with(Edges::when(
                bottom || ((left || right) && near_bottom),
                Edges::BOTTOM,
            ));
        Some(Part::Border(edges))
    }
}

/// Returns `initial` with the given `edges` dragged by `(dx, dy)`.
///
/// The opposite edges stay where they are, and the result is never smaller
/// than `minimum`.
#[must_use]
pub fn resize(initial: Rect, edges: Edges, dx: i32, dy: i32, minimum: Size) -> Rect {
    let (x, width) = drag(
        initial.origin.x,
        initial.size.width,
        dx,
        minimum.width,
        edges.contains(Edges::LEFT),
        edges.contains(Edges::RIGHT),
    );
    let (y, height) = drag(
        initial.origin.y,
        initial.size.height,
        dy,
        minimum.height,
        edges.contains(Edges::TOP),
        edges.contains(Edges::BOTTOM),
    );
    Rect::new(Point::new(x, y), Size::new(width, height))
}

/// Resizes one axis: `near` is the edge at `start`, `far` the opposite one.
fn drag(start: i32, length: u32, delta: i32, minimum: u32, near: bool, far: bool) -> (i32, u32) {
    let (start, length, delta) = (i64::from(start), i64::from(length), i64::from(delta));
    let minimum = i64::from(minimum);
    let (start, length) = if near {
        let resized = (length - delta).max(minimum);
        (start + length - resized, resized)
    } else if far {
        (start, (length + delta).max(minimum))
    } else {
        (start, length)
    };
    (
        i32::try_from(start).unwrap_or(i32::MAX),
        u32::try_from(length).unwrap_or(u32::MAX),
    )
}

const fn min(a: u32, b: u32) -> u32 {
    if a < b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRICS: Metrics = Metrics {
        border: 2,
        titlebar: 20,
    };
    const OUTER: Rect = Rect::new(Point::new(100, 50), Size::new(304, 222));

    fn frame(tabs: usize) -> Decoration {
        Decoration::new(OUTER, METRICS, tabs)
    }

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn a_frame_is_its_window_plus_a_titlebar_and_three_borders() {
        assert_eq!(METRICS.outer_size(Size::new(300, 200)), OUTER.size);
        let frame = frame(1);
        assert_eq!(frame.outer(), OUTER);
        assert_eq!(frame.client(), rect(102, 70, 300, 200));
        assert_eq!(frame.titlebar(), rect(100, 50, 304, 20));
        assert_eq!(
            frame.borders(),
            [
                rect(100, 70, 2, 202),
                rect(402, 70, 2, 202),
                rect(102, 270, 300, 2)
            ]
        );
    }

    #[test]
    fn without_metrics_the_window_fills_the_frame() {
        let bare = Decoration::new(OUTER, Metrics::default(), 1);
        assert_eq!(bare.client(), OUTER);
        assert_eq!(bare.titlebar().size.height, 0);
        assert_eq!(Metrics::default().outer_size(OUTER.size), OUTER.size);
    }

    #[test]
    fn a_frame_smaller_than_its_decoration_has_no_room_for_the_window() {
        let tiny = Decoration::new(rect(0, 0, 3, 10), METRICS, 1);
        assert_eq!(tiny.client().size, Size::new(0, 0));
        assert_eq!(tiny.titlebar().size, Size::new(3, 10));
    }

    #[test]
    fn tabs_share_the_titlebar_equally_and_leave_no_gaps() {
        let frame = frame(3);
        assert_eq!(frame.tabs(), 3);
        assert_eq!(frame.tab(0), rect(100, 50, 101, 20));
        assert_eq!(frame.tab(1), rect(201, 50, 101, 20));
        assert_eq!(frame.tab(2), rect(302, 50, 102, 20));
        assert_eq!(frame.tab(7).size.width, 0, "there is no such tab");
    }

    #[test]
    fn a_frame_always_has_at_least_one_tab() {
        let frame = Decoration::new(OUTER, METRICS, 0);
        assert_eq!(frame.tabs(), 1);
        assert_eq!(frame.tab(0), frame.titlebar());
    }

    #[test]
    fn a_point_on_the_window_hits_the_client() {
        assert_eq!(frame(1).hit(Point::new(102, 70)), Some(Part::Client));
        assert_eq!(frame(1).hit(Point::new(401, 269)), Some(Part::Client));
    }

    #[test]
    fn a_point_on_the_titlebar_hits_the_tab_under_it() {
        let frame = frame(3);
        assert_eq!(frame.hit(Point::new(100, 50)), Some(Part::Tab(0)));
        assert_eq!(frame.hit(Point::new(200, 69)), Some(Part::Tab(0)));
        assert_eq!(frame.hit(Point::new(201, 60)), Some(Part::Tab(1)));
        assert_eq!(frame.hit(Point::new(403, 60)), Some(Part::Tab(2)));
    }

    #[test]
    fn a_point_on_or_just_outside_a_border_grabs_that_edge() {
        let frame = frame(1);
        assert_eq!(
            frame.hit(Point::new(101, 150)),
            Some(Part::Border(Edges::LEFT))
        );
        assert_eq!(
            frame.hit(Point::new(95, 150)),
            Some(Part::Border(Edges::LEFT))
        );
        assert_eq!(
            frame.hit(Point::new(402, 150)),
            Some(Part::Border(Edges::RIGHT))
        );
        assert_eq!(
            frame.hit(Point::new(409, 150)),
            Some(Part::Border(Edges::RIGHT))
        );
        assert_eq!(
            frame.hit(Point::new(250, 270)),
            Some(Part::Border(Edges::BOTTOM))
        );
        assert_eq!(
            frame.hit(Point::new(250, 277)),
            Some(Part::Border(Edges::BOTTOM))
        );
        assert_eq!(
            frame.hit(Point::new(250, 45)),
            Some(Part::Border(Edges::TOP))
        );
    }

    #[test]
    fn near_a_corner_a_grab_takes_both_edges() {
        let frame = frame(1);
        let corner = |a: Edges, b: Edges| Some(Part::Border(a.with(b)));
        assert_eq!(
            frame.hit(Point::new(110, 271)),
            corner(Edges::LEFT, Edges::BOTTOM)
        );
        assert_eq!(
            frame.hit(Point::new(101, 265)),
            corner(Edges::LEFT, Edges::BOTTOM)
        );
        assert_eq!(
            frame.hit(Point::new(400, 271)),
            corner(Edges::RIGHT, Edges::BOTTOM)
        );
        assert_eq!(
            frame.hit(Point::new(403, 260)),
            corner(Edges::RIGHT, Edges::BOTTOM)
        );
        assert_eq!(
            frame.hit(Point::new(105, 46)),
            corner(Edges::LEFT, Edges::TOP)
        );
        assert_eq!(
            frame.hit(Point::new(96, 55)),
            corner(Edges::LEFT, Edges::TOP)
        );
        assert_eq!(
            frame.hit(Point::new(400, 46)),
            corner(Edges::RIGHT, Edges::TOP)
        );
        assert_eq!(
            frame.hit(Point::new(408, 55)),
            corner(Edges::RIGHT, Edges::TOP)
        );
    }

    #[test]
    fn a_point_beyond_the_grip_misses_the_frame() {
        let frame = frame(1);
        assert_eq!(frame.hit(Point::new(93, 150)), None);
        assert_eq!(frame.hit(Point::new(410, 150)), None);
        assert_eq!(frame.hit(Point::new(250, 43)), None);
        assert_eq!(frame.hit(Point::new(250, 278)), None);
    }

    #[test]
    fn corner_zones_do_not_overlap_on_a_small_frame() {
        let small = Decoration::new(rect(0, 0, 10, 30), METRICS, 1);
        assert_eq!(
            small.hit(Point::new(4, 31)),
            Some(Part::Border(Edges::LEFT.with(Edges::BOTTOM)))
        );
        assert_eq!(
            small.hit(Point::new(5, 31)),
            Some(Part::Border(Edges::RIGHT.with(Edges::BOTTOM)))
        );
    }

    #[test]
    fn edges_combine_and_report_what_they_contain() {
        let corner = Edges::LEFT.with(Edges::TOP);
        assert!(corner.contains(Edges::LEFT) && corner.contains(Edges::TOP));
        assert!(!corner.contains(Edges::RIGHT));
        assert!(corner.contains(Edges::NONE));
        assert_eq!(Edges::default(), Edges::NONE);
    }

    #[test]
    fn dragging_the_far_edges_grows_the_rectangle_in_place() {
        let initial = rect(100, 100, 300, 200);
        let edges = Edges::RIGHT.with(Edges::BOTTOM);
        assert_eq!(
            resize(initial, edges, 50, -20, Size::new(10, 10)),
            rect(100, 100, 350, 180)
        );
    }

    #[test]
    fn dragging_the_near_edges_keeps_the_opposite_corner_fixed() {
        let initial = rect(100, 100, 300, 200);
        let edges = Edges::LEFT.with(Edges::TOP);
        assert_eq!(
            resize(initial, edges, 50, -20, Size::new(10, 10)),
            rect(150, 80, 250, 220)
        );
    }

    #[test]
    fn edges_that_are_not_dragged_do_not_move() {
        let initial = rect(100, 100, 300, 200);
        assert_eq!(
            resize(initial, Edges::RIGHT, 50, 500, Size::new(10, 10)),
            rect(100, 100, 350, 200)
        );
        assert_eq!(
            resize(initial, Edges::NONE, 50, 50, Size::new(10, 10)),
            initial
        );
    }

    #[test]
    fn a_rectangle_cannot_be_dragged_below_the_minimum_size() {
        let initial = rect(100, 100, 300, 200);
        let minimum = Size::new(120, 80);
        let shrunk = resize(
            initial,
            Edges::RIGHT.with(Edges::BOTTOM),
            -1000,
            -1000,
            minimum,
        );
        assert_eq!(shrunk, rect(100, 100, 120, 80));
        let shrunk = resize(initial, Edges::LEFT.with(Edges::TOP), 1000, 1000, minimum);
        assert_eq!(shrunk, rect(280, 220, 120, 80), "the far corner stays put");
    }
}
