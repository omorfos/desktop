//! The compositor's state and its main operations.

use std::borrow::Cow;
use std::cell::RefCell;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use omorfos_config::{
    Action, Config, Debouncer, Half, Part, States, Store, Style, Theme, Watcher, WorkspaceNumber,
};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{
    Button, Decoration, Desktop, Frame, Home, Item, Menu, MenuMetrics, Place, Screen, Standing,
    WindowId,
};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::{
    Error as RenderError, OutputDamageTracker, RenderOutputResult,
};
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::desktop::{LayerSurface, PopupManager, Space, Window, layer_map_for_output};
use smithay::input::keyboard::{KeyboardHandle, Keycode, XkbConfig};
use smithay::input::pointer::{
    CursorIcon, CursorImageAttributes, CursorImageStatus, PointerHandle,
};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{
    Interest, LoopHandle, Mode as Trigger, PostAction, RegistrationToken,
};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::{
    IsAlive, Logical, Physical, Point, Rectangle, SERIAL_COUNTER, Size, Transform,
};
use smithay::wayland::compositor::{
    CompositorClientState, CompositorState, SurfaceData, send_surface_state, with_states,
};
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::foreign_toplevel_list::{ForeignToplevelHandle, ForeignToplevelListState};
use smithay::wayland::fractional_scale::{FractionalScaleManagerState, with_fractional_scale};
use smithay::wayland::idle_inhibit::IdleInhibitManagerState;
use smithay::wayland::idle_notify::IdleNotifierState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::session_lock::SessionLockManagerState;
use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, Layer, WlrLayerShellState};
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::{
    PopupSurface, ToplevelStateSet, ToplevelSurface, XdgShellState,
};
use smithay::wayland::shm::ShmState;
use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::xdg_activation::XdgActivationState;

use crate::Scale;
use crate::bridge::{self, Bindings};
use crate::corners::{self, Names, Strip};
use crate::cursor::{self, Themes};
use crate::ghost::{Ghost, Noted, Noting};
use crate::input::Drag;
use crate::ipc::Server;
use crate::lock::{Lock, Presence};
use crate::outputs::Head;
use crate::paint::{self, Motion, Text};
use crate::restarts::Restarts;
use crate::reveal::{Placing, Reveal};
use crate::tiles::Lines;
use crate::x11::{Focus, Placed, Role, X11};

/// What the desktop calls itself to programs that behave differently from
/// one desktop to another. The same name is in the login session entry,
/// `packaging/omorfos.desktop`.
pub const DESKTOP_NAME: &str = "omorfos";

/// How long the configuration directory must be quiet before it is reloaded.
const RELOAD_DELAY: Duration = Duration::from_millis(150);

/// How long something that has happened is said to have.
const EVENT_LASTS: Duration = Duration::from_secs(10);

/// What there is to tell the user: what is wrong, and after it what has
/// just happened.
fn said<'a>(problems: &'a [String], events: &[(String, Instant)]) -> Cow<'a, [String]> {
    if events.is_empty() {
        return Cow::Borrowed(problems);
    }
    let events = events.iter().map(|(what, _)| what.clone());
    Cow::Owned(problems.iter().cloned().chain(events).collect())
}

/// Tells a surface the scale of the output it is on, and that it is on it.
/// One that is on no output is told to draw at a scale of one.
fn tell_scale(head: Option<&Head>, surface: &WlSurface, states: &SurfaceData) {
    let scale = head.map_or(Scale::ONE, |head| head.scale);
    send_surface_state(surface, states, scale.whole(), Transform::Normal);
    with_fractional_scale(states, |fractional| {
        fractional.set_preferred_scale(scale.factor());
    });
    if let Some(head) = head {
        head.output.enter(surface);
    }
}

/// Starts watching the configuration directory, creating it if need be.
/// Failing to is not fatal: it is returned as a problem to tell the user.
fn watch(store: &Store) -> (Option<Watcher>, Vec<String>) {
    let watching =
        fs::create_dir_all(store.directory()).and_then(|()| Watcher::new(store.directory()));
    match watching {
        Ok(watcher) => (Some(watcher), Vec::new()),
        Err(error) => {
            let problem = format!(
                "{}: changes will not be picked up automatically: {error}",
                store.directory().display()
            );
            (None, vec![problem])
        }
    }
}

/// Per-client data the compositor attaches to every connection.
#[derive(Debug, Default)]
pub struct ClientState {
    pub(crate) compositor: CompositorClientState,
}

impl smithay::reexports::wayland_server::backend::ClientData for ClientState {}

/// A window that is part of the desktop.
#[derive(Debug)]
pub(crate) struct Mapped {
    pub(crate) id: WindowId,
    pub(crate) window: Window,
    /// The window's own surface.
    pub(crate) surface: WlSurface,
    /// What it is to its program, which is how the program is told things.
    pub(crate) role: Role,
    /// The window's entry in the list offered to taskbars.
    pub(crate) listed: ForeignToplevelHandle,
}

/// The compositor, minus the event loop and the screen.
#[derive(Debug)]
pub struct Compositor {
    pub(crate) display: DisplayHandle,
    pub(crate) surfaces: CompositorState,
    pub(crate) xdg_shell: XdgShellState,
    pub(crate) shm: ShmState,
    pub(crate) seats: SeatState<Self>,
    _output_manager_state: OutputManagerState,
    _decorations: XdgDecorationState,
    pub(crate) seat: Seat<Self>,
    pub(crate) keyboard: KeyboardHandle<Self>,
    pub(crate) pointer: PointerHandle<Self>,
    pub(crate) space: Space<Window>,
    pub(crate) popups: PopupManager,
    /// The outputs, left to right.
    pub(crate) heads: Vec<Head>,
    _fractional_scale: FractionalScaleManagerState,
    _viewporter: ViewporterState,
    _single_pixels: SinglePixelBufferState,
    /// The outputs as the desktop was last told about them.
    pub(crate) screens: Vec<Screen>,
    pub(crate) layer_shell: WlrLayerShellState,
    pub(crate) data_device: DataDeviceState,
    pub(crate) activation: XdgActivationState,
    pub(crate) primary_selection: PrimarySelectionState,
    pub(crate) foreign_toplevels: ForeignToplevelListState,

    pub(crate) desktop: Desktop,
    /// Windows the desktop knows about.
    pub(crate) windows: Vec<Mapped>,
    /// Toplevels that exist but have not shown any content yet.
    pub(crate) unmapped: Vec<ToplevelSurface>,
    /// What is on screen, bottom to top, as of the last sync.
    pub(crate) scene: Vec<Shown>,
    /// The move, resize or tab drag in progress.
    pub(crate) drag: Option<Drag>,
    /// The root menu, while it is open.
    pub(crate) menu: Option<Menu<Action>>,
    menu_chrome: Option<MenuChrome>,
    /// The layer surface that was clicked and accepts the keyboard, until
    /// something else is clicked.
    pub(crate) layer_focus: Option<WlSurface>,
    /// Popups, such as menus, that asked to be dismissed when the user
    /// clicks away from them, oldest first.
    pub(crate) grabbed_popups: Vec<PopupSurface>,
    /// The on-screen notice of what is wrong, while there is something.
    pub(crate) notice: Option<Notice>,
    /// The problems the user has clicked away, hidden until they change.
    pub(crate) dismissed: Vec<String>,
    text: Text,

    store: Store,
    watcher: Option<Watcher>,
    debouncer: Debouncer,
    pub(crate) bindings: Bindings,
    pub(crate) problems: Vec<String>,
    /// What has happened lately that the user might want to know of and
    /// need do nothing about, and when.
    events: Vec<(String, Instant)>,

    /// Keys whose press was consumed, so their release is consumed too.
    pub(crate) suppressed_keys: Vec<Keycode>,
    pub(crate) pointer_location: Point<f64, Logical>,
    /// Whether the compositor is the one to draw the pointer.
    draws_pointer: bool,
    /// What applications last asked the pointer to look like over them.
    pub(crate) cursor: CursorImageStatus,
    /// What the pointer looks like now, as of the last sync.
    shown: Look,
    /// The picture for the shape being shown, kept until it would differ.
    pointer_image: Option<PointerImage>,
    themes: Themes,
    _cursor_shapes: CursorShapeManagerState,
    /// The virtual terminal the user asked to switch to, until the backend
    /// collects it.
    pub(crate) terminal_wanted: Option<i32>,
    /// What the pointer is over, and what it is doing with titlebars.
    pub(crate) hover: Hover,
    /// The lock on the session, while it is locked.
    pub(crate) lock: Option<Lock>,
    pub(crate) lock_manager: SessionLockManagerState,
    /// Tells those who asked when the user has been away for a while.
    pub(crate) idle: IdleNotifierState<Self>,
    _idle_inhibit: IdleInhibitManagerState,
    /// Surfaces that ask for the session not to go idle while they are
    /// shown, such as a film being played.
    pub(crate) inhibitors: Vec<WlSurface>,
    /// Whether the user is there, and so whether the screens are on.
    pub(crate) presence: Presence,
    /// Programs written for X11, and the X server they run through.
    pub(crate) x11: X11,
    children: Vec<Child>,
    /// Whether the programs to start with the session have been.
    started: bool,
    /// The programs that come with omorfos and run for the whole session,
    /// once a backend has said where each is: the panel first.
    components: Vec<Component>,
    /// The launcher, once a backend has said where it is, and the one that
    /// is open, by the number of
    /// its process.
    launcher: Option<PathBuf>,
    launched: Option<u32>,
    /// What is changing colour or coming in, and the time it goes by.
    animation: Animation,
    pub(crate) ipc: Option<Server>,
    /// When IPC peers that could not be sent everything are tried again.
    pub(crate) ipc_retry: Option<Instant>,
    /// The event loop, to be woken by what the compositor waits for.
    pub(crate) event_loop: LoopHandle<'static, Self>,
    socket_name: Option<OsString>,
    running: bool,
}

impl Compositor {
    /// Creates a compositor that reads its configuration from
    /// `config_directory`. It has no output until one is added with
    /// [`Compositor::add_output`].
    ///
    /// `event_loop` is the loop that will drive the compositor. It is woken
    /// by whatever the compositor waits for besides its clients: changes to
    /// the configuration, IPC peers, and programs it started exiting.
    ///
    /// The directory is created if it does not exist. Problems with it, or
    /// with the configuration, do not prevent startup; see
    /// [`Compositor::problems`].
    ///
    /// # Errors
    ///
    /// Returns an error if the keyboard cannot be set up, which means XKB
    /// data is missing from the system.
    pub fn new(
        display: DisplayHandle,
        event_loop: LoopHandle<'static, Self>,
        config_directory: impl Into<PathBuf>,
    ) -> Result<Self, Box<dyn Error>> {
        let store = Store::open(config_directory);
        let (watcher, startup_problems) = watch(&store);

        // Version 6 lets a surface be told the scale to draw at.
        let surfaces = CompositorState::new_v6::<Self>(&display);
        let fractional_scale = FractionalScaleManagerState::new::<Self>(&display);
        let viewporter = ViewporterState::new::<Self>(&display);
        let cursor_shapes = CursorShapeManagerState::new::<Self>(&display);
        // Any client may lock the session: locking gives nothing away.
        let lock_manager = SessionLockManagerState::new::<Self, _>(&display, |_| true);
        let idle = IdleNotifierState::new(&display, event_loop.clone());
        let idle_inhibit = IdleInhibitManagerState::new::<Self>(&display);
        let xdg_shell = XdgShellState::new::<Self>(&display);
        let shm = ShmState::new::<Self>(&display, Vec::new());
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display);
        let decorations = XdgDecorationState::new::<Self>(&display);
        let layer_shell = WlrLayerShellState::new::<Self>(&display);
        let data_device = DataDeviceState::new::<Self>(&display);
        let activation = XdgActivationState::new::<Self>(&display);
        let primary_selection = PrimarySelectionState::new::<Self>(&display);
        let foreign_toplevels = ForeignToplevelListState::new::<Self>(&display);
        let mut seats = SeatState::new();
        let mut seat = seats.new_wl_seat(&display, "seat0");
        let pointer = seat.add_pointer();

        seat.add_keyboard(XkbConfig::default(), 400, 30)
            .map_err(Box::from)
            .map(|keyboard| {
                let mut compositor = Self {
                    _single_pixels: SinglePixelBufferState::new::<Self>(&display),
                    x11: X11::new(&display),
                    display,
                    surfaces,
                    xdg_shell,
                    shm,
                    seats,
                    _output_manager_state: output_manager_state,
                    _decorations: decorations,
                    seat,
                    keyboard,
                    pointer,
                    space: Space::default(),
                    popups: PopupManager::default(),
                    heads: Vec::new(),
                    _fractional_scale: fractional_scale,
                    _viewporter: viewporter,
                    screens: Vec::new(),
                    layer_shell,
                    data_device,
                    activation,
                    primary_selection,
                    foreign_toplevels,
                    desktop: Desktop::new(omorfos_core::Settings::default(), 1),
                    windows: Vec::new(),
                    unmapped: Vec::new(),
                    scene: Vec::new(),
                    drag: None,
                    menu: None,
                    menu_chrome: None,
                    layer_focus: None,
                    grabbed_popups: Vec::new(),
                    notice: None,
                    dismissed: Vec::new(),
                    text: Text::new(),
                    store,
                    watcher,
                    debouncer: Debouncer::new(RELOAD_DELAY),
                    bindings: Bindings::new([]),
                    problems: Vec::new(),
                    events: Vec::new(),
                    suppressed_keys: Vec::new(),
                    pointer_location: (0.0, 0.0).into(),
                    draws_pointer: false,
                    cursor: CursorImageStatus::default_named(),
                    shown: Look::Icon(CursorIcon::Default),
                    pointer_image: None,
                    themes: Themes::default(),
                    _cursor_shapes: cursor_shapes,
                    terminal_wanted: None,
                    hover: Hover::default(),
                    lock: None,
                    lock_manager,
                    idle,
                    _idle_inhibit: idle_inhibit,
                    inhibitors: Vec::new(),
                    presence: Presence::default(),
                    children: Vec::new(),
                    started: false,
                    components: Vec::new(),
                    launcher: None,
                    launched: None,
                    animation: Animation::new(),
                    ipc: None,
                    ipc_retry: None,
                    event_loop,
                    socket_name: None,
                    running: true,
                };
                compositor.apply_config(startup_problems);
                if let Some(watcher) = &compositor.watcher {
                    compositor.wake_on(watcher);
                }
                compositor
            })
    }

    // --- Clients -------------------------------------------------------------

    /// Accepts a client connected over `stream`.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be registered.
    pub fn insert_client(&mut self, stream: UnixStream) -> io::Result<Client> {
        self.display
            .insert_client(stream, Arc::new(ClientState::default()))
    }

    /// Sets the name of the Wayland socket, which programs started by the
    /// compositor receive as `WAYLAND_DISPLAY`.
    pub fn set_socket_name(&mut self, name: impl Into<OsString>) {
        self.socket_name = Some(name.into());
    }

    // --- Output --------------------------------------------------------------

    /// Tells every surface the scale it is shown at and the output it is
    /// on, so that applications draw at that size themselves and are not
    /// stretched. A surface is told once, and again only if either changes.
    ///
    /// A window is shown at the scale of the output whose workspaces it is
    /// on, which is the one most of it is on.
    fn announce_scale(&self) {
        // A window that has shown nothing yet will be placed by the pointer.
        let at_pointer = self.pointer_head();
        for toplevel in &self.unmapped {
            with_states(toplevel.wl_surface(), |states| {
                tell_scale(at_pointer, toplevel.wl_surface(), states);
            });
        }
        // A window is told the scale of the output it belongs to, whether
        // or not it is being shown. Told another's while hidden, it would
        // draw itself for a screen it is not on, and some change size
        // doing so.
        for mapped in &self.windows {
            let mut listed = self.desktop.windows();
            let listed = listed.find(|listed| listed.window.id == mapped.id);
            let head = listed.and_then(|listed| self.heads.get(listed.output));
            mapped
                .window
                .with_surfaces(|surface, states| tell_scale(head, surface, states));
        }
        for head in &self.heads {
            let here = Some(head);
            for layer in layer_map_for_output(&head.output).layers() {
                layer.with_surfaces(|surface, states| tell_scale(here, surface, states));
            }
            if let Some(surface) = self.lock_surface_on(&head.output) {
                with_states(surface, |states| tell_scale(here, surface, states));
            }
        }
        // The picture an application gave for the pointer is a surface too.
        if let Look::Surface(surface) = &self.shown {
            with_states(surface, |states| tell_scale(at_pointer, surface, states));
        }
    }

    // --- Lifecycle -----------------------------------------------------------

    /// Returns `false` once the session has been asked to end.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
    }

    /// Draws the pointer as part of the output from now on. For a backend
    /// where nothing else would show it, or would show what applications
    /// ask it to look like.
    pub fn show_pointer(&mut self) {
        self.draws_pointer = true;
    }

    /// Says where cursor themes are installed. Until this is called there
    /// are none, and the pointer is a plain arrow.
    pub fn set_cursor_themes(&mut self, themes: Themes) {
        self.themes = themes;
        self.pointer_image = None;
    }

    /// Works out what the pointer looks like now and readies its picture.
    fn update_pointer(&mut self) {
        if !self.draws_pointer {
            return;
        }
        self.shown = self.cursor_wanted();
        if let Look::Icon(icon) = self.shown {
            let theme = &self.store.current().general.cursor;
            let scale = self.pointer_scale();
            let current = self.pointer_image.take().filter(|image| {
                image.icon == icon && image.scale == scale && image.theme == *theme
            });
            self.pointer_image =
                current.or_else(|| Some(PointerImage::load(&self.themes, theme, icon, scale)));
        }
    }

    /// The virtual terminal the user has asked to switch to since this was
    /// last called, numbered from 1. Only a backend that holds the seat can
    /// do it; others have nothing to do with the answer.
    pub fn take_terminal_switch(&mut self) -> Option<i32> {
        self.terminal_wanted.take()
    }

    /// The window-management state.
    #[must_use]
    pub const fn desktop(&self) -> &Desktop {
        &self.desktop
    }

    /// The configuration in effect.
    #[must_use]
    pub fn config(&self) -> &Config {
        self.store.current()
    }

    /// What is currently wrong with the configuration or the environment,
    /// described for the user. Empty when all is well.
    #[must_use]
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// What has happened in the last few moments that the user might want
    /// to know of and need do nothing about, described for the user.
    pub fn events(&self) -> impl Iterator<Item = &str> {
        self.events.iter().map(|(what, _)| what.as_str())
    }

    /// What there is to tell the user just now.
    pub(crate) fn said(&self) -> Cow<'_, [String]> {
        said(&self.problems, &self.events)
    }

    /// Notes that something has happened, at `now`, that the user might
    /// want to know of. It is said for a short while.
    pub(crate) fn happened(&mut self, what: &str, now: Instant) {
        self.events.push((what.to_owned(), now));
    }

    /// Wakes the event loop whenever `source` has something to read, and
    /// returns what is needed to stop doing so.
    ///
    /// The loop only wakes; [`Compositor::refresh`] then finds out why, and
    /// has to read everything that is there, because the loop keeps waking
    /// for as long as something is left.
    pub(crate) fn wake_on(&self, source: impl AsFd) -> Option<RegistrationToken> {
        let wake = |_, _: &mut _, _: &mut Self| Ok(PostAction::Continue);
        let readable = source.as_fd().try_clone_to_owned().ok();
        readable
            .map(|readable| Generic::new(readable, Interest::READ, Trigger::Level))
            .and_then(|readable| self.event_loop.insert_source(readable, wake).ok())
    }

    /// Wakes the event loop once, when `child` exits, so that it is
    /// collected then and not whenever something else next happens.
    fn wake_when_exited(&self, child: &Child) {
        // A process stays readable once it has exited, so the source is
        // removed the first time it fires.
        let exited = |_, _: &mut _, _: &mut Self| Ok(PostAction::Remove);
        let _ = pidfd_open(Pid::from_child(child), PidfdFlags::empty())
            .ok()
            .map(|process| Generic::new(process, Interest::READ, Trigger::Level))
            .and_then(|process| self.event_loop.insert_source(process, exited).ok());
    }

    /// The time by which [`Compositor::refresh`] needs calling again even
    /// if nothing else happens, if there is one. Whoever runs the event loop
    /// should wait no longer than this.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        // Once the screens are off there is nothing more to wait for.
        let screens_off = self.screens_off_at().filter(|_| self.screens_on());
        // What is changing colour is drawn again a moment later.
        let moving = self.animation.epoch.zip(self.animation.motion.deadline());
        let moving = moving.map(|(epoch, due)| epoch + due);
        // What has happened is said for a while and then no longer.
        let said = self.events.iter().map(|(_, at)| *at + EVENT_LASTS).min();
        [
            self.debouncer.deadline(),
            self.ipc_retry,
            self.x11.deadline(),
            self.hover.reveal.deadline(),
            screens_off,
            moving,
            said,
        ]
        .into_iter()
        .flatten()
        .min()
    }

    /// Brings everything up to date. Call after dispatching client requests
    /// and input, and before flushing clients and drawing.
    ///
    /// `now` drives the delay between a configuration file changing and its
    /// reload, and how far what is changing colour has got.
    pub fn refresh(&mut self, now: Instant) {
        self.events
            .retain(|(_, at)| now.saturating_duration_since(*at) < EVENT_LASTS);
        let epoch = *self.animation.epoch.get_or_insert(now);
        self.animation.clock = now.saturating_duration_since(epoch);
        if self
            .watcher
            .as_mut()
            .is_some_and(|watcher| watcher.drain().unwrap_or(false))
        {
            self.debouncer.trigger(now);
        }
        if self.debouncer.fire(now) {
            self.reload_config();
        }
        // Collect children that have exited so they do not linger as
        // zombies, noting those that did not exit as they should have.
        let mut failed = Vec::new();
        self.children.retain_mut(|child| {
            // One that cannot even be asked is taken to have failed.
            let waited = child.try_wait();
            let exited = waited.map_or(Some(false), |status| status.map(|status| status.success()));
            if exited == Some(false) {
                failed.push(child.id());
            }
            exited.is_none()
        });
        self.revive(&failed, now);
        self.serve_ipc();
        self.manage_x11(now);
        self.reveal(now);
        self.sync();
        self.rest_or_wake(now);
        self.space.refresh();
        self.popups.cleanup();
        self.publish_ipc(now);
    }

    // --- Configuration -------------------------------------------------------

    /// Reads the configuration again and applies it.
    pub fn reload_config(&mut self) {
        self.store.reload();
        self.apply_config(Vec::new());
    }

    fn apply_config(&mut self, mut problems: Vec<String>) {
        let config = Arc::clone(self.store.current());
        problems.extend(self.store.diagnostics().iter().map(ToString::to_string));

        self.desktop.set_settings(bridge::settings(&config));
        self.desktop.set_rules(bridge::rules(&config.rules));
        self.desktop
            .set_workspace_count(config.general.workspaces.names().len());
        self.apply_scale();

        let keys_file = self.store.directory().join("keys.yaml");
        let (bindings, unknown_keys) = bridge::bindings(&config.keys, &keys_file);
        self.bindings = bindings;
        problems.extend(unknown_keys.iter().map(ToString::to_string));

        let keyboard = self.keyboard.clone();
        let settings = &config.general.keyboard;
        let layout = XkbConfig {
            layout: &settings.layout,
            variant: &settings.variant,
            options: Some(settings.options.clone()).filter(|options| !options.is_empty()),
            ..XkbConfig::default()
        };
        if keyboard.set_xkb_config(self, layout).is_err() {
            problems.push(format!(
                "config.yaml: keyboard layout `{}` (variant `{}`, options `{}`) is not valid; \
                 keeping the previous layout",
                settings.layout, settings.variant, settings.options
            ));
        }
        keyboard.change_repeat_info(settings.repeat_rate.into(), settings.repeat_delay.into());

        self.problems = problems;
    }

    // --- Actions -------------------------------------------------------------

    /// Carries out an action from a key binding or menu entry.
    pub fn perform(&mut self, action: &Action) {
        match action {
            Action::Spawn(command) => self.spawn(command),
            Action::Close => {
                if let Some(mapped) = self.desktop.focused().and_then(|id| self.mapped(id)) {
                    mapped.role.close();
                }
            }
            Action::Maximize => self.on_focused(Desktop::toggle_maximized),
            Action::Fullscreen => self.on_focused(Desktop::toggle_fullscreen),
            Action::Tile(half) => {
                let half = bridge::half(*half);
                self.on_focused(|desktop, id| desktop.tile(id, half));
            }
            Action::Swap(toward) => {
                let toward = bridge::half(*toward);
                self.on_focused(|desktop, id| desktop.swap_toward(id, toward));
            }
            Action::Focus(toward) => self.desktop.focus_toward(bridge::half(*toward)),
            Action::Minimize => self.on_focused(Desktop::minimize),
            Action::Shade => self.on_focused(Desktop::toggle_shaded),
            Action::KeepAbove => self.on_focused(Desktop::toggle_above),
            Action::Stick => self.on_focused(Desktop::toggle_sticky),
            Action::FocusNext => self.desktop.focus_next(),
            Action::FocusPrevious => self.desktop.focus_previous(),
            Action::NextTab => self.on_focused(Desktop::next_tab),
            Action::PreviousTab => self.on_focused(Desktop::previous_tab),
            // Switching workspace applies to the screen the pointer is on.
            Action::Workspace(number) => {
                self.desktop.turn_to(self.pointer_position());
                self.desktop
                    .switch_workspace(bridge::workspace_index(*number));
            }
            Action::NewWorkspace => {
                self.desktop.turn_to(self.pointer_position());
                self.desktop.new_workspace();
            }
            Action::NextWorkspace => {
                self.desktop.turn_to(self.pointer_position());
                self.desktop.next_workspace();
            }
            Action::PreviousWorkspace => {
                self.desktop.turn_to(self.pointer_position());
                self.desktop.previous_workspace();
            }
            Action::MoveToNextOutput => self.on_focused(Desktop::move_to_next_output),
            Action::MoveToWorkspace(number) => {
                let workspace = bridge::workspace_index(*number);
                self.on_focused(|desktop, id| desktop.move_to_workspace(id, workspace));
            }
            Action::MoveToNewWorkspace => self.on_focused(Desktop::move_to_new_workspace),
            Action::ShowMenu => self.open_menu(),
            Action::ShowLauncher => self.open_launcher(),
            Action::ReloadConfig => self.reload_config(),
            Action::Quit => self.running = false,
        }
    }

    /// Opens the root menu at the pointer.
    pub(crate) fn open_menu(&mut self) {
        let config = Arc::clone(self.store.current());
        let items = bridge::menu_items(&config.menu.items);
        self.open_menu_of(items);
    }

    /// Opens the menu of the window that has focus, at the pointer: what
    /// can be done with it, each thing done as its key binding would do
    /// it. A right click on a titlebar opens it, having given that window
    /// focus.
    pub(crate) fn open_window_menu(&mut self) {
        let focused = self.desktop.focused();
        let frame = self
            .desktop
            .frames()
            .find(|frame| Some(frame.window().id) == focused)
            .map(|frame| (frame.standing, frame.maximized));
        // It is opened by a click on a frame, so there is one.
        let (standing, maximized) = frame.unwrap_or_default();
        let entry = |label: &str, action: Action| Item::Entry {
            label: label.to_owned(),
            action,
        };
        // Each says what choosing it will do, which for those that are
        // one way or the other is the opposite of how things are.
        let either = |now: bool, on: &str, off: &str| if now { off } else { on }.to_owned();
        let here = self.desktop.current_output();
        let places = self.desktop.workspaces();
        // Windows are sent to desktops, not to a workspace made for
        // another's fullscreen window.
        let places = places.filter(|place| place.output == here && place.desktop.is_some());
        let send = (1..).zip(places).map(|(number, place)| Item::Entry {
            label: self.workspace_name(&place),
            action: Action::MoveToWorkspace(WorkspaceNumber::clamped(number)),
        });
        let send = send.chain([
            Item::Separator,
            entry("New workspace", Action::MoveToNewWorkspace),
        ]);
        let items = vec![
            entry("Close", Action::Close),
            Item::Entry {
                label: either(maximized, "Maximize", "Restore"),
                action: Action::Maximize,
            },
            entry("Minimize", Action::Minimize),
            entry("Fullscreen", Action::Fullscreen),
            Item::Submenu {
                label: "Tile".to_owned(),
                items: vec![
                    entry("Left half", Action::Tile(Half::Left)),
                    entry("Right half", Action::Tile(Half::Right)),
                    entry("Top half", Action::Tile(Half::Top)),
                    entry("Bottom half", Action::Tile(Half::Bottom)),
                ],
            },
            Item::Separator,
            Item::Entry {
                label: either(standing.shaded, "Shade", "Unshade"),
                action: Action::Shade,
            },
            Item::Entry {
                label: either(standing.above, "Keep above", "Stop keeping above"),
                action: Action::KeepAbove,
            },
            Item::Entry {
                label: either(
                    standing.sticky,
                    "Show on every workspace",
                    "Show on this workspace only",
                ),
                action: Action::Stick,
            },
            Item::Separator,
            Item::Submenu {
                label: "Send to workspace".to_owned(),
                items: send.collect(),
            },
        ];
        self.open_menu_of(items);
    }

    /// What a workspace is called: as the configuration names it, or by
    /// its number where it has no name for it, as it has none for those
    /// that were made.
    pub(crate) fn workspace_name(&self, place: &Place) -> String {
        let configured = self.config().general.workspaces.names();
        if let Some(desktop) = place.desktop {
            let name = configured.get(desktop).cloned();
            return name.unwrap_or_else(|| (desktop + 1).to_string());
        }
        // One made for windows is called what they are.
        let windows = place.tiles.iter();
        let windows = windows.filter_map(|id| self.desktop.window(*id));
        let titles: Vec<&str> = windows.map(|window| window.title.as_str()).collect();
        titles.join(" | ")
    }

    /// Opens a menu of `items` at the pointer.
    fn open_menu_of(&mut self, items: Vec<Item<Action>>) {
        let config = Arc::clone(self.store.current());
        // The menu stays on the output the pointer is on.
        let area = self.pointer_head().map(Head::area).unwrap_or_default();
        let area = Rect::new(
            DomainPoint::new(area.loc.x, area.loc.y),
            DomainSize::new(area.size.w.unsigned_abs(), area.size.h.unsigned_abs()),
        );
        let metrics = MenuMetrics {
            width: MENU_WIDTH,
            row: config.theme.style(Part::MenuItem, States::NONE).height,
            separator: MENU_SEPARATOR,
            padding: MENU_PADDING,
        };
        self.menu = Some(Menu::open(items, self.pointer_position(), area, metrics));
    }

    /// Applies an operation to the focused window, if there is one.
    fn on_focused(&mut self, operation: impl FnOnce(&mut Desktop, WindowId)) {
        if let Some(id) = self.desktop.focused() {
            operation(&mut self.desktop, id);
        }
    }

    fn spawn(&mut self, command: &omorfos_config::Command) {
        self.start(command.program().as_ref(), command.arguments());
    }

    /// Says where the launcher that comes with omorfos is: a name to look
    /// up in `PATH`, or a path. Until this is called there is none to
    /// open.
    pub fn set_launcher(&mut self, program: impl Into<PathBuf>) {
        self.launcher = Some(program.into());
    }

    /// Opens the launcher, unless one is open already.
    fn open_launcher(&mut self) {
        let open = self.launched;
        let open = open.is_some_and(|open| self.children.iter().any(|child| child.id() == open));
        let program = self.launcher.clone().filter(|_| !open);
        if let Some(program) = program {
            self.launched = self.start(program.as_os_str(), &[]);
        }
    }

    /// Starts a program in the session, and returns the number of its
    /// process. One that cannot be started is a problem to tell the user
    /// of.
    fn start(&mut self, program: &OsStr, arguments: &[String]) -> Option<u32> {
        let mut process = std::process::Command::new(program);
        process
            .args(arguments)
            .stdin(Stdio::null())
            .envs(self.session_environment());
        match process.spawn() {
            Ok(child) => {
                self.wake_when_exited(&child);
                let process = child.id();
                self.children.push(child);
                Some(process)
            }
            Err(error) => {
                self.problems
                    .push(format!("cannot start `{}`: {error}", program.display()));
                None
            }
        }
    }

    /// Says where the panel that comes with omorfos is: a name to look up
    /// in `PATH`, or a path. It is then started with the session, unless
    /// the configuration turns it off. Until this is called there is none
    /// to start.
    pub fn set_panel(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Panel, program.into());
    }

    /// Says where the program that shows notifications is: a name to look
    /// up in `PATH`, or a path. It is then started with the session,
    /// unless the configuration turns it off. Until this is called there
    /// is none to start.
    pub fn set_notifier(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Notifier, program.into());
    }

    /// Says where one of the programs that run for the whole session is,
    /// in place of wherever it was said to be before.
    fn set_component(&mut self, kind: Program, program: PathBuf) {
        self.components.retain(|component| component.kind != kind);
        self.components.push(Component {
            kind,
            program,
            process: None,
            restarts: Restarts::default(),
        });
        // The panel goes first, so that it has taken its edge of the screen
        // before anything else is placed.
        self.components.sort_by_key(|component| component.kind);
    }

    /// Starts the panel and the notifications, each if there is one and it
    /// is wanted, and then the programs the configuration lists to be
    /// started with the session. A
    /// backend calls this once it is ready for them: when the sockets are
    /// open and there is a screen to show them on.
    ///
    /// They are started once. Calling this again, or changing the list and
    /// reloading, starts nothing: what is already running would be started
    /// twice.
    pub fn autostart(&mut self) {
        if std::mem::replace(&mut self.started, true) {
            return;
        }
        let config = Arc::clone(self.store.current());
        let mut components = std::mem::take(&mut self.components);
        for component in &mut components {
            if component.kind.wanted(&config.general) {
                component.process = self.start(component.program.as_os_str(), &[]);
            }
        }
        self.components = components;
        for command in &config.general.autostart {
            self.spawn(command);
        }
    }

    /// Starts again the panel or the notifications if either is among the
    /// processes that have just `failed`: that stopped without being asked
    /// to, as a program does when it crashes. One that the configuration
    /// has since turned off is left stopped, and one that keeps stopping
    /// is given up on, which the user is told.
    ///
    /// One that stops of its own accord, without failing, is taken to have
    /// meant to.
    fn revive(&mut self, failed: &[u32], now: Instant) {
        if failed.is_empty() {
            return;
        }
        let config = Arc::clone(self.store.current());
        let mut components = std::mem::take(&mut self.components);
        for component in &mut components {
            let process = component
                .process
                .take_if(|process| failed.contains(process));
            if process.is_none() || !component.kind.wanted(&config.general) {
                continue;
            }
            if component.restarts.allow(now) {
                component.process = self.start(component.program.as_os_str(), &[]);
            } else {
                self.problems.push(format!(
                    "`{}` keeps stopping, and will not be started again until you log in",
                    component.program.display()
                ));
            }
        }
        self.components = components;
    }

    /// What a program has to be told to take part in the session: where
    /// the compositor is, that this is a Wayland session and whose, and
    /// which pointer to draw if it draws its own.
    ///
    /// Programs the compositor starts are given these. A backend that is
    /// the user's whole session passes them on to whatever else starts
    /// programs on the user's behalf.
    #[must_use]
    pub fn session_environment(&self) -> Vec<(&'static str, OsString)> {
        let cursor = &self.store.current().general.cursor;
        let display = self.socket_name.clone();
        let socket = self.ipc_path().map(|path| path.as_os_str().to_owned());
        let fixed = [
            ("XDG_SESSION_TYPE", OsString::from("wayland")),
            ("XDG_CURRENT_DESKTOP", OsString::from(DESKTOP_NAME)),
            ("XCURSOR_THEME", OsString::from(&cursor.theme)),
            (
                "XCURSOR_SIZE",
                OsString::from(cursor.size.get().to_string()),
            ),
        ];
        let located = [
            ("WAYLAND_DISPLAY", display),
            ("OMORFOS_SOCKET", socket),
            ("DISPLAY", self.x11.display()),
        ];
        let located = located
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value)));
        located.chain(fixed).collect()
    }

    /// Whether `surface` is the main surface of a window or panel that is
    /// on screen now.
    pub(crate) fn is_shown(&self, surface: &WlSurface) -> bool {
        let window = |shown: &Shown| shown.surface == *surface;
        let panel = |head: &Head| {
            let layers = layer_map_for_output(&head.output);
            layers.layers().any(|layer| layer.wl_surface() == surface)
        };
        self.scene.iter().any(window) || self.heads.iter().any(panel)
    }

    // --- Windows -------------------------------------------------------------

    pub(crate) fn mapped(&self, id: WindowId) -> Option<&Mapped> {
        self.windows.iter().find(|mapped| mapped.id == id)
    }

    /// The window whose toplevel surface is `surface`, if it is mapped.
    pub(crate) fn window_id(&self, surface: &WlSurface) -> Option<WindowId> {
        self.windows
            .iter()
            .find(|mapped| mapped.surface == *surface)
            .map(|mapped| mapped.id)
    }

    /// Makes Smithay's view of the windows match the desktop model: which
    /// are shown, where, in what order and state, and which has focus. Also
    /// repaints the decoration of any frame whose appearance changed.
    fn sync(&mut self) {
        for head in &self.heads {
            layer_map_for_output(&head.output).cleanup();
        }
        self.update_screens();
        let focused = self.desktop.focused();
        // A frame's decoration is painted for the output most of it is on.
        let scale_for = |geometry: Rect| {
            self.head_for(geometry)
                .map_or(Scale::ONE, |head| head.scale)
        };
        let over = self.hover.button;
        let frames: Vec<FrameInfo> = self
            .desktop
            .frames()
            .map(|frame| FrameInfo::of(&frame, scale_for(frame.geometry), over))
            .collect();
        let config = Arc::clone(self.store.current());
        let mut previous = std::mem::take(&mut self.scene);
        let text = &mut self.text;
        let motion = &mut self.animation.motion;
        motion.begin(self.animation.clock);

        let mut scene = Vec::new();
        for Mapped {
            id,
            window,
            surface,
            role,
            ..
        } in &self.windows
        {
            let frame = frames
                .iter()
                .enumerate()
                .find(|(_, frame)| frame.shown == *id);
            // A window rolled up into its titlebar is left the size it
            // was: it is not shown, but it is not made to be nothing.
            let unrolled = frame.filter(|(_, frame)| !frame.standing.shaded);
            let placed = unrolled.map(|(_, frame)| Placed {
                client: frame.client,
                maximized: frame.maximized,
                // A window that shares its workspace fills a part of the
                // screen and is not fullscreen.
                fullscreen: frame.fullscreen && frame.home == Home::Solo,
                tiled: frame.fullscreen && frame.home == Home::Shared,
            });
            role.arrange(Some(*id) == focused, placed);
            match frame {
                Some((order, frame)) => {
                    let chrome = frame.decoration.map(|decoration| {
                        let cached = previous
                            .iter()
                            .position(|shown| {
                                shown.chrome.as_ref().is_some_and(|c| c.tabs == frame.tabs)
                            })
                            .and_then(|index| previous.swap_remove(index).chrome);
                        let tools = (&mut *text, &mut *motion);
                        Chrome::paint(tools, &config.theme, cached, frame, decoration)
                    });
                    let origin = (frame.client.origin.x, frame.client.origin.y).into();
                    // Every window of the frame is in the picture, so that
                    // showing another of its tabs is not a coming in.
                    let mut opacity = 1.0;
                    for tab in &frame.tabs {
                        let share = motion.presence(&config.theme, Part::Window, tab.raw());
                        if tab == id {
                            opacity = share;
                        }
                    }
                    scene.push((
                        order,
                        Shown {
                            id: *id,
                            opacity,
                            shaded: frame.standing.shaded,
                            over: frame.home != Home::Desktop,
                            window: window.clone(),
                            surface: surface.clone(),
                            outer: frame.geometry,
                            origin,
                            chrome,
                        },
                    ));
                }
                None => self.space.unmap_elem(window),
            }
        }
        // Mapping raises an element, so mapping bottom to top sets the order.
        scene.sort_by_key(|(order, _)| *order);
        self.scene = scene.into_iter().map(|(_, shown)| shown).collect();
        self.lay_lines();
        for shown in &self.scene {
            self.space
                .map_element(shown.window.clone(), shown.origin, false);
        }
        self.animation.leave(previous, &self.scene);

        self.fade_layers(&config.theme);
        self.repaint_overlays();
        self.update_pointer();
        self.announce_scale();
        self.update_idle_inhibition();
        self.sync_keyboard_focus(focused);
    }

    /// Works out how much there is to see of each panel that a shell
    /// component has put up or taken down: one that is named after a part
    /// of the theme comes in, and goes, over the time that part's colours
    /// take to change. A panel of any other program is there at once and
    /// gone at once.
    fn fade_layers(&mut self, theme: &Theme) {
        let animation = &mut self.animation;
        std::mem::swap(&mut animation.layers, &mut animation.before);
        animation.layers.clear();
        for head in &self.heads {
            let layers = layer_map_for_output(&head.output);
            for layer in layers.layers() {
                let surface = layer.wl_surface();
                // Until it has drawn something it is not in the picture.
                let drawn = with_renderer_surface_state(surface, |state| state.buffer().is_some());
                let mut parts = Part::ALL.into_iter();
                let part = parts.find(|part| part.name() == layer.namespace());
                let part = part.filter(|_| drawn == Some(true));
                let mut hasher = DefaultHasher::new();
                surface.id().hash(&mut hasher);
                let key = hasher.finish();
                let present = part.map(|part| Present {
                    surface: surface.clone(),
                    part,
                    layer: layer.layer(),
                    share: animation.motion.presence(theme, part, key),
                });
                animation.layers.extend(present);
            }
        }
        let mut before = std::mem::take(&mut animation.before);
        for was in before.drain(..) {
            let mut now = animation.layers.iter();
            let still = now.any(|now| now.surface == was.surface);
            let pieces = (!still).then(|| animation.noted(&was.surface));
            let gone = pieces.flatten().map(|pieces| Gone {
                part: was.part,
                since: animation.clock,
                from: was.share,
                opacity: was.share,
                level: Level::Layer(was.layer),
                what: What::Surface(None, pieces),
            });
            animation.gone.extend(gone);
        }
        animation.before = before;
    }

    /// Brings the problem notice and the root menu up to date, repainting
    /// each only when what it shows would differ.
    fn repaint_overlays(&mut self) {
        // Show what is wrong until it is put right or the notice is clicked
        // away, and repaint it only when its contents would differ.
        let config = Arc::clone(self.store.current());
        let theme = &config.theme;
        let shown_elsewhere = self.problems_shown_elsewhere();
        let said = said(&self.problems, &self.events);
        if said.is_empty() {
            self.dismissed.clear();
        }
        // What is wrong, and what has just happened, is shown here only if
        // no program is showing it: the notifications that come with
        // omorfos do, among what other programs have to say, where neither
        // covers the other.
        let wanted = !said.is_empty() && *said != *self.dismissed && !shown_elsewhere;
        // The notice is on the first output, which is where a session
        // with one screen has it.
        let first = self.heads.first();
        let within = first.map(Head::area).unwrap_or_default();
        let scale = first.map_or(Scale::ONE, |head| head.scale);
        // One that is no longer wanted fades out as it was.
        let had = self.notice.take();
        let (had, closed) = if wanted { (had, None) } else { (None, had) };
        let closed =
            closed.map(|notice| self.animation.closing(Part::Notice, What::Notice(notice)));
        self.animation.gone.extend(closed);
        let current = had.filter(|notice| {
            notice.problems == *said
                && notice.theme == *theme
                && notice.within == within
                && notice.scale == scale
        });
        self.notice = match (current, wanted) {
            (Some(notice), _) => Some(notice),
            (None, true) => Some(Notice::paint(&mut self.text, theme, &said, within, scale)),
            (None, false) => None,
        };
        let notice = self.notice.as_ref();
        let share = notice.map(|_| self.animation.motion.presence(theme, Part::Notice, 0));
        self.animation.notice_opacity = share.unwrap_or(1.0);

        // Repaint the menu if it, how its rows look, or the theme changed
        // since it was drawn.
        let scale = self.pointer_scale();
        let menu = self.menu.as_ref();
        let looks = menu.map(|menu| paint::menu_looks(theme, &mut self.animation.motion, menu));
        // One that has been closed fades out as it was.
        let had = self.menu_chrome.take();
        let (had, closed) = if menu.is_some() {
            (had, None)
        } else {
            (None, had)
        };
        let closed = closed.map(|chrome| self.animation.closing(Part::Menu, What::Menu(chrome)));
        self.animation.gone.extend(closed);
        let current = had.filter(|chrome| {
            menu == Some(&chrome.menu)
                && Some(&chrome.looks) == looks.as_ref()
                && chrome.theme == *theme
                && chrome.scale == scale
        });
        self.menu_chrome = match (current, menu.zip(looks)) {
            (Some(chrome), _) => Some(chrome),
            (None, Some((menu, looks))) => {
                Some(MenuChrome::paint(&mut self.text, theme, menu, looks, scale))
            }
            (None, None) => None,
        };
        let share = menu.map(|_| self.animation.motion.presence(theme, Part::Menu, 0));
        self.animation.menu_opacity = share.unwrap_or(1.0);
        let locked = self.is_locked();
        self.animation.fade_out(theme, &self.scene, locked);
        // Everything that changes colour, comes in or goes has been asked
        // after by now.
        self.animation.motion.end();
    }

    /// Gives the keyboard to whoever should have it now.
    fn sync_keyboard_focus(&mut self, focused: Option<WindowId>) {
        if self.is_locked() {
            // The lock surface has the keyboard, or nothing does. Nothing
            // below is consulted.
            let focus = self.lock_focus().cloned().map(Focus::Surface);
            let keyboard = self.keyboard.clone();
            if keyboard.current_focus() != focus {
                keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
            }
            return;
        }
        // A launcher or lock prompt that demands the keyboard has it for as
        // long as it is up. Failing that, an open menu has it, so it can be
        // driven with the arrow keys. Otherwise the focused window does.
        self.grabbed_popups.retain(PopupSurface::alive);
        let menu = self
            .grabbed_popups
            .last()
            .map(|popup| popup.wl_surface().clone());
        // A panel that was clicked and accepts the keyboard comes next.
        self.layer_focus = self.layer_focus.take().filter(Resource::is_alive);
        let menu = menu.or_else(|| self.layer_focus.clone());
        let demanding = self.heads.iter().find_map(|head| {
            let layers = layer_map_for_output(&head.output);
            let demanding = layers.layers().rev().find(|layer| {
                layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                    && matches!(layer.layer(), Layer::Top | Layer::Overlay)
            });
            demanding.map(|layer| layer.wl_surface().clone())
        });
        let focus = demanding.or(menu).map(Focus::Surface).or_else(|| {
            let window = focused.and_then(|id| self.mapped(id));
            window.map(|mapped| mapped.role.focus(&mapped.surface))
        });
        // What has just gone cannot be given the keyboard.
        let focus = focus.filter(IsAlive::alive);
        let keyboard = self.keyboard.clone();
        if keyboard.current_focus() != focus {
            self.raise_x11(focus.as_ref());
            keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
        }
    }

    // --- Drawing -------------------------------------------------------------

    /// Draws what `output` shows into `framebuffer`.
    ///
    /// `age` is how many frames old the framebuffer's contents are, or `0`
    /// if unknown, and lets the damage tracker redraw only what changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the renderer fails.
    pub fn render<'tracker, R>(
        &self,
        output: &Output,
        renderer: &mut R,
        framebuffer: &mut R::Framebuffer<'_>,
        tracker: &'tracker mut OutputDamageTracker,
        age: usize,
    ) -> Result<RenderOutputResult<'tracker>, RenderError<R::Error>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let unspecified = Kind::Unspecified;
        let head = self.head(output);
        let view = View {
            origin: head.map(|head| head.area().loc).unwrap_or_default(),
            scale: head.map_or(Scale::ONE, |head| head.scale),
        };
        // Elements are listed from the top of the stack down.
        let mut elements: Vec<Element<R>> = Vec::new();
        self.pointer_elements(renderer, view, &mut elements);
        if self.is_locked() {
            // Nothing of the session is drawn while it is locked: only what
            // the locker shows, on black. This returns before anything
            // else is even looked at.
            let surface = self.lock_surface_on(output).into_iter();
            let shown = surface.flat_map(|surface| {
                render_elements_from_surface_tree(
                    renderer,
                    surface,
                    (0, 0),
                    view.scale.factor(),
                    1.0,
                    unspecified,
                )
            });
            elements.extend(shown.map(Element::Surface));
            if let Some(head) = head {
                head.blanked.set(true);
            }
            return tracker.render_output(renderer, framebuffer, age, &elements, Color32F::BLACK);
        }
        if let Some(notice) = &self.notice {
            let at = Point::<i32, Logical>::from((notice.area.origin.x, notice.area.origin.y));
            elements.extend(notice.pixels.element(
                renderer,
                at.to_f64(),
                view,
                self.animation.notice_opacity,
            ));
        }
        for (at, panel) in self.menu_chrome.iter().flat_map(|chrome| &chrome.panels) {
            elements.extend(panel.element(
                renderer,
                at.to_f64(),
                view,
                self.animation.menu_opacity,
            ));
        }
        self.gone_elements(renderer, view, Level::Over, &mut elements);
        // From the top down: overlays, fullscreen windows, panels and
        // docks, ordinary windows, then whatever sits behind them. What
        // has gone from among each of them is over those that are left.
        let layers = [Layer::Overlay, Layer::Top, Layer::Bottom, Layer::Background];
        let frames = [Some(true), Some(false), None, None];
        for (layer, fullscreen) in layers.into_iter().zip(frames) {
            self.gone_elements(renderer, view, Level::Layer(layer), &mut elements);
            self.layer_elements(
                renderer,
                output,
                view,
                (layer, Placing::Usual),
                &mut elements,
            );
            if let Some(fullscreen) = fullscreen {
                if fullscreen {
                    // Menus and tooltips of programs written for X11 are
                    // over every window, and under what is over all.
                    self.unmanaged_elements(renderer, view, &mut elements);
                    // Panels brought back over a fullscreen window.
                    let raised = (Layer::Top, Placing::Raised);
                    self.layer_elements(renderer, output, view, raised, &mut elements);
                    // The line between windows that share a workspace.
                    let lines = self.lines(|at| view.pixel(at), view.scale.factor());
                    elements.extend(lines.map(Element::Solid));
                }
                self.gone_elements(renderer, view, Level::Frames(fullscreen), &mut elements);
                self.frame_elements(renderer, view, fullscreen, &mut elements);
            }
        }
        tracker.render_output(
            renderer,
            framebuffer,
            age,
            &elements,
            color(
                self.config()
                    .theme
                    .style(Part::Desktop, States::NONE)
                    .background,
            ),
        )
    }

    /// Adds the pointer, if it is the compositor's to draw and is not
    /// hidden: a picture from the cursor theme, or one an application gave.
    fn pointer_elements<R>(&self, renderer: &mut R, view: View, elements: &mut Vec<Element<R>>)
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        if !self.draws_pointer {
            return;
        }
        match (&self.shown, &self.pointer_image) {
            (Look::Icon(_), Some(image)) => {
                let at = self.pointer_location - image.hotspot;
                elements.extend(image.picture.cursor(renderer, at, view));
            }
            (Look::Surface(surface), _) => {
                let hotspot = with_states(surface, |states| {
                    let attributes = states.data_map.get::<Mutex<CursorImageAttributes>>();
                    attributes
                        .and_then(|attributes| attributes.lock().ok().map(|held| held.hotspot))
                        .unwrap_or_default()
                });
                let at = self.pointer_location - hotspot.to_f64();
                let picture = render_elements_from_surface_tree(
                    renderer,
                    surface,
                    view.pixel(at),
                    view.scale.factor(),
                    1.0,
                    Kind::Cursor,
                );
                elements.extend(picture.into_iter().map(Element::Surface));
            }
            (Look::Hidden | Look::Icon(_), _) => {}
        }
    }

    /// Adds the windows of programs written for X11 that manage
    /// themselves, the last to come first.
    fn unmanaged_elements<R>(&self, renderer: &mut R, view: View, elements: &mut Vec<Element<R>>)
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        for (window, surface) in self.x11.unmanaged().rev() {
            let at = window.geometry().loc.to_f64();
            let picture = render_elements_from_surface_tree(
                renderer,
                surface,
                view.pixel(at),
                view.scale.factor(),
                1.0,
                Kind::Unspecified,
            );
            elements.extend(picture.into_iter().map(Element::Surface));
        }
    }

    /// Adds what has gone from one level of what is drawn and is still
    /// fading, the last to go first.
    fn gone_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        level: Level,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let gone = self.animation.gone.iter().rev();
        for gone in gone.filter(|gone| gone.level == level) {
            let opacity = gone.opacity;
            match &gone.what {
                What::Surface(chrome, pieces) => {
                    if let Some(chrome) = chrome {
                        Self::chrome_elements(renderer, view, chrome, opacity, elements);
                    }
                    let pieces = pieces.elements(view.origin, view.scale.factor(), opacity);
                    elements.extend(pieces.map(Element::Ghost));
                }
                What::Menu(chrome) => {
                    for (at, panel) in &chrome.panels {
                        elements.extend(panel.element(renderer, at.to_f64(), view, opacity));
                    }
                }
                What::Notice(notice) => {
                    let at = (notice.area.origin.x, notice.area.origin.y);
                    let at = Point::<i32, Logical>::from(at).to_f64();
                    elements.extend(notice.pixels.element(renderer, at, view, opacity));
                }
            }
        }
    }

    /// Adds the frames that are, or are not, fullscreen, topmost first.
    fn frame_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        fullscreen: bool,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        for shown in self
            .scene
            .iter()
            .rev()
            .filter(|shown| shown.over == fullscreen)
        {
            if let Some(chrome) = &shown.chrome {
                Self::chrome_elements(renderer, view, chrome, shown.opacity, elements);
            }
            if shown.shaded {
                // Rolled up into its titlebar, none of the window is
                // drawn, and none of it is left to fade if it goes.
                let nothing = self.noting::<R::TextureId>(view, Part::Window);
                self.animation.note(&shown.surface, nothing.done());
            } else {
                self.window_elements(renderer, view, shown, elements);
            }
        }
    }

    /// Adds a frame's decoration: its titlebar, its borders and where they
    /// turn the bottom corners.
    fn chrome_elements<R>(
        renderer: &mut R,
        view: View,
        chrome: &Chrome,
        opacity: f32,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let bar = chrome.decoration.titlebar().origin;
        let at = Point::<i32, Logical>::from((bar.x, bar.y)).to_f64();
        elements.extend(chrome.titlebar.element(renderer, at, view, opacity));
        let straight = corners::straight_borders(&chrome.decoration, chrome.radius);
        for (buffer, border) in chrome.borders.iter().zip(straight) {
            let at = Point::<i32, Logical>::from((border.origin.x, border.origin.y));
            let border = SolidColorRenderElement::from_buffer(
                buffer,
                view.pixel(at.to_f64()),
                view.scale.factor(),
                opacity,
                Kind::Unspecified,
            );
            elements.push(Element::Solid(border));
        }
        // Where the border turns the bottom corners.
        let outer = chrome.decoration.outer();
        let size = logical_size(outer.size);
        let radius = chrome.radius.cast_signed();
        let top = outer.origin.y + size.h - radius;
        let lefts = [outer.origin.x, outer.origin.x + size.w - radius];
        for (corner, left) in chrome.corners.iter().zip(lefts) {
            let at = Point::<i32, Logical>::from((left, top)).to_f64();
            elements.extend(corner.element(renderer, at, view, opacity));
        }
    }

    /// Adds what a window itself shows: its pop-ups whole, and its own
    /// picture, which is cut to the frame if the frame's bottom corners are
    /// rounded.
    fn window_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        shown: &Shown,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let scale = smithay::utils::Scale::from(view.scale.factor());
        // What it is drawn from is noted, to fade it out by when it goes.
        let mut noting = self.noting(view, Part::Window);
        // A window's surface may start above and left of its visible
        // geometry, where it draws its own shadows.
        let location = shown.origin - shown.window.geometry().loc;
        let rounded = shown.chrome.as_ref().filter(|chrome| chrome.radius > 0);
        let Some(chrome) = rounded else {
            let surfaces = shown
                .window
                .render_elements::<WaylandSurfaceRenderElement<R>>(
                    renderer,
                    view.pixel(location.to_f64()),
                    scale,
                    shown.opacity,
                );
            for surface in &surfaces {
                noting.add(surface, noting.paint(surface.texture()));
            }
            elements.extend(surfaces.into_iter().map(Element::Surface));
            self.animation.note(&shown.surface, noting.done());
            return;
        };
        // Pop-ups reach beyond the frame and are not cut to it.
        for (popup, offset) in PopupManager::popups_for_surface(&shown.surface) {
            let at = location + shown.window.geometry().loc + offset - popup.geometry().loc;
            let picture: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
                renderer,
                popup.wl_surface(),
                view.pixel(at.to_f64()),
                scale,
                shown.opacity,
                Kind::Unspecified,
            );
            for surface in &picture {
                noting.add(surface, noting.paint(surface.texture()));
            }
            elements.extend(picture.into_iter().map(Element::Surface));
        }
        let pixels = |area: Rect| {
            let corner = Point::<i32, Logical>::from((area.origin.x, area.origin.y));
            let size = logical_size(area.size);
            let from = view.pixel(corner.to_f64());
            let to = view.pixel((corner + size.to_point()).to_f64());
            Rectangle::new(from, (to - from).to_size())
        };
        let areas = corners::strips(
            pixels(chrome.decoration.outer()),
            pixels(chrome.decoration.client()),
            view.scale.pixels(chrome.radius.cast_signed()),
        );
        for (strip, area) in areas.into_iter().enumerate() {
            let whole: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
                renderer,
                &shown.surface,
                view.pixel(location.to_f64()),
                scale,
                shown.opacity,
                Kind::Unspecified,
            );
            for picture in whole {
                let paint = noting.paint(picture.texture());
                let cut = Strip::of(picture, view.scale.factor(), area, strip, &chrome.strips);
                if let Some(cut) = cut {
                    noting.add(&cut, paint);
                    elements.push(Element::Strip(cut));
                }
            }
        }
        self.animation.note(&shown.surface, noting.done());
    }

    /// Begins noting what something is drawn from, if it is a `part` that
    /// takes time to go.
    fn noting<T: Clone + 'static>(&self, view: View, part: Part) -> Noting<T> {
        let going = self.config().theme.style(part, States::NONE).transition;
        Noting::new(going.duration > 0, view.origin, view.scale.factor())
    }

    /// Adds the layer-shell surfaces of one layer that stand as asked,
    /// topmost first.
    fn layer_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        (layer, standing): (Layer, Placing),
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let layers = layer_map_for_output(output);
        let index = self.heads.iter().position(|head| head.output == *output);
        let index = index.unwrap_or_default();
        let wanted = |surface: &&LayerSurface| {
            let area = layers.layer_geometry(surface).unwrap_or_default();
            self.standing(index, layer, area) == standing
        };
        for surface in layers.layers_on(layer).rev().filter(wanted) {
            // A panel is placed within its own output.
            let at = layers.layer_geometry(surface).unwrap_or_default().loc + view.origin;
            // A panel of a shell component may be coming in, and what it
            // is drawn from is noted, to fade it out by when it goes.
            let mut known = self.animation.layers.iter();
            let known = known.find(|known| known.surface == *surface.wl_surface());
            let surfaces = surface.render_elements::<WaylandSurfaceRenderElement<R>>(
                renderer,
                view.pixel(at.to_f64()),
                smithay::utils::Scale::from(view.scale.factor()),
                known.map_or(1.0, |known| known.share),
            );
            if let Some(known) = known {
                let mut noting = self.noting(view, known.part);
                for surface in &surfaces {
                    noting.add(surface, noting.paint(surface.texture()));
                }
                self.animation.note(&known.surface, noting.done());
            }
            elements.extend(surfaces.into_iter().map(Element::Surface));
        }
    }

    /// Tells what is on `output` that a frame was shown there, so that it
    /// draws the next one. `elapsed` is the time since the compositor
    /// started.
    ///
    /// A window is told by the output most of it is on, so that it draws at
    /// that output's pace and not at several at once.
    pub fn send_frames(&mut self, output: &Output, elapsed: Duration) {
        self.confirm_lock();
        let shown = |surface: &WlSurface| {
            send_frames_surface_tree(surface, output, elapsed, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        };
        let here = |head: Option<&Head>| head.is_some_and(|head| head.output == *output);
        if let Some(surface) = self.lock_surface_on(output) {
            shown(surface);
        }
        for frame in &self.scene {
            if here(self.head_for(frame.outer)) {
                frame
                    .window
                    .send_frame(output, elapsed, Some(Duration::ZERO), |_, _| {
                        Some(output.clone())
                    });
            }
        }
        for layer in layer_map_for_output(output).layers() {
            layer.send_frame(output, elapsed, Some(Duration::ZERO), |_, _| {
                Some(output.clone())
            });
        }
        // An application may animate the picture it gave for the pointer.
        if let Look::Surface(surface) = &self.shown
            && here(self.pointer_head())
        {
            shown(surface);
        }
    }
}

/// One output's view of the space, for drawing on it.
#[derive(Debug, Clone, Copy)]
struct View {
    /// Where in the space the output's top-left corner is.
    origin: Point<i32, Logical>,
    scale: Scale,
}

impl View {
    /// The pixel of the output that a point of the space falls on.
    fn pixel(self, at: Point<f64, Logical>) -> Point<i32, Physical> {
        let within = at - self.origin.to_f64();
        within.to_physical(self.scale.factor()).to_i32_round()
    }
}

/// What the pointer looks like.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Look {
    /// Nothing: an application asked for it to be hidden.
    Hidden,
    /// One of the named shapes, drawn from the cursor theme.
    Icon(CursorIcon),
    /// A picture an application drew.
    Surface(WlSurface),
}

/// The picture of a named pointer shape, and everything it depends on.
#[derive(Debug)]
struct PointerImage {
    icon: CursorIcon,
    scale: Scale,
    theme: omorfos_config::Cursor,
    picture: Painted,
    /// Where in the picture the pointer's position is.
    hotspot: Point<f64, Logical>,
}

impl PointerImage {
    /// Finds the shape in the cursor theme at the size the scale calls
    /// for. A shape no theme has is shown as the ordinary pointer, and if
    /// there is not even that, as an arrow painted here.
    fn load(
        themes: &Themes,
        theme: &omorfos_config::Cursor,
        icon: CursorIcon,
        scale: Scale,
    ) -> Self {
        let pixels = scale.pixels(theme.size.get().cast_signed()).unsigned_abs();
        let found = [icon, CursorIcon::Default]
            .into_iter()
            .find_map(|icon| themes.load(&theme.theme, icon, pixels));
        let (picture, hotspot) = found.map_or_else(
            || (Painted::new(&paint::pointer(scale)), (0.0, 0.0)),
            |image| {
                let hotspot = (f64::from(image.hotspot.0), f64::from(image.hotspot.1));
                (Painted::from_image(&image, scale), hotspot)
            },
        );
        Self {
            icon,
            scale,
            theme: theme.clone(),
            picture,
            hotspot: Point::<f64, Physical>::from(hotspot).to_logical(scale.factor()),
        }
    }
}

render_elements! {
    /// Everything the compositor draws: client surfaces and its own
    /// decorations.
    Element<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Strip=Strip<WaylandSurfaceRenderElement<R>>,
    Ghost=Ghost<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
    Solid=SolidColorRenderElement,
}

/// The widest the problem notice gets, and its distance from the edges of
/// the output, in pixels.
const NOTICE_WIDTH: u32 = 520;
const NOTICE_MARGIN: u32 = 12;

/// The notice telling the user what is wrong, kept until that changes.
#[derive(Debug)]
pub(crate) struct Notice {
    problems: Vec<String>,
    theme: Theme,
    /// The output it was laid out on.
    within: Rectangle<i32, Logical>,
    scale: Scale,
    /// Where the notice is: the top-right corner of that output.
    pub(crate) area: Rect,
    pixels: Painted,
}

impl Notice {
    fn paint(
        text: &mut Text,
        theme: &Theme,
        problems: &[String],
        within: Rectangle<i32, Logical>,
        scale: Scale,
    ) -> Self {
        let output_width = within.size.w.unsigned_abs();
        let width = NOTICE_WIDTH.min(output_width.saturating_sub(NOTICE_MARGIN * 2));
        let canvas = paint::notice(text, theme, problems, width, scale);
        let (painted_width, painted_height) = canvas.units();
        let left = output_width
            .saturating_sub(width + NOTICE_MARGIN)
            .cast_signed();
        let area = Rect::new(
            DomainPoint::new(
                within.loc.x + left,
                within.loc.y + NOTICE_MARGIN.cast_signed(),
            ),
            DomainSize::new(painted_width, painted_height),
        );
        Self {
            problems: problems.to_vec(),
            theme: theme.clone(),
            within,
            scale,
            area,
            pixels: Painted::new(&canvas),
        }
    }
}

/// The width of a menu panel, in pixels.
const MENU_WIDTH: u32 = 220;
/// The height of a separator row in a menu.
const MENU_SEPARATOR: u32 = 9;
/// The space above the first and below the last row of a menu panel.
const MENU_PADDING: u32 = 4;

/// Which of the programs that run for the whole session one is. The panel
/// is the first to be started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Program {
    Panel,
    Notifier,
}

impl Program {
    /// Whether the configuration has it running.
    const fn wanted(self, general: &omorfos_config::General) -> bool {
        match self {
            Self::Panel => general.panel.enabled,
            Self::Notifier => general.notifications.enabled,
        }
    }
}

/// A program that comes with omorfos and runs for the whole session.
#[derive(Debug)]
struct Component {
    kind: Program,
    /// A name to look up in `PATH`, or a path.
    program: PathBuf,
    /// The number of its process, while it is running.
    process: Option<u32>,
    restarts: Restarts,
}

/// What the pointer is over, and what it is doing with titlebars.
#[derive(Debug, Default)]
pub(crate) struct Hover {
    /// The window whose frame it is over.
    pub(crate) window: Option<WindowId>,
    /// The button of a titlebar it is over, and the window whose frame
    /// has it.
    pub(crate) button: Option<(WindowId, Button)>,
    /// The button it was pressed on and has yet to be let go on.
    pub(crate) pressed: Option<(WindowId, Button)>,
    /// The titlebar it last pressed on, when and where, which is what a
    /// second press has to match to be a double click.
    pub(crate) clicked: Option<(WindowId, u32, DomainPoint)>,
    /// What it has brought back over a fullscreen window by resting
    /// against an edge.
    pub(crate) reveal: Reveal,
    /// The lines between windows that share a workspace, which it drags.
    pub(crate) lines: Lines,
}

/// What is changing colour, coming in or going.
#[derive(Debug)]
struct Animation {
    /// The frames and menu rows that are on their way from one look to
    /// another, and what has just come into the picture.
    motion: Motion,
    /// The time they go by: how long it is since the first
    /// [`Compositor::refresh`], and when that was.
    clock: Duration,
    epoch: Option<Instant>,
    /// How much of the menu and of the notice there is to see while each
    /// is coming in, from 0 to 1.
    menu_opacity: f32,
    notice_opacity: f32,
    /// The panels of shell components that are on the screen, and those
    /// that were when this was last worked out.
    layers: Vec<Present>,
    before: Vec<Present>,
    /// What each window and panel was last drawn from, noted while
    /// drawing, which does not change anything else.
    drawn: RefCell<Vec<(WlSurface, Noted)>>,
    /// What has gone from the screen and is still fading, oldest first.
    gone: Vec<Gone>,
}

/// A panel of a shell component that is on the screen.
#[derive(Debug)]
struct Present {
    surface: WlSurface,
    /// The part of the theme it is named after.
    part: Part,
    layer: Layer,
    /// How much of it there is to see, from 0 to 1.
    share: f32,
}

/// Something that has gone from the screen, while it fades.
#[derive(Debug)]
struct Gone {
    /// The part of the theme whose time it takes.
    part: Part,
    /// When it went, and how much of it there was to see then.
    since: Duration,
    from: f32,
    /// How much of it there is to see now.
    opacity: f32,
    /// Where among what is drawn it belongs.
    level: Level,
    what: What,
}

/// Where among what is drawn something that has gone belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    /// With the menu and the notice, over everything.
    Over,
    /// With the frames that are, or are not, fullscreen.
    Frames(bool),
    /// With the panels of a layer.
    Layer(Layer),
}

/// What it was that has gone.
#[derive(Debug)]
enum What {
    /// A window, with the decoration it had, or a panel, which has none.
    Surface(Option<Box<Chrome>>, Noted),
    Menu(MenuChrome),
    Notice(Notice),
}

impl Animation {
    fn new() -> Self {
        Self {
            motion: Motion::new(),
            clock: Duration::ZERO,
            epoch: None,
            menu_opacity: 1.0,
            notice_opacity: 1.0,
            layers: Vec::new(),
            before: Vec::new(),
            drawn: RefCell::new(Vec::new()),
            gone: Vec::new(),
        }
    }

    /// Keeps what a window or a panel was drawn from in the frame being
    /// drawn, in place of what it was drawn from before.
    fn note(&self, surface: &WlSurface, noted: Option<Noted>) {
        if let Some(noted) = noted {
            let mut drawn = self.drawn.borrow_mut();
            drawn.retain(|(known, _)| known != surface);
            drawn.push((surface.clone(), noted));
        }
    }

    /// Takes what a window or a panel was last drawn from, if it ever was,
    /// now that it has gone.
    fn noted(&self, surface: &WlSurface) -> Option<Noted> {
        let mut drawn = self.drawn.borrow_mut();
        let found = drawn.iter().position(|(known, _)| known == surface);
        found.map(|index| drawn.swap_remove(index).1.left())
    }

    /// Has the windows that were on the screen and no longer are go on
    /// being drawn, as they last were, while they fade.
    fn leave(&mut self, previous: Vec<Shown>, scene: &[Shown]) {
        for shown in previous {
            let still = scene.iter().any(|now| now.id == shown.id);
            let pieces = (!still).then(|| self.noted(&shown.surface));
            let gone = pieces.flatten().map(|pieces| Gone {
                part: Part::Window,
                since: self.clock,
                from: shown.opacity,
                opacity: shown.opacity,
                level: Level::Frames(shown.over),
                what: What::Surface(shown.chrome.map(Box::new), pieces),
            });
            self.gone.extend(gone);
        }
    }

    /// The menu or the notice, now that it has been closed.
    fn closing(&self, part: Part, what: What) -> Gone {
        // How much of it there was to see was worked out when it was
        // last there.
        let from = match what {
            What::Notice(_) => self.notice_opacity,
            What::Menu(_) | What::Surface(..) => self.menu_opacity,
        };
        Gone {
            part,
            since: self.clock,
            from,
            opacity: from,
            level: Level::Over,
            what,
        }
    }

    /// Works out how much is left to see of everything that has gone,
    /// lets go of what has gone altogether, and of what was noted of
    /// anything that is neither on the screen, as `scene` has it, nor
    /// fading. Nothing of the session lingers once it is `locked`.
    fn fade_out(&mut self, theme: &Theme, scene: &[Shown], locked: bool) {
        for gone in &mut self.gone {
            gone.opacity = gone.from * self.motion.leaving(theme, gone.part, gone.since);
        }
        self.gone.retain(|gone| gone.opacity > 0.0 && !locked);
        let layers = &self.layers;
        self.drawn.get_mut().retain(|(surface, _)| {
            let window = scene.iter().any(|shown| shown.surface == *surface);
            window || layers.iter().any(|layer| layer.surface == *surface)
        });
    }
}

/// The pixels of the open menu, kept until it, how its rows look or the
/// theme changes.
#[derive(Debug)]
struct MenuChrome {
    menu: Menu<Action>,
    /// How each row of each panel looked when it was painted.
    looks: Vec<Vec<Style>>,
    theme: Theme,
    scale: Scale,
    /// Each panel's position and pixels, from the root outwards.
    panels: Vec<(Point<i32, Logical>, Painted)>,
}

impl MenuChrome {
    fn paint(
        text: &mut Text,
        theme: &Theme,
        menu: &Menu<Action>,
        looks: Vec<Vec<Style>>,
        scale: Scale,
    ) -> Self {
        let panels = menu
            .panels()
            .iter()
            .zip(&looks)
            .map(|(panel, looks)| {
                let canvas = paint::menu_panel(text, theme, menu, panel, looks, scale);
                let origin = menu.panel_rect(panel).origin;
                (Point::from((origin.x, origin.y)), Painted::new(&canvas))
            })
            .collect();
        Self {
            menu: menu.clone(),
            looks,
            theme: theme.clone(),
            scale,
            panels,
        }
    }
}

/// Something the compositor painted: its pixels, and the size it stands for
/// on screen. The two differ by the scale it was painted at, at which it is
/// drawn pixel for pixel.
#[derive(Debug)]
struct Painted {
    pixels: MemoryRenderBuffer,
    /// The size on screen, in units of layout.
    size: Size<i32, Logical>,
    /// All of the pixels, which is the part to draw.
    whole: Rectangle<f64, Logical>,
}

impl Painted {
    /// Readies a canvas to be drawn.
    fn new(canvas: &paint::Canvas) -> Self {
        let pixels = (canvas.width().cast_signed(), canvas.height().cast_signed());
        let (width, height) = canvas.units();
        let size = (width.cast_signed(), height.cast_signed());
        Self::from_pixels(canvas.bytes(), pixels, size)
    }

    /// Readies a picture from a cursor theme to be drawn pixel for pixel at
    /// `scale`.
    fn from_image(image: &cursor::Image, scale: Scale) -> Self {
        let pixels = (image.size.0.cast_signed(), image.size.1.cast_signed());
        let size = (scale.units(pixels.0), scale.units(pixels.1));
        Self::from_pixels(&image.pixels, pixels, size)
    }

    /// `bytes` are `pixels` across and down, and stand for `size` units.
    fn from_pixels(bytes: &[u8], pixels: (i32, i32), size: (i32, i32)) -> Self {
        Self {
            pixels: MemoryRenderBuffer::from_slice(
                bytes,
                Fourcc::Argb8888,
                pixels,
                1,
                Transform::Normal,
                None,
            ),
            size: size.into(),
            whole: Rectangle::from_size(Size::from(pixels).to_f64()),
        }
    }

    /// The painting placed at `at` in the space, as an output sees it, as
    /// see-through as `opacity` says, where 1 is not at all. If it cannot
    /// be uploaded to the renderer it is left out, which is better than
    /// failing the whole frame.
    fn element<R>(
        &self,
        renderer: &mut R,
        at: Point<f64, Logical>,
        view: View,
        opacity: f32,
    ) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        self.placed(renderer, at, view, (opacity, Kind::Unspecified))
    }

    /// The painting as the pointer, which a backend may show by other
    /// means than drawing it.
    fn cursor<R>(&self, renderer: &mut R, at: Point<f64, Logical>, view: View) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        self.placed(renderer, at, view, (1.0, Kind::Cursor))
    }

    fn placed<R>(
        &self,
        renderer: &mut R,
        at: Point<f64, Logical>,
        view: View,
        (opacity, kind): (f32, Kind),
    ) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            view.pixel(at).to_f64(),
            &self.pixels,
            Some(opacity),
            Some(self.whole),
            Some(self.size),
            kind,
        )
        .ok()
        .map(Element::Memory)
    }
}

/// A window on screen: where it is and the decoration drawn around it.
#[derive(Debug)]
pub(crate) struct Shown {
    pub(crate) id: WindowId,
    /// Whether it is over the panels: a fullscreen window, or one of its
    /// program's that opened over it.
    pub(crate) over: bool,
    pub(crate) window: Window,
    /// The window's own surface.
    surface: WlSurface,
    /// The whole frame, decoration included.
    pub(crate) outer: Rect,
    /// Where the window's visible geometry starts.
    pub(crate) origin: Point<i32, Logical>,
    /// How much of it there is to see while it is coming in, from 0 to 1.
    opacity: f32,
    /// Whether it is rolled up into its titlebar, with none of the window
    /// showing.
    pub(crate) shaded: bool,
    chrome: Option<Chrome>,
}

impl Shown {
    /// The layout of the frame's titlebar and borders, if it has them.
    pub(crate) fn decoration(&self) -> Option<&Decoration> {
        self.chrome.as_ref().map(|chrome| &chrome.decoration)
    }
}

/// The pixels of a frame's decoration, kept until its appearance changes.
#[derive(Debug)]
struct Chrome {
    /// The windows in the frame, which identify it between syncs.
    tabs: Vec<WindowId>,
    appearance: Appearance,
    decoration: Decoration,
    titlebar: Painted,
    borders: [SolidColorBuffer; 3],
    /// How far the bottom corners are rounded, in units of layout.
    radius: u32,
    /// The border where it turns the bottom corners, left and right.
    corners: [Painted; 2],
    /// What the strips the window is drawn as are called.
    strips: Names,
}

/// Everything a titlebar's pixels depend on. Its position is not part of
/// this, so moving a frame does not repaint it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Appearance {
    titles: Vec<String>,
    /// How the frame and its tabs look, which says which tab is shown and
    /// whether the frame has focus.
    looks: paint::Looks,
    size: DomainSize,
    /// How far the bottom corners are rounded.
    radius: u32,
    theme: Theme,
    scale: Scale,
}

impl Chrome {
    /// Brings a frame's decoration up to date, reusing `cached` pixels when
    /// nothing they depend on has changed.
    fn paint(
        (text, motion): (&mut Text, &mut Motion),
        theme: &Theme,
        cached: Option<Self>,
        frame: &FrameInfo,
        decoration: Decoration,
    ) -> Self {
        let scale = frame.scale;
        let tabs = frame.tabs.iter().map(|window| window.raw());
        let shown = frame.shown.raw();
        let state = (frame.active, frame.hovered);
        let looks = paint::Looks::of(theme, motion, shown, tabs, state, frame.focused);
        let outline = looks.window.border.top.color;
        // A titlebar laid over a fullscreen window is as square as the
        // screen, and the window under it is not cut to any shape.
        let rounded = if frame.fullscreen {
            0
        } else {
            looks.window.radius
        };
        let radius = corners::radius(&decoration, rounded);
        let appearance = Appearance {
            titles: frame.titles.clone(),
            looks,
            size: decoration.titlebar().size,
            radius,
            theme: theme.clone(),
            scale,
        };
        let straight = corners::straight_borders(&decoration, radius);
        // Everything painted is kept for as long as it would look the same.
        let kept = cached.filter(|chrome| chrome.appearance == appearance);
        let mut chrome = kept.unwrap_or_else(|| {
            let titles: Vec<&str> = appearance.titles.iter().map(String::as_str).collect();
            let looks = &appearance.looks;
            let titlebar = paint::titlebar(text, theme, &decoration, &titles, looks, scale);
            let border = straight[0].size.width;
            let corner = |mirrored| {
                let canvas = paint::corner(outline, radius, border, mirrored, scale);
                Painted::new(&canvas)
            };
            Self {
                tabs: frame.tabs.clone(),
                appearance: appearance.clone(),
                decoration,
                titlebar: Painted::new(&titlebar),
                borders: <[SolidColorBuffer; 3]>::default(),
                radius,
                corners: [corner(false), corner(true)],
                strips: Names::default(),
            }
        });
        for (buffer, border) in chrome.borders.iter_mut().zip(straight) {
            // Updating with an unchanged size and colour is not damage.
            buffer.update(logical_size(border.size), color(outline));
        }
        chrome.tabs.clone_from(&frame.tabs);
        chrome.decoration = decoration;
        chrome.radius = radius;
        chrome
    }
}

/// What `sync` needs to know about a frame, copied out of the desktop.
struct FrameInfo {
    shown: WindowId,
    /// The scale of the output most of the frame is on.
    scale: Scale,
    geometry: Rect,
    client: Rect,
    maximized: bool,
    fullscreen: bool,
    /// What kind of workspace it is on.
    home: Home,
    focused: bool,
    decoration: Option<Decoration>,
    tabs: Vec<WindowId>,
    titles: Vec<String>,
    active: usize,
    /// The button of its titlebar that the pointer is over, if any.
    hovered: Option<Button>,
    /// How it stands apart from the others: rolled up, kept above.
    standing: Standing,
}

impl FrameInfo {
    /// What is known of a frame, drawn at `scale`, with the pointer `over`
    /// a button of some frame's titlebar or of none.
    fn of(frame: &Frame<'_>, scale: Scale, over: Option<(WindowId, Button)>) -> Self {
        let shown = frame.window().id;
        let over = over.filter(|(window, _)| *window == shown);
        Self {
            hovered: over.map(|(_, button)| button),
            standing: frame.standing,
            shown,
            scale,
            geometry: frame.geometry,
            client: frame.client(),
            maximized: frame.maximized,
            fullscreen: frame.fullscreen,
            home: frame.home,
            focused: frame.focused,
            decoration: frame.decoration,
            tabs: frame.tabs.iter().map(|window| window.id).collect(),
            titles: frame
                .tabs
                .iter()
                .map(|window| window.title.clone())
                .collect(),
            active: frame.active,
        }
    }
}

/// Converts a configured colour into the renderer's form.
pub(crate) fn color(color: omorfos_config::Color) -> Color32F {
    let channel = |value: u8| f32::from(value) / 255.0;
    Color32F::new(
        channel(color.red),
        channel(color.green),
        channel(color.blue),
        channel(color.alpha),
    )
}

pub(crate) fn set(states: &mut ToplevelStateSet, state: ToplevelState, on: bool) {
    if on {
        states.set(state);
    } else {
        states.unset(state);
    }
}

pub(crate) fn logical_size(size: DomainSize) -> Size<i32, Logical> {
    let clamp = |length: u32| i32::try_from(length).unwrap_or(i32::MAX);
    (clamp(size.width), clamp(size.height)).into()
}
