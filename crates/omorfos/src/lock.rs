//! Locking the session, and knowing when the user is away.
//!
//! A screen locker is an ordinary client that asks for the session to be
//! locked and then shows a surface to unlock it with. What makes a lock
//! worth having is on this side: while it holds, nothing but that surface
//! may be seen or reached.
//!
//! That is kept true by doing it in as few places as possible, each of
//! which takes over completely:
//!
//! - drawing: [`Compositor::render`] draws the lock surface on black and
//!   returns before it looks at anything else;
//! - input: [`Compositor::input`] hands every event to
//!   [`Compositor::locked_input`] and returns, so key bindings, the menu,
//!   drags and every window are never consulted;
//! - the keyboard: it is given to the lock surface, or to nothing.
//!
//! The session stays locked if the locker dies. Only an unlock from the
//! locker that locked it, or a new locker taking over from a dead one,
//! changes that.
//!
//! See `docs/adr/0014-idle-and-session-lock.md`.

use smithay::backend::input::{Axis, ButtonState, KeyState};
use smithay::desktop::WindowSurfaceType;
use smithay::desktop::utils::under_from_surface_tree;
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::input::pointer::{AxisFrame, ButtonEvent};
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::ExtSessionLockV1;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::compositor::get_parent;
use smithay::wayland::idle_inhibit::IdleInhibitHandler;
use smithay::wayland::idle_notify::{IdleNotifierHandler, IdleNotifierState};
use smithay::wayland::session_lock::{
    LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
};
use smithay::{delegate_idle_inhibit, delegate_idle_notify, delegate_session_lock};

use crate::Input;
use crate::compositor::Compositor;
use crate::input::{Placed, virtual_terminal};

/// A lock on the session. While the compositor holds one, the session is
/// locked.
#[derive(Debug)]
pub(crate) struct Lock {
    /// The locker's hold on the session. The lock is its to release, and
    /// nobody else's while it lives.
    by: ExtSessionLockV1,
    /// What the locker shows, once it has something to show.
    surface: Option<LockSurface>,
    /// The promise to tell the locker that the session is locked, kept once
    /// a frame without anything of the session in it has been shown.
    unconfirmed: Option<SessionLocker>,
}

impl Compositor {
    /// Whether the session is locked.
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.lock.is_some()
    }

    /// The surface the locker is showing, if the session is locked and it
    /// has one.
    pub(crate) fn lock_surface(&self) -> Option<&WlSurface> {
        let shown = self.lock.as_ref().and_then(|lock| lock.surface.as_ref());
        shown
            .filter(|surface| surface.alive())
            .map(LockSurface::wl_surface)
    }

    /// Handles input while the session is locked. Everything goes to the
    /// lock surface or nowhere.
    ///
    /// The one thing still acted on is switching virtual terminal, which
    /// leads to a login prompt and not into the session.
    pub(crate) fn locked_input(&mut self, event: Placed, time: u32) {
        match event {
            Placed::Key { code, pressed } => self.locked_key(code, pressed, time),
            Placed::Motion { x, y } => {
                self.pointer_location = (x, y).into();
                self.point_at_lock(time);
            }
            Placed::Button { button, pressed } => {
                let state = if pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                };
                let pointer = self.pointer.clone();
                let event = ButtonEvent {
                    serial: SERIAL_COUNTER.next_serial(),
                    time,
                    button,
                    state,
                };
                pointer.button(self, &event);
                pointer.frame(self);
            }
            Placed::Scroll {
                horizontal,
                vertical,
            } => {
                let frame = AxisFrame::new(time)
                    .value(Axis::Horizontal, horizontal)
                    .value(Axis::Vertical, vertical);
                let pointer = self.pointer.clone();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
        }
    }

    fn locked_key(&mut self, code: Keycode, pressed: bool, time: u32) {
        let keyboard = self.keyboard.clone();
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        let serial = SERIAL_COUNTER.next_serial();
        keyboard.input(
            self,
            code,
            state,
            serial,
            time,
            |compositor, _, key| match virtual_terminal(key.modified_sym()) {
                Some(terminal) => {
                    if pressed {
                        compositor.terminal_wanted = Some(terminal);
                    }
                    FilterResult::Intercept(())
                }
                None => FilterResult::Forward,
            },
        );
    }

    /// Points the pointer at the lock surface if it is over it, and at
    /// nothing otherwise.
    fn point_at_lock(&mut self, time: u32) {
        let focus = self.lock_surface().and_then(|surface| {
            let all = WindowSurfaceType::ALL;
            under_from_surface_tree(surface, self.pointer_location, (0, 0), all)
        });
        let focus = focus.map(|(surface, origin)| (surface, origin.to_f64()));
        self.move_pointer(focus, time);
    }

    /// Tells the lock surface how large the output is, which is the size it
    /// has to be.
    pub(crate) fn configure_lock_surface(&self) {
        let shown = self.lock.as_ref().and_then(|lock| lock.surface.as_ref());
        if let Some(surface) = shown {
            let size = self.logical_size();
            let size = (size.w.unsigned_abs(), size.h.unsigned_abs());
            surface.with_pending_state(|state| state.size = Some(size.into()));
            surface.send_configure();
        }
    }

    /// Tells the locker that the session is locked, if it has not been told
    /// and a frame showing nothing of the session has been drawn since it
    /// asked.
    pub(crate) fn confirm_lock(&mut self) {
        if !self.blanked.get() {
            return;
        }
        let lock = self.lock.as_mut();
        if let Some(confirmation) = lock.and_then(|lock| lock.unconfirmed.take()) {
            confirmation.lock();
        }
    }

    /// Works out whether something on screen is asking for the session not
    /// to go idle, and says so to those waiting for it to.
    ///
    /// A window that is not shown has no say, and while the session is
    /// locked nothing of it is shown.
    pub(crate) fn update_idle_inhibition(&mut self) {
        self.inhibitors.retain(Resource::is_alive);
        let shown = |surface: &WlSurface| {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }
            self.is_shown(&root)
        };
        let inhibited = !self.is_locked() && self.inhibitors.iter().any(shown);
        self.idle.set_is_inhibited(inhibited);
    }
}

impl SessionLockHandler for Compositor {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.lock_manager
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        if self.lock.as_ref().is_some_and(|lock| lock.by.is_alive()) {
            // Somebody holds the lock. Dropping the request refuses it.
            return;
        }
        // Nothing half done survives into, or out of, a locked session.
        self.menu = None;
        self.drag = None;
        self.blanked.set(false);
        self.lock = Some(Lock {
            by: confirmation.ext_session_lock().clone(),
            surface: None,
            unconfirmed: Some(confirmation),
        });
        // The keyboard and pointer leave whatever had them at once, not
        // when the session is next brought up to date: input handled in
        // between would otherwise still be the application's.
        let keyboard = self.keyboard.clone();
        keyboard.set_focus(self, None, SERIAL_COUNTER.next_serial());
        self.point_at_lock(0);
    }

    fn unlock(&mut self) {
        self.lock = None;
        // An application may have asked for a drag while the session was
        // locked. It did not get one.
        self.menu = None;
        self.drag = None;
        // Whatever is under the pointer finds out that it is.
        let at = self.pointer_location;
        self.input(Input::Motion { x: at.x, y: at.y }, 0);
    }

    fn new_surface(&mut self, surface: LockSurface, _output: WlOutput) {
        // Only the locker's own surface is ever shown. A client whose
        // request to lock was refused still has an object to ask with, and
        // must not get to put its surface over the real one, or to show one
        // when the session is not locked at all.
        let client = surface.wl_surface().client();
        let lock = self.lock.as_mut();
        if let Some(lock) = lock.filter(|lock| lock.by.client() == client) {
            lock.surface = Some(surface);
            self.configure_lock_surface();
        }
    }
}

impl IdleNotifierHandler for Compositor {
    fn idle_notifier_state(&mut self) -> &mut IdleNotifierState<Self> {
        &mut self.idle
    }
}

impl IdleInhibitHandler for Compositor {
    fn inhibit(&mut self, surface: WlSurface) {
        self.inhibitors.push(surface);
    }

    fn uninhibit(&mut self, surface: WlSurface) {
        self.inhibitors.retain(|inhibitor| *inhibitor != surface);
    }
}

delegate_session_lock!(Compositor);
delegate_idle_notify!(Compositor);
delegate_idle_inhibit!(Compositor);
