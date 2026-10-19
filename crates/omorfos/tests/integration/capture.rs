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
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::Shape;

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
fn a_line_goes_round_a_screen_only_once_it_is_taken_again_and_again() {
    let mut session = Session::new();
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let edge = (0, HEIGHT / 2);
    // One picture, as a screenshot takes, has the mark and no line.
    let _ = snap(&mut session, &mut camera, &taking);
    let screen = session.draw();
    assert_eq!(screen.at(MARK.0, MARK.1), DOT);
    assert_eq!(
        screen.at(edge.0, edge.1),
        BACKGROUND,
        "no line for one picture"
    );
    // Another asked for, as sharing goes on taking them, and the line is
    // round it. It is in no picture, which the tests of sharing show.
    let _ = snap(&mut session, &mut camera, &taking);
    assert_ne!(session.draw().at(edge.0, edge.1), BACKGROUND, "the line");
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
fn only_what_has_changed_and_what_the_program_says_is_stale_is_written() {
    use std::os::unix::fs::FileExt;
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    snap(&mut session, &mut camera, &taking);

    // A buffer of the program's that holds something else altogether, of
    // which it says only the bottom right corner is out of date.
    let (file, buffer) = camera.shot_buffer((WIDTH, HEIGHT));
    let old = 0x1234_5678_u32
        .to_le_bytes()
        .repeat((WIDTH * HEIGHT) as usize);
    file.write_all_at(&old, 0).unwrap();
    let frame = camera.shot(&taking);
    frame.attach_buffer(&buffer);
    frame.damage_buffer(WIDTH - 4, HEIGHT - 4, 4, 4);
    frame.capture();
    app.fill(&window.surface, (200, 100), GREEN);
    session.roundtrip(&mut app);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events.last(), Some(&Event::ShotReady));
    let shot = picture(&file);
    assert_eq!(shot.at(100, 80), GREEN, "what changed is written");
    let corner = shot.at(WIDTH - 2, HEIGHT - 2) & 0x00ff_ffff;
    assert_eq!(corner, BACKGROUND & 0x00ff_ffff, "and what was stale");
    assert_eq!(shot.at(400, 400), 0x1234_5678, "and nothing else");
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

/// How large a picture of the pointer is, across and down.
const POINTER: i32 = 256;

/// A session whose pointer is drawn from a theme of squares 8 pixels
/// across, one pixel in from whose corner is the pointer's position: blue
/// for the ordinary pointer and green over text. The pointer is at
/// `(300, 200)`.
fn pointing() -> Session {
    let mut session = Session::new();
    let themes = session.install_cursors("default", &[("default", 8, BLUE), ("text", 8, GREEN)]);
    session.compositor_mut().set_cursor_themes(themes);
    session.compositor_mut().show_pointer();
    session
        .compositor_mut()
        .input(Input::Motion { x: 300.0, y: 200.0 }, 0);
    session
}

/// Asks for a picture of the pointer, and returns what came of it and
/// where it was put.
fn point(session: &mut Session, camera: &mut Client, taking: &Taking) -> (Vec<Event>, Picture) {
    let (file, buffer) = camera.shot_buffer_of((POINTER, POINTER), Format::Argb8888);
    let frame = ask(camera, taking, &buffer);
    let events = develop(session, camera);
    frame.destroy();
    (events, Picture::read(&file, POINTER))
}

#[test]
fn a_program_following_the_pointer_is_told_where_it_is_and_given_its_picture() {
    let mut session = pointing();
    let mut camera = camera(&mut session);
    let (cursor, taking) = camera.capture_pointer(0);
    session.roundtrip(&mut camera);
    session.step();
    session.roundtrip(&mut camera);
    let told = [
        Event::CaptureSize(256, 256),
        Event::CaptureFormat(Format::Argb8888),
        Event::CaptureKnown,
        Event::PointerEntered,
        Event::PointerTip(1, 1),
        Event::PointerAt(300, 200),
    ];
    assert_eq!(camera.take_events(), told);

    // Its picture is the pointer's own, at the top left corner, and
    // nothing round it.
    let (events, picture) = point(&mut session, &mut camera, &taking);
    let whole = Event::ShotDamage(0, 0, POINTER, POINTER);
    assert_eq!(events, [whole.clone(), Event::ShotReady]);
    assert_eq!(picture.at(4, 4), BLUE);
    assert_eq!(picture.at(100, 100), 0, "what is not the pointer is clear");

    // It moving is only told; its picture is taken again only once it
    // looks different.
    let (file, buffer) = camera.shot_buffer_of((POINTER, POINTER), Format::Argb8888);
    let frame = ask(&camera, &taking, &buffer);
    session
        .compositor_mut()
        .input(Input::Motion { x: 310.0, y: 205.0 }, 0);
    assert_eq!(
        develop(&mut session, &mut camera),
        [Event::PointerAt(310, 205)]
    );
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 50.0 }, 0);
    session.roundtrip(&mut app);
    app.set_cursor_shape(Shape::Text);
    session.roundtrip(&mut app);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events, [Event::PointerAt(50, 50), whole, Event::ShotReady]);
    assert_eq!(Picture::read(&file, POINTER).at(4, 4), GREEN);
    frame.destroy();

    // A pointer an application draws is given as it draws it, and again
    // when it changes; one it hides, as nothing.
    let drawn = app.surface();
    app.fill(&drawn, (8, 8), RED);
    app.set_cursor(Some(&drawn), (2, 2));
    session.roundtrip(&mut app);
    let (events, picture) = point(&mut session, &mut camera, &taking);
    assert!(events.contains(&Event::PointerTip(2, 2)), "{events:?}");
    assert_eq!(picture.at(4, 4), RED);
    assert_eq!(picture.at(100, 100), 0, "what is not the pointer is clear");
    app.fill(&drawn, (8, 8), GREEN);
    session.roundtrip(&mut app);
    let (_, picture) = point(&mut session, &mut camera, &taking);
    assert_eq!(picture.at(4, 4), GREEN);
    app.set_cursor(None, (0, 0));
    session.roundtrip(&mut app);
    let (events, picture) = point(&mut session, &mut camera, &taking);
    assert_eq!(events[0], Event::PointerTip(0, 0));
    assert_eq!(picture.at(4, 4), 0);

    // Leaving the screen and coming back is told; away from the
    // application that hid it, the pointer is shown again.
    session.add_output((WIDTH, HEIGHT));
    let on_the_second = Input::Motion {
        x: f64::from(WIDTH + 10),
        y: 10.0,
    };
    session.compositor_mut().input(on_the_second, 0);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events, [Event::PointerLeft, Event::PointerTip(1, 1)]);
    session
        .compositor_mut()
        .input(Input::Motion { x: 20.0, y: 30.0 }, 0);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events, [Event::PointerEntered, Event::PointerAt(20, 30)]);

    // A program that lets go of the pointer's pictures is still told where
    // it is; one that stops following it is told nothing more.
    taking.destroy();
    session
        .compositor_mut()
        .input(Input::Motion { x: 40.0, y: 30.0 }, 0);
    assert_eq!(
        develop(&mut session, &mut camera),
        [Event::PointerAt(40, 30)]
    );
    cursor.destroy();
    session
        .compositor_mut()
        .input(Input::Motion { x: 50.0, y: 30.0 }, 0);
    assert_eq!(develop(&mut session, &mut camera), []);
}

#[test]
fn a_picture_of_the_pointer_is_taken_only_in_a_buffer_it_fits_and_not_while_locked() {
    let mut session = pointing();
    let mut camera = camera(&mut session);
    let (_cursor, taking) = camera.capture_pointer(0);
    session.roundtrip(&mut camera);
    camera.take_events();
    let (file, buffer) = camera.shot_buffer((POINTER, POINTER));
    let frame = ask(&camera, &taking, &buffer);
    let failed = Event::ShotFailed(FailureReason::BufferConstraints);
    assert_eq!(develop(&mut session, &mut camera), [failed]);
    assert_eq!(picture(&file).at(4, 4), 0, "nothing was written");
    frame.destroy();

    let mut locker = session.connect();
    let lock = locker.lock();
    let surface = locker.lock_surface(&lock);
    session.roundtrip(&mut locker);
    locker.fill(&surface, (WIDTH, HEIGHT), GREEN);
    session.roundtrip(&mut locker);
    session.draw();
    session.send_frames(Duration::from_millis(16));
    session.roundtrip(&mut locker);
    let (events, _) = point(&mut session, &mut camera, &taking);
    assert_eq!(events, [Event::ShotFailed(FailureReason::Unknown)]);
}

#[test]
fn the_pointer_is_followed_only_over_a_screen_by_a_program_that_may() {
    let mut session = pointing();
    // Not by one that may not, and the user is told.
    let mut plain = session.connect_plain();
    let _refused = plain.capture_pointer(0);
    session.roundtrip(&mut plain);
    session.step();
    session.roundtrip(&mut plain);
    assert_eq!(plain.take_events(), [Event::CaptureStopped]);
    let said: Vec<String> = session.compositor().events().map(str::to_owned).collect();
    assert!(said[0].contains("asked to follow the pointer"), "{said:?}");
    let mut app = session.connect();

    // Nor over a window.
    let mut camera = session.connect_as(Roles::CAPTURE.with(Roles::PANEL));
    session.open(&mut app, "app", (200, 100), RED);
    session.roundtrip(&mut camera);
    let (_cursor, _taking) = camera.capture_pointer_over_window("app");
    session.roundtrip(&mut camera);
    session.step();
    session.roundtrip(&mut camera);
    assert!(camera.take_events().contains(&Event::CaptureStopped));

    // Over a screen that goes, the pointer is no longer followed, and
    // the program is told it has left if it was over it.
    let second = session.add_output((WIDTH, HEIGHT));
    let third = session.add_output((WIDTH, HEIGHT));
    session.roundtrip(&mut camera);
    let over = Input::Motion {
        x: f64::from(WIDTH + 10),
        y: 10.0,
    };
    session.compositor_mut().input(over, 0);
    let (_over, _) = camera.capture_pointer(1);
    let (_away, _) = camera.capture_pointer(2);
    session.roundtrip(&mut camera);
    session.step();
    session.roundtrip(&mut camera);
    camera.take_events();
    session.compositor_mut().remove_output(&third);
    session.compositor_mut().remove_output(&second);
    session.step();
    session.roundtrip(&mut camera);
    let events = camera.take_events();
    let stopped = events
        .iter()
        .filter(|event| **event == Event::CaptureStopped);
    assert_eq!(stopped.count(), 2, "{events:?}");
    let left = events.iter().filter(|event| **event == Event::PointerLeft);
    assert_eq!(left.count(), 1, "{events:?}");
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

#[test]
fn a_program_that_asks_twice_for_pictures_of_the_pointer_it_follows_is_ended() {
    let mut session = pointing();
    let mut camera = camera(&mut session);
    let (cursor, _taking) = camera.capture_pointer(0);
    let _again = camera.capture_pointer_again(&cursor);
    let mut error = None;
    for _ in 0..10 {
        session.step();
        error = error.or_else(|| camera.error());
    }
    let about = "ext_image_copy_capture_cursor_session_v1".to_owned();
    assert_eq!(error, Some((about, 1)));
}

/// Windows called `wallet`, and any whose title has "Bank" in it, are
/// private; and what is blank in a picture is green, to tell it from the
/// desktop.
const PRIVATE: [(&str, &str); 2] = [
    (
        "rules.yaml",
        "rules:\n  - match: { app_id: wallet }\n    private: true\n  - match: { title: '*Bank*' }\n    private: true\n",
    ),
    ("theme.css", "capture { background: #00ff00; }"),
];

#[test]
fn a_private_window_that_closes_is_not_in_a_picture_as_it_fades() {
    let rules = PRIVATE[0];
    let theme = ("theme.css", "window { transition: 100ms linear; }\n");
    let mut session = Session::with_config(&[rules, theme]);
    let mut app = session.connect();
    let wallet = session.open(&mut app, "wallet", (200, 100), RED);
    let notes = session.open(&mut app, "notes", (200, 100), BLUE);
    session.advance(Duration::from_millis(100));
    let corner = |id: &str| {
        let frames = session.compositor().desktop().frames();
        let mut frames = frames.filter(|frame| frame.window().app_id == id);
        let at = frames.next().unwrap().geometry;
        (
            at.origin.x,
            at.origin.y,
            at.size.width.cast_signed(),
            at.size.height.cast_signed(),
        )
    };
    // Where one is and the other is not.
    let (x, y, ..) = corner("wallet");
    let at_wallet = (x + 2, y + 2);
    let (x, y, width, height) = corner("notes");
    let at_notes = (x + width - 3, y + height - 3);
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    snap(&mut session, &mut camera, &taking);

    for window in [wallet, notes] {
        window.role.destroy();
        window.xdg_surface.destroy();
        window.surface.destroy();
    }
    session.roundtrip(&mut app);
    session.advance(Duration::from_millis(50));
    let screen = session.draw();
    assert!(crate::common::between(
        screen.at(at_wallet.0, at_wallet.1),
        BACKGROUND,
        RED
    ));
    let (shot, _) = snap(&mut session, &mut camera, &taking);
    assert_eq!(
        shot.at(at_wallet.0, at_wallet.1),
        BACKGROUND,
        "gone at once"
    );
    let fading = shot.at(at_notes.0, at_notes.1);
    assert!(
        crate::common::between(fading, BACKGROUND, BLUE),
        "{fading:#x}"
    );
}

#[test]
fn a_private_window_is_blank_in_a_picture_and_not_on_the_screen() {
    let mut session = Session::with_config(&PRIVATE);
    let mut app = session.connect();
    let _wallet = session.open_decorated(&mut app, "wallet", (200, 100), RED);
    let on_screen = session.draw();
    let frame = on_screen.at(10, 5);
    assert_eq!(on_screen.at(100, 80), RED, "the user sees it as ever");

    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    let (shot, _) = snap(&mut session, &mut camera, &taking);
    assert_eq!(shot.at(100, 80), GREEN, "the window");
    assert_eq!(
        shot.at(10, 5),
        GREEN,
        "and its titlebar, which says what it is"
    );
    assert_eq!(shot.at(400, 400), BACKGROUND, "and nothing else");
    let after = session.draw();
    assert_eq!((after.at(100, 80), after.at(10, 5)), (RED, frame));

    // A picture after the first still waits for something to change:
    // what is blank is not new each time.
    let (_file, buffer) = camera.shot_buffer((WIDTH, HEIGHT));
    let _frame = ask(&camera, &taking, &buffer);
    assert_eq!(develop(&mut session, &mut camera), []);
}

#[test]
fn a_window_becomes_private_when_its_title_comes_to_say_so() {
    let mut session = Session::with_config(&PRIVATE);
    let mut app = session.connect();
    let browser = session.open(&mut app, "browser", (200, 100), RED);
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, false);
    let (shot, _) = snap(&mut session, &mut camera, &taking);
    assert_eq!(
        shot.at(100, 50),
        RED,
        "an ordinary window is in the picture"
    );

    browser.role.set_title("My Bank - statement".to_owned());
    session.roundtrip(&mut app);
    let (shot, _) = snap(&mut session, &mut camera, &taking);
    assert_eq!(shot.at(100, 50), GREEN);
    assert_eq!(session.draw().at(100, 50), RED);

    browser.role.set_title("News".to_owned());
    session.roundtrip(&mut app);
    let (shot, _) = snap(&mut session, &mut camera, &taking);
    assert_eq!(shot.at(100, 50), RED);
}

/// A program that may take pictures and is told what windows there are,
/// which is how it says which window it wants one of.
fn window_camera(session: &mut Session) -> Client {
    session.connect_as(Roles::CAPTURE.with(Roles::PANEL))
}

/// Takes a picture as large as `size` through `taking`.
fn snap_sized(
    session: &mut Session,
    client: &mut Client,
    taking: &Taking,
    size: (i32, i32),
) -> (Picture, Vec<Event>) {
    snap_as(session, client, taking, (size, Format::Xrgb8888))
}

/// The same, in a buffer of pixels of `format`.
fn snap_as(
    session: &mut Session,
    client: &mut Client,
    taking: &Taking,
    (size, format): ((i32, i32), Format),
) -> (Picture, Vec<Event>) {
    let (file, buffer) = client.shot_buffer_of(size, format);
    let frame = client.shot(taking);
    frame.attach_buffer(&buffer);
    frame.damage_buffer(0, 0, size.0, size.1);
    frame.capture();
    let events = develop(session, client);
    frame.destroy();
    (Picture::read(&file, size.0), events)
}

#[test]
fn a_picture_of_a_window_is_see_through_where_the_window_is() {
    let mut session = Session::new();
    let mut app = session.connect();
    // Half see-through red, as a buffer of a window has it: with the red
    // already made half as strong.
    let glass = 0x8080_0000;
    session.open(&mut app, "glass", (200, 100), glass);
    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("glass");
    session.roundtrip(&mut camera);
    camera.take_events();

    let see_through = ((200, 100), Format::Argb8888);
    let (shot, _) = snap_as(&mut session, &mut camera, &taking, see_through);
    assert_eq!(shot.at(100, 50), glass);
    // In a buffer that cannot be see-through it is as dark as it is red.
    let solid = camera.capture_window("glass");
    session.roundtrip(&mut camera);
    let (shot, _) = snap_as(
        &mut session,
        &mut camera,
        &solid,
        ((200, 100), Format::Xrgb8888),
    );
    assert_eq!(shot.at(100, 50), 0xff80_0000);
}

#[test]
fn a_picture_of_a_window_after_the_first_waits_for_it_to_change() {
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "app", (200, 100), RED);
    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("app");
    session.roundtrip(&mut camera);
    snap_sized(&mut session, &mut camera, &taking, (200, 100));

    let (file, buffer) = camera.shot_buffer((200, 100));
    let frame = camera.shot(&taking);
    frame.attach_buffer(&buffer);
    frame.damage_buffer(0, 0, 200, 100);
    frame.capture();
    assert_eq!(
        develop(&mut session, &mut camera),
        [],
        "nothing has changed"
    );

    app.fill(&window.surface, (200, 100), GREEN);
    session.roundtrip(&mut app);
    let events = develop(&mut session, &mut camera);
    assert_eq!(events.last(), Some(&Event::ShotReady));
    assert_eq!(Picture::read(&file, 200).at(10, 10), GREEN);
}

#[test]
fn a_picture_of_a_window_has_the_shadow_it_draws_round_itself() {
    let mut session = Session::new();
    let mut app = session.connect();
    // A window that draws a shadow ten pixels wide round what it says is
    // the window, as a program that draws its own titlebar does.
    let window = app.toplevel("shadowed", "shadowed");
    session.roundtrip(&mut app);
    window.xdg_surface.set_window_geometry(10, 10, 180, 80);
    app.fill(&window.surface, (200, 100), 0x4000_0000);
    session.roundtrip(&mut app);
    session.roundtrip(&mut app);
    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("shadowed");
    session.roundtrip(&mut camera);
    assert_eq!(camera.take_events()[0], Event::CaptureSize(200, 100));

    let see_through = ((200, 100), Format::Argb8888);
    let (shot, _) = snap_as(&mut session, &mut camera, &taking, see_through);
    assert_eq!(shot.at(0, 0), 0x4000_0000, "the shadow, faint");
    assert_eq!(shot.at(199, 99), 0x4000_0000);
}

#[test]
fn a_window_larger_than_its_screen_is_given_whole() {
    let mut session = Session::new();
    let mut app = session.connect();
    let size = (WIDTH + 160, HEIGHT + 120);
    session.open(&mut app, "large", size, RED);
    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("large");
    session.roundtrip(&mut camera);
    let large = (size.0.unsigned_abs(), size.1.unsigned_abs());
    assert_eq!(
        camera.take_events()[0],
        Event::CaptureSize(large.0, large.1)
    );

    let (shot, events) = snap_sized(&mut session, &mut camera, &taking, size);
    assert_eq!(events.last(), Some(&Event::ShotReady), "{events:?}");
    assert_eq!(shot.at(size.0 - 1, size.1 - 1), RED, "to its far corner");
}

#[test]
fn a_picture_of_one_window_is_of_that_window_and_nothing_else() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open_decorated(&mut app, "editor", (200, 100), RED);
    session.open(&mut app, "terminal", (300, 150), BLUE);
    let mut camera = window_camera(&mut session);

    let taking = camera.capture_window("editor");
    session.roundtrip(&mut camera);
    // As large as the window draws itself: the frame round it is not its.
    // It may be see-through, as a screen is not.
    let told = [
        Event::CaptureSize(200, 100),
        Event::CaptureFormat(Format::Argb8888),
        Event::CaptureFormat(Format::Xrgb8888),
        Event::CaptureKnown,
    ];
    assert_eq!(camera.take_events(), told);
    assert_eq!(
        session.draw().at(MARK.0, MARK.1),
        DOT,
        "it is marked all the same"
    );

    let before = session.draw();
    let (shot, events) = snap_sized(&mut session, &mut camera, &taking, (200, 100));
    assert_eq!(
        events,
        [Event::ShotDamage(0, 0, 200, 100), Event::ShotReady]
    );
    for (x, y) in [(0, 0), (199, 0), (0, 99), (199, 99), (100, 50)] {
        assert_eq!(shot.at(x, y), RED, "at {x}, {y}");
    }
    // The screen is as it was: the window was drawn by itself only for
    // the picture.
    let after = session.draw();
    for (x, y) in [(5, 5), (150, 60), (400, 300), (MARK.0, MARK.1)] {
        assert_eq!(after.at(x, y), before.at(x, y), "at {x}, {y}");
    }

    // Another window, another picture, of its own size.
    let other = camera.capture_window("terminal");
    session.roundtrip(&mut camera);
    assert!(camera.take_events().contains(&Event::CaptureSize(300, 150)));
    let (shot, _) = snap_sized(&mut session, &mut camera, &other, (300, 150));
    assert_eq!((shot.at(0, 0), shot.at(299, 149)), (BLUE, BLUE));
    camera.finish_capturing();
    session.roundtrip(&mut camera);
}

#[test]
fn a_program_is_told_when_its_window_changes_size_or_goes() {
    let mut session = Session::new();
    let mut app = session.connect();
    let window = session.open(&mut app, "editor", (200, 100), RED);
    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("editor");
    snap_sized(&mut session, &mut camera, &taking, (200, 100));

    app.fill(&window.surface, (240, 120), GREEN);
    session.roundtrip(&mut app);
    session.roundtrip(&mut camera);
    let events = camera.take_events();
    assert!(events.contains(&Event::CaptureSize(240, 120)), "{events:?}");
    // The next picture is all new, at the new size.
    let (shot, events) = snap_sized(&mut session, &mut camera, &taking, (240, 120));
    assert_eq!(
        events,
        [Event::ShotDamage(0, 0, 240, 120), Event::ShotReady]
    );
    assert_eq!(shot.at(239, 119), GREEN);

    window.role.destroy();
    window.xdg_surface.destroy();
    window.surface.destroy();
    session.roundtrip(&mut app);
    session.roundtrip(&mut camera);
    let events = camera.take_events();
    assert!(events.contains(&Event::CaptureStopped), "{events:?}");

    // What is asked for of a window that has gone is stopped at once.
    let _late = camera.capture_window("editor");
    session.roundtrip(&mut camera);
    assert_eq!(camera.take_events(), [Event::CaptureStopped]);
}

#[test]
fn a_window_that_is_not_shown_can_still_have_its_picture_taken() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "editor", (200, 100), RED);
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::Minimize);
    session.roundtrip(&mut app);
    assert_eq!(session.draw().at(100, 50), BACKGROUND, "it is out of sight");

    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("editor");
    let (shot, events) = snap_sized(&mut session, &mut camera, &taking, (200, 100));
    assert_eq!(events.last(), Some(&Event::ShotReady));
    assert_eq!(shot.at(100, 50), RED);
    assert_eq!(session.draw().at(100, 50), BACKGROUND, "and stays so");
}

#[test]
fn a_private_window_is_blank_in_a_picture_of_itself() {
    let mut session = Session::with_config(&PRIVATE);
    let mut app = session.connect();
    session.open(&mut app, "wallet", (200, 100), RED);
    let mut camera = window_camera(&mut session);
    let taking = camera.capture_window("wallet");
    let (shot, events) = snap_sized(&mut session, &mut camera, &taking, (200, 100));
    assert_eq!(events.last(), Some(&Event::ShotReady));
    assert_eq!((shot.at(0, 0), shot.at(199, 99)), (GREEN, GREEN));
    assert_eq!(session.draw().at(100, 50), RED);
}

#[test]
fn programs_that_want_the_same_are_given_it_from_one_drawing() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "editor", (200, 100), RED);
    let mut camera = window_camera(&mut session);
    let takings = [camera.capture(0, false), camera.capture(0, false)];
    let windows = [
        camera.capture_window("editor"),
        camera.capture_window("editor"),
    ];
    let screens = takings.map(|taking| (camera.shot_buffer((WIDTH, HEIGHT)), taking));
    let alone = windows.map(|taking| (camera.shot_buffer((200, 100)), taking));
    let mut frames = Vec::new();
    for ((_, buffer), taking) in screens.iter().chain(&alone) {
        let frame = camera.shot(taking);
        frame.attach_buffer(buffer);
        frame.capture();
        frames.push(frame);
    }
    let events = develop(&mut session, &mut camera);
    let ready = events.iter().filter(|event| **event == Event::ShotReady);
    assert_eq!(ready.count(), 4);
    for ((file, _), _) in &screens {
        assert_eq!(picture(file).at(300, 300), BACKGROUND);
    }
    for ((file, _), _) in &alone {
        assert_eq!(Picture::read(file, 200).at(199, 99), RED);
    }
}

#[test]
fn pictures_go_on_being_given_while_a_private_window_is_open() {
    let mut session = Session::with_config(&PRIVATE);
    let mut app = session.connect();
    let notes = session.open(&mut app, "notes", (200, 100), BLUE);
    session.compositor_mut().show_pointer();
    let mut camera = camera(&mut session);
    let taking = camera.capture(0, true);
    let (_, events) = snap(&mut session, &mut camera, &taking);
    assert_eq!(events.last(), Some(&Event::ShotReady), "{events:?}");

    let _wallet = session.open(&mut app, "wallet", (200, 100), RED);
    let (_, events) = snap(&mut session, &mut camera, &taking);
    assert_eq!(events.last(), Some(&Event::ShotReady), "opened: {events:?}");

    for (x, y) in [(300.0, 300.0), (310.0, 320.0), (50.0, 400.0)] {
        session.compositor_mut().input(Input::Motion { x, y }, 0);
        session.step();
        let (_, events) = snap(&mut session, &mut camera, &taking);
        assert_eq!(events.last(), Some(&Event::ShotReady), "moved: {events:?}");
    }
    app.fill(&notes.surface, (200, 100), GREEN);
    session.roundtrip(&mut app);
    let (_, events) = snap(&mut session, &mut camera, &taking);
    assert_eq!(events.last(), Some(&Event::ShotReady), "drawn: {events:?}");
}

#[test]
fn the_mark_is_under_a_panel_along_the_top_and_not_over_its_end() {
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
        Anchor, KeyboardInteractivity,
    };

    let mut session = Session::new();
    let mut client = session.connect();
    let panel = crate::common::LayerShape {
        layer: Layer::Top,
        anchor: Anchor::Top | Anchor::Left | Anchor::Right,
        size: (0, 30),
        exclusive_zone: 30,
        keyboard: KeyboardInteractivity::None,
    };
    session.open_layer(&mut client, &panel, (WIDTH, 30), BLUE);
    let mut camera = camera(&mut session);
    let _taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let screen = session.draw();
    assert_eq!(
        screen.at(MARK.0, MARK.1),
        BLUE,
        "the end of the panel shows"
    );
    assert_eq!(screen.at(MARK.0, MARK.1 + 30), DOT, "the mark is under it");
}

#[test]
fn resting_on_the_mark_says_which_program_takes_pictures_of_what() {
    let mut session = Session::new();
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let mut camera = window_camera(&mut session);
    let screen = camera.capture(0, false);
    let window = camera.capture_window("app");
    session.roundtrip(&mut camera);
    let rest = |session: &mut Session, (x, y): (i32, i32)| {
        let (x, y) = (f64::from(x), f64::from(y));
        session.compositor_mut().input(Input::Motion { x, y }, 0);
        session.step();
        session.step();
    };
    rest(&mut session, (400, 300));
    assert_eq!(
        session.compositor().mark_tooltip(),
        [""; 0],
        "not until rested on"
    );

    // Programs are called what their files are called: here, this one.
    rest(&mut session, MARK);
    let program = std::env::current_exe().unwrap();
    let program = program.file_name().unwrap().to_string_lossy().into_owned();
    let says = [
        format!("{program} is taking pictures of this screen"),
        format!("{program} is taking pictures of a window on this screen"),
    ];
    assert_eq!(session.compositor().mark_tooltip(), says);
    let picture = session.draw();
    assert_ne!(
        picture.at(MARK.0, MARK.1 + 20),
        BACKGROUND,
        "under the mark"
    );

    rest(&mut session, (400, 300));
    assert_eq!(session.compositor().mark_tooltip(), [""; 0]);
    assert_eq!(session.draw().at(MARK.0, MARK.1 + 20), BACKGROUND);

    // Once they have stopped, for as long as the mark stays.
    screen.destroy();
    window.destroy();
    session.roundtrip(&mut camera);
    rest(&mut session, MARK);
    let said = ["Pictures of this screen were taken a moment ago"];
    assert_eq!(session.compositor().mark_tooltip(), said);
}
