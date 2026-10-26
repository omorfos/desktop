//! The holder taking up its own new version in its place.
//!
//! The holder is the one process that cannot be stopped under the session,
//! so a new version of it is taken up by becoming it: the process the login
//! screen watches runs the new program, and is handed by the old what it
//! held. The new is first run on its own, to see that it starts at all, and
//! a new one that cannot take what it is handed gives it back to the old.
//!
//! What is decided is here. Becoming another program is in `exec`, which
//! decides nothing.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use omorfos_ipc::hold::{self, Told};
use rustix::fs::{OFlags, fcntl_setfl};
use rustix::process::Pid;

use crate::{Hold, Policy, Socket, Which};

/// How long a new version is given to say it starts.
const CHECKING: Duration = Duration::from_secs(2);

/// What a new version is asked to show it starts.
pub const CHECK: &str = "--check";
/// What the new version is started with, to take what the old held.
pub const TAKING: &str = "--taking";
/// What the old version is started with again, to take back what the new
/// could not take.
pub const TAKING_BACK: &str = "--taking-back";

impl Hold {
    /// The holder's own new version to become, if one has been installed
    /// where it is and starts, with the connection that hands it what is
    /// held, the compositor to start next being from `which` file. Says on
    /// `log` why there is none to become, where there was one installed.
    ///
    /// The relay, where there is one, is handed over with the rest: its
    /// connection to the holder, and its process (ADR 61).
    pub fn renewal(&mut self, which: Which, log: &mut dyn Write) -> Option<(PathBuf, UnixStream)> {
        let own = self.own.clone().filter(|own| replaced(own))?;
        if !starts(&own) {
            let said = "its own new version does not start; going on as it is";
            let _ = writeln!(log, "omorfos-hold: {said}");
            return None;
        }
        let handing = self.handing(which).ok();
        handing.map(|handing| (own, handing))
    }

    /// The connection that hands what is held to another version, which is
    /// to start the compositor next from `which` file.
    fn handing(&self, which: Which) -> io::Result<UnixStream> {
        let running = File::open("/proc/self/exe");
        let pair = running.and_then(|running| UnixStream::pair().map(|pair| (running, pair)));
        pair.and_then(|(running, (ours, theirs))| {
            let mut fds = vec![running.as_fd(), self.socket.as_fd(), self.socket.lock()];
            fds.extend(self.held.as_ref().map(AsFd::as_fd));
            fds.extend(self.relay.as_ref().map(|relay| relay.connection.as_fd()));
            hold::give(&ours, &self.written(which), &fds).map(|()| theirs)
        })
    }

    /// What is handed to another version, beside the descriptors.
    fn written(&self, which: Which) -> String {
        let mut text = self.policy.told().written();
        // The empty line that ends what is told ends what is handed.
        text.pop();
        let held = if self.held.is_some() { "held\n" } else { "" };
        let next = if which == Which::Held {
            "next held\n"
        } else {
            ""
        };
        let socket = self.socket.path().display();
        let compositor = self.compositor.display();
        let own = self.own.as_deref().map(Path::display);
        let own = own.map(|own| format!("own {own}\n")).unwrap_or_default();
        let relay = self.relay_program.as_deref().map(Path::display);
        let relay = relay
            .map(|relay| format!("relay {relay}\n"))
            .unwrap_or_default();
        let relayed = self.relay.as_ref().map(|relay| relay.pid.as_raw_nonzero());
        let relayed = relayed
            .map(|pid| format!("relayed by {pid}\n"))
            .unwrap_or_default();
        format!(
            "{text}socket {socket}\ncompositor {compositor}\n{own}{relay}{relayed}{held}{next}\n"
        )
    }
}

/// Whether the holder installed at `own` is another file than the one
/// running.
fn replaced(own: &Path) -> bool {
    let identity = |metadata: fs::Metadata| (metadata.dev(), metadata.ino());
    let running = fs::metadata("/proc/self/exe").map(identity).ok();
    let installed = fs::metadata(own).map(identity).ok();
    installed.is_some() && installed != running
}

/// Whether the holder at `own` says it starts, in time.
fn starts(own: &Path) -> bool {
    let child = Command::new(own)
        .arg(CHECK)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        return false;
    };
    let until = Instant::now() + CHECKING;
    let mut status = child.try_wait();
    while matches!(status, Ok(None)) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(10));
        status = child.try_wait();
    }
    let _ = child.kill();
    let _ = child.wait();
    // What it said is in the pipe by the time it has ended; what it
    // started may hold the pipe open after, and is not waited for.
    let mut said = [0; 64];
    let out = child.stdout.take();
    let unwaited = out.filter(|out| fcntl_setfl(out, OFlags::NONBLOCK).is_ok());
    let read = unwaited.and_then(|mut out| out.read(&mut said).ok());
    let said = &said[..read.unwrap_or(0)];
    let started = matches!(status, Ok(Some(status)) if status.success());
    started && said.starts_with(b"omorfos-hold ")
}

/// Takes what another version handed over, as `text` and `fds`, and holds
/// the session with it as that version did: returns the hold, and which
/// file the compositor is to be started from next.
///
/// # Errors
///
/// Returns what was handed as it came, if it cannot be taken, to give back
/// to the version before with.
pub fn take(text: &str, fds: Vec<OwnedFd>) -> Result<(Hold, Which), (String, Vec<OwnedFd>)> {
    let line = |name: &str| {
        let mut found = text.lines().filter_map(|line| line.strip_prefix(name));
        found.next().map(PathBuf::from)
    };
    let flag = |name: &str| text.lines().any(|line| line == name);
    let relayed = text
        .lines()
        .find_map(|line| line.strip_prefix("relayed by "));
    let relayed = relayed
        .and_then(|pid| pid.parse().ok())
        .and_then(Pid::from_raw);
    let enough = fds.len() >= 3 + usize::from(flag("held")) + usize::from(relayed.is_some());
    let (Some(path), Some(compositor), true) = (line("socket "), line("compositor "), enough)
    else {
        return Err((text.to_owned(), fds));
    };
    // The first is the version before's own file, wanted only to go back.
    let mut fds = fds.into_iter().skip(1);
    let listener = fds.next().map(UnixListener::from);
    let lock = fds.next().map(File::from);
    let held = flag("held").then(|| fds.next()).flatten().map(File::from);
    let relay = relayed.zip(fds.next());
    let socket = listener.zip(lock);
    let socket = socket.map(|(listener, lock)| Socket::from_parts(listener, path, lock));
    let socket = socket.ok_or((String::new(), Vec::new()));
    socket.map(|socket| {
        let policy = Policy::resumed(Told::read(text));
        let mut taken = Hold::new(socket, compositor, policy);
        taken.held = held;
        taken.own = line("own ");
        // A version from before there was a relay says nothing of it: the
        // one installed beside this one is used.
        let beside = || taken.own.as_deref().and_then(crate::relay_beside);
        taken.relay_program = line("relay ").or_else(beside);
        taken.relay_started = relay.is_some();
        taken.relay = relay.map(|(pid, connection)| crate::Relay { pid, connection });
        let which = if flag("next held") {
            Which::Held
        } else {
            Which::Installed
        };
        (taken, which)
    })
}

/// The version before, and the connection that hands it back `text` and
/// `fds`, the first of which is its file.
///
/// # Errors
///
/// Returns an error if the version before is not among `fds`, or what is
/// handed back cannot be.
pub fn handing_back(text: &str, fds: &[OwnedFd]) -> io::Result<(PathBuf, UnixStream)> {
    let old = fds.first();
    let old = old.ok_or(io::Error::new(
        io::ErrorKind::NotFound,
        "the version before is not known",
    ))?;
    let program = PathBuf::from(format!("/proc/self/fd/{}", old.as_raw_fd()));
    let fds: Vec<BorrowedFd<'_>> = fds.iter().map(AsFd::as_fd).collect();
    UnixStream::pair()
        .and_then(|(ours, theirs)| hold::give(&ours, text, &fds).map(|()| (program, theirs)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Puts `script` at `path` as a program, written by a process of its
    /// own: one written by this process could not be run while a process
    /// another test is starting holds it open.
    fn program(path: &Path, script: &str) {
        let mut writer = Command::new("sh")
            .args([
                "-c",
                "cat > \"$1.new\" && chmod 755 \"$1.new\" && mv \"$1.new\" \"$1\"",
                "sh",
            ])
            .arg(path)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = writer.stdin.take().unwrap();
        input.write_all(script.as_bytes()).unwrap();
        drop(input);
        assert!(writer.wait().unwrap().success());
    }

    /// A hold of a socket in `runtime`, for a compositor that is not there.
    fn hold(runtime: &Path) -> Hold {
        let socket = Socket::bind(runtime).unwrap();
        Hold::new(socket, runtime.join("omorfos"), Policy::default())
    }

    #[test]
    fn what_is_held_is_handed_over_and_taken_as_it_was() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut held = hold(runtime.path());
        held.policy.heard(hold::Word::Locked);
        held.policy.heard(hold::Word::Display(4));
        held.held = Some(File::open("/proc/self/exe").unwrap());
        held.own = Some(runtime.path().join("omorfos-hold"));
        let handing = held.handing(Which::Held).unwrap();
        let (text, fds) = hold::take(&handing).unwrap();
        assert_eq!(
            fds.len(),
            4,
            "its own file, the socket, its lock and the one held"
        );
        let (mut taken, which) = take(&text, fds).unwrap();
        assert_eq!(which, Which::Held);
        assert_eq!(taken.socket.path(), held.socket.path());
        assert_eq!(taken.compositor, held.compositor);
        assert_eq!(taken.own, held.own);
        assert!(taken.held.is_some());
        let told = taken.policy.told();
        assert_eq!((told.locked, told.display), (true, Some(4)));
        // The one that takes it is not the first.
        assert!(taken.policy.starting(Which::Installed).again);
        // Programs still find the socket, which the version before keeps
        // until it ends: both name the same.
        UnixStream::connect(taken.socket.path()).unwrap();
        drop(held);
        assert!(!runtime.path().join("wayland-1").exists());
    }

    #[test]
    fn with_nothing_held_and_the_installed_one_next_less_is_handed_over() {
        let runtime = tempfile::TempDir::new().unwrap();
        let held = hold(runtime.path());
        let handing = held.handing(Which::Installed).unwrap();
        let (text, fds) = hold::take(&handing).unwrap();
        assert_eq!(fds.len(), 3);
        let (taken, which) = take(&text, fds).unwrap();
        assert_eq!(which, Which::Installed);
        assert!(taken.held.is_none());
        assert!(taken.own.is_none());
        std::mem::forget(held);
    }

    #[test]
    fn a_version_from_before_the_relay_has_the_relay_beside_it_taken_up() {
        let runtime = tempfile::TempDir::new().unwrap();
        let relay = runtime.path().join("omorfos-relay");
        program(&relay, "#!/bin/sh\nexec sleep 30\n");
        // It knows where it is installed, and says nothing of a relay.
        let held = hold(runtime.path()).renewed_from(runtime.path().join("omorfos-hold"));
        let handing = held.handing(Which::Installed).unwrap();
        let (text, fds) = hold::take(&handing).unwrap();
        assert!(!text.contains("relay "), "{text}");
        let (taken, _) = take(&text, fds).unwrap();
        assert_eq!(taken.relay_program, Some(relay));
    }

    #[test]
    fn what_cannot_be_taken_is_given_back_as_it_came() {
        let (_, fds) = (String::new(), Vec::<OwnedFd>::new());
        let (text, fds) = take("socket /x\n\n", fds).unwrap_err();
        assert_eq!((text.as_str(), fds.len()), ("socket /x\n\n", 0));
        let file = OwnedFd::from(File::open("/proc/self/exe").unwrap());
        let (_, fds) = take("compositor /y\n\n", vec![file]).unwrap_err();
        assert_eq!(fds.len(), 1);
        let fds: Vec<OwnedFd> = (0..3)
            .map(|_| OwnedFd::from(File::open("/proc/self/exe").unwrap()))
            .collect();
        let short = "socket /x\ncompositor /y\nheld\n\n";
        let (_, fds) = take(short, fds).unwrap_err();
        assert_eq!(fds.len(), 3, "one held was said, and not handed");
    }

    #[test]
    fn what_is_handed_back_goes_with_the_version_before_it() {
        let old = OwnedFd::from(File::open("/proc/self/exe").unwrap());
        let (program, handing) = handing_back("socket /x\n\n", &[old]).unwrap();
        assert!(program.starts_with("/proc/self/fd/"));
        let (text, fds) = hold::take(&handing).unwrap();
        assert_eq!((text.as_str(), fds.len()), ("socket /x\n\n", 1));
        let none = handing_back("\n", &[]).unwrap_err();
        assert_eq!(none.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn a_holder_not_replaced_has_nothing_to_become() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut log = Vec::new();
        let mut held = hold(runtime.path());
        assert!(
            held.renewal(Which::Installed, &mut log).is_none(),
            "not known where"
        );
        // Installed where it is, and the same file as the one running.
        let mut held = held.renewed_from(PathBuf::from("/proc/self/exe"));
        assert!(held.renewal(Which::Installed, &mut log).is_none());
        // Not there at all.
        let mut held = held.renewed_from(runtime.path().join("gone"));
        assert!(held.renewal(Which::Installed, &mut log).is_none());
        assert_eq!(log, b"");
    }

    #[test]
    fn a_new_version_that_does_not_start_is_not_become() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut log = Vec::new();
        for script in [
            "#!/bin/sh\nexit 1\n",
            "#!/bin/sh\necho something else\n",
            "#!/bin/sh\nsleep 10\n",
        ] {
            let own = runtime.path().join("omorfos-hold");
            program(&own, script);
            let mut held = hold(runtime.path()).renewed_from(own);
            assert!(
                held.renewal(Which::Installed, &mut log).is_none(),
                "{script}"
            );
        }
        // Nor one that cannot be run at all.
        let own = runtime.path().join("not-a-program");
        fs::write(&own, "").unwrap();
        let mut held = hold(runtime.path()).renewed_from(own);
        assert!(held.renewal(Which::Installed, &mut log).is_none());
        let said = String::from_utf8(log).unwrap();
        assert_eq!(said.matches("does not start").count(), 4, "{said}");
    }

    #[test]
    fn a_new_version_that_starts_is_become_with_what_is_held() {
        let runtime = tempfile::TempDir::new().unwrap();
        let own = runtime.path().join("omorfos-hold");
        program(&own, "#!/bin/sh\necho omorfos-hold 9.9.9\n");
        let relay = runtime.path().join("omorfos-relay");
        program(&relay, "#!/bin/sh\nexec sleep 30\n");
        let held = hold(runtime.path()).renewed_from(own.clone());
        let mut held = held.relayed_by(relay.clone());
        let mut log = Vec::new();
        assert!(held.relay_connection(&mut log).is_some(), "a relay running");
        let (program, handing) = held.renewal(Which::Installed, &mut log).unwrap();
        assert_eq!(program, own);
        // The relay is handed over running: where it is, its process, and
        // its connection to the holder.
        let pid = held.relay.as_ref().map(|relay| relay.pid);
        let (text, fds) = hold::take(&handing).unwrap();
        assert!(text.contains(&format!("own {}\n", own.display())), "{text}");
        assert_eq!(
            fds.len(),
            4,
            "its own file, the socket, its lock and the relay's connection"
        );
        let (mut taken, _) = take(&text, fds).unwrap();
        assert_eq!(taken.relay_program, Some(relay));
        assert_eq!(taken.relay.as_ref().map(|relay| relay.pid), pid);
        assert!(
            taken.relay_connection(&mut log).is_some(),
            "joined as it was"
        );
        // The version before is not to stop it as it ends.
        held.relay = None;
    }
}
