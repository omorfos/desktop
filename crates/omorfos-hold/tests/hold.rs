//! The holder starting the compositor, and again as it ends, with stand-ins
//! for the compositor: each a shell script that reads what it is told, and
//! says what it is to say back on the same connection.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omorfos_hold::{Hold, Policy, Socket};

/// Puts `script` at `path`, as a new file in the place of any old one,
/// written by a process of its own: one written by this process could not
/// be run while a process another test is starting holds it open, as it
/// does for the moment before it becomes its own program.
fn written(path: &Path, script: &str) {
    let mut writer = std::process::Command::new("sh")
        .args([
            "-c",
            "cat > \"$1.new\" && chmod 755 \"$1.new\" && mv \"$1.new\" \"$1\"",
            "sh",
        ])
        .arg(path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = writer.stdin.take().unwrap();
    std::io::Write::write_all(&mut input, script.as_bytes()).unwrap();
    drop(input);
    assert!(writer.wait().unwrap().success());
}

/// A place for the session: its runtime directory, where stand-ins leave
/// their marks, and where the compositor is installed.
struct Place {
    directory: tempfile::TempDir,
}

impl Place {
    fn new() -> Self {
        Self {
            directory: tempfile::TempDir::new().unwrap(),
        }
    }

    fn path(&self) -> &Path {
        self.directory.path()
    }

    /// Where the compositor is installed.
    fn compositor(&self) -> PathBuf {
        self.path().join("omorfos")
    }

    /// Installs at `path`, as installing does, by putting a new file in
    /// place of the old, a stand-in that counts its runs in `runs`, writes
    /// what it was told to `told`, and then does `then`, a shell command
    /// that knows its run as `$n`.
    fn install(&self, path: &Path, then: &str) {
        let marks = self.path().display();
        let script = format!(
            "#!/bin/sh\n\
             marks='{marks}'\n\
             n=$(cat \"$marks/runs\" 2>/dev/null || echo 0); n=$((n + 1)); echo $n > \"$marks/runs\"\n\
             told=''\n\
             while read -r line && [ -n \"$line\" ]; do told=\"$told$line,\"; done\n\
             printf '%s;' \"$told\" >> \"$marks/told\"\n\
             {then}\n"
        );
        written(path, &script);
    }

    /// What the stand-ins were told, one `;` after each.
    fn told(&self) -> String {
        fs::read_to_string(self.path().join("told")).unwrap_or_default()
    }

    /// How many times a stand-in has run.
    fn runs(&self) -> u32 {
        let runs = fs::read_to_string(self.path().join("runs")).unwrap_or_default();
        runs.trim().parse().unwrap_or(0)
    }

    /// Holds the session for the compositor installed, waiting only a
    /// moment after a failure; returns whether it was given up and what
    /// the holder said.
    fn hold(&self) -> (bool, String) {
        let socket = Socket::bind(self.path()).unwrap();
        let policy = Policy::with_pause(Duration::from_millis(10));
        let mut hold = Hold::new(socket, self.compositor(), policy);
        let mut log = Vec::new();
        let failed = hold.run(&mut log);
        (failed, String::from_utf8(log).unwrap())
    }
}

#[test]
fn a_compositor_that_comes_up_and_ends_ends_the_session() {
    let place = Place::new();
    place.install(&place.compositor(), "printf 'up\\n' >&0; exit 0");
    let (failed, said) = place.hold();
    assert!(!failed);
    assert_eq!(
        (place.runs(), place.told().as_str()),
        (1, ";"),
        "told nothing"
    );
    assert!(said.contains("the session has ended"), "{said}");
    assert!(
        !place.path().join("wayland-1").exists(),
        "the name is given back"
    );
}

#[test]
fn one_that_steps_down_is_started_again_and_told_what_it_left() {
    let place = Place::new();
    let then = "if [ $n = 1 ]; then printf 'up\\nlocked\\ndisplay 3\\nstepping down\\n' >&0; \
                exit 0; fi; printf 'up\\n' >&0; exit 0";
    place.install(&place.compositor(), then);
    let (failed, said) = place.hold();
    assert!(!failed);
    assert_eq!(place.told(), ";again,locked,display 3,;");
    assert!(said.contains("as it asked"), "{said}");
}

#[test]
fn one_that_keeps_failing_is_started_again_and_then_given_up() {
    let place = Place::new();
    place.install(&place.compositor(), "printf 'up\\n' >&0; exit 1");
    let (failed, said) = place.hold();
    assert!(failed);
    assert_eq!(place.runs(), 4, "once, and three times again");
    assert!(said.contains("failed; starting it again"), "{said}");
    assert!(said.contains("keeps failing; ending the session"), "{said}");
}

#[test]
fn a_new_version_that_will_not_come_up_gives_way_to_the_one_before_it() {
    let place = Place::new();
    let compositor = place.compositor();
    let failing = place.path().join("failing");
    place.install(&failing, "exit 1");
    // The first installs a new version as it runs, and steps down for it;
    // started again from the file held, it comes up.
    let then = format!(
        "if [ $n = 1 ]; then cp '{}' '{}.next' && mv '{}.next' '{}'; \
         printf 'up\\nstepping down\\n' >&0; exit 0; fi; printf 'up\\n' >&0; exit 0",
        failing.display(),
        compositor.display(),
        compositor.display(),
        compositor.display()
    );
    place.install(&compositor, &then);
    let (failed, said) = place.hold();
    assert!(!failed, "{said}");
    // Each run of the new one counted too, and the last was the one before
    // it again, told it is older.
    assert_eq!(place.runs(), 6, "{said}");
    assert!(place.told().ends_with(";again,older,;"), "{}", place.told());
    assert!(said.contains("starting the one before it"), "{said}");
}

#[test]
fn a_compositor_that_cannot_be_started_is_tried_and_given_up() {
    let place = Place::new();
    let (failed, said) = place.hold();
    assert!(failed);
    assert_eq!(
        said.matches("cannot start the compositor").count(),
        4,
        "{said}"
    );
}

#[test]
fn a_compositor_that_is_not_a_program_is_tried_and_given_up() {
    let place = Place::new();
    fs::write(place.compositor(), "not a program").unwrap();
    let (failed, said) = place.hold();
    assert!(failed);
    assert_eq!(
        said.matches("cannot start the compositor").count(),
        4,
        "{said}"
    );
}
