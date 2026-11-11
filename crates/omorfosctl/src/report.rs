//! `omorfosctl report`: what a report of a fault needs, gathered into one
//! text for the user to look over before handing it on. It is only
//! written out, never sent anywhere.

use std::ffi::OsStr;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use omorfos_ipc::State;
use omorfos_ipc::session_log::{BEFORE, NAME};

/// The configuration's files, as the compositor reads them.
pub(crate) const CONFIGURATION: [&str; 5] = [
    "config.yaml",
    "keys.yaml",
    "rules.yaml",
    "theme.css",
    "menu.yaml",
];

/// How much of the end of each log is put in: a login's log is not
/// bounded, and what came last is what a report is for.
const LOGGED: u64 = 256 * 1024;

/// What a window's title says in a report that leaves titles out.
const NO_TITLE: &str = "(left out)";

/// Where the report reads from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Places {
    /// The configuration's directory.
    pub(crate) config: Option<PathBuf>,
    /// Where the session's logs are kept.
    pub(crate) logs: Option<PathBuf>,
    /// The user's home, whose paths are left out.
    pub(crate) home: Option<PathBuf>,
    /// Where the system is read from: `/`, but in tests.
    pub(crate) root: PathBuf,
}

impl Places {
    /// Where the environment says each is, as the compositor and the holder
    /// find them: `$XDG_CONFIG_HOME/omorfos`, or `~/.config/omorfos`, and
    /// the logs as `omorfos_ipc::session_log` has them. A relative path is
    /// passed over, as the XDG Base Directory specification says.
    pub(crate) fn of(
        config_home: Option<&OsStr>,
        state_home: Option<&OsStr>,
        home: Option<&OsStr>,
    ) -> Self {
        let absolute =
            |value: Option<&OsStr>| value.map(PathBuf::from).filter(|path| path.is_absolute());
        let config = absolute(config_home)
            .or_else(|| absolute(home).map(|home| home.join(".config")))
            .map(|base| base.join("omorfos"));
        Self {
            config,
            logs: omorfos_ipc::session_log::directory(state_home, home),
            home: absolute(home),
            root: PathBuf::from("/"),
        }
    }
}

/// Writes the report to `out`. `compositor` is what the compositor said
/// its version and state are, or why it could not be asked. Unless the
/// report is `whole`, window titles and every path in the user's home are
/// left out.
pub(crate) fn write(
    places: &Places,
    compositor: Result<(String, State), String>,
    whole: bool,
    out: &mut dyn Write,
) {
    let mut text = String::new();
    let _ = writeln!(
        text,
        "omorfos report, made by omorfosctl {}\n",
        env!("CARGO_PKG_VERSION")
    );
    let kept = if whole {
        "Window titles and paths in your home are kept, as `--whole` was given."
    } else {
        "Window titles and paths in your home are left out; `omorfosctl report \
         --whole` keeps them."
    };
    let _ = writeln!(text, "Look it over before handing it on. {kept}");

    heading(&mut text, "Versions");
    let (version, state) = match compositor {
        Ok((version, state)) => (version, Some(state)),
        Err(why) => (format!("did not answer: {why}"), None),
    };
    let _ = writeln!(text, "omorfosctl {}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(text, "compositor: {version}");
    let kernel = fs::read_to_string(places.root.join("proc/sys/kernel/osrelease"));
    let kernel = kernel.map_or_else(|_| "unknown".to_owned(), |kernel| kernel.trim().to_owned());
    let _ = writeln!(text, "kernel: {kernel}");
    let _ = writeln!(text, "system: {}", system(&places.root));

    heading(&mut text, "Graphics cards");
    for card in cards(&places.root) {
        let _ = writeln!(text, "{card}");
    }

    heading(&mut text, "Screens");
    let mut screens = Vec::new();
    if let Some(state) = &state {
        crate::cli::screens(state, &mut screens);
    }
    text.push_str(&String::from_utf8_lossy(&screens));

    heading(&mut text, "State");
    let state = state.map(|state| if whole { state } else { untitled(state) });
    let json = state.map(|state| serde_json::to_string_pretty(&state).unwrap_or_default());
    let _ = writeln!(text, "{}", json.as_deref().unwrap_or("none"));

    for file in CONFIGURATION {
        heading(&mut text, file);
        let read = places
            .config
            .as_ref()
            .map(|config| fs::read(config.join(file)));
        match read {
            Some(Ok(bytes)) => text.push_str(&String::from_utf8_lossy(&bytes)),
            _ => text.push_str("not there\n"),
        }
    }

    for (file, which) in [(NAME, "this login's"), (BEFORE, "the login before's")] {
        heading(&mut text, &format!("{file}, {which} log"));
        let logged = places
            .logs
            .as_ref()
            .and_then(|logs| end_of(&logs.join(file)));
        text.push_str(logged.as_deref().unwrap_or("not there\n"));
    }

    let home = places.home.as_ref().filter(|_| !whole);
    let text = home.map_or_else(|| text.clone(), |home| left_out(&text, home));
    let _ = out.write_all(text.as_bytes());
}

/// Starts a section of the report called `title`.
fn heading(text: &mut String, title: &str) {
    let _ = write!(text, "\n== {title}\n");
}

/// What the system says it is in `os-release`, under `root`.
fn system(root: &Path) -> String {
    let read = fs::read_to_string(root.join("etc/os-release"))
        .or_else(|_| fs::read_to_string(root.join("usr/lib/os-release")))
        .unwrap_or_default();
    let named = read
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="));
    let named = named.map(|name| name.trim_matches('"').to_owned());
    named.unwrap_or_else(|| "unknown".to_owned())
}

/// Each graphics card under `root`, by its name, its driver and the
/// numbers its maker and model have, in order of name.
fn cards(root: &Path) -> Vec<String> {
    let found = fs::read_dir(root.join("sys/class/drm"));
    let found = found
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path());
    // A card, as `card0`, and not one of its connectors, as `card0-eDP-1`.
    let card = |path: &PathBuf| {
        let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
        let number = name.strip_prefix("card").unwrap_or_default();
        !number.is_empty() && number.chars().all(|digit| digit.is_ascii_digit())
    };
    let mut cards: Vec<String> = found
        .filter(card)
        .map(|path| {
            let device = path.join("device");
            let driver = fs::read_link(device.join("driver")).unwrap_or_default();
            let driver = driver
                .file_name()
                .map_or("unknown".into(), OsStr::to_string_lossy);
            let number = |what: &str| {
                let read = fs::read_to_string(device.join(what)).unwrap_or_default();
                read.trim().to_owned()
            };
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            format!(
                "{name}: {driver}, vendor {}, device {}",
                number("vendor"),
                number("device")
            )
        })
        .collect();
    cards.sort();
    cards
}

/// `state` with each window's title left out.
fn untitled(mut state: State) -> State {
    for window in &mut state.windows {
        NO_TITLE.clone_into(&mut window.title);
    }
    state
}

/// The end of the log at `path`, as text: all of it, or as much of its end
/// as is put in, from the start of a line, saying so. Nothing if it cannot
/// be read.
fn end_of(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let length = file
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or_default();
    let from = length.saturating_sub(LOGGED);
    let mut bytes = Vec::new();
    file.seek(SeekFrom::Start(from))
        .and_then(|_| file.read_to_end(&mut bytes))
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    if from == 0 {
        return Some(text.into_owned());
    }
    let whole_lines = text.split_once('\n').map_or("", |(_, rest)| rest);
    Some(format!("(what came before is left out)\n{whole_lines}"))
}

/// `text` with each path in `home` left out, from the home's path to the
/// next space, quote or bracket, and the home itself written `~`.
pub(crate) fn left_out(text: &str, home: &Path) -> String {
    let home = home.to_string_lossy();
    let home = home.trim_end_matches('/');
    // The root as a home would leave out every path there is.
    if home.is_empty() {
        return text.to_owned();
    }
    let ends = |c: char| c.is_whitespace() || "\"'`()[]{}<>,;".contains(c);
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(home) {
        let after = &rest[at + home.len()..];
        kept.push_str(&rest[..at]);
        let next = after.chars().next();
        rest = if next == Some('/') {
            kept.push_str("~/(left out)");
            after.trim_start_matches(|c: char| !ends(c))
        } else if next.is_none_or(ends) {
            kept.push('~');
            after
        } else {
            // Another's, as `/home/me2` is beside `/home/me`.
            kept.push_str(home);
            after
        };
    }
    kept.push_str(rest);
    kept
}

#[cfg(test)]
mod tests {
    use omorfos_ipc::{Screen, Window};
    use tempfile::TempDir;

    use super::*;

    /// A system under a directory of its own: a kernel, a release, and a
    /// card with a connector, as `/proc`, `/etc` and `/sys` have them.
    fn system_at(root: &Path) {
        let write = |path: &str, text: &str| {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write("proc/sys/kernel/osrelease", "6.12.0-test\n");
        write(
            "etc/os-release",
            "NAME=Test\nPRETTY_NAME=\"Test Linux 1\"\n",
        );
        write("sys/class/drm/card0/device/vendor", "0x8086\n");
        write("sys/class/drm/card0/device/device", "0x9b41\n");
        write("sys/class/drm/card0-eDP-1/status", "connected\n");
        write("drivers/i915/.keep", "");
        std::os::unix::fs::symlink(
            root.join("drivers/i915"),
            root.join("sys/class/drm/card0/device/driver"),
        )
        .unwrap();
        write("sys/class/drm/card1/device/vendor", "0x1002\n");
    }

    fn places(root: &Path) -> Places {
        Places {
            config: Some(root.join("home/me/.config/omorfos")),
            logs: Some(root.join("home/me/.local/state/omorfos")),
            home: Some(root.join("home/me")),
            root: root.to_path_buf(),
        }
    }

    fn written(
        places: &Places,
        compositor: Result<(String, State), String>,
        whole: bool,
    ) -> String {
        let mut out = Vec::new();
        write(places, compositor, whole, &mut out);
        String::from_utf8(out).unwrap()
    }

    fn state() -> State {
        State {
            windows: vec![Window {
                id: 1,
                app_id: "foot".to_owned(),
                title: "secret plans".to_owned(),
                output: "eDP-1".to_owned(),
                workspace: 1,
                focused: true,
                shown: true,
                minimized: false,
                standing: omorfos_ipc::Standing::default(),
                handle: String::new(),
            }],
            screens: vec![Screen {
                name: "eDP-1".to_owned(),
                width: 1920,
                height: 1200,
                ..Screen::default()
            }],
            problems: vec!["a problem".to_owned()],
            ..State::default()
        }
    }

    #[test]
    fn a_report_puts_in_each_file_the_compositor_reads() {
        assert_eq!(CONFIGURATION, omorfos_config::FILES);
    }

    #[test]
    fn the_places_are_where_the_compositor_and_the_holder_find_them() {
        let os = |value: &'static str| Some(OsStr::new(value));
        let given = Places::of(os("/config"), os("/state"), os("/home/me"));
        assert_eq!(given.config, Some(PathBuf::from("/config/omorfos")));
        assert_eq!(given.logs, Some(PathBuf::from("/state/omorfos")));
        assert_eq!(given.home, Some(PathBuf::from("/home/me")));
        assert_eq!(given.root, PathBuf::from("/"));
        let homed = Places::of(os("config"), None, os("/home/me"));
        assert_eq!(
            homed.config,
            Some(PathBuf::from("/home/me/.config/omorfos"))
        );
        let nowhere = Places::of(None, None, os("me"));
        assert_eq!(
            (nowhere.config, nowhere.logs, nowhere.home),
            (None, None, None)
        );
    }

    #[test]
    fn a_report_says_what_runs_where_and_what_was_said_leaving_out_titles_and_home_paths() {
        let place = TempDir::new().unwrap();
        let root = place.path();
        system_at(root);
        let places = places(root);
        let config = places.config.clone().unwrap();
        fs::create_dir_all(&config).unwrap();
        let picture = root.join("home/me/Pictures/me.png");
        fs::write(
            config.join("config.yaml"),
            format!("wallpaper: {}\n", picture.display()),
        )
        .unwrap();
        fs::write(config.join("theme.css"), "window { color: red; }\n").unwrap();
        let logs = places.logs.clone().unwrap();
        fs::create_dir_all(&logs).unwrap();
        fs::write(logs.join(NAME), "omorfos: cannot draw on eDP-1\n").unwrap();

        let compositor = Ok(("omorfos 0.1.0 (protocol 3)".to_owned(), state()));
        let report = written(&places, compositor, false);
        assert!(
            report.starts_with("omorfos report, made by omorfosctl "),
            "{report}"
        );
        for said in [
            "compositor: omorfos 0.1.0 (protocol 3)\n",
            "kernel: 6.12.0-test\n",
            "system: Test Linux 1\n",
            "== Graphics cards\ncard0: i915, vendor 0x8086, device 0x9b41\ncard1: unknown, vendor 0x1002, device \n",
            "eDP-1: 1920 by 1200 units at 0,0\n",
            "\"app_id\": \"foot\"",
            "\"title\": \"(left out)\"",
            "a problem",
            "== config.yaml\nwallpaper: ~/(left out)\n",
            "== keys.yaml\nnot there\n",
            "== theme.css\nwindow { color: red; }\n",
            "== session.log, this login's log\nomorfos: cannot draw on eDP-1\n",
            "== session.log.old, the login before's log\nnot there\n",
        ] {
            assert!(report.contains(said), "{said:?} in:\n{report}");
        }
        assert!(!report.contains("secret plans"), "{report}");
        assert!(!report.contains("Pictures"), "{report}");

        let whole = written(&places, Ok((String::new(), state())), true);
        assert!(whole.contains("\"title\": \"secret plans\""), "{whole}");
        assert!(whole.contains(&picture.display().to_string()), "{whole}");
    }

    #[test]
    fn without_a_compositor_or_anything_to_read_the_report_says_so() {
        let place = TempDir::new().unwrap();
        let places = Places {
            config: None,
            logs: None,
            home: None,
            root: place.path().to_path_buf(),
        };
        let report = written(&places, Err("no socket".to_owned()), false);
        for said in [
            "compositor: did not answer: no socket\n",
            "kernel: unknown\n",
            "system: unknown\n",
            "== Graphics cards\n\n== Screens\n\n== State\nnone\n",
            "== config.yaml\nnot there\n",
            "== session.log, this login's log\nnot there\n",
        ] {
            assert!(report.contains(said), "{said:?} in:\n{report}");
        }
    }

    #[test]
    fn a_log_that_cannot_be_read_is_not_there() {
        let place = TempDir::new().unwrap();
        // Opened as a file is, and read as none can be.
        fs::create_dir(place.path().join(NAME)).unwrap();
        assert_eq!(end_of(&place.path().join(NAME)), None);
    }

    #[test]
    fn a_release_said_where_the_system_keeps_it_is_read_from_there() {
        let place = TempDir::new().unwrap();
        let release = place.path().join("usr/lib/os-release");
        fs::create_dir_all(release.parent().unwrap()).unwrap();
        fs::write(&release, "PRETTY_NAME=Other\n").unwrap();
        assert_eq!(system(place.path()), "Other");
    }

    #[test]
    fn of_a_long_log_its_end_is_put_in_from_the_start_of_a_line() {
        let place = TempDir::new().unwrap();
        let log = place.path().join(NAME);
        let line = "a line of the log said again and again\n";
        let lines = usize::try_from(LOGGED).unwrap() / line.len() + 10;
        fs::write(&log, line.repeat(lines) + "the last\n").unwrap();
        let end = end_of(&log).unwrap();
        assert!(
            end.starts_with("(what came before is left out)\na line"),
            "{end:.80}"
        );
        assert!(end.ends_with("again\nthe last\n"));
        assert!(end.len() <= usize::try_from(LOGGED).unwrap() + 40);
    }

    #[test]
    fn a_path_in_the_home_is_left_out_and_the_home_itself_said_as_a_tilde() {
        let home = Path::new("/home/me/");
        let said =
            "read /home/me/a b.png, \"/home/me/x\" in /home/me and /home/me2/y; /usr/bin/foot";
        assert_eq!(
            left_out(said, home),
            "read ~/(left out) b.png, \"~/(left out)\" in ~ and /home/me2/y; /usr/bin/foot"
        );
        assert_eq!(
            left_out("/home/me", Path::new("/")),
            "/home/me",
            "not the root"
        );
    }
}
