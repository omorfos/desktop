//! The bar: what is on it, where, how it is drawn, and what a click on it
//! asks for.

use omorfos_config::{Part, States, Theme};
use omorfos_ipc::{Request, State, Window, Workspace};
use omorfos_ui::{Canvas, Scale, Text};
use serde_json::json;

/// The room left and right of a label, in units of layout.
const PADDING: u32 = 10;
/// The widest a window's button gets.
const WIDEST_TASK: u32 = 240;

/// What a button stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// A workspace of the bar's output, by its number.
    Workspace(u32),
    /// A window, by its identifier.
    Window(u64),
}

/// A button on the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Button {
    left: u32,
    width: u32,
    label: String,
    /// Whether it is the workspace being shown or the window with focus.
    lit: bool,
    target: Target,
}

/// The time, at the right-hand end of the bar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Clock {
    left: u32,
    width: u32,
    time: String,
}

/// The bar of one output: a button for each of its workspaces, then one for
/// each window on the workspace it is showing, and the time at the end.
///
/// Two bars are equal when they would be drawn alike under one theme, so a
/// bar made from news that changed nothing on it need not be drawn again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bar {
    width: u32,
    height: u32,
    buttons: Vec<Button>,
    clock: Clock,
}

impl Bar {
    /// The bar for the output called `output`, `width` units of layout
    /// wide, showing what `state` says of that output, and `time` as the
    /// time. The time is whatever is to be read there, such as `14:05`; a
    /// bar told none has no clock.
    ///
    /// A workspace's button is as wide as its name needs, and the clock as
    /// wide as the time. The windows share what room is left, up to a
    /// width each.
    #[must_use]
    pub fn new(
        state: &State,
        output: &str,
        width: u32,
        time: &str,
        theme: &Theme,
        text: &mut Text,
    ) -> Self {
        let height = theme.style(Part::Panel, States::NONE).height;
        let mut buttons = Vec::new();
        let mut left: u32 = 0;

        let workspaces = state.workspaces.iter();
        let workspaces = workspaces.filter(|workspace| workspace.output == output);
        for workspace in workspaces.clone() {
            let label = workspace_label(workspace);
            let lit = States::NONE.when(omorfos_config::State::Active, workspace.active);
            let needed = text.measure(&label, theme.font(Part::PanelButton, lit));
            // Never narrower than it is tall, so a short name is easy to hit.
            let wide = needed.saturating_add(PADDING * 2).max(height);
            buttons.push(Button {
                left,
                width: wide,
                label,
                lit: workspace.active,
                target: Target::Workspace(workspace.number),
            });
            left = left.saturating_add(wide);
        }

        let mut workspaces = workspaces;
        let shown = workspaces.find(|workspace| workspace.active);
        let shown = shown.map(|workspace| workspace.number);
        let windows = state.windows.iter();
        let windows: Vec<&Window> = windows
            .filter(|window| window.output == output && Some(window.workspace) == shown)
            .collect();
        let count = u32::try_from(windows.len()).unwrap_or(u32::MAX);
        let clock = Clock::new(time, width, theme, text);
        let room = clock.left.saturating_sub(left);
        let each = room.checked_div(count).unwrap_or(0).min(WIDEST_TASK);
        for window in windows {
            buttons.push(Button {
                left,
                width: each,
                label: window_label(window),
                lit: window.focused,
                target: Target::Window(window.id),
            });
            left = left.saturating_add(each);
        }

        Self {
            width,
            height,
            buttons,
            clock,
        }
    }

    /// How tall the bar is, in units of layout.
    #[must_use]
    pub fn height(&self) -> u32 {
        self.height
    }

    /// What a click `x` units from the bar's left edge asks of the
    /// compositor: to show the workspace or to bring up the window whose
    /// button is there. Nothing where there is no button.
    #[must_use]
    pub fn click(&self, x: i32) -> Option<Request> {
        let x = u32::try_from(x).ok()?;
        let mut buttons = self.buttons.iter();
        let button = buttons.find(|button| {
            let along = x.checked_sub(button.left);
            along.is_some_and(|along| along < button.width)
        })?;
        Some(match button.target {
            // The compositor switches workspace on the screen the pointer
            // is on, which for a click on this bar is this bar's screen.
            Target::Workspace(number) => Request::Perform {
                action: json!({ "workspace": number }),
            },
            Target::Window(window) => Request::Activate { window },
        })
    }

    /// Draws the bar, with `scale` pixels to a unit of layout.
    #[must_use]
    pub fn paint(&self, text: &mut Text, theme: &Theme, scale: Scale) -> Canvas {
        let panel = theme.style(Part::Panel, States::NONE);
        let mut canvas = Canvas::scaled(self.width, self.height, scale);
        canvas.fill((0, 0, self.width, self.height), panel.background);
        for button in &self.buttons {
            let left = button.left.cast_signed();
            let states = States::NONE.when(omorfos_config::State::Active, button.lit);
            let style = theme.style(Part::PanelButton, states);
            let line = style.underline;
            let top = self.height.saturating_sub(line.width);
            canvas.fill((left, 0, button.width, top), style.background);
            canvas.fill(
                (left, top.cast_signed(), button.width, line.width),
                line.color,
            );
            let label = (
                left.saturating_add_unsigned(PADDING),
                0,
                button.width.saturating_sub(PADDING * 2),
                self.height,
            );
            let font = theme.font(Part::PanelButton, states);
            text.draw(&mut canvas, &button.label, label, style.color, font);
        }
        let clock = &self.clock;
        let time = (
            clock.left.cast_signed().saturating_add_unsigned(PADDING),
            0,
            clock.width.saturating_sub(PADDING * 2),
            self.height,
        );
        let ink = theme.style(Part::Clock, States::NONE).color;
        let font = theme.font(Part::Clock, States::NONE);
        text.draw(&mut canvas, &clock.time, time, ink, font);
        canvas
    }
}

impl Clock {
    /// A clock showing `time` at the end of a bar `width` wide: as wide as
    /// the time needs, or nothing at all if there is no time to show.
    fn new(time: &str, width: u32, theme: &Theme, text: &mut Text) -> Self {
        let needed = text.measure(time, theme.font(Part::Clock, States::NONE));
        let room = needed.saturating_add(PADDING * 2);
        let wide = if time.is_empty() { 0 } else { room };
        Self {
            left: width.saturating_sub(wide),
            width: wide,
            time: time.to_owned(),
        }
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
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE);
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
        let canvas = narrow.paint(&mut Text::new(), &Theme::default(), Scale::ONE);
        assert_eq!(canvas.width(), 10);
    }

    #[test]
    fn what_is_shown_and_what_has_focus_are_lit_and_underlined() {
        let theme = Theme::default();
        let colors = palette(&theme);
        let bar = bar(&state(), "left");
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE);
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
        let canvas = bar.paint(&mut Text::new(), &Theme::default(), Scale::new(2.0));
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
        let canvas = bar.paint(&mut Text::new(), &theme, Scale::ONE);
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
        let canvas = longer.paint(&mut Text::new(), &theme, Scale::ONE);
        let all = usize::try_from(40 * height).unwrap();
        assert!(count(&canvas, WIDTH - 140..WIDTH - 100, 0..height, background) < all);

        // A bar with no time has nothing there.
        let canvas = self::bar(&state(), "left").paint(&mut Text::new(), &theme, Scale::ONE);
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
