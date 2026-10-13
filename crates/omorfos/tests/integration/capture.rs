//! Pictures of the screen: a program that may take them is given what
//! the screen shows, and the user is shown that it is happening.

use std::fs::File;
use std::time::Duration;

use omorfos::{Input, Roles};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm::Format;
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{ExtImageCopyCaptureFrameV1, FailureReason},
    ext_image_copy_capture_session_v1::ExtImageCopyCaptureSessionV1,
};

use crate::common::{Client, Event, HEIGHT, Picture, Session, WIDTH};
use crate::windows::{BACKGROUND, BLUE, GREEN, RED};

/// The dot in the middle of the mark that says the screen is captured.
const DOT: u32 = 0xfff0_524f;
/// The middle of the mark: it is 24 by 16, and 8 from the top and the
/// right of the screen.
const MARK: (i32, i32) = (WIDTH - 8 - 12, 8 + 8);

type Taking = ExtImageCopyCaptureSessionV1;

/// A program that may take pictures of the screen.
fn camera(session: &mut Session) -> Client {
    session.connect_as(Roles::CAPTURE)
}

/// Asks for a picture in `buffer`, without waiting for it.
fn ask(client: &Client, taking: &Taking, buffer: &WlBuffer) -> ExtImageCopyCaptureFrameV1 {
    let frame = client.shot(taking);
    frame.attach_buffer(buffer);
    frame.damage_buffer(0, 0, WIDTH, HEIGHT);
    frame.capture();
    frame
}

/// What came of a picture asked for: the events about it, once the
/// screen has been drawn.
fn develop(session: &mut Session, client: &mut Client) -> Vec<Event> {
    session.roundtrip(client);
    session.draw();
    session.roundtrip(client);
    client.take_events()
}

/// Takes a picture of the first screen as it is now.
fn snap(session: &mut Session, client: &mut Client, taking: &Taking) -> (Picture, Vec<Event>) {
    let (file, buffer) = client.shot_buffer((WIDTH, HEIGHT));
    let frame = ask(client, taking, &buffer);
    let events = develop(session, client);
    frame.destroy();
    (picture(&file), events)
}

fn picture(file: &File) -> Picture {
    Picture::read(file, WIDTH)
}

#[test]
fn a_program_that_may_is_given_what_the_screen_shows() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let mut camera = camera(&mut session);

    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let width = WIDTH.unsigned_abs();
    let told = [
        Event::CaptureSize(width, HEIGHT.unsigned_abs()),
        Event::CaptureFormat(Format::Xrgb8888),
        Event::CaptureKnown,
    ];
    assert_eq!(camera.take_events(), told);

    let (shot, events) = snap(&mut session, &mut camera, &taking);
    let whole = Event::ShotDamage(0, 0, WIDTH, HEIGHT);
    assert_eq!(events, [whole, Event::ShotReady], "the first is all new");
    assert_eq!(shot.at(10, 10), RED, "the window");
    assert_eq!(shot.at(199, 99), RED);
    assert_eq!(shot.at(300, 300), BACKGROUND, "the desktop");
    assert_eq!(shot.at(WIDTH - 1, HEIGHT - 1), BACKGROUND);

    // Having finished, it lets go of everything.
    taking.destroy();
    camera.finish_capturing();
    session.roundtrip(&mut camera);
}

#[test]
fn a_mark_is_shown_while_pictures_are_taken_and_is_not_in_them() {
    let mut session = Session::new();
    let mut camera = camera(&mut session);
    assert_eq!(session.draw().at(MARK.0, MARK.1), BACKGROUND);

    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    assert_eq!(session.draw().at(MARK.0, MARK.1), DOT, "as soon as it may");

    let (shot, _) = snap(&mut session, &mut camera, &taking);
    assert_eq!(shot.at(MARK.0, MARK.1), BACKGROUND, "not in the picture");
    assert_eq!(session.draw().at(MARK.0, MARK.1), DOT, "but on the screen");

    // It stays a moment after the last picture, so that one picture
    // cannot be taken unseen, and then goes.
    taking.destroy();
    session.roundtrip(&mut camera);
    session.advance(Duration::from_millis(1400));
    assert_eq!(session.draw().at(MARK.0, MARK.1), DOT);
    session.advance(Duration::from_millis(200));
    assert_eq!(session.draw().at(MARK.0, MARK.1), BACKGROUND);
}

#[test]
fn the_mark_is_only_on_the_screen_pictures_are_taken_of() {
    let mut session = Session::new();
    session.add_output((WIDTH, HEIGHT));
    let mut camera = camera(&mut session);
    let _taking = camera.capture(1, false);
    session.roundtrip(&mut camera);
    assert_eq!(session.draw_on(1).at(MARK.0, MARK.1), DOT);
    assert_eq!(session.draw_on(0).at(MARK.0, MARK.1), BACKGROUND);
}

#[test]
fn the_mark_follows_the_theme() {
    let mut session = Session::new();
    let mut camera = camera(&mut session);
    let _taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    assert_eq!(session.draw().at(MARK.0, MARK.1), DOT);
    session.write_config("theme.css", "capture { color: #00ff00; }");
    session.settle();
    assert_eq!(session.draw().at(MARK.0, MARK.1), GREEN);
}

#[test]
fn an_ordinary_program_is_given_no_pictures_and_the_user_is_told() {
    let mut session = Session::new();
    let mut client = session.connect_plain();
    let taking = client.capture(0, false);
    session.roundtrip(&mut client);
    assert_eq!(client.take_events(), [Event::CaptureStopped]);
    // What is in that corner is the notice, which says what was refused.
    assert_ne!(session.draw().at(MARK.0, MARK.1), DOT, "nothing is taken");

    let said: Vec<String> = session.compositor().events().map(str::to_owned).collect();
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(
        said[0].contains("asked to take pictures of the screen"),
        "{said:?}"
    );
    assert!(said[0].contains("may: [capture]"), "{said:?}");

    // A picture it asks for all the same fails.
    let (file, buffer) = client.shot_buffer((WIDTH, HEIGHT));
    let _frame = ask(&client, &taking, &buffer);
    let events = develop(&mut session, &mut client);
    assert_eq!(events, [Event::ShotFailed(FailureReason::Stopped)]);
    assert_eq!(picture(&file).at(10, 10), 0, "nothing was written");
}

#[test]
fn the_pointer_is_in_a_picture_only_if_the_program_asks() {
    let mut session = Session::new();
    let themes = session.install_cursors("default", &[("default", 8, BLUE)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    session
        .compositor_mut()
        .input(Input::Motion { x: 300.0, y: 200.0 }, 0);
    let mut camera = camera(&mut session);
    assert_eq!(session.draw().at(303, 203), BLUE, "the pointer is drawn");

    // Both are asked for at once, and both are taken when the screen is
    // next drawn.
    let without = camera.capture(0, false);
    let with = camera.capture(0, true);
    let (plain, first) = camera.shot_buffer((WIDTH, HEIGHT));
    let (pointed, second) = camera.shot_buffer((WIDTH, HEIGHT));
    let _first = ask(&camera, &without, &first);
    let _second = ask(&camera, &with, &second);
    let events = develop(&mut session, &mut camera);
    assert_eq!(
        events
            .iter()
            .filter(|event| **event == Event::ShotReady)
            .count(),
        2
    );
    assert_eq!(picture(&plain).at(303, 203), BACKGROUND);
    assert_eq!(picture(&pointed).at(303, 203), BLUE);
    assert_eq!(session.draw().at(303, 203), BLUE, "and still on the screen");
}

#[test]
fn a_picture_after_the_first_waits_for_something_to_change() {
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    snap(&mut session, &mut camera, &taking);

    let (file, buffer) = camera.shot_buffer((WIDTH, HEIGHT));
    let _frame = ask(&camera, &taking, &buffer);
    assert_eq!(
        develop(&mut session, &mut camera),
        [],
        "nothing has changed"
    );
    assert_eq!(picture(&file).at(10, 10), 0);

    app.fill(&window.surface, (200, 100), GREEN);
    session.roundtrip(&mut app);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events.last(), Some(&Event::ShotReady));
    assert_eq!(picture(&file).at(10, 10), GREEN);
    // What is said to have changed is the window, not all of the screen.
    let changed = |event: &Event| match event {
        Event::ShotDamage(x, y, width, height) => Some((*x, *y, *width, *height)),
        _ => None,
    };
    let changed: Vec<_> = events.iter().filter_map(changed).collect();
    assert_ne!(changed, []);
    assert!(
        changed
            .iter()
            .all(|(.., width, height)| *width <= 200 && *height <= 100),
        "{changed:?}"
    );
}

#[test]
fn a_buffer_that_is_not_as_asked_fails_the_picture() {
    let mut session = Session::new();
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    camera.take_events();
    let failed = [Event::ShotFailed(FailureReason::BufferConstraints)];

    // Too small.
    let (file, buffer) = camera.shot_buffer((WIDTH / 2, HEIGHT));
    let frame = ask(&camera, &taking, &buffer);
    assert_eq!(develop(&mut session, &mut camera)[..], failed);
    assert_eq!(Picture::read(&file, WIDTH / 2).at(10, 10), 0);
    frame.destroy();

    // Not memory shared with the compositor at all.
    let buffer = camera.pixel_buffer();
    let frame = ask(&camera, &taking, &buffer);
    assert_eq!(develop(&mut session, &mut camera)[..], failed);
    frame.destroy();

    // The next, in a buffer as asked, is still the first to be taken.
    let (shot, events) = snap(&mut session, &mut camera, &taking);
    assert_eq!(events.last(), Some(&Event::ShotReady));
    assert_eq!(shot.at(10, 10), BACKGROUND);
}

#[test]
fn nothing_is_given_while_the_session_is_locked() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "secret", (200, 100), RED);
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    camera.take_events();

    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    session.draw();
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    assert!(locker.take_events().contains(&Event::Locked));

    let (file, buffer) = camera.shot_buffer((WIDTH, HEIGHT));
    let _frame = ask(&camera, &taking, &buffer);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events, [Event::ShotFailed(FailureReason::Unknown)]);
    assert_eq!(
        picture(&file).at(10, 10),
        0,
        "neither the locker nor what is behind it"
    );
}

#[test]
fn a_program_is_told_when_its_screen_changes_size_or_goes() {
    let mut session = Session::new();
    let second = session.add_output((WIDTH, HEIGHT));
    let mut camera = camera(&mut session);
    let taking = camera.capture(1, false);
    session.roundtrip(&mut camera);
    camera.take_events();

    session.compositor_mut().resize_output(&second, (320, 240));
    session.roundtrip(&mut camera);
    let told = [
        Event::CaptureSize(320, 240),
        Event::CaptureFormat(Format::Xrgb8888),
        Event::CaptureKnown,
    ];
    let events = camera.take_events();
    assert!(
        told.iter().all(|event| events.contains(event)),
        "{events:?}"
    );

    // A picture is asked for, and the screen is unplugged before it is
    // taken: the picture fails and no more are given.
    let (_file, buffer) = camera.shot_buffer((320, 240));
    let _frame = ask(&camera, &taking, &buffer);
    session.roundtrip(&mut camera);
    session.compositor_mut().remove_output(&second);
    session.roundtrip(&mut camera);
    let events = camera.take_events();
    assert!(events.contains(&Event::CaptureStopped), "{events:?}");
    assert!(
        events.contains(&Event::ShotFailed(FailureReason::Stopped)),
        "{events:?}"
    );
}

#[test]
fn the_pointer_by_itself_is_not_given_as_pictures() {
    let mut session = Session::new();
    let mut camera = camera(&mut session);
    let _taking = camera.capture_pointer();
    session.roundtrip(&mut camera);
    assert_eq!(camera.take_events(), [Event::CaptureStopped]);
}

#[test]
fn a_picture_let_go_of_before_it_is_asked_for_is_forgotten() {
    let mut session = Session::new();
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    camera.shot(&taking).destroy();
    session.roundtrip(&mut camera);
    let (_, events) = snap(&mut session, &mut camera, &taking);
    assert_eq!(events.last(), Some(&Event::ShotReady));
}

/// The errors a program that does not keep to the rules is ended with:
/// what it did, and the number of the error on a picture or on what
/// pictures are asked for through.
#[test]
fn a_program_that_breaks_the_rules_of_asking_is_ended() {
    type Wrong = fn(&Client, &Taking, &WlBuffer);
    let picture = "ext_image_copy_capture_frame_v1";
    let wrongs: [(&str, u32, Wrong); 5] = [
        (picture, 1, |client, taking, _| {
            client.shot(taking).capture();
        }),
        (picture, 2, |client, taking, _| {
            client.shot(taking).damage_buffer(-1, 0, 10, 10);
        }),
        (picture, 2, |client, taking, _| {
            client.shot(taking).damage_buffer(0, 0, 0, 10);
        }),
        (picture, 3, |client, taking, buffer| {
            ask(client, taking, buffer).attach_buffer(buffer);
        }),
        (
            "ext_image_copy_capture_session_v1",
            1,
            |client, taking, _| {
                let _first = client.shot(taking);
                let _second = client.shot(taking);
            },
        ),
    ];
    for (about, number, wrong) in wrongs {
        let mut session = Session::new();
        let mut camera = camera(&mut session);
        let taking = camera.capture(0, false);
        let (_file, buffer) = camera.shot_buffer((WIDTH, HEIGHT));
        wrong(&camera, &taking, &buffer);
        let mut error = None;
        for _ in 0..10 {
            session.step();
            error = error.or_else(|| camera.error());
        }
        assert_eq!(error, Some((about.to_owned(), number)));
    }
}
