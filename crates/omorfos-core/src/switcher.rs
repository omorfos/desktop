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
    /// always is.
    pub items: Vec<SwitcherItem>,
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

/// The switcher, while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switcher {
    windows: Vec<WindowId>,
    at: usize,
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
        (count > 0).then_some(Self { windows, at })
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
    }

    /// Picks the window before, going round from the first to the last.
    pub fn previous(&mut self) {
        self.at = (self.at + self.windows.len() - 1) % self.windows.len();
    }

    /// Picks `window`, if it is in the switcher.
    pub fn pick(&mut self, window: WindowId) {
        if let Some(at) = self.windows.iter().position(|other| *other == window) {
            self.at = at;
        }
    }

    /// Takes away a window that has gone; the one after it is picked if it
    /// was. With none left the switcher is gone too.
    #[must_use]
    pub fn without(mut self, window: WindowId) -> Option<Self> {
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
            SwitcherItem {
                window: *window,
                place: Rect::new(Point::new(x, y), item),
                picked: at == self.at,
                opacity: 1000,
            }
        });
        SwitcherLayout {
            area,
            items: items.collect(),
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
    /// back.
    #[must_use]
    pub fn queue(&self, screen: Rect, metrics: QueueMetrics, turning: f64) -> SwitcherLayout {
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
                let card = SwitcherItem {
                    window: *window,
                    place,
                    picked: at == self.at,
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
        SwitcherLayout {
            area,
            items: cards.into_iter().map(|(_, card)| card).collect(),
        }
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
        let layout = switcher.queue(screen, QUEUE, turning);
        let items = layout.items.iter();
        items
            .map(|item| (item.window.raw(), item.place, item.opacity))
            .collect()
    }

    #[test]
    fn the_queue_stands_the_one_picked_in_front_and_the_rest_behind_it_going_round() {
        let switcher = Switcher::open(ids(&[3, 2, 1]), false).unwrap();
        let layout = switcher.queue(screen(1600, 1000), QUEUE, 0.0);
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
        let layout = switcher.queue(small, QUEUE, 0.0);
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
}
