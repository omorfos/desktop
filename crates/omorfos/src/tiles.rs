//! The line between two windows that share a workspace: drawn, pointed
//! at, and dragged to give one more room and the other less.
//!
//! Where the line is, and how far it may go, is the desktop model's
//! business. See `docs/adr/0039-windows-sharing-a-workspace.md`.

use omorfos_config::{Color, Part, State, States};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Arrangement, WindowId};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::utils::{Logical, Point};

use crate::compositor::{Compositor, color, logical_size};
use crate::input::Drag;
use crate::outputs::Head;

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
}

impl Compositor {
    /// Brings the lines that are drawn up to date with where the desktop
    /// has them and how the theme has them look: the colour of a window's
    /// border.
    pub(crate) fn lay_lines(&mut self) {
        let ink = self.config().theme.style(Part::Window, States::NONE);
        let ink = ink.border.top.color;
        let outputs = 0..self.heads.len();
        let lines = outputs.flat_map(|output| self.desktop.lines(output));
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
        let lines: Vec<(Rect, Color)> = lines.map(|line| (line.at, ink)).chain(offered).collect();
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

    /// Moves one of the lines of an output to where the pointer is.
    pub(crate) fn drag_line(&mut self, (output, which): (usize, usize), to: DomainPoint) {
        self.desktop.move_line(output, which, Some(to));
    }
}
