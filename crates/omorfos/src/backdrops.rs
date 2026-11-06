//! What is behind a program blurred where it asks
//! (`ext-background-effect-v1`, ADR 33).
//!
//! A program says which parts of a surface it wants what is behind blurred,
//! as a region, and draws its own colour over them; the compositor puts
//! glass with no tint of its own there. How far it is blurred is the
//! theme's `window` part's `backdrop-filter`, for every program alike. A
//! program is told it may have blur unless the theme gives programs none
//! or the user wants transparency reduced, and is told again whenever that
//! changes.
//!
//! The region is double-buffered with the surface, and kept as the
//! rectangles it comes to, which do not overlap: those it adds, less those
//! it takes away. It is cut to the surface, whatever the program asks, a
//! window's to the window proper and not the shadow it may draw round
//! itself on the same surface, and the whole of it is blurred as one
//! picture, however many rectangles it is made of, as a program that
//! rounds its corners makes it of many.

use std::cell::Cell;

use omorfos_config::{Part, States};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::reexports::wayland_protocols::ext::background_effect::v1::server::{
    ext_background_effect_manager_v1::{self, Capability, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::{self, ExtBackgroundEffectSurfaceV1},
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Logical, Physical, Point, Rectangle};
use smithay::wayland::compositor::{
    Cacheable, RectangleKind, RegionAttributes, get_region_attributes, with_states,
};

use crate::compositor::{Compositor, View};
use crate::glass::{self, Pane};

/// The protocol, and what programs that bound it were last told.
#[derive(Debug)]
pub(crate) struct Backdrops {
    /// Each program's binding of it, to be told when what it may have
    /// changes.
    bound: Vec<ExtBackgroundEffectManagerV1>,
    /// What they were last told they may have.
    told: Capability,
}

impl Backdrops {
    /// Offers the protocol on `display`, and KDE's for the same (ADR 64).
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        display.create_global::<Compositor, ExtBackgroundEffectManagerV1, ()>(1, ());
        crate::kde_blur::offer(display);
        Self {
            bound: Vec::new(),
            told: Capability::empty(),
        }
    }
}

/// The parts of a surface its program asks to have what is behind blurred,
/// double-buffered with it; none until it asks.
#[derive(Debug, Clone, Default)]
pub(crate) enum Asked {
    /// None of it.
    #[default]
    Nothing,
    /// All of it, as only KDE's protocol asks (ADR 64).
    Whole,
    /// These rectangles of it, in its own coordinates.
    Within(Vec<Rectangle<i32, Logical>>),
}

impl Cacheable for Asked {
    fn commit(&mut self, _: &DisplayHandle) -> Self {
        self.clone()
    }

    fn merge_into(self, into: &mut Self, _: &DisplayHandle) {
        *into = self;
    }
}

/// Whether a surface has an object to ask for blur through, of which it may
/// have one at a time.
#[derive(Debug, Default)]
struct Asking(Cell<bool>);

/// The rectangles a region comes to, which do not overlap: those added,
/// less those taken away since, in order.
pub(crate) fn rectangles(region: &RegionAttributes) -> Vec<Rectangle<i32, Logical>> {
    let mut within: Vec<Rectangle<i32, Logical>> = Vec::new();
    for (kind, rectangle) in &region.rects {
        within = match kind {
            RectangleKind::Add => {
                let new = rectangle.subtract_rects(within.iter().copied());
                within.extend(new);
                within
            }
            RectangleKind::Subtract => Rectangle::subtract_rects_many(within, [*rectangle]),
        };
    }
    within
}

impl Compositor {
    /// What programs may have behind them: blur, unless the theme gives
    /// them none or the user wants transparency reduced.
    fn backdrop_capabilities(&self) -> Capability {
        let config = self.config();
        let blur = config.theme.style(Part::Window, States::NONE).blur;
        let reduced = config.general.appearance.reduce_transparency;
        if blur > 0 && !reduced {
            Capability::Blur
        } else {
            Capability::empty()
        }
    }

    /// Tells every program that bound the protocol what it may have, if
    /// that has changed since it was last told.
    pub(crate) fn tell_backdrops(&mut self) {
        let capabilities = self.backdrop_capabilities();
        let backdrops = &mut self.protocols.backdrops;
        backdrops.bound.retain(Resource::is_alive);
        if capabilities != backdrops.told {
            for manager in &backdrops.bound {
                manager.capabilities(capabilities);
            }
            backdrops.told = capabilities;
        }
    }

    /// The glass `surface`'s program asked for behind it, drawn with its
    /// top left corner at `origin` and as much of it to see as `opacity`,
    /// within `bounds` of the output, in pixels, if any are given: none if
    /// it asked for none or may have none. It goes at `at` in what is
    /// drawn.
    pub(crate) fn backdrop(
        &self,
        surface: &WlSurface,
        (origin, view): (Point<i32, Logical>, View),
        bounds: Option<&[Rectangle<i32, Physical>]>,
        (at, opacity): (usize, f32),
    ) -> Option<Pane> {
        // Nothing is looked at for a program that may have none.
        let may = !self.backdrop_capabilities().is_empty();
        let asked = may.then(|| {
            with_states(surface, |states| {
                let mut cached = states.cached_state.get::<Asked>();
                cached.current().clone()
            })
        });
        let size = with_renderer_surface_state(surface, |state| state.surface_size());
        let whole = Rectangle::from_size(size.flatten().unwrap_or_default());
        let asked = match asked.unwrap_or_default() {
            Asked::Nothing => Vec::new(),
            Asked::Whole => vec![whole],
            Asked::Within(asked) => asked,
        };
        let placed = |area: Rectangle<i32, Logical>| {
            view.pixels(Rectangle::new(origin + area.loc, area.size))
        };
        let strips = asked.into_iter();
        let strips = strips
            .filter_map(|area| area.intersection(whole))
            .map(placed);
        let strips: Vec<_> = match bounds {
            Some(bounds) => strips
                .flat_map(|strip| {
                    bounds
                        .iter()
                        .filter_map(move |bound| strip.intersection(*bound))
                })
                .collect(),
            None => strips.collect(),
        };
        let blur = self.config().theme.style(Part::Window, States::NONE).blur;
        let key = glass::key(&("backdrop", surface.id()));
        Pane::behind(strips, (blur, view), (key, at, 0), opacity)
    }
}

impl GlobalDispatch<ExtBackgroundEffectManagerV1, ()> for Compositor {
    fn bind(
        state: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        manager: New<ExtBackgroundEffectManagerV1>,
        (): &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(manager, ());
        manager.capabilities(state.backdrop_capabilities());
        state.protocols.backdrops.bound.push(manager);
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        manager: &ExtBackgroundEffectManagerV1,
        request: ext_background_effect_manager_v1::Request,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let ext_background_effect_manager_v1::Request::GetBackgroundEffect { id, surface } =
            request
        {
            let had = with_states(&surface, |states| {
                let asking = states.data_map.get_or_insert(Asking::default);
                asking.0.replace(true)
            });
            if had {
                manager.post_error(
                    ext_background_effect_manager_v1::Error::BackgroundEffectExists,
                    "the surface already has an object to ask for blur through",
                );
                return;
            }
            data_init.init(id, surface);
        }
    }
}

impl Dispatch<ExtBackgroundEffectSurfaceV1, WlSurface> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        effect: &ExtBackgroundEffectSurfaceV1,
        request: ext_background_effect_surface_v1::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let ext_background_effect_surface_v1::Request::SetBlurRegion { region } = request {
            if !surface.is_alive() {
                effect.post_error(
                    ext_background_effect_surface_v1::Error::SurfaceDestroyed,
                    "the surface has been destroyed",
                );
                return;
            }
            let asked = region.map_or(Asked::Nothing, |region| {
                Asked::Within(rectangles(&get_region_attributes(&region)))
            });
            with_states(surface, |states| {
                *states.cached_state.get::<Asked>().pending() = asked;
            });
        }
    }

    fn destroyed(_: &mut Self, _: ClientId, _: &ExtBackgroundEffectSurfaceV1, surface: &WlSurface) {
        // What it asked for goes on the surface's next commit, and the
        // surface may be asked for through another.
        if surface.is_alive() {
            with_states(surface, |states| {
                *states.cached_state.get::<Asked>().pending() = Asked::Nothing;
                let asking = states.data_map.get_or_insert(Asking::default);
                asking.0.set(false);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rectangle(x: i32, y: i32, width: i32, height: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (width, height).into())
    }

    /// How many points of a 100 by 100 square `within` covers, and whether
    /// any twice.
    fn covered(within: &[Rectangle<i32, Logical>]) -> (usize, bool) {
        let mut count = 0;
        let mut twice = false;
        for y in 0..100 {
            for x in 0..100 {
                let times = within.iter().filter(|area| area.contains((x, y))).count();
                count += usize::from(times > 0);
                twice |= times > 1;
            }
        }
        (count, twice)
    }

    #[test]
    fn a_region_comes_to_what_was_added_less_what_was_taken_away_since_and_never_twice() {
        let region = RegionAttributes {
            rects: vec![
                (RectangleKind::Add, rectangle(0, 0, 50, 50)),
                (RectangleKind::Add, rectangle(25, 25, 50, 50)),
                (RectangleKind::Subtract, rectangle(0, 0, 10, 10)),
                (RectangleKind::Add, rectangle(0, 0, 5, 5)),
            ],
        };
        let within = rectangles(&region);
        // Two squares of 2500 overlapping by 625, less 100, plus 25.
        assert_eq!(covered(&within), (2500 + 2500 - 625 - 100 + 25, false));
        assert!(
            within.iter().any(|area| area.contains((2, 2))),
            "added again"
        );
        assert!(
            !within.iter().any(|area| area.contains((7, 7))),
            "taken away"
        );
        assert_eq!(rectangles(&RegionAttributes::default()), Vec::new());
    }
}
