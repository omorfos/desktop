//! The line between two windows that share a workspace: drawn, pointed
//! at, and dragged to give one more room and the other less.
//!
//! Where the line is, and how far it may go, is the desktop model's
//! business. See `docs/adr/0039-windows-sharing-a-workspace.md`.

use std::sync::Arc;
use std::time::Duration;

use omorfos_config::{Color, Config, Part, State, States};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Arrangement, Offer, WindowId};
use smithay::backend::renderer::element::AsRenderElements as _;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::SurfaceCachedState;

use crate::Scale;
use crate::compositor::{Compositor, Element, Mapped, Painted, View, color, logical_size};
use crate::input::Drag;
use crate::outputs::Head;
use crate::paint;
use crate::x11::Role;

/// How far either side of a line the pointer takes hold of it, in units
/// of layout. The line itself is a hair wide.
const GRIP: i32 = 4;
/// How near a corner of the screen a frame dragged to an edge is offered
/// a quarter and not a half.
const CORNER: i32 = 64;
/// How strong the wash is that shows where a frame being dragged would be
/// put, out of 255.
const WASH: u32 = 80;

/// The lines being drawn, and what the pointer last did with one.
#[derive(Debug, Default)]
pub(crate) struct Lines {
    /// Each line's place and colour, and what it is drawn from, kept for
    /// as long as neither changes so that a line that has not moved is
    /// not drawn again.
    drawn: Vec<(Rect, Color, SolidColorBuffer)>,
    /// When a line was last pressed on, which is what a second press has
    /// to follow closely to be a double click.
    pub(crate) pressed: Option<u32>,
    /// The windows offered in parts with nothing in them, on every output,
    /// as they were when the lines were last laid.
    offers: Vec<Offer>,
    /// Their titles, painted, each kept until it would be painted
    /// differently.
    titles: Vec<Written>,
}

/// The title of a window offered in a part with nothing in it, painted,
/// with what it was painted from.
#[derive(Debug)]
struct Written {
    window: WindowId,
    title: String,
    /// How wide it may be, and the scale it was painted at.
    room: u32,
    scale: Scale,
    /// The configuration it was painted by, whose theme it follows.
    config: Arc<Config>,
    /// Where it is drawn, in the middle of the line it is written on.
    at: DomainPoint,
    pixels: Painted,
}

impl Compositor {
    /// Brings the lines that are drawn up to date with where the desktop
    /// has them and how the theme has them look.
    pub(crate) fn lay_lines(&mut self) {
        let ink = self.config().theme.style(Part::TileLine, States::NONE);
        let ink = ink.color;
        let outputs = 0..self.heads.len();
        let lines: Vec<_> = outputs
            .flat_map(|output| self.desktop.lines(output))
            .collect();
        // With them, the part a frame being dragged would be put in if it
        // were let go: a wash of the colour of a focused window's border.
        let focused = States::NONE.with(State::Focused);
        let wash = self.config().theme.style(Part::Window, focused);
        let wash = wash.border.top.color;
        let faint = |channel: u8| u8::try_from(u32::from(channel) * WASH / 255).unwrap_or(0);
        let wash = Color {
            red: faint(wash.red),
            green: faint(wash.green),
            blue: faint(wash.blue),
            alpha: faint(255),
        };
        let offered = self.offered().map(|(_, at, ..)| at);
        let offered = offered.or_else(|| self.carried_to()).map(|at| (at, wash));
        // Over the windows offered in a part with nothing in it, the same
        // wash on the one under the pointer, and a blank in the colour of
        // the `capture` part on any a rule says is private.
        let outputs = 0..self.heads.len();
        self.hover.lines.offers = outputs
            .flat_map(|output| self.desktop.offers(output))
            .collect();
        self.paint_titles();
        let point = self.pointer_position();
        let blank = self.config().theme.style(Part::Capture, States::NONE);
        let offers = self.hover.lines.offers.iter();
        let offers = offers.flat_map(|offer| {
            let private = self.desktop.private(offer.window);
            let under = offer.at.contains(point);
            let blank = private.then_some((offer.at, blank.background));
            blank.into_iter().chain(under.then_some((offer.at, wash)))
        });
        let lines: Vec<(Rect, Color)> = lines
            .iter()
            .map(|line| (line.at, ink))
            .chain(offered)
            .chain(offers)
            .collect();
        let drawn = self.hover.lines.drawn.iter();
        let same = drawn
            .map(|(at, ink, _)| (*at, *ink))
            .eq(lines.iter().copied());
        if !same {
            let buffer = |(at, ink): &(Rect, Color)| {
                (
                    *at,
                    *ink,
                    SolidColorBuffer::new(logical_size(at.size), color(*ink)),
                )
            };
            self.hover.lines.drawn = lines.iter().map(buffer).collect();
        }
    }

    /// The part of a workspace a frame being dragged would be put in if it
    /// were let go where the pointer is: against an edge of its output a
    /// half, and near a corner a quarter. Which window, where the part
    /// is, and which part of which arrangement.
    ///
    /// An edge that another output is beyond is not one: the pointer is on
    /// its way there.
    pub(crate) fn offered(&self) -> Option<(WindowId, Rect, Arrangement, usize)> {
        let Some(Drag::Move { window, .. }) = self.drag else {
            return None;
        };
        let point = self.pointer_location;
        let head = self.head_at(point)?;
        let area = head.area();
        let beyond = |dx: f64, dy: f64| {
            let past = point + Point::from((dx, dy));
            self.heads.iter().any(|head| head.contains(past))
        };
        let (left, top) = (f64::from(area.loc.x), f64::from(area.loc.y));
        let (right, bottom) = (left + f64::from(area.size.w), top + f64::from(area.size.h));
        let corner = f64::from(CORNER);
        // Against an edge, and near one.
        let at = [
            point.x < left + 1.0 && !beyond(-1.0, 0.0),
            point.x >= right - 1.0 && !beyond(1.0, 0.0),
            point.y < top + 1.0 && !beyond(0.0, -1.0),
            point.y >= bottom - 1.0 && !beyond(0.0, 1.0),
        ];
        let near = [
            point.x < left + corner,
            point.x >= right - corner,
            point.y < top + corner,
            point.y >= bottom - corner,
        ];
        let cornered = |side: usize, end: usize| (at[side] && near[end]) || (at[end] && near[side]);
        // Quarters are read down the left and then down the right.
        let quarters = [(0, 2), (0, 3), (1, 2), (1, 3)];
        let quarter = quarters
            .iter()
            .position(|(side, end)| cornered(*side, *end));
        let quarter = quarter.map(|part| (Arrangement::Quarters, part));
        let halves = [
            (Arrangement::SideBySide, 0),
            (Arrangement::SideBySide, 1),
            (Arrangement::Stacked, 0),
            (Arrangement::Stacked, 1),
        ];
        let half = at.iter().position(|at| *at).map(|edge| halves[edge]);
        let (arrangement, part) = quarter.or(half)?;
        let whole = Rect::new(
            DomainPoint::new(area.loc.x, area.loc.y),
            DomainSize::new(area.size.w.unsigned_abs(), area.size.h.unsigned_abs()),
        );
        Some((window, arrangement.parts(whole)[part], arrangement, part))
    }

    /// The part a window being carried from its own would change places
    /// with if it were let go where the pointer is.
    fn carried_to(&self) -> Option<Rect> {
        let Some(Drag::Tile { window }) = self.drag else {
            return None;
        };
        self.desktop.part_under(window, self.pointer_position())
    }

    /// Puts the frame being dragged in the part that was offered for it,
    /// if one was: on a workspace made for it, cut up that way. On the
    /// desktop it is left where it was before it was dragged, for when it
    /// comes back; or, dragged to another screen, in the middle of that.
    pub(crate) fn drop_at_edge(&mut self) {
        let from = match self.drag {
            Some(Drag::Move { from, size, .. }) => Some(Rect::new(from, size)),
            _ => None,
        };
        if let Some(Drag::Tile { window }) = self.drag {
            // Carried from one part of a workspace to another, it changes
            // places with what is there.
            self.desktop.swap_at(window, self.pointer_position());
        }
        let Some(((window, _, arrangement, part), from)) = self.offered().zip(from) else {
            return;
        };
        let here = self.head_at(self.pointer_location).map(Head::area);
        let was = self.head_for(from).map(Head::area);
        let middle = here.filter(|here| Some(*here) != was).map(|here| {
            let across = here.size.w - from.size.width.cast_signed();
            let down = here.size.h - from.size.height.cast_signed();
            DomainPoint::new(here.loc.x + across / 2, here.loc.y + down / 2)
        });
        let rest = Rect::new(middle.unwrap_or(from.origin), from.size);
        self.desktop.set_geometry(window, rest);
        self.desktop.arrange(window, arrangement, part);
    }

    /// The lines, to be drawn over the windows they are between.
    pub(crate) fn lines(
        &self,
        pixel: impl Fn(Point<f64, Logical>) -> Point<i32, smithay::utils::Physical>,
        scale: f64,
    ) -> impl Iterator<Item = SolidColorRenderElement> {
        self.hover.lines.drawn.iter().map(move |(at, _, buffer)| {
            let corner = Point::<i32, Logical>::from((at.origin.x, at.origin.y));
            let at = pixel(corner.to_f64());
            SolidColorRenderElement::from_buffer(buffer, at, scale, 1.0, Kind::Unspecified)
        })
    }

    /// The window offered in a part with nothing in it that is under the
    /// pointer, if one is.
    pub(crate) fn over_offer(&self) -> Option<WindowId> {
        let point = self.pointer_position();
        let offers = self.hover.lines.offers.iter();
        let mut under = offers.filter(|offer| offer.at.contains(point));
        under.next().map(|offer| offer.window)
    }

    /// Puts the window offered under the pointer in the part it is offered
    /// in.
    pub(crate) fn choose_offered(&mut self, window: WindowId) {
        self.desktop.choose(window, self.pointer_position());
    }

    /// Paints the title of each window offered in a part with nothing in it,
    /// keeping those that would be painted as they were.
    fn paint_titles(&mut self) {
        let config = Arc::clone(self.store.current());
        let wanted: Vec<_> = self
            .hover
            .lines
            .offers
            .iter()
            .map(|offer| {
                let window = self.desktop.window(offer.window);
                let title = window.map_or_else(String::new, |window| window.title.clone());
                let scale = self
                    .head_for(offer.title)
                    .map_or(Scale::ONE, |head| head.scale);
                (offer.window, title, offer.title, scale)
            })
            .collect();
        let mut had = std::mem::take(&mut self.hover.lines.titles);
        let titles = wanted.into_iter().map(|(window, title, line, scale)| {
            let room = line.size.width;
            let same = |kept: &Written| {
                kept.window == window
                    && kept.title == title
                    && kept.room == room
                    && kept.scale == scale
                    && Arc::ptr_eq(&kept.config, &config)
            };
            let kept = had.iter().position(same).map(|at| had.swap_remove(at));
            let mut painted = kept.unwrap_or_else(|| {
                let theme = &config.theme;
                let canvas = paint::offer_title(
                    &mut self.text,
                    theme,
                    &title,
                    (room, line.size.height),
                    scale,
                );
                Written {
                    window,
                    title,
                    room,
                    scale,
                    config: Arc::clone(&config),
                    at: line.origin,
                    pixels: Painted::new(&canvas),
                }
            });
            let width = painted.pixels.width();
            let left = (room.saturating_sub(width) / 2).cast_signed();
            painted.at = line.origin.offset(left, 0);
            painted
        });
        self.hover.lines.titles = titles.collect();
    }

    /// The titles of the windows offered in parts with nothing in them,
    /// under their pictures.
    pub(crate) fn offer_titles<R>(&self, renderer: &mut R, view: View) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let titles = self.hover.lines.titles.iter();
        let titles = titles.filter_map(|title| {
            let at = Point::<i32, Logical>::from((title.at.x, title.at.y));
            title.pixels.element(renderer, at.to_f64(), view, 1.0)
        });
        titles.collect()
    }

    /// The windows offered in parts with nothing in them, drawn small where
    /// each is offered, to be under the lines and what is laid over them.
    /// One a rule says is private is not drawn: a blank is laid over it.
    pub(crate) fn offer_elements<R>(&self, renderer: &mut R, view: View) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let offers = self.hover.lines.offers.iter();
        let offers = offers.filter(|offer| !self.desktop.private(offer.window));
        let shown =
            offers.filter_map(|offer| self.mapped(offer.window).map(|mapped| (offer, mapped)));
        let pictures = shown.flat_map(|(offer, mapped)| {
            // As small as it has to be to fit, which with its shape kept is
            // as small across as down.
            let size = mapped.window.geometry().size;
            let factor = f64::from(offer.at.size.width) / f64::from(size.w.max(1));
            let scale = view.scale.factor() * factor;
            let corner = Point::<i32, Logical>::from((offer.at.origin.x, offer.at.origin.y));
            let corner = view.pixel(corner.to_f64());
            let within = mapped.window.geometry().loc.to_f64().to_physical(scale);
            let at = corner - within.to_i32_round();
            let picture = logical_size(offer.at.size).to_f64();
            let crop = Rectangle::new(
                corner,
                picture.to_physical(view.scale.factor()).to_i32_round(),
            );
            let scale = smithay::utils::Scale::from(scale);
            let surfaces = mapped
                .window
                .render_elements::<WaylandSurfaceRenderElement<R>>(renderer, at, scale, 1.0);
            surfaces
                .into_iter()
                .filter_map(move |surface| CropRenderElement::from_element(surface, scale, crop))
        });
        pictures.map(Element::Cropped).collect()
    }

    /// Tells each window offered in a part with nothing in it on `output`
    /// that what it drew has been shown, so that its picture there is
    /// kept up to date as it would be on the screen.
    pub(crate) fn send_offered_frames(&self, output: &Output, elapsed: Duration) {
        let here = self.head(output).map(Head::area);
        let offers = self.hover.lines.offers.iter();
        let offers = offers.filter(|offer| {
            let corner = Point::<i32, Logical>::from((offer.at.origin.x, offer.at.origin.y));
            here.is_some_and(|here| here.contains(corner))
        });
        for mapped in offers.filter_map(|offer| self.mapped(offer.window)) {
            let output = output.clone();
            mapped
                .window
                .send_frame(&output, elapsed, Some(Duration::ZERO), |_, _| {
                    Some(output.clone())
                });
        }
    }

    /// The line the pointer can take hold of where it is: which output's,
    /// which of its lines, and whether it lies across and is dragged up
    /// and down.
    pub(crate) fn over_line(&self) -> Option<(usize, usize, bool)> {
        let point = self.pointer_position();
        let outputs = 0..self.heads.len();
        let mut lines = outputs.flat_map(|output| {
            let lines = self.desktop.lines(output).into_iter();
            lines.map(move |line| (output, line))
        });
        lines.find_map(|(output, line)| {
            let (out, up) = if line.across { (0, GRIP) } else { (GRIP, 0) };
            let grip = Rect::new(
                line.at.origin.offset(-out, -up),
                DomainSize::new(
                    line.at.size.width + 2 * out.unsigned_abs(),
                    line.at.size.height + 2 * up.unsigned_abs(),
                ),
            );
            grip.contains(point)
                .then_some((output, line.which, line.across))
        })
    }

    /// Moves one of the lines of an output to `to`, or with `None` back to
    /// the middle, as far as the windows either side of it can be made
    /// smaller.
    pub(crate) fn move_line(&mut self, (output, which): (usize, usize), to: Option<DomainPoint>) {
        self.desktop.move_line(output, which, to);
    }
}

/// The least a window says it can be, as its program last told: nothing
/// where it has not said.
pub(crate) fn least(mapped: &Mapped) -> DomainSize {
    let size = match &mapped.role {
        Role::Xdg(toplevel) => with_states(toplevel.wl_surface(), |states| {
            let mut cached = states.cached_state.get::<SurfaceCachedState>();
            cached.current().min_size
        }),
        Role::X11(surface) => surface.min_size().unwrap_or_default(),
    };
    DomainSize::new(size.w.max(0).unsigned_abs(), size.h.max(0).unsigned_abs())
}
