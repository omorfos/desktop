//! Starting the programs that are to start with the session, as the
//! freedesktop specification for it has them: the desktop entries in the
//! user's `~/.config/autostart` and in the system's `/etc/xdg/autostart`,
//! a user's entry taking the place of the system's of the same name.
//!
//! What to start is a value made from an entry and what is known of the
//! session, and tested without starting anything; starting it is the rest.

use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use omorfos_entries::{arguments, fields, list, read};

use crate::{Failure, Places};

/// The most entries that are read from one directory.
const MOST: usize = 1024;

/// A program to start, as its entry says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Start {
    /// The program.
    pub(crate) program: String,
    /// Its arguments.
    pub(crate) arguments: Vec<String>,
    /// The directory it is started in, where the entry names one.
    pub(crate) directory: Option<PathBuf>,
}

/// Where autostart entries are, the user's own first: the `autostart`
/// directory of `XDG_CONFIG_HOME`, or of `~/.config`; then of each of
/// `XDG_CONFIG_DIRS`, or of `/etc/xdg`. A relative directory is passed
/// over, as the specification has it.
#[must_use]
pub fn directories(
    config_home: Option<&OsStr>,
    home: Option<&OsStr>,
    config_dirs: Option<&OsStr>,
) -> Vec<PathBuf> {
    let set = |value: Option<&OsStr>| value.filter(|value| !value.is_empty()).map(PathBuf::from);
    let own = set(config_home).or_else(|| set(home).map(|home| home.join(".config")));
    let system = set(config_dirs).unwrap_or_else(|| PathBuf::from("/etc/xdg"));
    let system = std::env::split_paths(&system).collect::<Vec<_>>();
    own.into_iter()
        .chain(system)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join("autostart"))
        .collect()
}

/// What `entry` asks to be started in a session of the desktops named, if
/// anything, where programs are looked for in `path`. Nothing is started
/// for an entry that is not a program's, is hidden or turned off, is for
/// other desktops, needs a terminal, or names a program that is not there.
pub(crate) fn wanted(entry: &str, desktops: &[String], path: &OsStr) -> Option<Start> {
    let (mut application, mut on, mut terminal) = (false, true, false);
    let (mut exec, mut directory, mut tried) = (None, None, None);
    let (mut only, mut not) = (None, None);
    for (key, value) in fields(entry) {
        match key {
            "Type" => application = value == "Application",
            "Exec" => exec = Some(value),
            "Path" => directory = Some(value).filter(|value| !value.is_empty()),
            "TryExec" => tried = Some(value),
            "Hidden" => on &= value != "true",
            // GNOME's own way of turning an entry off, which its tools
            // write and other desktops honour.
            "X-GNOME-Autostart-enabled" => on &= value != "false",
            "Terminal" => terminal = value == "true",
            "OnlyShowIn" => only = Some(value),
            "NotShowIn" => not = Some(value),
            _ => {}
        }
    }
    let ours = |names: &str| list(names).any(|name| desktops.iter().any(|ours| ours == name));
    let shown = only.is_none_or(ours) && !not.is_some_and(ours);
    let there = tried.is_none_or(|program| found(program, path));
    let command = exec.map(arguments);
    let command = command.filter(|_| application && on && shown && there && !terminal);
    let command = command.as_ref().and_then(|command| command.split_first());
    command.map(|(program, arguments)| Start {
        program: program.clone(),
        arguments: arguments.to_vec(),
        directory: directory.map(PathBuf::from),
    })
}

/// Whether `program` is there to be run: a file that may be run, at the
/// path it names, or found by name in `path`.
fn found(program: &str, path: &OsStr) -> bool {
    let runnable = |file: &Path| {
        let mode = fs::metadata(file).map(|file| (file.is_file(), file.permissions().mode()));
        mode.is_ok_and(|(file, mode)| file && mode & 0o111 != 0)
    };
    if program.contains('/') {
        return runnable(Path::new(program));
    }
    std::env::split_paths(path).any(|directory| runnable(&directory.join(program)))
}

/// What the entries in `directories` ask to be started, the user's taking
/// the place of the system's of the same file name, hidden or not, which
/// is how a user turns one of the system's off.
pub(crate) fn entries(directories: &[PathBuf], desktops: &[String], path: &OsStr) -> Vec<Start> {
    let mut seen: Vec<std::ffi::OsString> = Vec::new();
    let mut started = Vec::new();
    for directory in directories {
        let entries = fs::read_dir(directory).into_iter().flatten().flatten();
        let entries = entries.map(|entry| entry.path());
        let entries = entries.filter(|path| path.extension() == Some(OsStr::new("desktop")));
        let mut entries: Vec<PathBuf> = entries.take(MOST).collect();
        // The order a directory lists its files in is not one to rely on.
        entries.sort();
        for entry in entries {
            let name = entry.file_name().unwrap_or_default().to_owned();
            if seen.contains(&name) {
                continue;
            }
            seen.push(name);
            let wanted = read(&entry).and_then(|entry| wanted(&entry, desktops, path));
            started.extend(wanted);
        }
    }
    started
}

/// The program and arguments that start `start`: by `scoper`,
/// `systemd-run`, in a scope that is part of the graphical session and
/// stops with it, where there is one, since a program that is not the
/// compositor's client is told of nothing else that ends; and as it is
/// otherwise.
fn scoped(scoper: Option<&Path>, start: &Start) -> (std::ffi::OsString, Vec<String>) {
    let Some(scoper) = scoper else {
        return (start.program.clone().into(), start.arguments.clone());
    };
    let through = [
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        "--slice=app-graphical.slice",
        "--property=PartOf=graphical-session.target",
        "--property=After=graphical-session.target",
        "--",
        start.program.as_str(),
    ];
    let mut all: Vec<String> = through.into_iter().map(str::to_owned).collect();
    all.extend_from_slice(&start.arguments);
    (scoper.as_os_str().to_owned(), all)
}

/// Starts what the session's autostart entries ask, each once, and leaves
/// each running.
///
/// # Errors
///
/// If any could not be started, saying which and why. The others are
/// started all the same.
pub(crate) fn start(places: &Places) -> Result<(), Failure> {
    let mut failed = Vec::new();
    for start in entries(&places.autostart, &places.desktops, &places.path) {
        let (program, arguments) = scoped(places.scoper.as_deref(), &start);
        let mut command = Command::new(program);
        command.args(arguments).stdin(Stdio::null());
        if let Some(directory) = &start.directory {
            command.current_dir(directory);
        }
        // It is the session's to run, not this one's to wait for.
        if let Err(error) = command.spawn() {
            failed.push(format!("cannot start `{}`: {error}", start.program));
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; ").into())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn entry(lines: &[&str]) -> String {
        format!("[Desktop Entry]\nType=Application\n{}\n", lines.join("\n"))
    }

    fn ours() -> Vec<String> {
        vec!["omorfos".to_owned()]
    }

    /// What is started for an entry with `lines`, in a session of omorfos
    /// where programs are looked for where `sh` is.
    fn started(lines: &[&str]) -> Option<Start> {
        wanted(&entry(lines), &ours(), OsStr::new("/usr/bin:/bin"))
    }

    fn program(program: &str, arguments: &[&str]) -> Start {
        Start {
            program: program.to_owned(),
            arguments: arguments.iter().map(|word| (*word).to_owned()).collect(),
            directory: None,
        }
    }

    #[test]
    fn an_entry_says_what_to_start_and_where() {
        let start = started(&["Name=Agent", "Exec=agent --quiet %U"]);
        assert_eq!(start, Some(program("agent", &["--quiet"])));
        let start = started(&["Exec=agent", "Path=/srv/agent"]);
        let expected = Start {
            directory: Some(PathBuf::from("/srv/agent")),
            ..program("agent", &[])
        };
        assert_eq!(start, Some(expected));
        let start = started(&["Exec=agent", "Path="]);
        assert_eq!(start, Some(program("agent", &[])), "an empty path is none");
    }

    #[test]
    fn what_is_not_a_program_or_is_turned_off_is_not_started() {
        let link = "[Desktop Entry]\nType=Link\nExec=agent\n";
        assert_eq!(
            wanted(link, &ours(), OsStr::new("")),
            None,
            "not a program's"
        );
        assert_eq!(started(&[]), None, "no command");
        assert_eq!(
            started(&["Exec=%f"]),
            None,
            "nothing but a place for a file"
        );
        assert_eq!(started(&["Exec=agent", "Hidden=true"]), None);
        assert_eq!(
            started(&["Exec=agent", "X-GNOME-Autostart-enabled=false"]),
            None
        );
        assert_eq!(started(&["Exec=agent", "Terminal=true"]), None);
        for kept in [
            "Hidden=false",
            "X-GNOME-Autostart-enabled=true",
            "Terminal=false",
        ] {
            assert!(started(&["Exec=agent", kept]).is_some(), "{kept}");
        }
    }

    #[test]
    fn an_entry_for_other_desktops_or_not_for_this_one_is_not_started() {
        assert_eq!(started(&["Exec=agent", "OnlyShowIn=GNOME;KDE;"]), None);
        assert!(started(&["Exec=agent", "OnlyShowIn=GNOME;omorfos;"]).is_some());
        assert_eq!(started(&["Exec=agent", "NotShowIn=omorfos;"]), None);
        assert!(started(&["Exec=agent", "NotShowIn=KDE;GNOME;"]).is_some());
        // Told it is more than one desktop, it is each of them.
        let both = ["GNOME".to_owned(), "omorfos".to_owned()];
        let for_gnome = entry(&["Exec=agent", "OnlyShowIn=GNOME;"]);
        assert!(wanted(&for_gnome, &both, OsStr::new("")).is_some());
    }

    #[test]
    fn an_entry_is_started_only_if_the_program_it_tries_is_there() {
        assert!(
            started(&["Exec=agent", "TryExec=sh"]).is_some(),
            "found in the path"
        );
        assert!(
            started(&["Exec=agent", "TryExec=/bin/sh"]).is_some(),
            "at its path"
        );
        assert_eq!(started(&["Exec=agent", "TryExec=no-such-program"]), None);
        assert_eq!(started(&["Exec=agent", "TryExec=/nowhere/agent"]), None);
        // A file that is not to be run is not a program.
        let directory = tempfile::TempDir::new().unwrap();
        let plain = directory.path().join("plain");
        fs::write(&plain, "").unwrap();
        let tried = format!("TryExec={}", plain.display());
        assert_eq!(started(&["Exec=agent", &tried]), None);
        let tried = format!("TryExec={}", directory.path().display());
        assert_eq!(started(&["Exec=agent", &tried]), None, "nor is a directory");
    }

    #[test]
    fn entries_are_looked_for_in_the_users_directory_and_then_the_systems() {
        let os = |text: &'static str| Some(OsStr::new(text));
        assert_eq!(
            directories(os("/c"), os("/home/u"), os("/etc/a:relative:/etc/b")),
            [
                PathBuf::from("/c/autostart"),
                PathBuf::from("/etc/a/autostart"),
                PathBuf::from("/etc/b/autostart"),
            ]
        );
        assert_eq!(
            directories(None, os("/home/u"), os("")),
            [
                PathBuf::from("/home/u/.config/autostart"),
                PathBuf::from("/etc/xdg/autostart"),
            ]
        );
        assert_eq!(
            directories(os(""), None, None),
            [PathBuf::from("/etc/xdg/autostart")]
        );
    }

    /// Writes an entry with `lines` as `name` in `directory`.
    fn write(directory: &Path, name: &str, lines: &[&str]) {
        fs::create_dir_all(directory).unwrap();
        fs::write(directory.join(name), entry(lines)).unwrap();
    }

    #[test]
    fn a_users_entry_takes_the_place_of_the_systems_of_the_same_name() {
        let root = tempfile::TempDir::new().unwrap();
        let (own, system) = (root.path().join("own"), root.path().join("system"));
        write(&own, "agent.desktop", &["Exec=own-agent"]);
        write(&system, "agent.desktop", &["Exec=system-agent"]);
        // Turned off by the user, though the system's would start.
        write(&own, "keyring.desktop", &["Exec=keyring", "Hidden=true"]);
        write(&system, "keyring.desktop", &["Exec=keyring"]);
        write(&system, "b.desktop", &["Exec=b"]);
        write(&system, "a.desktop", &["Exec=a"]);
        fs::write(system.join("notes.txt"), entry(&["Exec=notes"])).unwrap();
        fs::write(system.join("binary.desktop"), [0xff, 0xfe]).unwrap();
        let found = entries(&[own, system], &ours(), OsStr::new(""));
        let programs: Vec<&str> = found.iter().map(|start| start.program.as_str()).collect();
        assert_eq!(programs, ["own-agent", "a", "b"]);
    }

    /// Makes a pipe at `mark`, for a program started to write into.
    fn pipe(mark: &Path) {
        let made = Command::new("mkfifo").arg(mark).status();
        assert!(made.unwrap().success());
    }

    /// What the program started writes into the pipe at `mark`: read once
    /// it has written, without waiting by the clock. One that never starts
    /// is said to, after a while.
    fn told(mark: &Path) -> String {
        let late = mark.to_owned();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(10));
            let _ = fs::write(late, "never started");
        });
        fs::read_to_string(mark).unwrap().trim().to_owned()
    }

    #[test]
    fn what_is_found_is_started_and_left_running_and_what_cannot_be_is_said() {
        let root = tempfile::TempDir::new().unwrap();
        let directory = root.path().join("autostart");
        let mark = root.path().join("started");
        let exec = format!("Exec=sh -c \"pwd > '{}'\"", mark.display());
        let path = format!("Path={}", root.path().display());
        write(&directory, "marks.desktop", &[&exec, &path]);
        pipe(&mark);
        let places = Places {
            autostart: vec![directory.clone()],
            desktops: ours(),
            path: std::ffi::OsString::from("/usr/bin:/bin"),
            ..Places::from(&|_| None)
        };
        start(&places).unwrap();
        assert_eq!(
            told(&mark),
            root.path().display().to_string(),
            "where it says"
        );

        // Started in a scope of its own where there is what makes one,
        // which here writes down what it was asked, and starts it.
        let scoper = root.path().join("systemd-run");
        let script = "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$(dirname \"$0\")/scoped\"\n\
                      while [ \"$1\" != -- ]; do shift; done\nshift\nexec \"$@\"\n";
        fs::write(&scoper, script).unwrap();
        fs::set_permissions(&scoper, fs::Permissions::from_mode(0o755)).unwrap();
        let scoping = Places {
            scoper: Some(scoper),
            ..places.clone()
        };
        start(&scoping).unwrap();
        assert_eq!(told(&mark), root.path().display().to_string());
        let asked = fs::read_to_string(root.path().join("scoped")).unwrap();
        assert!(asked.starts_with("--user --scope "), "{asked}");
        let rest = "--property=PartOf=graphical-session.target";
        assert!(asked.contains(rest), "{asked}");
        assert!(asked.contains(" -- sh -c "), "{asked}");

        // One that cannot be started, by itself.
        let lacking = root.path().join("lacking");
        write(&lacking, "missing.desktop", &["Exec=no-such-program --now"]);
        let places = Places {
            autostart: vec![lacking],
            ..places
        };
        let failure = start(&places).unwrap_err().to_string();
        assert!(
            failure.starts_with("cannot start `no-such-program`: "),
            "{failure}"
        );
    }
}
