//! Drawing the compositor's own pixels: titlebars and, later, menus.
//!
//! Everything is drawn on the CPU into a [`Canvas`], which is then uploaded
//! as a texture. That keeps drawing independent of the renderer, so the same
//! code runs on a GPU and in tests, and it only happens when what is drawn
//! changes, not every frame.

use omorfos_config::{Color, Edges, Font, Line, Part, State, States, Style, Theme};
use omorfos_core::{Button, Decoration, Item, Mark, Menu, Panel, SwitcherLayout};
use omorfos_ui::{Area, Image, inset};
pub(crate) use omorfos_ui::{Canvas, Motion, Text};

use crate::Scale;
use crate::bridge::BUTTONS;
use crate::icons::{WindowIcons, icon_pixels};

/// The width set aside on a menu row for the arrow that marks a submenu.
const ARROW: u32 = 14;

/// How large a sign, or a picture, in a row is, as `docs/DESIGN.md` has
/// it.
pub(crate) const SIGN: u32 = 16;

/// `area` grown by `edges` on each side.
const fn outset((x, y, width, height): Area, edges: Edges<u32>) -> Area {
    (
        x.saturating_sub_unsigned(edges.left),
        y.saturating_sub_unsigned(edges.top),
        width.saturating_add(edges.left).saturating_add(edges.right),
        height
            .saturating_add(edges.top)
            .saturating_add(edges.bottom),
    )
}

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

    // The background within the outline alone, so that a background that
    // can be seen through shows what is behind the titlebar, its glass if
    // it has any. It and the outline are each the top of a box taller than
    // the titlebar by as much as its corners need, so that only its top
    // corners are rounded.
    let radius = looks.window.radius;
    let below = radius.saturating_mul(2);
    let inner_height = height.saturating_sub(border);
    let inner_width = width.saturating_sub(border * 2);
    let within = Style {
        background: looks.titlebar.background,
        gradient: looks.titlebar.gradient,
        inset: looks.titlebar.inset,
        ..Style::NONE
    };
    let inside = (edge, edge, inner_width, inner_height.saturating_add(below));
    canvas.draw_box_within(
        (edge, edge, inner_width, inner_height),
        &within,
        (inside, radius.saturating_sub(border)),
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

    // The outline along the top, down each side and round the corners
    // between, over what reaches into them.
    canvas.round_top_corners(radius);
    let rim = Style {
        border: Edges {
            bottom: Line::NONE,
            ..Edges::all(outline)
        },
        radius,
        ..Style::NONE
    };
    canvas.draw_box((0, 0, width, height.saturating_add(below)), &rim);
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
/// which is how the row under the pointer comes to be highlighted, what
/// stands before each label, from `icons`, in the room kept for it on every
/// row, and an arrow on rows that open a submenu.
pub(crate) fn menu_panel<A: Clone>(
    (text, icons): (&mut Text, &mut WindowIcons),
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
    // Rows run from one side of the menu to the other, inside its border
    // and padding, and are kept to its rounded corners, which within the
    // border are rounded less by as much as the border is thick.
    let widths = style.border.widths();
    let inside = inset(whole, widths);
    let (edge, _, inner, _) = inset(whole, style.inset());
    let thickest = [widths.right, widths.bottom, widths.left]
        .into_iter()
        .fold(widths.top, u32::max);
    let within = (inside, style.radius.saturating_sub(thickest));

    let rows = panel.items().iter().zip(menu.rows(panel)).zip(looks);
    for (index, ((item, row), style)) in rows.enumerate() {
        let top = row.origin.y - bounds.origin.y;
        let area = (edge, top, inner, row.size.height);
        let (label, mark) = match item {
            Item::Separator => {
                // From where the signs of the rows around it begin to as
                // far from the other side.
                let plain = theme.style(Part::MenuItem, States::NONE);
                let (left, _, _, _) = inset(area, plain.inset());
                let room = inner.saturating_sub(plain.inset().left * 2);
                let middle = top + (row.size.height / 2).cast_signed();
                let line = theme.style(Part::MenuSeparator, States::NONE);
                canvas.fill((left, middle, room, 1), line.color);
                continue;
            }
            Item::Entry { label, mark, .. } | Item::Submenu { label, mark, .. } => (label, mark),
        };
        let highlighted = panel.highlighted() == Some(index);
        let states = States::NONE.when(State::Hovered, highlighted);
        let font = theme.font(Part::MenuItem, states);
        canvas.draw_box_within(area, style, within);
        let (left, top, room, tall) = inset(area, style.inset());
        // The sign is in the middle of its row, and as far from its label
        // as from the row's start. What the theme gives `menu-sign` is
        // drawn round it, as far out as its padding, and moves nothing.
        let around = tall.saturating_sub(SIGN) / 2;
        let place = (left, top.saturating_add_unsigned(around), SIGN, SIGN);
        let on = matches!(mark, Mark::Toggle(_, true));
        let sign = theme.style(Part::MenuSign, states.when(State::Active, on));
        canvas.draw_box(outset(place, sign.inset()), &sign);
        let pixels = icon_pixels(SIGN, scale);
        if let Some(image) = icons.mark(mark, pixels, sign.color) {
            canvas.draw_image(place, image);
        }
        let used = SIGN + style.inset().left;
        let left = left.saturating_add_unsigned(used);
        let arrow = if matches!(item, Item::Submenu { .. }) {
            ARROW
        } else {
            0
        };
        let room = room.saturating_sub(used + arrow);
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

/// How tall a line of "About" is, in units of layout.
pub(crate) const ABOUT_ROW: u32 = 20;
/// How large the mark is at the top of "About", and the room between it
/// and the name beside it.
pub(crate) const ABOUT_MARK: u32 = 56;
/// The room after the mark, and after each list of what is someone else's.
const ABOUT_GAP: u32 = 12;

/// A button of "About".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AboutButton {
    /// Puts it away.
    Close,
    /// Copies what the computer is, as text.
    Copy,
}

/// How tall "About" is but for what it lists: its padding and border, and
/// the mark with the room below it.
pub(crate) fn about_head(theme: &Theme) -> u32 {
    let style = theme.style(Part::About, States::NONE);
    style.inset().down() + ABOUT_MARK + ABOUT_GAP
}

/// A line of what "About" lists: what is first on it, in its part; then
/// what follows it, in its own, from `column` across where the line is
/// one of a table, or straight after where it is not; and how tall it is.
struct Row {
    first: (Part, String),
    then: (Part, String),
    column: Option<u32>,
    tall: u32,
}

impl Row {
    /// A line with `text` alone on it, in `part`.
    const fn alone(part: Part, text: String, tall: u32) -> Self {
        Self {
            first: (part, text),
            then: (Part::About, String::new()),
            column: None,
            tall,
        }
    }
}

/// What "About" lists, `room` across and as tall as all of it is, on
/// nothing, to be shown on it through a window that scrolls: what the
/// computer is, `system`, in a table of each kind of thing and what there
/// is of it; then each list of `credits` under its heading, each thing's
/// name and its licence, and under them whose it is.
pub(crate) fn about_list(
    text: &mut Text,
    theme: &Theme,
    (system, credits): (&omorfos_core::System, &[omorfos_core::Credits]),
    (room, scale): (u32, Scale),
) -> Canvas {
    let mut lines: Vec<Row> = Vec::new();
    // What it is goes after the widest of the kinds, and a little room.
    let kind = theme.font(Part::AboutDetail, States::NONE);
    let widest = system
        .things
        .iter()
        .map(|(name, _)| text.measure(name, kind));
    let column = widest.max().unwrap_or_default() + ABOUT_GAP;
    let heading = "This computer".to_owned();
    lines.push(Row::alone(Part::AboutHeading, heading, ABOUT_ROW));
    for (name, things) in &system.things {
        for (index, thing) in things.iter().enumerate() {
            // The kind is said once, beside the first of its things.
            let name = if index == 0 { *name } else { "" };
            lines.push(Row {
                first: (Part::AboutDetail, name.to_owned()),
                then: (Part::About, thing.clone()),
                column: Some(column),
                tall: ABOUT_ROW,
            });
        }
    }
    lines.push(Row::alone(Part::About, String::new(), ABOUT_GAP));
    for section in credits {
        let heading = section.heading.clone();
        lines.push(Row::alone(Part::AboutHeading, heading, ABOUT_ROW));
        for credit in &section.credits {
            let licence = format!(" \u{b7} {}", credit.licence);
            lines.push(Row {
                first: (Part::About, credit.what.clone()),
                then: (Part::AboutDetail, licence),
                column: None,
                tall: ABOUT_ROW,
            });
            let by = credit.by.clone();
            lines.push(Row::alone(Part::AboutDetail, by, ABOUT_ROW));
        }
        lines.push(Row::alone(Part::About, String::new(), ABOUT_GAP));
    }
    let height: u32 = lines.iter().map(|line| line.tall).sum();
    let mut canvas = Canvas::scaled(room, height, scale);
    let looks = |part| {
        (
            theme.style(part, States::NONE).color,
            theme.font(part, States::NONE),
        )
    };
    let mut at = 0;
    for line in lines {
        let ((part, first), (then_part, then)) = (line.first, line.then);
        let (ink, font) = looks(part);
        let wide = text.measure(&first, font).min(room);
        text.draw(&mut canvas, &first, (0, at, wide, line.tall), ink, font);
        let after = line.column.unwrap_or(wide).min(room);
        let (ink, font) = looks(then_part);
        let rest = (after.cast_signed(), at, room - after, line.tall);
        text.draw(&mut canvas, &then, rest, ink, font);
        at += line.tall.cast_signed();
    }
    canvas
}

/// Paints "About omorfos", `width` across, with room `list` tall below the
/// mark for what it lists, which is drawn on it apart: `mark`, which is
/// empty if it could not be drawn, and beside it the name, `version` and
/// the licence; and its buttons, the one `hovered` under the pointer. Says
/// where on it the button that closes it is and the one that copies, and
/// where what it lists goes.
pub(crate) fn about(
    text: &mut Text,
    theme: &Theme,
    (mark, version): (&[Image], &str),
    (width, list, scale, hovered): (u32, u32, Scale, Option<AboutButton>),
    copy: (u32, u32),
) -> (Canvas, (Area, Area), Area) {
    let style = theme.style(Part::About, States::NONE);
    let height = about_head(theme) + list;
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    let (left, top, room, _) = inset(whole, style.inset());

    let size = ABOUT_MARK.cast_signed();
    for mark in mark {
        canvas.draw_image((left, top, ABOUT_MARK, ABOUT_MARK), mark);
    }
    // A window's button that closes it, as it is on a window with focus,
    // in the top right corner.
    let under = |button| States::NONE.when(State::Hovered, hovered == Some(button));
    let states = under(AboutButton::Close).with(State::Focused);
    let button = theme.style(Part::ButtonClose, states);
    let across = button.min_width;
    let close = (
        left + room.saturating_sub(across).cast_signed(),
        top,
        across,
        across,
    );
    canvas.draw_box(close, &button);
    sign(&mut canvas, Button::Close, close, button.color);
    // The place of the one that copies, `copy` large, at the foot of the
    // mark, under it. It is drawn apart, as it changes.
    let (wide, high) = copy;
    let copy = (
        left + room.saturating_sub(wide).cast_signed(),
        top + size - high.cast_signed(),
        wide,
        high,
    );

    let beside = left + size + ABOUT_GAP.cast_signed();
    let narrower = room.saturating_sub(ABOUT_MARK + ABOUT_GAP + wide);
    let titles = [
        (Part::AboutHeading, "omorfos".to_owned()),
        (Part::AboutDetail, format!("Version {version}")),
        (Part::AboutDetail, "Under the MIT licence".to_owned()),
    ];
    let mut at = top + (size - (3 * ABOUT_ROW).cast_signed()) / 2;
    for (part, line) in titles {
        let ink = theme.style(part, States::NONE).color;
        let font = theme.font(part, States::NONE);
        text.draw(
            &mut canvas,
            &line,
            (beside, at, narrower, ABOUT_ROW),
            ink,
            font,
        );
        at += ABOUT_ROW.cast_signed();
    }
    let below = (left, top + size + ABOUT_GAP.cast_signed(), room, list);
    (canvas, (close, copy), below)
}

/// What the button of "About" that copies says, before and once it has.
const COPY: &str = "Copy";
const COPIED: &str = "Copied";
/// How large the tick it shows once it has copied is, and the room after
/// it.
const TICK: u32 = 14;

/// How large the button of "About" that copies is: as wide as the wider
/// of what it says before and once it has, so that it stays the size it
/// is.
pub(crate) fn about_copy_size(text: &mut Text, theme: &Theme) -> (u32, u32) {
    let (style, font) = (
        theme.style(Part::AboutButton, States::NONE),
        theme.font(Part::AboutButton, States::NONE),
    );
    let before = text.measure(COPY, font);
    let after = TICK + TICK / 2 + text.measure(COPIED, font);
    let padding = style.inset();
    (
        before.max(after) + padding.across(),
        ABOUT_ROW + padding.down(),
    )
}

/// Paints the button of "About" that copies, `size` large, looking as
/// `style` and `font` say: what it does; or, once it has copied, a tick as
/// much of it drawn as `tick` says, from 0 to 1, and that it has.
pub(crate) fn about_copy(
    text: &mut Text,
    (style, font): (&Style, &Font),
    (size, scale): ((u32, u32), Scale),
    tick: Option<f64>,
) -> Canvas {
    let (wide, high) = size;
    let mut canvas = Canvas::scaled(wide, high, scale);
    canvas.draw_box((0, 0, wide, high), style);
    let (left, top, room, tall) = inset((0, 0, wide, high), style.inset());
    let said = if tick.is_some() { COPIED } else { COPY };
    // What it says, and the tick before it once there is one, in the
    // middle of it.
    let ticked = tick.map_or(0, |_| TICK + TICK / 2);
    let words = text.measure(said, font);
    let start = left + (room.saturating_sub(ticked + words) / 2).cast_signed();
    if let Some(share) = tick {
        let (x, y) = (
            f64::from(start),
            f64::from(top) + f64::from(tall.saturating_sub(TICK)) / 2.0,
        );
        let at =
            |(across, down): (f64, f64)| (x + across * f64::from(TICK), y + down * f64::from(TICK));
        let points = [at((0.1, 0.55)), at((0.38, 0.82)), at((0.9, 0.2))];
        canvas.draw_stroke(&points, 2.0, style.color, share);
    }
    let after = start + ticked.cast_signed();
    text.draw(
        &mut canvas,
        said,
        (after, top, words, tall),
        style.color,
        font,
    );
    canvas
}

/// Paints the question whether to keep the screens as they stand now,
/// saying what that is, and how many seconds are `left` before they go
/// back as they were.
pub(crate) fn keep_screens(
    text: &mut Text,
    theme: &Theme,
    (what, left): (&str, u64),
    scale: Scale,
) -> Canvas {
    let seconds = if left == 1 { "second" } else { "seconds" };
    let lines = [
        what.to_owned(),
        "Keep the screens like this?".to_owned(),
        format!("Enter keeps them. Escape, or {left} {seconds}, puts them back."),
    ];
    let style = theme.style(Part::Notice, States::NONE);
    let edges = style.inset();
    // The first line is the heading.
    let parts = [Part::NoticeHeading, Part::Notice, Part::Notice];
    let fonts = parts.map(|part| (part, theme.font(part, States::NONE)));
    let widest = lines
        .iter()
        .zip(fonts)
        .map(|(line, (_, font))| text.measure(line, font));
    let width = widest.max().unwrap_or(0).saturating_add(edges.across());
    let rows = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    let height = NOTICE_ROW.saturating_mul(rows).saturating_add(edges.down());
    let whole = (0, 0, width, height);
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box(whole, &style);
    let (left_edge, mut top, room, _) = inset(whole, edges);
    for (line, (part, font)) in lines.iter().zip(fonts) {
        let ink = theme.style(part, States::NONE).color;
        text.draw(
            &mut canvas,
            line,
            (left_edge, top, room, NOTICE_ROW),
            ink,
            font,
        );
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
/// than `room`, where it is cut short; after `before`, as tall as the line.
pub(crate) fn offer_title(
    text: &mut Text,
    (theme, states): (&Theme, States),
    (title, before): (&str, Before<'_>),
    (room, height): (u32, u32),
    scale: Scale,
) -> Canvas {
    let style = theme.style(Part::Offer, states);
    let font = theme.font(Part::Offer, states);
    let aside = if matches!(before, Before::Nothing) {
        0
    } else {
        (height + BESIDE).min(room)
    };
    let width = (aside + text.measure(title, font)).clamp(1, room.max(1));
    let mut canvas = Canvas::scaled(width, height, scale);
    if let Before::Icon(image) = before {
        canvas.draw_image((0, 0, height.min(width), height), image);
    }
    let rest = (aside.cast_signed(), 0, width.saturating_sub(aside), height);
    text.draw(&mut canvas, title, rest, style.color, font);
    canvas
}

/// What goes before a title, in a square as tall as its line.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Before<'a> {
    /// Nothing: the title starts where it is.
    Nothing,
    /// An icon, painted with it.
    Icon(&'a Image),
    /// Room for an icon the renderer draws, as a program gave it.
    Room,
}

/// Paints an icon `side` units across, as a window's picture is shown by
/// its icon where it is not shown.
pub(crate) fn large_icon(image: &Image, side: u32, scale: Scale) -> Canvas {
    let mut canvas = Canvas::scaled(side, side, scale);
    canvas.draw_image((0, 0, side, side), image);
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
    for (item, shown) in layout.items.iter().zip(shown) {
        let (at, picked) = (item.place, item.picked);
        let states = States::NONE.when(State::Active, picked);
        let style = theme.style(Part::SwitcherItem, states);
        let left = at.origin.x - area.origin.x;
        let top = at.origin.y - area.origin.y;
        let place = (left, top, at.size.width, at.size.height);
        canvas.draw_box(place, &style);
        let places = Places::of(place, theme, states, picture, shown.icon.is_some());
        let font = theme.font(Part::SwitcherTitle, states);
        let ink = theme.style(Part::SwitcherTitle, states).color;
        text.draw(&mut canvas, &shown.title, places.title, ink, font);
        // A private window is blank where its picture would be, as in
        // pictures of the screen.
        if !shown.pictured {
            let (left, top, room, down) = inset(place, style.inset());
            let blank = theme.style(Part::Capture, States::NONE).ground();
            canvas.fill((left, top, room, picture.min(down)), blank);
        }
        if let Some(Some(image)) = &shown.icon {
            canvas.draw_image(places.icon, image);
            if !shown.pictured {
                canvas.draw_image(places.large, image);
            }
        }
    }
    canvas
}

/// Where the title of the window picked goes in the switcher's box under
/// the queue, `area`, and its icon beside it if it has one.
pub(crate) fn title_places(area: Area, theme: &Theme, iconic: bool) -> (Area, Area) {
    let style = theme.style(Part::Switcher, States::NONE);
    let (left, top, room, down) = inset(area, style.inset());
    let font = theme.font(Part::SwitcherTitle, States::NONE.with(State::Active));
    let tall = line_height(font.size.get()).min(down);
    let icon = (left, top, tall.min(room), tall);
    let aside = if iconic { (tall + BESIDE).min(room) } else { 0 };
    let title = (left.saturating_add_unsigned(aside), top, room - aside, tall);
    (icon, title)
}

/// Paints the switcher's box under the queue, `width` by `height`, with
/// the title of the window picked, and its icon beside it if it has one.
pub(crate) fn switcher_title(
    text: &mut Text,
    theme: &Theme,
    (width, height): (u32, u32),
    (title, icon): (&str, Option<&Image>),
    scale: Scale,
) -> Canvas {
    let mut canvas = Canvas::scaled(width, height, scale);
    let area = (0, 0, width, height);
    canvas.draw_box(area, &theme.style(Part::Switcher, States::NONE));
    let states = States::NONE.with(State::Active);
    let (beside, line) = title_places(area, theme, icon.is_some());
    let font = theme.font(Part::SwitcherTitle, states);
    let ink = theme.style(Part::SwitcherTitle, states).color;
    text.draw(&mut canvas, title, line, ink, font);
    if let Some(image) = icon {
        canvas.draw_image(beside, image);
    }
    canvas
}

/// Paints a window in the queue round its picture, `picture` as large as
/// it is in front: its border, its padding away from the picture, set
/// apart if it is `picked`; and, where its picture is not shown, a blank in
/// the picture's stead, in the colour of the `capture` part as a private
/// window's is in every picture, with its icon large in the middle.
pub(crate) fn switcher_window(
    theme: &Theme,
    (across, down): (u32, u32),
    picked: bool,
    shown: &Shown<Option<Image>>,
    scale: Scale,
) -> Canvas {
    let mut style = theme.style(
        Part::SwitcherWindow,
        States::NONE.when(State::Active, picked),
    );
    style.background = Color::rgba(0, 0, 0, 0);
    style.gradient = None;
    let edges = style.inset();
    let (width, height) = (across + edges.across(), down + edges.down());
    let mut canvas = Canvas::scaled(width, height, scale);
    canvas.draw_box((0, 0, width, height), &style);
    if !shown.pictured {
        let picture = (
            edges.left.cast_signed(),
            edges.top.cast_signed(),
            across,
            down,
        );
        canvas.fill(picture, theme.style(Part::Capture, States::NONE).ground());
    }
    let side = LARGE_ICON.min(across).min(down);
    let large = (
        (edges.left + (across - side) / 2).cast_signed(),
        (edges.top + (down - side) / 2).cast_signed(),
        side,
        side,
    );
    if let Some(Some(image)) = shown.icon.as_ref().filter(|_| !shown.pictured) {
        canvas.draw_image(large, image);
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

#[cfg(test)]
mod tests {
    use omorfos_core::Metrics as FrameMetrics;
    use omorfos_core::Sign;
    use omorfos_core::geometry::{Point, Rect, Size};

    use super::*;

    /// The colours of a theme, by what the built-in theme uses them for.
    struct Palette {
        accent: Color,
        /// A row under the pointer.
        raised: Color,
        /// What is wrong.
        danger: Color,
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
            accent: theme.style(Part::Picker, States::NONE).border.top.color,
            raised: theme
                .style(Part::MenuItem, States::NONE.with(State::Hovered))
                .background,
            danger: theme.style(Part::NoticeHeading, States::NONE).color,
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
        let tools = (text, &mut WindowIcons::default());
        super::menu_panel(tools, theme, menu, &menu.panels()[0], &looks[0], Scale::ONE)
    }

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
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
                mark: Mark::Blank,
            },
            Item::Separator,
            Item::Submenu {
                label: "Session".to_owned(),
                items: Vec::new(),
                mark: Mark::Sign(Sign::Quit),
            },
        ];
        let metrics = MenuMetrics {
            width: 200,
            row: 28,
            separator: 9,
            above: 4,
            below: 4,
            overlap: 0,
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
        // A sign is drawn 16 across, after the border of 1 and the row's
        // padding of 8, and the label 8 after it, in the middle of its row.
        let background = argb(colors.focused.titlebar);
        let signed = |rows: std::ops::Range<u32>, columns: std::ops::Range<u32>| {
            let place = rows.flat_map(|y| columns.clone().map(move |x| (x, y)));
            place.filter(|&(x, y)| plain.at(x, y) != background).count()
        };
        assert!(signed(41..69, 9..25) > 10, "the submenu row's sign");
        // Below the menu's rounded corner.
        assert_eq!(
            signed(12..32, 1..33),
            0,
            "none for the other, nor its label"
        );
        assert_eq!(
            signed(41..69, 25..33),
            0,
            "room between the sign and the label"
        );

        menu.hover(Point::new(350, 215));
        let lit = menu_panel(&mut text, &theme, &menu);
        assert_eq!(
            lit.at(190, 10),
            argb(colors.raised),
            "the hovered row is highlighted"
        );
        assert_eq!(
            lit.at(190, 50),
            argb(colors.focused.titlebar),
            "the others are not"
        );
    }

    #[test]
    fn the_sign_of_what_is_on_is_pressed_in_and_that_of_what_is_off_is_not() {
        use omorfos_core::MenuMetrics;

        let theme = Theme::default();
        let colors = palette(&theme);
        let mut text = Text::new();
        let entry = |label: &str, on| Item::Entry {
            label: label.to_owned(),
            action: (),
            mark: Mark::Toggle(Sign::Pin, on),
        };
        let metrics = MenuMetrics {
            width: 200,
            row: 28,
            separator: 9,
            above: 4,
            below: 4,
            overlap: 0,
        };
        let area = Rect::new(Point::new(0, 0), Size::new(1000, 800));
        let items = vec![entry("Keep above", false), entry("Keep above", true)];
        let menu = Menu::open(items, Point::new(300, 200), area, metrics);

        // Rows: 4..32 and 32..60, each with its sign at 9..25 across, after
        // the border and the row's padding, and 6 below the row's top; and
        // round the sign the theme's 3 of padding, at 6..28.
        let panel = menu_panel(&mut text, &theme, &menu);
        let background = argb(colors.focused.titlebar);
        assert_eq!(panel.at(7, 18), background, "off, it is not pressed in");
        let tint = panel.at(7, 46);
        assert_ne!(tint, background, "on, it is");
        assert_eq!(panel.at(27, 46), tint, "all round the sign");
        assert_eq!(panel.at(29, 46), background, "and no further");
        // The pin's lines are in the accent, cyan, while it is on, and in the
        // colour of the text, nearly white, while it is off: bright, and
        // with as much red as the rest or almost none.
        let bright = |rows: std::ops::Range<u32>| {
            let place = (9..25).flat_map(move |x| rows.clone().map(move |y| (x, y)));
            let pixels = place.map(|(x, y)| panel.at(x, y));
            pixels
                .filter(|pixel| pixel >> 8 & 0xff > 150)
                .collect::<Vec<u32>>()
        };
        let (off, on) = (bright(10..26), bright(38..54));
        assert!(!off.is_empty() && !on.is_empty(), "both are drawn");
        assert!(
            off.iter().all(|pixel| pixel >> 16 & 0xff > 150),
            "off: {off:x?}"
        );
        assert!(
            on.iter().all(|pixel| pixel >> 16 & 0xff < 100),
            "on: {on:x?}"
        );
    }

    #[test]
    fn the_first_and_last_rows_highlighted_keep_to_the_menus_rounded_corners() {
        use omorfos_core::MenuMetrics;

        let red = Color::rgb(255, 0, 0);
        let theme =
            Theme::parse("menu { border-radius: 16px; } menu-item:hover { background: #ff0000; }")
                .unwrap();
        let mut text = Text::new();
        let entry = |label: &str| Item::Entry {
            label: label.to_owned(),
            action: (),
            mark: Mark::Blank,
        };
        let metrics = MenuMetrics {
            width: 200,
            row: 28,
            separator: 9,
            above: 4,
            below: 4,
            overlap: 0,
        };
        let area = Rect::new(Point::new(0, 0), Size::new(1000, 800));
        let items = vec![entry("Terminal"), entry("Files")];
        let mut menu = Menu::open(items, Point::new(300, 200), area, metrics);

        // Rows: Terminal at 4..32, Files at 32..60, in a menu 64 high.
        menu.hover(Point::new(350, 215));
        let first = menu_panel(&mut text, &theme, &menu);
        assert_eq!(first.at(100, 10), argb(red), "the row is highlighted");
        assert_eq!(first.at(2, 5), 0, "but not beyond the top left corner");
        assert_eq!(first.at(197, 5), 0, "nor the top right one");
        menu.hover(Point::new(350, 245));
        let last = menu_panel(&mut text, &theme, &menu);
        assert_eq!(last.at(100, 50), argb(red), "the row is highlighted");
        assert_eq!(last.at(2, 58), 0, "but not beyond the bottom left corner");
        assert_eq!(last.at(197, 58), 0, "nor the bottom right one");
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
        assert_eq!((canvas.width(), canvas.height()), (400, 92));
        assert_eq!(
            canvas.at(0, 40),
            argb(colors.danger),
            "outlined in the colour of danger"
        );
        assert_eq!(canvas.at(395, 40), argb(colors.focused.titlebar));
        let row = |top: u32| (10..300).flat_map(move |x| (top..top + 22).map(move |y| (x, y)));
        let ink = |top: u32| {
            row(top)
                .filter(|(x, y)| canvas.at(*x, *y) != argb(colors.focused.titlebar))
                .count()
        };
        assert!(
            ink(13) > 30 && ink(35) > 30 && ink(57) > 30,
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
        assert_eq!(exactly.height(), 7 * 22 + 26, "a heading and six problems");
        assert_eq!(
            capped.height(),
            8 * 22 + 26,
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

    #[test]
    fn a_titlebar_that_can_be_seen_through_is_so_within_its_outline() {
        let theme =
            Theme::parse("titlebar:focus { background: rgba(255, 255, 255, 0.5); }").unwrap();
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
        let border = argb(palette(&theme).focused.border);
        assert_eq!(canvas.at(200, 0), border, "the outline along the top");
        assert_eq!(canvas.at(0, 20), border, "and down the side");
        assert_eq!(
            canvas.at(300, 14),
            0x8080_8080,
            "white at half, and nothing under it"
        );
    }

    /// The colours the built-in theme gives the buttons of a frame with
    /// focus, from the right, and the one it gives them all without.
    const BUTTON_COLORS: [u32; 3] = [0xfff8_616d, 0xffa3_e635, 0xfffd_e047];
    const BUTTON_PLAIN: u32 = 0xff2b_3a6b;

    /// The middle of each button, from the right, under the built-in
    /// theme: twelve across with room around them, the outermost further
    /// in from the edge than the others are from each other.
    const BUTTON_MIDDLES: [u32; 3] = [382, 362, 342];

    /// A frame with room for the buttons the built-in theme has.
    fn buttoned() -> Decoration {
        let outer = Rect::new(Point::new(300, 200), Size::new(400, 300));
        let metrics = FrameMetrics {
            buttons: [28, 20, 20],
            ..METRICS
        };
        Decoration::new(outer, metrics, 1)
    }

    #[test]
    fn a_titlebar_has_three_round_buttons_at_its_right_coloured_when_it_has_focus() {
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
        for between in [350, 370] {
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
        assert_eq!(count(&close, 382), 12);
        assert_eq!(close.at(379, 11), ink, "from one corner");
        assert_eq!(close.at(384, 11), ink, "and from the other");
        assert_eq!(count(&close, 362), 0, "and on no other button");
        let maximize = hovering(Some(Button::Maximize), true);
        assert_eq!(count(&maximize, 362), 11);
        let minimize = hovering(Some(Button::Minimize), true);
        assert_eq!(count(&minimize, 342), 6);

        // Pointed at, a button has its colour whether or not its frame
        // has focus.
        let unfocused = hovering(Some(Button::Close), false);
        assert_eq!(unfocused.at(377, 14), BUTTON_COLORS[0]);
        assert_eq!(unfocused.at(362, 14), BUTTON_PLAIN);
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
