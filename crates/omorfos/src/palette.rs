//! The arrangements a window can be put in, shown as small pictures under
//! the second button of its titlebar when the pointer rests on it.
//!
//! A click on a part of a picture makes a workspace cut up that way, with
//! the window in that part. A click on the button itself still makes the
//! window fullscreen. See `docs/adr/0039-windows-sharing-a-workspace.md`.

use std::time::{Duration, Instant};

use omorfos_config::{Part, State, States, Theme};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Arrangement, Button, Home, WindowId};

use crate::Scale;
use crate::compositor::{Compositor, Painted};
use crate::paint::{Canvas, Text};

/// How long the pointer rests on the button before the pictures come.
const REST: Duration = Duration::from_millis(400);
/// The room round the pictures and between one and the next, the size of
/// each, and the room between the parts of one, in units of layout.
const PAD: u32 = 8;
const PICTURE: (u32, u32) = (44, 30);
const GAP: i32 = 2;
/// How far outside the pictures the pointer may stray before they go.
const STRAY: i32 = 8;
/// The height of the line of words under the pictures, which says what
/// the one under the pointer is.
const CAPTION: u32 = 22;
/// How far the arrow that points at the button stands up from the
/// pictures, and so how wide it is at its foot: twice that.
pub(crate) const ARROW: u32 = 6;

/// What each arrangement is called, and each of its parts, in the order
/// of [`Arrangement::ALL`] and of the parts.
const NAMES: [(&str, &[&str]); 7] = [
    ("Two side by side", &["on the left", "on the right"]),
    ("One above the other", &["at the top", "at the bottom"]),
    (
        "One beside two",
        &["on the left", "at the top right", "at the bottom right"],
    ),
    (
        "Two beside one",
        &["at the top left", "at the bottom left", "on the right"],
    ),
    (
        "One above two",
        &["at the top", "at the bottom left", "at the bottom right"],
    ),
    (
        "Two above one",
        &["at the top left", "at the top right", "at the bottom"],
    ),
    (
        "Four quarters",
        &[
            "at the top left",
            "at the bottom left",
            "at the top right",
            "at the bottom right",
        ],
    ),
];

/// What the words under the pictures say: what the part under the pointer
/// is, or what the pictures are for.
fn caption(picked: Option<(usize, usize)>) -> String {
    let named = picked.map(|(arrangement, part)| {
        let (name, parts) = NAMES[arrangement];
        format!("{name}: this window {}", parts[part])
    });
    named.unwrap_or_else(|| "Choose where this window goes".to_owned())
}

/// The pictures while they are up: whose, where, and which part of which
/// the pointer is on.
#[derive(Debug)]
pub(crate) struct Shown {
    window: WindowId,
    pub(crate) area: Rect,
    picked: Option<(usize, usize)>,
    /// How far along them the arrow that points at the button is.
    arrow: u32,
    /// What they were painted for, to know when to paint them again.
    look: (Theme, Scale),
    pub(crate) pixels: Painted,
}

/// The pictures as they are wanted: whose, where, how far along them the
/// arrow is, and which part of which the pointer is on.
type Wanted = (WindowId, Rect, u32, Option<(usize, usize)>);

/// The pictures of arrangements, and the pointer resting for them.
#[derive(Debug, Default)]
pub(crate) struct Palette {
    /// The window on whose button the pointer is resting, and since when.
    resting: Option<(WindowId, Instant)>,
    pub(crate) shown: Option<Shown>,
}

impl Palette {
    /// When the pictures the pointer is resting for are due.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        let waiting = self.resting.filter(|_| self.shown.is_none());
        waiting.map(|(_, since)| since + REST)
    }

    /// Takes the pictures away, as a key does.
    pub(crate) fn dismiss(&mut self) {
        self.shown = None;
        self.resting = None;
    }
}

/// Where each part of each arrangement is drawn, for pictures whose corner
/// is at `corner`: which arrangement, which of its parts, and where.
fn parts(corner: DomainPoint) -> impl Iterator<Item = (usize, usize, Rect)> {
    let pictures = Arrangement::ALL.into_iter().enumerate();
    pictures.flat_map(move |(index, arrangement)| {
        let along = u32::try_from(index).unwrap_or(0) * (PICTURE.0 + PAD) + PAD;
        let at = corner.offset(along.cast_signed(), PAD.cast_signed());
        let whole = Rect::new(at, DomainSize::new(PICTURE.0, PICTURE.1));
        let parts = arrangement.parts(whole).into_iter().enumerate();
        parts.map(move |(part, at)| (index, part, at))
    })
}

/// How large the pictures are altogether, with the words under them.
fn size() -> DomainSize {
    let count = u32::try_from(Arrangement::ALL.len()).unwrap_or(1);
    let height = PICTURE.1 + 2 * PAD + CAPTION;
    DomainSize::new(count * (PICTURE.0 + PAD) + PAD, height)
}

/// Paints the pictures: each part a block, and the one the pointer is on
/// picked out as a menu's row is; under them, in words, what that one is;
/// and over them an arrow, `arrow` along, that points up at the button
/// they belong to. The arrow stands [`ARROW`] above the rest.
fn paint(
    text: &mut Text,
    theme: &Theme,
    scale: Scale,
    (picked, arrow): (Option<(usize, usize)>, u32),
) -> Canvas {
    let size = size();
    let ground = theme.style(Part::Menu, States::NONE);
    let mut canvas = Canvas::scaled(size.width, size.height + ARROW, scale);
    let down = ARROW.cast_signed();
    canvas.draw_box((0, down, size.width, size.height), &ground);
    // The arrow: a row for each step up, each narrower than the last.
    for step in 0..ARROW {
        let half = ARROW - step;
        let left = arrow.saturating_sub(half).cast_signed();
        let row = (ARROW - 1 - step).cast_signed();
        canvas.fill((left, row, 2 * half, 1), ground.background);
    }
    let plain = theme.style(Part::MenuSeparator, States::NONE).color;
    let chosen = States::NONE.with(State::Hovered);
    let chosen = theme.style(Part::MenuItem, chosen).background;
    for (arrangement, part, at) in parts(DomainPoint::new(0, down)) {
        let ink = if picked == Some((arrangement, part)) {
            chosen
        } else {
            plain
        };
        // A little apart, so that the parts of a picture can be told.
        let width = at.size.width.saturating_sub(GAP.unsigned_abs());
        let height = at.size.height.saturating_sub(GAP.unsigned_abs());
        canvas.fill((at.origin.x, at.origin.y, width, height), ink);
    }
    let words = theme.style(Part::MenuItem, States::NONE).color;
    let font = theme.font(Part::MenuItem, States::NONE);
    let under = (PICTURE.1 + 2 * PAD).cast_signed() + down;
    let room = size.width.saturating_sub(2 * PAD);
    let line = (PAD.cast_signed(), under - 4, room, CAPTION);
    text.draw(&mut canvas, &caption(picked), line, words, font);
    canvas
}

impl Compositor {
    /// Whether the pointer is on the pictures, which then have it and
    /// nothing under them does.
    pub(crate) fn over_palette(&self) -> bool {
        let shown = self.hover.palette.shown.as_ref();
        shown.is_some_and(|shown| shown.area.contains(self.pointer_position()))
    }

    /// Puts the window the pictures are of in the part the pointer is on,
    /// if it is on one, and takes the pictures away.
    pub(crate) fn choose_arrangement(&mut self) {
        let shown = self.hover.palette.shown.take();
        let chosen = shown.and_then(|shown| Some((shown.window, shown.picked?)));
        if let Some((window, (arrangement, part))) = chosen {
            self.desktop
                .arrange(window, Arrangement::ALL[arrangement], part);
        }
    }

    /// Brings the pictures up when the pointer has rested on the second
    /// button of the titlebar of a window on a desktop, keeps them while
    /// it is on them or the button, and paints them as the pointer moves
    /// over them.
    pub(crate) fn offer_arrangements(&mut self, now: Instant) {
        let was = self.hover.palette.shown.take();
        let rested = self.hover.palette.resting;
        let (wanted, resting) = self.arrangements(was.as_ref(), rested, now);
        let scale = self.pointer_scale();
        self.hover.palette.shown = wanted.map(|(window, area, arrow, picked)| {
            self.paint_with(|text, theme| {
                // Painted again only if something about them has changed.
                let same = |shown: &Shown| {
                    let placed = (shown.window, shown.area, shown.arrow, shown.picked);
                    placed == (window, area, arrow, picked)
                        && shown.look.0 == *theme
                        && shown.look.1 == scale
                };
                was.filter(same).unwrap_or_else(|| Shown {
                    window,
                    area,
                    picked,
                    arrow,
                    look: (theme.clone(), scale),
                    pixels: Painted::new(&paint(text, theme, scale, (picked, arrow))),
                })
            })
        });
        self.hover.palette.resting = resting;
    }

    /// What pictures are wanted now and what the pointer is resting for,
    /// given what `was` shown and how long it had `rested`.
    fn arrangements(
        &self,
        was: Option<&Shown>,
        rested: Option<(WindowId, Instant)>,
        now: Instant,
    ) -> (Option<Wanted>, Option<(WindowId, Instant)>) {
        let point = self.pointer_position();
        let button = self.hover.button;
        let button = button.filter(|(_, which)| *which == Button::Maximize);
        let on = button.map(|(window, _)| window);
        // Where the pictures of a window would go, if it is on a desktop:
        // under its button, with the arrow at the middle of the button.
        let under = |window: WindowId| {
            let mut frames = self.desktop.frames();
            let frame = frames.find(|frame| frame.window().id == window)?;
            let button = frame.decoration?.button(Button::Maximize);
            let corner = button.origin.offset(0, button.size.height.cast_signed());
            let placed = (Rect::new(corner, size()), button.size.width / 2);
            (frame.home == Home::Desktop).then_some(placed)
        };
        let since = |window: WindowId| {
            let already = rested.filter(|(on, _)| *on == window);
            (window, already.map_or(now, |(_, since)| since))
        };
        let resting = on.filter(|_| self.drag.is_none()).map(since);
        let due = resting.filter(|(_, since)| now.saturating_duration_since(*since) >= REST);
        let kept = was.filter(|shown| {
            let grown = Rect::new(
                shown.area.origin.offset(-STRAY, -STRAY),
                DomainSize::new(
                    shown.area.size.width + 2 * STRAY.unsigned_abs(),
                    shown.area.size.height + 2 * STRAY.unsigned_abs(),
                ),
            );
            let near = grown.contains(point) || on == Some(shown.window);
            near && under(shown.window) == Some((shown.area, shown.arrow))
        });
        let wanted = kept.map(|shown| (shown.window, shown.area, shown.arrow));
        let wanted = wanted.or_else(|| {
            let (window, _) = due?;
            let (area, arrow) = under(window)?;
            Some((window, area, arrow))
        });
        let wanted = wanted.map(|(window, area, arrow)| {
            let picked = parts(area.origin).find(|(.., at)| at.contains(point));
            let picked = picked.map(|(arrangement, part, _)| (arrangement, part));
            (window, area, arrow, picked)
        });
        (wanted, resting)
    }
}
