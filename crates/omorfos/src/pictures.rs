//! Windows drawn small, in the switcher and the overview (ADR 38): each
//! window's picture as large as fits where it goes, no larger than it is,
//! and a private window as it is in pictures of the screen, blank or under
//! glass that blurs it.

use smithay::backend::renderer::element::AsRenderElements as _;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::utils::{Logical, Point, Rectangle};

use omorfos_config::{Part, States};
use omorfos_core::WindowId;

use crate::compositor::{Compositor, Element, Mapped, PRIVATE_BLUR, View};
use crate::glass::{self, Pane};

/// A rectangle worked out in fractions of units of layout: its left, its
/// top, how wide and how tall.
pub(crate) type Exact = (f64, f64, f64, f64);

/// Where a picture of something `width` by `height` goes in `room`: as
/// large as fits, its shape kept, but no larger than it is, which would
/// only blur it; in the middle.
pub(crate) fn fitted((width, height): (i32, i32), (left, top, across, down): Exact) -> Exact {
    let (width, height) = (f64::from(width.max(1)), f64::from(height.max(1)));
    let factor = (across / width).min(down / height).min(1.0);
    let shown = (width * factor, height * factor);
    let left = left + (across - shown.0) / 2.0;
    (left, top + (down - shown.1) / 2.0, shown.0, shown.1)
}

/// A length worked out in fractions, as the nearest whole one.
#[expect(
    clippy::cast_possible_truncation,
    reason = "within a screen, far inside an i32"
)]
pub(crate) fn whole(length: f64) -> i32 {
    length.round() as i32
}

/// The size of `mapped`, as its frame has it.
pub(crate) fn shape(mapped: &Mapped) -> (i32, i32) {
    let size = mapped.window.geometry().size;
    (size.w, size.h)
}

/// `mapped`'s picture made as large as `picture`, which has its shape, as
/// see-through as `opacity` says, and nothing of it outside `clip`.
pub(crate) fn pictured<R>(
    renderer: &mut R,
    mapped: &Mapped,
    ((left, top, across, down), clip): (Exact, Exact),
    view: View,
    opacity: f32,
) -> impl Iterator<Item = Element<R>>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Texture + Send + 'static,
{
    let size = mapped.window.geometry().size;
    let factor = across / f64::from(size.w.max(1));
    let shown = (across, down);
    let corner = view.pixel(Point::<f64, Logical>::from((left, top)));
    let scale = view.scale.factor() * factor;
    let within = mapped.window.geometry().loc.to_f64().to_physical(scale);
    let at = corner - within.to_i32_round();
    let picture = smithay::utils::Size::<f64, Logical>::from(shown);
    let crop = Rectangle::new(
        corner,
        picture.to_physical(view.scale.factor()).to_i32_round(),
    );
    let (left, top, across, down) = clip;
    let from = view.pixel(Point::<f64, Logical>::from((left, top)));
    let size = smithay::utils::Size::<f64, Logical>::from((across, down));
    let clip = Rectangle::new(from, size.to_physical(view.scale.factor()).to_i32_round());
    let crop = crop.intersection(clip).unwrap_or_default();
    let scale = smithay::utils::Scale::from(scale);
    let surfaces = mapped
        .window
        .render_elements::<WaylandSurfaceRenderElement<R>>(renderer, at, scale, opacity);
    surfaces
        .into_iter()
        .filter_map(move |surface| CropRenderElement::from_element(surface, scale, crop))
        .map(Element::Cropped)
}

impl Compositor {
    /// Whether `window` is shown blank where it is pictured small, as in
    /// pictures of the screen: private, and not to be blurred.
    pub(crate) fn blank(&self, window: WindowId) -> bool {
        self.desktop.private(window) && !self.desktop.blurred(window)
    }

    /// Adds `window`'s picture, as large as fits in `room` and as
    /// see-through as `opacity` says: under glass that blurs it, and is
    /// blank where glass cannot, if a rule says it is private but to be
    /// blurred, as in pictures of the screen; and not at all if it is to be
    /// blank, or is not there.
    pub(crate) fn picture_elements<R>(
        &self,
        renderer: &mut R,
        window: (WindowId, f32),
        room: Exact,
        view: View,
        drawn: (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        self.picture_within(renderer, window, (room, room, 0), view, drawn);
    }

    /// Adds `window`'s picture as [`Compositor::picture_elements`] does,
    /// and nothing of it outside `within`: a window hanging off the edge of
    /// what it is pictured in is cut there. `place` tells apart the places
    /// one window is pictured in at once, as the overview pictures it both
    /// laid out and on its workspace in the strip.
    pub(crate) fn picture_within<R>(
        &self,
        renderer: &mut R,
        (window, opacity): (WindowId, f32),
        (room, within, place): (Exact, Exact, usize),
        view: View,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let Some(mapped) = self.mapped(window).filter(|_| !self.blank(window)) else {
            return;
        };
        let picture = fitted(shape(mapped), room);
        let first = elements.len();
        elements.extend(pictured(renderer, mapped, (picture, within), view, opacity));
        if self.desktop.blurred(window) {
            let blank = self
                .config()
                .theme
                .style(Part::Capture, States::NONE)
                .ground();
            // Out to the next whole unit on every side, so that whatever
            // pixel the picture is drawn to at any scale is under it.
            let area = |(left, top, across, down): Exact| {
                let (left, top) = (left.floor(), top.floor());
                let (right, bottom) = ((left + across).ceil(), (top + down).ceil());
                Rectangle::<i32, Logical>::new(
                    (whole(left), whole(top)).into(),
                    (whole(right - left), whole(bottom - top)).into(),
                )
            };
            let area = area(picture).intersection(area(within)).unwrap_or_default();
            // Whole however faint the window is, as what it blurs is the
            // faint picture: glass as faint would let the picture through.
            // Known by the window and where it is pictured, not by where
            // that is now, so that what the glass keeps is kept as it moves.
            let key = glass::key(&("private, pictured small", window, place));
            panes.push(Pane::veiling(
                (area, view),
                (PRIVATE_BLUR, blank),
                (key, first),
            ));
        }
    }
}
