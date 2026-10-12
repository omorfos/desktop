//! The compositor's state and its main operations.

use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use omorfos_config::{Action, Config, Debouncer, Part, State, States, Store, Theme, Watcher};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Decoration, Desktop, Frame, Menu, MenuMetrics, Screen, WindowId};
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
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::desktop::{PopupManager, Space, Window, layer_map_for_output};
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
use smithay::utils::{Logical, Physical, Point, Rectangle, SERIAL_COUNTER, Size, Transform};
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
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::xdg_activation::XdgActivationState;

use crate::Scale;
use crate::bridge::{self, Bindings};
use crate::corners::{self, Names, Strip};
use crate::cursor::{self, Themes};
use crate::input::Drag;
use crate::ipc::Server;
use crate::lock::{Lock, Presence};
use crate::outputs::Head;
use crate::paint::{self, Text};

/// What the desktop calls itself to programs that behave differently from
/// one desktop to another. The same name is in the login session entry,
/// `packaging/omorfos.desktop`.
pub const DESKTOP_NAME: &str = "omorfos";

/// How long the configuration directory must be quiet before it is reloaded.
const RELOAD_DELAY: Duration = Duration::from_millis(150);

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
    pub(crate) toplevel: ToplevelSurface,
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
    problems: Vec<String>,

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
    pub(crate) hovered: Option<WindowId>,
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
    children: Vec<Child>,
    /// Whether the programs to start with the session have been.
    started: bool,
    /// The panel to start with the session, once a backend has said where
    /// it is.
    panel: Option<PathBuf>,
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
                    suppressed_keys: Vec::new(),
                    pointer_location: (0.0, 0.0).into(),
                    draws_pointer: false,
                    cursor: CursorImageStatus::default_named(),
                    shown: Look::Icon(CursorIcon::Default),
                    pointer_image: None,
                    themes: Themes::default(),
                    _cursor_shapes: cursor_shapes,
                    terminal_wanted: None,
                    hovered: None,
                    lock: None,
                    lock_manager,
                    idle,
                    _idle_inhibit: idle_inhibit,
                    inhibitors: Vec::new(),
                    presence: Presence::default(),
                    children: Vec::new(),
                    started: false,
                    panel: None,
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
        [self.debouncer.deadline(), self.ipc_retry, screens_off]
            .into_iter()
            .flatten()
            .min()
    }

    /// Brings everything up to date. Call after dispatching client requests
    /// and input, and before flushing clients and drawing.
    ///
    /// `now` drives the delay between a configuration file changing and its
    /// reload.
    pub fn refresh(&mut self, now: Instant) {
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
        // Collect children that have exited so they do not linger as zombies.
        self.children
            .retain_mut(|child| child.try_wait().is_ok_and(|status| status.is_none()));
        self.serve_ipc();
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
                    mapped.toplevel.send_close();
                }
            }
            Action::Maximize => self.on_focused(Desktop::toggle_maximized),
            Action::Fullscreen => self.on_focused(Desktop::toggle_fullscreen),
            Action::Minimize => self.on_focused(Desktop::minimize),
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
            Action::ShowMenu => self.open_menu(),
            Action::ReloadConfig => self.reload_config(),
            Action::Quit => self.running = false,
        }
    }

    /// Opens the root menu at the pointer.
    pub(crate) fn open_menu(&mut self) {
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
        let items = bridge::menu_items(&config.menu.items);
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

    /// Starts a program in the session. One that cannot be started is a
    /// problem to tell the user of.
    fn start(&mut self, program: &OsStr, arguments: &[String]) {
        let mut process = std::process::Command::new(program);
        process
            .args(arguments)
            .stdin(Stdio::null())
            .envs(self.session_environment());
        match process.spawn() {
            Ok(child) => {
                self.wake_when_exited(&child);
                self.children.push(child);
            }
            Err(error) => {
                self.problems
                    .push(format!("cannot start `{}`: {error}", program.display()));
            }
        }
    }

    /// Says where the panel that comes with omorfos is: a name to look up
    /// in `PATH`, or a path. It is then started with the session, unless
    /// the configuration turns it off. Until this is called there is none
    /// to start.
    pub fn set_panel(&mut self, program: impl Into<PathBuf>) {
        self.panel = Some(program.into());
    }

    /// Starts the panel, if there is one and it is wanted, and then the
    /// programs the configuration lists to be started with the session. A
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
        // The panel goes first, so that it has taken its edge of the screen
        // before anything else is placed.
        let panel = self.panel.clone();
        if let Some(panel) = panel.filter(|_| config.general.panel.enabled) {
            self.start(panel.as_os_str(), &[]);
        }
        for command in &config.general.autostart {
            self.spawn(command);
        }
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
        let located = [("WAYLAND_DISPLAY", display), ("OMORFOS_SOCKET", socket)];
        let located = located
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value)));
        located.chain(fixed).collect()
    }

    /// Whether `surface` is the main surface of a window or panel that is
    /// on screen now.
    pub(crate) fn is_shown(&self, surface: &WlSurface) -> bool {
        let window = |shown: &Shown| {
            let toplevel = shown.window.toplevel();
            toplevel.is_some_and(|toplevel| toplevel.wl_surface() == surface)
        };
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
            .find(|mapped| mapped.toplevel.wl_surface() == surface)
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
        let frames: Vec<FrameInfo> = self
            .desktop
            .frames()
            .map(|frame| FrameInfo::of(&frame, scale_for(frame.geometry)))
            .collect();
        let config = Arc::clone(self.store.current());
        let mut previous = std::mem::take(&mut self.scene);
        let text = &mut self.text;

        let mut scene = Vec::new();
        for Mapped {
            id,
            window,
            toplevel,
            ..
        } in &self.windows
        {
            let frame = frames
                .iter()
                .enumerate()
                .find(|(_, frame)| frame.shown == *id);
            let activated = Some(*id) == focused;
            toplevel.with_pending_state(|state| {
                set(&mut state.states, ToplevelState::Activated, activated);
                if let Some((_, frame)) = frame {
                    set(&mut state.states, ToplevelState::Maximized, frame.maximized);
                    set(
                        &mut state.states,
                        ToplevelState::Fullscreen,
                        frame.fullscreen,
                    );
                    state.size = Some(logical_size(frame.client.size));
                }
            });
            toplevel.send_pending_configure();
            match frame {
                Some((order, frame)) => {
                    let chrome = frame.decoration.map(|decoration| {
                        let cached = previous
                            .iter()
                            .position(|shown| {
                                shown.chrome.as_ref().is_some_and(|c| c.tabs == frame.tabs)
                            })
                            .and_then(|index| previous.swap_remove(index).chrome);
                        Chrome::paint(text, &config.theme, frame.scale, cached, frame, decoration)
                    });
                    let origin = (frame.client.origin.x, frame.client.origin.y).into();
                    scene.push((
                        order,
                        Shown {
                            id: *id,
                            fullscreen: frame.fullscreen,
                            window: window.clone(),
                            surface: toplevel.wl_surface().clone(),
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
        for shown in &self.scene {
            self.space
                .map_element(shown.window.clone(), shown.origin, false);
        }

        self.repaint_overlays();
        self.update_pointer();
        self.announce_scale();
        self.update_idle_inhibition();
        self.sync_keyboard_focus(focused);
    }

    /// Brings the problem notice and the root menu up to date, repainting
    /// each only when what it shows would differ.
    fn repaint_overlays(&mut self) {
        // Show what is wrong until it is put right or the notice is clicked
        // away, and repaint it only when its contents would differ.
        let config = Arc::clone(self.store.current());
        let theme = &config.theme;
        if self.problems.is_empty() {
            self.dismissed.clear();
        }
        let wanted = !self.problems.is_empty() && self.problems != self.dismissed;
        // The notice is on the first output, which is where a session
        // with one screen has it.
        let first = self.heads.first();
        let within = first.map(Head::area).unwrap_or_default();
        let scale = first.map_or(Scale::ONE, |head| head.scale);
        let current = self.notice.take().filter(|notice| {
            wanted
                && notice.problems == self.problems
                && notice.theme == *theme
                && notice.within == within
                && notice.scale == scale
        });
        self.notice = match (current, wanted) {
            (Some(notice), _) => Some(notice),
            (None, true) => Some(Notice::paint(
                &mut self.text,
                theme,
                &self.problems,
                within,
                scale,
            )),
            (None, false) => None,
        };

        // Repaint the menu if it, or the theme, changed since it was drawn.
        let scale = self.pointer_scale();
        let current = self.menu_chrome.take().filter(|chrome| {
            self.menu.as_ref() == Some(&chrome.menu)
                && chrome.theme == *theme
                && chrome.scale == scale
        });
        self.menu_chrome = match (current, &self.menu) {
            (Some(chrome), _) => Some(chrome),
            (None, Some(menu)) => Some(MenuChrome::paint(&mut self.text, theme, menu, scale)),
            (None, None) => None,
        };
    }

    /// Gives the keyboard to whoever should have it now.
    fn sync_keyboard_focus(&mut self, focused: Option<WindowId>) {
        if self.is_locked() {
            // The lock surface has the keyboard, or nothing does. Nothing
            // below is consulted.
            let focus = self.lock_focus().cloned();
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
        let focus = demanding.or(menu).or_else(|| {
            focused
                .and_then(|id| self.mapped(id))
                .map(|mapped| mapped.toplevel.wl_surface().clone())
        });
        let keyboard = self.keyboard.clone();
        if keyboard.current_focus() != focus {
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
            elements.extend(
                notice
                    .pixels
                    .element(renderer, at.to_f64(), view, unspecified),
            );
        }
        for (at, panel) in self.menu_chrome.iter().flat_map(|chrome| &chrome.panels) {
            elements.extend(panel.element(renderer, at.to_f64(), view, unspecified));
        }
        // From the top down: overlays, fullscreen windows, panels and
        // docks, ordinary windows, then whatever sits behind them.
        Self::layer_elements(renderer, output, view, Layer::Overlay, &mut elements);
        self.frame_elements(renderer, view, true, &mut elements);
        Self::layer_elements(renderer, output, view, Layer::Top, &mut elements);
        self.frame_elements(renderer, view, false, &mut elements);
        Self::layer_elements(renderer, output, view, Layer::Bottom, &mut elements);
        Self::layer_elements(renderer, output, view, Layer::Background, &mut elements);
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
                elements.extend(image.picture.element(renderer, at, view, Kind::Cursor));
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
        let unspecified = Kind::Unspecified;
        for shown in self
            .scene
            .iter()
            .rev()
            .filter(|shown| shown.fullscreen == fullscreen)
        {
            if let Some(chrome) = &shown.chrome {
                let bar = chrome.decoration.titlebar().origin;
                let at = Point::<i32, Logical>::from((bar.x, bar.y)).to_f64();
                elements.extend(chrome.titlebar.element(renderer, at, view, unspecified));
                let straight = corners::straight_borders(&chrome.decoration, chrome.radius);
                for (buffer, border) in chrome.borders.iter().zip(straight) {
                    let at = Point::<i32, Logical>::from((border.origin.x, border.origin.y));
                    let border = SolidColorRenderElement::from_buffer(
                        buffer,
                        view.pixel(at.to_f64()),
                        view.scale.factor(),
                        1.0,
                        unspecified,
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
                    elements.extend(corner.element(renderer, at, view, unspecified));
                }
            }
            Self::window_elements(renderer, view, shown, elements);
        }
    }

    /// Adds what a window itself shows: its pop-ups whole, and its own
    /// picture, which is cut to the frame if the frame's bottom corners are
    /// rounded.
    fn window_elements<R>(
        renderer: &mut R,
        view: View,
        shown: &Shown,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let scale = smithay::utils::Scale::from(view.scale.factor());
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
                    1.0,
                );
            elements.extend(surfaces.into_iter().map(Element::Surface));
            return;
        };
        // Pop-ups reach beyond the frame and are not cut to it.
        for (popup, offset) in PopupManager::popups_for_surface(&shown.surface) {
            let at = location + shown.window.geometry().loc + offset - popup.geometry().loc;
            let picture = render_elements_from_surface_tree(
                renderer,
                popup.wl_surface(),
                view.pixel(at.to_f64()),
                scale,
                1.0,
                Kind::Unspecified,
            );
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
                1.0,
                Kind::Unspecified,
            );
            let strips = whole.into_iter().filter_map(|picture| {
                Strip::of(picture, view.scale.factor(), area, strip, &chrome.strips)
            });
            elements.extend(strips.map(Element::Strip));
        }
    }

    /// Adds the layer-shell surfaces of one layer, topmost first.
    fn layer_elements<R>(
        renderer: &mut R,
        output: &Output,
        view: View,
        layer: Layer,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let layers = layer_map_for_output(output);
        for surface in layers.layers_on(layer).rev() {
            // A panel is placed within its own output.
            let at = layers.layer_geometry(surface).unwrap_or_default().loc + view.origin;
            let surfaces = surface.render_elements::<WaylandSurfaceRenderElement<R>>(
                renderer,
                view.pixel(at.to_f64()),
                smithay::utils::Scale::from(view.scale.factor()),
                1.0,
            );
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

/// The pixels of the open menu, kept until it or the theme changes.
#[derive(Debug)]
struct MenuChrome {
    menu: Menu<Action>,
    theme: Theme,
    scale: Scale,
    /// Each panel's position and pixels, from the root outwards.
    panels: Vec<(Point<i32, Logical>, Painted)>,
}

impl MenuChrome {
    fn paint(text: &mut Text, theme: &Theme, menu: &Menu<Action>, scale: Scale) -> Self {
        let panels = menu
            .panels()
            .iter()
            .map(|panel| {
                let canvas = paint::menu_panel(text, theme, menu, panel, scale);
                let origin = menu.panel_rect(panel).origin;
                (Point::from((origin.x, origin.y)), Painted::new(&canvas))
            })
            .collect();
        Self {
            menu: menu.clone(),
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

    /// The painting placed at `at` in the space, as an output sees it. If it
    /// cannot be uploaded to the renderer it is left out, which is better
    /// than failing the whole frame.
    fn element<R>(
        &self,
        renderer: &mut R,
        at: Point<f64, Logical>,
        view: View,
        kind: Kind,
    ) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            view.pixel(at).to_f64(),
            &self.pixels,
            None,
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
    pub(crate) fullscreen: bool,
    pub(crate) window: Window,
    /// The window's own surface.
    surface: WlSurface,
    /// The whole frame, decoration included.
    pub(crate) outer: Rect,
    /// Where the window's visible geometry starts.
    pub(crate) origin: Point<i32, Logical>,
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
    active: usize,
    focused: bool,
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
        text: &mut Text,
        theme: &Theme,
        scale: Scale,
        cached: Option<Self>,
        frame: &FrameInfo,
        decoration: Decoration,
    ) -> Self {
        let states = States::NONE.when(State::Focused, frame.focused);
        let outline = theme.style(Part::Window, states).border.top.color;
        let radius = corners::radius(&decoration, theme.style(Part::Window, states).radius);
        let appearance = Appearance {
            titles: frame.titles.clone(),
            active: frame.active,
            focused: frame.focused,
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
            let titlebar = paint::titlebar(
                text,
                theme,
                &decoration,
                &titles,
                frame.active,
                frame.focused,
                scale,
            );
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
    focused: bool,
    decoration: Option<Decoration>,
    tabs: Vec<WindowId>,
    titles: Vec<String>,
    active: usize,
}

impl FrameInfo {
    fn of(frame: &Frame<'_>, scale: Scale) -> Self {
        Self {
            shown: frame.window().id,
            scale,
            geometry: frame.geometry,
            client: frame.client(),
            maximized: frame.maximized,
            fullscreen: frame.fullscreen,
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
fn color(color: omorfos_config::Color) -> Color32F {
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
