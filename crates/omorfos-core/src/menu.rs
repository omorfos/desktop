//! Pop-up menus: where their panels go, and what the pointer does to them.
//!
//! A menu is a column of rows. A row that opens a submenu shows it in a
//! further panel beside itself while the pointer is on it. This module
//! decides layout and behaviour; the compositor draws the panels and feeds
//! in pointer events.

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

    /// The row the pointer is on, if it is one that can be chosen.
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

    #[test]
    fn an_empty_menu_is_just_its_padding() {
        let menu: Menu<&str> = Menu::open(Vec::new(), Point::new(10, 10), AREA, METRICS);
        assert_eq!(menu.panel_rect(&menu.panels()[0]), rect(10, 10, 200, 8));
        assert_eq!(menu.rows(&menu.panels()[0]), []);
    }
}
