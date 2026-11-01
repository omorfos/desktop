//! Holds an omorfos session while its compositor is started again
//! (ADR 60).
//!
//! The login screen's `omorfos` becomes `omorfos-hold`, which binds the
//! socket programs connect to and starts the compositor with it, as
//! `omorfos --held`. When the compositor ends, because it crashed or
//! stepped down for a new version, the holder starts it again under the
//! same name, telling it what the one before left: whether the session was
//! locked, which number its X server had, and that what starts with the
//! session has been started. It decides nothing else.

mod exec;
mod policy;
pub mod renew;
mod seat;
mod seating;
mod socket;

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use omorfos_ipc::hold::{self, Heard, Told, Word};
use omorfos_ipc::relay::{self, Hand};
use rustix::process::{Pid, Signal, WaitOptions, kill_process, waitpid};
use smithay::backend::session::libseat::LibSeatSessionNotifier;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{
    EventLoop, Interest, LoopHandle, Mode, PostAction, RegistrationToken,
};

use crate::seating::Seating;

pub use exec::give_back;
pub use policy::{Next, Policy, Which};
pub use socket::Socket;

/// What the holder holds for the session.
#[derive(Debug)]
pub struct Hold {
    socket: Socket,
    /// Where the compositor is installed.
    compositor: PathBuf,
    /// The file of the compositor that last came up, held open so that it
    /// can be started again if a new version installed will not.
    held: Option<File>,
    policy: Policy,
    /// Where the holder itself is installed, where it takes up its own new
    /// versions.
    own: Option<PathBuf>,
    /// Where the relay is installed, while programs are to reach the
    /// compositor through it (ADR 61).
    relay_program: Option<PathBuf>,
    /// The relay, once one has been started.
    pub(crate) relay: Option<Relay>,
    /// Whether a relay has been started in this login.
    pub(crate) relay_started: bool,
    /// The graphics card the compositor last running handed over as it
    /// stepped down, open, so that the screens go on showing what it drew
    /// until the next draws, which it is handed to (ADR 62).
    pub(crate) card: Option<OwnedFd>,
    /// The seat, where the holder keeps it for the compositors (ADR 63).
    pub(crate) seating: Option<Seating>,
    /// What says the seat is paused or active again, until there is a loop
    /// to hear it on.
    pub(crate) notifier: Option<LibSeatSessionNotifier>,
    /// What says so, once heard on the loop.
    pub(crate) watching: Option<RegistrationToken>,
    /// Whether the compositor running has ended, as far as has been heard.
    ended: bool,
    /// When the compositor before stepped down, or ended, until the next
    /// says the session is shown, for how long that took to be said.
    stood: Option<Instant>,
    /// What is to be said on the log, as the holder next can.
    logged: Vec<String>,
}

/// The relay running, as far as the holder is concerned.
#[derive(Debug)]
pub(crate) struct Relay {
    /// Its process, waited on by its number: a version of the holder that
    /// takes up another's has the relay as its child, and no handle to it.
    pub(crate) pid: Pid,
    /// The holder's end of the connection the relay was started with.
    pub(crate) connection: OwnedFd,
}

impl Relay {
    /// Joins the relay to a compositor about to be started: hands it one
    /// end of a new connection, and returns the other, for the compositor.
    /// Nothing, if the relay has ended or will not take it.
    fn join(&mut self) -> Option<OwnedFd> {
        let running = matches!(waitpid(Some(self.pid), WaitOptions::NOHANG), Ok(None));
        let pair = relay::pair().ok().filter(|_| running);
        pair.and_then(|(relays, compositor)| {
            let handed = relay::send(
                self.connection.as_fd(),
                &Hand::Compositor,
                &[relays.as_fd()],
            );
            handed.ok().map(|()| compositor)
        })
    }
}

/// What the compositor running is, as far as the holder is concerned.
struct Running {
    child: Child,
    connection: UnixStream,
    file: File,
    /// The holder's end of the connection the compositor asks for the
    /// seat's devices over, where the holder keeps the seat.
    asking: Option<UnixStream>,
}

impl Hold {
    /// Holds `socket` for the compositor installed at `compositor`, as
    /// `policy` says.
    #[must_use]
    pub const fn new(socket: Socket, compositor: PathBuf, policy: Policy) -> Self {
        Self {
            socket,
            compositor,
            held: None,
            policy,
            own: None,
            relay_program: None,
            relay: None,
            relay_started: false,
            card: None,
            seating: None,
            notifier: None,
            watching: None,
            ended: false,
            stood: None,
            logged: Vec::new(),
        }
    }

    /// Takes up the holder's own new versions installed at `own`.
    #[must_use]
    pub fn renewed_from(mut self, own: PathBuf) -> Self {
        self.own = Some(own);
        self
    }

    /// Has programs reach the compositor through the relay installed at
    /// `relay` (ADR 61).
    #[must_use]
    pub fn relayed_by(mut self, relay: PathBuf) -> Self {
        self.relay_program = Some(relay);
        self
    }

    /// Starts the compositor, and again each time it ends as it should be
    /// started again, until the session ends; says why it started one
    /// again on `log`. Returns whether the session was given up.
    pub fn run(&mut self, log: &mut dyn Write) -> bool {
        self.run_from(Which::Installed, log)
    }

    /// Runs as [`Hold::run`] does, starting the compositor from `which`
    /// file first, as another version of the holder said to.
    pub fn run_from(&mut self, which: Which, log: &mut dyn Write) -> bool {
        let event_loop = EventLoop::try_new();
        event_loop.map_or(true, |mut event_loop| {
            self.run_in(&mut event_loop, which, log)
        })
    }

    /// Runs as [`Hold::run_from`] does, waiting on `event_loop`.
    fn run_in(
        &mut self,
        event_loop: &mut EventLoop<'static, Self>,
        mut which: Which,
        log: &mut dyn Write,
    ) -> bool {
        seat::watch(self, &event_loop.handle());
        loop {
            match self.once(event_loop, which, log) {
                Next::Start { which: next, after } => {
                    self.wait(event_loop, after);
                    which = next;
                    // Between one compositor and the next, nothing on the
                    // screen is the holder's, and its own new version is
                    // taken up.
                    exec::renew(self, which, log);
                }
                Next::End { failed } => return failed,
            }
        }
    }

    /// Waits for `after` to pass, answering the seat meanwhile.
    fn wait(&mut self, event_loop: &mut EventLoop<'static, Self>, after: Duration) {
        let until = Instant::now() + after;
        let mut left = after;
        while !left.is_zero() && event_loop.dispatch(left, self).is_ok() {
            left = until.saturating_duration_since(Instant::now());
        }
    }

    /// Takes in `word`, said by the compositor running: as the policy does,
    /// and, as the session is shown after a change of compositor, how long
    /// it stood still, from when the compositor before stepped down or
    /// ended, to be said.
    fn heard(&mut self, word: Word) {
        match word {
            Word::SteppingDown => self.stood = Some(Instant::now()),
            Word::Shown => {
                let stood = self.stood.take().map(|since| since.elapsed().as_millis());
                self.logged.extend(stood.map(|stood| {
                    format!("omorfos-hold: the session stood still for {stood} ms between one compositor and the next")
                }));
            }
            _ => {}
        }
        self.policy.heard(word);
    }

    /// Says on `log` what is to be said there.
    fn say(&mut self, log: &mut dyn Write) {
        for said in self.logged.drain(..) {
            let _ = writeln!(log, "{said}");
        }
    }

    /// Takes in that the seat kept is the session's to use, or not.
    pub(crate) fn seat_changed(&mut self, active: bool) {
        let seating = self.seating.as_mut();
        let _ = seating.map(|seating| seating.changed(active));
    }

    /// Starts the compositor from `which` file, hears it on `event_loop`
    /// until it ends, and says what is next.
    fn once(
        &mut self,
        event_loop: &mut EventLoop<'static, Self>,
        which: Which,
        log: &mut dyn Write,
    ) -> Next {
        let told = self.policy.starting(which);
        let failed = match self.start(which, &told, log, &event_loop.handle()) {
            Ok(running) => self.hear(event_loop, running, log),
            Err(error) => {
                let _ = writeln!(log, "omorfos-hold: cannot start the compositor: {error}");
                true
            }
        };
        let next = self.policy.ended(failed, Instant::now(), self.other());
        let said = match next {
            Next::Start {
                which: Which::Held, ..
            } if which == Which::Installed => {
                "the version installed keeps failing; starting the one before it"
            }
            Next::Start { .. } if failed => "the compositor failed; starting it again",
            Next::Start { .. } => "starting the compositor again, as it asked",
            Next::End { failed: true } => "the compositor keeps failing; ending the session",
            Next::End { failed: false } => "the session has ended",
        };
        let _ = writeln!(log, "omorfos-hold: {said}");
        next
    }

    /// Starts the compositor from `which` file, handing it the socket, or
    /// its connection to the relay and the socket's name, and a connection
    /// to ask for the seat over, and telling it `told`; letting the seat
    /// go, on `handle`, first, where it is to take its own.
    fn start(
        &mut self,
        which: Which,
        told: &Told,
        log: &mut dyn Write,
        handle: &LoopHandle<'static, Self>,
    ) -> io::Result<Running> {
        let file = match which {
            Which::Installed => File::open(&self.compositor),
            Which::Held => {
                let none = Err(io::ErrorKind::NotFound.into());
                self.held.as_ref().map_or(none, File::try_clone)
            }
        }?;
        // One held may be older than the relay, and is handed the socket
        // itself; the relay is not used again until the next login.
        if which == Which::Held {
            self.relay_program = None;
            self.stop_relay();
            self.seating = None;
            seat::unwatch(self.watching.take(), handle);
        }
        let relayed = self.relay_connection(log);
        // The compositor has the card from then on, and the holder keeps it
        // no longer.
        let card = self.card.take();
        // Where it cannot be handed a connection for the seat, the
        // compositor takes its own, which it cannot while the holder has it.
        let seated = self.seating.as_mut().map(Seating::link);
        let seated = seated.and_then(Result::ok);
        let seat = seated.as_ref().and(self.seating.as_ref());
        let told = Told {
            relayed: relayed.is_some(),
            showing: card.is_some(),
            seat: seat.map(Seating::name),
            ..*told
        };
        // Started from the file held, by the holder's own name for it,
        // which stays good however the file installed is replaced.
        let program = format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd());
        let pair = UnixStream::pair();
        let started = pair.and_then(|(connection, theirs)| {
            Command::new(program)
                .arg0("omorfos")
                .arg("--held")
                .stdin(OwnedFd::from(theirs))
                // Whatever the holder was started in, the compositor drives
                // the screen, and has no session to connect to.
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("WAYLAND_SOCKET")
                .env_remove("DISPLAY")
                .spawn()
                .map(|child| (connection, child))
        });
        started.map(|(connection, child)| {
            // One that has gone already cannot be handed anything, and is
            // heard to have ended.
            let fds = relayed.iter().map(AsFd::as_fd).chain([self.socket.as_fd()]);
            let fds = fds.chain(card.as_ref().map(AsFd::as_fd));
            // Last, for a compositor that does not know it to pass it over.
            let fds = fds.chain(seated.as_ref().map(|(theirs, _)| theirs.as_fd()));
            let fds: Vec<BorrowedFd<'_>> = fds.collect();
            let _ = hold::give(&connection, &told.written(), &fds);
            Running {
                child,
                connection,
                file,
                asking: seated.map(|(_, reading)| reading),
            }
        })
    }

    /// Hears what the compositor running says on `event_loop` until it
    /// ends, answering what it asks of the seat meanwhile; closes what was
    /// opened for it, and keeps its file if it came up. Returns whether it
    /// failed.
    fn hear(
        &mut self,
        event_loop: &mut EventLoop<'static, Self>,
        running: Running,
        log: &mut dyn Write,
    ) -> bool {
        let Running {
            mut child,
            connection,
            file,
            asking,
        } = running;
        let handle = event_loop.handle();
        let mut heard = Heard::default();
        // The connection closes when the compositor ends: nothing it starts
        // has it.
        let words = Generic::new(connection, Interest::READ, Mode::Level);
        let words = handle.insert_source(words, move |_, connection, holding: &mut Self| {
            let mut bytes = [0; 256];
            let Ok((read @ 1.., handed)) = hold::read(connection, &mut bytes) else {
                holding.ended = true;
                return Ok(PostAction::Remove);
            };
            // What a compositor hands with what it says is its card alone.
            if let Some(card) = handed.into_iter().next() {
                holding.card = Some(card);
            }
            for word in heard.hear(&bytes[..read]) {
                holding.heard(word);
            }
            Ok(PostAction::Continue)
        });
        let asking = asking.map(|asking| Generic::new(asking, Interest::READ, Mode::Level));
        let asking = asking.map(|asking| {
            handle.insert_source(asking, |_, from, holding: &mut Self| {
                let seating = holding.seating.as_mut();
                Ok(seating.map_or(PostAction::Remove, |seating| seating.asked(from)))
            })
        });
        self.ended = words.is_err();
        while !self.ended && event_loop.dispatch(None, self).is_ok() {
            self.say(log);
        }
        self.say(log);
        self.ended = false;
        // One that ended without stepping down stood still from then.
        self.stood = self.stood.or_else(|| Some(Instant::now()));
        let tokens = [words.ok(), asking.and_then(Result::ok)];
        for token in tokens.into_iter().flatten() {
            handle.remove(token);
        }
        // Closed, the next may open it; the card stays open, for what the
        // compositor drew to stay on the screens.
        let card = self.seating.as_mut().and_then(Seating::ended);
        self.card = self.card.take().or(card);
        let status = child.wait();
        if self.policy.is_up() {
            self.held = Some(file);
        }
        status.map_or(true, |status| !status.success())
    }

    /// The compositor's end of a connection to the relay, which has been
    /// handed the other: the relay running, or started for it, again if it
    /// ended. Nothing where there is no relay, or it has ended too often,
    /// and programs connect to the compositor itself.
    fn relay_connection(&mut self, log: &mut dyn Write) -> Option<OwnedFd> {
        loop {
            let program = self.relay_program.clone()?;
            if let Some(compositor) = self.relay.as_mut().and_then(Relay::join) {
                return Some(compositor);
            }
            // It has ended, or never been started.
            self.stop_relay();
            if self.relay_started && !self.policy.relay_ended(Instant::now()) {
                let said = "the relay keeps failing; programs connect to the compositor itself \
                            until the next login";
                let _ = writeln!(log, "omorfos-hold: {said}");
                self.relay_program = None;
                continue;
            }
            self.relay_started = true;
            match self.start_relay(&program) {
                Ok(relay) => self.relay = Some(relay),
                Err(error) => {
                    let _ = writeln!(log, "omorfos-hold: cannot start the relay: {error}");
                }
            }
        }
    }

    /// Starts the relay installed at `program`, and hands it the socket.
    fn start_relay(&self, program: &Path) -> io::Result<Relay> {
        relay::pair().and_then(|(ours, theirs)| {
            let started = Command::new(program).stdin(Stdio::from(theirs)).spawn();
            started.map(|child| {
                // One that has ended already is found so when it is joined.
                let _ = relay::send(ours.as_fd(), &Hand::Socket, &[self.socket.as_fd()]);
                Relay {
                    pid: Pid::from_child(&child),
                    connection: ours,
                }
            })
        })
    }

    /// Stops the relay, if one is running, and waits for it to end.
    pub(crate) fn stop_relay(&mut self) {
        if let Some(relay) = self.relay.take() {
            let _ = kill_process(relay.pid, Signal::KILL);
            let _ = waitpid(Some(relay.pid), WaitOptions::empty());
        }
    }

    /// Whether the file held open is another than the one installed now.
    fn other(&self) -> bool {
        let identity = |metadata: std::fs::Metadata| (metadata.dev(), metadata.ino());
        let held = self
            .held
            .as_ref()
            .map(|held| held.metadata().map(identity).ok());
        let installed = std::fs::metadata(&self.compositor).map(identity).ok();
        held.is_some_and(|held| held != installed)
    }
}

/// Holds the session in `runtime`, the user's runtime directory, for the
/// compositor installed beside the holder, which is installed at `own`,
/// until it ends. Returns whether it was given up.
///
/// # Errors
///
/// Returns an error if no socket can be bound for it.
pub fn hold(runtime: &Path, own: PathBuf, log: &mut dyn Write) -> io::Result<bool> {
    let socket = Socket::bind(runtime)?;
    let compositor = own.with_file_name("omorfos");
    let mut hold = Hold::new(socket, compositor, Policy::default()).seated();
    hold.relay_program = relay_beside(&own);
    Ok(hold.renewed_from(own).run(log))
}

/// The relay installed beside the holder installed at `own`, if there is
/// one.
pub(crate) fn relay_beside(own: &Path) -> Option<PathBuf> {
    Some(own.with_file_name(RELAY)).filter(|relay| relay.is_file())
}

/// What the relay is called, installed beside the holder.
const RELAY: &str = "omorfos-relay";

impl Drop for Hold {
    fn drop(&mut self) {
        self.stop_relay();
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use omorfos_ipc::seat::Seated;

    use super::*;
    use crate::renew::tests::{hold, program};
    use crate::seating::tests::{Fake, Log};

    /// A stand-in compositor at `program` that writes what it was told to
    /// `told`, and ends.
    fn telling(program_at: &Path, told: &Path) {
        let script = format!(
            "#!/bin/sh\nwhile read -r line && [ -n \"$line\" ]; do echo \"$line\" >> '{}'; done\n",
            told.display()
        );
        program(program_at, &script);
    }

    /// A compositor that has started, with the holder's ends of what it
    /// says over and, where the holder keeps the seat, what it asks over.
    fn running(connection: UnixStream, asking: Option<UnixStream>) -> Running {
        Running {
            child: Command::new("true").spawn().unwrap(),
            connection,
            file: File::open("/proc/self/exe").unwrap(),
            asking,
        }
    }

    #[test]
    fn the_card_a_compositor_hands_as_it_ends_goes_to_the_next_alone() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut held = hold(runtime.path());
        let mut event_loop = EventLoop::try_new().unwrap();
        // A compositor stepping down, with what it drew on its card.
        let (ours, theirs) = UnixStream::pair().unwrap();
        let (card, _other) = UnixStream::pair().unwrap();
        (&ours).write_all(b"up\n").unwrap();
        hold::give(&ours, &Word::Showing.line(), &[card.as_fd()]).unwrap();
        hold::give(&ours, &Word::SteppingDown.line(), &[]).unwrap();
        drop(ours);
        assert!(
            !held.hear(&mut event_loop, running(theirs, None), &mut Vec::new()),
            "stepped down"
        );
        assert!(held.card.is_some(), "kept open");

        // The next is told so, and handed the card after the socket.
        let told = runtime.path().join("told");
        telling(&held.compositor, &told);
        let (mut log, handle) = (Vec::new(), event_loop.handle());
        let starting = held.policy.starting(Which::Installed);
        let next = held.start(Which::Installed, &starting, &mut log, &handle);
        assert!(held.card.is_none(), "the holder keeps it no longer");
        held.hear(&mut event_loop, next.unwrap(), &mut Vec::new());
        assert_eq!(fs_told(&told), "showing\n");

        // The one after, handed none, is told nothing of it.
        std::fs::remove_file(&told).unwrap();
        let starting = held.policy.starting(Which::Installed);
        let after = held.start(Which::Installed, &starting, &mut log, &handle);
        held.hear(&mut event_loop, after.unwrap(), &mut Vec::new());
        assert_eq!(fs_told(&told), "again\n");
    }

    #[test]
    fn a_compositor_takes_the_seat_kept_but_not_one_from_before_that_may_not_know_it() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut held = hold(runtime.path());
        held.seating = Some(Seating::new(Box::new(Fake(Log::default())), true));
        let mut event_loop = EventLoop::try_new().unwrap();
        let (mut log, handle) = (Vec::new(), event_loop.handle());
        let told = runtime.path().join("told");
        telling(&held.compositor, &told);
        let starting = held.policy.starting(Which::Installed);
        let next = held.start(Which::Installed, &starting, &mut log, &handle);
        held.hear(&mut event_loop, next.unwrap(), &mut Vec::new());
        assert_eq!(fs_told(&told), "seat seat9\n");

        // The version held takes its own, and the holder keeps it no more.
        std::fs::remove_file(&told).unwrap();
        program(&runtime.path().join("held"), "#!/bin/sh\nexit 0\n");
        held.held = Some(File::open(held.compositor.clone()).unwrap());
        let starting = held.policy.starting(Which::Held);
        let older = held.start(Which::Held, &starting, &mut log, &handle);
        held.hear(&mut event_loop, older.unwrap(), &mut Vec::new());
        assert_eq!(fs_told(&told), "again\nolder\n");
        assert!(held.seating.is_none());
    }

    #[test]
    fn what_a_compositor_asks_of_the_seat_is_answered_and_closed_as_it_ends() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut held = hold(runtime.path());
        let asked = Log::default();
        let mut seating = Seating::new(Box::new(Fake(Rc::clone(&asked))), true);
        let (theirs, reading) = seating.link().unwrap();
        held.seating = Some(seating);
        let mut event_loop = EventLoop::try_new().unwrap();
        let (ours, words) = UnixStream::pair().unwrap();
        writeln!(&mut &theirs, "open /dev/dri/card0").unwrap();
        held.seat_changed(false);
        drop(ours);
        held.hear(
            &mut event_loop,
            running(words, Some(reading)),
            &mut Vec::new(),
        );
        assert_eq!(*asked.borrow(), ["open /dev/dri/card0", "close"]);
        assert!(held.card.is_some(), "its picture kept");
        theirs.set_nonblocking(true).unwrap();
        let (mut said, mut fds, mut bytes) = (String::new(), 0, [0; 64]);
        while let Ok((read @ 1.., handed)) = hold::read(&theirs, &mut bytes) {
            said.push_str(&String::from_utf8_lossy(&bytes[..read]));
            fds += handed.len();
        }
        let expected = [Seated::Active, Seated::Paused, Seated::Opened(0)].map(Seated::line);
        assert_eq!((said, fds), (expected.concat(), 1));

        // Asked of after the seat has gone, it has nothing to answer with.
        let (ours, words) = UnixStream::pair().unwrap();
        let (theirs, reading) = UnixStream::pair().unwrap();
        writeln!(&mut &theirs, "open /dev/dri/card0").unwrap();
        held.seating = None;
        held.seat_changed(true);
        drop(ours);
        held.hear(
            &mut event_loop,
            running(words, Some(reading)),
            &mut Vec::new(),
        );
        assert_eq!(asked.borrow().len(), 2);
    }

    /// A compositor that says `said` and ends, heard by `held`, whose log
    /// is `log`.
    fn heard(
        held: &mut Hold,
        event_loop: &mut EventLoop<'static, Hold>,
        said: &str,
        log: &mut Vec<u8>,
    ) {
        let (ours, theirs) = UnixStream::pair().unwrap();
        (&ours).write_all(said.as_bytes()).unwrap();
        drop(ours);
        held.hear(event_loop, running(theirs, None), log);
    }

    #[test]
    fn how_long_the_session_stood_still_between_two_compositors_is_said() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut held = hold(runtime.path());
        let mut event_loop = EventLoop::try_new().unwrap();
        let mut log = Vec::new();
        // The first shows the session with nothing before it.
        heard(&mut held, &mut event_loop, "up\nshown\n", &mut log);
        assert_eq!(log, b"", "nothing stood still");
        heard(&mut held, &mut event_loop, "stepping down\n", &mut log);
        heard(&mut held, &mut event_loop, "up\nshown\n", &mut log);
        let said = String::from_utf8(std::mem::take(&mut log)).unwrap();
        let stood = said.strip_prefix("omorfos-hold: the session stood still for ");
        let stood =
            stood.and_then(|stood| stood.strip_suffix(" ms between one compositor and the next\n"));
        assert!(
            stood.is_some_and(|stood| stood.parse::<u128>().is_ok()),
            "{said}"
        );
        // One that ended without stepping down, as one that crashed does.
        heard(&mut held, &mut event_loop, "up\n", &mut log);
        heard(&mut held, &mut event_loop, "shown\n", &mut log);
        assert_eq!(String::from_utf8(log).unwrap().lines().count(), 1);
    }

    #[test]
    fn between_two_compositors_the_seat_is_answered_while_the_holder_waits() {
        let runtime = tempfile::TempDir::new().unwrap();
        let mut held = hold(runtime.path());
        let mut event_loop = EventLoop::try_new().unwrap();
        let begun = Instant::now();
        held.wait(&mut event_loop, Duration::from_millis(30));
        assert!(begun.elapsed() >= Duration::from_millis(30));
    }

    /// What a stand-in wrote it was told, nothing if it was told nothing.
    fn fs_told(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }
}
