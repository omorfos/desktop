//! The wallpaper, as a client of a real compositor.
//!
//! How a picture is read and laid on a screen is tested with the wallpaper
//! itself. Here it is the program: that what the configuration names gets
//! behind everything on each screen, and keeps up with the configuration,
//! the screens and the time.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::cell::RefCell;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omorfos::Scale;
use omorfos_wallpaper::Wallpaper;
use smithay::reexports::wayland_server::ListeningSocket;

use crate::common::{HEIGHT, Picture, Session, WIDTH};
use crate::windows::{BACKGROUND, BLUE, RED};

const GREEN: u32 = 0xff00_ff00;
const WHITE: u32 = 0xffff_ffff;

/// A wallpaper in the session, with only the role a panel has, whose home
/// directory is `home`.
fn joined(session: &mut Session, home: Option<&Path>) -> Wallpaper {
    keeping(session, None, home)
}

/// A wallpaper in the session as [`joined`] is, which keeps what it draws
/// in `cache`.
fn keeping(session: &mut Session, cache: Option<&Path>, home: Option<&Path>) -> Wallpaper {
    let (ours, theirs) = UnixStream::pair().unwrap();
    session
        .compositor_mut()
        .insert_client(ours, omorfos::Roles::PANEL)
        .unwrap();
    let config = session.config.path().to_owned();
    let (cache, home) = (cache.map(Path::to_owned), home.map(Path::to_owned));
    Wallpaper::new(theirs, &config, cache, home, 0).unwrap()
}

/// Lets the wallpaper and the compositor talk, `now` into the wallpaper's
/// running, until they have nothing more to say to each other.
fn settle(session: &mut Session, wallpaper: &mut Wallpaper, now: Duration) {
    for _ in 0..20 {
        wallpaper.turn(now).unwrap();
        session.step();
    }
}

/// Writes a PNG at `path` of pixels `width` by `height`, each as `color`
/// says from where it is.
fn png(path: &Path, (width, height): (u32, u32), color: impl Fn(u32, u32) -> u32) {
    let pixels = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));
    let bytes: Vec<u8> = pixels
        .flat_map(|(x, y)| {
            let [_, red, green, blue] = color(x, y).to_be_bytes();
            [red, green, blue]
        })
        .collect();
    let file = std::io::BufWriter::new(std::fs::File::create(path).unwrap());
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgb);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&bytes).unwrap();
}

/// A picture in four colours, a quarter each: red at the top left, green
/// at the top right, blue at the bottom left and white at the bottom right.
fn quarters(path: &Path) {
    png(path, (2, 2), |x, y| {
        [[RED, GREEN], [BLUE, WHITE]][y as usize][x as usize]
    });
}

/// Whether the screen's four corners are, from the top left, these.
fn corners(picture: &Picture, (width, height): (i32, i32)) -> [u32; 4] {
    let (right, bottom) = (width - 1, height - 1);
    [(0, 0), (right, 0), (0, bottom), (right, bottom)].map(|(x, y)| picture.at(x, y))
}

/// A configuration that names a picture, and how to lay it.
fn picture_config(picture: &str, rest: &str) -> String {
    format!("wallpaper: {{ picture: '{picture}', {rest} }}\n")
}

#[test]
fn without_a_picture_or_a_colour_the_desktop_shows_and_nothing_is_put_up() {
    let mut session = Session::new();
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [BACKGROUND; 4]);
    assert!(wallpaper.deadline().is_none());
    assert_eq!(wallpaper.problems(), [] as [String; 0]);
}

#[test]
fn a_colour_covers_every_screen_and_windows_are_over_it() {
    let config = "wallpaper: { color: '#ff0000' }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.add_output((800, 600));
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [RED; 4]);
    assert_eq!(corners(&session.draw_on(1), (800, 600)), [RED; 4]);

    let mut client = session.connect();
    session.open_decorated(&mut client, "app", (200, 100), BLUE);
    let picture = session.draw();
    let places = (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| (x, y)));
    let shown = places.filter(|(x, y)| picture.at(*x, *y) == BLUE);
    assert!(shown.count() > 100 * 50);
}

#[test]
fn a_picture_is_laid_on_every_screen_as_the_configuration_says() {
    let pictures = tempfile::TempDir::new().unwrap();
    let path = pictures.path().join("quarters.png");
    quarters(&path);
    let config = picture_config(&path.to_string_lossy(), "fit: stretch");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    // Two screens of different shapes, from one picture read once.
    session.add_output((300, 600));
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    let colours = [RED, GREEN, BLUE, WHITE];
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), colours);
    assert_eq!(corners(&session.draw_on(1), (300, 600)), colours);
    assert_eq!(wallpaper.problems(), [] as [String; 0]);
}

#[test]
fn a_photograph_in_the_home_directory_fills_the_screen() {
    let home = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(home.path().join("Pictures")).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../omorfos-wallpaper/testdata");
    // Sixteen by eight, red on the left and blue on the right.
    let jpeg = std::fs::read(fixture.join("red-blue.jpg")).unwrap();
    std::fs::write(home.path().join("Pictures/hills.jpg"), jpeg).unwrap();
    let config = picture_config("~/Pictures/hills.jpg", "fit: fill");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = joined(&mut session, Some(home.path()));
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    let picture = session.draw();
    // As tall as the screen, and so twice as wide as it, its sides cut off:
    // the middle is where red meets blue, and nothing else shows.
    let reddish = |x: i32| {
        let pixel = picture.at(x, HEIGHT / 2);
        (pixel >> 16) & 0xff > 200 && pixel & 0xff < 60
    };
    let bluish = |x: i32| {
        let pixel = picture.at(x, HEIGHT / 2);
        pixel & 0xff > 200 && (pixel >> 16) & 0xff < 60
    };
    assert!(reddish(0) && reddish(WIDTH / 2 - 40));
    assert!(bluish(WIDTH - 1) && bluish(WIDTH / 2 + 40));
}

#[test]
fn a_screen_named_in_the_configuration_shows_its_own() {
    let config =
        "wallpaper:\n  color: '#ff0000'\n  screens: [{ name: test-1, color: '#0000ff' }]\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    session.add_output((800, 600));
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [RED; 4]);
    assert_eq!(corners(&session.draw_on(1), (800, 600)), [BLUE; 4]);
}

#[test]
fn a_picture_shown_as_it_is_has_a_pixel_to_each_of_the_screens_at_any_scale() {
    let pictures = tempfile::TempDir::new().unwrap();
    let path = pictures.path().join("red.png");
    png(&path, (4, 4), |_, _| RED);
    let config = picture_config(&path.to_string_lossy(), "fit: center, color: '#0000ff'");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    session.suggest_scale(Scale::new(2.0));
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    let picture = session.draw();
    let (middle, row) = (WIDTH / 2, HEIGHT / 2);
    let across: Vec<u32> = (middle - 3..middle + 3)
        .map(|x| picture.at(x, row))
        .collect();
    assert_eq!(across, [BLUE, RED, RED, RED, RED, BLUE]);
}

#[test]
fn the_pictures_of_a_directory_are_shown_in_turn() {
    let pictures = tempfile::TempDir::new().unwrap();
    png(&pictures.path().join("a.png"), (1, 1), |_, _| RED);
    png(&pictures.path().join("b.png"), (1, 1), |_, _| GREEN);
    let config = picture_config(&pictures.path().to_string_lossy(), "every: 5, fade: 0");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(session.draw().at(10, 10), RED);
    // The next is made ready half the time before it is due.
    assert_eq!(wallpaper.deadline(), Some(Duration::from_millis(2500)));
    settle(&mut session, &mut wallpaper, Duration::from_secs(4));
    assert_eq!(session.draw().at(10, 10), RED, "not yet");
    assert_eq!(wallpaper.deadline(), Some(Duration::from_secs(5)));
    settle(&mut session, &mut wallpaper, Duration::from_secs(5));
    assert_eq!(session.draw().at(10, 10), GREEN);
    settle(&mut session, &mut wallpaper, Duration::from_secs(10));
    assert_eq!(session.draw().at(10, 10), RED);
}

#[test]
fn a_picture_that_cannot_be_read_is_said_and_the_colour_shown() {
    let config = picture_config("/no/such/picture.png", "color: '#0000ff'");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    session.add_output((800, 600));
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [BLUE; 4]);
    // Said once, though two screens could not show it.
    let problems = wallpaper.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("cannot show `/no/such/picture.png`"),
        "{problems:?}"
    );
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(wallpaper.problems(), [] as [String; 0], "and not again");

    // Named from a home there is none of, it is not there at all: the
    // colour alone is shown.
    let config = picture_config("~/picture.png", "color: '#ff0000'");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [RED; 4]);
    assert_eq!(wallpaper.problems(), [] as [String; 0]);
}

#[test]
fn the_wallpaper_follows_the_configuration_as_it_is_saved() {
    let config = "wallpaper: { color: '#ff0000', fade: 0 }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    let program = RefCell::new(joined(&mut session, None));
    settle(&mut session, &mut program.borrow_mut(), Duration::ZERO);
    assert_eq!(session.draw().at(10, 10), RED);

    let turned = |session: &mut Session, colour: u32| {
        program.borrow_mut().turn(Duration::ZERO).unwrap();
        session.step();
        session.draw().at(10, 10) == colour
    };
    session.write_config("config.yaml", "wallpaper: { color: '#0000ff', fade: 0 }\n");
    session.eventually(|session| turned(session, BLUE));
    // With nothing to show, it takes itself off the screen.
    session.write_config("config.yaml", "");
    session.eventually(|session| turned(session, BACKGROUND));
}

#[test]
fn a_screen_that_goes_takes_its_wallpaper_with_it() {
    let config = "wallpaper: { color: '#ff0000' }\n";
    let mut session = Session::with_config(&[("config.yaml", config)]);
    let other = session.add_output((800, 600));
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [RED; 4]);
    session.compositor_mut().remove_output(&other);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [RED; 4]);
    // And one plugged in gets one of its own. The harness still counts the
    // one that went.
    session.add_output((300, 200));
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw_on(2), (300, 200)), [RED; 4]);
}

#[test]
fn the_wallpaper_runs_by_itself_in_the_session_it_is_started_in() {
    let mut session = Session::new();
    let runtime = session.config.path().to_owned();
    let socket = ListeningSocket::bind_absolute(runtime.join("wayland-test")).unwrap();
    session
        .inner()
        .serve(socket, omorfos::Roles::PANEL)
        .unwrap();
    // The configuration is looked for where the session would have it,
    // and names a directory whose first picture cannot be read.
    let pictures = runtime.join("pictures");
    std::fs::create_dir_all(runtime.join("omorfos")).unwrap();
    std::fs::create_dir(&pictures).unwrap();
    std::fs::write(pictures.join("a.png"), "not a picture").unwrap();
    png(&pictures.join("b.png"), (1, 1), |_, _| GREEN);
    let config: PathBuf = runtime.join("omorfos/config.yaml");
    let named = picture_config(&pictures.to_string_lossy(), "color: '#ff0000', every: 5");
    std::fs::write(&config, named).unwrap();

    let wallpaper = std::thread::spawn(move || {
        let runtime = Some(runtime.as_os_str());
        let display = Some(std::ffi::OsStr::new("wayland-test"));
        omorfos_wallpaper::start(runtime, display, runtime, runtime, None)
    });

    session.eventually(|session| session.draw().at(10, 10) == RED);
    // It keeps up without being prodded: asleep until the configuration
    // changes.
    std::fs::write(&config, "wallpaper: { color: '#0000ff' }\n").unwrap();
    session.eventually(|session| session.draw().at(10, 10) == BLUE);

    // It ends with the session, saying why.
    drop(session);
    let failure = wallpaper.join().unwrap();
    assert_ne!(failure.to_string(), "");
}

#[test]
fn the_wallpaper_says_why_when_there_is_no_session_where_it_is_told() {
    let directory = tempfile::TempDir::new().unwrap();
    let runtime = Some(directory.path().as_os_str());
    let display = Some(std::ffi::OsStr::new("wayland-none"));
    let failure = omorfos_wallpaper::start(runtime, display, runtime, runtime, None);
    assert_ne!(failure.to_string(), "");
}

/// The one file kept in `cache`.
fn kept(cache: &Path) -> PathBuf {
    let mut files = std::fs::read_dir(cache).unwrap().flatten();
    let file = files.next().unwrap().path();
    assert!(files.next().is_none());
    file
}

#[test]
fn what_a_screen_was_given_is_given_again_at_once_in_the_next_session() {
    let pictures = tempfile::TempDir::new().unwrap();
    let cache = tempfile::TempDir::new().unwrap();
    let path = pictures.path().join("quarters.png");
    quarters(&path);
    let config = picture_config(&path.to_string_lossy(), "fit: stretch");
    let colours = [RED, GREEN, BLUE, WHITE];
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = keeping(&mut session, Some(cache.path()), None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), colours);
    drop((wallpaper, session));

    // Its last pixel made black, what was kept is told from what would be
    // drawn: four bytes each, blue, green, red and opacity.
    let file = kept(cache.path());
    let mut bytes = std::fs::read(&file).unwrap();
    let end = bytes.len();
    bytes[end - 4..].copy_from_slice(&[0, 0, 0, 0xff]);
    std::fs::write(&file, bytes).unwrap();
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = keeping(&mut session, Some(cache.path()), None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    let shown = corners(&session.draw(), (WIDTH, HEIGHT));
    assert_eq!(shown, [RED, GREEN, BLUE, 0xff00_0000]);
    drop((wallpaper, session));

    // Once the picture changes, it is read again.
    png(&path, (3, 3), |_, _| GREEN);
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = keeping(&mut session, Some(cache.path()), None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT)), [GREEN; 4]);
    assert_eq!(wallpaper.problems(), [] as [String; 0]);
}

#[test]
fn what_cannot_be_kept_is_said_and_shown_even_so() {
    let pictures = tempfile::TempDir::new().unwrap();
    let path = pictures.path().join("quarters.png");
    quarters(&path);
    let config = picture_config(&path.to_string_lossy(), "fit: stretch");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    // Where a directory would be, there is a file.
    let cache = path.join("cache");
    let mut wallpaper = keeping(&mut session, Some(&cache), None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(corners(&session.draw(), (WIDTH, HEIGHT))[0], RED);
    let problems = wallpaper.problems();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("cannot keep what"), "{problems:?}");
}

#[test]
fn a_directory_starts_from_the_picture_kept_from_the_session_before() {
    let pictures = tempfile::TempDir::new().unwrap();
    let cache = tempfile::TempDir::new().unwrap();
    png(&pictures.path().join("a.png"), (1, 1), |_, _| RED);
    png(&pictures.path().join("b.png"), (1, 1), |_, _| GREEN);
    png(&pictures.path().join("c.png"), (1, 1), |_, _| BLUE);
    let config = picture_config(&pictures.path().to_string_lossy(), "every: 5, fade: 0");
    let next = |cache: &Path| {
        let mut session = Session::with_config(&[("config.yaml", &config)]);
        let mut wallpaper = keeping(&mut session, Some(cache), None);
        settle(&mut session, &mut wallpaper, Duration::ZERO);
        session.draw().at(10, 10)
    };
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = keeping(&mut session, Some(cache.path()), None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(session.draw().at(10, 10), RED);
    // The next picture, so soon after, is not kept in its place.
    settle(&mut session, &mut wallpaper, Duration::from_secs(5));
    assert_eq!(session.draw().at(10, 10), GREEN);
    assert_eq!(next(cache.path()), RED);
    // Half an hour after the first, the one shown then is.
    settle(&mut session, &mut wallpaper, Duration::from_mins(30));
    assert_eq!(session.draw().at(10, 10), BLUE);
    assert_eq!(next(cache.path()), BLUE);
    // Nor is one that has gone started from.
    std::fs::remove_file(pictures.path().join("c.png")).unwrap();
    assert_eq!(next(cache.path()), RED);
}

#[test]
fn the_next_picture_of_a_directory_is_ready_before_it_is_due() {
    let pictures = tempfile::TempDir::new().unwrap();
    png(&pictures.path().join("a.png"), (1, 1), |_, _| RED);
    png(&pictures.path().join("b.png"), (1, 1), |_, _| GREEN);
    // And another screen that only ever shows its colour, which is not
    // made ready for anything.
    let rest = "every: 10, fade: 0, screens: [{ name: test-2, color: '#0000ff' }]";
    let config = picture_config(&pictures.path().to_string_lossy(), rest);
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    session.add_output((300, 200));
    session.add_output((200, 100));
    let mut wallpaper = joined(&mut session, None);
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(wallpaper.deadline(), Some(Duration::from_secs(5)));
    settle(&mut session, &mut wallpaper, Duration::from_secs(5));
    assert_eq!(session.draw().at(10, 10), RED);
    assert_eq!(wallpaper.deadline(), Some(Duration::from_secs(10)));
    // Read already, it is shown though it is gone by the time it is due.
    std::fs::remove_file(pictures.path().join("b.png")).unwrap();
    settle(&mut session, &mut wallpaper, Duration::from_secs(10));
    assert_eq!(session.draw().at(10, 10), GREEN);
    assert_eq!(session.draw_on(1).at(10, 10), GREEN);
    assert_eq!(session.draw_on(2).at(10, 10), BLUE);
    assert_eq!(wallpaper.problems(), [] as [String; 0]);
}

#[test]
fn one_picture_fades_into_the_next() {
    let pictures = tempfile::TempDir::new().unwrap();
    png(&pictures.path().join("a.png"), (1, 1), |_, _| RED);
    png(&pictures.path().join("b.png"), (1, 1), |_, _| GREEN);
    let config = picture_config(&pictures.path().to_string_lossy(), "every: 5, fade: 1000");
    let mut session = Session::with_config(&[("config.yaml", &config)]);
    let mut wallpaper = joined(&mut session, None);
    // The first is shown at once.
    settle(&mut session, &mut wallpaper, Duration::ZERO);
    assert_eq!(session.draw().at(10, 10), RED);
    let at = |session: &mut Session, wallpaper: &mut Wallpaper, millis| {
        settle(session, wallpaper, Duration::from_millis(millis));
        let [_, red, green, _] = session.draw().at(10, 10).to_be_bytes();
        (red, green)
    };
    assert_eq!(at(&mut session, &mut wallpaper, 5000), (255, 0), "not yet");
    assert_eq!(wallpaper.deadline(), Some(Duration::from_secs(6)));
    // Halfway, half of each.
    let (red, green) = at(&mut session, &mut wallpaper, 5500);
    assert!(
        red.abs_diff(128) < 8 && green.abs_diff(128) < 8,
        "{red} {green}"
    );
    let (red, green) = at(&mut session, &mut wallpaper, 5900);
    assert!(red < 20 && green > 235, "nearly: {red} {green}");
    assert_eq!(at(&mut session, &mut wallpaper, 6000), (0, 255));
    assert_eq!(session.draw().at(10, 10), GREEN);
    assert_eq!(wallpaper.deadline(), Some(Duration::from_millis(7500)));

    // A change of scale halfway shows what it was fading to at once, and
    // what follows at once too, at the new size.
    at(&mut session, &mut wallpaper, 10_000);
    let (red, _) = at(&mut session, &mut wallpaper, 10_500);
    assert!(red > 100, "{red}");
    session.suggest_scale(Scale::new(2.0));
    assert_eq!(at(&mut session, &mut wallpaper, 10_500), (255, 0));
    assert_eq!(wallpaper.problems(), [] as [String; 0]);
}
