//! The overview: every window of the workspace laid out small side by side,
//! gliding there from where it is, to pick one from.

use std::time::Duration;

use omorfos::Input;
use omorfos_config::{Action, Part, State, States, Theme};
use omorfos_core::WindowId;
use omorfos_core::geometry::Rect;
use smithay::input::keyboard::Keycode;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Client, HEIGHT, Session, WIDTH, key};
use crate::windows::{BLUE, GREEN, RED};

const LEFT_BUTTON: u32 = 0x110;

/// The screen behind the overview dimmed but not blurred, which takes the
/// software renderer a while, for what is not about it.
const UNBLURRED: (&str, &str) = (
    "theme.css",
    "overview-backdrop { backdrop-filter: none; }\n",
);

/// How long the overview takes to glide open or closed, and a little more.
const GLIDE: Duration = Duration::from_millis(260);

fn press(session: &mut Session, code: Keycode, pressed: bool) {
    session
        .compositor_mut()
        .input(Input::Key { code, pressed }, 0);
}

fn tap(session: &mut Session, code: Keycode) {
    press(session, code, true);
    press(session, code, false);
    session.step();
}

fn click(session: &mut Session, (x, y): (i32, i32)) {
    let compositor = session.compositor_mut();
    let (x, y) = (f64::from(x), f64::from(y));
    compositor.input(Input::Motion { x, y }, 0);
    for pressed in [true, false] {
        let button = LEFT_BUTTON;
        compositor.input(Input::Button { button, pressed }, 0);
    }
    session.step();
}

fn focused(session: &Session) -> Option<String> {
    let desktop = session.compositor().desktop();
    let window = desktop.focused().and_then(|id| desktop.window(id));
    window.map(|window| window.app_id.clone())
}

fn name(session: &Session, id: WindowId) -> String {
    let desktop = session.compositor().desktop();
    let window = desktop.window(id).map(|window| window.app_id.clone());
    window.unwrap_or_default()
}

/// The middle of `rect`.
fn middle(rect: Rect) -> (i32, i32) {
    let x = rect.origin.x + rect.size.width.cast_signed() / 2;
    let y = rect.origin.y + rect.size.height.cast_signed() / 2;
    (x, y)
}

/// Where the overview shows each window, by name, and the one picked.
type Shown = (Vec<(String, Rect)>, Option<String>);

/// Where the overview shows each window, by name, and the one picked.
fn shown(session: &Session) -> Option<Shown> {
    let overviewed = session.compositor().overview_layout()?;
    let places = overviewed.windows.into_iter();
    let places = places
        .map(|(id, place)| (name(session, id), place))
        .collect();
    Some((places, overviewed.picked.map(|id| name(session, id))))
}

/// Where the window called `which` is shown in the overview.
fn place(session: &Session, which: &str) -> Rect {
    let (places, _) = shown(session).unwrap_or_default();
    let found = places.into_iter().find(|(name, _)| name == which);
    found.map(|(_, place)| place).unwrap_or_default()
}

/// Three windows side by side across the screen, the last with focus,
/// with `files` as the configuration, the clock started.
fn three(files: &[(&str, &str)]) -> (Session, Client) {
    let mut session = Session::with_config(files);
    let mut client = session.connect();
    for (name, colour) in [("one", RED), ("two", BLUE), ("three", GREEN)] {
        session.open(&mut client, name, (180, 120), colour);
    }
    session.step();
    (session, client)
}

/// Opens the overview, and lets it glide open.
fn open(session: &mut Session) {
    session.compositor_mut().perform(&Action::Overview);
    session.step();
    session.advance(GLIDE);
}

/// The colour `0xAARRGGBB` a part's border is drawn in, in `states`.
fn border(part: Part, states: States) -> u32 {
    let line = Theme::default().style(part, states).border.left.color;
    u32::from_be_bytes([line.alpha, line.red, line.green, line.blue])
}

#[test]
fn the_windows_of_the_workspace_are_laid_out_small_side_by_side_and_hidden_where_they_were() {
    let (mut session, _client) = three(&[UNBLURRED]);
    let before = session.draw();
    let corner = (WIDTH - 3, HEIGHT - 3);
    open(&mut session);
    let (places, picked) = shown(&session).unwrap();
    assert_eq!(places.len(), 3);
    assert_eq!(picked.as_deref(), Some("three"), "the one with focus");
    let frame = session.draw();
    for (which, colour) in [("one", RED), ("two", BLUE), ("three", GREEN)] {
        let (x, y) = middle(place(&session, which));
        assert_eq!(frame.at(x, y), colour, "{which}");
    }
    // Round the one picked, the accent, a little way out.
    let three = place(&session, "three");
    let active = States::NONE.with(State::Active);
    let accent = border(Part::OverviewWindow, active);
    let (_, y) = middle(three);
    assert_eq!(frame.at(three.origin.x - 5, y), accent);
    // The screen behind is dimmed, and where no window shows in the
    // overview, none is: not drawn where it was.
    assert_ne!(frame.at(corner.0, corner.1), before.at(corner.0, corner.1));
    for (_, place) in &places {
        assert!(
            place.size.width <= 180 && place.size.height <= 120,
            "{place:?}"
        );
    }
}

#[test]
fn opening_each_window_glides_from_where_it_is_and_closing_back() {
    let (mut session, _client) = three(&[UNBLURRED]);
    let desktop = session.compositor().desktop();
    let where_one = desktop.frame_of(desktop.windows().next().unwrap().window.id);
    let home = where_one.unwrap().geometry;
    session.compositor_mut().perform(&Action::Overview);
    session.step();
    // It sets out from where it is, and is drawn there.
    let now = session.now;
    let due = session.compositor().deadline();
    let frame = Duration::from_millis(20);
    assert!(
        due.is_some_and(|due| due > now && due <= now + frame),
        "{due:?}"
    );
    let (x, y) = (
        home.origin.x + 20,
        home.origin.y + home.size.height.cast_signed() - 20,
    );
    assert_eq!(session.draw().at(x, y), RED, "where it was");
    session.advance(GLIDE);
    assert_ne!(session.draw().at(x, y), RED, "gone to its place");
    // Closed, it glides back, and only then is the window drawn again.
    session.compositor_mut().perform(&Action::Overview);
    session.step();
    session.advance(Duration::from_millis(120));
    assert!(
        session.compositor().overview_layout().is_some(),
        "still closing"
    );
    session.advance(GLIDE);
    assert_eq!(session.compositor().overview_layout(), None);
    assert_eq!(session.draw().at(x, y), RED);
}

#[test]
fn the_arrows_walk_the_windows_and_enter_goes_to_the_one_picked_once_closed() {
    let (mut session, _client) = three(&[UNBLURRED]);
    open(&mut session);
    let picked = |session: &Session| shown(session).and_then(|(_, picked)| picked);
    let first = shown(&session).unwrap().0[0].0.clone();
    tap(&mut session, key::RIGHT);
    assert_eq!(picked(&session).as_deref(), Some(first.as_str()), "round");
    tap(&mut session, key::LEFT);
    assert_eq!(picked(&session).as_deref(), Some("three"));
    tap(&mut session, key::LEFT);
    let second = shown(&session).unwrap().0[1].0.clone();
    assert_eq!(picked(&session), Some(second.clone()));
    tap(&mut session, key::RETURN);
    assert_eq!(focused(&session).as_deref(), Some("three"), "not yet");
    session.advance(GLIDE);
    assert_eq!(focused(&session), Some(second));
    assert_eq!(session.compositor().overview_layout(), None);
}

#[test]
fn escape_closes_it_where_it_was_and_other_keys_are_bindings() {
    let (mut session, _client) = three(&[UNBLURRED]);
    open(&mut session);
    tap(&mut session, key::LEFT);
    tap(&mut session, key::ESCAPE);
    session.advance(GLIDE);
    assert_eq!(session.compositor().overview_layout(), None);
    assert_eq!(focused(&session).as_deref(), Some("three"));
    // The action again closes it too.
    open(&mut session);
    session.compositor_mut().perform(&Action::Overview);
    session.step();
    session.advance(GLIDE);
    assert_eq!(session.compositor().overview_layout(), None);
}

#[test]
fn the_pointer_picks_the_window_under_it_and_a_click_goes_to_it_or_closes_elsewhere() {
    let (mut session, _client) = three(&[UNBLURRED]);
    open(&mut session);
    let one = middle(place(&session, "one"));
    let (x, y) = (f64::from(one.0), f64::from(one.1));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
    let picked = shown(&session).and_then(|(_, picked)| picked);
    assert_eq!(picked.as_deref(), Some("one"));
    click(&mut session, one);
    session.advance(GLIDE);
    assert_eq!(focused(&session).as_deref(), Some("one"));
    // A click on nothing closes it, choosing nothing.
    open(&mut session);
    click(&mut session, (WIDTH - 3, HEIGHT - 3));
    session.advance(GLIDE);
    assert_eq!(session.compositor().overview_layout(), None);
    assert_eq!(focused(&session).as_deref(), Some("one"));
}

#[test]
fn with_motion_reduced_the_windows_fade_in_their_places_rather_than_glide() {
    let reduced = ("config.yaml", "appearance: { reduce_motion: true }\n");
    let (mut session, _client) = three(&[UNBLURRED, reduced]);
    let desktop = session.compositor().desktop();
    let first = desktop.windows().next().unwrap().window.id;
    let home = desktop.frame_of(first).unwrap().geometry;
    let (x, y) = (
        home.origin.x + 20,
        home.origin.y + home.size.height.cast_signed() - 20,
    );
    session.compositor_mut().perform(&Action::Overview);
    session.step();
    assert_ne!(session.draw().at(x, y), RED, "not where it was");
    session.advance(GLIDE);
    let (x, y) = middle(place(&session, "one"));
    assert_eq!(session.draw().at(x, y), RED);
}

#[test]
fn an_empty_workspace_opens_an_empty_overview_and_a_minimized_window_is_in_it() {
    let mut session = Session::with_config(&[UNBLURRED]);
    let corner = (WIDTH - 3, HEIGHT - 3);
    let before = session.draw().at(corner.0, corner.1);
    open(&mut session);
    assert_eq!(shown(&session), Some((Vec::new(), None)));
    assert_ne!(session.draw().at(corner.0, corner.1), before, "dimmed");
    session.compositor_mut().perform(&Action::Overview);
    session.advance(GLIDE);
    // A minimized window has nowhere to glide from, and fades in its place.
    let mut client = session.connect();
    session.open(&mut client, "small", (180, 120), BLUE);
    session.compositor_mut().perform(&Action::Minimize);
    session.step();
    open(&mut session);
    let (x, y) = middle(place(&session, "small"));
    assert_eq!(session.draw().at(x, y), BLUE);
}

#[test]
fn a_private_window_is_blank_in_the_overview() {
    let rules = "rules:\n  - match: { app_id: two }\n    private: true\n";
    let (mut session, _client) = three(&[UNBLURRED, ("rules.yaml", rules)]);
    open(&mut session);
    let (x, y) = middle(place(&session, "two"));
    let blank = Theme::default().style(Part::Capture, States::NONE).ground();
    let blank = u32::from_be_bytes([blank.alpha, blank.red, blank.green, blank.blue]);
    assert_eq!(session.draw().at(x, y), blank);
}

#[test]
fn the_switcher_and_the_overview_close_each_other_and_locking_closes_it() {
    let (mut session, _client) = three(&[UNBLURRED]);
    open(&mut session);
    // Alt+Tab over the overview opens the switcher in its stead.
    press(&mut session, key::ALT, true);
    tap(&mut session, key::TAB);
    assert_eq!(session.compositor().overview_layout(), None);
    assert!(session.compositor().switcher_layout().is_some());
    press(&mut session, key::ALT, false);
    session.step();
    open(&mut session);
    assert_eq!(session.compositor().switcher_layout(), None);
    assert!(session.compositor().overview_layout().is_some());
    let _locked = crate::lock::lock(&mut session);
    session.step();
    assert_eq!(session.compositor().overview_layout(), None);
}

#[test]
fn a_swipe_does_not_change_the_workspace_under_the_overview() {
    let mut session = Session::with_workspaces();
    let mut client = session.connect();
    session.open(&mut client, "one", (180, 120), RED);
    open(&mut session);
    let compositor = session.compositor_mut();
    compositor.input(
        Input::Gesture(omorfos::Gesture::SwipeBegin { fingers: 3 }),
        0,
    );
    let swipe = omorfos::Gesture::SwipeUpdate {
        dx: -600.0,
        dy: 0.0,
    };
    compositor.input(Input::Gesture(swipe), 10);
    let end = omorfos::Gesture::SwipeEnd { cancelled: false };
    compositor.input(Input::Gesture(end), 20);
    session.step();
    assert_eq!(session.compositor().desktop().active_workspace(), 0);
    assert!(session.compositor().overview_layout().is_some());
}

/// A window on each of the first two workspaces, the second shown, and the
/// overview open.
fn two_workspaces() -> (Session, Client) {
    let mut session = Session::with_workspaces();
    session.write_config(UNBLURRED.0, UNBLURRED.1);
    session.compositor_mut().reload_config();
    let mut client = session.connect();
    session.open(&mut client, "one", (300, 200), RED);
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.settle();
    session.open(&mut client, "two", (300, 200), BLUE);
    session.step();
    open(&mut session);
    (session, client)
}

#[test]
fn the_strip_shows_each_workspace_small_with_its_windows_and_the_one_shown_lit() {
    let (mut session, _client) = two_workspaces();
    let overviewed = session.compositor().overview_layout().unwrap();
    let shown: Vec<bool> = overviewed
        .workspaces
        .iter()
        .map(|(_, shown)| *shown)
        .collect();
    assert_eq!(shown[..2], [false, true], "the second shown");
    assert_eq!(shown.iter().filter(|shown| **shown).count(), 1);
    let (first, _) = overviewed.workspaces[0];
    let (second, _) = overviewed.workspaces[1];
    // Each window where it stands on the screen, made as small as the
    // workspace is: one on the first, two on the second.
    let desktop = session.compositor().desktop();
    let spot = |name: &str, on: Rect| {
        let id = desktop
            .windows()
            .find(|listed| listed.window.app_id == name);
        let frame = desktop.frame_of(id.unwrap().window.id).unwrap().geometry;
        let (x, y) = middle(frame);
        let x = on.origin.x + x * on.size.width.cast_signed() / WIDTH;
        let y = on.origin.y + y * on.size.width.cast_signed() / WIDTH;
        (x, y)
    };
    let (one, two) = (spot("one", first), spot("two", second));
    let frame = session.draw();
    assert_eq!(frame.at(one.0, one.1), RED);
    assert_eq!(frame.at(two.0, two.1), BLUE);
    // The one shown has the accent round it, the others the border.
    let (_, y) = middle(second);
    let active = States::NONE.with(State::Active);
    assert_eq!(
        frame.at(second.origin.x, y),
        border(Part::OverviewWorkspace, active)
    );
    let (_, y) = middle(first);
    let plain = border(Part::OverviewWorkspace, States::NONE);
    assert_eq!(frame.at(first.origin.x, y), plain);
    // Where no window is, the desktop's colour, as no wallpaper runs.
    let ground = Theme::default().style(Part::Desktop, States::NONE).ground();
    let ground = u32::from_be_bytes([ground.alpha, ground.red, ground.green, ground.blue]);
    let right = first.origin.x + first.size.width.cast_signed();
    let bottom = first.origin.y + first.size.height.cast_signed();
    let corner = (right - 10, bottom - 10);
    assert_eq!(frame.at(corner.0, corner.1), ground);
}

#[test]
fn a_click_on_a_workspace_in_the_strip_goes_to_it_and_shows_its_windows() {
    let (mut session, _client) = two_workspaces();
    let overviewed = session.compositor().overview_layout().unwrap();
    let (first, _) = overviewed.workspaces[0];
    // Over the strip, the workspace under the pointer is lit, and no window
    // is picked for it.
    let (x, y) = middle(first);
    let (fx, fy) = (f64::from(x), f64::from(y));
    session
        .compositor_mut()
        .input(Input::Motion { x: fx, y: fy }, 0);
    session.step();
    let overviewed = session.compositor().overview_layout().unwrap();
    assert_eq!(overviewed.hovered, Some(0));
    assert_eq!(
        overviewed.picked.map(|id| name(&session, id)).as_deref(),
        Some("two")
    );
    let frame = session.draw();
    let hovered = border(Part::OverviewWorkspace, States::NONE.with(State::Hovered));
    assert_eq!(frame.at(first.origin.x, y), hovered);
    click(&mut session, (x, y));
    assert_eq!(session.compositor().desktop().active_workspace(), 0);
    let (places, _) = shown(&session).unwrap();
    let names: Vec<&str> = places.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["one"], "still open, on the first");
    // Its window glides in from where it is, and is there a moment on.
    session.advance(GLIDE);
    let (x, y) = middle(place(&session, "one"));
    assert_eq!(session.draw().at(x, y), RED);
    tap(&mut session, key::RETURN);
    session.advance(GLIDE);
    assert_eq!(focused(&session).as_deref(), Some("one"));
}

#[test]
fn a_workspace_in_the_strip_has_the_wallpaper_and_its_windows_cut_at_its_edges() {
    let mut session = Session::with_workspaces();
    session.write_config(UNBLURRED.0, UNBLURRED.1);
    session.compositor_mut().reload_config();
    let mut client = session.connect();
    // A wallpaper in the bottom left corner, and a window larger than the
    // screen, which hangs off it.
    let corner = crate::common::LayerShape {
        layer: Layer::Background,
        anchor: Anchor::Bottom | Anchor::Left,
        size: (100, 50),
        exclusive_zone: 0,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &corner, (100, 50), GREEN);
    session.open(&mut client, "big", (800, 600), RED);
    session.step();
    open(&mut session);
    let overviewed = session.compositor().overview_layout().unwrap();
    let (shown, _) = overviewed.workspaces[0];
    let (empty, _) = overviewed.workspaces[1];
    let frame = session.draw();
    // The wallpaper on the empty one, as small as it is.
    let x = empty.origin.x + 50 * empty.size.width.cast_signed() / WIDTH;
    let y = empty.origin.y + 455 * empty.size.width.cast_signed() / WIDTH;
    assert_eq!(frame.at(x, y), GREEN);
    // Nothing of the window beyond the edges of the one it is on.
    let (_, y) = middle(shown);
    let right = shown.origin.x + shown.size.width.cast_signed();
    let inside = frame.at(right - 4, y);
    let beyond = frame.at(right + 3, y);
    assert_eq!(inside, RED);
    assert_ne!(beyond, RED);
    let bottom = shown.origin.y + shown.size.height.cast_signed();
    assert_ne!(frame.at(shown.origin.x + 10, bottom + 3), RED);
}

#[test]
fn the_screen_behind_is_blurred_unless_the_theme_says_not() {
    for (files, blurred) in [(&[][..], true), (&[UNBLURRED][..], false)] {
        let mut session = Session::with_config(files);
        drop(session.draw());
        let before = session.compositor().blurs();
        open(&mut session);
        drop(session.draw());
        assert_eq!(session.compositor().blurs() > before, blurred, "{files:?}");
    }
}

#[test]
fn a_private_window_is_blank_on_its_workspace_in_the_strip_too() {
    let mut session = Session::with_workspaces();
    let rules = "rules:\n  - match: { app_id: secret }\n    private: true\n";
    session.write_config("rules.yaml", rules);
    // A blank of its own colour, which the built-in theme shares with the
    // desktop.
    let theme = format!("{}capture {{ background: #ff00ff; }}\n", UNBLURRED.1);
    session.write_config(UNBLURRED.0, &theme);
    session.compositor_mut().reload_config();
    let mut client = session.connect();
    session.open(&mut client, "secret", (300, 200), RED);
    session.step();
    open(&mut session);
    let overviewed = session.compositor().overview_layout().unwrap();
    let (first, _) = overviewed.workspaces[0];
    let desktop = session.compositor().desktop();
    let id = desktop.windows().next().unwrap().window.id;
    let frame = desktop.frame_of(id).unwrap().geometry;
    let (x, y) = middle(frame);
    let x = first.origin.x + x * first.size.width.cast_signed() / WIDTH;
    let y = first.origin.y + y * first.size.width.cast_signed() / WIDTH;
    assert_eq!(session.draw().at(x, y), 0xffff_00ff);
    // Each window it shows is kept drawing as on the screen.
    session.send_frames(Duration::from_millis(16));
}

/// How far apart in green the two sides of an edge at `x` are, `by` either
/// side of it, on row `y`: much for a sharp edge, little for a blurred one.
fn apart(frame: &crate::common::Picture, (x, y): (i32, i32), by: i32) -> i32 {
    let green = |x: i32| i32::try_from((frame.at(x, y) >> 8) & 0xff).unwrap_or_default();
    (green(x + by) - green(x - by)).abs()
}

/// A session with four workspaces, `rules` and a blank colour of its own,
/// and on the first a window called `secret`, red on its left and white on
/// its right, which an edge down its middle that blur softens.
fn secret(rules: &str) -> (Session, Client) {
    let mut session = Session::with_workspaces();
    session.write_config("rules.yaml", rules);
    let theme = format!("{}capture {{ background: #ff00ff; }}\n", UNBLURRED.1);
    session.write_config(UNBLURRED.0, &theme);
    session.compositor_mut().reload_config();
    let mut client = session.connect();
    let window = session.open(&mut client, "secret", (300, 200), RED);
    let _white = client.subsurface(&window.surface, (150, 0), (150, 200), 0xffff_ffff);
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    (session, client)
}

/// Where the middle of the window called `secret` is drawn in workspace
/// `on` of the strip.
fn secret_in(session: &Session, on: Rect) -> (i32, i32) {
    let desktop = session.compositor().desktop();
    let id = desktop
        .windows()
        .find(|listed| listed.window.app_id == "secret");
    let id = id.map(|listed| listed.window.id);
    let frame = id.and_then(|id| desktop.frame_of(id));
    let frame = frame.map(|frame| frame.geometry).unwrap_or_default();
    let (x, y) = middle(frame);
    let x = on.origin.x + x * on.size.width.cast_signed() / WIDTH;
    let y = on.origin.y + y * on.size.width.cast_signed() / WIDTH;
    (x, y)
}

#[test]
fn a_window_private_by_blur_is_blurred_laid_out_and_on_a_workspace_not_shown() {
    let rules = "rules:\n  - match: { app_id: secret }\n    private: blur\n";
    let (mut session, _client) = secret(rules);
    // Laid out, where its edge is, blurred.
    open(&mut session);
    let edge = middle(place(&session, "secret"));
    let frame = session.draw();
    assert!(
        apart(&frame, edge, 2) < 0x20,
        "sharp laid out: {:#x}",
        apart(&frame, edge, 2)
    );
    session.compositor_mut().perform(&Action::Overview);
    session.advance(GLIDE);
    // On its workspace in the strip, while another is shown.
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.settle();
    open(&mut session);
    let overviewed = session.compositor().overview_layout().unwrap();
    let (first, shown) = overviewed.workspaces[0];
    assert!(!shown);
    let edge = secret_in(&session, first);
    let frame = session.draw();
    assert!(
        apart(&frame, edge, 2) < 0x20,
        "sharp in the strip: {:#x}",
        apart(&frame, edge, 2)
    );
}

#[test]
fn a_window_private_by_blank_is_blank_on_a_workspace_not_shown() {
    let rules = "rules:\n  - match: { app_id: secret }\n    private: true\n";
    let (mut session, _client) = secret(rules);
    session.compositor_mut().perform(&Action::NextWorkspace);
    session.settle();
    open(&mut session);
    let overviewed = session.compositor().overview_layout().unwrap();
    let (first, _) = overviewed.workspaces[0];
    let (x, y) = secret_in(&session, first);
    let frame = session.draw();
    for x in [x - 3, x, x + 3] {
        assert_eq!(frame.at(x, y), 0xffff_00ff, "at {x}");
    }
}

#[test]
fn the_button_after_the_strip_makes_a_new_workspace_and_goes_to_it() {
    let (mut session, _client) = two_workspaces();
    let overviewed = session.compositor().overview_layout().unwrap();
    let before = overviewed.workspaces.len();
    let (last, _) = *overviewed.workspaces.last().unwrap();
    let add = overviewed.add;
    assert!(i64::from(add.origin.x) > last.right(), "after the last");
    assert_eq!(add.origin.y, last.origin.y);
    // Under the pointer it is lit, and its plus is in the middle.
    let (x, y) = middle(add);
    let (fx, fy) = (f64::from(x), f64::from(y));
    session
        .compositor_mut()
        .input(Input::Motion { x: fx, y: fy }, 0);
    session.step();
    assert_eq!(
        session.compositor().overview_layout().unwrap().hovered,
        Some(before)
    );
    let frame = session.draw();
    let hovered = border(Part::OverviewAdd, States::NONE.with(State::Hovered));
    assert_eq!(frame.at(add.origin.x, y), hovered);
    let ink = Theme::default()
        .style(Part::OverviewAdd, States::NONE)
        .color;
    let ink = u32::from_be_bytes([ink.alpha, ink.red, ink.green, ink.blue]);
    assert_eq!(frame.at(x, y), ink, "the plus");
    // A click makes one at the end, and shows it, empty.
    click(&mut session, (x, y));
    let overviewed = session.compositor().overview_layout().unwrap();
    assert_eq!(overviewed.workspaces.len(), before + 1);
    let shown: Vec<bool> = overviewed
        .workspaces
        .iter()
        .map(|(_, shown)| *shown)
        .collect();
    assert_eq!(shown.last(), Some(&true));
    assert_eq!(overviewed.windows, []);
}
