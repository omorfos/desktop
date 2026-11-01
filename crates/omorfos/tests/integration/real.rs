//! Real programs kept through changes of compositor (ADR 61, ADR 62).
//!
//! Each starts a program as a user would, through the relay, and starts the
//! compositor anew under it several times, checking each time that the
//! program is kept and draws again for the next. They need the programs
//! installed, run for seconds each, and depend on how each program behaves,
//! so they are not among the tests every change runs: they are run by hand,
//! and in CI, as `docs/TESTING.md` says, and a program that is not
//! installed is passed over, said to be, unless `OMORFOS_REAL` is
//! `required`.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::io::{Write, stderr};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use omorfos_config::Action;
use omorfos_ipc::{Client, Reply, Request};
use rustix::process::{Pid, Signal, kill_process};

use crate::common::Session;
use crate::hold::Holder;
use crate::relayed::Relayed;
use crate::windows::BACKGROUND;

/// How many times the compositor is started anew under each program.
const CHANGES: usize = 3;

/// How long a program is waited for, to come up or to draw again.
const WAITING: Duration = Duration::from_secs(60);

/// How large the screen is: larger than any of the programs' first windows,
/// so that one made as large as the screen is seen to be.
const SCREEN: (i32, i32) = (1920, 1200);

/// Where on the screen no program's first window reaches, and a window as
/// large as the screen does: its bottom right.
const CORNER: (i32, i32) = (SCREEN.0 - 20, SCREEN.1 - 20);

/// Says that `what` is not installed, and is not tried: or, where
/// `OMORFOS_REAL` is `required`, as in CI, which installs each, fails.
fn missing(what: &str) {
    let required = std::env::var_os("OMORFOS_REAL").is_some_and(|real| real == "required");
    assert!(
        !required,
        "{what} is not installed, and every program is required"
    );
    let _ = writeln!(stderr(), "{what} is not installed, and is not tried");
}

/// Whether `program` is installed, where `PATH` finds it.
fn installed(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|folder| folder.join(program).is_file())
    })
}

/// A compositor started under `relayed`, with the screen as large as
/// [`SCREEN`], and something that asks it what it has.
fn compositor(relayed: &Relayed) -> (Session, Holder, Client) {
    let (mut session, holder) = relayed.compositor_with(|session| session.resize_screen(SCREEN));
    let path = session.listen();
    let peer = Client::connect(&path).unwrap();
    (session, holder, peer)
}

/// How many windows the compositor of `session` has.
fn windows(session: &mut Session, peer: &mut Client) -> usize {
    match session.ask(peer, &Request::State) {
        Reply::State(state) => state.windows.len(),
        other => unreachable!("expected the state, got {other:?}"),
    }
}

/// Runs `session`, with its clock moving and frames shown, until `done`
/// holds, for no longer than [`WAITING`]; says whether it held.
fn until(session: &mut Session, mut done: impl FnMut(&mut Session) -> bool) -> bool {
    let begun = Instant::now();
    while begun.elapsed() < WAITING {
        if done(session) {
            return true;
        }
        // A program draws its next frame once told the last was shown.
        drop(session.draw());
        session.send_frames(begun.elapsed());
        session.advance(Duration::from_millis(16));
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

/// Starts `script` under `sh`, on a bus of its own, as a program that
/// connects to `relayed`'s socket, in `home`, writing what it says to
/// `said` there.
fn start(relayed: &Relayed, home: &Path, script: &str) -> Child {
    let said = fs::File::create(home.join("said")).unwrap();
    let mut command = Command::new("dbus-run-session");
    command
        .args(["--", "sh", "-c", script])
        .env("WAYLAND_DISPLAY", &relayed.path)
        .env_remove("WAYLAND_SOCKET")
        .env_remove("DISPLAY")
        .env("XDG_RUNTIME_DIR", relayed.path.parent().unwrap())
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_STATE_HOME", home.join("state"))
        // Drawn in memory, as the test's compositor shows what is.
        .env("GSK_RENDERER", "cairo")
        .env("LIBGL_ALWAYS_SOFTWARE", "1")
        .env("MOZ_ENABLE_WAYLAND", "1")
        .env("MOZ_CRASHREPORTER_DISABLE", "1")
        .env("QT_QPA_PLATFORM", "wayland")
        .env("GDK_BACKEND", "wayland")
        .stdin(Stdio::null())
        .stdout(said.try_clone().unwrap())
        .stderr(said)
        .process_group(0);
    command.spawn().unwrap()
}

/// Stops every process connected to `display` as its Wayland display,
/// whatever started it: a bus of its own leaves its portals behind.
fn reap(display: &Path) {
    let wanted = format!("WAYLAND_DISPLAY={}", display.display());
    let processes = fs::read_dir("/proc").into_iter().flatten().flatten();
    for process in processes {
        let pid = process
            .file_name()
            .to_str()
            .and_then(|pid| pid.parse().ok());
        let Some(pid) = pid.and_then(Pid::from_raw) else {
            continue;
        };
        let environ = fs::read(process.path().join("environ")).unwrap_or_default();
        if environ
            .split(|byte| *byte == 0)
            .any(|set| set == wanted.as_bytes())
        {
            let _ = kill_process(pid, Signal::KILL);
        }
    }
}

/// Checks that the program the one window in `session` is of draws, as
/// it is told to be as large as the screen, and makes it as it was; says
/// what `said` says it said if not.
fn draws(session: &mut Session, peer: &mut Client, which: &str, said: &dyn Fn() -> String) {
    let focused = match session.ask(peer, &Request::State) {
        Reply::State(state) => state.windows.iter().any(|window| window.focused),
        other => unreachable!("expected the state, got {other:?}"),
    };
    assert!(focused, "{which} has the keyboard");
    let (x, y) = CORNER;
    assert_eq!(
        session.draw().at(x, y),
        BACKGROUND,
        "{which} smaller at first"
    );
    session.compositor_mut().perform(&Action::Maximize);
    let drew = until(session, |session| session.draw().at(x, y) != BACKGROUND);
    assert!(drew, "{which} drew as large as the screen: {}", said());
    session.compositor_mut().perform(&Action::Maximize);
    let back = until(session, |session| session.draw().at(x, y) == BACKGROUND);
    assert!(back, "{which} drew as it was: {}", said());
}

/// Starts `program` with `script` and the compositor anew under it
/// [`CHANGES`] times, checking each time that it is kept, and draws again
/// as the next tells it to be as large as the screen.
fn kept_through_changes(program: &str, script: &str) {
    if !installed(program) {
        missing(program);
        return;
    }
    let home = tempfile::TempDir::new().unwrap();
    let relayed = Relayed::new();
    let (mut session, mut holder, mut peer) = compositor(&relayed);
    let mut child = start(&relayed, home.path(), script);
    let said = || fs::read_to_string(home.path().join("said")).unwrap_or_default();
    let up = until(&mut session, |session| windows(session, &mut peer) > 0);
    assert!(up, "{program} came up: {}", said());
    draws(&mut session, &mut peer, program, &said);
    let connected = relayed.relay.borrow().programs();
    for change in 1..=CHANGES {
        drop((session, holder, peer));
        relayed.step();
        relayed.step();
        (session, holder, peer) = compositor(&relayed);
        let back = until(&mut session, |session| windows(session, &mut peer) > 0);
        let relay = String::from_utf8_lossy(&relayed.said.borrow()).into_owned();
        assert!(
            back,
            "{program} back after change {change}: {relay}{}",
            said()
        );
        assert_eq!(
            relayed.relay.borrow().programs(),
            connected,
            "kept: {relay}"
        );
        assert!(
            child.try_wait().unwrap().is_none(),
            "{program} running: {}",
            said()
        );
        draws(
            &mut session,
            &mut peer,
            &format!("{program}, after change {change}"),
            &said,
        );
    }
    drop((session, holder, peer));
    let _ = child.kill();
    let _ = child.wait();
    reap(&relayed.path);
}

#[test]
#[ignore = "runs a real program; run by hand, as docs/TESTING.md says"]
fn foot_is_kept_through_changes_of_compositor() {
    kept_through_changes("foot", "exec foot --override=colors.alpha=1.0");
}

#[test]
#[ignore = "runs a real program; run by hand, as docs/TESTING.md says"]
fn firefox_is_kept_through_changes_of_compositor() {
    kept_through_changes(
        "firefox-esr",
        "exec firefox-esr --no-remote --profile \"$XDG_CONFIG_HOME\" about:blank",
    );
}

#[test]
#[ignore = "runs a real program; run by hand, as docs/TESTING.md says"]
fn a_gtk_program_is_kept_through_changes_of_compositor() {
    kept_through_changes(
        "zenity",
        "exec zenity --text-info --filename=/etc/hostname --title=kept",
    );
}

#[test]
#[ignore = "runs a real program; run by hand, as docs/TESTING.md says"]
fn a_qt_program_in_a_flatpak_is_kept_through_changes_of_compositor() {
    // Its own data, made for the test inside the program's, which is all
    // of the user's the sandbox lets it have, keeps the user's account out
    // of it: it shows its first screen and no more. It is removed after.
    let data = Path::new(&std::env::var_os("HOME").unwrap()).join(".var/app/org.telegram.desktop");
    let kept = data.join("kept");
    let script = format!(
        "mkdir -p '{kept}' && exec flatpak run --env=QT_QPA_PLATFORM=wayland \
         org.telegram.desktop -workdir '{kept}/'",
        kept = kept.display()
    );
    let installed = Command::new("flatpak")
        .args(["info", "org.telegram.desktop"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if installed {
        kept_through_changes("flatpak", &script);
    } else {
        missing("org.telegram.desktop");
    }
    let _ = fs::remove_dir_all(kept);
}
