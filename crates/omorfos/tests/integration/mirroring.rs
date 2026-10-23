//! A screen that mirrors another: it shows what that one shows, as large
//! as fits on it, and stands nowhere of its own.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::time::Duration;

use omorfos::Input;
use omorfos_config::OfferedMode;

use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    Anchor, KeyboardInteractivity,
};

use crate::common::{Event, HEIGHT, LayerShape, Picture, Session, WIDTH};
use crate::windows::{BACKGROUND, BLUE, GREEN};

const BLACK: u32 = 0xff00_0000;

/// The second screen plugged in mirrors the first.
const MIRRORED: &str = "outputs:\n  - { name: test-0 }\n  - { name: test-1, mirror: test-0 }\n";

/// Where `colour` is on a picture `size` large: its left, top, right and
/// bottom pixels.
fn bounds(picture: &Picture, (width, height): (i32, i32), colour: u32) -> (i32, i32, i32, i32) {
    let places = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));
    let mut found = places.filter(|(x, y)| picture.at(*x, *y) == colour);
    let first = found.next().unwrap();
    found.fold(
        (first.0, first.1, first.0, first.1),
        |(l, t, r, b), (x, y)| (l.min(x), t.min(y), r.max(x), b.max(y)),
    )
}

#[test]
fn a_screen_that_mirrors_another_shows_what_it_shows_as_large_as_fits() {
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    let mirror = session.add_output((WIDTH * 2, HEIGHT * 2));
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), BLUE);
    let (left, top, right, bottom) = bounds(&session.draw(), (WIDTH, HEIGHT), BLUE);
    let shown = bounds(&session.draw_on(1), (WIDTH * 2, HEIGHT * 2), BLUE);
    // Twice as large each way, where the window is twice as far in.
    let near = |shown: i32, wanted: i32| (shown - wanted).abs() <= 2;
    assert!(
        near(shown.0, left * 2) && near(shown.1, top * 2),
        "{shown:?}"
    );
    assert!(
        near(shown.2, right * 2 + 1) && near(shown.3, bottom * 2 + 1),
        "{shown:?}"
    );
    // It stands nowhere, and programs are not told of it.
    assert_eq!(client.state.outputs, 1);
    assert!(session.compositor().area_of(&mirror).is_empty());
}

#[test]
fn a_mirror_of_another_shape_is_black_either_side_of_what_it_shows() {
    // Wider: black to the left and the right of a picture as tall as it.
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    session.add_output((WIDTH * 2, HEIGHT));
    // Nor does what is on a screen beyond the one it mirrors show there.
    let beside = session.add_output((WIDTH, HEIGHT));
    let mut client = session.connect();
    let on = session.compositor().area_of(&beside);
    let middle = (f64::from(on.loc.x + 100), f64::from(on.loc.y + 100));
    let motion = Input::Motion {
        x: middle.0,
        y: middle.1,
    };
    session.compositor_mut().input(motion, 0);
    session.open(&mut client, "app", (WIDTH, HEIGHT), BLUE);
    let picture = session.draw_on(1);
    let row = HEIGHT / 2;
    let across = [
        0,
        WIDTH / 2 - 1,
        WIDTH / 2,
        WIDTH * 3 / 2 - 1,
        WIDTH * 3 / 2,
        WIDTH * 2 - 1,
    ];
    let shown = across.map(|x| picture.at(x, row));
    assert_eq!(shown, [BLACK, BLACK, BACKGROUND, BACKGROUND, BLACK, BLACK]);

    // Taller: black above and below.
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    session.add_output((WIDTH, HEIGHT * 2));
    let picture = session.draw_on(1);
    let down = [
        0,
        HEIGHT / 2 - 1,
        HEIGHT / 2,
        HEIGHT * 3 / 2 - 1,
        HEIGHT * 3 / 2,
    ];
    let shown = down.map(|y| picture.at(WIDTH / 2, y));
    assert_eq!(shown, [BLACK, BLACK, BACKGROUND, BACKGROUND, BLACK]);
}

#[test]
fn a_mirror_stands_on_its_own_once_the_configuration_or_the_one_it_mirrors_goes() {
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    let mirror = session.add_output((800, 600));
    assert!(session.compositor().area_of(&mirror).is_empty());
    session.write_config("config.yaml", "");
    session.eventually(|session| session.compositor().area_of(&mirror).loc.x == WIDTH);
    assert_eq!(
        session.connect().state.outputs,
        2,
        "and programs are told of it"
    );

    session.write_config("config.yaml", MIRRORED);
    session.eventually(|session| session.compositor().area_of(&mirror).is_empty());
    let first = session.output();
    session.compositor_mut().remove_output(&first);
    let area = session.compositor().area_of(&mirror);
    assert_eq!((area.loc.x, area.size.w, area.size.h), (0, 800, 600));
    assert_eq!(session.draw_on(1).at(10, 10), BACKGROUND);
}

#[test]
fn what_a_mirror_had_on_it_goes_to_the_screen_it_mirrors() {
    let mut session = Session::new();
    let second = session.add_output((800, 600));
    let on = session.compositor().area_of(&second);
    let motion = Input::Motion {
        x: f64::from(on.loc.x + 100),
        y: f64::from(on.loc.y + 100),
    };
    session.compositor_mut().input(motion, 0);
    let mut client = session.connect();
    session.open(&mut client, "app", (200, 100), BLUE);
    let blue = |picture: &Picture, (width, height): (i32, i32)| {
        let places = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));
        places.into_iter().any(|(x, y)| picture.at(x, y) == BLUE)
    };
    assert!(blue(&session.draw_on(1), (800, 600)));
    assert!(!blue(&session.draw(), (WIDTH, HEIGHT)));
    session.write_config("config.yaml", MIRRORED);
    session.eventually(|session| session.compositor().area_of(&second).is_empty());
    assert!(blue(&session.draw(), (WIDTH, HEIGHT)));
}

#[test]
fn a_locked_session_is_hidden_on_a_mirror_before_it_is_said_to_be_locked() {
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    session.add_output((WIDTH, HEIGHT));
    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    session.draw();
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    assert!(
        !locker.take_events().contains(&Event::Locked),
        "not while the mirror may still show the session"
    );
    assert_eq!(session.draw_on(1).at(10, 10), GREEN);
    session.send_frames(Duration::from_millis(32));
    session.roundtrip(&mut locker);
    assert!(locker.take_events().contains(&Event::Locked));
}

#[test]
fn a_mirror_is_listed_with_the_screen_it_mirrors_and_its_own_modes() {
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    let mirror = session.add_output((1920, 1080));
    let mode = OfferedMode {
        width: 1920,
        height: 1080,
        refresh: 60,
        preferred: true,
    };
    let compositor = session.compositor_mut();
    compositor.offer_modes(&mirror, &[mode], mode);
    compositor.suggest_scale(&mirror, omorfos::Scale::new(2.0));
    let mut peer = omorfos_ipc::Client::connect(&session.listen()).unwrap();
    let omorfos_ipc::Reply::State(state) = session.ask(&mut peer, &omorfos_ipc::Request::State)
    else {
        unreachable!("the state was asked for");
    };
    let names: Vec<(&str, &str)> = state
        .screens
        .iter()
        .map(|screen| (screen.name.as_str(), screen.mirrors.as_str()))
        .collect();
    assert_eq!(names, [("test-0", ""), ("test-1", "test-0")]);
    let listed = &state.screens[1];
    assert_eq!(
        (listed.width, listed.height),
        (WIDTH.unsigned_abs(), HEIGHT.unsigned_abs())
    );
    assert_eq!(listed.mode, "1920x1080@60");
}

#[test]
fn a_mirror_of_a_screen_at_another_scale_shows_everything_as_large_as_the_rest() {
    // The first at a scale of two, and the mirror, as large in pixels, at
    // one: what it shows is the same picture, at the same size.
    let mut session = Session::with_config(&[("config.yaml", MIRRORED)]);
    session.suggest_scale(omorfos::Scale::new(2.0));
    session.add_output((WIDTH, HEIGHT));
    let mut client = session.connect();
    session.open(&mut client, "app", (100, 50), BLUE);
    // And a panel along the bottom, as wide as the screen.
    let panel = LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
        size: (0, 20),
        exclusive_zone: 20,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &panel, (WIDTH / 2, 20), GREEN);
    let motion = Input::Motion {
        x: f64::from(WIDTH / 4),
        y: f64::from(HEIGHT / 4),
    };
    session.compositor_mut().input(motion, 0);
    let source = session.draw();
    let shown = session.draw_on(1);
    assert_eq!(
        bounds(&shown, (WIDTH, HEIGHT), BLUE),
        bounds(&source, (WIDTH, HEIGHT), BLUE)
    );
    // The panel as wide as the screen, as it is on the other.
    let (left, top, right, bottom) = bounds(&shown, (WIDTH, HEIGHT), GREEN);
    assert!(left <= 1 && right >= WIDTH - 2, "{left} {right}");
    assert!(top >= HEIGHT - 42 && bottom >= HEIGHT - 2, "{top} {bottom}");
    assert_eq!(
        (left, top, right, bottom),
        bounds(&source, (WIDTH, HEIGHT), GREEN)
    );
    // The pointer too, and everything else: the two are the same.
    let places = (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| (x, y)));
    let differ = places.filter(|(x, y)| shown.at(*x, *y) != source.at(*x, *y));
    assert_eq!(differ.count(), 0);
}
