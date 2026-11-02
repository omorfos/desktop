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
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use omorfos_config::Action;
use omorfos_ipc::{Client, Reply, Request};
use rustix::process::{Pid, Signal, kill_process};

use crate::common::{Session, runnable};
use crate::hold::Holder;
use crate::panel::{BAR, plain};
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

/// The components that run for the whole session, as their programs are
/// called.
const COMPONENTS: [&str; 4] = [
    "omorfos-panel",
    "omorfos-wallpaper",
    "omorfos-notifications",
    "omorfos-clipboard",
];

/// Where `program`, built with the tests, is: beside the directory the
/// tests run from.
fn built(program: &str) -> Option<PathBuf> {
    let tests = std::env::current_exe().ok()?;
    let built = tests.parent()?.parent()?.join(program);
    built.is_file().then_some(built)
}

/// The processes running `program`, the one at that path.
fn running(program: &Path) -> Vec<u32> {
    let processes = fs::read_dir("/proc").into_iter().flatten().flatten();
    let running = processes.filter(|process| {
        fs::read_link(process.path().join("exe")).is_ok_and(|exe| exe == program)
    });
    let pids = running.filter_map(|process| process.file_name().to_str()?.parse().ok());
    pids.collect()
}

/// A compositor started under `relayed` with `components`, answering over
/// IPC at `ipc`, the same for each, as it is in a session.
fn with_components(relayed: &Relayed, components: &Components, ipc: &Path) -> (Session, Holder) {
    let started = |program: &PathBuf| components.home.path().join(program.file_name().unwrap());
    let started: Vec<PathBuf> = components.programs.iter().map(started).collect();
    let (mut session, holder) = relayed.compositor_with(|session| {
        session.resize_screen(SCREEN);
        let compositor = session.compositor_mut();
        compositor.set_panel(&started[0]);
        compositor.set_wallpaper(&started[1]);
        compositor.set_notifier(&started[2]);
        compositor.set_keeper(&started[3]);
    });
    let _ = fs::remove_file(ipc);
    session.compositor_mut().listen_ipc(ipc).unwrap();
    session.compositor_mut().autostart();
    (session, holder)
}

/// The components at these paths, each started on a D-Bus of its own and
/// with its configuration, cache and data in `home`, as the user's are not
/// the test's to touch; ended with the test however it ends.
///
/// Their theme changes colours at once, so that the panel draws each state
/// it is told of once, and not as it goes from one to the next.
struct Components {
    programs: Vec<PathBuf>,
    home: tempfile::TempDir,
}

impl Components {
    /// The components at `programs`, with a script in a new `home` to start
    /// each by, of the same name.
    fn new(programs: Vec<PathBuf>) -> Self {
        let home = tempfile::TempDir::new().unwrap();
        let config = home.path().join("config/omorfos");
        fs::create_dir_all(&config).unwrap();
        fs::write(config.join("theme.css"), "* { transition: none; }\n").unwrap();
        let place = home.path().display();
        for program in &programs {
            let script = format!(
                "#!/bin/sh\nexec dbus-run-session -- env XDG_CONFIG_HOME='{place}/config' \
                 XDG_CACHE_HOME='{place}/cache' XDG_DATA_HOME='{place}/data' \
                 XDG_STATE_HOME='{place}/state' '{}'\n",
                program.display()
            );
            runnable(&home.path().join(program.file_name().unwrap()), &script);
        }
        Self { programs, home }
    }

    /// Each running once, by its process, or nothing while any is not.
    fn one_each(&self) -> Option<Vec<u32>> {
        let each = self
            .programs
            .iter()
            .map(|program| match running(program)[..] {
                [pid] => Some(pid),
                _ => None,
            });
        each.collect()
    }
}

impl Drop for Components {
    fn drop(&mut self) {
        for pid in self.programs.iter().flat_map(|program| running(program)) {
            let pid = Pid::from_raw(pid.cast_signed());
            let _ = pid.map(|pid| kill_process(pid, Signal::KILL));
        }
    }
}

/// The bottom of the screen as `session` draws it, once the panel's bar
/// is there: along the very bottom, mostly the colour of a bar with nothing
/// on it, as neither a wallpaper nor a screen not yet drawn is.
fn bar(session: &mut Session) -> Option<Vec<u32>> {
    let drawn = session.draw();
    let rows = SCREEN.1 - BAR..SCREEN.1;
    let along = |y| (0..SCREEN.0).map(move |x| (x, y));
    let strip: Vec<u32> = rows.flat_map(along).map(|(x, y)| drawn.at(x, y)).collect();
    let plain = plain();
    let bottom = (0..SCREEN.0).filter(|x| drawn.at(*x, SCREEN.1 - 1) == plain);
    let width = usize::try_from(SCREEN.0).unwrap();
    (bottom.count() > width / 2).then_some(strip)
}

/// Whether the panel `session` runs says the session is kept awake once
/// it is, and stops saying so once it is let go: as it is told over IPC.
fn told_of_staying_awake(session: &mut Session) -> bool {
    let mut before = None;
    until(session, |session| {
        before = bar(session);
        before.is_some()
    });
    let Some(before) = before else {
        return false;
    };
    session.compositor_mut().perform(&Action::StayAwake);
    let mut awake = None;
    until(session, |session| {
        awake = bar(session).filter(|bar| *bar != before);
        awake.is_some()
    });
    let Some(awake) = awake else {
        return false;
    };
    session.compositor_mut().perform(&Action::StayAwake);
    until(session, |session| {
        bar(session).is_some_and(|bar| bar != awake)
    })
}

#[test]
#[ignore = "runs the components; run by hand, as docs/TESTING.md says"]
fn the_components_are_kept_through_changes_of_compositor() {
    let programs: Option<Vec<PathBuf>> = COMPONENTS.iter().map(|name| built(name)).collect();
    let Some(programs) = programs else {
        missing("the components, built");
        return;
    };
    let components = Components::new(programs);
    let relayed = Relayed::new();
    let ipc = relayed.path.with_file_name("omorfos-ipc.sock");
    let (mut session, mut holder) = with_components(&relayed, &components, &ipc);
    let up = until(&mut session, |_| components.one_each().is_some());
    assert!(up, "each component running");
    let pids = components.one_each();
    assert!(told_of_staying_awake(&mut session), "the panel drew");
    let connected = relayed.relay.borrow().programs();
    for change in 1..=CHANGES {
        drop((session, holder));
        relayed.step();
        relayed.step();
        (session, holder) = with_components(&relayed, &components, &ipc);
        // The panel draws again, through the relay, and hears from the
        // next compositor over IPC, connecting to it again.
        assert!(
            told_of_staying_awake(&mut session),
            "the panel drew again after change {change}"
        );
        let relay = String::from_utf8_lossy(&relayed.said.borrow()).into_owned();
        assert_eq!(
            relayed.relay.borrow().programs(),
            connected,
            "kept: {relay}"
        );
        assert_eq!(
            components.one_each(),
            pids,
            "kept, not started again, after change {change}"
        );
    }
}
