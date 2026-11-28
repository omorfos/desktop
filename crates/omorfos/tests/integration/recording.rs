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
use omorfos_ipc::Captured;

use crate::common::{Event, Session, WIDTH, runnable};
use crate::windows::BACKGROUND;

/// The colour the signs of the mark are drawn in.
const INK: u32 = 0xfff8_616d;

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

/// A session whose watcher says, of the process the tests run in, that it
/// shares a screen through PipeWire with another, the watcher itself, as
/// the portal shares one with a browser; and which that is.
fn shared_through(session: &mut Session) -> (u32, std::path::PathBuf) {
    let socket = session.listen();
    let folder = session.config.path().to_owned();
    let script = format!(
        "#!/bin/sh\n\
         echo $$ > '{folder}/seer.new' && mv '{folder}/seer.new' '{folder}/seer'\n\
         printf 'screen %s %s\\n.\\n' $$ $PPID\n\
         while [ -d '{folder}' ] && [ ! -e '{folder}/stop' ]; do sleep 0.01; done\n",
        folder = folder.display(),
    );
    let watcher = folder.join("watcher");
    runnable(&watcher, &script);
    session.compositor_mut().set_recorder(watcher);
    session.compositor_mut().autostart();
    let seer = folder.join("seer");
    session.eventually(|_| seer.exists());
    let seer = std::fs::read_to_string(&seer)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    (seer, socket)
}

#[test]
fn a_screen_shared_through_the_portal_is_said_listed_and_stopped_by_what_it_is_shared_with() {
    let mut session = Session::new();
    let (seer, socket) = shared_through(&mut session);
    let mut peer = omorfos_ipc::Client::connect(&socket).unwrap();
    let name = std::fs::read_link(format!("/proc/{seer}/exe")).unwrap();
    let name = name.file_name().unwrap().to_string_lossy().into_owned();
    let mut camera = crate::capture::camera(&mut session);
    let taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    session.eventually(|session| {
        let omorfos_ipc::Reply::State(state) = session.ask(&mut peer, &omorfos_ipc::Request::State)
        else {
            unreachable!("the compositor answers a question about the state with it");
        };
        state
            .capturing
            .iter()
            .any(|capturing| capturing.pid == Some(seer))
    });
    let _ = crate::capture::snap(&mut session, &mut camera, &taking);
    let _ = crate::capture::snap(&mut session, &mut camera, &taking);
    session.step();
    // Said of what it is shared with, not of what shares it.
    let said: Vec<String> = session.compositor().events().map(str::to_owned).collect();
    let begun: Vec<&String> = said
        .iter()
        .filter(|event| event.contains("begun"))
        .collect();
    assert_eq!(begun.len(), 1, "{said:?}");
    assert!(begun[0].starts_with(&name), "{said:?}");
    camera.take_events();
    // Stopped by the number the state gives it.
    session
        .compositor_mut()
        .perform(&omorfos_config::Action::StopCapturing(seer));
    session.step();
    session.roundtrip(&mut camera);
    assert!(camera.take_events().contains(&Event::CaptureStopped));
    told(session.config.path(), "stop");
}

#[test]
fn the_state_says_what_records_the_microphone_the_sound_and_the_camera() {
    let mut session = watched();
    let socket = session.config.path().join("ipc.sock");
    let mut peer = omorfos_ipc::Client::connect(&socket).unwrap();
    let mut capturing = Vec::new();
    session.eventually(|session| {
        let omorfos_ipc::Reply::State(state) = session.ask(&mut peer, &omorfos_ipc::Request::State)
        else {
            unreachable!("the compositor answers a question about the state with it");
        };
        capturing = state.capturing;
        !capturing.is_empty()
    });
    let pid = Some(std::process::id());
    for kind in [Captured::Microphone, Captured::Sound, Captured::Camera] {
        let said = capturing
            .iter()
            .find(|capturing| capturing.what == kind && capturing.pid == pid);
        let said = said.unwrap_or_else(|| unreachable!("{kind:?} is said: {capturing:?}"));
        assert_eq!((said.screen.as_deref(), said.since), (None, None));
    }
    told(session.config.path(), "stop");
}
