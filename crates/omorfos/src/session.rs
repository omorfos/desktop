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
use std::path::PathBuf;
use std::time::{Duration, Instant};

use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, LoopHandle, Mode, PostAction};
use smithay::reexports::wayland_server::{Display, ListeningSocket};

use crate::Compositor;

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
                    .map(|display| (event_loop, display))
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

    /// Accepts the clients that connect to `socket` from now on.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be added to the event loop.
    pub fn serve(&mut self, socket: ListeningSocket) -> Result<(), Box<dyn Error>> {
        let socket = Generic::new(socket, Interest::READ, Mode::Level);
        self.event_loop
            .handle()
            .insert_source(socket, |_, socket, compositor| {
                accept_all(socket, compositor).map(|()| PostAction::Continue)
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

/// Takes on every client waiting to connect.
fn accept_all(socket: &ListeningSocket, compositor: &mut Compositor) -> io::Result<()> {
    let mut waiting = socket.accept();
    while let Ok(Some(stream)) = waiting {
        waiting = compositor
            .insert_client(stream)
            .and_then(|_| socket.accept());
    }
    waiting.map(drop)
}
