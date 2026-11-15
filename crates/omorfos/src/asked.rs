//! Whether the user asked for a window that opens.
//!
//! A window that opens while a workspace made for windows is shown, with
//! no part left for it, goes to the desktop that workspace was made from.
//! If nobody asked for it, it waits there unseen, and the user is left
//! where they were; if the user asked for it, that desktop is shown with
//! it. See `docs/adr/0036-workspaces-that-come-and-go.md`.
//!
//! What says a window was asked for:
//!
//! - an activation token: one omorfos handed the program it started, in
//!   `XDG_ACTIVATION_TOKEN` and `DESKTOP_STARTUP_ID`, or one a program
//!   made from something the user did, as a launcher does. A program
//!   written for Wayland activates its window with it, before the window
//!   is shown or after; one written for X11 names it in `_NET_STARTUP_ID`.
//!   A program's token is honoured only for the serial of the last key
//!   or button pressed, or one since, when that press was the program's:
//!   not for one it made up, nor for one from before the user went to
//!   another program.
//! - failing that, a window being the first of a process omorfos started,
//!   for the many programs that say nothing of tokens.

use std::time::{Duration, Instant};

use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::utils::{SERIAL_COUNTER, Serial};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::xdg_activation::{XdgActivationState, XdgActivationToken};

use crate::compositor::Compositor;

/// The most tokens kept at once: more than are ever in use together, as a
/// browser makes one for each link it opens and uses few.
const MOST_TOKENS: usize = 32;

/// How long a token is good for once made: one that was for something the
/// user just did is used at once.
const TOKEN_LIFE: Duration = Duration::from_secs(60);

/// What has been seen of windows being asked for, before they opened.
#[derive(Debug)]
pub(crate) struct Asked {
    /// The tokens there are, as xdg-activation has them.
    pub(crate) tokens: XdgActivationState,
    /// The processes omorfos started that have not shown a window yet.
    started: Vec<u32>,
    /// Windows activated with a token before they were shown.
    early: Vec<WlSurface>,
    /// The serial of the last key or button pressed, and the program it
    /// went to, if it went to one.
    pressed: Option<(Serial, ClientId)>,
}

impl Asked {
    /// Offers xdg-activation on `display`, with nothing asked for yet.
    pub(crate) fn new(display: &DisplayHandle) -> Self {
        Self {
            tokens: XdgActivationState::new::<Compositor>(display),
            started: Vec::new(),
            early: Vec::new(),
            pressed: None,
        }
    }
}

impl Compositor {
    /// A token for a program about to be started, for it to hand on to the
    /// window it opens.
    pub(crate) fn ticket(&mut self) -> String {
        let (token, _) = self.asked.tokens.create_external_token(None);
        token.to_string()
    }

    /// A serial for a key `pressed` or let go of; a press is noted as the
    /// program's that has the keyboard.
    pub(crate) fn key_serial(&mut self, pressed: bool) -> Serial {
        let serial = SERIAL_COUNTER.next_serial();
        let focus = self.keyboard.current_focus();
        let surface = focus.as_ref().and_then(WaylandFocus::wl_surface);
        if pressed {
            self.pressed(serial, surface.as_deref());
        }
        serial
    }

    /// A serial for a button `pressed` or let go of; a press is noted as
    /// the program's that has the pointer.
    pub(crate) fn button_serial(&mut self, pressed: bool) -> Serial {
        let serial = SERIAL_COUNTER.next_serial();
        if pressed {
            self.pressed(serial, self.pointer.current_focus().as_ref());
        }
        serial
    }

    /// Notes that a key or a button was pressed, with `serial`, and went to
    /// `surface`'s program, or to none.
    fn pressed(&mut self, serial: Serial, surface: Option<&WlSurface>) {
        let client = surface.and_then(|surface| surface.client().map(|client| client.id()));
        self.asked.pressed = client.map(|client| (serial, client));
    }

    /// Whether a program, `client`, may have a token that moves focus, for
    /// it says it is for `serial`: the last key or button pressed went to
    /// it, and `serial` is that press's or one given since; not one the
    /// compositor has not given anyone yet.
    pub(crate) fn may_move_focus(&self, client: &ClientId, serial: Serial) -> bool {
        let given = SERIAL_COUNTER.next_serial().is_no_older_than(&serial);
        let pressed = self.asked.pressed.as_ref();
        let since =
            pressed.is_some_and(|(pressed, to)| to == client && serial.is_no_older_than(pressed));
        given && since
    }

    /// Forgets the tokens that are old, and those beyond the most kept, the
    /// oldest first, so that a program making them without end costs
    /// nothing that lasts.
    pub(crate) fn forget_old_tokens(&mut self) {
        let mut made: Vec<Instant> = self
            .asked
            .tokens
            .tokens()
            .map(|(_, data)| data.timestamp)
            .collect();
        made.sort_unstable_by(|one, other| other.cmp(one));
        let oldest = made.get(MOST_TOKENS - 1).copied();
        self.asked.tokens.retain_tokens(|_, data| {
            data.timestamp.elapsed() < TOKEN_LIFE
                && oldest.is_none_or(|oldest| data.timestamp >= oldest)
        });
    }

    /// Notes that omorfos started the process numbered `pid`, or, with
    /// `None`, could not: the token it was to have is no longer good.
    pub(crate) fn started(&mut self, ticket: String, pid: Option<u32>) {
        match pid {
            Some(pid) => self.asked.started.push(pid),
            None => {
                self.asked
                    .tokens
                    .remove_token(&XdgActivationToken::from(ticket));
            }
        }
    }

    /// Forgets the processes omorfos started that have ended.
    pub(crate) fn forget_ended(&mut self) {
        let children = &self.children;
        let alive = |pid: &u32| children.iter().any(|child| child.id() == *pid);
        self.asked.started.retain(alive);
    }

    /// Notes that a window not yet shown was activated with a good token.
    /// Anything else is not a window, and nothing is noted.
    pub(crate) fn ask_early(&mut self, surface: WlSurface) {
        let waiting = self.unmapped.iter();
        if waiting
            .map(ToplevelSurface::wl_surface)
            .any(|waiting| *waiting == surface)
        {
            self.asked.early.retain(Resource::is_alive);
            self.asked.early.push(surface);
        }
    }

    /// Whether a window being shown for the first time was asked for: its
    /// surface activated before it was shown, a token named by `startup`,
    /// or the process numbered `pid` one that omorfos started and that has
    /// shown no window yet. Each says so once.
    pub(crate) fn asked_for(
        &mut self,
        surface: &WlSurface,
        pid: Option<u32>,
        startup: Option<String>,
    ) -> bool {
        let early = self.asked.early.iter().position(|early| early == surface);
        let early = early.map(|at| self.asked.early.remove(at)).is_some();
        let token = startup.map(XdgActivationToken::from);
        let ticket = token.is_some_and(|token| self.asked.tokens.remove_token(&token));
        let started = self.asked.started.iter();
        let process = pid.and_then(|pid| started.clone().position(|started| *started == pid));
        let process = process.map(|at| self.asked.started.remove(at)).is_some();
        early || ticket || process
    }
}
