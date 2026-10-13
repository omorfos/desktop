//! Pop-up menus: where their panels go, and what the pointer and the
//! keyboard do to them.
//!
//! A menu is a column of rows. A row that opens a submenu shows it in a
//! further panel beside itself while the pointer is on it, or when the
//! keyboard goes into it. This module decides layout and behaviour; the
//! compositor draws the panels and feeds in pointer events and keys.

use crate::geometry::{Point, Rect, Size};

/// One row of a menu. `A` is whatever an entry does when chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item<A> {
    /// Does something when chosen.
    Entry {
        /// The text shown.
        label: String,
        /// What choosing the entry does.
        action: A,
    },
    /// Opens a further panel.
    Submenu {
        /// The text shown.
        label: String,
        /// The rows of the further panel.
        items: Vec<Self>,
    },
    /// A dividing line. Cannot be chosen.
    Separator,
}

/// The sizes a menu is laid out with, in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MenuMetrics {
    /// The width of every panel.
    pub width: u32,
    /// The height of an entry or submenu row.
    pub row: u32,
    /// The height of a separator row.
    pub separator: u32,
    /// The space above the first row and below the last.
    pub padding: u32,
}

/// One column of an open menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Panel<A> {
    origin: Point,
    items: Vec<Item<A>>,
    highlighted: Option<usize>,
}

impl<A> Panel<A> {
    /// The rows, top to bottom.
    #[must_use]
    pub fn items(&self) -> &[Item<A>] {
        &self.items
    }

    /// The row the pointer is on or the keyboard went to, if it is one
    /// that can be chosen.
    #[must_use]
    pub const fn highlighted(&self) -> Option<usize> {
        self.highlighted
    }
}

/// What a press on an open menu means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuOutcome<A> {
    /// An entry was chosen: close the menu and do this.
    Activate(A),
    /// The press was outside the menu: close it.
    Dismiss,
    /// The press was on the menu but chose nothing: keep it open.
    Nothing,
}

/// A key that means something to an open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MenuKey {
    /// To the row above, and from the first to the last.
    Up,
    /// To the row below, and from the last to the first.
    Down,
    /// Out of a submenu, back to the row it belongs to.
    Left,
    /// Into the submenu of the highlighted row.
    Right,
    /// Chooses the highlighted row.
    Enter,
}

/// An open menu: a root panel and the submenu panels leading to wherever
/// the pointer is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu<A> {
    metrics: MenuMetrics,
    area: Rect,
    /// Never empty. Each panel after the first belongs to the highlighted
    /// row of the one before it.
    panels: Vec<Panel<A>>,
}

impl<A: Clone> Menu<A> {
    /// Opens a menu with its top-left corner at `at`, moved as needed to
    /// stay within `area`.
    #[must_use]
    pub fn open(items: Vec<Item<A>>, at: Point, area: Rect, metrics: MenuMetrics) -> Self {
        let mut menu = Self {
            metrics,
            area,
            panels: Vec::new(),
        };
        menu.push(items, at);
        menu
    }

    /// The panels, from the root outwards.
    #[must_use]
    pub fn panels(&self) -> &[Panel<A>] {
        &self.panels
    }

    /// The rectangle a panel occupies.
    #[must_use]
    pub fn panel_rect(&self, panel: &Panel<A>) -> Rect {
        Rect::new(panel.origin, self.panel_size(&panel.items))
    }

    /// The rectangle of each of a panel's rows, top to bottom.
    #[must_use]
    pub fn rows(&self, panel: &Panel<A>) -> Vec<Rect> {
        let mut top = panel.origin.offset(0, self.metrics.padding.cast_signed());
        panel
            .items
            .iter()
            .map(|item| {
                let height = self.row_height(item);
                let row = Rect::new(top, Size::new(self.metrics.width, height));
                top = top.offset(0, height.cast_signed());
                row
            })
            .collect()
    }

    /// Reports the pointer moving to `point`.
    ///
    /// The row under it is highlighted, and if that row has a submenu its
    /// panel opens. Panels that no longer lead to the pointer close. Moving
    /// off the menu altogether changes nothing, so a submenu stays open
    /// while the pointer crosses the gap to it.
    pub fn hover(&mut self, point: Point) {
        let Some((panel, row)) = self.locate(point) else {
            return;
        };
        self.panels.truncate(panel + 1);
        self.panels[panel].highlighted = row;
        let submenu = row.and_then(|row| match &self.panels[panel].items[row] {
            Item::Submenu { items, .. } => Some((row, items.clone())),
            _ => None,
        });
        if let Some((row, items)) = submenu {
            self.unfold(panel, row, items);
        }
    }

    /// Reports a key being pressed.
    ///
    /// The keys act on the outermost panel: the root, or the submenu the
    /// pointer or the keyboard last opened. Up and down go round its rows,
    /// passing over separators. Right goes into the submenu of the
    /// highlighted row and left comes back out. Enter chooses the row: an
    /// entry is activated, a submenu is gone into.
    #[must_use]
    pub fn key(&mut self, key: MenuKey) -> MenuOutcome<A> {
        let last = self.panels.len().saturating_sub(1);
        let highlighted = self.panels[last].highlighted;
        match key {
            MenuKey::Up | MenuKey::Down => {
                let down = key == MenuKey::Down;
                let panel = &mut self.panels[last];
                let count = panel.items.len();
                // Where to look first: the row after or before the
                // highlighted one, or the first or last row of all.
                let from = match (highlighted, down) {
                    (Some(row), true) => row + 1,
                    (Some(row), false) => row + count - 1,
                    (None, true) => 0,
                    (None, false) => count.saturating_sub(1),
                };
                // Going all the way round comes back to the highlighted
                // row, so a row stays highlighted once one is.
                panel.highlighted = (0..count)
                    .map(|step| {
                        if down {
                            (from + step) % count
                        } else {
                            (from + count - step) % count
                        }
                    })
                    .find(|row| !matches!(panel.items[*row], Item::Separator));
            }
            MenuKey::Left => {
                if last > 0 {
                    self.panels.pop();
                }
            }
            MenuKey::Right | MenuKey::Enter => {
                let chosen = highlighted.map(|row| (row, self.panels[last].items[row].clone()));
                match chosen {
                    Some((_, Item::Entry { action, .. })) if key == MenuKey::Enter => {
                        return MenuOutcome::Activate(action);
                    }
                    Some((row, Item::Submenu { items, .. })) => {
                        self.unfold(last, row, items);
                        return self.key(MenuKey::Down);
                    }
                    _ => {}
                }
            }
        }
        MenuOutcome::Nothing
    }

    /// Opens the panel of `items` beside the row of a panel they belong to.
    fn unfold(&mut self, panel: usize, row: usize, items: Vec<Item<A>>) {
        let beside = self.rows(&self.panels[panel])[row];
        let width = self.metrics.width.cast_signed();
        let padding = self.metrics.padding.cast_signed();
        // Open to the right, or to the left if that would leave the area.
        let fits = beside.right() + i64::from(width) <= self.area.right();
        let x = if fits {
            beside.origin.offset(width, 0).x
        } else {
            beside.origin.offset(-width, 0).x
        };
        self.push(items, Point::new(x, beside.origin.offset(0, -padding).y));
    }

    /// Reports a button press at `point`.
    #[must_use]
    pub fn press(&mut self, point: Point) -> MenuOutcome<A> {
        self.hover(point);
        match self.locate(point) {
            None => MenuOutcome::Dismiss,
            Some((panel, Some(row))) => match &self.panels[panel].items[row] {
                Item::Entry { action, .. } => MenuOutcome::Activate(action.clone()),
                _ => MenuOutcome::Nothing,
            },
            Some((_, None)) => MenuOutcome::Nothing,
        }
    }

    /// The outermost panel under `point` and, if the point is on a row that
    /// can be chosen, that row.
    fn locate(&self, point: Point) -> Option<(usize, Option<usize>)> {
        let index = self
            .panels
            .iter()
            .rposition(|panel| self.panel_rect(panel).contains(point))?;
        let panel = &self.panels[index];
        let row = self
            .rows(panel)
            .iter()
            .position(|row| row.contains(point))
            .filter(|row| !matches!(panel.items[*row], Item::Separator));
        Some((index, row))
    }

    fn push(&mut self, items: Vec<Item<A>>, at: Point) {
        let size = self.panel_size(&items);
        let fit = |wanted: i32, start: i32, end: i64, length: u32| {
            let last = (end - i64::from(length)).max(i64::from(start));
            i32::try_from(i64::from(wanted).clamp(i64::from(start), last)).unwrap_or(i32::MAX)
        };
        let origin = Point::new(
            fit(at.x, self.area.origin.x, self.area.right(), size.width),
            fit(at.y, self.area.origin.y, self.area.bottom(), size.height),
        );
        self.panels.push(Panel {
            origin,
            items,
            highlighted: None,
        });
    }

    fn panel_size(&self, items: &[Item<A>]) -> Size {
        let rows: u32 = items.iter().map(|item| self.row_height(item)).sum();
        Size::new(
            self.metrics.width,
            rows.saturating_add(self.metrics.padding * 2),
        )
    }

    fn row_height(&self, item: &Item<A>) -> u32 {
        match item {
            Item::Separator => self.metrics.separator,
            _ => self.metrics.row,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRICS: MenuMetrics = MenuMetrics {
        width: 200,
        row: 20,
        separator: 10,
        padding: 4,
    };
    const AREA: Rect = Rect::new(Point::new(0, 0), Size::new(1000, 800));

    fn entry(label: &str) -> Item<&'static str> {
        Item::Entry {
            label: label.to_owned(),
            action: Box::leak(label.to_owned().into_boxed_str()),
        }
    }

    /// Terminal / ---- / Session > (Reload, Quit) / About
    fn items() -> Vec<Item<&'static str>> {
        vec![
            entry("Terminal"),
            Item::Separator,
            Item::Submenu {
                label: "Session".to_owned(),
                items: vec![entry("Reload"), entry("Quit")],
            },
            entry("About"),
        ]
    }

    fn open_at(x: i32, y: i32) -> Menu<&'static str> {
        Menu::open(items(), Point::new(x, y), AREA, METRICS)
    }

    fn rect(x: i32, y: i32, width: u32, height: u32) -> Rect {
        Rect::new(Point::new(x, y), Size::new(width, height))
    }

    #[test]
    fn a_menu_opens_as_one_panel_at_the_given_point() {
        let menu = open_at(100, 100);
        assert_eq!(menu.panels().len(), 1);
        let panel = &menu.panels()[0];
        assert_eq!(panel.items(), items());
        assert_eq!(panel.highlighted(), None);
        // Three rows of 20, one separator of 10, and 4 of padding each end.
        assert_eq!(menu.panel_rect(panel), rect(100, 100, 200, 78));
    }

    #[test]
    fn rows_are_stacked_inside_the_padding_and_separators_are_shorter() {
        let menu = open_at(100, 100);
        assert_eq!(
            menu.rows(&menu.panels()[0]),
            [
                rect(100, 104, 200, 20),
                rect(100, 124, 200, 10),
                rect(100, 134, 200, 20),
                rect(100, 154, 200, 20)
            ]
        );
    }

    #[test]
    fn a_menu_opened_near_an_edge_is_moved_to_stay_on_screen() {
        let menu = open_at(950, 790);
        assert_eq!(menu.panel_rect(&menu.panels()[0]), rect(800, 722, 200, 78));
        let menu = open_at(-50, -50);
        assert_eq!(menu.panel_rect(&menu.panels()[0]).origin, Point::new(0, 0));
    }

    #[test]
    fn a_menu_taller_than_the_area_starts_at_its_top() {
        let short = Rect::new(Point::new(0, 0), Size::new(1000, 50));
        let menu = Menu::open(items(), Point::new(10, 40), short, METRICS);
        assert_eq!(menu.panel_rect(&menu.panels()[0]).origin, Point::new(10, 0));
    }

    #[test]
    fn the_row_under_the_pointer_is_highlighted() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(150, 110));
        assert_eq!(menu.panels()[0].highlighted(), Some(0));
        menu.hover(Point::new(150, 160));
        assert_eq!(menu.panels()[0].highlighted(), Some(3));
    }

    #[test]
    fn separators_and_padding_are_not_highlighted() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(150, 110));
        menu.hover(Point::new(150, 128));
        assert_eq!(menu.panels()[0].highlighted(), None, "on the separator");
        menu.hover(Point::new(150, 110));
        menu.hover(Point::new(150, 101));
        assert_eq!(menu.panels()[0].highlighted(), None, "in the padding");
    }

    #[test]
    fn hovering_a_submenu_row_opens_its_panel_beside_it() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(150, 140));
        assert_eq!(menu.panels().len(), 2);
        let submenu = &menu.panels()[1];
        // To the right of the row, with its first row level with it.
        assert_eq!(menu.panel_rect(submenu), rect(300, 130, 200, 48));
        assert_eq!(submenu.items().len(), 2);
    }

    #[test]
    fn a_submenu_opens_to_the_left_when_there_is_no_room_on_the_right() {
        let mut menu = open_at(790, 100);
        menu.hover(Point::new(850, 140));
        assert_eq!(
            menu.panel_rect(&menu.panels()[1]).origin,
            Point::new(590, 130)
        );
    }

    #[test]
    fn a_submenu_stays_open_while_the_pointer_is_on_it_or_off_the_menu() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(150, 140));
        menu.hover(Point::new(350, 140));
        assert_eq!(menu.panels().len(), 2);
        assert_eq!(menu.panels()[1].highlighted(), Some(0));
        assert_eq!(
            menu.panels()[0].highlighted(),
            Some(2),
            "the row it belongs to stays lit"
        );

        menu.hover(Point::new(700, 700));
        assert_eq!(
            menu.panels().len(),
            2,
            "moving off the menu changes nothing"
        );
    }

    #[test]
    fn a_submenu_closes_when_the_pointer_moves_to_another_row_of_its_parent() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(150, 140));
        menu.hover(Point::new(150, 160));
        assert_eq!(menu.panels().len(), 1);
        assert_eq!(menu.panels()[0].highlighted(), Some(3));
    }

    #[test]
    fn pressing_an_entry_activates_it() {
        let mut menu = open_at(100, 100);
        assert_eq!(
            menu.press(Point::new(150, 110)),
            MenuOutcome::Activate("Terminal")
        );
    }

    #[test]
    fn pressing_an_entry_in_a_submenu_activates_it() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(150, 140));
        assert_eq!(
            menu.press(Point::new(350, 160)),
            MenuOutcome::Activate("Quit")
        );
    }

    #[test]
    fn pressing_a_submenu_row_opens_it_and_keeps_the_menu() {
        let mut menu = open_at(100, 100);
        assert_eq!(menu.press(Point::new(150, 140)), MenuOutcome::Nothing);
        assert_eq!(menu.panels().len(), 2);
    }

    #[test]
    fn pressing_a_separator_or_the_padding_does_nothing() {
        let mut menu = open_at(100, 100);
        assert_eq!(menu.press(Point::new(150, 128)), MenuOutcome::Nothing);
        assert_eq!(menu.press(Point::new(150, 101)), MenuOutcome::Nothing);
    }

    #[test]
    fn pressing_outside_the_menu_dismisses_it() {
        let mut menu = open_at(100, 100);
        assert_eq!(menu.press(Point::new(50, 50)), MenuOutcome::Dismiss);
        assert_eq!(
            menu.press(Point::new(350, 140)),
            MenuOutcome::Dismiss,
            "no submenu is open there"
        );
    }

    fn highlights(menu: &Menu<&'static str>) -> Vec<Option<usize>> {
        menu.panels().iter().map(Panel::highlighted).collect()
    }

    fn keys(menu: &mut Menu<&'static str>, keys: &[MenuKey]) -> MenuOutcome<&'static str> {
        keys.iter()
            .map(|key| menu.key(*key))
            .last()
            .unwrap_or(MenuOutcome::Nothing)
    }

    #[test]
    fn down_goes_to_the_first_row_then_on_past_separators_and_round() {
        let mut menu = open_at(100, 100);
        assert_eq!(menu.key(MenuKey::Down), MenuOutcome::Nothing);
        assert_eq!(highlights(&menu), [Some(0)]);
        let _ = menu.key(MenuKey::Down);
        assert_eq!(highlights(&menu), [Some(2)]);
        let _ = keys(&mut menu, &[MenuKey::Down, MenuKey::Down]);
        assert_eq!(highlights(&menu), [Some(0)]);
    }

    #[test]
    fn up_goes_to_the_last_row_then_back_past_separators_and_round() {
        let mut menu = open_at(100, 100);
        let _ = menu.key(MenuKey::Up);
        assert_eq!(highlights(&menu), [Some(3)]);
        let _ = keys(&mut menu, &[MenuKey::Up, MenuKey::Up]);
        assert_eq!(highlights(&menu), [Some(0)]);
        let _ = menu.key(MenuKey::Up);
        assert_eq!(highlights(&menu), [Some(3)]);
    }

    #[test]
    fn the_keys_carry_on_from_the_row_the_pointer_is_on() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(110, 160));
        assert_eq!(highlights(&menu), [Some(3)]);
        let _ = menu.key(MenuKey::Up);
        assert_eq!(highlights(&menu), [Some(2)]);
    }

    #[test]
    fn enter_activates_the_highlighted_entry() {
        let mut menu = open_at(100, 100);
        assert_eq!(menu.key(MenuKey::Enter), MenuOutcome::Nothing);
        assert_eq!(
            keys(&mut menu, &[MenuKey::Down, MenuKey::Enter]),
            MenuOutcome::Activate("Terminal")
        );
    }

    #[test]
    fn right_does_not_activate_an_entry() {
        let mut menu = open_at(100, 100);
        assert_eq!(
            keys(&mut menu, &[MenuKey::Down, MenuKey::Right]),
            MenuOutcome::Nothing
        );
        assert_eq!(highlights(&menu), [Some(0)]);
    }

    #[test]
    fn right_and_enter_go_into_a_submenu_at_its_first_row() {
        for into in [MenuKey::Right, MenuKey::Enter] {
            let mut menu = open_at(100, 100);
            let outcome = keys(&mut menu, &[MenuKey::Down, MenuKey::Down, into]);
            assert_eq!(outcome, MenuOutcome::Nothing);
            assert_eq!(highlights(&menu), [Some(2), Some(0)]);
            assert_eq!(
                menu.panel_rect(&menu.panels()[1]),
                rect(300, 130, 200, 48),
                "where the pointer would have opened it"
            );
            assert_eq!(
                keys(&mut menu, &[MenuKey::Down, MenuKey::Enter]),
                MenuOutcome::Activate("Quit")
            );
        }
    }

    #[test]
    fn up_and_down_stay_in_the_submenu_that_is_open() {
        let mut menu = open_at(100, 100);
        let into = [MenuKey::Up, MenuKey::Up, MenuKey::Right];
        let _ = keys(&mut menu, &into);
        let _ = keys(&mut menu, &[MenuKey::Down, MenuKey::Down, MenuKey::Down]);
        assert_eq!(highlights(&menu), [Some(2), Some(1)]);
    }

    #[test]
    fn left_comes_out_of_a_submenu_to_the_row_it_belongs_to() {
        let mut menu = open_at(100, 100);
        let _ = keys(&mut menu, &[MenuKey::Up, MenuKey::Up, MenuKey::Right]);
        assert_eq!(menu.key(MenuKey::Left), MenuOutcome::Nothing);
        assert_eq!(highlights(&menu), [Some(2)]);
        let _ = menu.key(MenuKey::Left);
        assert_eq!(highlights(&menu), [Some(2)], "the root stays");
    }

    #[test]
    fn the_keys_go_into_a_submenu_the_pointer_opened() {
        let mut menu = open_at(100, 100);
        menu.hover(Point::new(110, 140));
        assert_eq!(highlights(&menu), [Some(2), None]);
        let _ = menu.key(MenuKey::Down);
        assert_eq!(highlights(&menu), [Some(2), Some(0)]);
    }

    #[test]
    fn the_keys_find_nothing_to_highlight_in_a_menu_with_nothing_to_choose() {
        for items in [Vec::new(), vec![Item::Separator]] {
            let mut menu = Menu::<&str>::open(items, Point::new(0, 0), AREA, METRICS);
            let all = [
                MenuKey::Down,
                MenuKey::Up,
                MenuKey::Right,
                MenuKey::Left,
                MenuKey::Enter,
            ];
            assert_eq!(keys(&mut menu, &all), MenuOutcome::Nothing);
            assert_eq!(highlights(&menu), [None]);
        }
    }

    #[test]
    fn an_empty_menu_is_just_its_padding() {
        let menu: Menu<&str> = Menu::open(Vec::new(), Point::new(10, 10), AREA, METRICS);
        assert_eq!(menu.panel_rect(&menu.panels()[0]), rect(10, 10, 200, 8));
        assert_eq!(menu.rows(&menu.panels()[0]), []);
    }
}
