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
//!   [`Compositor::guarded_input`] and returns, so key bindings, the menu,
//!   drags and every window are never consulted;
//! - the keyboard: it is given to the lock surface, or to nothing.
//!
//! The session stays locked if the locker dies. Only an unlock from the
//! locker that locked it, or a new locker taking over from a dead one,
//! changes that.
//!
//! See `docs/adr/0014-idle-and-session-lock.md`.

use std::time::{Duration, Instant};

use smithay::backend::input::{Axis, ButtonState, KeyState};
use smithay::desktop::WindowSurfaceType;
use smithay::desktop::utils::under_from_surface_tree;
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::input::pointer::{AxisFrame, ButtonEvent};
use smithay::output::Output;
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
use crate::roles::Roles;

/// A lock on the session. While the compositor holds one, the session is
/// locked.
#[derive(Debug)]
pub(crate) struct Lock {
    /// The locker's hold on the session. The lock is its to release, and
    /// nobody else's while it lives.
    by: ExtSessionLockV1,
    /// What the locker shows on each output, once it has something to
    /// show there.
    surfaces: Vec<(Output, LockSurface)>,
    /// The promise to tell the locker that the session is locked, kept once
    /// a frame without anything of the session in it has been shown.
    unconfirmed: Option<SessionLocker>,
}

/// What is known of whether the user is there.
#[derive(Debug)]
pub(crate) struct Presence {
    /// Whether something on screen is asking for the session to stay awake.
    pub(crate) kept_awake: bool,
    /// Whether there has been input since the compositor was last brought
    /// up to date.
    pub(crate) stirred: bool,
    /// When the user was last known to be there.
    last_active: Option<Instant>,
    /// Whether the screens are to be on, and if not whether they are off.
    screens: Screens,
    /// The time away the session was last locked for, and the one the
    /// machine was last put to sleep for, each known by when the user was
    /// last there before it: each is done once a time away.
    locked_for: Option<Instant>,
    slept_for: Option<Instant>,
}

/// Whether the screens are on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screens {
    /// They are to be on.
    On,
    /// They are to be off.
    ToBeOff,
    /// They are off: the backend has said it turned them off.
    Off,
}

impl Default for Presence {
    fn default() -> Self {
        Self {
            kept_awake: false,
            stirred: false,
            last_active: None,
            screens: Screens::On,
            locked_for: None,
            slept_for: None,
        }
    }
}

impl Compositor {
    /// Whether the session is locked.
    #[must_use]
    pub const fn is_locked(&self) -> bool {
        self.lock.is_some()
    }

    /// Whether the session is locked and the locker has been told so: a
    /// frame with nothing of the session in it has been shown on every
    /// output. Until then what was on the screen may still be.
    pub(crate) fn is_locked_for_certain(&self) -> bool {
        self.lock
            .as_ref()
            .is_some_and(|lock| lock.unconfirmed.is_none())
    }

    /// The surfaces the locker is showing, and the output each is on.
    fn lock_surfaces(&self) -> impl Iterator<Item = &(Output, LockSurface)> {
        let surfaces = self.lock.iter().flat_map(|lock| &lock.surfaces);
        surfaces.filter(|(_, surface)| surface.alive())
    }

    /// The surface the locker is showing on `output`, if the session is
    /// locked and it has one there.
    pub(crate) fn lock_surface_on(&self, output: &Output) -> Option<&WlSurface> {
        self.lock_surfaces()
            .find(|(on, _)| on == output)
            .map(|(_, surface)| surface.wl_surface())
    }

    /// The lock surface that has the keyboard: the one on the output the
    /// pointer is on, or failing that any there is.
    pub(crate) fn lock_focus(&self) -> Option<&WlSurface> {
        let here = self.pointer_head().map(|head| &head.output);
        here.and_then(|output| self.lock_surface_on(output))
            .or_else(|| {
                let any = self.lock_surfaces().next();
                any.map(|(_, surface)| surface.wl_surface())
            })
    }

    /// Stops showing a lock surface on an output that has gone.
    pub(crate) fn forget_lock_surface(&mut self, output: &Output) {
        if let Some(lock) = &mut self.lock {
            lock.surfaces.retain(|(on, _)| on != output);
        }
    }

    /// Handles input while the session is locked, or while a prompt asks
    /// for a secret over it. Everything goes to the lock surface or the
    /// prompt, or nowhere.
    ///
    /// The one thing still acted on is switching virtual terminal, which
    /// leads to a login prompt and not into the session.
    pub(crate) fn guarded_input(&mut self, event: Placed, time: u32) {
        match event {
            Placed::Key { code, pressed } => self.guarded_key(code, pressed, time),
            Placed::Motion { x, y } => {
                self.pointer_location = (x, y).into();
                if self.is_locked() {
                    self.point_at_lock(time);
                } else {
                    self.point_at_sealed(time);
                }
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
            // Nothing a gesture does is done behind a lock or a prompt.
            Placed::Gesture(_) => {}
        }
    }

    fn guarded_key(&mut self, code: Keycode, pressed: bool, time: u32) {
        let keyboard = self.keyboard.clone();
        // What is typed now is the user's secret. The only grab of the
        // keyboard there is, is an input method's, and it is let go of
        // rather than shown the secret; it grabs again when next it is
        // asked to type, once the session is open.
        if keyboard.is_grabbed() {
            keyboard.unset_grab(self);
        }
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

    /// Points the pointer at the lock surface on the output it is on, if it
    /// is over it, and at nothing otherwise.
    fn point_at_lock(&mut self, time: u32) {
        let head = self.head_at(self.pointer_location);
        let focus = head.and_then(|head| {
            let surface = self.lock_surface_on(&head.output)?;
            let origin = head.area().loc;
            let all = WindowSurfaceType::ALL;
            under_from_surface_tree(surface, self.pointer_location, origin, all)
        });
        let focus = focus.map(|(surface, origin)| (surface, origin.to_f64()));
        self.move_pointer(focus, time);
    }

    /// Tells each lock surface how large its output is, which is the size
    /// it has to be.
    pub(crate) fn configure_lock_surfaces(&self) {
        for (output, surface) in self.lock_surfaces() {
            let size = self.area_of(output).size;
            let size = (size.w.unsigned_abs(), size.h.unsigned_abs());
            surface.with_pending_state(|state| state.size = Some(size.into()));
            surface.send_configure();
        }
    }

    /// Tells the locker that the session is locked, if it has not been told
    /// and a frame showing nothing of the session has been drawn on every
    /// output since it asked, or every screen is off.
    pub(crate) fn confirm_lock(&mut self) {
        // A screen that mirrors another shows the session as much as that
        // one does; one that is off shows nothing.
        let mirrors = self.aside.mirrors.iter().map(|mirror| &mirror.head);
        let shown = self.heads.iter().chain(mirrors);
        let blanked = shown.into_iter().all(|head| head.blanked.get());
        if !blanked && self.presence.screens != Screens::Off {
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
        self.presence.kept_awake = inhibited;
        self.idle.set_is_inhibited(inhibited);
    }

    /// Says that the screens are off, as [`Compositor::screens_on`] asked,
    /// which a backend that drives screens says once it has turned them
    /// off. A screen that is off shows nothing of the session, so a lock
    /// asked for meanwhile is as good as shown, and the locker is told so
    /// without waiting for a frame that will not be drawn until the user
    /// is back.
    pub fn screens_turned_off(&mut self) {
        if self.presence.screens == Screens::ToBeOff {
            self.presence.screens = Screens::Off;
        }
    }

    /// Whether the screens are to be on. They are turned off once there has
    /// been no input for as long as the configuration says, and on again at
    /// the next. A backend that drives screens acts on this each turn.
    #[must_use]
    pub fn screens_on(&self) -> bool {
        self.presence.screens == Screens::On
    }

    /// When the screens are due to go off: the configured time after the
    /// user was last there, if a time is configured.
    pub(crate) fn screens_off_at(&self) -> Option<Instant> {
        let after = u64::from(self.config().general.idle.screen_off);
        let since = self.presence.last_active.filter(|_| after > 0);
        since.map(|since| since + Duration::from_secs(after))
    }

    /// When the session is to be locked, and when the machine put to sleep,
    /// for the user being away: the configured times after they were last
    /// there, where times are configured and it has not been done yet.
    pub(crate) fn away_at(&self) -> [Option<Instant>; 2] {
        let idle = self.config().general.idle;
        let since = self.presence.last_active;
        let at = |after: u32, done: Option<Instant>| {
            let since = since.filter(|since| after > 0 && done != Some(*since));
            since.map(|since| since + Duration::from_secs(u64::from(after)))
        };
        [
            at(idle.lock, self.presence.locked_for),
            at(idle.sleep, self.presence.slept_for),
        ]
    }

    /// Locks the session, and puts the machine to sleep, once the user has
    /// been away for as long as the configuration says, as of `now`. Each
    /// is done once a time away; the session is not locked again if it is
    /// locked already.
    fn act_on_absence(&mut self, now: Instant) {
        let [lock, sleep] = self.away_at().map(|at| at.is_some_and(|at| at <= now));
        let away = self.presence.last_active;
        if lock {
            self.presence.locked_for = away;
            if !self.is_locked() {
                self.start_locker();
            }
        }
        if sleep {
            self.presence.slept_for = away;
            // Asked as the menu asks it, of what speaks to logind, which
            // has the session locked before the machine sleeps.
            self.ask_session("sleep");
        }
    }

    /// Notes whether the user is there as of `now`, and settles whether the
    /// screens are on. Something being watched counts as the user being
    /// there, as it does for the session going idle.
    pub(crate) fn rest_or_wake(&mut self, now: Instant) {
        let presence = &mut self.presence;
        let here = std::mem::take(&mut presence.stirred) || presence.kept_awake;
        if here || presence.last_active.is_none() {
            presence.last_active = Some(now);
        }
        let on = self.screens_off_at().is_none_or(|at| now < at);
        self.presence.screens = match (on, self.presence.screens) {
            (true, _) => Screens::On,
            (false, Screens::Off) => Screens::Off,
            (false, _) => Screens::ToBeOff,
        };
        self.act_on_absence(now);
    }
}

impl SessionLockHandler for Compositor {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.lock_manager
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        // Locking is for the program that was started to: any other could
        // lock the user out, or show what only looks like a lock.
        let client = confirmation.ext_session_lock().client();
        if !Self::roles(client.as_ref()).has(Roles::LOCK) {
            self.refuse(client.as_ref(), "lock the session", "lock");
            return;
        }
        if self.lock.as_ref().is_some_and(|lock| lock.by.is_alive()) {
            // Somebody holds the lock. Dropping the request refuses it.
            return;
        }
        // Nothing half done survives into, or out of, a locked session.
        self.menu = None;
        self.drag = None;
        for head in self.every_head() {
            head.blanked.set(false);
        }
        self.lock = Some(Lock {
            by: confirmation.ext_session_lock().clone(),
            surfaces: Vec::new(),
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

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        // Only the locker's own surface is ever shown. A client whose
        // request to lock was refused still has an object to ask with, and
        // must not get to put its surface over the real one, or to show one
        // when the session is not locked at all.
        let client = surface.wl_surface().client();
        let lock = self.lock.as_mut();
        let held = lock.filter(|lock| lock.by.client() == client);
        if let Some((lock, output)) = held.zip(Output::from_resource(&output)) {
            lock.surfaces.push((output, surface));
            self.configure_lock_surfaces();
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
