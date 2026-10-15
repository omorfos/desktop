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
//! - failing that, a window being the first of a process omorfos started,
//!   for the many programs that say nothing of tokens.

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{DisplayHandle, Resource};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::xdg_activation::{XdgActivationState, XdgActivationToken};

use crate::compositor::Compositor;

/// What has been seen of windows being asked for, before they opened.
#[derive(Debug)]
pub(crate) struct Asked {
    /// The tokens there are, as xdg-activation has them.
    pub(crate) tokens: XdgActivationState,
    /// The processes omorfos started that have not shown a window yet.
    started: Vec<u32>,
    /// Windows activated with a token before they were shown.
    early: Vec<WlSurface>,
}

impl Asked {
    /// Offers xdg-activation on `display`, with nothing asked for yet.
    pub(crate) fn new(display: &DisplayHandle) -> Self {
        Self {
            tokens: XdgActivationState::new::<Compositor>(display),
            started: Vec::new(),
            early: Vec::new(),
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
