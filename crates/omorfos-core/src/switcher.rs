//! The switcher on `Alt+Tab`: the windows in the order they were last
//! used, walked through while a key is held and chosen when it is let go.
//!
//! [`Used`] keeps the order of use from one opening of the switcher to the
//! next. [`Switcher`] is the switcher while it is open: the windows as
//! they stood when it opened, which of them is picked, and where each is
//! shown. The order stands still while it is open, so that a window passed
//! over on the way has not been used; the one chosen goes to the front
//! once the switcher has closed and it has focus.
//!
//! It shows the windows in rows ([`Switcher::layout`]), or one behind
//! another on a diagonal, as a queue ([`Switcher::queue`]): the one picked
//! in front, those after it going back up and to the right, each smaller
//! and fainter, and its title under it. Going on to the next window sends
//! the one in front to the back, and the queue turns there, out of the
//! front as it fades and into the back.

use crate::WindowId;
use crate::geometry::{Point, Rect, Size};

/// The windows in the order they were last used, the most recent first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Used {
    order: Vec<WindowId>,
}

impl Used {
    /// Nothing used yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Notes that `window` is the one in use now, which puts it first; the
    /// rest keep their places. Nothing in use changes nothing.
    pub fn note(&mut self, window: Option<WindowId>) {
        let Some(window) = window else { return };
        self.order.retain(|other| *other != window);
        self.order.insert(0, window);
    }

    /// Forgets a window that has gone.
    pub fn forget(&mut self, window: WindowId) {
        self.order.retain(|other| *other != window);
    }

    /// The windows there are, given in any order, in the order of use: those
    /// used, the most recent first, and then those never used, in the
    /// order given.
    #[must_use]
    pub fn sorted(&self, windows: impl IntoIterator<Item = WindowId>) -> Vec<WindowId> {
        let windows: Vec<WindowId> = windows.into_iter().collect();
        let used = self.order.iter().filter(|window| windows.contains(window));
        let mut sorted: Vec<WindowId> = used.copied().collect();
        let unused: Vec<WindowId> = windows
            .into_iter()
            .filter(|window| !sorted.contains(window))
            .collect();
        sorted.extend(unused);
        sorted
    }
}

/// How the switcher is laid out: how large each window's place is, the
/// room between places, the room inside the switcher's edges, and the
/// room kept between it and the edges of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitcherMetrics {
    /// The size of each window's place, its picture and title together.
    pub item: Size,
    /// The room between one place and the next, across and down.
    pub gap: u32,
    /// The room between the switcher's edges and the places inside it.
    pub inset: u32,
    /// The least room kept between the switcher and the screen's edges.
    pub margin: u32,
}

/// How the switcher stands the windows on a diagonal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueMetrics {
    /// The widest the window in front is made.
    pub widest: u32,
    /// How tall the title under it is.
    pub title: u32,
    /// The room between the window in front and its title.
    pub gap: u32,
    /// The least room kept between it all and the screen's edges.
    pub margin: u32,
}

/// One window as the switcher shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwitcherItem {
    /// The window.
    pub window: WindowId,
    /// Where it is shown.
    pub place: Rect,
    /// Whether it is the one picked.
    pub picked: bool,
    /// How much of it is seen, in thousandths: 1000 is all of it.
    pub opacity: u16,
}

/// Where the switcher and each window in it are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitcherLayout {
    /// The switcher's box: round the rows, or under the queue, round the
    /// title of the window picked.
    pub area: Rect,
    /// Each window shown, the one in front first where they stand one
    /// behind another. Only those that fit are shown, and the one picked
    /// always is: as the window of its program gone to among them, if one
    /// has been.
    pub items: Vec<SwitcherItem>,
    /// The windows of the program picked: in rows, its others stacked
    /// behind its place, the next to be gone to first, each smaller than
    /// the one before; in the queue, all of them side by side in a row
    /// under it, in the order of use as it was picked, the one gone to
    /// picked. None but for a program of more than one window.
    pub program: Vec<SwitcherItem>,
    /// In the queue, where the ring round the one gone to among the
    /// program's windows in its row is.
    pub focus: Option<Rect>,
}

/// How much smaller each window in the queue is than the one before it.
const SHRINK: f64 = 0.85;
/// How far each window in the queue is to the right of the one before it,
/// and above it, as a share of how wide the one in front is.
const STEP: (f64, f64) = (0.3, 0.18);
/// How much fainter each window in the queue is than the one before it.
const FAINTER: f64 = 0.12;
/// The most windows the queue shows at once.
const DEEPEST: usize = 6;
/// How tall a window's place in the queue is for how wide it is.
const SHAPE: f64 = 10.0 / 16.0;

/// A length worked out as a fraction, as the nearest whole one.
#[expect(
    clippy::cast_possible_truncation,
    reason = "within a screen, far inside an i32"
)]
fn whole(length: f64) -> i32 {
    length.round() as i32
}

/// Where a window `depth` places back in the queue goes, and how much of
/// it is seen, with the window in front `width` wide and its lower left
/// corner at `corner`. A depth below none is a window on its way out of
/// the front, which is larger.
fn standing(depth: f64, width: f64, corner: (f64, f64)) -> (Rect, f64) {
    let scale = SHRINK.powf(depth);
    let size = (width * scale, width * scale * SHAPE);
    let left = STEP.0.mul_add(depth * width, corner.0);
    let bottom = (-STEP.1).mul_add(depth * width, corner.1);
    let place = Rect::new(
        Point::new(whole(left), whole(bottom - size.1)),
        Size::new(
            whole(size.0).max(1).cast_unsigned(),
            whole(size.1).max(1).cast_unsigned(),
        ),
    );
    let faint = FAINTER.mul_add(-depth.max(0.0), 1.0);
    (place, faint)
}

/// How far each window stacked behind the program's in rows is above the
/// one before it, as a share of how tall that is.
const RISE: f64 = 0.12;
/// How wide each window in the row under the queue is, as a share of how
/// wide the one in front is.
const COLUMN: f64 = 0.34;
/// How much of each of the program's windows in the row under the queue is
/// seen but the one gone to, so that that stands out.
const DIM: f64 = 0.6;
/// How much of the row under the queue is seen above the screen's bottom
/// edge before it is raised, as a share of a window in it.
const PEEK: f64 = 0.25;

/// The windows of the program picked, and which of them is gone to.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Within {
    /// In the order of use, the program's window that stands for it first.
    windows: Vec<WindowId>,
    at: usize,
    /// Whether one has been gone to among them, by the key that goes on to
    /// the next window of a program.
    gone: bool,
}

/// How much of a window `depth` places behind another is seen, in
/// thousandths, as it fades further back.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "within 0 to 1000 by the clamp"
)]
fn faded(depth: f64) -> u16 {
    (FAINTER.mul_add(-depth, 1.0) * 1000.0)
        .round()
        .clamp(0.0, 1000.0) as u16
}

/// The switcher, while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switcher {
    windows: Vec<WindowId>,
    at: usize,
    within: Option<Within>,
    /// For each program gone through, by the window that stands for it, the
    /// one of its windows gone to, to be at again when it is picked again.
    gone: Vec<(WindowId, WindowId)>,
}

impl Switcher {
    /// Opens the switcher on `windows`, in the order of use. Going forward
    /// it starts at the window used before the one in use, so that letting
    /// go at once goes back to it; going back it starts at the one used
    /// longest ago. With no windows there is nothing to open.
    #[must_use]
    pub fn open(windows: Vec<WindowId>, back: bool) -> Option<Self> {
        let count = windows.len();
        let at = if back {
            count.saturating_sub(1)
        } else {
            usize::from(count > 1)
        };
        (count > 0).then_some(Self {
            windows,
            at,
            within: None,
            gone: Vec::new(),
        })
    }

    /// Has it show the program picked's `windows`, in the order of use:
    /// the one picked stands for the program, and the others are shown
    /// with it, to be gone through, at the one gone to the last time the
    /// program was picked while it was open, if one was. A program of one
    /// window shows nothing more.
    pub fn show_program(&mut self, mut windows: Vec<WindowId>) {
        let picked = self.picked();
        let first = windows.iter().position(|window| *window == picked);
        if let Some(first) = first {
            windows.rotate_left(first);
        }
        let before = self.gone.iter().find(|(stands, _)| *stands == picked);
        let at = before.and_then(|(_, gone)| windows.iter().position(|window| window == gone));
        self.within = (first.is_some() && windows.len() > 1).then_some(Within {
            windows,
            at: at.unwrap_or(0),
            gone: at.is_some(),
        });
    }

    /// Goes on to the next of the program picked's windows, or back to the
    /// one before if `back`, going round. Says whether there were any to go
    /// through.
    pub fn go_within(&mut self, back: bool) -> bool {
        let Some(within) = self.within.as_mut() else {
            return false;
        };
        let count = within.windows.len();
        within.at = (within.at + if back { count - 1 } else { 1 }) % count;
        within.gone = true;
        // Kept, for the program picked again.
        let (stands, gone) = (within.windows[0], within.windows[within.at]);
        self.gone.retain(|(other, _)| *other != stands);
        self.gone.push((stands, gone));
        true
    }

    /// The window to go to: the one gone to among the program picked's, or
    /// the one picked.
    #[must_use]
    pub fn chosen(&self) -> WindowId {
        let within = self.within.as_ref();
        within.map_or_else(|| self.picked(), |within| within.windows[within.at])
    }

    /// The window a program's place shows, by the window that stands for
    /// it: the one gone to within it while the switcher has been open, if
    /// one has been, or that.
    fn shown_for(&self, stands: WindowId) -> WindowId {
        let gone = self.gone.iter().find(|(other, _)| *other == stands);
        gone.map_or(stands, |(_, gone)| *gone)
    }

    /// The program picked's windows, in the order of use, the one that
    /// stands for it first; none but for a program of more than one.
    #[must_use]
    pub fn program(&self) -> &[WindowId] {
        self.within.as_ref().map_or(&[], |within| &within.windows)
    }

    /// Whether one has been gone to among the program picked's windows.
    #[must_use]
    pub fn gone_within(&self) -> bool {
        self.within.as_ref().is_some_and(|within| within.gone)
    }

    /// The program picked's windows but the one gone to, the next first,
    /// as many as `most`.
    fn others(&self, most: usize) -> Vec<WindowId> {
        let within = self.within.iter();
        let others = within.flat_map(|within| {
            let count = within.windows.len();
            (1..count.min(most + 1)).map(move |after| within.windows[(within.at + after) % count])
        });
        others.collect()
    }

    /// The windows, in the order they stood when it opened.
    #[must_use]
    pub fn windows(&self) -> &[WindowId] {
        &self.windows
    }

    /// The window picked.
    #[must_use]
    pub fn picked(&self) -> WindowId {
        // There is always one, and `at` is kept among them.
        self.windows[self.at]
    }

    /// Picks the next window, going round from the last to the first.
    pub fn next(&mut self) {
        self.at = (self.at + 1) % self.windows.len();
        self.within = None;
    }

    /// Picks the window before, going round from the first to the last.
    pub fn previous(&mut self) {
        self.at = (self.at + self.windows.len() - 1) % self.windows.len();
        self.within = None;
    }

    /// Picks `window`, if it is in the switcher.
    pub fn pick(&mut self, window: WindowId) {
        if let Some(at) = self.windows.iter().position(|other| *other == window) {
            self.at = at;
            self.within = None;
        }
    }

    /// Takes away a window that has gone; the one after it is picked if it
    /// was. With none left the switcher is gone too.
    #[must_use]
    pub fn without(mut self, window: WindowId) -> Option<Self> {
        // Gone from the program's, which stands for it as the one used next.
        if let Some(within) = self.within.take() {
            let windows: Vec<WindowId> = within
                .windows
                .iter()
                .copied()
                .filter(|other| *other != window)
                .collect();
            let at = within.windows[within.at];
            let at = windows.iter().position(|other| *other == at).unwrap_or(0);
            self.within = (windows.len() > 1).then_some(Within {
                windows,
                at,
                ..within
            });
        }
        if let Some(at) = self.windows.iter().position(|other| *other == window) {
            self.windows.remove(at);
            if at < self.at || self.at == self.windows.len() {
                self.at = self.at.saturating_sub(1);
            }
        }
        (!self.windows.is_empty()).then_some(self)
    }

    /// Where the switcher and the windows in it are shown, in the middle of
    /// `screen`: in rows, as many across as fit, and as many rows as fit,
    /// those shown being the ones that end with the row of the window
    /// picked, so that it is always in sight.
    #[must_use]
    pub fn layout(&self, screen: Rect, metrics: SwitcherMetrics) -> SwitcherLayout {
        let SwitcherMetrics {
            item,
            gap,
            inset,
            margin,
        } = metrics;
        let fit = |room: u32, size: u32| {
            let room = room.saturating_sub(2 * (margin + inset)) + gap;
            (room / (size + gap).max(1)).max(1)
        };
        let count = u32::try_from(self.windows.len()).unwrap_or(u32::MAX);
        let across = fit(screen.size.width, item.width).min(count);
        let rows = count.div_ceil(across);
        let shown_rows = fit(screen.size.height, item.height).min(rows);
        let row_of_picked = u32::try_from(self.at).unwrap_or(0) / across;
        let first_row = (row_of_picked + 1).saturating_sub(shown_rows);
        let span = |many: u32, size: u32| many * size + many.saturating_sub(1) * gap + 2 * inset;
        let size = Size::new(span(across, item.width), span(shown_rows, item.height));
        let left =
            screen.origin.x + (screen.size.width.saturating_sub(size.width) / 2).cast_signed();
        let top =
            screen.origin.y + (screen.size.height.saturating_sub(size.height) / 2).cast_signed();
        let area = Rect::new(Point::new(left, top), size);
        let shown = (first_row * across) as usize..((first_row + shown_rows) * across) as usize;
        let items = self
            .windows
            .iter()
            .enumerate()
            .filter(|(at, _)| shown.contains(at));
        let items = items.map(|(at, window)| {
            let at32 = u32::try_from(at).unwrap_or(0);
            let (column, row) = (at32 % across, at32 / across - first_row);
            let x = left + (inset + column * (item.width + gap)).cast_signed();
            let y = top + (inset + row * (item.height + gap)).cast_signed();
            let picked = at == self.at;
            SwitcherItem {
                window: if picked {
                    self.chosen()
                } else {
                    self.shown_for(*window)
                },
                place: Rect::new(Point::new(x, y), item),
                picked,
                opacity: 1000,
            }
        });
        let items: Vec<SwitcherItem> = items.collect();
        // Behind the one picked, its program's others, each smaller and a
        // little higher than the one before, and fainter.
        let front = items.iter().find(|item| item.picked).map(|item| item.place);
        let others = self.others(DEEPEST - 1);
        let program = front.into_iter().flat_map(|front| {
            others.iter().zip(1_u32..).map(move |(window, depth)| {
                let depth = f64::from(depth);
                let scale = SHRINK.powf(depth);
                let (width, height) = (f64::from(front.size.width), f64::from(front.size.height));
                let size = (width * scale, height * scale);
                let left = f64::from(front.origin.x) + (width - size.0) / 2.0;
                let top = (-RISE * depth).mul_add(height, f64::from(front.origin.y));
                SwitcherItem {
                    window: *window,
                    place: Rect::new(
                        Point::new(whole(left), whole(top)),
                        Size::new(whole(size.0).cast_unsigned(), whole(size.1).cast_unsigned()),
                    ),
                    picked: false,
                    opacity: faded(depth),
                }
            })
        });
        SwitcherLayout {
            area,
            items,
            program: program.collect(),
            focus: None,
        }
    }

    /// Where the switcher and the windows in it are shown one behind
    /// another on a diagonal, in the middle of `screen`: the one picked in
    /// front, as wide as fits up to `metrics.widest`, with the switcher's
    /// box under it for its title, and those after it, going round, up and
    /// to the right behind it, six at most, the last fading as it goes
    /// beyond them. `turning` is how far the queue has still to turn to
    /// stand so, in windows: below none while the one that was in front
    /// goes to the back, above while one comes from the back to the front.
    /// Going round, a window fades out of the front, larger, and into the
    /// back. Under the box, the program picked's other windows stand side by
    /// side in a row, `peeked` of the way up from wholly below the screen's
    /// bottom edge to where a little of each is seen, and from there
    /// `raised` of the way to just under the box,
    /// the queue going up as far as it can to make room for it, or as near
    /// it as leaves them whole on the screen.
    #[must_use]
    pub fn queue(
        &self,
        screen: Rect,
        metrics: QueueMetrics,
        (turning, peeked, raised, sliding): (f64, f64, f64, f64),
    ) -> SwitcherLayout {
        let QueueMetrics {
            widest,
            title,
            gap,
            margin,
        } = metrics;
        let count = self.windows.len();
        let windows = f64::from(u32::try_from(count).unwrap_or(u32::MAX));
        let deep = f64::from(u32::try_from(count.min(DEEPEST)).unwrap_or(1));
        // How far across and up the queue reaches, beside how wide the one
        // in front is.
        let depths =
            (0..count.min(DEEPEST)).map(|depth| f64::from(u32::try_from(depth).unwrap_or(0)));
        let (across, up) = depths.fold((0.0_f64, 0.0_f64), |(across, up), depth| {
            let scale = SHRINK.powf(depth);
            let reach = STEP.0.mul_add(depth, scale);
            let height = STEP.1.mul_add(depth, scale * SHAPE);
            (across.max(reach), up.max(height))
        });
        let room = |length: u32, taken: u32| f64::from(length.saturating_sub(2 * margin + taken));
        let width = f64::from(widest)
            .min(room(screen.size.width, 0) / across)
            .min(room(screen.size.height, gap + title) / up)
            .max(1.0);
        let tall = width.mul_add(up, f64::from(gap + title));
        let left = (f64::from(screen.size.width) - width * across) / 2.0;
        let left = f64::from(screen.origin.x) + left;
        let top = f64::from(screen.origin.y) + (f64::from(screen.size.height) - tall) / 2.0;
        // The row of the program's others, and as it rises, the queue going
        // up to make room for it under its box, as far as the margin at the
        // screen's top.
        let (narrow, short) = (width * COLUMN, width * COLUMN * SHAPE);
        let bottom = f64::from(screen.origin.y) + f64::from(screen.size.height);
        let wanted = top + tall + f64::from(gap) + short - (bottom - f64::from(margin));
        let free = top - f64::from(screen.origin.y) - f64::from(margin);
        let row = !self.others(1).is_empty();
        let rise = if row { wanted.min(free).max(0.0) } else { 0.0 };
        let raised = raised.clamp(0.0, 1.0);
        let top = rise.mul_add(-raised, top);
        let corner = (left, width.mul_add(up, top));
        // A single window has nowhere to turn to.
        let turning = if count > 1 { turning } else { 0.0 };
        let mut cards: Vec<(f64, SwitcherItem)> = self
            .windows
            .iter()
            .enumerate()
            .filter_map(|(at, window)| {
                let after = (at + count - self.at) % count;
                let after = f64::from(u32::try_from(after).unwrap_or(0));
                let place = (after - turning).rem_euclid(windows);
                // Between the back and the front again, it is on its way
                // out of the front for the first half, and into the back
                // for the second.
                let gone = windows - place;
                let (depth, share) = if place <= windows - 1.0 {
                    (place, 1.0)
                } else if gone < 0.5 {
                    (-gone, 2.0_f64.mul_add(-gone, 1.0))
                } else {
                    (place, 2.0_f64.mul_add(gone, -1.0))
                };
                let share = share * (deep - depth).min(1.0);
                let (place, faint) = standing(depth, width, corner);
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "within 0 to 1000 by the clamp"
                )]
                let opacity = (faint * share * 1000.0).round().clamp(0.0, 1000.0) as u16;
                let picked = at == self.at;
                let card = SwitcherItem {
                    window: if picked {
                        self.chosen()
                    } else {
                        self.shown_for(*window)
                    },
                    place,
                    picked,
                    opacity,
                };
                (opacity > 0).then_some((depth, card))
            })
            .collect();
        cards.sort_by(|(one, _), (other, _)| one.total_cmp(other));
        let area = Rect::new(
            Point::new(whole(left), whole(corner.1) + gap.cast_signed()),
            Size::new(whole(width).cast_unsigned(), title),
        );
        let place = (area, left + width / 2.0, (narrow, short));
        let screen_sides = (f64::from(screen.origin.x), f64::from(screen.size.width));
        let edges = (bottom, screen_sides, f64::from(margin));
        let (program, focus) = self.row(place, (edges, f64::from(gap)), (peeked, raised, sliding));
        SwitcherLayout {
            area,
            items: cards.into_iter().map(|(_, card)| card).collect(),
            program,
            focus,
        }
    }

    /// The row of the program picked's windows under the queue's box,
    /// `area`, side by side about `middle`, each `size` large, `gap` apart,
    /// within the screen's sides and above its `bottom`, by its `margin`;
    /// `raised` of the way up: just under the box, or as near it as leaves
    /// them whole on the screen, from mostly below the bottom, a little of
    /// each seen, which they come up to from wholly below it, `peeked` of
    /// the way there. The one gone to has the ring round it, and the others
    /// are fainter. As many as fit are shown; with more, the row slides to
    /// keep the ring in sight, `sliding` windows short of it on its way
    /// there, those towards its ends fainter still, and those beyond them
    /// not shown.
    fn row(
        &self,
        (area, middle, (across, down)): (Rect, f64, (f64, f64)),
        ((bottom, (left, wide), margin), gap): ((f64, (f64, f64), f64), f64),
        (peeked, raised, sliding): (f64, f64, f64),
    ) -> (Vec<SwitcherItem>, Option<Rect>) {
        let Some(within) = self.within.as_ref() else {
            return (Vec::new(), None);
        };
        let count = within.windows.len();
        let many = f64::from(u32::try_from(count).unwrap_or(u32::MAX));
        let step = across + gap;
        let room = 2.0f64.mul_add(-margin, wide);
        // How many fit, and how long the row is that they make.
        let fit = ((room + gap) / step).floor().max(1.0).min(many);
        let long = fit.mul_add(step, -gap);
        // In the middle under the one in front, but within the screen.
        let first = (middle - long / 2.0)
            .min(left + wide - margin - long)
            .max(left + margin);
        // The ring, on its way; and the row, slid to keep it in sight.
        let at = f64::from(u32::try_from(within.at).unwrap_or(0)) - sliding;
        let half = (fit - 1.0) / 2.0;
        let shown = if many > fit {
            at.clamp(half, many - 1.0 - half) - half
        } else {
            0.0
        };
        let under = f64::from(area.origin.y) + f64::from(area.size.height) + gap;
        let up = under.min(bottom - margin - down);
        let low = (-PEEK * peeked.clamp(0.0, 1.0)).mul_add(down, bottom);
        let top = (up - low).mul_add(raised, low);
        let place = |at: f64| {
            Rect::new(
                Point::new(whole((at - shown).mul_add(step, first)), whole(top)),
                Size::new(whole(across).cast_unsigned(), whole(down).cast_unsigned()),
            )
        };
        // Fainter the nearer the end of the row they are where more are
        // beyond it, none past it.
        let (start, end) = (first, first + long);
        let seen = |x: f64| {
            let inside = (x - start).min(end - (x + across));
            if many > fit {
                (inside / step + 1.0).clamp(0.0, 1.0)
            } else {
                1.0
            }
        };
        let row = within
            .windows
            .iter()
            .zip(0_u32..)
            .filter_map(|(window, index)| {
                let place = place(f64::from(index));
                let picked = index as usize == within.at;
                let dim = if picked { 1.0 } else { DIM };
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "within 0 to 1000 by the clamp"
                )]
                let opacity = (seen(f64::from(place.origin.x)) * dim * 1000.0).round() as u16;
                (opacity > 0).then_some(SwitcherItem {
                    window: *window,
                    place,
                    picked,
                    opacity,
                })
            });
        let gone_to = f64::from(u32::try_from(within.at).unwrap_or(0));
        (row.collect(), Some(place(gone_to)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(numbers: &[u64]) -> Vec<WindowId> {
        numbers
            .iter()
            .map(|number| WindowId::from_raw(*number))
            .collect()
    }

    fn picked(switcher: &Switcher) -> u64 {
        switcher.picked().raw()
    }

    #[test]
    fn the_order_of_use_puts_the_last_used_first_and_the_rest_keep_their_places() {
        let mut used = Used::new();
        for number in [1, 2, 3] {
            used.note(Some(WindowId::from_raw(number)));
        }
        assert_eq!(used.sorted(ids(&[1, 2, 3])), ids(&[3, 2, 1]));
        used.note(Some(WindowId::from_raw(1)));
        assert_eq!(used.sorted(ids(&[1, 2, 3])), ids(&[1, 3, 2]));
        used.note(None);
        assert_eq!(
            used.sorted(ids(&[1, 2, 3])),
            ids(&[1, 3, 2]),
            "nothing in use"
        );
        // Windows never used come after, as they were given; one gone is
        // not there.
        used.forget(WindowId::from_raw(3));
        assert_eq!(used.sorted(ids(&[5, 2, 4, 1])), ids(&[1, 2, 5, 4]));
        assert_eq!(
            used,
            Used {
                order: ids(&[1, 2])
            }
        );
    }

    #[test]
    fn going_forward_starts_at_the_window_before_and_back_at_the_oldest() {
        let switcher = Switcher::open(ids(&[3, 2, 1]), false).unwrap();
        assert_eq!(picked(&switcher), 2);
        assert_eq!(switcher.windows(), ids(&[3, 2, 1]));
        assert_eq!(picked(&Switcher::open(ids(&[3, 2, 1]), true).unwrap()), 1);
        // One window is the one picked either way, and none is no switcher.
        assert_eq!(picked(&Switcher::open(ids(&[7]), false).unwrap()), 7);
        assert_eq!(picked(&Switcher::open(ids(&[7]), true).unwrap()), 7);
        assert_eq!(Switcher::open(Vec::new(), false), None);
    }

    #[test]
    fn walking_goes_round_both_ways_and_a_window_can_be_picked_as_it_is() {
        let mut switcher = Switcher::open(ids(&[1, 2, 3]), false).unwrap();
        switcher.next();
        assert_eq!(picked(&switcher), 3);
        switcher.next();
        assert_eq!(picked(&switcher), 1, "round to the first");
        switcher.previous();
        assert_eq!(picked(&switcher), 3, "and back round to the last");
        switcher.pick(WindowId::from_raw(2));
        assert_eq!(picked(&switcher), 2);
        switcher.pick(WindowId::from_raw(9));
        assert_eq!(picked(&switcher), 2, "one not in it is not picked");
    }

    #[test]
    fn a_window_that_goes_is_taken_out_and_the_next_is_picked_in_its_place() {
        let switcher = Switcher::open(ids(&[1, 2, 3, 4]), false).unwrap();
        // The one picked goes: the one after it is picked.
        let switcher = switcher.without(WindowId::from_raw(2)).unwrap();
        assert_eq!(
            (switcher.windows(), picked(&switcher)),
            (&ids(&[1, 3, 4])[..], 3)
        );
        // One before it goes: the same one stays picked.
        let switcher = switcher.without(WindowId::from_raw(1)).unwrap();
        assert_eq!(picked(&switcher), 3);
        // One not in it changes nothing.
        let switcher = switcher.without(WindowId::from_raw(9)).unwrap();
        assert_eq!(switcher.windows(), ids(&[3, 4]));
        // The last, picked, goes: the one before is picked.
        let mut switcher = switcher;
        switcher.next();
        let switcher = switcher.without(WindowId::from_raw(4)).unwrap();
        assert_eq!(picked(&switcher), 3);
        assert_eq!(switcher.without(WindowId::from_raw(3)), None, "none left");
    }

    const METRICS: SwitcherMetrics = SwitcherMetrics {
        item: Size::new(100, 80),
        gap: 10,
        inset: 5,
        margin: 20,
    };

    fn screen(width: u32, height: u32) -> Rect {
        Rect::new(Point::new(1000, 0), Size::new(width, height))
    }

    #[test]
    fn windows_are_laid_out_in_a_row_in_the_middle_of_the_screen() {
        let switcher = Switcher::open(ids(&[1, 2, 3]), false).unwrap();
        let layout = switcher.layout(screen(1000, 600), METRICS);
        // Three across with two gaps between and the inset round.
        let size = Size::new(3 * 100 + 2 * 10 + 2 * 5, 80 + 2 * 5);
        assert_eq!(
            layout.area,
            Rect::new(Point::new(1000 + (1000 - 330) / 2, (600 - 90) / 2), size)
        );
        let x = |column: i32| 1000 + 335 + 5 + column * 110;
        let expected =
            [(1, 0, false), (2, 1, true), (3, 2, false)].map(|(id, column, picked)| SwitcherItem {
                window: WindowId::from_raw(id),
                place: Rect::new(Point::new(x(column), 255 + 5), METRICS.item),
                picked,
                opacity: 1000,
            });
        assert_eq!(layout.items, expected);
    }

    #[test]
    fn as_many_go_across_as_fit_and_the_rows_shown_keep_the_picked_one_in_sight() {
        // 370 across holds three; 330 down holds three rows.
        let mut switcher =
            Switcher::open(ids(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]), false).unwrap();
        let layout = |switcher: &Switcher| switcher.layout(screen(370, 330), METRICS);
        let shown = |switcher: &Switcher| {
            let layout = layout(switcher);
            layout
                .items
                .iter()
                .map(|item| item.window.raw())
                .collect::<Vec<_>>()
        };
        assert_eq!(shown(&switcher), [1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(
            layout(&switcher).area.size,
            Size::new(330, 3 * 80 + 2 * 10 + 10)
        );
        switcher.pick(WindowId::from_raw(11));
        assert_eq!(
            shown(&switcher),
            [4, 5, 6, 7, 8, 9, 10, 11],
            "the rows that end with it"
        );
        let picked = layout(&switcher).items.into_iter().find(|item| item.picked);
        assert_eq!(
            picked.map(|item| (item.window.raw(), item.place.origin.y)),
            Some((11, (330 - 270) / 2 + 5 + 2 * 90))
        );
    }

    #[test]
    fn a_screen_too_small_for_one_still_shows_the_one_picked() {
        let switcher = Switcher::open(ids(&[1, 2]), false).unwrap();
        let layout = switcher.layout(screen(50, 40), METRICS);
        let items: Vec<_> = layout
            .items
            .iter()
            .map(|item| (item.window.raw(), item.picked))
            .collect();
        assert_eq!(items, [(2, true)]);
        assert_eq!(
            layout.area.origin,
            Point::new(1000, 0),
            "from the corner, as it does not fit"
        );
    }

    const QUEUE: QueueMetrics = QueueMetrics {
        widest: 400,
        title: 30,
        gap: 10,
        margin: 20,
    };

    /// Each window the queue shows, front to back: which, where, and how
    /// much of it is seen.
    fn queued(switcher: &Switcher, screen: Rect, turning: f64) -> Vec<(u64, Rect, u16)> {
        let layout = switcher.queue(screen, QUEUE, (turning, 1.0, 0.0, 0.0));
        let items = layout.items.iter();
        items
            .map(|item| (item.window.raw(), item.place, item.opacity))
            .collect()
    }

    #[test]
    fn the_queue_stands_the_one_picked_in_front_and_the_rest_behind_it_going_round() {
        let switcher = Switcher::open(ids(&[3, 2, 1]), false).unwrap();
        let layout = switcher.queue(screen(1600, 1000), QUEUE, (0.0, 1.0, 0.0, 0.0));
        let order: Vec<_> = layout
            .items
            .iter()
            .map(|item| (item.window.raw(), item.picked))
            .collect();
        assert_eq!(order, [(2, true), (1, false), (3, false)], "front first");
        let places: Vec<_> = layout.items.iter().map(|item| item.place).collect();
        // The one in front as wide as it may be, each behind smaller, to
        // the right and above, and fainter.
        assert_eq!(places[0].size, Size::new(400, 250));
        assert_eq!(places[1].size, Size::new(340, 213));
        assert_eq!(places[2].size, Size::new(289, 181));
        assert_eq!(places[1].origin.x - places[0].origin.x, 120);
        let up = places[0].bottom() - places[1].bottom();
        assert!((71..=72).contains(&up), "{up}, as each is rounded");
        let opacities: Vec<_> = layout.items.iter().map(|item| item.opacity).collect();
        assert_eq!(opacities, [1000, 880, 760]);
        // All of it in the middle of the screen, the title under the one
        // in front, as wide as it.
        let right = places.iter().map(|place| place.right()).max().unwrap();
        let top = places.iter().map(|place| place.origin.y).min().unwrap();
        let left = i64::from(places[0].origin.x) - 1000;
        assert!((left - (1600 - right + 1000)).abs() <= 1, "{left} {right}");
        assert_eq!(
            layout.area,
            Rect::new(
                Point::new(places[0].origin.x, places[0].origin.y + 250 + 10),
                Size::new(400, 30)
            )
        );
        let bottom = layout.area.bottom();
        assert!(
            (i64::from(top) - (1000 - bottom)).abs() <= 1,
            "{top} {bottom}"
        );
    }

    #[test]
    fn the_queue_shows_six_at_most_and_is_made_small_enough_to_fit() {
        let switcher = Switcher::open(ids(&[1, 2, 3, 4, 5, 6, 7, 8]), false).unwrap();
        let shown = queued(&switcher, screen(1600, 1000), 0.0);
        let windows: Vec<_> = shown.iter().map(|(window, _, _)| *window).collect();
        assert_eq!(windows, [2, 3, 4, 5, 6, 7]);
        // On a small screen all of it fits inside the margin.
        let small = screen(500, 400);
        let layout = switcher.queue(small, QUEUE, (0.0, 1.0, 0.0, 0.0));
        let front = layout.items[0].place;
        assert!(front.size.width < 400, "{front:?}");
        for item in &layout.items {
            assert!(item.place.origin.x >= 1000 + 20 && item.place.right() <= 1000 + 480);
            assert!(item.place.origin.y >= 20, "{item:?}");
        }
        assert!(layout.area.bottom() <= 380, "{:?}", layout.area);
    }

    #[test]
    fn a_turn_begins_where_the_queue_stood_and_sends_the_one_in_front_to_the_back() {
        let mut switcher = Switcher::open(ids(&[1, 2, 3]), false).unwrap();
        let screen = screen(1600, 1000);
        let before = queued(&switcher, screen, 0.0);
        switcher.next();
        assert_eq!(queued(&switcher, screen, -1.0), before, "nothing has moved");
        // A quarter of the way, the one that was in front is on its way out,
        // larger, lower and fading; three quarters, into the back.
        let leaving = queued(&switcher, screen, -0.75);
        let (window, place, opacity) = leaving[0];
        assert_eq!(window, 2);
        assert!(place.size.width > 400 && place.bottom() > before[0].1.bottom());
        assert_eq!(opacity, 500);
        let arriving = queued(&switcher, screen, -0.25);
        let (window, place, opacity) = arriving[2];
        assert_eq!(window, 2);
        assert!(place.size.width < 289 && place.origin.x > before[2].1.origin.x);
        assert!(0 < opacity && opacity < 500, "{opacity}");
        // Once it has turned, the next stands in front.
        let after = queued(&switcher, screen, 0.0);
        let windows: Vec<_> = after.iter().map(|(window, _, _)| *window).collect();
        assert_eq!(windows, [3, 1, 2]);
        // Going back turns it the other way, from the back to the front.
        switcher.previous();
        assert_eq!(queued(&switcher, screen, 1.0), after);
    }

    #[test]
    fn the_last_shown_fades_as_it_goes_behind_the_rest_and_one_window_stays_still() {
        let mut switcher = Switcher::open(ids(&[1, 2, 3, 4, 5, 6, 7, 8]), false).unwrap();
        switcher.next();
        let screen = screen(1600, 1000);
        let turning = queued(&switcher, screen, -0.5);
        let (window, _, opacity) = *turning.last().unwrap();
        assert_eq!(window, 8, "the next to come into sight");
        assert!(0 < opacity && opacity < 400, "{opacity}");
        let alone = Switcher::open(ids(&[7]), false).unwrap();
        assert_eq!(queued(&alone, screen, -0.5), queued(&alone, screen, 0.0));
    }

    #[test]
    fn the_program_picked_s_windows_are_gone_through_and_the_one_gone_to_is_chosen() {
        // Programs, each by its window used last: 1, then 2 picked.
        let mut switcher = Switcher::open(ids(&[1, 2, 3]), false).unwrap();
        assert!(!switcher.go_within(false), "none shown yet");
        // Its windows, in the order of use, as they come: 2 stands for it.
        switcher.show_program(ids(&[5, 2, 6]));
        assert_eq!(switcher.chosen().raw(), 2);
        assert_eq!(switcher.program(), ids(&[2, 6, 5]));
        assert!(!switcher.gone_within());
        assert!(switcher.go_within(false));
        assert_eq!(switcher.chosen().raw(), 6, "the one after it");
        assert!(switcher.gone_within());
        switcher.go_within(false);
        assert_eq!(switcher.chosen().raw(), 5, "round");
        switcher.go_within(true);
        switcher.go_within(true);
        assert_eq!(switcher.chosen().raw(), 2, "and back");
        assert_eq!(picked(&switcher), 2, "the program picked is as it was");

        // Gone from among them, they are taken as they stand.
        switcher.go_within(false);
        let switcher = switcher.without(WindowId::from_raw(5)).unwrap();
        assert_eq!(switcher.chosen().raw(), 6);
        let mut switcher = switcher.without(WindowId::from_raw(6)).unwrap();
        assert_eq!(
            switcher.chosen().raw(),
            2,
            "one left, nothing to go through"
        );
        assert!(!switcher.go_within(false));

        // Another program picked, or one of one window, or one not among
        // them: nothing to go through.
        for go in [Switcher::next, Switcher::previous] {
            switcher.show_program(ids(&[2, 5]));
            go(&mut switcher);
            assert!(!switcher.go_within(false));
        }
        switcher.show_program(ids(&[2, 5]));
        switcher.pick(WindowId::from_raw(3));
        assert!(!switcher.go_within(false));
        switcher.pick(WindowId::from_raw(9));
        switcher.show_program(ids(&[3]));
        assert!(!switcher.go_within(false));
        switcher.show_program(ids(&[8, 9]));
        assert_eq!(switcher.chosen().raw(), 3);
        assert!(!switcher.go_within(false));
    }

    #[test]
    fn the_window_gone_to_within_a_program_is_kept_while_another_is_picked() {
        let mut switcher = Switcher::open(ids(&[1, 2, 3]), false).unwrap();
        switcher.show_program(ids(&[2, 5, 6]));
        switcher.go_within(false);
        assert_eq!(switcher.chosen().raw(), 5);
        // Another picked, and back: at the one gone to, as before.
        switcher.next();
        switcher.show_program(ids(&[3]));
        assert_eq!(switcher.chosen().raw(), 3);
        switcher.previous();
        switcher.show_program(ids(&[2, 5, 6]));
        assert_eq!(switcher.chosen().raw(), 5);
        assert!(switcher.gone_within());
        // Gone on from there, and that kept in its stead.
        switcher.go_within(false);
        switcher.next();
        switcher.previous();
        switcher.show_program(ids(&[2, 5, 6]));
        assert_eq!(switcher.chosen().raw(), 6);
        // Picked another, the place of the one gone through shows the one
        // gone to within it.
        switcher.next();
        let layout = switcher.layout(screen(1600, 1000), METRICS);
        let shown: Vec<u64> = layout.items.iter().map(|item| item.window.raw()).collect();
        assert_eq!(shown, [1, 6, 3]);
        switcher.previous();
        switcher.show_program(ids(&[2, 5, 6]));
        // One gone since, the program at the one that stands for it.
        switcher.next();
        switcher.previous();
        switcher.show_program(ids(&[2, 5]));
        assert_eq!(switcher.chosen().raw(), 2);
        assert!(!switcher.gone_within());
    }

    #[test]
    fn in_rows_the_program_s_other_windows_are_stacked_behind_it_each_smaller() {
        let mut switcher = Switcher::open(ids(&[1, 2]), false).unwrap();
        let before = switcher.layout(screen(1600, 1000), METRICS);
        assert_eq!(before.program, []);
        switcher.show_program(ids(&[2, 5, 6]));
        switcher.go_within(false);
        let layout = switcher.layout(screen(1600, 1000), METRICS);
        let front = layout.items[1];
        assert_eq!((front.window.raw(), front.picked), (5, true));
        let stacked: Vec<u64> = layout
            .program
            .iter()
            .map(|item| item.window.raw())
            .collect();
        assert_eq!(stacked, [6, 2], "the next first");
        let (first, second) = (layout.program[0], layout.program[1]);
        assert!(first.place.size.width < front.place.size.width);
        assert!(second.place.size.width < first.place.size.width);
        assert!(
            first.place.origin.y < front.place.origin.y,
            "a little higher"
        );
        assert!(second.place.origin.y < first.place.origin.y);
        assert!(second.opacity < first.opacity && first.opacity < 1000);
    }

    #[test]
    fn in_the_queue_the_program_s_windows_stand_in_a_row_raised_as_gone_through() {
        let screen = screen(1600, 1000);
        let mut switcher = Switcher::open(ids(&[1, 2]), false).unwrap();
        switcher.show_program(ids(&[2, 5, 6]));
        let row = |raised| {
            switcher
                .queue(screen, QUEUE, (0.0, 1.0, raised, 0.0))
                .program
        };
        let low = row(0.0);
        let windows: Vec<u64> = low.iter().map(|item| item.window.raw()).collect();
        assert_eq!(windows, [2, 5, 6], "all of them, as they were used");
        let picked: Vec<bool> = low.iter().map(|item| item.picked).collect();
        assert_eq!(picked, [true, false, false]);
        // A quarter of each seen above the bottom edge, side by side under
        // the middle of the one in front.
        let first = low[0].place;
        let bottom = i64::from(screen.size.height);
        assert_eq!(
            bottom - i64::from(first.origin.y),
            i64::from(first.size.height) / 4
        );
        let next = low[1].place;
        assert_eq!(next.origin.y, first.origin.y, "beside it");
        assert!(i64::from(next.origin.x) > first.right(), "to its right");
        let front = switcher.queue(screen, QUEUE, (0.0, 1.0, 0.0, 0.0)).items[0].place;
        let middle = |place: Rect| i64::from(place.origin.x) * 2 + i64::from(place.size.width);
        assert!(
            (middle(next) - middle(front)).abs() <= 2,
            "the middle one under it"
        );
        // Raised, just under the box.
        let layout = switcher.queue(screen, QUEUE, (0.0, 1.0, 1.0, 0.0));
        let up = layout.program[0].place;
        let under = layout.area.origin.y + (QUEUE.title + QUEUE.gap).cast_signed();
        assert_eq!(up.origin.y, under);
        // On a screen as tall as most, the queue goes up as it rises, and
        // the row stands clear of the one in front, under the box.
        let wide = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
        let large = QueueMetrics {
            widest: 720,
            ..QUEUE
        };
        // Six programs, as tall as the queue stands.
        let mut tall = Switcher::open(ids(&[1, 2, 3, 4, 7, 8]), false).unwrap();
        tall.show_program(ids(&[2, 5, 6]));
        let lowered = tall.queue(wide, large, (0.0, 1.0, 0.0, 0.0));
        let raised = tall.queue(wide, large, (0.0, 1.0, 1.0, 0.0));
        assert!(raised.items[0].place.origin.y < lowered.items[0].place.origin.y);
        let front = raised.items[0].place;
        let first = raised.program[0].place;
        assert!(i64::from(first.origin.y) > front.bottom());
        assert!(first.bottom() <= 1080 - i64::from(QUEUE.margin));
        // Or, on a screen with no room under it, whole above the margin.
        let short = Rect::new(Point::new(0, 0), Size::new(640, 480));
        let raised = switcher.queue(short, QUEUE, (0.0, 1.0, 1.0, 0.0)).program[0].place;
        let foot = i64::from(raised.origin.y) + i64::from(raised.size.height);
        assert_eq!(foot, 480 - i64::from(QUEUE.margin));
        let front = f64::from(layout.items[0].place.size.width);
        assert_eq!(
            f64::from(up.size.width),
            (front * COLUMN).round(),
            "a third as wide"
        );
        let half = row(0.5)[0].place.origin.y;
        // Not yet come up, wholly below the bottom edge; half way, an
        // eighth seen.
        let hidden = switcher.queue(screen, QUEUE, (0.0, 0.0, 0.0, 0.0)).program[0].place;
        assert_eq!(i64::from(hidden.origin.y), bottom);
        let coming = switcher.queue(screen, QUEUE, (0.0, 0.5, 0.0, 0.0)).program[0].place;
        assert!(hidden.origin.y > coming.origin.y && coming.origin.y > first.origin.y);
        assert!(up.origin.y < half && half < first.origin.y);
    }

    #[test]
    fn the_ring_is_round_the_one_gone_to_the_others_fainter_and_a_row_too_long_slides_to_it() {
        let screen = screen(1600, 1000);
        let mut switcher = Switcher::open(ids(&[1, 2]), false).unwrap();
        switcher.show_program(ids(&[2, 5, 6]));
        let row =
            |switcher: &Switcher, sliding| switcher.queue(screen, QUEUE, (0.0, 1.0, 0.0, sliding));
        // Round the one gone to, in its place in the row.
        let layout = row(&switcher, 0.0);
        assert_eq!(layout.focus, Some(layout.program[0].place));
        switcher.go_within(false);
        let layout = row(&switcher, 0.0);
        assert_eq!(layout.focus, Some(layout.program[1].place));
        assert!(layout.program[1].picked);
        // While it was on its way there, the row, which fits, standing
        // still, and the ring round it all the same.
        let half = row(&switcher, 0.5);
        assert_eq!(half.program, layout.program);
        assert_eq!(half.focus, layout.focus);
        let opacities: Vec<u16> = layout.program.iter().map(|item| item.opacity).collect();
        assert_eq!(opacities, [600, 1000, 600], "the others fainter");

        // More than fit: as many as fit shown, the row slid to keep the one
        // gone to in sight, the ends fainter.
        let narrow = Rect::new(Point::new(0, 0), Size::new(640, 480));
        let mut long = Switcher::open(ids(&[1, 2]), false).unwrap();
        long.show_program(ids(&[2, 10, 11, 12, 13, 14, 15]));
        for _ in 0..4 {
            long.go_within(false);
        }
        let layout = long.queue(narrow, QUEUE, (0.0, 1.0, 0.0, 0.0));
        let gone = layout.program.iter().find(|item| item.picked).unwrap();
        assert_eq!(gone.window.raw(), 13);
        assert_eq!(layout.focus, Some(gone.place));
        assert!(i64::from(gone.place.origin.x) >= i64::from(QUEUE.margin));
        assert!(gone.place.right() <= 640 - i64::from(QUEUE.margin));
        assert!(layout.program.len() < 7, "not all of them");
        let ends = [
            layout.program[0].opacity,
            layout.program[layout.program.len() - 1].opacity,
        ];
        assert!(ends.iter().all(|opacity| *opacity < 1000), "{ends:?}");
        assert_eq!(gone.opacity, 1000);
        assert!(ends.iter().all(|opacity| *opacity < 600), "fainter still");
        // Slid there from the one before.
        let slid = long.queue(narrow, QUEUE, (0.0, 1.0, 0.0, 1.0));
        assert_ne!(slid.program, layout.program);
    }
}
