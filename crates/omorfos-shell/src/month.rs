//! A month, as the clock opens it: six weeks from the day the locale
//! begins them on, the days of the month among those of the months beside
//! it, a day of the one before at least, today marked, and arrows to the month before and the month
//! after, a month at a time. Always six weeks, so that it is as large
//! whichever month it shows and the arrows stay where they are. Drawn as a
//! popover's content.
//!
//! Nothing here knows of Wayland.

use jiff::ToSpan;
pub use jiff::civil::{Date, Weekday};
use omorfos_config::{Edges, Part, State, States, Theme};
use omorfos_ui::{Area, Canvas, Scale, Text, inset};

/// How many weeks a month is shown in: as many as any month touches.
const WEEKS: u32 = 6;

/// The territories whose weeks begin on Sunday, by their locales in glibc
/// 2.39 (`first_weekday` in `LC_TIME`), each as most of its locales have it.
const SUNDAY: &[&str] = &[
    "AG", "AR", "BR", "BT", "BW", "CA", "CO", "DO", "ET", "FJ", "GT", "HK", "HN", "ID", "IL", "IN",
    "JP", "KE", "KH", "KR", "LA", "MM", "MT", "MU", "MX", "NI", "NP", "PA", "PE", "PG", "PH", "PK",
    "PR", "PY", "SA", "SG", "SV", "TH", "TN", "TO", "TW", "US", "VE", "VU", "WS", "YE", "ZA", "ZW",
];
/// Those whose weeks begin on Saturday, as above.
const SATURDAY: &[&str] = &[
    "AE", "AF", "BH", "DJ", "DZ", "EG", "IQ", "IR", "JO", "KW", "LY", "MA", "OM", "QA", "SD", "SY",
];
/// Those whose weeks begin on Friday, as above.
const FRIDAY: &[&str] = &["BD", "MV"];

/// The day a week begins on where time is told in the locale `named`, as
/// `en_US.UTF-8`: by its territory, and Monday where it names none.
#[must_use]
pub fn first_weekday(named: &str) -> Weekday {
    let name = named.split(['.', '@']).next().unwrap_or(named);
    let territory = name.split_once('_').map(|(_, territory)| territory);
    match territory {
        Some(territory) if SUNDAY.contains(&territory) => Weekday::Sunday,
        Some(territory) if SATURDAY.contains(&territory) => Weekday::Saturday,
        Some(territory) if FRIDAY.contains(&territory) => Weekday::Friday,
        _ => Weekday::Monday,
    }
}

/// What a month says, in the user's language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    /// The months' names, January first.
    pub months: [String; 12],
    /// The days of the week, short, Monday first, whichever the week
    /// begins on.
    pub weekdays: [String; 7],
}

/// An arrow, to another month.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrow {
    /// To the month before.
    Before,
    /// To the month after.
    After,
}

/// What on a month the pointer or a click can be on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Over {
    /// An arrow.
    Arrow(Arrow),
    /// The day so many days across and weeks down.
    Day(u32, u32),
}

/// A month on show: which, and what today is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Month {
    said: Said,
    today: Date,
    /// The day each week begins on.
    first_weekday: Weekday,
    /// The first day of the month shown.
    shown: Date,
    /// What the pointer is on.
    hovered: Option<Over>,
}

/// Where a month's parts are, under a theme.
struct Layout {
    whole: Area,
    /// How large a day is.
    side: u32,
    /// How far apart days are, across and down: a day and its margin.
    pitch: (u32, u32),
    /// The room kept clear around a day.
    margin: Edges<u32>,
    /// Where the days begin.
    left: i32,
    top: i32,
}

impl Layout {
    fn new(theme: &Theme) -> Self {
        let room = theme.style(Part::Popover, States::NONE).inset();
        let day = theme.style(Part::PopoverDay, States::NONE);
        let side = day.height.max(day.min_width);
        let pitch = (
            side.saturating_add(day.margin.across()),
            side.saturating_add(day.margin.down()),
        );
        // The heading with its arrows, the days of the week, the weeks.
        let width = (7 * pitch.0).saturating_add(room.across());
        let height = ((2 + WEEKS) * pitch.1).saturating_add(room.down());
        Self {
            whole: (0, 0, width, height),
            side,
            pitch,
            margin: day.margin,
            left: room.left.cast_signed(),
            top: room.top.cast_signed(),
        }
    }

    /// The room for what is `across` days in and `down` rows down, the
    /// heading's row being the first: a day and its margin.
    fn slot(&self, across: u32, down: u32) -> Area {
        let x = self.left.saturating_add_unsigned(across * self.pitch.0);
        let y = self.top.saturating_add_unsigned(down * self.pitch.1);
        (x, y, self.pitch.0, self.pitch.1)
    }

    /// Where what is `across` days in and `down` rows down is drawn.
    fn cell(&self, across: u32, down: u32) -> Area {
        let (x, y, ..) = inset(self.slot(across, down), self.margin);
        (x, y, self.side, self.side)
    }

    /// What is at `x`, `y`, if anything is: the margins between days
    /// belong to the days, so that the pointer is never between two.
    fn at(&self, x: i32, y: i32) -> Option<Over> {
        let along = |at: i32, from: i32, pitch: u32, count: u32| {
            u32::try_from(at.saturating_sub(from))
                .ok()
                .and_then(|at| at.checked_div(pitch))
                .filter(|at| *at < count)
        };
        let across = along(x, self.left, self.pitch.0, 7);
        let down = along(y, self.top, self.pitch.1, 2 + WEEKS);
        across.zip(down).and_then(|place| match place {
            (0, 0) => Some(Over::Arrow(Arrow::Before)),
            (6, 0) => Some(Over::Arrow(Arrow::After)),
            (_, 0 | 1) => None,
            (across, down) => Some(Over::Day(across, down - 2)),
        })
    }
}

impl Month {
    /// The month `today` is in, saying `said`, its weeks beginning on
    /// `first_weekday`.
    #[must_use]
    pub fn new(said: Said, today: Date, first_weekday: Weekday) -> Self {
        Self {
            said,
            today,
            first_weekday,
            shown: today.first_of_month(),
            hovered: None,
        }
    }

    /// The first day of the month shown.
    #[must_use]
    pub const fn shown(&self) -> Date {
        self.shown
    }

    /// How large the month is under `theme`, in units of layout: the same
    /// whichever month it shows.
    #[must_use]
    pub fn size(theme: &Theme) -> (u32, u32) {
        let (_, _, width, height) = Layout::new(theme).whole;
        (width, height)
    }

    /// Shows the month before, or `after`, the one shown.
    pub fn turn(&mut self, after: bool) {
        let by = if after { 1 } else { -1 };
        // Within the years jiff counts, which are far beyond anyone's
        // walking.
        self.shown = self.shown.checked_add(by.months()).unwrap_or(self.shown);
    }

    /// What a click at `x`, `y` on the month comes to: says whether it
    /// shows another month.
    pub fn click(&mut self, x: i32, y: i32, theme: &Theme) -> bool {
        let arrow = match Layout::new(theme).at(x, y) {
            Some(Over::Arrow(arrow)) => Some(arrow),
            _ => None,
        };
        arrow.inspect(|arrow| self.turn(*arrow == Arrow::After));
        arrow.is_some()
    }

    /// Takes in where the pointer is on the month, and says whether that
    /// changes how it looks.
    pub fn point(&mut self, x: i32, y: i32, theme: &Theme) -> bool {
        let over = Layout::new(theme).at(x, y);
        let changed = over != self.hovered;
        self.hovered = over;
        changed
    }

    /// The days shown, each with how many days across and weeks down it
    /// is: six weeks, from the first day of the week the month begins in,
    /// or of the week before if it begins on that day.
    fn days(&self) -> impl Iterator<Item = (u32, u32, Date)> {
        // At least a day of the month before, so that the first row always
        // ends it: a week of it before a month that begins on the first day
        // of a week, which leaves February's four weeks between two others.
        let into = self.shown.weekday().since(self.first_weekday);
        let before = i64::from(if into == 0 { 7 } else { into });
        let from = self.shown.checked_sub(before.days()).unwrap_or(self.shown);
        (0..WEEKS * 7)
            .zip(from.series(1.day()))
            .map(|(place, date)| (place % 7, place / 7, date))
    }

    /// The days of the week, short, in the order the weeks are shown in.
    fn weekdays(&self) -> impl Iterator<Item = &String> {
        let monday_to_first = self.first_weekday.to_monday_zero_offset().unsigned_abs();
        let skipped = usize::from(monday_to_first);
        self.said.weekdays.iter().cycle().skip(skipped).take(7)
    }

    /// What a day is drawn as: a part, a day of the month shown or of one
    /// beside it, and its states.
    fn day(&self, across: u32, week: u32, date: Date) -> (Part, States) {
        let part = if date.first_of_month() == self.shown {
            Part::PopoverDay
        } else {
            Part::PopoverDayOutside
        };
        let over = match self.hovered {
            Some(Over::Day(across, week)) => Some((across, week)),
            _ => None,
        };
        let states = States::NONE
            .when(State::Hovered, over == Some((across, week)))
            .when(
                State::Focused,
                over.is_some_and(|over| over.0 == across || over.1 == week),
            )
            .when(State::Active, date == self.today);
        (part, states)
    }

    /// Draws the month at `scale`, under `theme`.
    #[must_use]
    pub fn paint(&self, text: &mut Text, theme: &Theme, scale: Scale) -> Canvas {
        let layout = Layout::new(theme);
        let (_, _, width, height) = layout.whole;
        let mut canvas = Canvas::scaled(width, height, scale);
        canvas.draw_box(layout.whole, &theme.style(Part::Popover, States::NONE));
        let mut write = |canvas: &mut Canvas, words: &str, area: Area, (part, states)| {
            let style = theme.style(part, states);
            canvas.draw_box(area, &style);
            let font = theme.font(part, states);
            text.draw_centered(canvas, words, area, style.color, font);
        };

        // The month and year between the arrows.
        let hovered = |arrow| {
            let states =
                States::NONE.when(State::Hovered, self.hovered == Some(Over::Arrow(arrow)));
            (Part::PopoverItem, states)
        };
        write(&mut canvas, "‹", layout.cell(0, 0), hovered(Arrow::Before));
        write(&mut canvas, "›", layout.cell(6, 0), hovered(Arrow::After));
        let name = &self.said.months[usize::from(self.shown.month().unsigned_abs()) - 1];
        let heading = format!("{name} {}", self.shown.year());
        let (left, top, ..) = layout.cell(1, 0);
        let (right, ..) = layout.cell(5, 0);
        let wide = right.abs_diff(left).saturating_add(layout.side);
        write(
            &mut canvas,
            &heading,
            (left, top, wide, layout.side),
            (Part::PopoverItem, States::NONE),
        );

        for (across, weekday) in (0..).zip(self.weekdays()) {
            let plain = (Part::PopoverItem, States::NONE);
            write(&mut canvas, weekday, layout.cell(across, 1), plain);
        }
        for (across, week, date) in self.days() {
            let drawn = self.day(across, week, date);
            write(
                &mut canvas,
                &date.day().to_string(),
                layout.cell(across, 2 + week),
                drawn,
            );
        }
        canvas
    }
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;
    use omorfos_config::{Accent, Scheme};

    use super::*;

    fn said() -> Said {
        let months = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let weekdays = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
        Said {
            months: months.map(str::to_owned),
            weekdays: weekdays.map(str::to_owned),
        }
    }

    fn theme() -> Theme {
        Theme::built_in(Scheme::Dark, Accent::default())
    }

    /// The middle of `area`.
    fn middle((left, top, width, height): Area) -> (i32, i32) {
        (
            left + (width / 2).cast_signed(),
            top + (height / 2).cast_signed(),
        )
    }

    /// What `color` is on a canvas.
    fn argb(color: omorfos_config::Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    #[test]
    fn a_month_is_six_weeks_from_monday_filled_with_the_days_of_the_months_beside_it() {
        let shown = |today| {
            let month = Month::new(said(), today, Weekday::Monday);
            let days: Vec<(u32, u32, Date, Part)> = month
                .days()
                .map(|(across, week, date)| (across, week, date, month.day(across, week, date).0))
                .collect();
            days
        };
        // February 2027 begins on a Monday and has 28 days: a week of
        // January before it, as every month has a day of the one before,
        // and a week of March after.
        let february = shown(date(2027, 2, 10));
        assert_eq!(february.len(), 42);
        let january = (0, 0, date(2027, 1, 25), Part::PopoverDayOutside);
        assert_eq!(february[0], january);
        assert_eq!(february[7], (0, 1, date(2027, 2, 1), Part::PopoverDay));
        assert_eq!(february[34].2, date(2027, 2, 28));
        let march = (0, 5, date(2027, 3, 1), Part::PopoverDayOutside);
        assert_eq!(february[35], march);
        let last = (6, 5, date(2027, 3, 7), Part::PopoverDayOutside);
        assert_eq!(february[41], last);
        // March 2027 begins on a Monday too, and has 31 days: a week before
        // it, and four days after.
        let march = shown(date(2027, 3, 1));
        assert_eq!(march[7], (0, 1, date(2027, 3, 1), Part::PopoverDay));
        let last = (6, 5, date(2027, 4, 4), Part::PopoverDayOutside);
        assert_eq!(march[41], last);
        // November 2026 began on a Sunday: six days of October before it.
        let november = shown(date(2026, 11, 14));
        assert_eq!(
            november[0],
            (0, 0, date(2026, 10, 26), Part::PopoverDayOutside)
        );
        assert_eq!(november[6], (6, 0, date(2026, 11, 1), Part::PopoverDay));
        assert_eq!(
            november[41],
            (6, 5, date(2026, 12, 6), Part::PopoverDayOutside)
        );
    }

    #[test]
    fn the_arrows_turn_a_month_at_a_time_across_the_years_and_stay_where_they_are() {
        let theme = theme();
        let size = Month::size(&theme);
        let mut month = Month::new(said(), date(2026, 12, 25), Weekday::Monday);
        assert_eq!(month.shown(), date(2026, 12, 1));
        let layout = Layout::new(&theme);
        let (x, y) = middle(layout.cell(6, 0));
        assert!(month.click(x, y, &theme));
        assert_eq!(month.shown(), date(2027, 1, 1));
        let (x, y) = middle(layout.cell(0, 0));
        assert!(month.click(x, y, &theme));
        assert!(month.click(x, y, &theme));
        assert_eq!(month.shown(), date(2026, 11, 1));
        assert_eq!(Month::size(&theme), size, "as large");
        // A day, or the heading, is only read.
        let (x, y) = middle(layout.cell(3, 0));
        assert!(!month.click(x, y, &theme));
        let (x, y) = middle(layout.cell(3, 4));
        assert!(!month.click(x, y, &theme));
        assert!(!month.click(-1, -1, &theme));
        month.turn(true);
        assert_eq!(month.shown(), date(2026, 12, 1));
    }

    #[test]
    fn the_pointer_on_an_arrow_and_off_it_changes_how_it_looks() {
        let theme = theme();
        let mut month = Month::new(said(), date(2026, 11, 14), Weekday::Monday);
        let (x, y) = middle(Layout::new(&theme).cell(6, 0));
        assert!(month.point(x, y, &theme));
        assert!(!month.point(x + 1, y, &theme), "still on it");
        let lit = month.paint(&mut Text::new(), &theme, Scale::ONE);
        assert!(month.point(-1, -1, &theme));
        let unlit = month.paint(&mut Text::new(), &theme, Scale::ONE);
        let (left, top, ..) = Layout::new(&theme).cell(6, 0);
        let corner = (left.unsigned_abs() + 4, top.unsigned_abs() + 4);
        assert_ne!(lit.at(corner.0, corner.1), unlit.at(corner.0, corner.1));
    }

    #[test]
    fn the_day_under_the_pointer_stands_out_most_and_its_week_and_weekday_more_than_the_rest() {
        let theme = theme();
        let layout = Layout::new(&theme);
        let mut month = Month::new(said(), date(2026, 11, 14), Weekday::Monday);
        // The nineteenth, a Thursday in the fourth week; the room between
        // it and the next day is still its own.
        let (left, top, side, _) = layout.cell(3, 5);
        let between = left + side.cast_signed() + 1;
        assert!(month.point(between, top + 4, &theme));
        assert!(!month.point(left + 4, top + 4, &theme), "the same day");
        let border = |states: States| argb(theme.style(Part::PopoverDay, states).border.left.color);
        let (plain, in_line, under, today) = (
            border(States::NONE),
            border(States::NONE.with(State::Focused)),
            border(States::NONE.with(State::Focused).with(State::Hovered)),
            border(States::NONE.with(State::Focused).with(State::Active)),
        );
        assert!(plain != in_line && in_line != under && under != plain);
        let canvas = month.paint(&mut Text::new(), &theme, Scale::ONE);
        let edge = |across, down| {
            let (left, top, ..) = layout.cell(across, down);
            canvas.at(left.unsigned_abs(), top.unsigned_abs() + layout.side / 2)
        };
        assert_eq!(edge(3, 5), under);
        // Its weekday, October's day in it among them, and its week.
        for (across, down) in [(3, 2), (3, 7), (0, 5), (6, 5)] {
            assert_eq!(edge(across, down), in_line, "{across}, {down}");
        }
        assert_eq!(edge(0, 2), plain);
        assert_eq!(edge(5, 4), today, "today is in the accent wherever it is");
    }

    #[test]
    fn a_week_begins_on_the_day_given_and_the_names_of_the_days_follow_it() {
        let shown = |first| {
            let month = Month::new(said(), date(2026, 11, 14), first);
            let days: Vec<(u32, u32, Date)> = month.days().collect();
            let weekdays: Vec<String> = month.weekdays().cloned().collect();
            (days, weekdays)
        };
        // November 2026 begins on a Sunday: begun on Sunday, its weeks have
        // a week of October before it, and begun on Saturday, a day.
        let (days, weekdays) = shown(Weekday::Sunday);
        assert_eq!(days[0], (0, 0, date(2026, 10, 25)));
        assert_eq!(days[7], (0, 1, date(2026, 11, 1)));
        assert_eq!(days[20], (6, 2, date(2026, 11, 14)));
        assert_eq!(weekdays, ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]);
        let (days, weekdays) = shown(Weekday::Saturday);
        assert_eq!(days[0], (0, 0, date(2026, 10, 31)));
        assert_eq!(days[1], (1, 0, date(2026, 11, 1)));
        assert_eq!(weekdays, ["Sa", "Su", "Mo", "Tu", "We", "Th", "Fr"]);
        let (days, weekdays) = shown(Weekday::Monday);
        assert_eq!(days[6], (6, 0, date(2026, 11, 1)));
        assert_eq!(weekdays, ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]);
    }

    #[test]
    fn the_names_of_the_days_are_drawn_over_the_weeks_from_the_day_they_begin_on() {
        let theme = theme();
        let layout = Layout::new(&theme);
        // Only Sunday is named, so its ink says where it is drawn, in any
        // font.
        let only_sunday = |name: &str| Said {
            weekdays: ["", "", "", "", "", "", name].map(str::to_owned),
            ..said()
        };
        let sunday_at = |first| {
            let paint = |said| {
                let month = Month::new(said, date(2026, 11, 14), first);
                month.paint(&mut Text::new(), &theme, Scale::ONE)
            };
            let (unnamed, named) = (paint(only_sunday("")), paint(only_sunday("Su")));
            let inked = |across: &u32| {
                let (left, top, side, _) = layout.cell(*across, 1);
                let (left, top) = (left.unsigned_abs(), top.unsigned_abs());
                (top..top + side)
                    .any(|y| (left..left + side).any(|x| unnamed.at(x, y) != named.at(x, y)))
            };
            (0..7).filter(inked).collect::<Vec<u32>>()
        };
        assert_eq!(sunday_at(Weekday::Monday), [6]);
        assert_eq!(sunday_at(Weekday::Sunday), [0]);
        assert_eq!(sunday_at(Weekday::Saturday), [1]);
    }

    #[test]
    fn a_week_begins_on_the_day_the_locale_of_the_time_begins_it_on() {
        assert_eq!(first_weekday("en_US.UTF-8"), Weekday::Sunday);
        assert_eq!(first_weekday("pl_PL.UTF-8"), Weekday::Monday);
        assert_eq!(first_weekday("ar_EG.UTF-8"), Weekday::Saturday);
        assert_eq!(first_weekday("bn_BD"), Weekday::Friday);
        assert_eq!(first_weekday("sr_RS@latin"), Weekday::Monday);
        assert_eq!(first_weekday("he_IL@euro"), Weekday::Sunday);
        assert_eq!(first_weekday("C.UTF-8"), Weekday::Monday, "no territory");
        assert_eq!(first_weekday(""), Weekday::Monday, "nothing set");
    }

    #[test]
    fn today_is_drawn_in_the_accent_and_no_other_day() {
        let theme = theme();
        let accent = theme
            .style(Part::PopoverDay, States::NONE.with(State::Active))
            .background;
        let layout = Layout::new(&theme);
        let mut month = Month::new(said(), date(2026, 11, 14), Weekday::Monday);
        // The fourteenth of November 2026 is a Saturday, in the third week,
        // the first being the one the month begins in, on a Sunday.
        let cells = |month: &Month| {
            let canvas = month.paint(&mut Text::new(), &theme, Scale::ONE);
            let lit: Vec<(u32, u32)> = (0..7)
                .flat_map(|across| (2..2 + WEEKS).map(move |down| (across, down)))
                .filter(|(across, down)| {
                    let (left, top, ..) = layout.cell(*across, *down);
                    canvas.at(
                        left.unsigned_abs() + 2,
                        top.unsigned_abs() + layout.side / 2,
                    ) == argb(accent)
                })
                .collect();
            lit
        };
        assert_eq!(cells(&month), [(5, 4)]);
        month.turn(true);
        assert_eq!(cells(&month), [], "today is not in December's weeks");
    }
}
