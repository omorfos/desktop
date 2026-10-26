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
use std::io;
use std::os::fd::OwnedFd;
use std::process::Stdio;
use std::time::{Duration, Instant};

use omorfos_config::X11Scaling;
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
use smithay::wayland::selection::SelectionTarget;
use smithay::wayland::selection::data_device::{
    clear_data_device_selection, current_data_device_selection_userdata,
    request_data_device_client_selection, set_data_device_selection,
};
use smithay::wayland::selection::primary_selection::{
    clear_primary_selection, current_primary_selection_userdata, request_primary_client_selection,
    set_primary_selection,
};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::xwayland_shell::{XWaylandShellHandler, XWaylandShellState};
use smithay::xwayland::xwm::settings::Value;
use smithay::xwayland::xwm::{Reorder, ResizeEdge, WmWindowProperty, XwmId};
use smithay::xwayland::{X11Surface, X11Wm, XWayland, XWaylandClientData, XwmHandler};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, PropMode};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use crate::compositor::{Compositor, Mapped, logical_size, set};
use crate::input::Wanted;
use crate::restarts::Restarts;
use crate::tiles;

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
    /// Whether it fills a part of a workspace it shares, with other
    /// windows or the edges of the screen all round it.
    pub(crate) tiled: bool,
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
                        // Told it has something against every edge, a
                        // program draws its window square and without a
                        // shadow.
                        for edge in [
                            ToplevelState::TiledLeft,
                            ToplevelState::TiledRight,
                            ToplevelState::TiledTop,
                            ToplevelState::TiledBottom,
                        ] {
                            set(&mut state.states, edge, placed.tiled);
                        }
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

/// Sees that what the window manager has asked of the X server has been
/// sent to it.
///
/// Smithay leaves some of what it asks, such as that a selection is now
/// held, waiting to be sent along with whatever is asked next. Until then
/// no program written for X11 knows of it, and next may be a long time.
/// Asking something that has an answer sends everything, and the answer
/// to this is short: which screen comes first, which is none.
fn sent(manager: &X11Wm) {
    let _ = manager.get_randr_primary_output();
}

/// The nearest whole number to a value that is small and not negative.
fn rounded(value: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the value is a few hundred thousand at most, and a cast saturates"
    )]
    let rounded = value.round() as i32;
    rounded
}

/// Leaves how many dots there are to an inch, `Xft.dpi`, among the
/// resources the X server at `display` keeps for programs to read, in
/// place of what was there for it and beside whatever else is.
///
/// Nothing depends on this having worked: a program that is not told
/// draws small.
fn set_density(display: &str, density: i32) {
    let (kind, text) = (AtomEnum::RESOURCE_MANAGER, AtomEnum::STRING);
    let connected = RustConnection::connect(Some(display)).ok();
    let _ = connected.and_then(|(server, screen)| {
        let root = server.setup().roots.get(screen).map(|screen| screen.root);
        root.and_then(|root| {
            let had = server.get_property(false, root, kind, text, 0, u32::MAX);
            let had = had.ok().and_then(|had| had.reply().ok());
            let had = had.map(|had| String::from_utf8_lossy(&had.value).into_owned());
            let had = had.unwrap_or_default();
            let ours = format!("Xft.dpi:\t{density}");
            let others = had.lines().filter(|line| !line.starts_with("Xft.dpi"));
            let lines: Vec<&str> = others.chain([ours.as_str()]).collect();
            let resources = format!("{}\n", lines.join("\n"));
            let set =
                server.change_property8(PropMode::REPLACE, root, kind, text, resources.as_bytes());
            set.ok().and_then(|set| set.check().ok())
        })
    });
}

/// How long the number of an X server that stopped is tried for, when
/// something still holds it.
const TAKING_AGAIN: Duration = Duration::from_secs(5);
/// How long after finding the number held it is tried again.
const TRYING_AGAIN: Duration = Duration::from_millis(50);

/// The X server and what the compositor keeps as its window manager.
#[derive(Debug)]
pub(crate) struct X11 {
    shell: XWaylandShellState,
    /// The X server, once it has been started.
    server: Option<Server>,
    /// When the X server was started again after stopping, lately.
    restarts: Restarts,
    /// How dense programs written for X11 have been told the screen is,
    /// once the X server could be told.
    told: Option<f64>,
    /// An X server that has stopped and is to be replaced, once its window
    /// manager has heard the last of it. It is kept until then because it
    /// holds its number, which the next is to have.
    stopped: Option<Server>,
    /// The number an X server is to be started with in place of one that
    /// stopped and could not be yet, with when to give up, and when to
    /// try next if nothing happens sooner.
    again: Option<(u32, Instant, Instant)>,
    /// The number the session's X server had under the compositor before,
    /// which the next is to have, until it is tried.
    held: Option<u32>,
    /// The window manager, once the X server is ready for one.
    wm: Option<X11Wm>,
    /// Windows that are to be shown and are not part of the desktop, the
    /// last to come last: those that manage themselves, which menus and
    /// tooltips are, and any that have yet to draw anything.
    apart: Vec<(X11Surface, WlSurface)>,
    /// Whether a program written for X11 holds the clipboard, and the
    /// selection, as the X server has them.
    clipboard: Holding,
    primary: Holding,
}

/// Whether a program written for X11 holds a selection, and what is
/// offered in its place that was kept back from the X server meanwhile.
#[derive(Debug, Default)]
struct Holding {
    held: bool,
    back: Option<Vec<String>>,
}

/// The form `omorfos-clipboard` offers what it keeps in beside the others,
/// by which what it keeps is known; its `MARK`.
const KEPT: &str = "application/x-omorfos-kept";

impl X11 {
    /// Whether a program written for X11 holds the clipboard, or the
    /// selection.
    fn holding(&mut self, which: SelectionTarget) -> &mut Holding {
        match which {
            SelectionTarget::Clipboard => &mut self.clipboard,
            SelectionTarget::Primary => &mut self.primary,
        }
    }

    pub(crate) fn new(display: &DisplayHandle) -> Self {
        Self {
            shell: XWaylandShellState::new::<Compositor>(display),
            server: None,
            restarts: Restarts::default(),
            told: None,
            stopped: None,
            again: None,
            held: None,
            wm: None,
            apart: Vec::new(),
            clipboard: Holding::default(),
            primary: Holding::default(),
        }
    }

    /// The windows that manage themselves and are showing, with the
    /// surface each is drawn from: the last to come is on top.
    pub(crate) fn unmanaged(&self) -> impl DoubleEndedIterator<Item = &(X11Surface, WlSurface)> {
        let apart = self.apart.iter();
        apart.filter(|(window, _)| window.is_override_redirect())
    }

    /// When to try again to start an X server in place of one that
    /// stopped, if its number could not be had yet.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.again.map(|(.., next)| next)
    }

    /// What programs written for X11 are to be told to connect to, once
    /// the X server has been started.
    pub(crate) fn display(&self) -> Option<OsString> {
        let number = self.number();
        number.map(|number| format!(":{number}").into())
    }

    /// The number of the session's X server, once it has been started or
    /// is to be: one that is about to be replaced is replaced where it was,
    /// and one the session had before is had again.
    fn number(&self) -> Option<u32> {
        let server = self.server.as_ref().or(self.stopped.as_ref());
        let number = server.map(|server| server.process.display_number());
        let number = number.or(self.again.map(|(number, ..)| number));
        number.or(self.held)
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
        // The number the session had, where it was started again, is taken
        // as one that stopped is: its X server may hold it for a moment yet.
        match display.or(self.held_display()) {
            Some(number) if display.is_none() => self.x11.held = Some(number),
            display => {
                let started = self.spawn_x11(display);
                self.started_x11(started);
            }
        }
    }

    /// The number of the session's X server, once it has been started or
    /// is to be.
    pub(crate) fn x11_number(&self) -> Option<u32> {
        self.x11.number()
    }

    /// Starts an X server with the number `display`, or the first that is
    /// free.
    fn spawn_x11(&self, display: Option<u32>) -> io::Result<(XWayland, Client)> {
        let none: [(OsString, OsString); 0] = [];
        let quiet = Stdio::null;
        XWayland::spawn(&self.display, display, none, true, quiet(), quiet(), |_| ())
    }

    /// Takes up an X server that has been started, or tells the user that
    /// it could not be.
    fn started_x11(&mut self, started: io::Result<(XWayland, Client)>) {
        let started = self.or_problem("cannot run programs written for X11", started);
        self.x11.server = started.map(|(process, client)| Server {
            // The loop is woken when the X server says it is ready, which
            // is noticed the next time round.
            waking: self.wake_on(process.poll_fd()),
            process,
            client,
        });
        // It is to know how many of its pixels there are to a unit of
        // layout before any program draws with it.
        self.x11.told = None;
        self.size_x11_pixels(self.x11_scale());
    }

    /// How many pixels programs written for X11 draw to a unit of layout:
    /// as many as the densest screen has, so that they are sharp on it, or
    /// one if the configuration has them stretched.
    fn x11_scale(&self) -> f64 {
        let native = self.config().general.xwayland.scaling == X11Scaling::Native;
        let densest = self.heads.iter().map(|head| head.scale.factor());
        let densest = densest.fold(1.0, f64::max);
        if native { densest } else { 1.0 }
    }

    /// Says how many pixels of the X server's there are to a unit of
    /// layout. Smithay goes by this for the size of everything the X
    /// server draws and for where everything of its is.
    fn size_x11_pixels(&self, scale: f64) {
        let server = self.x11.server.as_ref();
        let known = server.and_then(|server| server.client.get_data::<XWaylandClientData>());
        let _ = known.map(|known| known.compositor_state.set_client_scale(scale));
    }

    /// Tells programs written for X11 how dense the screen is, when the X
    /// server is first there to tell and whenever it changes. They are
    /// told in the two places such programs look: the settings toolkits
    /// follow as they change, and the resources older programs read once.
    fn scale_x11(&mut self) {
        let scale = self.x11_scale();
        if self.x11.wm.is_none() || self.x11.told == Some(scale) {
            return;
        }
        self.x11.told = Some(scale);
        self.size_x11_pixels(scale);
        // Windows that are there already are as large as they were, which
        // is now a different number of the X server's pixels.
        for mapped in &self.windows {
            if let Role::X11(window) = &mapped.role {
                let _ = window.configure(None);
            }
        }
        // Toolkits scale what they draw by whole numbers, and their text
        // by the rest.
        let whole = scale.floor().max(1.0);
        let dots = 96.0 * 1024.0;
        let pointer = f64::from(self.config().general.cursor.size.get());
        let settings = [
            ("Xft/DPI", dots * scale),
            ("Gdk/UnscaledDPI", dots * scale / whole),
            ("Gdk/WindowScalingFactor", whole),
            // The pointer is as many of the X server's pixels as it takes
            // to be the size it is everywhere else.
            ("Gtk/CursorThemeSize", pointer * scale),
        ];
        let settings = settings.map(|(name, value)| (name.to_owned(), Value::from(rounded(value))));
        let manager = self.x11.wm.as_mut();
        // The only failure is the X server having gone.
        let _ = manager.map(|manager| {
            let _ = manager.set_xsettings(settings.into_iter());
            sent(manager);
        });
        let density = rounded(96.0 * scale);
        let display = self
            .x11
            .display()
            .and_then(|display| display.into_string().ok());
        // Asked of the X server from a thread of its own: it may not
        // answer until it has had an answer from here.
        let _ = display.map(|display| std::thread::spawn(move || set_density(&display, density)));
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
            self.x11.again = Some((number, now + TAKING_AGAIN, now));
        }
        if let Some(number) = self.x11.held.take() {
            self.x11.again = Some((number, now + TAKING_AGAIN, now));
        }
        if let Some((number, until, _)) = self.x11.again.take() {
            let started = self.spawn_x11(Some(number));
            // What is left of the one that stopped can hold on to the
            // number for a moment after it is known to have stopped: a
            // process is not gone all at once. It is tried again until it
            // can be had, and not for ever.
            let held = |error: &io::Error| error.kind() == io::ErrorKind::AddrInUse && now < until;
            if started.as_ref().is_err_and(held) {
                self.x11.again = Some((number, until, now + TRYING_AGAIN));
            } else {
                self.started_x11(started);
            }
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
        self.scale_x11();
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
        self.x11.told = None;

        if self.x11.restarts.allow(now) {
            let said = "the X server stopped and has been started again; programs written for \
                        X11 that were running have closed";
            self.happened(said, now);
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

    /// Tells programs written for X11 what a program written for Wayland
    /// has copied or selected, in what forms, or that it no longer has.
    ///
    /// What the keeper keeps of what a program written for X11 still holds
    /// is kept back: the X server goes on asking that program for it, as it
    /// would anywhere, which a program need not have the keyboard to do, as
    /// `xsel` does not for an editor in a terminal. It is offered once the
    /// program lets go of it.
    pub(crate) fn offer_to_x11(&mut self, which: SelectionTarget, kinds: Option<Vec<String>>) {
        let kept = kinds
            .as_ref()
            .is_some_and(|kinds| kinds.iter().any(|kind| kind == KEPT));
        let holding = self.x11.holding(which);
        if kept && holding.held {
            holding.back = kinds;
            return;
        }
        *holding = Holding::default();
        let manager = self.x11.wm.as_mut();
        // The only failure is the X server having gone.
        let _ = manager.map(|manager| {
            let _ = manager.new_selection(which, kinds);
            sent(manager);
        });
    }

    /// Has what a program written for X11 copied or selected written to
    /// `to`, in the form `kind`, for a program written for Wayland that is
    /// pasting it.
    pub(crate) fn send_from_x11(&mut self, which: SelectionTarget, kind: String, to: OwnedFd) {
        let handle = self.event_loop.clone();
        let manager = self.x11.wm.as_mut();
        // A failure leaves the program that asked with nothing to read.
        let _ = manager.map(|manager| manager.send_selection(which, kind, to, handle));
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
        let (pid, startup) = (x11.pid(), x11.startup_id());
        self.windows.push(Mapped {
            id,
            window,
            surface: surface.clone(),
            role: Role::X11(x11),
            listed,
        });
        // How small it said it can be before it was shown.
        let least = self.windows.last().map(tiles::least).unwrap_or_default();
        self.desktop.set_least(id, least);
        if self.asked_for(surface, pid, startup) {
            self.desktop.activate(id);
        }
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
        // A program written for X11 says how small it can be in a property
        // of its own, which it may change without drawing anything new.
        let hints = (property == WmWindowProperty::NormalHints).then_some(());
        let least = self.managed(&window).zip(hints);
        if let Some((id, least)) = least.map(|(mapped, ())| (mapped.id, tiles::least(mapped))) {
            self.desktop.set_least(id, least);
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

    fn allow_selection_access(&mut self, _: XwmId, _: SelectionTarget) -> bool {
        // As for programs written for Wayland: what has been copied is for
        // whoever has the keyboard, and no one else. Programs written for
        // X11 share it among themselves, which cannot be helped.
        matches!(self.keyboard.current_focus(), Some(Focus::X11(_)))
    }

    fn send_selection(&mut self, _: XwmId, which: SelectionTarget, kind: String, to: OwnedFd) {
        // A program written for X11 is pasting what a program written for
        // Wayland copied, which that program is asked for. If it has gone,
        // there is nothing to paste.
        match which {
            SelectionTarget::Clipboard => {
                let _ = request_data_device_client_selection(&self.seat, kind, to);
            }
            SelectionTarget::Primary => {
                let _ = request_primary_client_selection(&self.seat, kind, to);
            }
        }
    }

    fn new_selection(&mut self, _: XwmId, which: SelectionTarget, kinds: Vec<String>) {
        // What a program written for X11 has copied or selected is offered
        // by the compositor on its behalf.
        *self.x11.holding(which) = Holding {
            held: true,
            back: None,
        };
        match which {
            SelectionTarget::Clipboard => {
                set_data_device_selection(&self.display, &self.seat, kinds, ());
            }
            SelectionTarget::Primary => set_primary_selection(&self.display, &self.seat, kinds, ()),
        }
    }

    fn cleared_selection(&mut self, _: XwmId, which: SelectionTarget) {
        let held = std::mem::take(self.x11.holding(which));
        // What was kept back meanwhile is offered now, in its place.
        if let Some(back) = held.back {
            self.offer_to_x11(which, Some(back));
            return;
        }
        // It is taken back only if it is still what is on offer: a
        // selection of ours is one made on behalf of the X server.
        match which {
            SelectionTarget::Clipboard => {
                let ours = current_data_device_selection_userdata(&self.seat).is_some();
                let _ = ours.then(|| clear_data_device_selection(&self.display, &self.seat));
            }
            SelectionTarget::Primary => {
                let ours = current_primary_selection_userdata(&self.seat).is_some();
                let _ = ours.then(|| clear_primary_selection(&self.display, &self.seat));
            }
        }
    }

    fn disconnected(&mut self, _: XwmId) {
        // The X server it was for has gone, and has no more to say.
        self.x11.wm = None;
    }
}

smithay::delegate_xwayland_shell!(Compositor);
