//! The compositor together with the event loop that drives it.
//!
//! A [`Session`] owns the loop, the Wayland display and the [`Compositor`].
//! It waits for clients, hands their requests to the compositor, brings it
//! up to date and sends the replies. A backend adds what only it has: where
//! input comes from, as further sources on the loop, and where pixels go, as
//! the work it does each turn.
//!
//! See `docs/adr/0009-session-and-event-loop.md`.

use std::error::Error;
use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omorfos_ipc::hold::{self, Told};
use omorfos_ipc::relay::{Hand, Received};

use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::{Display, ListeningSocket};

use crate::Compositor;
use crate::Roles;

/// How much the compositor keeps, in bytes, of what it has to send a
/// program that is not reading, once the program's socket is full. A
/// program busy for a while, with the pointer moved over its window, is
/// not disconnected for it, while one that never reads is, before it has
/// cost more than this. See `docs/adr/0045-a-program-that-is-not-reading.md`.
const KEPT_FOR_A_PROGRAM: usize = 1 << 20;

/// A compositor and the event loop that drives it.
#[derive(Debug)]
pub struct Session {
    event_loop: EventLoop<'static, Compositor>,
    compositor: Compositor,
}

impl Session {
    /// Creates a session whose compositor reads its configuration from
    /// `config_directory`. See [`Compositor::new`].
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop or the Wayland display cannot be
    /// created, or if [`Compositor::new`] fails.
    pub fn new(config_directory: impl Into<PathBuf>) -> Result<Self, Box<dyn Error>> {
        let config_directory = config_directory.into();
        EventLoop::try_new()
            .map_err(Box::from)
            .and_then(|event_loop| {
                Display::new()
                    .map(|display| {
                        // Before any program connects: it holds for those
                        // that connect after.
                        display
                            .handle()
                            .set_default_max_buffer_size(KEPT_FOR_A_PROGRAM);
                        (event_loop, display)
                    })
                    .map_err(Box::from)
            })
            .and_then(|(event_loop, display)| {
                Compositor::new(display.handle(), event_loop.handle(), config_directory)
                    .map(|compositor| (event_loop, display, compositor))
            })
            .and_then(|(event_loop, mut display, compositor)| {
                // The display is moved into its own event source, which is
                // the one place that needs all of it. The descriptor that
                // says when clients have something to say is duplicated to
                // wake the loop.
                let clients = display.backend().poll_fd().try_clone_to_owned();
                clients
                    .map(|clients| Generic::new(clients, Interest::READ, Mode::Level))
                    .map_err(Box::from)
                    .and_then(|clients| {
                        let handle = event_loop.handle();
                        handle
                            .insert_source(clients, move |_, _, compositor: &mut Compositor| {
                                display
                                    .dispatch_clients(compositor)
                                    .map(|_| PostAction::Continue)
                            })
                            .map_err(Box::from)
                    })
                    .map(|_| Self {
                        event_loop,
                        compositor,
                    })
            })
    }

    /// Accepts the clients that connect to `socket` from now on, each with
    /// `roles`.
    ///
    /// The socket a session is known by is for any program that finds it,
    /// and is served with no roles: what a program may do beyond the
    /// ordinary comes from the compositor having started it, not from its
    /// having found a socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be added to the event loop.
    pub fn serve(&mut self, socket: ListeningSocket, roles: Roles) -> Result<(), Box<dyn Error>> {
        self.serve_on(socket, roles)
    }

    /// Takes up what `omorfos-hold` hands over `holder`, the compositor's
    /// end of the connection it was started with: serves the socket it
    /// hands, with the IPC socket beside it, comes up as the holder says
    /// the compositor before it left the session, and tells it from then
    /// on what it is to keep. See `docs/adr/0060-the-session-outlives-the-compositor.md`.
    ///
    /// # Errors
    ///
    /// Returns an error if nothing is handed over, if no socket with a
    /// name comes with it, or if the IPC socket cannot be opened.
    pub fn hold(&mut self, holder: UnixStream) -> Result<(), Box<dyn Error>> {
        let (text, fds) = hold::take(&holder)?;
        let told = Told::read(&text);
        let mut fds = fds.into_iter();
        // Relayed, the connection to the relay comes first, and the socket
        // after it only for its name: the relay serves it (ADR 61).
        let relay = told.relayed.then(|| fds.next()).flatten();
        let socket = fds.next().map(UnixListener::from);
        let socket = socket.ok_or("omorfos-hold handed over no socket to serve")?;
        let address = socket.local_addr().ok();
        let path = address.and_then(|address| address.as_pathname().map(Path::to_owned));
        let path = path.ok_or("the socket omorfos-hold handed over has no name")?;
        // Neither is ever to make the compositor wait.
        socket
            .set_nonblocking(true)
            .and_then(|()| holder.set_nonblocking(true))
            .and_then(|()| self.compositor.take_up_holder(holder, told, &path))
            .map_err(Box::from)
            .and_then(|()| match relay {
                Some(relay) => self.serve_relay(relay),
                None => self.serve_on(socket, Roles::NONE),
            })
    }

    /// Takes the programs the relay hands over `relay`, each with who it
    /// is; and, once the relay has gone, steps down, as they have gone with
    /// it, for the holder to start a relay and a compositor anew.
    fn serve_relay(&mut self, relay: OwnedFd) -> Result<(), Box<dyn Error>> {
        let relay = Generic::new(relay, Interest::READ, Mode::Level);
        self.event_loop
            .handle()
            .insert_source(relay, |_, relay, compositor| {
                Ok(take_relayed(relay.as_fd(), compositor))
            })
            .map(drop)
            .map_err(Box::from)
    }

    /// Accepts the clients that connect to `socket`, of whichever kind it
    /// is, each with `roles`.
    fn serve_on<S: Listening + AsFd + 'static>(
        &mut self,
        socket: S,
        roles: Roles,
    ) -> Result<(), Box<dyn Error>> {
        let socket = Generic::new(socket, Interest::READ, Mode::Level);
        self.event_loop
            .handle()
            .insert_source(socket, move |_, socket, compositor| {
                accept_all(&**socket, compositor, roles).map(|()| PostAction::Continue)
            })
            .map(drop)
            .map_err(Box::from)
    }

    /// A handle to the event loop, for adding sources of events such as a
    /// backend's input devices.
    #[must_use]
    pub fn handle(&self) -> LoopHandle<'static, Compositor> {
        self.event_loop.handle()
    }

    /// The compositor.
    #[must_use]
    pub const fn compositor(&self) -> &Compositor {
        &self.compositor
    }

    /// The compositor, to act on.
    pub const fn compositor_mut(&mut self) -> &mut Compositor {
        &mut self.compositor
    }

    /// Takes one turn: waits up to `timeout` for something to happen (for
    /// ever if `None`), handles it, brings the compositor up to date as of
    /// `now` and sends clients what they are owed.
    ///
    /// # Errors
    ///
    /// Returns an error if an event source fails or clients cannot be
    /// written to.
    pub fn dispatch(
        &mut self,
        timeout: Option<Duration>,
        now: Instant,
    ) -> Result<(), Box<dyn Error>> {
        self.event_loop
            .dispatch(timeout, &mut self.compositor)
            .map_err(Box::from)
            .and_then(|()| {
                self.compositor.refresh(now);
                self.compositor.display.flush_clients().map_err(Box::from)
            })
    }

    /// Takes turns until the session is asked to end. After each one `turn`
    /// is called, for a backend to draw, and clients are then sent anything
    /// it produced for them.
    ///
    /// Between turns the session sleeps until something happens or the
    /// compositor's [deadline](Compositor::deadline) arrives. When nothing
    /// changes, nothing runs.
    ///
    /// # Errors
    ///
    /// Returns the first error from [`Session::dispatch`] or from `turn`.
    pub fn run(
        &mut self,
        mut turn: impl FnMut(&mut Compositor) -> Result<(), Box<dyn Error>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut outcome = Ok(());
        while outcome.is_ok() && self.compositor.is_running() {
            let asleep = Instant::now();
            let deadline = self.compositor.deadline();
            let timeout = deadline.map(|deadline| deadline.saturating_duration_since(asleep));
            // The compositor is told the time it is once the wait is over.
            outcome = self
                .dispatch(timeout, Instant::now())
                .and_then(|()| turn(&mut self.compositor))
                .and_then(|()| self.compositor.display.flush_clients().map_err(Box::from));
        }
        outcome
    }
}

/// What programs connect to: a socket the session bound itself, or one
/// handed to it.
trait Listening {
    /// The next program waiting to connect, if one is.
    fn next(&self) -> io::Result<Option<UnixStream>>;
}

impl Listening for ListeningSocket {
    fn next(&self) -> io::Result<Option<UnixStream>> {
        self.accept()
    }
}

impl Listening for UnixListener {
    fn next(&self) -> io::Result<Option<UnixStream>> {
        let waiting = |error: io::Error| {
            let none = error.kind() == io::ErrorKind::WouldBlock;
            none.then_some(None).ok_or(error)
        };
        self.accept()
            .map(|(stream, _)| Some(stream))
            .or_else(waiting)
    }
}

/// Takes on every program the relay has handed over `relay`; once it has
/// gone, steps down, and says it is to hear no more of it.
fn take_relayed(relay: BorrowedFd<'_>, compositor: &mut Compositor) -> PostAction {
    loop {
        match omorfos_ipc::relay::receive(relay) {
            Ok(Some(Received {
                hand: Some(Hand::Program(program)),
                fds,
            })) => {
                // One that cannot be taken has its connection closed, and
                // the relay lets the program go.
                let stream = fds.into_iter().next().map(UnixStream::from);
                let _ = stream.map(|stream| compositor.insert_relayed(stream, program));
            }
            // What is not known is passed over.
            Ok(Some(_)) => {}
            Ok(None) => return PostAction::Continue,
            Err(_) => {
                compositor.restart_compositor();
                return PostAction::Remove;
            }
        }
    }
}

/// Takes on every client waiting to connect.
fn accept_all(
    socket: &impl Listening,
    compositor: &mut Compositor,
    roles: Roles,
) -> io::Result<()> {
    let mut waiting = socket.next();
    while let Ok(Some(stream)) = waiting {
        waiting = compositor
            .insert_client(stream, roles)
            .and_then(|_| socket.next());
    }
    waiting.map(drop)
}
