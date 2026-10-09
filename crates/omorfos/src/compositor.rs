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

use omorfos_config::{Action, Config, Debouncer, Store, Theme, Watcher};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Decoration, Desktop, Frame, Menu, MenuMetrics, Screen, WindowId};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::{
    Error as RenderError, OutputDamageTracker, RenderOutputResult,
};
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::{Color32F, ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::{PopupManager, Space, Window, layer_map_for_output};
use smithay::input::keyboard::{KeyboardHandle, Keycode, XkbConfig};
use smithay::input::pointer::PointerHandle;
use smithay::input::{Seat, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DisplayHandle, Resource};
use smithay::utils::{Logical, Physical, Point, Rectangle, SERIAL_COUNTER, Size, Transform};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::wlr_layer::{KeyboardInteractivity, Layer, WlrLayerShellState};
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shell::xdg::{
    PopupSurface, ToplevelStateSet, ToplevelSurface, XdgShellState,
};
use smithay::wayland::shm::ShmState;

use crate::bridge::{self, Bindings};
use crate::input::Drag;
use crate::ipc::Server;
use crate::paint::{self, Text};

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
    /// The output as the desktop was last told about it.
    screen: Screen,
    pub(crate) layer_shell: WlrLayerShellState,
    pub(crate) data_device: DataDeviceState,

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
    pub(crate) hovered: Option<WindowId>,
    children: Vec<Child>,
    pub(crate) ipc: Option<Server>,
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
        let layer_shell = WlrLayerShellState::new::<Self>(&display);
        let data_device = DataDeviceState::new::<Self>(&display);
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
                    screen: Screen::default(),
                    layer_shell,
                    data_device,
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
                    hovered: None,
                    children: Vec::new(),
                    ipc: None,
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
        layer_map_for_output(&self.output).arrange();
        self.update_screen();
    }

    /// Tells the desktop how big the output is and how much of it panels
    /// have left for windows, if either changed.
    fn update_screen(&mut self) {
        let rect = |rectangle: Rectangle<i32, Logical>| {
            Rect::new(
                DomainPoint::new(rectangle.loc.x, rectangle.loc.y),
                DomainSize::new(
                    rectangle.size.w.unsigned_abs(),
                    rectangle.size.h.unsigned_abs(),
                ),
            )
        };
        let size = self.output_size().to_logical(1);
        let screen = Screen {
            area: rect(Rectangle::from_size(size)),
            usable: rect(layer_map_for_output(&self.output).non_exclusive_zone()),
        };
        if screen != self.screen {
            self.screen = screen;
            self.desktop.set_outputs([screen]);
        }
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
        self.serve_ipc();
        self.sync();
        self.space.refresh();
        self.popups.cleanup();
        self.publish_ipc();
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
            Action::ShowMenu => self.open_menu(),
            Action::ReloadConfig => self.reload_config(),
            Action::Quit => self.running = false,
        }
    }

    /// Opens the root menu at the pointer.
    pub(crate) fn open_menu(&mut self) {
        let config = Arc::clone(self.store.current());
        let size = self.output_size();
        let area = Rect::new(
            DomainPoint::new(0, 0),
            DomainSize::new(size.w.unsigned_abs(), size.h.unsigned_abs()),
        );
        let metrics = MenuMetrics {
            width: MENU_WIDTH,
            row: config.theme.titlebar.height.get(),
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
        let mut process = std::process::Command::new(command.program());
        process.args(command.arguments()).stdin(Stdio::null());
        if let Some(name) = &self.socket_name {
            process.env("WAYLAND_DISPLAY", name);
        }
        if let Some(path) = self.ipc_path() {
            process.env("OMORFOS_SOCKET", path);
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
    /// are shown, where, in what order and state, and which has focus. Also
    /// repaints the decoration of any frame whose appearance changed.
    fn sync(&mut self) {
        layer_map_for_output(&self.output).cleanup();
        self.update_screen();
        let focused = self.desktop.focused();
        let frames: Vec<FrameInfo> = self
            .desktop
            .frames()
            .map(|frame| FrameInfo::of(&frame))
            .collect();
        let config = Arc::clone(self.store.current());
        let mut previous = std::mem::take(&mut self.scene);
        let text = &mut self.text;

        let mut scene = Vec::new();
        for Mapped {
            id,
            window,
            toplevel,
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
                        Chrome::paint(text, &config.theme, cached, frame, decoration)
                    });
                    let origin = (frame.client.origin.x, frame.client.origin.y).into();
                    scene.push((
                        order,
                        Shown {
                            id: *id,
                            fullscreen: frame.fullscreen,
                            window: window.clone(),
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
        let output_width = self.screen.area.size.width;
        let current = self.notice.take().filter(|notice| {
            wanted
                && notice.problems == self.problems
                && notice.theme == *theme
                && notice.output_width == output_width
        });
        self.notice = match (current, wanted) {
            (Some(notice), _) => Some(notice),
            (None, true) => Some(Notice::paint(
                &mut self.text,
                theme,
                &self.problems,
                output_width,
            )),
            (None, false) => None,
        };

        // Repaint the menu if it, or the theme, changed since it was drawn.
        let current = self
            .menu_chrome
            .take()
            .filter(|chrome| self.menu.as_ref() == Some(&chrome.menu) && chrome.theme == *theme);
        self.menu_chrome = match (current, &self.menu) {
            (Some(chrome), _) => Some(chrome),
            (None, Some(menu)) => Some(MenuChrome::paint(&mut self.text, theme, menu)),
            (None, None) => None,
        };
    }

    /// Gives the keyboard to whoever should have it now.
    fn sync_keyboard_focus(&mut self, focused: Option<WindowId>) {
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
        let demanding = layer_map_for_output(&self.output)
            .layers()
            .rev()
            .find(|layer| {
                layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                    && matches!(layer.layer(), Layer::Top | Layer::Overlay)
            })
            .map(|layer| layer.wl_surface().clone());
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
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let unspecified = Kind::Unspecified;
        // Elements are listed from the top of the stack down.
        let mut elements: Vec<Element<R>> = Vec::new();
        if let Some(notice) = &self.notice {
            let at = (
                f64::from(notice.area.origin.x),
                f64::from(notice.area.origin.y),
            );
            let notice = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                at,
                &notice.pixels,
                None,
                None,
                None,
                unspecified,
            );
            elements.extend(notice.ok().map(Element::Memory));
        }
        for (at, panel) in self.menu_chrome.iter().flat_map(|chrome| &chrome.panels) {
            let panel = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                at.to_f64().to_physical(1.0),
                panel,
                None,
                None,
                None,
                unspecified,
            );
            elements.extend(panel.ok().map(Element::Memory));
        }
        // From the top down: overlays, fullscreen windows, panels and
        // docks, ordinary windows, then whatever sits behind them.
        self.layer_elements(renderer, Layer::Overlay, &mut elements);
        self.frame_elements(renderer, true, &mut elements);
        self.layer_elements(renderer, Layer::Top, &mut elements);
        self.frame_elements(renderer, false, &mut elements);
        self.layer_elements(renderer, Layer::Bottom, &mut elements);
        self.layer_elements(renderer, Layer::Background, &mut elements);
        tracker.render_output(
            renderer,
            framebuffer,
            age,
            &elements,
            color(self.config().theme.colors.background),
        )
    }

    /// Adds the frames that are, or are not, fullscreen, topmost first.
    fn frame_elements<R>(&self, renderer: &mut R, fullscreen: bool, elements: &mut Vec<Element<R>>)
    where
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
                let at = (f64::from(bar.x), f64::from(bar.y));
                // A texture that cannot be uploaded is skipped, not fatal.
                let titlebar = MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    at,
                    &chrome.titlebar,
                    None,
                    None,
                    None,
                    unspecified,
                );
                elements.extend(titlebar.ok().map(Element::Memory));
                for (buffer, border) in chrome.borders.iter().zip(chrome.decoration.borders()) {
                    let at = (border.origin.x, border.origin.y);
                    let border =
                        SolidColorRenderElement::from_buffer(buffer, at, 1.0, 1.0, unspecified);
                    elements.push(Element::Solid(border));
                }
            }
            // A window's surface may start above and left of its visible
            // geometry, where it draws its own shadows.
            let location = (shown.origin - shown.window.geometry().loc).to_physical(1);
            let surfaces = shown
                .window
                .render_elements::<WaylandSurfaceRenderElement<R>>(
                    renderer,
                    location,
                    smithay::utils::Scale::from(1.0),
                    1.0,
                );
            elements.extend(surfaces.into_iter().map(Element::Surface));
        }
    }

    /// Adds the layer-shell surfaces of one layer, topmost first.
    fn layer_elements<R>(&self, renderer: &mut R, layer: Layer, elements: &mut Vec<Element<R>>)
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let layers = layer_map_for_output(&self.output);
        for surface in layers.layers_on(layer).rev() {
            let at = layers
                .layer_geometry(surface)
                .unwrap_or_default()
                .loc
                .to_physical(1);
            let surfaces = surface.render_elements::<WaylandSurfaceRenderElement<R>>(
                renderer,
                at,
                smithay::utils::Scale::from(1.0),
                1.0,
            );
            elements.extend(surfaces.into_iter().map(Element::Surface));
        }
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

render_elements! {
    /// Everything the compositor draws: client surfaces and its own
    /// decorations.
    Element<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
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
    output_width: u32,
    /// Where on the output the notice is: its top-right corner.
    pub(crate) area: Rect,
    pixels: MemoryRenderBuffer,
}

impl Notice {
    fn paint(text: &mut Text, theme: &Theme, problems: &[String], output_width: u32) -> Self {
        let width = NOTICE_WIDTH.min(output_width.saturating_sub(NOTICE_MARGIN * 2));
        let canvas = paint::notice(text, theme, problems, width);
        let left = output_width
            .saturating_sub(width + NOTICE_MARGIN)
            .cast_signed();
        let area = Rect::new(
            DomainPoint::new(left, NOTICE_MARGIN.cast_signed()),
            DomainSize::new(canvas.width(), canvas.height()),
        );
        Self {
            problems: problems.to_vec(),
            theme: theme.clone(),
            output_width,
            area,
            pixels: buffer(&canvas),
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
    /// Each panel's position and pixels, from the root outwards.
    panels: Vec<(Point<i32, Logical>, MemoryRenderBuffer)>,
}

impl MenuChrome {
    fn paint(text: &mut Text, theme: &Theme, menu: &Menu<Action>) -> Self {
        let panels = menu
            .panels()
            .iter()
            .map(|panel| {
                let canvas = paint::menu_panel(text, theme, menu, panel);
                let origin = menu.panel_rect(panel).origin;
                (Point::from((origin.x, origin.y)), buffer(&canvas))
            })
            .collect();
        Self {
            menu: menu.clone(),
            theme: theme.clone(),
            panels,
        }
    }
}

/// Uploads a canvas as a texture source.
fn buffer(canvas: &paint::Canvas) -> MemoryRenderBuffer {
    let size = (canvas.width().cast_signed(), canvas.height().cast_signed());
    MemoryRenderBuffer::from_slice(
        canvas.bytes(),
        Fourcc::Argb8888,
        size,
        1,
        Transform::Normal,
        None,
    )
}

/// A window on screen: where it is and the decoration drawn around it.
#[derive(Debug)]
pub(crate) struct Shown {
    pub(crate) id: WindowId,
    pub(crate) fullscreen: bool,
    pub(crate) window: Window,
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
    titlebar: MemoryRenderBuffer,
    borders: [SolidColorBuffer; 3],
}

/// Everything a titlebar's pixels depend on. Its position is not part of
/// this, so moving a frame does not repaint it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Appearance {
    titles: Vec<String>,
    active: usize,
    focused: bool,
    size: DomainSize,
    theme: Theme,
}

impl Chrome {
    /// Brings a frame's decoration up to date, reusing `cached` pixels when
    /// nothing they depend on has changed.
    fn paint(
        text: &mut Text,
        theme: &Theme,
        cached: Option<Self>,
        frame: &FrameInfo,
        decoration: Decoration,
    ) -> Self {
        let appearance = Appearance {
            titles: frame.titles.clone(),
            active: frame.active,
            focused: frame.focused,
            size: decoration.titlebar().size,
            theme: theme.clone(),
        };
        let (cached_titlebar, mut borders) = cached
            .map(|chrome| {
                (
                    (chrome.appearance == appearance).then_some(chrome.titlebar),
                    chrome.borders,
                )
            })
            .unwrap_or_default();
        let titlebar = cached_titlebar.unwrap_or_else(|| {
            let titles: Vec<&str> = appearance.titles.iter().map(String::as_str).collect();
            buffer(&paint::titlebar(
                text,
                theme,
                &decoration,
                &titles,
                frame.active,
                frame.focused,
            ))
        });
        let palette = if frame.focused {
            theme.colors.focused
        } else {
            theme.colors.unfocused
        };
        for (buffer, border) in borders.iter_mut().zip(decoration.borders()) {
            // Updating with an unchanged size and colour is not damage.
            buffer.update(logical_size(border.size), color(palette.border));
        }
        Self {
            tabs: frame.tabs.clone(),
            appearance,
            decoration,
            titlebar,
            borders,
        }
    }
}

/// What `sync` needs to know about a frame, copied out of the desktop.
struct FrameInfo {
    shown: WindowId,
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
    fn of(frame: &Frame<'_>) -> Self {
        Self {
            shown: frame.window().id,
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
