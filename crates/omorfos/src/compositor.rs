//! The compositor's state and its main operations.

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use omorfos_config::{Action, Config, Debouncer, Store, Watcher};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Desktop, WindowId};
use smithay::backend::renderer::damage::{
    Error as RenderError, OutputDamageTracker, RenderOutputResult,
};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::{Color32F, ImportAll, Renderer, Texture};
use smithay::desktop::{PopupManager, Space, Window, space};
use smithay::input::keyboard::{KeyboardHandle, Keycode, XkbConfig};
use smithay::input::pointer::PointerHandle;
use smithay::input::{Seat, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle};
use smithay::utils::{Logical, Physical, Point, SERIAL_COUNTER, Size, Transform};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::{ToplevelStateSet, ToplevelSurface, XdgShellState};
use smithay::wayland::shm::ShmState;

use crate::bridge::{self, Bindings};

/// How long the configuration directory must be quiet before it is reloaded.
const RELOAD_DELAY: Duration = Duration::from_millis(150);

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
    _seat: Seat<Self>,
    pub(crate) keyboard: KeyboardHandle<Self>,
    pub(crate) pointer: PointerHandle<Self>,
    pub(crate) space: Space<Window>,
    pub(crate) popups: PopupManager,
    output: Output,
    output_transform: Transform,

    pub(crate) desktop: Desktop,
    /// Windows the desktop knows about.
    pub(crate) windows: Vec<Mapped>,
    /// Toplevels that exist but have not shown any content yet.
    pub(crate) unmapped: Vec<ToplevelSurface>,

    store: Store,
    watcher: Option<Watcher>,
    debouncer: Debouncer,
    pub(crate) bindings: Bindings,
    problems: Vec<String>,

    /// Keys whose press was consumed, so their release is consumed too.
    pub(crate) suppressed_keys: Vec<Keycode>,
    pub(crate) pointer_location: Point<f64, Logical>,
    pub(crate) hovered: Option<WindowId>,
    children: Vec<Child>,
    socket_name: Option<OsString>,
    running: bool,
}

impl Compositor {
    /// Creates a compositor with one output of `output_size` pixels, reading
    /// its configuration from `config_directory`.
    ///
    /// `output_transform` describes how the backend's framebuffer is
    /// oriented; a backend drawing through OpenGL into a window is flipped.
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
        config_directory: impl Into<PathBuf>,
        output_size: (i32, i32),
        output_transform: Transform,
    ) -> Result<Self, Box<dyn Error>> {
        let store = Store::open(config_directory);
        let watching =
            fs::create_dir_all(store.directory()).and_then(|()| Watcher::new(store.directory()));
        let mut startup_problems = Vec::new();
        let watcher = match watching {
            Ok(watcher) => Some(watcher),
            Err(error) => {
                startup_problems.push(format!(
                    "{}: changes will not be picked up automatically: {error}",
                    store.directory().display()
                ));
                None
            }
        };

        let surfaces = CompositorState::new::<Self>(&display);
        let xdg_shell = XdgShellState::new::<Self>(&display);
        let shm = ShmState::new::<Self>(&display, Vec::new());
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display);
        let decorations = XdgDecorationState::new::<Self>(&display);
        let mut seats = SeatState::new();
        let mut seat = seats.new_wl_seat(&display, "seat0");
        let pointer = seat.add_pointer();

        let output = Output::new(
            "omorfos-0".to_owned(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "omorfos".to_owned(),
                model: "virtual".to_owned(),
            },
        );
        let _global = output.create_global::<Self>(&display);
        let mut space = Space::default();
        space.map_output(&output, (0, 0));

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
                    _seat: seat,
                    keyboard,
                    pointer,
                    space,
                    popups: PopupManager::default(),
                    output,
                    output_transform,
                    desktop: Desktop::new(omorfos_core::Settings::default(), 1),
                    windows: Vec::new(),
                    unmapped: Vec::new(),
                    store,
                    watcher,
                    debouncer: Debouncer::new(RELOAD_DELAY),
                    bindings: Bindings::new([]),
                    problems: Vec::new(),
                    suppressed_keys: Vec::new(),
                    pointer_location: (0.0, 0.0).into(),
                    hovered: None,
                    children: Vec::new(),
                    socket_name: None,
                    running: true,
                };
                compositor.resize_output(output_size);
                compositor.apply_config(startup_problems);
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

    /// Changes the size of the output, in pixels.
    pub fn resize_output(&mut self, (width, height): (i32, i32)) {
        let mode = Mode {
            size: (width, height).into(),
            refresh: 60_000,
        };
        self.output.change_current_state(
            Some(mode),
            Some(self.output_transform),
            Some(Scale::Integer(1)),
            Some((0, 0).into()),
        );
        self.output.set_preferred(mode);
        let size = DomainSize::new(width.unsigned_abs(), height.unsigned_abs());
        self.desktop
            .set_outputs(vec![Rect::new(DomainPoint::new(0, 0), size)]);
    }

    /// The output, for a backend to create a damage tracker from.
    #[must_use]
    pub const fn output(&self) -> &Output {
        &self.output
    }

    /// The size of the output in pixels.
    #[must_use]
    pub fn output_size(&self) -> Size<i32, Physical> {
        self.output
            .current_mode()
            .map(|mode| mode.size)
            .unwrap_or_default()
    }

    // --- Lifecycle -----------------------------------------------------------

    /// Returns `false` once the session has been asked to end.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        self.running
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
        self.sync();
        self.space.refresh();
        self.popups.cleanup();
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

        let (bindings, unknown_keys) = bridge::bindings(&config.keys);
        self.bindings = bindings;
        problems.extend(unknown_keys);

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
            Action::Workspace(number) => {
                self.desktop
                    .switch_workspace(bridge::workspace_index(*number));
            }
            Action::NextWorkspace => self.desktop.next_workspace(),
            Action::PreviousWorkspace => self.desktop.previous_workspace(),
            Action::MoveToWorkspace(number) => {
                let workspace = bridge::workspace_index(*number);
                self.on_focused(|desktop, id| desktop.move_to_workspace(id, workspace));
            }
            // The root menu arrives with server-side decorations.
            Action::ShowMenu => {}
            Action::ReloadConfig => self.reload_config(),
            Action::Quit => self.running = false,
        }
    }

    /// Applies an operation to the focused window, if there is one.
    fn on_focused(&mut self, operation: impl FnOnce(&mut Desktop, WindowId)) {
        if let Some(id) = self.desktop.focused() {
            operation(&mut self.desktop, id);
        }
    }

    fn spawn(&mut self, command: &omorfos_config::Command) {
        let mut process = std::process::Command::new(command.program());
        process.args(command.arguments()).stdin(Stdio::null());
        if let Some(name) = &self.socket_name {
            process.env("WAYLAND_DISPLAY", name);
        }
        match process.spawn() {
            Ok(child) => self.children.push(child),
            Err(error) => {
                self.problems
                    .push(format!("cannot start `{}`: {error}", command.program()));
            }
        }
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
    /// are shown, where, in what order and state, and which has focus.
    fn sync(&mut self) {
        let focused = self.desktop.focused();
        let frames: Vec<(WindowId, Rect, bool, bool)> = self
            .desktop
            .frames()
            .map(|frame| {
                (
                    frame.window().id,
                    frame.client(),
                    frame.maximized,
                    frame.fullscreen,
                )
            })
            .collect();

        let mut shown = Vec::new();
        for Mapped {
            id,
            window,
            toplevel,
        } in &self.windows
        {
            let frame = frames.iter().enumerate().find(|(_, frame)| frame.0 == *id);
            let activated = Some(*id) == focused;
            toplevel.with_pending_state(|state| {
                set(&mut state.states, ToplevelState::Activated, activated);
                if let Some((_, (_, geometry, maximized, fullscreen))) = frame {
                    set(&mut state.states, ToplevelState::Maximized, *maximized);
                    set(&mut state.states, ToplevelState::Fullscreen, *fullscreen);
                    state.size = Some(logical_size(geometry.size));
                }
            });
            toplevel.send_pending_configure();
            match frame {
                Some((order, (_, geometry, ..))) => shown.push((order, window, geometry.origin)),
                None => self.space.unmap_elem(window),
            }
        }
        // Mapping raises an element, so mapping bottom to top sets the order.
        shown.sort_by_key(|(order, ..)| *order);
        for (_, window, origin) in shown {
            self.space
                .map_element(window.clone(), (origin.x, origin.y), false);
        }

        let focus = focused
            .and_then(|id| self.mapped(id))
            .map(|mapped| mapped.toplevel.wl_surface().clone());
        let keyboard = self.keyboard.clone();
        if keyboard.current_focus() != focus {
            keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
        }
    }

    // --- Drawing -------------------------------------------------------------

    /// Draws the output into `framebuffer`.
    ///
    /// `age` is how many frames old the framebuffer's contents are, or `0`
    /// if unknown, and lets the damage tracker redraw only what changed.
    ///
    /// # Errors
    ///
    /// Returns an error if the renderer fails.
    pub fn render<'tracker, R>(
        &self,
        renderer: &mut R,
        framebuffer: &mut R::Framebuffer<'_>,
        tracker: &'tracker mut OutputDamageTracker,
        age: usize,
    ) -> Result<RenderOutputResult<'tracker>, RenderError<R::Error>>
    where
        R: Renderer + ImportAll,
        R::TextureId: Clone + Texture + 'static,
    {
        let background = self.config().theme.colors.background;
        let channel = |value: u8| f32::from(value) / 255.0;
        let clear = Color32F::new(
            channel(background.red),
            channel(background.green),
            channel(background.blue),
            channel(background.alpha),
        );
        space::render_output::<_, WaylandSurfaceRenderElement<R>, _, _>(
            &self.output,
            renderer,
            framebuffer,
            1.0,
            age,
            [&self.space],
            &[],
            tracker,
            clear,
        )
    }

    /// Tells the windows on screen that a frame was shown, so they draw the
    /// next one. `elapsed` is the time since the compositor started.
    pub fn send_frames(&self, elapsed: Duration) {
        for window in self.space.elements() {
            window.send_frame(&self.output, elapsed, Some(Duration::ZERO), |_, _| {
                Some(self.output.clone())
            });
        }
    }
}

fn set(states: &mut ToplevelStateSet, state: ToplevelState, on: bool) {
    if on {
        states.set(state);
    } else {
        states.unset(state);
    }
}

fn logical_size(size: DomainSize) -> Size<i32, Logical> {
    let clamp = |length: u32| i32::try_from(length).unwrap_or(i32::MAX);
    (clamp(size.width), clamp(size.height)).into()
}
