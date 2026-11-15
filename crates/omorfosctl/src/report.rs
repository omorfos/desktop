//! `omorfosctl report`: what a report of a fault needs, gathered into one
//! text for the user to look over before handing it on. It is only
//! written out, never sent anywhere.

use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// How many of the last lines of the journal are asked for.
const JOURNALED: u32 = 1000;

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
    /// The user's names, a path elsewhere with one of which as a part is
    /// left out, as a disk mounted at `/media/me` is.
    pub(crate) names: Vec<String>,
    /// The program that reads the journal, and what it is given before
    /// what the report asks of it: `journalctl`, but in tests.
    pub(crate) journal: (OsString, Vec<OsString>),
    /// Whose of what the journal holds is put in.
    pub(crate) whose: Whose,
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
            names: Vec::new(),
            journal: (OsString::from("/usr/bin/journalctl"), Vec::new()),
            whose: Whose::Nobody,
            root: PathBuf::from("/"),
        }
    }

    /// The same places, with the user's names: `user` and `logname`, as
    /// `USER` and `LOGNAME` give them, and the home's last part, each once.
    #[must_use]
    pub(crate) fn named(mut self, user: Option<&OsStr>, logname: Option<&OsStr>) -> Self {
        let homed = self.home.as_deref().and_then(Path::file_name);
        for name in [user, logname, homed].into_iter().flatten() {
            let name = name.to_string_lossy().into_owned();
            if !self.names.contains(&name) {
                self.names.push(name);
            }
        }
        self
    }
}

/// Whose of what the journal holds a report puts in: what the programs
/// the session starts write goes there (`omorfos-journal`), with what
/// everything else the user and the system run writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Whose {
    /// This login's, by logind's name for it.
    Session(String),
    /// The user's, by their number, since the computer started, where the
    /// login is not known.
    User(u32),
    /// No one's, where neither is known: never everyone's.
    Nobody,
}

impl Whose {
    /// This login's, if `session`, as `XDG_SESSION_ID` gives it, names one;
    /// or else `user`'s, the user's number, if it is known.
    pub(crate) fn of(session: Option<&OsStr>, user: Option<u32>) -> Self {
        // logind names a login by letters and figures; anything else would
        // be read by the journal as more than the name of one.
        let session = session.and_then(OsStr::to_str).filter(|session| {
            !session.is_empty() && session.chars().all(|c| c.is_ascii_alphanumeric())
        });
        match (session, user) {
            (Some(session), _) => Self::Session(session.to_owned()),
            (None, Some(user)) => Self::User(user),
            (None, None) => Self::Nobody,
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
        "Window titles and paths in your home or naming you are kept, as `--whole` was given."
    } else {
        "Window titles, paths in your home and paths elsewhere naming you are left out; \
         `omorfosctl report --whole` keeps them."
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
    journal(&places.journal, &places.whose, &mut text);

    if !whole {
        if let Some(home) = &places.home {
            text = left_out(&text, home);
        }
        text = named_out(&text, &places.names);
    }
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

/// Puts in what `whose` said in the journal, asking `journal` for the last
/// [`JOURNALED`] lines of it this boot, and of those as much of the end as
/// a log's is put in.
fn journal((program, given): &(OsString, Vec<OsString>), whose: &Whose, text: &mut String) {
    let (title, matched) = match whose {
        Whose::Session(session) => ("this login's", format!("_SYSTEMD_SESSION={session}")),
        Whose::User(user) => (
            "the user's since the computer started",
            format!("_UID={user}"),
        ),
        Whose::Nobody => {
            heading(text, "The journal, of no one known");
            text.push_str("not there\n");
            return;
        }
    };
    heading(text, &format!("The journal, {title}"));
    let lines = format!("--lines={JOURNALED}");
    let asked = [
        "--no-pager",
        "--quiet",
        "--output=short-iso",
        &lines,
        "--boot",
    ];
    let output = Command::new(program)
        .args(given)
        .args(asked)
        .arg(&matched)
        .stdin(Stdio::null())
        .output();
    match output {
        Err(_) => text.push_str("not there\n"),
        Ok(output) if output.status.success() => {
            let said = &output.stdout;
            let from = said
                .len()
                .saturating_sub(usize::try_from(LOGGED).unwrap_or(usize::MAX));
            text.push_str(&tail(&String::from_utf8_lossy(&said[from..]), from > 0));
        }
        Ok(output) => {
            let why = String::from_utf8_lossy(&output.stderr);
            let why = why.lines().next().map(str::to_owned);
            let why = why.unwrap_or_else(|| output.status.to_string());
            let _ = writeln!(text, "did not answer: {why}");
        }
    }
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
    Some(tail(&String::from_utf8_lossy(&bytes), from > 0))
}

/// The end of something said, as `text`: all of it, or, if what came
/// before it was `cut` off, from the start of a line, saying so.
fn tail(text: &str, cut: bool) -> String {
    if !cut {
        return text.to_owned();
    }
    let whole_lines = text.split_once('\n').map_or("", |(_, rest)| rest);
    format!("(what came before is left out)\n{whole_lines}")
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

/// Parts of the paths of the system common enough that a user called by
/// one is named by it only where [`MOUNTS`] are.
const COMMON_PARTS: [&str; 26] = [
    "applications",
    "bin",
    "boot",
    "cache",
    "dev",
    "doc",
    "etc",
    "fonts",
    "games",
    "home",
    "icons",
    "include",
    "lib",
    "lib64",
    "local",
    "log",
    "media",
    "mnt",
    "opt",
    "run",
    "sbin",
    "share",
    "src",
    "srv",
    "tmp",
    "usr",
];

/// Where a user's own disks are mounted, under a directory of their name.
const MOUNTS: [&str; 3] = ["/media", "/run/media", "/mnt"];

/// `text` with each path that has one of `names` as a whole part of it
/// left out after the parts before it, as `/media/me/USB` is written
/// `/media/(left out)`. A path begins with `/`, and ends at a space, a
/// quote, a bracket, or the `:` between the paths of a list.
fn named_out(text: &str, names: &[String]) -> String {
    let ends = |c: char| c.is_whitespace() || "\"'`()[]{}<>,;:=".contains(c);
    let mut text = text.to_owned();
    // A name that could not be a part of a path names none.
    let names = names
        .iter()
        .filter(|name| !name.is_empty() && !name.contains('/'));
    for name in names {
        // A name that is also a common part of the system's paths names
        // the user only where the user's own disks are mounted.
        let common = COMMON_PARTS.contains(&name.to_lowercase().as_str());
        let part = format!("/{name}");
        let mut kept = String::with_capacity(text.len());
        // Where what is not yet kept begins.
        let mut from = 0;
        for (at, _) in text.match_indices(&part) {
            let after = &text[at + part.len()..];
            let whole = after.chars().next().is_none_or(|c| c == '/' || ends(c));
            let end = text[..at].char_indices().rev().find(|(_, c)| ends(*c));
            let begun = end.map_or(0, |(end, c)| end + c.len_utf8());
            let path = text[begun..at].is_empty() || text[begun..at].starts_with('/');
            let mounted = MOUNTS.contains(&&text[begun..at]);
            if at >= from && whole && path && (mounted || !common) {
                kept.push_str(&text[from..at]);
                kept.push_str("/(left out)");
                from = text.len() - after.trim_start_matches(|c: char| !ends(c)).len();
            }
        }
        kept.push_str(&text[from..]);
        text = kept;
    }
    text
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
            names: vec!["me".to_owned()],
            journal: said(&format!(
                "printf '%s\\n' \"$@\" {}/home/me/Documents/a.txt",
                root.display()
            )),
            whose: Whose::Session("3".to_owned()),
            root: root.to_path_buf(),
        }
    }

    /// A journal that is `sh` running `script`, with what the report asks
    /// of the journal as its arguments.
    fn said(script: &str) -> (OsString, Vec<OsString>) {
        let given = ["-c", script, "journalctl"].map(OsString::from).to_vec();
        ("sh".into(), given)
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
        fs::write(config.join("rules.yaml"), "- icon: /media/me/USB/a.png\n").unwrap();
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
            "== rules.yaml\n- icon: /media/(left out)\n",
            "== theme.css\nwindow { color: red; }\n",
            "== session.log, this login's log\nomorfos: cannot draw on eDP-1\n",
            "== session.log.old, the login before's log\nnot there\n",
            "== The journal, this login's\n--no-pager\n--quiet\n--output=short-iso\n\
             --lines=1000\n--boot\n_SYSTEMD_SESSION=3\n~/(left out)\n",
        ] {
            assert!(report.contains(said), "{said:?} in:\n{report}");
        }
        assert!(!report.contains("secret plans"), "{report}");
        assert!(!report.contains("Pictures"), "{report}");

        let whole = written(&places, Ok((String::new(), state())), true);
        assert!(whole.contains("\"title\": \"secret plans\""), "{whole}");
        assert!(whole.contains(&picture.display().to_string()), "{whole}");
        assert!(whole.contains("/media/me/USB/a.png"), "{whole}");
    }

    #[test]
    fn without_a_compositor_or_anything_to_read_the_report_says_so() {
        let place = TempDir::new().unwrap();
        let places = Places {
            config: None,
            logs: None,
            home: None,
            names: Vec::new(),
            journal: (place.path().join("journalctl").into(), Vec::new()),
            whose: Whose::Nobody,
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
            "== The journal, of no one known\nnot there\n",
        ] {
            assert!(report.contains(said), "{said:?} in:\n{report}");
        }
    }

    #[test]
    fn the_journal_is_asked_for_what_this_login_said_or_else_what_the_user_said() {
        let asked = |whose: &Whose| {
            let mut out = String::new();
            journal(&said("printf '%s\\n' \"$@\""), whose, &mut out);
            out
        };
        let session = asked(&Whose::Session("c2".to_owned()));
        assert!(
            session.starts_with("\n== The journal, this login's\n"),
            "{session}"
        );
        assert!(
            session.ends_with("--boot\n_SYSTEMD_SESSION=c2\n"),
            "{session}"
        );
        let user = asked(&Whose::User(1000));
        assert!(user.starts_with("\n== The journal, the user's since the computer started\n"));
        assert!(user.ends_with("--boot\n_UID=1000\n"), "{user}");
        // Never everyone's.
        assert_eq!(
            asked(&Whose::Nobody),
            "\n== The journal, of no one known\nnot there\n"
        );
    }

    #[test]
    fn whose_the_journal_is_read_for_is_the_login_or_else_the_user() {
        let os = |value: &'static str| Some(OsStr::new(value));
        assert_eq!(
            Whose::of(os("7"), Some(1000)),
            Whose::Session("7".to_owned())
        );
        assert_eq!(Whose::of(None, Some(1000)), Whose::User(1000));
        assert_eq!(Whose::of(os(""), Some(1000)), Whose::User(1000));
        // What would be read as more than one login's is not taken as one.
        assert_eq!(Whose::of(os("7 _UID=0"), Some(1000)), Whose::User(1000));
        assert_eq!(Whose::of(os("+"), None), Whose::Nobody);
    }

    #[test]
    fn a_journal_that_fails_says_why_and_a_long_one_is_put_in_by_its_end() {
        let mut out = String::new();
        let failing = said("echo cannot read the journal >&2; exit 1");
        journal(&failing, &Whose::User(1), &mut out);
        assert!(
            out.ends_with("\ndid not answer: cannot read the journal\n"),
            "{out}"
        );
        let mut out = String::new();
        let missing = (OsString::from("/nonexistent/journalctl"), Vec::new());
        journal(&missing, &Whose::User(1), &mut out);
        assert!(
            out.ends_with("since the computer started\nnot there\n"),
            "{out}"
        );
        let mut out = String::new();
        journal(&said("exit 3"), &Whose::User(1), &mut out);
        assert!(out.contains("did not answer: "), "{out}");
        let mut out = String::new();
        let long = format!("yes 'a line the journal said' | head -n {}", LOGGED / 10);
        journal(&said(&long), &Whose::User(1), &mut out);
        assert!(
            out.contains("(what came before is left out)\na line"),
            "{out:.200}"
        );
        assert!(out.len() <= usize::try_from(LOGGED).unwrap() + 100);
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

    #[test]
    fn a_path_elsewhere_that_names_the_user_is_left_out_after_the_parts_before_it() {
        let names = ["me".to_owned(), "mine".to_owned()];
        let said = "mounted /media/me/USB stick, (/run/media/mine) and PATH=/opt/me/bin:/mnt/me";
        assert_eq!(
            named_out(said, &names),
            "mounted /media/(left out) stick, (/run/media/(left out)) and \
             PATH=/opt/(left out):/mnt/(left out)"
        );
        // Only a whole part of a path names the user, and only in a path.
        let kept = "/media/meg/x /media/ame/x /meme me said so, a/me/b and http://x/me";
        assert_eq!(
            named_out(kept, &names),
            kept.replace("http://x/me", "http://x/(left out)")
        );
        // A name that could not be a part of a path names nothing.
        let odd = [String::new(), "a/b".to_owned()];
        assert_eq!(named_out("/x/a/b /y//z", &odd), "/x/a/b /y//z");
    }

    #[test]
    fn a_name_that_is_also_a_common_part_of_paths_is_left_out_only_where_disks_are_mounted() {
        let names = ["share".to_owned(), "Local".to_owned()];
        let said = "/usr/share/icons /usr/local/bin /media/share/USB /run/media/share \
                    /mnt/share/x /home/Local/y /media/Local";
        assert_eq!(
            named_out(said, &names),
            "/usr/share/icons /usr/local/bin /media/(left out) /run/media/(left out) \
             /mnt/(left out) /home/Local/y /media/(left out)"
        );
    }

    #[test]
    fn whom_the_report_leaves_out_is_the_user_by_each_name_the_environment_gives() {
        let os = |value: &'static str| Some(OsStr::new(value));
        let given = Places::of(None, None, os("/home/me")).named(os("rafal"), os("rafal"));
        assert_eq!(given.names, ["rafal", "me"]);
        let homeless = Places::of(None, None, None).named(None, os("me"));
        assert_eq!(homeless.names, ["me"]);
        let rooted = Places::of(None, None, os("/")).named(None, None);
        assert_eq!(rooted.names, [""; 0]);
    }
}
