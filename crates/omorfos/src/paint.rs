//! Drawing the compositor's own pixels: titlebars and, later, menus.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which is then uploaded
//! as a texture. That keeps drawing independent of the renderer, so the same
//! code runs on a GPU and in tests, and it only happens when what is drawn
//! changes, not every frame.

use omorfos_config::{Color, Edges, Line, Part, State, States, Style, Theme};
use omorfos_core::{Button, Decoration, Item, Menu, Panel, SwitcherLayout};
use omorfos_ui::{Area, Image, inset};
pub(crate) use omorfos_ui::{Canvas, Motion, Text};

use crate::Scale;
use crate::bridge::BUTTONS;

/// The width set aside on a menu row for the arrow that marks a submenu.
const ARROW: u32 = 14;

/// The height of the pointer arrow, in pixels.
const POINTER: u32 = 16;

/// Paints the border where it turns a bottom corner of a frame: a quarter
/// ring `radius` across and `border` thick, smooth at both edges. It is the
/// left-hand corner, or the right-hand one if `mirrored`.
pub(crate) fn corner(
    color: Color,
    radius: u32,
    border: u32,
    mirrored: bool,
    scale: Scale,
) -> Canvas {
    let mut canvas = Canvas::scaled(radius, radius, scale);
    canvas.quarter_ring(color, border, mirrored);
    canvas
}

/// How a frame's decoration looks at the moment, which while it is gaining
/// or losing focus is somewhere between how it looks with and without.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Looks {
    /// Whether the frame has keyboard focus.
    pub(crate) focused: bool,
    /// Which tab is the one being shown.
    pub(crate) active: usize,
    /// The frame: its border and how far its corners are rounded.
    pub(crate) window: Style,
    pub(crate) titlebar: Style,
    /// Each tab, in order.
    pub(crate) tabs: Vec<Style>,
    /// Each button of the titlebar, from the left.
    pub(crate) buttons: [Style; 3],
}

impl Looks {
    /// The states of a frame's tab.
    fn tab_states(focused: bool, active: bool) -> States {
        let states = States::NONE.when(State::Focused, focused);
        states.when(State::Active, active)
    }

    /// How the frame showing the window numbered `shown` looks, with a tab
    /// for each of the windows numbered `tabs`, of which `active` is the
    /// one being shown, and the pointer over the button `hovered`, if over
    /// any. Whatever has just changed state is as far on its way as
    /// `motion` has it.
    pub(crate) fn of(
        theme: &Theme,
        motion: &mut Motion,
        shown: u64,
        tabs: impl Iterator<Item = u64>,
        (active, hovered): (usize, Option<Button>),
        focused: bool,
    ) -> Self {
        let states = States::NONE.when(State::Focused, focused);
        let buttons = BUTTONS.map(|(button, part)| {
            let states = states.when(State::Hovered, hovered == Some(button));
            motion.style(theme, part, shown, states)
        });
        let tabs = tabs.enumerate().map(|(index, window)| {
            let states = Self::tab_states(focused, index == active);
            motion.style(theme, Part::Tab, window, states)
        });
        Self {
            focused,
            active,
            tabs: tabs.collect(),
            buttons,
            window: motion.style(theme, Part::Window, shown, states),
            titlebar: motion.style(theme, Part::Titlebar, shown, states),
        }
    }
}

/// Paints a frame's titlebar: one tab per window with its title, and the
/// outline that continues the border around the top.
///
/// `titles` are the tabs' titles in order, and `looks` is how the frame
/// and each of them looks.
pub(crate) fn titlebar(
    text: &mut Text,
    theme: &Theme,
    decoration: &Decoration,
    titles: &[&str],
    looks: &Looks,
    scale: Scale,
) -> Canvas {
    let bar = decoration.titlebar();
    let (width, height) = (bar.size.width, bar.size.height);
    // A frame's border is the same all round; the top stands for it.
    let outline = looks.window.border.top;
    let border = outline.width;
    let edge = border.cast_signed();
    let mut canvas = Canvas::scaled(width, height, scale);

    canvas.fill((0, 0, width, height), outline.color);
    let inner_height = height.saturating_sub(border);
    canvas.fill(
        (edge, edge, width.saturating_sub(border * 2), inner_height),
        looks.titlebar.background,
    );

    for ((button, _), style) in BUTTONS.into_iter().zip(&looks.buttons) {
        let share = decoration.button(button);
        let left = share.origin.x - bar.origin.x;
        let area = inset((left, 0, share.size.width, share.size.height), style.margin);
        canvas.draw_box(area, style);
        sign(&mut canvas, button, area, style.color);
    }

    let tabbed = titles.len() > 1;
    for (index, (title, style)) in titles.iter().zip(&looks.tabs).enumerate() {
        let tab = decoration.tab(index);
        let left = tab.origin.x - bar.origin.x;
        let tab_states = Looks::tab_states(looks.focused, index == looks.active);
        let mut style = *style;
        // The line on a tab's left is the line between it and the tab
        // before, which the first has none of. A title alone in its
        // titlebar is not set apart as a tab at all.
        if index == 0 {
            style.border.left.width = 0;
        }
        if !tabbed {
            style.border = Edges::all(Line::NONE);
        }
        let area = (left, edge, tab.size.width, inner_height);
        if tabbed {
            canvas.draw_box(area, &style);
        }
        let font = theme.font(Part::Tab, tab_states);
        text.draw(
            &mut canvas,
            title,
            inset(area, style.inset()),
            style.color,
            font,
        );
    }

    canvas.round_top_corners(looks.window.radius);
    canvas
}

/// Draws on a titlebar button what it does, for those who cannot tell
/// the buttons apart by colour: a cross to close, a plus to make larger,
/// a bar to put away. Each is half as large as the button and in the
/// middle of it.
fn sign(canvas: &mut Canvas, button: Button, (left, top, width, height): Area, ink: Color) {
    let size = width.min(height) / 2;
    let x = left.saturating_add_unsigned((width - size) / 2);
    let y = top.saturating_add_unsigned((height - size) / 2);
    let middle = (size / 2).cast_signed();
    match button {
        Button::Close => {
            for step in 0..size.cast_signed() {
                let across = size.cast_signed() - 1 - step;
                canvas.fill((x + step, y + step, 1, 1), ink);
                canvas.fill((x + across, y + step, 1, 1), ink);
            }
        }
        Button::Maximize => {
            canvas.fill((x, y + middle, size, 1), ink);
            canvas.fill((x + middle, y, 1, size), ink);
        }
        Button::Minimize => canvas.fill((x, y + middle, size, 1), ink),
    }
}

/// How each row of each panel of an open menu looks, from the root panel
/// outwards. A row the pointer has just come onto or left is as far on its
/// way as `motion` has it.
pub(crate) fn menu_looks<A: Clone>(
    theme: &Theme,
    motion: &mut Motion,
    menu: &Menu<A>,
) -> Vec<Vec<Style>> {
    let panels = menu.panels().iter().zip(0_u64..);
    let looks = panels.map(|(panel, depth)| {
        let rows = (0..panel.items().len()).zip(0_u64..);
        let looks = rows.map(|(index, row)| {
            let highlighted = panel.highlighted() == Some(index);
            let states = States::NONE.when(State::Hovered, highlighted);
            // A row is told from the others by its panel and its place.
            motion.style(theme, Part::MenuItem, depth << 32 | row, states)
        });
        looks.collect()
    });
    looks.collect()
}

/// Paints one panel of an open menu: its rows, each as `looks` has it,
/// which is how the row under the pointer comes to be highlighted, and an
/// arrow on rows that open a submenu.
pub(crate) fn menu_panel<A: Clone>(
    text: &mut Text,
    theme: &Theme,
    menu: &Menu<A>,
    panel: &Panel<A>,
    looks: &[Style],
    scale: Scale,
) -> Canvas {
    let bounds = menu.panel_rect(panel);
    let (width, height) = (bounds.size.width, bounds.size.height);
    let style = theme.style(Part::Menu, States::NONE);
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    // Rows run from one side of the menu to the other, inside its border.
    let (edge, _, inner, _) = inset(whole, style.border.widths());

    let rows = panel.items().iter().zip(menu.rows(panel)).zip(looks);
    for (index, ((item, row), style)) in rows.enumerate() {
        let top = row.origin.y - bounds.origin.y;
        let area = (edge, top, inner, row.size.height);
        let label = match item {
            Item::Separator => {
                // As long as the labels of the rows around it.
                let plain = theme.style(Part::MenuItem, States::NONE);
                let (left, _, room, _) = inset(area, plain.inset());
                let middle = top + (row.size.height / 2).cast_signed();
                let line = theme.style(Part::MenuSeparator, States::NONE);
                canvas.fill((left, middle, room, 1), line.color);
                continue;
            }
            Item::Entry { label, .. } | Item::Submenu { label, .. } => label,
        };
        let highlighted = panel.highlighted() == Some(index);
        let states = States::NONE.when(State::Hovered, highlighted);
        let font = theme.font(Part::MenuItem, states);
        canvas.draw_box(area, style);
        let (left, top, room, tall) = inset(area, style.inset());
        let arrow = if matches!(item, Item::Submenu { .. }) {
            ARROW
        } else {
            0
        };
        let room = room.saturating_sub(arrow);
        text.draw(
            &mut canvas,
            label,
            (left, top, room, tall),
            style.color,
            font,
        );
        if arrow > 0 {
            let left = left.saturating_add_unsigned(room);
            let area = (left, top, ARROW, tall);
            text.draw(&mut canvas, "\u{203a}", area, style.color, font);
        }
    }
    canvas
}

/// The height of one line of the problem notice, in pixels.
const NOTICE_ROW: u32 = 22;

/// The most problems the notice lists before summarising the rest.
const NOTICE_LINES: usize = 6;

/// Paints the notice that tells the user something is wrong: a heading and
/// one line per problem, each cut short if it does not fit.
pub(crate) fn notice(
    text: &mut Text,
    theme: &Theme,
    problems: &[String],
    width: u32,
    scale: Scale,
) -> Canvas {
    let hidden = problems.len().saturating_sub(NOTICE_LINES);
    let heading = "Problems (click to dismiss)".to_owned();
    let more = (hidden > 0).then(|| format!("\u{2026} and {hidden} more"));
    let lines: Vec<&String> = std::iter::once(&heading)
        .chain(problems.iter().take(NOTICE_LINES))
        .chain(more.iter())
        .collect();

    let style = theme.style(Part::Notice, States::NONE);
    let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    let height = rows.saturating_mul(NOTICE_ROW);
    let height = height.saturating_add(style.inset().down());
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    let (left, mut top, room, _) = inset(whole, style.inset());
    for (index, line) in lines.into_iter().enumerate() {
        // The first line is the heading.
        let part = if index == 0 {
            Part::NoticeHeading
        } else {
            Part::Notice
        };
        let ink = theme.style(part, States::NONE).color;
        let font = theme.font(part, States::NONE);
        text.draw(&mut canvas, line, (left, top, room, NOTICE_ROW), ink, font);
        top += NOTICE_ROW.cast_signed();
    }
    canvas
}

/// Paints what the compositor says of what the pointer rests on: a line of
/// text for each of `lines`, in a box as wide as the widest.
pub(crate) fn tooltip(text: &mut Text, theme: &Theme, lines: &[String], scale: Scale) -> Canvas {
    let style = theme.style(Part::Tooltip, States::NONE);
    let font = theme.font(Part::Tooltip, States::NONE);
    let widest = lines
        .iter()
        .map(|line| text.measure(line, font))
        .max()
        .unwrap_or(0);
    let inset = style.inset();
    let width = widest.saturating_add(inset.across());
    let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    let height = rows.saturating_mul(NOTICE_ROW).saturating_add(inset.down());
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    let (left, mut top, room, _) = omorfos_ui::inset(whole, inset);
    for line in lines {
        text.draw(
            &mut canvas,
            line,
            (left, top, room, NOTICE_ROW),
            style.color,
            font,
        );
        top += NOTICE_ROW.cast_signed();
    }
    canvas
}

/// Paints a window's title as it is written under its picture where it is
/// offered in a part with nothing in it: in the `offer` part's colour and
/// font in `states`, `height` tall and only as wide as it is, but no wider
/// than `room`, where it is cut short.
pub(crate) fn offer_title(
    text: &mut Text,
    (theme, states): (&Theme, States),
    title: &str,
    (room, height): (u32, u32),
    scale: Scale,
) -> Canvas {
    let style = theme.style(Part::Offer, states);
    let font = theme.font(Part::Offer, states);
    let width = text.measure(title, font).clamp(1, room.max(1));
    let mut canvas = Canvas::scaled(width, height, scale);
    text.draw(&mut canvas, title, (0, 0, width, height), style.color, font);
    canvas
}

/// How tall a line of text of a size in points is, in units of layout.
pub(crate) const fn line_height(points: u32) -> u32 {
    points.saturating_mul(5) / 3
}

/// The room between an icon and the title beside it.
const BESIDE: u32 = 6;

/// The largest an icon is drawn where a window's picture would be.
pub(crate) const LARGE_ICON: u32 = 64;

/// What the switcher shows of one window, but for its picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shown<I> {
    pub(crate) title: String,
    /// Its icon, if it has one: a picture, or what the renderer draws one
    /// from.
    pub(crate) icon: Option<I>,
    /// Whether its picture is shown; if not, its icon is shown large in
    /// its place.
    pub(crate) pictured: bool,
}

/// Where the switcher shows what it shows of one window in its `place`,
/// `picture` tall: its icon beside its title, the title, and the icon
/// large where the picture would be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Places {
    pub(crate) icon: Area,
    pub(crate) title: Area,
    pub(crate) large: Area,
}

impl Places {
    /// Where things go in `place`, as the theme has it in `states`, for a
    /// window with an icon if `iconic`.
    pub(crate) fn of(
        place: Area,
        theme: &Theme,
        states: States,
        picture: u32,
        iconic: bool,
    ) -> Self {
        let style = theme.style(Part::SwitcherItem, states);
        let (left, top, room, down) = inset(place, style.inset());
        let font = theme.font(Part::SwitcherTitle, states);
        let tall = line_height(font.size.get()).min(down);
        let bottom = top.saturating_add_unsigned(down - tall);
        let icon = (left, bottom, tall.min(room), tall);
        let aside = if iconic { (tall + BESIDE).min(room) } else { 0 };
        let title = (
            left.saturating_add_unsigned(aside),
            bottom,
            room - aside,
            tall,
        );
        let picture = picture.min(down);
        let side = LARGE_ICON.min(room).min(picture);
        let large = (
            left.saturating_add_unsigned((room - side) / 2),
            top.saturating_add_unsigned((picture - side) / 2),
            side,
            side,
        );
        Self { icon, title, large }
    }
}

/// Paints the switcher as `layout` has it: its box, each window's place in
/// it, the one picked set apart, and under where each picture is to go its
/// title and the icon it has as a picture, in the order of `shown`. Where
/// a window's picture is not shown, its icon is, large.
pub(crate) fn switcher(
    text: &mut Text,
    theme: &Theme,
    layout: &SwitcherLayout,
    shown: &[Shown<Option<Image>>],
    picture: u32,
    scale: Scale,
) -> Canvas {
    let area = layout.area;
    let (width, height) = (area.size.width, area.size.height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(
        (0, 0, width, height),
        &theme.style(Part::Switcher, States::NONE),
    );
    for ((_, at, picked), shown) in layout.items.iter().zip(shown) {
        let states = States::NONE.when(State::Active, *picked);
        let style = theme.style(Part::SwitcherItem, states);
        let left = at.origin.x - area.origin.x;
        let top = at.origin.y - area.origin.y;
        let place = (left, top, at.size.width, at.size.height);
        canvas.draw_box(place, &style);
        let places = Places::of(place, theme, states, picture, shown.icon.is_some());
        let font = theme.font(Part::SwitcherTitle, states);
        let ink = theme.style(Part::SwitcherTitle, states).color;
        text.draw(&mut canvas, &shown.title, places.title, ink, font);
        if let Some(Some(image)) = &shown.icon {
            canvas.draw_image(places.icon, image);
            if !shown.pictured {
                canvas.draw_image(places.large, image);
            }
        }
    }
    canvas
}

/// What the mark that says something is captured shows a sign of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Signs {
    /// That pictures are taken of the screen.
    pub(crate) screen: bool,
    /// That a microphone, or the sound that plays, is recorded.
    pub(crate) sound: bool,
    /// That a camera is.
    pub(crate) camera: bool,
}

impl Signs {
    /// Whether there is anything to show.
    pub(crate) const fn any(self) -> bool {
        self.screen || self.sound || self.camera
    }

    /// The signs there are, in the order they are shown.
    fn shown(self) -> impl Iterator<Item = Sign> {
        let signs = [
            (self.screen, Sign::Screen),
            (self.sound, Sign::Sound),
            (self.camera, Sign::Camera),
        ];
        signs
            .into_iter()
            .filter(|(shown, _)| *shown)
            .map(|(_, sign)| sign)
    }
}

/// One sign of the mark.
#[derive(Debug, Clone, Copy)]
enum Sign {
    Screen,
    Sound,
    Camera,
}

/// How much room the mark has round its signs, and how much each has.
const CAPTURE_EDGE: u32 = 4;
const CAPTURE_SIGN: u32 = 16;
const CAPTURE_DOT: u32 = 8;

/// The size of the mark with `signs`: a sign's room each, side by side.
pub(crate) fn capture_mark_size(signs: Signs) -> (u32, u32) {
    let count = u32::try_from(signs.shown().count()).unwrap_or(1).max(1);
    (CAPTURE_EDGE * 2 + CAPTURE_SIGN * count, CAPTURE_SIGN)
}

/// Paints the mark that says something is captured: a sign for each of
/// `signs`, in the part's text colour on its background. A dot for the
/// screen, a microphone for sound, and a camera.
pub(crate) fn capture_mark(theme: &Theme, scale: Scale, signs: Signs) -> Canvas {
    let style = theme.style(Part::Capture, States::NONE);
    let (width, height) = capture_mark_size(signs);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box((0, 0, width, height), &style);
    let ink = |radius: u32| Style {
        background: style.color,
        radius,
        ..Style::NONE
    };
    for (index, sign) in signs.shown().enumerate() {
        let across = u32::try_from(index).unwrap_or(0);
        let left = (CAPTURE_EDGE + CAPTURE_SIGN * across).cast_signed();
        let shapes: &[(i32, i32, u32, u32, u32)] = match sign {
            // A dot in the middle.
            Sign::Screen => &[(4, 4, CAPTURE_DOT, CAPTURE_DOT, CAPTURE_DOT / 2)],
            // A microphone on a stand.
            Sign::Sound => &[(5, 2, 6, 8, 3), (7, 10, 2, 2, 0), (4, 12, 8, 2, 1)],
            // A camera, its body and its lens.
            Sign::Camera => &[(1, 4, 10, 8, 2), (11, 6, 4, 4, 1)],
        };
        for (x, y, wide, high, radius) in shapes {
            canvas.draw_box((left + x, *y, *wide, *high), &ink(*radius));
        }
    }
    canvas
}

/// Paints the pointer: a white arrow with a dark outline, whose tip is the
/// top-left pixel, so that it shows on any background.
pub(crate) fn pointer(scale: Scale) -> Canvas {
    let outline = Color::rgb(0x14, 0x16, 0x1b);
    let inside = Color::rgb(0xff, 0xff, 0xff);
    // The arrow widens by a pixel a row down its left edge, then closes
    // twice as fast.
    let width = |row: u32| (row + 1).min((POINTER - row) * 2);
    let mut canvas = Canvas::scaled(POINTER * 3 / 4, POINTER, scale);
    for row in 0..POINTER {
        canvas.fill((0, row.cast_signed(), width(row), 1), outline);
    }
    for row in 2..POINTER - 2 {
        let inner = width(row).min(width(row + 1)).saturating_sub(2);
        canvas.fill((1, row.cast_signed(), inner, 1), inside);
    }
    canvas
}

#[cfg(test)]
mod tests {
    use omorfos_core::Metrics as FrameMetrics;
    use omorfos_core::geometry::{Point, Rect, Size};

    use super::*;

    /// The colours of a theme, by what the built-in theme uses them for.
    struct Palette {
        accent: Color,
        focused: Tone,
        unfocused: Tone,
    }

    /// The colours of a frame and its titlebar, with focus or without.
    #[derive(Clone, Copy)]
    struct Tone {
        border: Color,
        titlebar: Color,
    }

    fn palette(theme: &Theme) -> Palette {
        let tone = |states| Tone {
            border: theme.style(Part::Window, states).border.top.color,
            titlebar: theme.style(Part::Titlebar, states).background,
        };
        Palette {
            accent: theme.style(Part::NoticeHeading, States::NONE).color,
            focused: tone(States::NONE.with(State::Focused)),
            unfocused: tone(States::NONE),
        }
    }

    /// A titlebar with nothing on its way anywhere.
    fn titlebar(
        text: &mut Text,
        theme: &Theme,
        decoration: &Decoration,
        titles: &[&str],
        active: usize,
        focused: bool,
        scale: Scale,
    ) -> Canvas {
        let tabs = (0_u64..).take(titles.len());
        let state = (active, None);
        let looks = Looks::of(theme, &mut Motion::new(), 0, tabs, state, focused);
        super::titlebar(text, theme, decoration, titles, &looks, scale)
    }

    /// The first panel of a menu with nothing on its way anywhere.
    fn menu_panel(text: &mut Text, theme: &Theme, menu: &Menu<()>) -> Canvas {
        let looks = menu_looks(theme, &mut Motion::new(), menu);
        super::menu_panel(text, theme, menu, &menu.panels()[0], &looks[0], Scale::ONE)
    }

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    #[test]
    fn the_pointer_is_a_light_arrow_with_a_dark_edge_and_its_tip_at_the_corner() {
        let arrow = pointer(Scale::ONE);
        let dark = 0xff14_161b;
        assert_eq!(arrow.at(0, 0), dark, "the tip");
        assert_eq!(arrow.at(0, 8), dark, "its left edge");
        assert_eq!(arrow.at(2, 8), 0xffff_ffff, "its inside");
        assert_eq!(arrow.at(8, 8), dark, "its slanted edge");
        assert_eq!(arrow.at(9, 8), 0, "clear beyond it");
        assert_eq!(arrow.at(5, 2), 0, "clear above the slant");
        assert_eq!(arrow.at(1, 15), dark, "closed at the bottom");
    }

    #[test]
    fn text_and_corners_grow_with_the_scale() {
        let theme = Theme::default();
        let mut text = Text::new();
        let mut paint =
            |scale| titlebar(&mut text, &theme, &frame(1), &["Terminal"], 0, true, scale);
        let plain = paint(Scale::ONE);
        let doubled = paint(Scale::new(2.0));
        assert_eq!((doubled.width(), doubled.height()), (800, 56));
        assert_eq!(doubled.units(), (400, 28));

        let background = argb(palette(&theme).focused.titlebar);
        let ink = |canvas: &Canvas, columns| inked(canvas, columns, background);
        assert!(
            ink(&doubled, 20..240) > ink(&plain, 10..120) * 3,
            "letters twice as tall and twice as wide cover about four times as much"
        );
        assert_eq!(plain.at(1, 1), 0, "inside the rounded corner");
        assert_eq!(doubled.at(3, 3), 0, "which is twice the size");
    }

    /// The number of pixels in a row range that are not exactly `background`.
    fn inked(canvas: &Canvas, columns: std::ops::Range<u32>, background: u32) -> usize {
        (0..canvas.height())
            .flat_map(|y| columns.clone().map(move |x| (x, y)))
            .filter(|(x, y)| canvas.at(*x, *y) != background)
            .count()
    }

    #[test]
    fn a_menu_panel_shows_its_rows_with_the_hovered_one_highlighted() {
        use omorfos_core::MenuMetrics;

        let theme = Theme::default();
        let colors = palette(&theme);
        let mut text = Text::new();
        let items = vec![
            Item::Entry {
                label: "Terminal".to_owned(),
                action: (),
            },
            Item::Separator,
            Item::Submenu {
                label: "Session".to_owned(),
                items: Vec::new(),
            },
        ];
        let metrics = MenuMetrics {
            width: 200,
            row: 28,
            separator: 9,
            padding: 4,
        };
        let area = Rect::new(Point::new(0, 0), Size::new(1000, 800));
        let mut menu = Menu::open(items, Point::new(300, 200), area, metrics);

        // Rows: Terminal at 4..32, the separator at 32..41, Session at 41..69.
        let plain = menu_panel(&mut text, &theme, &menu);
        assert_eq!((plain.width(), plain.height()), (200, 73));
        assert_eq!(plain.at(0, 30), argb(colors.unfocused.border), "outlined");
        assert_eq!(plain.at(190, 10), argb(colors.focused.titlebar));
        assert_eq!(
            plain.at(100, 36),
            argb(colors.unfocused.border),
            "the separator's line"
        );
        assert!(
            inked(&plain, 10..100, argb(colors.focused.titlebar)) > 40,
            "labels"
        );
        let arrow = (176..190).flat_map(|x| (41..69).map(move |y| (x, y)));
        let arrow_ink = arrow
            .filter(|(x, y)| plain.at(*x, *y) != argb(colors.focused.titlebar))
            .count();
        assert!(arrow_ink > 3, "the submenu row has an arrow");

        menu.hover(Point::new(350, 215));
        let lit = menu_panel(&mut text, &theme, &menu);
        assert_eq!(
            lit.at(190, 10),
            argb(colors.accent),
            "the hovered row is highlighted"
        );
        assert_eq!(
            lit.at(190, 50),
            argb(colors.focused.titlebar),
            "the others are not"
        );
    }

    #[test]
    fn a_notice_lists_each_problem_under_a_heading() {
        let theme = Theme::default();
        let colors = palette(&theme);
        let mut text = Text::new();
        let problems = vec![
            "theme.css:2:13: `nope` is not a color".to_owned(),
            "keys.yaml: no such key".to_owned(),
        ];

        let canvas = notice(&mut text, &theme, &problems, 400, Scale::ONE);

        // A heading and two problems, 22 pixels each, inside the padding
        // and the border.
        assert_eq!((canvas.width(), canvas.height()), (400, 88));
        assert_eq!(
            canvas.at(0, 40),
            argb(colors.accent),
            "outlined in the accent colour"
        );
        assert_eq!(canvas.at(395, 40), argb(colors.focused.titlebar));
        let row = |top: u32| (10..300).flat_map(move |x| (top..top + 22).map(move |y| (x, y)));
        let ink = |top: u32| {
            row(top)
                .filter(|(x, y)| canvas.at(*x, *y) != argb(colors.focused.titlebar))
                .count()
        };
        assert!(
            ink(10) > 30 && ink(32) > 30 && ink(54) > 30,
            "three lines of text"
        );
    }

    #[test]
    fn a_long_list_of_problems_is_summarised() {
        let mut text = Text::new();
        let few: Vec<String> = (0..6).map(|n| format!("problem {n}")).collect();
        let many: Vec<String> = (0..40).map(|n| format!("problem {n}")).collect();
        let exactly = notice(&mut text, &Theme::default(), &few, 400, Scale::ONE);
        let capped = notice(&mut text, &Theme::default(), &many, 400, Scale::ONE);
        assert_eq!(exactly.height(), 7 * 22 + 22, "a heading and six problems");
        assert_eq!(
            capped.height(),
            8 * 22 + 22,
            "the same, and a line counting the rest"
        );
    }

    const METRICS: FrameMetrics = FrameMetrics {
        border: 1,
        titlebar: 28,
        buttons: [0; 3],
    };

    fn frame(tabs: usize) -> Decoration {
        let outer = Rect::new(Point::new(300, 200), Size::new(400, 300));
        Decoration::new(outer, METRICS, tabs)
    }

    #[test]
    fn a_titlebar_spans_the_frame_with_an_outline_and_rounded_corners() {
        let theme = Theme::default();
        let mut text = Text::new();
        let canvas = titlebar(
            &mut text,
            &theme,
            &frame(1),
            &["Terminal"],
            0,
            true,
            Scale::ONE,
        );

        assert_eq!((canvas.width(), canvas.height()), (400, 28));
        assert_eq!(canvas.at(0, 0), 0, "rounded");
        assert_eq!(
            canvas.at(200, 0),
            argb(palette(&theme).focused.border),
            "outline along the top"
        );
        assert_eq!(
            canvas.at(0, 20),
            argb(palette(&theme).focused.border),
            "and down the side"
        );
        assert_eq!(canvas.at(399, 20), argb(palette(&theme).focused.border));
        assert_eq!(canvas.at(300, 14), argb(palette(&theme).focused.titlebar));
        assert_eq!(
            canvas.at(300, 27),
            argb(palette(&theme).focused.titlebar),
            "no underline"
        );
        assert!(
            inked(&canvas, 10..120, argb(palette(&theme).focused.titlebar)) > 20,
            "the title"
        );
    }

    #[test]
    fn an_unfocused_titlebar_uses_the_unfocused_colours() {
        let theme = Theme::default();
        let mut text = Text::new();
        let canvas = titlebar(
            &mut text,
            &theme,
            &frame(1),
            &["Terminal"],
            0,
            false,
            Scale::ONE,
        );
        assert_eq!(canvas.at(200, 0), argb(palette(&theme).unfocused.border));
        assert_eq!(canvas.at(300, 14), argb(palette(&theme).unfocused.titlebar));
    }

    /// The colours the built-in theme gives the buttons of a frame with
    /// focus, from the left, and the one it gives them all without.
    const BUTTON_COLORS: [u32; 3] = [0xffe0_6c75, 0xff98_c379, 0xffe5_c07b];
    const BUTTON_PLAIN: u32 = 0xff2c_303a;

    /// The middle of each button under the built-in theme: twelve across
    /// with room around them, the first further in than the others.
    const BUTTON_MIDDLES: [u32; 3] = [16, 36, 56];

    /// A frame with room for the buttons the built-in theme has.
    fn buttoned() -> Decoration {
        let outer = Rect::new(Point::new(300, 200), Size::new(400, 300));
        let metrics = FrameMetrics {
            buttons: [26, 20, 20],
            ..METRICS
        };
        Decoration::new(outer, metrics, 1)
    }

    #[test]
    fn a_titlebar_has_three_round_buttons_at_its_left_coloured_when_it_has_focus() {
        let theme = Theme::default();
        let mut text = Text::new();
        let frame = buttoned();
        let focused = titlebar(&mut text, &theme, &frame, &["Title"], 0, true, Scale::ONE);
        let other = titlebar(&mut text, &theme, &frame, &["Title"], 0, false, Scale::ONE);
        for (middle, color) in BUTTON_MIDDLES.into_iter().zip(BUTTON_COLORS) {
            assert_eq!(focused.at(middle, 14), color);
            assert_eq!(
                other.at(middle, 14),
                BUTTON_PLAIN,
                "all alike without focus"
            );
            // Round, and twelve across: its corner is the titlebar's.
            assert_eq!(focused.at(middle - 5, 14), color);
            assert_ne!(focused.at(middle - 6, 9), color);
            assert_ne!(focused.at(middle + 7, 14), color);
        }
        // The titlebar shows between them, and no title is written there.
        let background = argb(palette(&theme).focused.titlebar);
        for between in [26, 46] {
            assert_eq!(focused.at(between, 14), background);
        }
    }

    #[test]
    fn the_button_under_the_pointer_says_what_it_does() {
        let theme = Theme::default();
        let mut text = Text::new();
        let ink = argb(theme.style(Part::Desktop, States::NONE).background);
        let mut hovering = |button: Option<Button>, focused: bool| {
            let looks = Looks::of(&theme, &mut Motion::new(), 0, 0..1, (0, button), focused);
            super::titlebar(
                &mut text,
                &theme,
                &buttoned(),
                &["Title"],
                &looks,
                Scale::ONE,
            )
        };
        let count = |canvas: &Canvas, middle: u32| {
            let around = (middle - 6..middle + 6).flat_map(|x| (8..20).map(move |y| (x, y)));
            around.filter(|(x, y)| canvas.at(*x, *y) == ink).count()
        };
        let plain = hovering(None, true);
        for middle in BUTTON_MIDDLES {
            assert_eq!(count(&plain, middle), 0, "nothing until it is pointed at");
        }

        // A cross of two diagonals six long, a plus of two bars, and a bar.
        let close = hovering(Some(Button::Close), true);
        assert_eq!(count(&close, 16), 12);
        assert_eq!(close.at(13, 11), ink, "from one corner");
        assert_eq!(close.at(18, 11), ink, "and from the other");
        assert_eq!(count(&close, 36), 0, "and on no other button");
        let maximize = hovering(Some(Button::Maximize), true);
        assert_eq!(count(&maximize, 36), 11);
        let minimize = hovering(Some(Button::Minimize), true);
        assert_eq!(count(&minimize, 56), 6);

        // Pointed at, a button has its colour whether or not its frame
        // has focus.
        let unfocused = hovering(Some(Button::Close), false);
        assert_eq!(unfocused.at(11, 14), BUTTON_COLORS[0]);
        assert_eq!(unfocused.at(36, 14), BUTTON_PLAIN);
    }

    #[test]
    fn a_theme_can_do_without_a_button_and_the_title_takes_its_room() {
        let theme = "button-close, button-maximize, button-minimize { min-width: 0; }";
        let theme = Theme::parse(theme).unwrap();
        let mut text = Text::new();
        let canvas = titlebar(
            &mut text,
            &theme,
            &frame(1),
            &["Title"],
            0,
            true,
            Scale::ONE,
        );
        for (middle, color) in BUTTON_MIDDLES.into_iter().zip(BUTTON_COLORS) {
            assert_ne!(canvas.at(middle, 14), color);
        }
        let background = argb(palette(&theme).focused.titlebar);
        assert!(
            inked(&canvas, 10..66, background) > 10,
            "the title is there"
        );
    }

    #[test]
    fn tabs_are_separated_and_the_shown_one_is_marked() {
        let theme = Theme::default();
        let colors = palette(&theme);
        let mut text = Text::new();
        let titles = ["one", "two"];
        let focused = titlebar(&mut text, &theme, &frame(2), &titles, 1, true, Scale::ONE);

        // Tab 0 covers columns 0..200, tab 1 covers 200..400.
        assert_eq!(
            focused.at(150, 14),
            argb(colors.unfocused.titlebar),
            "the hidden tab is dimmed"
        );
        assert_eq!(focused.at(350, 14), argb(colors.focused.titlebar));
        assert_eq!(
            focused.at(200, 14),
            argb(colors.focused.border),
            "a line separates the tabs"
        );
        assert_eq!(
            focused.at(350, 27),
            argb(colors.accent),
            "the shown tab is underlined"
        );
        assert_eq!(focused.at(150, 27), argb(colors.unfocused.titlebar));

        let unfocused = titlebar(&mut text, &theme, &frame(2), &titles, 0, false, Scale::ONE);
        assert_eq!(
            unfocused.at(150, 27),
            argb(colors.unfocused.border),
            "marked without the accent"
        );
        assert_eq!(unfocused.at(350, 27), argb(colors.unfocused.titlebar));
    }
}
