//! Programs written for X11.
//!
//! They talk to `Xwayland`, an X server that is itself a Wayland client of
//! this compositor, and the compositor is the window manager that X server
//! expects to find. An X11 window is then a window like any other: it is
//! placed, framed, focused and closed by the same desktop model.
//!
//! See `docs/adr/0027-xwayland.md`.

use std::borrow::Cow;
use std::ffi::OsString;
use std::fmt::Display;
use std::process::Stdio;
use std::time::Instant;

use omorfos_core::geometry::Rect;
use omorfos_core::{Desktop, WindowId};
use smithay::backend::input::KeyState;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::Window;
use smithay::input::Seat;
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState};
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle};
use smithay::utils::{IsAlive, Logical, Rectangle, Serial};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::{Reorder, ResizeEdge, WmWindowProperty, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XwmHandler};

use crate::compositor::{Compositor, Mapped, logical_size, set};
use crate::input::Wanted;
use crate::restarts::Restarts;

/// What a window is to the program it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Role {
    /// A window of a program written for Wayland.
    Xdg(ToplevelSurface),
    /// A window of a program written for X11.
    X11(X11Surface),
}

/// Where a window has been put, and in what state.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Placed {
    /// The part of the frame that is the window's own.
    pub(crate) client: Rect,
    pub(crate) maximized: bool,
    pub(crate) fullscreen: bool,
}

impl Role {
    /// Asks the program to close the window.
    pub(crate) fn close(&self) {
        match self {
            Self::Xdg(toplevel) => toplevel.send_close(),
            // The only failure is the X server having gone, and the window
            // with it.
            Self::X11(window) => drop(window.close()),
        }
    }

    /// What has the keyboard when the window does.
    pub(crate) fn focus(&self, surface: &WlSurface) -> Focus {
        match self {
            Self::Xdg(_) => Focus::Surface(surface.clone()),
            Self::X11(window) => Focus::X11(window.clone()),
        }
    }

    /// Tells the program whether its window has focus and, if it is being
    /// shown, where it is and in what state. Nothing is said that the
    /// program already knows.
    pub(crate) fn arrange(&self, activated: bool, placed: Option<Placed>) {
        match self {
            Self::Xdg(toplevel) => {
                toplevel.with_pending_state(|state| {
                    set(&mut state.states, ToplevelState::Activated, activated);
                    if let Some(placed) = placed {
                        set(
                            &mut state.states,
                            ToplevelState::Maximized,
                            placed.maximized,
                        );
                        set(
                            &mut state.states,
                            ToplevelState::Fullscreen,
                            placed.fullscreen,
                        );
                        state.size = Some(logical_size(placed.client.size));
                    }
                });
                toplevel.send_pending_configure();
            }
            Self::X11(window) => {
                // Each of these fails only if the X server has gone.
                if window.is_activated() != activated {
                    let _ = window.set_activated(activated);
                }
                let Some(placed) = placed else {
                    return;
                };
                if window.is_maximized() != placed.maximized {
                    let _ = window.set_maximized(placed.maximized);
                }
                if window.is_fullscreen() != placed.fullscreen {
                    let _ = window.set_fullscreen(placed.fullscreen);
                }
                // A program written for X11 places its menus by where its
                // window is, so it is told that as well as its size.
                let corner = (placed.client.origin.x, placed.client.origin.y);
                let area = Rectangle::new(corner.into(), logical_size(placed.client.size));
                if window.geometry() != area {
                    let _ = window.configure(area);
                }
            }
        }
    }
}

/// What has the keyboard.
#[derive(Debug, Clone, PartialEq)]
pub enum Focus {
    /// A surface of a program written for Wayland, or one of ours.
    Surface(WlSurface),
    /// A window of a program written for X11, which is told so by way of
    /// the X server as well.
    X11(X11Surface),
}

impl IsAlive for Focus {
    fn alive(&self) -> bool {
        match self {
            Self::Surface(surface) => surface.alive(),
            Self::X11(window) => window.alive(),
        }
    }
}

impl WaylandFocus for Focus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        match self {
            Self::Surface(surface) => Some(Cow::Borrowed(surface)),
            Self::X11(window) => window.wl_surface().map(Cow::Owned),
        }
    }
}

impl KeyboardTarget<Compositor> for Focus {
    fn enter(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        match self {
            Self::Surface(surface) => KeyboardTarget::enter(surface, seat, data, keys, serial),
            Self::X11(window) => KeyboardTarget::enter(window, seat, data, keys, serial),
        }
    }

    fn leave(&self, seat: &Seat<Compositor>, data: &mut Compositor, serial: Serial) {
        match self {
            Self::Surface(surface) => KeyboardTarget::leave(surface, seat, data, serial),
            Self::X11(window) => KeyboardTarget::leave(window, seat, data, serial),
        }
    }

    fn key(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    ) {
        match self {
            Self::Surface(surface) => {
                KeyboardTarget::key(surface, seat, data, key, state, serial, time);
            }
            Self::X11(window) => KeyboardTarget::key(window, seat, data, key, state, serial, time),
        }
    }

    fn modifiers(
        &self,
        seat: &Seat<Compositor>,
        data: &mut Compositor,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        match self {
            Self::Surface(surface) => {
                KeyboardTarget::modifiers(surface, seat, data, modifiers, serial);
            }
            Self::X11(window) => KeyboardTarget::modifiers(window, seat, data, modifiers, serial),
        }
    }
}

/// An X server that has been started.
#[derive(Debug)]
struct Server {
    process: XWayland,
    /// The Wayland client it is.
    client: Client,
    /// What wakes the loop when it says it is ready, until it has.
    waking: Option<RegistrationToken>,
}

/// The X server and what the compositor keeps as its window manager.
#[derive(Debug)]
pub(crate) struct X11 {
    shell: XWaylandShellState,
    /// The X server, once it has been started.
    server: Option<Server>,
    /// When the X server was started again after stopping, lately.
    restarts: Restarts,
    /// An X server that has stopped and is to be replaced, once its window
    /// manager has heard the last of it. It is kept until then because it
    /// holds its number, which the next is to have.
    stopped: Option<Server>,
    /// The window manager, once the X server is ready for one.
    wm: Option<X11Wm>,
    /// Windows that are to be shown and are not part of the desktop, the
    /// last to come last: those that manage themselves, which menus and
    /// tooltips are, and any that have yet to draw anything.
    apart: Vec<(X11Surface, WlSurface)>,
}

impl X11 {
    pub(crate) fn new(display: &DisplayHandle) -> Self {
        Self {
            shell: XWaylandShellState::new::<Compositor>(display),
            server: None,
            restarts: Restarts::default(),
            stopped: None,
            wm: None,
            apart: Vec::new(),
        }
    }

    /// The windows that manage themselves and are showing, with the
    /// surface each is drawn from: the last to come is on top.
    pub(crate) fn unmanaged(&self) -> impl DoubleEndedIterator<Item = &(X11Surface, WlSurface)> {
        let apart = self.apart.iter();
        apart.filter(|(window, _)| window.is_override_redirect())
    }

    /// What programs written for X11 are to be told to connect to, once
    /// the X server has been started.
    pub(crate) fn display(&self) -> Option<OsString> {
        // One that is about to be replaced is replaced where it was.
        let server = self.server.as_ref().or(self.stopped.as_ref());
        server.map(|server| format!(":{}", server.process.display_number()).into())
    }
}

impl Compositor {
    /// Starts the X server that programs written for X11 run through,
    /// unless the configuration turns it off or it has been started.
    /// `Xwayland` has to be installed.
    ///
    /// `display` is the number the X server is to have, as in `DISPLAY=:1`.
    /// Without one, the first that is free is taken.
    ///
    /// Not being able to start it is a problem the user is told of.
    pub fn start_xwayland(&mut self, display: Option<u32>) {
        if !self.config().general.xwayland.enabled || self.x11.server.is_some() {
            return;
        }
        let none: [(OsString, OsString); 0] = [];
        let quiet = Stdio::null;
        let started = XWayland::spawn(&self.display, display, none, true, quiet(), quiet(), |_| ());
        let started = self.or_problem("cannot run programs written for X11", started);
        self.x11.server = started.map(|(process, client)| Server {
            // The loop is woken when the X server says it is ready, which
            // is noticed the next time round.
            waking: self.wake_on(process.poll_fd()),
            process,
            client,
        });
    }

    /// What a step that can fail came to, the failure being told to the
    /// user as a problem.
    fn or_problem<T, E: Display>(&mut self, what: &str, result: Result<T, E>) -> Option<T> {
        match result {
            Ok(done) => Some(done),
            Err(error) => {
                self.problems.push(format!("{what}: {error}"));
                None
            }
        }
    }

    /// Looks after the X server: becomes its window manager once it is
    /// ready for one, and starts it again if it has stopped.
    pub(crate) fn manage_x11(&mut self, now: Instant) {
        // An X server that has stopped is no longer a client of ours.
        let display = self.display.clone();
        let stopped = |server: &mut Server| server.client.get_credentials(&display).is_err();
        if let Some(server) = self.x11.server.take_if(stopped) {
            self.recover_x11(server, now);
        }
        // The window manager of an X server that stopped is kept until it
        // has heard everything that X server had still to say. Only then
        // is there room for another.
        let free = self.x11.wm.is_none();
        if let Some(stopped) = self.x11.stopped.take_if(|_| free) {
            // Its number is free the moment it is let go of, and is taken
            // again at once, before anything else can have it.
            let number = stopped.process.display_number();
            drop(stopped);
            self.start_xwayland(Some(number));
        }
        let x11 = &mut self.x11;
        let starting = x11.server.as_mut().filter(|_| x11.wm.is_none());
        let ready = starting.and_then(|server| {
            // An X server that is not ready yet has nothing to take.
            let socket = server.process.take_socket().ok().flatten();
            socket.map(|socket| (socket, server.client.clone(), server.waking.take()))
        });
        if let Some((socket, client, waking)) = ready {
            // It has said what it had to say, and would wake the loop for
            // ever after if it were still listened to.
            self.stop_waking(waking);
            let manager = X11Wm::start_wm(self.event_loop.clone(), socket, client);
            self.x11.wm = self.or_problem("cannot manage programs written for X11", manager);
        }
    }

    /// Stops waking the loop for what an X server has to say.
    fn stop_waking(&self, waking: Option<RegistrationToken>) {
        if let Some(waking) = waking {
            self.event_loop.remove(waking);
        }
    }

    /// Clears up after an X server that has stopped, and starts another in
    /// its place, with the same number so that programs already told where
    /// it is can still find it. One that keeps stopping is given up on.
    fn recover_x11(&mut self, mut server: Server, now: Instant) {
        self.stop_waking(server.waking.take());
        // Every window it had has gone with it.
        let gone = self.windows.iter();
        let gone = gone.filter(|mapped| matches!(mapped.role, Role::X11(_)));
        let gone: Vec<WindowId> = gone.map(|mapped| mapped.id).collect();
        for id in gone {
            self.remove_window(id);
        }
        self.x11.apart.clear();

        if self.x11.restarts.allow(now) {
            let said = "the X server stopped and has been started again; programs written for \
                        X11 that were running have closed";
            self.problems.push(said.to_owned());
            self.x11.stopped = Some(server);
        } else {
            let said = "the X server keeps stopping; programs written for X11 will not run \
                        until you log in again";
            self.problems.push(said.to_owned());
        }
    }

    /// Has the X server put a window that is being given the keyboard
    /// over its others, as the desktop has it.
    pub(crate) fn raise_x11(&mut self, focus: Option<&Focus>) {
        if let (Some(Focus::X11(window)), Some(manager)) = (focus, self.x11.wm.as_mut()) {
            // The only failure is the X server having gone.
            let _ = manager.raise_window(window);
        }
    }

    /// The window of a program written for X11 that is part of the desktop.
    fn managed(&self, window: &X11Surface) -> Option<&Mapped> {
        let role = Role::X11(window.clone());
        self.windows.iter().find(|mapped| mapped.role == role)
    }

    /// Hands a window of a program written for X11 to the desktop once it
    /// has drawn something to show.
    pub(crate) fn map_x11_if_ready(&mut self, surface: &WlSurface) {
        let mut apart = self.x11.apart.iter();
        let found =
            apart.position(|(window, own)| own == surface && !window.is_override_redirect());
        let drawn = with_renderer_surface_state(surface, |state| state.buffer().is_some());
        let Some(index) = found.filter(|_| drawn == Some(true)) else {
            return;
        };
        let (x11, _) = self.x11.apart.remove(index);
        let window = Window::new_x11_window(x11.clone());
        window.on_commit();
        let size = x11.geometry().size;
        let size = omorfos_core::geometry::Size::new(size.w.unsigned_abs(), size.h.unsigned_abs());
        let (app_id, title) = (x11.class(), x11.title());
        let listed = self.foreign_toplevels.new_toplevel::<Self>(&title, &app_id);
        // A window that does not draw its own titlebar is given ours.
        let decorated = !x11.is_decorated();
        let at = self.pointer_position();
        let id = self.desktop.add_window(app_id, title, size, at, decorated);
        self.windows.push(Mapped {
            id,
            window,
            surface: surface.clone(),
            role: Role::X11(x11),
            listed,
        });
    }

    /// Lets go of a window of a program written for X11 that is no longer
    /// to be shown.
    fn forget_x11(&mut self, window: &X11Surface) {
        self.x11.apart.retain(|(apart, _)| apart != window);
        if let Some(id) = self.managed(window).map(|mapped| mapped.id) {
            self.remove_window(id);
        }
    }

    /// Applies an operation to a window of a program written for X11, if
    /// it is part of the desktop.
    fn on_x11(&mut self, window: &X11Surface, operation: impl FnOnce(&mut Desktop, WindowId)) {
        if let Some(id) = self.managed(window).map(|mapped| mapped.id) {
            operation(&mut self.desktop, id);
        }
    }

    /// Starts moving or resizing the frame of a window of a program
    /// written for X11, which has asked for it.
    fn begin_x11(&mut self, window: &X11Surface, wanted: impl FnOnce(&Self, WindowId) -> Wanted) {
        if let Some(id) = self.managed(window).map(|mapped| mapped.id) {
            let wanted = wanted(self, id);
            self.begin(id, wanted);
        }
    }
}

impl XWaylandShellHandler for Compositor {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.x11.shell
    }

    fn surface_associated(&mut self, _: XwmId, surface: WlSurface, window: X11Surface) {
        // From here on the window has something to be drawn from. One
        // that does not manage itself joins the desktop once it has drawn,
        // which it may have already.
        self.x11.apart.push((window, surface.clone()));
        self.map_x11_if_ready(&surface);
    }
}

impl XwmHandler for Compositor {
    fn xwm_state(&mut self, _: XwmId) -> &mut X11Wm {
        #[expect(
            clippy::expect_used,
            reason = "only the window manager calls this, so there is one"
        )]
        let manager = self.x11.wm.as_mut().expect("no X11 window manager");
        manager
    }

    fn new_window(&mut self, _: XwmId, _: X11Surface) {}

    fn new_override_redirect_window(&mut self, _: XwmId, _: X11Surface) {}

    fn map_window_request(&mut self, _: XwmId, window: X11Surface) {
        // Every window may be shown. It joins the desktop when it has
        // drawn something.
        let _ = window.set_mapped(true);
    }

    fn mapped_override_redirect_window(&mut self, _: XwmId, _: X11Surface) {}

    fn unmapped_window(&mut self, _: XwmId, window: X11Surface) {
        self.forget_x11(&window);
    }

    fn destroyed_window(&mut self, _: XwmId, window: X11Surface) {
        self.forget_x11(&window);
    }

    fn configure_request(
        &mut self,
        _: XwmId,
        window: X11Surface,
        _x: Option<i32>,
        _y: Option<i32>,
        width: Option<u32>,
        height: Option<u32>,
        _reorder: Option<Reorder>,
    ) {
        // A window that is not part of the desktop yet may be whatever
        // size it likes. One that is has the size the desktop gave it.
        if self.managed(&window).is_none() {
            let mut area: Rectangle<i32, Logical> = window.geometry();
            let wide = width.and_then(|width| i32::try_from(width).ok());
            let tall = height.and_then(|height| i32::try_from(height).ok());
            area.size.w = wide.unwrap_or(area.size.w);
            area.size.h = tall.unwrap_or(area.size.h);
            let _ = window.configure(area);
        }
    }

    fn configure_notify(
        &mut self,
        _: XwmId,
        _: X11Surface,
        _: Rectangle<i32, Logical>,
        _: Option<u32>,
    ) {
    }

    fn property_notify(&mut self, _: XwmId, window: X11Surface, property: WmWindowProperty) {
        let title = (property == WmWindowProperty::Title).then(|| window.title());
        if let Some((mapped, title)) = self.managed(&window).zip(title) {
            mapped.listed.send_title(&title);
            mapped.listed.send_done();
            let id = mapped.id;
            self.desktop.set_title(id, title);
        }
    }

    fn maximize_request(&mut self, _: XwmId, window: X11Surface) {
        self.on_x11(&window, |desktop, id| desktop.set_maximized(id, true));
    }

    fn unmaximize_request(&mut self, _: XwmId, window: X11Surface) {
        self.on_x11(&window, |desktop, id| desktop.set_maximized(id, false));
    }

    fn fullscreen_request(&mut self, _: XwmId, window: X11Surface) {
        self.on_x11(&window, |desktop, id| desktop.set_fullscreen(id, true));
    }

    fn unfullscreen_request(&mut self, _: XwmId, window: X11Surface) {
        self.on_x11(&window, |desktop, id| desktop.set_fullscreen(id, false));
    }

    fn resize_request(&mut self, _: XwmId, window: X11Surface, _: u32, _: ResizeEdge) {
        // Whichever edge was asked for, the corner nearest the pointer is
        // what is dragged, as with a drag begun from the keyboard.
        self.begin_x11(&window, |compositor, id| {
            let frame = compositor.frame_facts(id);
            Wanted::Resize(frame.nearest_corner(compositor.pointer_position()))
        });
    }

    fn move_request(&mut self, _: XwmId, window: X11Surface, _: u32) {
        self.begin_x11(&window, |_, _| Wanted::Move);
    }

    fn disconnected(&mut self, _: XwmId) {
        // The X server it was for has gone, and has no more to say.
        self.x11.wm = None;
    }
}

smithay::delegate_xwayland_shell!(Compositor);
