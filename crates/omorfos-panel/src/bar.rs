//! The bar: what is on it, where, how it is drawn, and what a click on it
//! asks for.

use omorfos_config::{Part, States, Theme};
use omorfos_ipc::{Request, State, Window, Workspace};
use omorfos_ui::{Area, Canvas, Motion, Scale, Text, inset};
use serde_json::json;

/// What a button stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// A workspace of the bar's output, by its number.
    Workspace(u32),
    /// A window, by its identifier.
    Window(u64),
    /// Nothing: the clock is only read.
    Nothing,
}

/// Something on the bar: a workspace, a window or the clock.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Item {
    /// The box it is drawn as, which is also where it is clicked.
    area: Area,
    label: String,
    part: Part,
    /// Whether it is the workspace being shown or the window with focus.
    states: States,
    target: Target,
}

/// The bar of one output: a button for each of its workspaces, then one for
/// each window on the workspace it is showing, and the time at the end.
///
/// Each of these, and the bar itself, is a box as the theme describes it,
/// with its margin, border and padding.
///
/// Two bars are equal when they would be drawn alike under one theme, so a
/// bar made from news that changed nothing on it need not be drawn again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bar {
    width: u32,
    height: u32,
    items: Vec<Item>,
}

/// Lays boxes out along the inside of the bar.
struct Row<'a> {
    theme: &'a Theme,
    text: &'a mut Text,
    /// The top of the row and how tall it is.
    top: i32,
    height: u32,
}

impl Row<'_> {
    /// How wide a part's box is to hold `label`, before any limit the theme
    /// puts on it.
    fn needed(&mut self, part: Part, states: States, label: &str) -> u32 {
        let style = self.theme.style(part, states);
        let wide = self.text.measure(label, self.theme.font(part, states));
        wide.saturating_add(style.inset().across())
    }

    /// The box of a part whose margin begins at `left`, `width` wide inside
    /// its margin, and where the margin of the next one begins.
    fn place(&self, part: Part, states: States, left: i32, width: u32) -> (Area, i32) {
        let margin = self.theme.style(part, states).margin;
        let area = (
            left.saturating_add_unsigned(margin.left),
            self.top.saturating_add_unsigned(margin.top),
            width,
            self.height.saturating_sub(margin.down()),
        );
        let next = area.0.saturating_add_unsigned(width);
        (area, next.saturating_add_unsigned(margin.right))
    }
}

impl Bar {
    /// How tall a bar is under `theme`, with the room its margin keeps
    /// clear above and below it.
    #[must_use]
    pub fn thickness(theme: &Theme) -> u32 {
        let panel = theme.style(Part::Panel, States::NONE);
        panel.height.saturating_add(panel.margin.down())
    }

    /// The bar for the output called `output`, `width` units of layout
    /// wide, showing what `state` says of that output, and `time` as the
    /// time. The time is whatever is to be read there, such as `14:05`; a
    /// bar told none has no clock.
    ///
    /// A workspace's button is as wide as its name needs, and the clock as
    /// wide as the time. The windows share what room is left. The theme
    /// may hold any of them to a narrowest or a widest.
    #[must_use]
    pub fn new(
        state: &State,
        output: &str,
        width: u32,
        time: &str,
        theme: &Theme,
        text: &mut Text,
    ) -> Self {
        let height = Self::thickness(theme);
        let panel = theme.style(Part::Panel, States::NONE);
        let inside = inset(inset((0, 0, width, height), panel.margin), panel.inset());
        let mut row = Row {
            theme,
            text,
            top: inside.1,
            height: inside.3,
        };
        let mut items = Vec::new();
        let mut left = inside.0;
        let lit = |active: bool| States::NONE.when(omorfos_config::State::Active, active);

        let workspaces = state.workspaces.iter();
        let workspaces = workspaces.filter(|workspace| workspace.output == output);
        for workspace in workspaces.clone() {
            let (part, states) = (Part::Workspace, lit(workspace.active));
            let label = workspace_label(workspace);
            let wide = theme
                .style(part, states)
                .fit(row.needed(part, states, &label));
            let (area, next) = row.place(part, states, left, wide);
            left = next;
            items.push(Item {
                area,
                label,
                part,
                states,
                target: Target::Workspace(workspace.number),
            });
        }

        // The clock stands at the far end, and the windows stop short of it.
        let mut end = inside.0.saturating_add_unsigned(inside.2);
        let clock = (!time.is_empty()).then(|| {
            let (part, states) = (Part::Clock, States::NONE);
            let style = theme.style(part, states);
            let wide = style.fit(row.needed(part, states, time));
            let outer = wide.saturating_add(style.margin.across());
            end = end.saturating_sub_unsigned(outer);
            let (area, _) = row.place(part, states, end, wide);
            Item {
                area,
                label: time.to_owned(),
                part,
                states,
                target: Target::Nothing,
            }
        });

        let mut workspaces = workspaces;
        let shown = workspaces.find(|workspace| workspace.active);
        let shown = shown.map(|workspace| workspace.number);
        let windows = state.windows.iter();
        let windows: Vec<&Window> = windows
            .filter(|window| window.output == output && Some(window.workspace) == shown)
            .collect();
        let count = u32::try_from(windows.len()).unwrap_or(u32::MAX);
        let room = u32::try_from(end.saturating_sub(left)).unwrap_or(0);
        let share = room.checked_div(count).unwrap_or(0);
        for window in windows {
            let (part, states) = (Part::Task, lit(window.focused));
            let style = theme.style(part, states);
            let wide = style.fit(share.saturating_sub(style.margin.across()));
            let (area, next) = row.place(part, states, left, wide);
            left = next;
            items.push(Item {
                area,
                label: window_label(window),
                part,
                states,
                target: Target::Window(window.id),
            });
        }
        items.extend(clock);

        Self {
            width,
            height,
            items,
        }
    }

    /// How tall the bar is, in units of layout, with its margin.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// What a click `x` units from the bar's left edge asks of the
    /// compositor: to show the workspace or to bring up the window whose
    /// button is there. Nothing where there is no button.
    #[must_use]
    pub fn click(&self, x: i32) -> Option<Request> {
        let mut items = self.items.iter();
        let item = items.find(|item| {
            let (left, _, width, _) = item.area;
            let along = u32::try_from(x.saturating_sub(left)).ok();
            along.is_some_and(|along| along < width)
        })?;
        match item.target {
            // The compositor switches workspace on the screen the pointer
            // is on, which for a click on this bar is this bar's screen.
            Target::Workspace(number) => Some(Request::Perform {
                action: json!({ "workspace": number }),
            }),
            Target::Window(window) => Some(Request::Activate { window }),
            Target::Nothing => None,
        }
    }

    /// Draws the bar, with `scale` pixels to a unit of layout. A button
    /// whose workspace has just been shown or whose window has just been
    /// given focus, or has just stopped being, is as far on its way to its
    /// new colours as `motion` has it.
    #[must_use]
    pub fn paint(
        &self,
        text: &mut Text,
        theme: &Theme,
        scale: Scale,
        motion: &mut Motion,
    ) -> Canvas {
        let panel = theme.style(Part::Panel, States::NONE);
        let mut canvas = Canvas::scaled(self.width, self.height, scale);
        let whole = (0, 0, self.width, self.height);
        canvas.draw_box(inset(whole, panel.margin), &panel);
        for item in &self.items {
            let key = match item.target {
                Target::Workspace(number) => u64::from(number),
                Target::Window(id) => id,
                Target::Nothing => 0,
            };
            let style = motion.style(theme, item.part, key, item.states);
            canvas.draw_box(item.area, &style);
            let label = inset(item.area, style.inset());
            let font = theme.font(item.part, item.states);
            text.draw(&mut canvas, &item.label, label, style.color, font);
        }
        canvas
    }
}

/// What a workspace's button says: its name, or its number if it has none.
fn workspace_label(workspace: &Workspace) -> String {
    if workspace.name.is_empty() {
        workspace.number.to_string()
    } else {
        workspace.name.clone()
    }
}

/// What a window's button says: its title, or the application's name while
/// it has none, in brackets while the window is minimized.
fn window_label(window: &Window) -> String {
    let name = if window.title.is_empty() {
        &window.app_id
    } else {
        &window.title
    };
    if window.minimized {
        format!("({name})")
    } else {
        name.clone()
    }
}

#[cfg(test)]
mod tests {
    use omorfos_config::Color;

    /// The room the default theme leaves either side of a label.
    const PADDING: u32 = 10;
    /// The widest the default theme makes a window's button.
    const WIDEST_TASK: u32 = 240;

    const ACTIVE: omorfos_config::State = omorfos_config::State::Active;

    /// How thick the default theme makes the line under a lit button.
    const LINE: u32 = 2;

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
        titlebar: Color,
    }

    fn palette(theme: &Theme) -> Palette {
        let tone = |states| Tone {
            titlebar: theme.style(Part::Titlebar, states).background,
        };
        Palette {
            accent: theme.style(Part::NoticeHeading, States::NONE).color,
            focused: tone(States::NONE.with(omorfos_config::State::Focused)),
            unfocused: tone(States::NONE),
        }
    }

    const WIDTH: u32 = 800;

    fn workspace(output: &str, number: u32, name: &str, active: bool) -> Workspace {
        Workspace {
            output: output.to_owned(),
            number,
            name: name.to_owned(),
            active,
        }
    }

    fn window(output: &str, workspace: u32, id: u64, title: &str) -> Window {
        Window {
            id,
            app_id: "app".to_owned(),
            title: title.to_owned(),
            output: output.to_owned(),
            workspace,
            focused: false,
            shown: true,
            minimized: false,
        }
    }

    /// Two screens: the first shows its second workspace, which has two
    /// windows on it, the second of them focused.
    fn state() -> State {
        State {
            workspaces: vec![
                workspace("left", 1, "web", false),
                workspace("left", 2, "code", true),
                workspace("right", 1, "web", true),
                workspace("right", 2, "code", false),
            ],
            windows: vec![
                window("left", 1, 1, "elsewhere"),
                window("left", 2, 2, "editor"),
                Window {
                    focused: true,
                    ..window("left", 2, 3, "terminal")
                },
                window("right", 1, 4, "player"),
            ],
            problems: Vec::new(),
            events: Vec::new(),
        }
    }

    fn bar(state: &State, output: &str) -> Bar {
        timed(state, output, "")
    }

    fn timed(state: &State, output: &str, time: &str) -> Bar {
        let theme = Theme::default();
        Bar::new(state, output, WIDTH, time, &theme, &mut Text::new())
    }

    fn workspace_request(number: u32) -> Request {
        Request::Perform {
            action: json!({ "workspace": number }),
        }
    }

    /// What a click asks for at each place along the bar from the left,
    /// with where each stretch begins. Stretches with no button are left
    /// out.
    fn stretches(bar: &Bar) -> Vec<(u32, Request)> {
        let mut stretches: Vec<(u32, Request)> = Vec::new();
        for x in 0..WIDTH {
            let Some(request) = bar.click(x.cast_signed()) else {
                continue;
            };
            if stretches.last().map(|(_, last)| last) != Some(&request) {
                stretches.push((x, request));
            }
        }
        stretches
    }

    fn requests(bar: &Bar) -> Vec<Request> {
        let stretches = stretches(bar);
        stretches.into_iter().map(|(_, request)| request).collect()
    }

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    /// The number of pixels in some columns and rows that are `color`.
    fn count(
        canvas: &Canvas,
        columns: std::ops::Range<u32>,
        rows: std::ops::Range<u32>,
        color: Color,
    ) -> usize {
        rows.flat_map(|y| columns.clone().map(move |x| (x, y)))
            .filter(|(x, y)| canvas.at(*x, *y) == argb(color))
            .count()
    }

    #[test]
    fn a_bar_has_its_screens_workspaces_and_then_the_windows_on_the_one_shown() {
        assert_eq!(
            requests(&bar(&state(), "left")),
            [
                workspace_request(1),
                workspace_request(2),
                Request::Activate { window: 2 },
                Request::Activate { window: 3 },
            ]
        );
        assert_eq!(
            requests(&bar(&state(), "right")),
            [
                workspace_request(1),
                workspace_request(2),
                Request::Activate { window: 4 },
            ]
        );
    }

    #[test]
    fn a_bar_for_a_screen_nothing_is_known_about_is_empty() {
        let bar = bar(&state(), "gone");
        assert_eq!(requests(&bar), []);
        // It is still a bar: as tall as the theme says and drawn all over.
        let theme = Theme::default();
        assert_eq!(bar.height(), theme.style(Part::Panel, States::NONE).height);
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let background = palette(&theme).unfocused.titlebar;
        let all = usize::try_from(WIDTH * bar.height()).unwrap();
        assert_eq!(count(&canvas, 0..WIDTH, 0..bar.height(), background), all);
    }

    #[test]
    fn a_click_beside_the_buttons_asks_for_nothing() {
        let bar = bar(&state(), "left");
        assert_eq!(bar.click(-1), None);
        assert_eq!(bar.click(WIDTH.cast_signed() - 1), None, "past the last");
        assert_eq!(bar.click(i32::MAX), None);
    }

    #[test]
    fn a_workspace_with_a_longer_name_has_a_wider_button() {
        let mut state = state();
        state.workspaces[1].name = "a much longer name than that".to_owned();
        let stretches = stretches(&bar(&state, "left"));
        let first = stretches[1].0 - stretches[0].0;
        let second = stretches[2].0 - stretches[1].0;
        assert!(second > first * 2, "{second} against {first}");
    }

    #[test]
    fn a_workspace_without_a_name_goes_by_its_number_and_is_still_easy_to_hit() {
        let mut state = state();
        state.workspaces[0].name = String::new();
        let bar = bar(&state, "left");
        let stretches = stretches(&bar);
        assert_eq!(stretches[0], (0, workspace_request(1)));
        assert!(stretches[1].0 >= bar.height());
        assert_eq!(workspace_label(&state.workspaces[0]), "1");
        assert_eq!(workspace_label(&state.workspaces[1]), "code");
    }

    #[test]
    fn windows_share_the_room_the_workspaces_leave() {
        let mut state = state();
        let lone = bar(&state, "right");
        let lone = stretches(&lone);
        // One window does not spread over the whole bar.
        assert_eq!(lone.len(), 3);
        let end = (0..WIDTH)
            .rev()
            .find(|x| bar(&state, "right").click(x.cast_signed()).is_some());
        assert_eq!(end.unwrap() + 1 - lone[2].0, WIDEST_TASK);

        // Many windows each get less, and none is pushed off the end.
        for id in 10..40 {
            state.windows.push(window("right", 1, id, "another"));
        }
        let crowded = stretches(&bar(&state, "right"));
        assert_eq!(crowded.len(), 2 + 31);
        let widths: Vec<u32> = crowded[2..]
            .windows(2)
            .map(|pair| pair[1].0 - pair[0].0)
            .collect();
        assert!(
            widths
                .iter()
                .all(|width| *width == widths[0] && *width < WIDEST_TASK)
        );
    }

    #[test]
    fn a_bar_too_narrow_for_its_workspaces_has_no_room_for_windows() {
        let narrow = Bar::new(
            &state(),
            "left",
            10,
            "",
            &Theme::default(),
            &mut Text::new(),
        );
        assert_eq!(narrow.click(0), Some(workspace_request(1)));
        let asked = (0..2000).filter_map(|x| narrow.click(x));
        assert!(
            asked
                .into_iter()
                .all(|request| request != Request::Activate { window: 2 })
        );
        // Drawing it stays within the bar.
        let canvas = narrow.paint(
            &mut Text::new(),
            &Theme::default(),
            Scale::ONE,
            &mut Motion::new(),
        );
        assert_eq!(canvas.width(), 10);
    }

    #[test]
    fn what_is_shown_and_what_has_focus_are_lit_and_underlined() {
        let theme = Theme::default();
        let colors = palette(&theme);
        let bar = bar(&state(), "left");
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let stretches = stretches(&bar);
        let starts: Vec<u32> = stretches.iter().map(|(start, _)| *start).collect();
        let end = starts[3] + (starts[3] - starts[2]);
        let height = bar.height();
        let underline = height - LINE..height;

        // The second workspace and the second window.
        for lit in [starts[1]..starts[2], starts[3]..end] {
            let wide = usize::try_from(lit.end - lit.start).unwrap();
            assert_eq!(
                count(&canvas, lit.clone(), underline.clone(), colors.accent),
                wide * 2
            );
            assert!(
                count(
                    &canvas,
                    lit.clone(),
                    0..height - LINE,
                    colors.focused.titlebar
                ) > 0
            );
            assert_eq!(count(&canvas, lit, 0..height, colors.unfocused.titlebar), 0);
        }
        // The first workspace and the first window.
        for plain in [starts[0]..starts[1], starts[2]..starts[3]] {
            assert_eq!(count(&canvas, plain.clone(), 0..height, colors.accent), 0);
            assert_eq!(
                count(&canvas, plain.clone(), 0..height, colors.focused.titlebar),
                0
            );
            // Its label is there, and keeps clear of the button's edges.
            let wide = usize::try_from(plain.end - plain.start).unwrap();
            let tall = usize::try_from(height).unwrap();
            let background = count(&canvas, plain.clone(), 0..height, colors.unfocused.titlebar);
            assert!(background < wide * tall, "nothing is written on it");
            let edge = plain.start..plain.start + PADDING;
            assert_eq!(
                count(&canvas, edge, 0..height, colors.unfocused.titlebar),
                usize::try_from(PADDING).unwrap() * tall
            );
        }
    }

    #[test]
    fn a_bar_is_drawn_with_as_many_pixels_as_the_screen_has() {
        let bar = bar(&state(), "left");
        let canvas = bar.paint(
            &mut Text::new(),
            &Theme::default(),
            Scale::new(2.0),
            &mut Motion::new(),
        );
        assert_eq!(
            (canvas.width(), canvas.height()),
            (WIDTH * 2, bar.height() * 2)
        );
    }

    #[test]
    fn a_window_is_called_by_its_title_or_else_its_application() {
        let titled = window("left", 1, 1, "notes");
        assert_eq!(window_label(&titled), "notes");
        let untitled = window("left", 1, 1, "");
        assert_eq!(window_label(&untitled), "app");
        let minimized = Window {
            minimized: true,
            ..titled
        };
        assert_eq!(window_label(&minimized), "(notes)");
    }

    #[test]
    fn the_time_is_at_the_right_hand_end_and_is_not_a_button() {
        let theme = Theme::default();
        let background = palette(&theme).unfocused.titlebar;
        let bar = timed(&state(), "left", "14:05");
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let height = bar.height();
        let blank = |columns: std::ops::Range<u32>| {
            let all = usize::try_from((columns.end - columns.start) * height).unwrap();
            count(&canvas, columns, 0..height, background) == all
        };

        // Something is written towards the end, clear of the edge, and
        // nothing between it and the last window.
        assert!(!blank(WIDTH - 100..WIDTH - PADDING));
        assert!(blank(WIDTH - PADDING..WIDTH));
        let last = (0..WIDTH)
            .rev()
            .find(|x| bar.click(x.cast_signed()).is_some());
        assert!(blank(last.unwrap() + 1..WIDTH - 100));
        // A longer time takes more room.
        let longer = timed(&state(), "left", "14:05:09 on a Saturday");
        let canvas = longer.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let all = usize::try_from(40 * height).unwrap();
        assert!(count(&canvas, WIDTH - 140..WIDTH - 100, 0..height, background) < all);

        // A bar with no time has nothing there.
        let canvas = self::bar(&state(), "left").paint(
            &mut Text::new(),
            &theme,
            Scale::ONE,
            &mut Motion::new(),
        );
        let all = usize::try_from(100 * height).unwrap();
        assert_eq!(
            count(&canvas, WIDTH - 100..WIDTH, 0..height, background),
            all
        );
    }

    #[test]
    fn windows_keep_clear_of_the_time() {
        let mut state = state();
        for id in 10..40 {
            state.windows.push(window("right", 1, id, "another"));
        }
        let without = bar(&state, "right");
        let with = timed(&state, "right", "14:05");
        let end = |bar: &Bar| {
            (0..WIDTH)
                .rev()
                .find(|x| bar.click(x.cast_signed()).is_some())
        };
        let (without, with) = (end(&without).unwrap(), end(&with).unwrap());
        assert!(without > WIDTH - 32, "they fill the bar: {without}");
        assert!(
            with < WIDTH - PADDING * 2,
            "but stop short of the time: {with}"
        );
        assert_eq!(requests(&timed(&state, "right", "14:05")).len(), 2 + 31);
    }

    #[test]
    fn bars_showing_different_times_are_not_equal() {
        let state = state();
        assert_eq!(
            timed(&state, "left", "14:05"),
            timed(&state, "left", "14:05")
        );
        assert_ne!(
            timed(&state, "left", "14:05"),
            timed(&state, "left", "14:06")
        );
    }

    /// The first screen's bar under the built-in theme with `stylesheet`
    /// laid over it.
    fn styled(stylesheet: &str) -> (Bar, Theme) {
        let theme = Theme::parse(stylesheet).unwrap();
        let bar = Bar::new(&state(), "left", WIDTH, "14:05", &theme, &mut Text::new());
        (bar, theme)
    }

    /// Where each stretch of the bar that asks for something begins and
    /// ends.
    fn spans(bar: &Bar) -> Vec<(u32, u32)> {
        let asked = |x: &u32| bar.click(x.cast_signed());
        let starts = stretches(bar).into_iter().map(|(start, _)| start);
        let spans = starts.map(|start| {
            let request = asked(&start);
            let end = (start..WIDTH).find(|x| asked(x) != request);
            (start, end.unwrap_or(WIDTH))
        });
        spans.collect()
    }

    #[test]
    fn a_margin_moves_the_bar_in_from_the_edges_of_the_screen() {
        let (bar, theme) = styled("panel { margin: 4px 6px 8px 10px; }");
        assert_eq!(Bar::thickness(&theme), 28 + 4 + 8);
        assert_eq!(bar.height(), 40);
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let background = argb(theme.style(Part::Panel, States::NONE).background);
        // Nothing is drawn in the margin, so the screen shows through.
        for (x, y) in [(400, 3), (400, 32), (9, 20), (WIDTH - 6, 20), (0, 0)] {
            assert_eq!(canvas.at(x, y), 0, "({x}, {y})");
        }
        assert_eq!(canvas.at(WIDTH - 7, 4), background);
        assert_eq!(canvas.at(WIDTH - 7, 31), background);
        // And the buttons begin where the bar does.
        assert_eq!(spans(&bar)[0].0, 10);
    }

    #[test]
    fn room_kept_around_a_button_is_not_part_of_it() {
        let (plain, _) = styled("");
        let (bar, theme) = styled("workspace { margin: 3px 5px; }");
        let (plain, spaced) = (spans(&plain), spans(&bar));
        assert_eq!(spaced[0].0, 5);
        assert_eq!(
            spaced[1].0 - spaced[0].1,
            10,
            "a margin each side of the gap"
        );
        let width = |span: (u32, u32)| span.1 - span.0;
        assert_eq!(width(spaced[0]), width(plain[0]), "the button is as wide");
        // It is drawn shorter, with the bar showing above and below it.
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let lit = argb(
            theme
                .style(Part::Workspace, States::NONE.with(ACTIVE))
                .background,
        );
        let panel = argb(theme.style(Part::Panel, States::NONE).background);
        let middle = spaced[1].0 + 2;
        assert_eq!(canvas.at(middle, 2), panel);
        assert_eq!(canvas.at(middle, 3), lit);
        assert_eq!(canvas.at(middle, 25), panel);
    }

    #[test]
    fn padding_makes_a_button_wider_than_its_label() {
        let (plain, _) = styled("");
        let (padded, _) = styled("workspace { padding: 0 30px; }");
        let width = |bar: &Bar, index: usize| {
            let span = spans(bar)[index];
            span.1 - span.0
        };
        // Twenty a side more than the ten the theme has.
        assert_eq!(width(&padded, 1), width(&plain, 1) + 40);
    }

    #[test]
    fn a_button_is_held_to_its_narrowest_and_its_widest() {
        let (bar, _) = styled("workspace { min-width: 100px; } task { max-width: 50px; }");
        let widths: Vec<u32> = spans(&bar).iter().map(|span| span.1 - span.0).collect();
        assert_eq!(widths, [100, 100, 50, 50]);
        // With no limit, the windows share all the room there is.
        let (bar, _) = styled("task { max-width: 0; }");
        let spans = spans(&bar);
        assert!(spans[3].1 - spans[3].0 > 240, "{spans:?}");
    }

    #[test]
    fn a_bar_and_its_buttons_can_have_rounded_corners_and_borders() {
        let (bar, theme) = styled(
            "panel { border-radius: 12px; border: 2px solid #ff0000; }\n\
             workspace { border-radius: 6px; margin: 2px; }",
        );
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        assert_eq!(canvas.at(0, 0), 0, "the corner is cut away");
        assert_eq!(canvas.at(WIDTH - 1, 27), 0);
        assert_eq!(canvas.at(400, 0), argb(Color::rgb(255, 0, 0)));
        assert_eq!(canvas.at(400, 27), argb(Color::rgb(255, 0, 0)));
        let panel = argb(theme.style(Part::Panel, States::NONE).background);
        assert_eq!(
            canvas.at(WIDTH - 150, 14),
            panel,
            "between the windows and the clock"
        );
        // The buttons are inside the bar's border.
        assert_eq!(spans(&bar)[0].0, 2 + 2);
    }

    #[test]
    fn the_clock_is_a_box_like_any_other() {
        let (bar, theme) = styled("clock { background: #00ff00; margin: 0 7px 0 0; }");
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE, &mut Motion::new());
        let green = argb(Color::rgb(0, 255, 0));
        assert_eq!(canvas.at(WIDTH - 8, 1), green);
        assert_ne!(canvas.at(WIDTH - 7, 1), green, "its margin is the bar's");
        // It is read, not clicked.
        assert_eq!(bar.click((WIDTH - 20).cast_signed()), None);
    }

    #[test]
    fn bars_that_would_look_alike_are_equal() {
        let mut state = state();
        let before = bar(&state, "left");
        // Something happens on the other screen.
        state.windows[3].title = "another film".to_owned();
        assert_eq!(bar(&state, "left"), before);
        assert_ne!(bar(&state, "right"), before);
        // Something happens on this one.
        state.windows[1].title = "another file".to_owned();
        assert_ne!(bar(&state, "left"), before);
    }
}
