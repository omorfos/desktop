//! The switcher on `Alt+Tab`: the windows in the order they were last
//! used, walked through while a key is held and chosen when it is let go.
//!
//! [`Used`] keeps the order of use from one opening of the switcher to the
//! next. [`Switcher`] is the switcher while it is open: the windows as
//! they stood when it opened, which of them is picked, and where each is
//! shown. The order stands still while it is open, so that a window passed
//! over on the way has not been used; the one chosen goes to the front
//! once the switcher has closed and it has focus.

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

/// Where the switcher and each window in it are shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitcherLayout {
    /// The switcher itself.
    pub area: Rect,
    /// Each window shown, where, and whether it is the one picked. Only
    /// those that fit are shown, and the one picked always is.
    pub items: Vec<(WindowId, Rect, bool)>,
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
            (*window, Rect::new(Point::new(x, y), item), at == self.at)
        });
        SwitcherLayout {
            area,
            items: items.collect(),
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
        let expected = [(1, 0, false), (2, 1, true), (3, 2, false)].map(|(id, column, picked)| {
            (
                WindowId::from_raw(id),
                Rect::new(Point::new(x(column), 255 + 5), METRICS.item),
                picked,
            )
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
                .map(|(id, _, _)| id.raw())
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
        let picked = layout(&switcher)
            .items
            .into_iter()
            .find(|(_, _, picked)| *picked);
        assert_eq!(
            picked.map(|(id, at, _)| (id.raw(), at.origin.y)),
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
            .map(|(id, _, picked)| (id.raw(), *picked))
            .collect();
        assert_eq!(items, [(2, true)]);
        assert_eq!(
            layout.area.origin,
            Point::new(1000, 0),
            "from the corner, as it does not fit"
        );
    }
}
