//! Rounding the bottom corners of a frame.
//!
//! The top corners belong to the titlebar, which the compositor paints and
//! so can cut to any shape. The bottom corners are the window's own
//! picture, which it cannot. A renderer could clip that picture with a
//! shader, but only a GPU one, and then nothing here could be tested.
//!
//! Instead the window is drawn as a stack of strips, one for each run of
//! pixel rows that the curve leaves at the same width, and the border is
//! painted over the join as an arc. Strips have hard edges; the arc, which
//! is smooth, hides them. Any renderer can draw a strip.
//!
//! See `docs/adr/0016-rounded-corners.md`.

use std::cell::RefCell;
use std::collections::HashMap;

use omorfos_core::Decoration;
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use smithay::backend::renderer::Renderer;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::utils::{Buffer, Physical, Rectangle, Scale, Transform};

/// The radius the bottom corners of a frame are actually rounded to, in
/// units of layout: the one asked for, or as much of it as there is room
/// for beside the other corner and below the titlebar.
pub(crate) fn radius(decoration: &Decoration, wanted: u32) -> u32 {
    let outer = decoration.outer().size;
    let below_titlebar = outer
        .height
        .saturating_sub(decoration.titlebar().size.height);
    wanted.min(outer.width / 2).min(below_titlebar)
}

/// The left, right and bottom borders of a frame, cut short where the
/// rounded corners take over from them.
pub(crate) fn straight_borders(decoration: &Decoration, radius: u32) -> [Rect; 3] {
    let [left, right, bottom] = decoration.borders();
    let shorter = |side: Rect| {
        let height = side.size.height.saturating_sub(radius);
        Rect::new(side.origin, DomainSize::new(side.size.width, height))
    };
    // The bottom border runs between the side borders, so it begins one
    // border's width in from the corner of the frame already.
    let reach = radius.saturating_sub(left.size.width);
    let narrower = Rect::new(
        DomainPoint::new(bottom.origin.x + reach.cast_signed(), bottom.origin.y),
        DomainSize::new(
            bottom.size.width.saturating_sub(reach.saturating_mul(2)),
            bottom.size.height,
        ),
    );
    [shorter(left), shorter(right), narrower]
}

/// The parts of a window's area that are inside its frame once the frame's
/// bottom corners are rounded, as strips from the top down. All in pixels.
///
/// `outer` is the whole frame, whose corners are the ones rounded, and
/// `client` the window's area within it. A pixel the curve passes through
/// is left out, to be covered by the border painted along the curve.
pub(crate) fn strips(
    outer: Rectangle<i32, Physical>,
    client: Rectangle<i32, Physical>,
    radius: i32,
) -> Vec<Rectangle<i32, Physical>> {
    let bottom = outer.loc.y + outer.size.h;
    let right = outer.loc.x + outer.size.w;
    let client_bottom = client.loc.y + client.size.h;
    let client_right = client.loc.x + client.size.w;
    // Where the curve begins. Above it the window is as wide as it is.
    let curve = (bottom - radius).clamp(client.loc.y, client_bottom);
    let mut strips = vec![Rectangle::new(
        client.loc,
        (client.size.w, curve - client.loc.y).into(),
    )];
    for row in curve..client_bottom {
        let cut = cut(
            radius.unsigned_abs(),
            (row - (bottom - radius)).unsigned_abs(),
        );
        let from = client.loc.x.max(outer.loc.x + cut);
        // Where the two curves meet there is nothing left of the row.
        let width = (client_right.min(right - cut) - from).max(0);
        match strips.last_mut() {
            // Rows of the same width make one strip.
            Some(above) if above.loc.x == from && above.size.w == width => above.size.h += 1,
            _ => strips.push(Rectangle::new((from, row).into(), (width, 1).into())),
        }
    }
    strips.retain(|strip| strip.size.w > 0 && strip.size.h > 0);
    strips
}

/// How many pixels the curve takes from each end of a row of a rounded
/// corner: those wholly outside it and the one it passes through.
///
/// `row` counts down from where the curve begins, and the corner is a
/// quarter circle of `radius` pixels.
fn cut(radius: u32, row: u32) -> i32 {
    // In half pixels, so that the middle of a row is a whole number.
    let diameter = u64::from(radius) * 2;
    let down = u64::from(row) * 2 + 1;
    let across = (diameter * diameter).saturating_sub(down * down).isqrt();
    let outside = diameter.saturating_sub(across) / 2;
    i32::try_from(outside + 1).unwrap_or(i32::MAX)
}

/// The names the strips of one frame go by from frame to frame.
///
/// The renderer works out what changed by following each thing it draws
/// under a name of its own. A window's picture has one name however many
/// strips it is drawn as, so each strip is given another, kept here so that
/// it is the same the next time.
#[derive(Debug, Default)]
pub(crate) struct Names(RefCell<HashMap<(Id, usize), Id>>);

impl Names {
    /// The most names kept. A window's pop-ups come and go, each leaving
    /// names behind; past this they are all forgotten, at the cost of one
    /// frame drawn in full.
    const MOST: usize = 256;

    /// The name of one strip of the picture called `whole`.
    fn of(&self, whole: &Id, strip: usize) -> Id {
        let mut names = self.0.borrow_mut();
        if names.len() >= Self::MOST {
            names.clear();
        }
        names
            .entry((whole.clone(), strip))
            .or_insert_with(Id::new)
            .clone()
    }
}

/// One strip of something drawn.
#[derive(Debug)]
pub(crate) struct Strip<E> {
    cropped: CropRenderElement<E>,
    id: Id,
}

impl<E: Element> Strip<E> {
    /// The part of `whole` inside `area`, if any of it is. `strip` tells
    /// the strips of one picture apart.
    pub(crate) fn of(
        whole: E,
        scale: f64,
        area: Rectangle<i32, Physical>,
        strip: usize,
        names: &Names,
    ) -> Option<Self> {
        let id = names.of(whole.id(), strip);
        CropRenderElement::from_element(whole, scale, area).map(|cropped| Self { cropped, id })
    }
}

impl<E: Element> Element for Strip<E> {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.cropped.current_commit()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.cropped.src()
    }

    fn transform(&self) -> Transform {
        self.cropped.transform()
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.cropped.geometry(scale)
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        self.cropped.damage_since(scale, commit)
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        self.cropped.opaque_regions(scale)
    }

    fn alpha(&self) -> f32 {
        self.cropped.alpha()
    }
}

impl<R: Renderer, E: RenderElement<R>> RenderElement<R> for Strip<E> {
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        self.cropped.draw(frame, src, dst, damage, opaque_regions)
    }
}

#[cfg(test)]
mod tests {
    use omorfos_core::Metrics;
    use omorfos_core::geometry::{Point, Size};

    use super::*;

    fn rectangle(x: i32, y: i32, width: i32, height: i32) -> Rectangle<i32, Physical> {
        Rectangle::new((x, y).into(), (width, height).into())
    }

    /// A frame at the origin with a border of one and a titlebar of 28.
    fn frame(width: u32, height: u32) -> Decoration {
        let outer = Rect::new(Point::new(0, 0), Size::new(width, height));
        Decoration::new(
            outer,
            Metrics {
                border: 1,
                titlebar: 28,
                buttons: [0; 3],
            },
            1,
        )
    }

    /// Whether a pixel is in any of the strips.
    fn covered(strips: &[Rectangle<i32, Physical>], x: i32, y: i32) -> bool {
        strips.iter().any(|strip| strip.contains((x, y)))
    }

    #[test]
    fn a_window_with_square_corners_is_one_strip() {
        let outer = rectangle(10, 20, 202, 129);
        let client = rectangle(11, 48, 200, 100);
        assert_eq!(strips(outer, client, 0), [client]);
    }

    #[test]
    fn rounding_takes_the_corners_and_nothing_else() {
        let outer = rectangle(0, 0, 202, 129);
        let client = rectangle(1, 28, 200, 100);
        let strips = strips(outer, client, 8);

        assert_eq!(
            strips[0],
            rectangle(1, 28, 200, 96),
            "down to where the curve reaches in past the border"
        );
        assert!(!covered(&strips, 1, 127), "the corner itself");
        assert!(!covered(&strips, 200, 127), "and the other");
        assert!(covered(&strips, 100, 127), "the middle of the last row");
        assert!(covered(&strips, 8, 127), "the last row beyond the curve");
        assert!(covered(&strips, 3, 122), "inside the curve higher up");
        assert!(!covered(&strips, 1, 126));
        for strip in &strips {
            assert!(client.contains_rect(*strip), "{strip:?} is in the window");
        }
        // Every row is covered once, by one strip.
        let rows: i32 = strips.iter().map(|strip| strip.size.h).sum();
        assert_eq!(rows, 100);
        // The same is taken from each side.
        for strip in &strips {
            let left = strip.loc.x - outer.loc.x;
            let right = (outer.loc.x + outer.size.w) - (strip.loc.x + strip.size.w);
            assert_eq!(left, right, "{strip:?}");
        }
    }

    #[test]
    fn the_curve_takes_more_the_nearer_the_bottom() {
        let cuts: Vec<i32> = (0..8).map(|row| cut(8, row)).collect();
        assert_eq!(cuts, [1, 1, 1, 2, 2, 3, 4, 6]);
        assert!(cuts.is_sorted());
        assert_eq!(cut(0, 0), 1, "nothing to round still gives up the edge");
    }

    #[test]
    fn a_window_too_small_for_its_corners_keeps_what_is_left() {
        // The curve starts above the window, and lower down its two ends
        // meet, leaving nothing of those rows.
        let outer = rectangle(0, 0, 20, 40);
        let client = rectangle(1, 28, 18, 11);
        let strips = strips(outer, client, 30);
        assert_eq!(strips[0], rectangle(7, 28, 6, 1), "narrowed from the top");
        assert!(strips.iter().all(|strip| client.contains_rect(*strip)));
        assert!(strips.iter().all(|strip| strip.size.w > 0));
        assert!(!covered(&strips, 10, 38), "the last row is gone altogether");
    }

    #[test]
    fn corners_are_no_larger_than_there_is_room_for() {
        assert_eq!(radius(&frame(202, 129), 8), 8);
        assert_eq!(radius(&frame(202, 129), 0), 0);
        assert_eq!(radius(&frame(10, 129), 8), 5, "half the width");
        assert_eq!(radius(&frame(202, 32), 8), 4, "what is below the titlebar");
    }

    #[test]
    fn the_straight_borders_stop_where_the_corners_begin() {
        let decoration = frame(202, 129);
        assert_eq!(straight_borders(&decoration, 0), decoration.borders());

        let [left, right, bottom] = straight_borders(&decoration, 8);
        let [whole_left, whole_right, whole_bottom] = decoration.borders();
        assert_eq!(left.origin, whole_left.origin);
        assert_eq!(left.size.height, whole_left.size.height - 8);
        assert_eq!(right.origin, whole_right.origin);
        assert_eq!(right.size.height, whole_right.size.height - 8);
        assert_eq!(bottom.origin.x, 8, "the radius in from the corner");
        assert_eq!(bottom.size.width, 202 - 16);
        assert_eq!(bottom.origin.y, whole_bottom.origin.y);
    }

    #[test]
    fn a_strip_keeps_its_name_and_strips_do_not_share_one() {
        let names = Names::default();
        let (whole, other) = (Id::new(), Id::new());
        let first = names.of(&whole, 0);
        assert_eq!(names.of(&whole, 0), first, "the same from frame to frame");
        assert_ne!(names.of(&whole, 1), first);
        assert_ne!(names.of(&other, 0), first);
        assert_ne!(first, whole, "and not the name of the whole");

        // Names are not kept without limit.
        for strip in 0..Names::MOST {
            names.of(&other, strip + 1);
        }
        assert_ne!(names.of(&whole, 0), first, "forgotten, and named afresh");
    }
}
