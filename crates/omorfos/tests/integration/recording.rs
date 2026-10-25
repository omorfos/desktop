//! What records from a microphone or a camera, shown on the mark, as the
//! watcher of what records says it. A script stands in for the watcher,
//! saying what it is told to, of this process.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::path::Path;
use std::time::Duration;

use omorfos::{Input, Roles};

use crate::common::{Session, WIDTH, runnable};
use crate::windows::BACKGROUND;

/// The colour the signs of the mark are drawn in.
const INK: u32 = 0xfff0_524f;

/// A session whose watcher says, of the process the tests run in, that it
/// records from the microphone, the sound that plays and the camera, and is
/// shown a screen it shares itself; then, once `next` is there, nothing;
/// and, once `stop` is, stops.
fn watched() -> Session {
    watched_until(false)
}

/// The same, or, if it `ends` at once, a watcher that stops as soon as it
/// has said what records.
fn watched_until(ends: bool) -> Session {
    let mut session = Session::new();
    session.listen();
    let folder = session.config.path().to_owned();
    let long = "1".repeat(300);
    let script = format!(
        "#!/bin/sh\n\
         p=$PPID\n\
         printf 'microphone %s\\nsound %s\\ncamera %s\\nscreen %s %s\\n' $p $p $p $p $p\n\
         printf 'camera 4000000000\\n'\n\
         printf 'something else\\nmicrophone none\\nmicrophone {long}\\n.\\n'\n\
         {end}\n\
         while [ -d '{folder}' ] && [ ! -e '{folder}/next' ]; do sleep 0.01; done\n\
         printf 'camera %s\\n.\\n' $p\n\
         while [ -d '{folder}' ] && [ ! -e '{folder}/stop' ]; do sleep 0.01; done\n",
        folder = folder.display(),
        end = if ends { "exit 0" } else { ":" },
    );
    let watcher = folder.join("watcher");
    runnable(&watcher, &script);
    session.compositor_mut().set_recorder(watcher);
    session.compositor_mut().autostart();
    session
}

/// What the mark says with the pointer on it.
fn says(session: &mut Session, (x, y): (i32, i32)) -> Vec<String> {
    let (x, y) = (f64::from(x), f64::from(y));
    session.compositor_mut().input(Input::Motion { x, y }, 0);
    session.step();
    session.compositor().mark_tooltip().to_vec()
}

fn told(folder: &Path, what: &str) {
    std::fs::write(folder.join(what), "").unwrap();
}

#[test]
fn a_microphone_or_a_camera_recorded_from_is_shown_on_the_mark() {
    let mut session = watched();
    // A microphone and a camera, side by side in the corner, where there
    // is no picture being taken.
    let (top, left) = (8, WIDTH - 8 - 40);
    session.eventually(|session| session.draw().at(left + 4 + 7, top + 5) == INK);
    let picture = session.draw();
    assert_eq!(picture.at(left + 20 + 5, top + 8), INK, "the camera");
    assert_eq!(picture.at(left - 2, top + 8), BACKGROUND, "and no more");

    let program = std::env::current_exe().unwrap();
    let program = program.file_name().unwrap().to_string_lossy().into_owned();
    let recording = [
        format!("{program} is recording from the microphone"),
        format!("{program} is recording the sound that plays"),
        format!("{program} is using the camera"),
        // One whose process is not there to be named.
        "A program is using the camera".to_owned(),
    ];
    assert_eq!(says(&mut session, (left + 10, top + 8)), recording);

    // Shown a screen it shares itself: it is named for both.
    let mut camera = session.connect_as(Roles::CAPTURE);
    let _taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let wide = WIDTH - 8 - 56;
    let mut shown = vec![format!("{program} is shown this screen, through {program}")];
    shown.extend(recording);
    assert_eq!(says(&mut session, (wide + 10, top + 8)), shown);
}

#[test]
fn what_stopped_recording_stays_on_the_mark_a_moment() {
    let mut session = watched();
    let folder = session.config.path().to_owned();
    let (top, left) = (8, WIDTH - 8 - 40);
    session.eventually(|session| session.draw().at(left + 4 + 7, top + 5) == INK);

    // Then only the camera records: sound stays a moment, and goes.
    told(&folder, "next");
    let program = std::env::current_exe().unwrap();
    let program = program.file_name().unwrap().to_string_lossy().into_owned();
    let camera = format!("{program} is using the camera");
    let ago = [camera.clone(), "Sound was recorded a moment ago".to_owned()];
    session.eventually(|session| says(session, (left + 10, top + 8)) == ago);
    session.advance(Duration::from_millis(1600));
    let one = WIDTH - 8 - 24;
    assert_eq!(
        session.draw().at(one + 4 + 5, top + 8),
        INK,
        "the camera alone"
    );
    assert_eq!(says(&mut session, (one + 10, top + 8)), [camera]);

    // A watcher that stops has nothing recording that it knows of.
    told(&folder, "stop");
    let ago = ["The camera was used a moment ago"];
    session.eventually(|session| says(session, (one + 10, top + 8)) == ago);
    session.advance(Duration::from_millis(1600));
    assert_eq!(
        session.draw().at(one + 4 + 5, top + 8),
        BACKGROUND,
        "then nothing"
    );
}

#[test]
fn a_watcher_that_stops_leaves_what_it_said_on_the_mark_a_moment() {
    let mut session = watched_until(true);
    let (top, left) = (8, WIDTH - 8 - 40);
    let ago = [
        "Sound was recorded a moment ago",
        "The camera was used a moment ago",
    ];
    session.eventually(|session| says(session, (left + 10, top + 8)) == ago);
    session.advance(Duration::from_millis(1600));
    assert_eq!(session.draw().at(left + 4 + 7, top + 5), BACKGROUND);
}
