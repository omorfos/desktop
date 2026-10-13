//! The line between two windows that share a workspace: drawn, pointed
//! at, and dragged to give one more room and the other less.
//!
//! Where the line is, and how far it may go, is the desktop model's
//! business. See `docs/adr/0039-windows-sharing-a-workspace.md`.

use omorfos_config::{Color, Part, States};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::utils::{Logical, Point};

use crate::compositor::{Compositor, color, logical_size};

/// How far either side of a line the pointer takes hold of it, in units
/// of layout. The line itself is a hair wide.
const GRIP: i32 = 4;

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
        let lines = outputs.filter_map(|output| self.desktop.line(output));
        let lines: Vec<Rect> = lines.map(|line| line.at).collect();
        let drawn = self.hover.lines.drawn.iter();
        let same = drawn
            .map(|(at, ink, _)| (*at, *ink))
            .eq(lines.iter().map(|at| (*at, ink)));
        if !same {
            let buffer = |at: &Rect| SolidColorBuffer::new(logical_size(at.size), color(ink));
            let lines = lines.iter().map(|at| (*at, ink, buffer(at)));
            self.hover.lines.drawn = lines.collect();
        }
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
    /// and whether it lies across the output and is dragged up and down.
    pub(crate) fn over_line(&self) -> Option<(usize, bool)> {
        let point = self.pointer_position();
        let outputs = 0..self.heads.len();
        let mut lines = outputs.filter_map(|output| Some((output, self.desktop.line(output)?)));
        lines.find_map(|(output, line)| {
            let (out, up) = if line.across { (0, GRIP) } else { (GRIP, 0) };
            let grip = Rect::new(
                line.at.origin.offset(-out, -up),
                DomainSize::new(
                    line.at.size.width + 2 * out.unsigned_abs(),
                    line.at.size.height + 2 * up.unsigned_abs(),
                ),
            );
            grip.contains(point).then_some((output, line.across))
        })
    }

    /// Moves the line of an output to where the pointer is.
    pub(crate) fn drag_line(&mut self, output: usize, to: DomainPoint) {
        self.desktop.move_line(output, Some(to));
    }
}
