//! The compositor's state and its main operations.

use std::borrow::Cow;
use std::cell::RefCell;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use omorfos_config::{
    Action, Config, Debouncer, Half, Part, States, Store, Style, Theme, Watcher, WorkspaceNumber,
};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{
    Button, Decoration, Desktop, Frame, Home, Item, Menu, MenuMetrics, Place, Screen, Standing,
    WindowId, WorkspaceId,
};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use smithay::backend::allocator::Format;
use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::Bind;
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
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::utils::CropRenderElement;
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::backend::renderer::{
    Color32F, ExportMem, ImportAll, ImportMem, Offscreen, Renderer, RendererSuper, Texture,
};
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
use smithay::wayland::dmabuf::{DmabufFeedbackBuilder, DmabufGlobal, DmabufState};
use smithay::wayland::foreign_toplevel_list::{ForeignToplevelHandle, ForeignToplevelListState};
use smithay::wayland::fractional_scale::{FractionalScaleManagerState, with_fractional_scale};
use smithay::wayland::idle_inhibit::IdleInhibitManagerState;
use smithay::wayland::idle_notify::IdleNotifierState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::security_context::SecurityContextState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::ext_data_control::DataControlState;
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

use crate::Scale;
use crate::asked::Asked;
use crate::bridge::{self, Bindings};
use crate::capture::{self, Capture, Due, Gpu, Pass, Subject};
use crate::corners::{self, Names, Strip};
use crate::cursor::{self, Themes};
use crate::ghost::{Ghost, Noted, Noting};
use crate::gpu;
use crate::icons::WindowIcons;
use crate::input::Drag;
use crate::input::Focused;
use crate::ipc::Server;
use crate::lock::{Lock, Presence};
use crate::outputs::Head;
use crate::paint::{self, Motion, Text};
use crate::palette::{self, Palette};
use crate::recording::Recorded;
use crate::restarts::Restarts;
use crate::reveal::{Placing, Reveal};
use crate::roles::{Refused, Roles};
use crate::switcher::Switch;
use crate::tiles::Lines;
use crate::typing::Typing;
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

/// What drawing a screen came to.
#[derive(Debug)]
pub struct Drawn {
    /// The parts of the screen that changed, in its pixels, or nothing if
    /// none did and nothing was drawn.
    pub damage: Option<Vec<Rectangle<i32, Physical>>>,
    /// When the drawing is finished.
    pub sync: SyncPoint,
}

impl Drawn {
    /// What the last drawing of a screen came to, with what the drawings
    /// before it in the same turn changed.
    fn of(last: RenderOutputResult<'_>, mut before: Vec<Rectangle<i32, Physical>>) -> Self {
        let damage = if before.is_empty() {
            last.damage.cloned()
        } else {
            before.extend(last.damage.into_iter().flatten().copied());
            Some(before)
        };
        Self {
            damage,
            sync: last.sync,
        }
    }
}

/// What of a screen is drawn besides the session itself.
#[derive(Debug, Clone, Copy)]
struct Extras {
    /// The pointer.
    pointer: bool,
    /// The mark that says the screen is being captured, which is also to
    /// say that this is what the user sees and not a picture for a
    /// program: in a picture, windows that are private are left blank.
    mark: bool,
}

/// What a locked screen is cleared to.
const BLACK: Color32F = Color32F::BLACK;

/// Per-client data the compositor attaches to every connection.
#[derive(Debug, Default)]
pub struct ClientState {
    pub(crate) compositor: CompositorClientState,
    /// What the client may do that an ordinary program may not.
    pub(crate) roles: Roles,
    /// Whether the client came in through a sandbox, which told the
    /// compositor so itself.
    pub(crate) sandboxed: bool,
}

impl wayland_server::backend::ClientData for ClientState {}

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
    /// Buffers in the memory of the graphics card, which programs hand over
    /// once the backend says which card and what it can draw.
    pub(crate) gpu_buffers: GpuBuffers,
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
    /// Pictures of the screen being taken by programs that may.
    pub(crate) capture: Capture,
    _fractional_scale: FractionalScaleManagerState,
    _viewporter: ViewporterState,
    _single_pixels: SinglePixelBufferState,
    _sandboxes: SecurityContextState,
    /// The outputs as the desktop was last told about them.
    pub(crate) screens: Vec<Screen>,
    pub(crate) layer_shell: WlrLayerShellState,
    pub(crate) data_device: DataDeviceState,
    /// The clipboard as given to a program that may read and set it at
    /// any time.
    pub(crate) clipboards: DataControlState,
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
    pub(crate) text: Text,

    pub(crate) store: Store,
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
    /// The pictures of the pointer, kept until they would differ.
    pointer_images: PointerImages,
    themes: Themes,
    _cursor_shapes: CursorShapeManagerState,
    /// The virtual terminal the user asked to switch to, until the backend
    /// collects it.
    pub(crate) terminal_wanted: Option<i32>,
    /// What the pointer is over, and what it is doing with titlebars.
    pub(crate) hover: Hover,
    /// The switcher on `Alt+Tab`, and the order of use it goes by.
    pub(crate) switch: Switch,
    /// The lock on the session, while it is locked.
    pub(crate) lock: Option<Lock>,
    pub(crate) lock_manager: SessionLockManagerState,
    /// Tells those who asked when the user has been away for a while.
    pub(crate) idle: IdleNotifierState<Self>,
    _idle_inhibit: IdleInhibitManagerState,
    /// The input method, and what is known of it.
    pub(crate) typing: Typing,
    /// What dims each screen while a prompt asks for a secret.
    pub(crate) seal: Vec<(Output, SolidColorBuffer)>,
    /// Surfaces that ask for the session not to go idle while they are
    /// shown, such as a film being played.
    pub(crate) inhibitors: Vec<WlSurface>,
    /// Whether the user is there, and so whether the screens are on.
    pub(crate) presence: Presence,
    /// Programs written for X11, and the X server they run through.
    pub(crate) x11: X11,
    pub(crate) children: Vec<Child>,
    /// What says windows that are to open were asked for.
    pub(crate) asked: Asked,
    /// Whether the programs to start with the session have been, and
    /// what starts them in a scope of their own.
    autostarted: Autostarted,
    /// The programs that come with omorfos and run for the whole session,
    /// once a backend has said where each is: the panel first.
    components: Vec<Component>,
    /// The launcher, once a backend has said where it is, and the one that
    /// is open, by the number of
    /// its process.
    launcher: (Option<PathBuf>, Option<u32>),
    /// What programs have been refused for want of a role.
    pub(crate) refused: Refused,
    /// What is changing colour or coming in, and the time it goes by.
    animation: Animation,
    pub(crate) ipc: Option<Server>,
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
        let lock_manager = SessionLockManagerState::new::<Self, _>(&display, Self::outside);
        let idle = IdleNotifierState::new(&display, event_loop.clone());
        let xdg_shell = XdgShellState::new::<Self>(&display);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&display);
        let layer_shell = WlrLayerShellState::new_with_filter::<Self, _>(&display, Self::outside);
        let data_device = DataDeviceState::new::<Self>(&display);
        let primary_selection = PrimarySelectionState::new::<Self>(&display);
        let foreign_toplevels =
            ForeignToplevelListState::new_with_filter::<Self>(&display, Self::lists);
        let mut seats = SeatState::new();
        let mut seat = seats.new_wl_seat(&display, "seat0");
        let pointer = seat.add_pointer();

        seat.add_keyboard(XkbConfig::default(), 400, 30)
            .map_err(Box::from)
            .map(|keyboard| {
                let mut compositor = Self {
                    _single_pixels: SinglePixelBufferState::new::<Self>(&display),
                    _sandboxes: SecurityContextState::new::<Self, _>(&display, Self::outside),
                    _viewporter: ViewporterState::new::<Self>(&display),
                    _cursor_shapes: CursorShapeManagerState::new::<Self>(&display),
                    capture: Capture::offer(&display),
                    asked: Asked::new(&display),
                    switch: Switch::offer(&display),
                    typing: Typing::offer(&display),
                    seal: Vec::new(),
                    _fractional_scale: FractionalScaleManagerState::new::<Self>(&display),
                    _idle_inhibit: IdleInhibitManagerState::new::<Self>(&display),
                    clipboards: DataControlState::new::<Self, _>(&display, None, Self::copies),
                    _decorations: XdgDecorationState::new::<Self>(&display),
                    x11: X11::new(&display),
                    shm: ShmState::new::<Self>(&display, Vec::new()),
                    gpu_buffers: GpuBuffers::default(),
                    display,
                    surfaces,
                    xdg_shell,
                    seats,
                    _output_manager_state: output_manager_state,
                    seat,
                    keyboard,
                    pointer,
                    space: Space::default(),
                    popups: PopupManager::default(),
                    heads: Vec::new(),
                    screens: Vec::new(),
                    layer_shell,
                    data_device,
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
                    pointer_images: PointerImages::default(),
                    themes: Themes::default(),
                    terminal_wanted: None,
                    hover: Hover::default(),
                    lock: None,
                    lock_manager,
                    idle,
                    inhibitors: Vec::new(),
                    presence: Presence::default(),
                    children: Vec::new(),
                    autostarted: Autostarted::default(),
                    components: Vec::new(),
                    launcher: (None, None),
                    refused: Refused::default(),
                    animation: Animation::new(),
                    ipc: None,
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

    /// Accepts a client connected over `stream` that may do what `roles`
    /// allow: nothing beyond the ordinary, for a program that found the
    /// socket by itself. Roles are for a stream the caller made and handed
    /// to a program it started, which is what makes the program known.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be registered.
    pub fn insert_client(&mut self, stream: UnixStream, roles: Roles) -> io::Result<Client> {
        let state = ClientState {
            roles,
            ..ClientState::default()
        };
        self.display.insert_client(stream, Arc::new(state))
    }

    /// Accepts a client that connected through a sandbox. It has no role
    /// and is not shown what only a role allows.
    pub(crate) fn insert_sandboxed(&mut self, stream: UnixStream) -> io::Result<Client> {
        let state = ClientState {
            sandboxed: true,
            ..ClientState::default()
        };
        self.display.insert_client(stream, Arc::new(state))
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

    /// Lets programs hand over pictures in the memory of the graphics card
    /// whose device number is `device`, in the formats and layouts it can
    /// draw, `formats`, which it then draws without copying them. Until
    /// this is called a program can hand over only shared memory.
    ///
    /// # Errors
    ///
    /// Returns an error if the table of formats a program is given cannot
    /// be made.
    pub fn offer_gpu_buffers(
        &mut self,
        device: u64,
        formats: impl IntoIterator<Item = Format>,
    ) -> io::Result<()> {
        let formats: Vec<Format> = formats.into_iter().collect();
        let gpu = Gpu::of(device, &formats);
        let feedback = DmabufFeedbackBuilder::new(device, formats).build();
        feedback.map(|feedback| {
            // Pictures of the screen can be taken in the card's memory too.
            self.capture.gpu = Some(gpu);
            let state = &mut self.gpu_buffers.state;
            let global =
                state.create_global_with_default_feedback::<Self>(&self.display, &feedback);
            self.gpu_buffers.global = Some(global);
        })
    }

    /// Says where cursor themes are installed. Until this is called there
    /// are none, and the pointer is a plain arrow.
    pub fn set_cursor_themes(&mut self, themes: Themes) {
        self.themes = themes;
        self.pointer_images = PointerImages::default();
    }

    /// Says where programs install what they share, desktop entries and
    /// icons among it, the user's own first (see
    /// [`omorfos_icons::data_directories`]). Until this is called windows
    /// have only the icons their programs give as pixels.
    pub fn set_data_directories(&mut self, data: Vec<PathBuf>) {
        self.switch.icons = WindowIcons::new(data);
    }

    /// Works out what the pointer looks like now and readies its picture.
    fn update_pointer(&mut self) {
        if !self.draws_pointer {
            return;
        }
        self.shown = self.cursor_wanted();
        let theme = &self.store.current().general.cursor;
        let scale = self.pointer_scale();
        let images = &mut self.pointer_images;
        let loaded = &mut images.loaded;
        let mut ready = |kept: Option<PointerImage>, icon| {
            let current = kept.filter(|image| {
                image.icon == icon && image.scale == scale && image.theme == *theme
            });
            current.unwrap_or_else(|| {
                *loaded += 1;
                PointerImage::load(&self.themes, theme, icon, scale, *loaded)
            })
        };
        match self.shown {
            Look::Icon(icon) => images.shown = Some(ready(images.shown.take(), icon)),
            // A pointer an application draws is given to programs that take
            // pictures of it as the ordinary one until it has been drawn for
            // them.
            Look::Surface(_) => {
                let kept = images.stand_in.take();
                images.stand_in = Some(ready(kept, CursorIcon::Default));
            }
            Look::Hidden => {}
        }
    }

    /// How long the session has been drawn for, which is the time a
    /// picture is said to have been taken at.
    pub(crate) const fn clock(&self) -> Duration {
        self.animation.clock
    }

    /// The picture of the pointer as programs that take pictures of it are
    /// given it, with what they are kept in: what is shown, or one an
    /// application draws as it was drawn for them, or until it has been,
    /// the ordinary pointer. Nothing, while it is hidden. And whether it
    /// is to be given yet, which it is not while one an application draws
    /// waits to be drawn again.
    pub(crate) fn pointer_picture(&mut self) -> (Option<&Given>, bool, &mut Capture) {
        let images = &self.pointer_images;
        let picture = match &self.shown {
            Look::Icon(_) => images.shown.as_ref().map(|image| &image.given),
            Look::Surface(surface) => {
                let drawn = images.drawn.as_ref();
                let drawn = drawn.filter(|drawn| drawn.surface == *surface);
                let drawn = drawn.map(|drawn| &drawn.given);
                drawn.or_else(|| images.stand_in.as_ref().map(|image| &image.given))
            }
            Look::Hidden => None,
        };
        let waiting = self.pointer_to_draw().is_some();
        (picture, !waiting, &mut self.capture)
    }

    /// The pointer an application draws, if a program following the pointer
    /// is to be given a picture of it and it has not been drawn as it is
    /// now, at the scale of the screen it is on.
    fn pointer_to_draw(&self) -> Option<&WlSurface> {
        let Look::Surface(surface) = &self.shown else {
            return None;
        };
        let scale = self.pointer_scale();
        let drawn = self.pointer_images.drawn.as_ref();
        let current = drawn.is_some_and(|drawn| drawn.surface == *surface && drawn.scale == scale)
            && !self.pointer_images.stale;
        (!current && self.capture.pointer_wanted()).then_some(surface)
    }

    /// Draws the pointer an application draws by itself, as large as any
    /// pointer is drawn, for the programs following the pointer, and gives
    /// it to those waiting for it.
    fn draw_pointer<R, P>(&mut self, renderer: &mut R, surface: &WlSurface, scale: Scale)
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let whole = capture::POINTER.cast_signed();
        let size = Size::<i32, Physical>::from((whole, whole));
        let factor = scale.factor();
        let elements: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
            renderer,
            surface,
            Point::<i32, Physical>::default(),
            factor,
            1.0,
            Kind::Cursor,
        );
        let elements: Vec<Element<R>> = elements.into_iter().map(Element::Surface).collect();
        let mut tracker = OutputDamageTracker::new(size, factor, Transform::Normal);
        let made: Option<P> = renderer
            .create_buffer(Fourcc::Abgr8888, Size::from((whole, whole)))
            .ok();
        let read = made.and_then(|mut picture| {
            let target = renderer.bind(&mut picture).ok();
            target.and_then(|mut target| {
                let clear = Color32F::TRANSPARENT;
                let drawn = tracker.render_output(renderer, &mut target, 0, &elements, clear);
                let all = Rectangle::from_size(Size::from((whole, whole)));
                let read = drawn
                    .ok()
                    .map(|_| renderer.copy_framebuffer(&target, all, Fourcc::Abgr8888));
                read.and_then(Result::ok)
            })
        });
        let pixels = read
            .as_ref()
            .and_then(|read| renderer.map_texture(read).ok());
        let mut bytes = Vec::new();
        capture::turn(pixels.unwrap_or_default(), &mut bytes, true);
        let hotspot = cursor_hotspot(surface)
            .to_f64()
            .to_physical(factor)
            .to_i32_round::<i32>();
        let tip = (
            hotspot.x.max(0).unsigned_abs(),
            hotspot.y.max(0).unsigned_abs(),
        );
        let images = &mut self.pointer_images;
        images.loaded += 1;
        let given = Given {
            bytes,
            pixels: (capture::POINTER, capture::POINTER),
            tip,
            serial: images.loaded,
        };
        images.drawn = Some(DrawnPointer {
            surface: surface.clone(),
            scale,
            given,
        });
        images.stale = false;
        self.settle_following();
    }

    /// Notes that the pointer an application draws may look different,
    /// after `surface`, part of it or not, has changed.
    pub(crate) fn pointer_changed(&mut self, surface: &WlSurface) {
        let drawn = self.pointer_images.drawn.as_ref();
        let ours = drawn.is_some_and(|drawn| drawn.surface == *surface);
        self.pointer_images.stale |= ours;
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

    /// Says what is wrong with how the screen called `name` is driven, or
    /// that nothing is: as a mode that it does not offer was asked for. It
    /// is among the problems until the backend says otherwise, the
    /// configuration being read again or not.
    pub fn screen_wrong(&mut self, name: &str, wrong: Option<String>) {
        let screens = &mut self.refused.screens;
        let at = screens.iter().position(|(screen, _)| screen == name);
        let was = at.map(|at| screens.remove(at));
        if let Some((_, said)) = was {
            self.problems.retain(|problem| *problem != said);
        }
        if let Some(said) = wrong {
            self.problems.push(said.clone());
            self.refused.screens.push((name.to_owned(), said));
        }
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
            self.ipc.as_ref().and_then(|server| server.retry),
            self.x11.deadline(),
            self.hover.reveal.deadline(),
            self.hover.palette.deadline(),
            self.capture.deadline(),
            self.capture.recording.deadline(),
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
        for refused in self.refused.take() {
            self.happened(&refused, now);
        }
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
        self.settle_recording(now);
        self.settle_capture(now);
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
        self.forget_ended();
        self.revive(&failed, now);
        self.serve_ipc();
        self.manage_x11(now);
        self.reveal(now);
        self.offer_arrangements(now);
        self.sync();
        self.note_use();
        self.rest_or_wake(now);
        // With the screens off a lock is shown without drawing, and is
        // said to be before the state is.
        self.confirm_lock();
        self.update_seal();
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
        problems.extend(crate::outputs::misplaced(&config.general));

        self.desktop.set_settings(bridge::settings(&config));
        self.desktop.set_rules(bridge::rules(&config.rules));
        self.desktop
            .set_workspace_count(config.general.workspaces.names().len());
        self.apply_scale();
        self.capture.repaint();

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

        let screens = self.refused.screens.iter();
        problems.extend(screens.map(|(_, said)| said.clone()));
        self.problems = problems;
    }

    // --- Actions -------------------------------------------------------------

    /// Carries out an action from a key binding or menu entry.
    pub fn perform(&mut self, action: &Action) {
        // A menu that is open was opened on what was there then: a
        // window, or the desktop being shown. Whatever is done next may
        // change that, so the menu goes, as it does when one of its own
        // entries is chosen.
        self.menu = None;
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
            Action::SwitchWindows => self.switch_windows(false),
            Action::SwitchWindowsBack => self.switch_windows(true),
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
            Action::JoinWorkspace(id) => {
                let workspace = WorkspaceId::from_raw(*id);
                self.on_focused(|desktop, window| desktop.join(window, workspace));
            }
            Action::ShowMenu => self.open_menu(),
            Action::ShowLauncher => self.open_launcher(),
            Action::Lock => self.start_locker(),
            Action::NextLayout => {
                let keyboard = self.keyboard.clone();
                keyboard.with_xkb_state(self, |mut xkb| xkb.cycle_next_layout());
            }
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
        let places = || {
            let places = self.desktop.workspaces();
            places.filter(|place| place.output == here)
        };
        let desktops = places().filter(|place| place.desktop.is_some());
        let send = (1..).zip(desktops).map(|(number, place)| Item::Entry {
            label: self.workspace_name(&place),
            action: Action::MoveToWorkspace(WorkspaceNumber::clamped(number)),
        });
        // A workspace made for windows takes another where it has room,
        // unless it is where this window is already.
        let own = self.desktop.focused();
        let joinable =
            |place: &Place| place.room && !own.is_some_and(|id| place.tiles.contains(&id));
        let join = places().filter(joinable).map(|place| Item::Entry {
            label: format!("Beside {}", self.workspace_name(&place)),
            action: Action::JoinWorkspace(place.id.get()),
        });
        let send = send.chain(join).chain([
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

    /// Has something painted with the compositor's text and theme.
    pub(crate) fn paint_with<T>(&mut self, paint: impl FnOnce(&mut Text, &Theme) -> T) -> T {
        let config = Arc::clone(self.store.current());
        paint(&mut self.text, &config.theme)
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

    fn spawn(&mut self, started: &omorfos_config::Started) {
        let roles = Roles::named(&started.may);
        let command = &started.run;
        self.start(command.program().as_ref(), command.arguments(), roles);
    }

    /// Starts the program the configuration names to lock the session,
    /// which is let lock by having been started for it. With none named
    /// there is nothing to lock with, which the user is told.
    fn start_locker(&mut self) {
        let config = Arc::clone(self.store.current());
        match &config.general.locker {
            Some(locker) => {
                self.start(locker.program().as_ref(), locker.arguments(), Roles::LOCK);
            }
            None => self.refused.say(NO_LOCKER),
        }
    }

    /// Says where the launcher that comes with omorfos is: a name to look
    /// up in `PATH`, or a path. Until this is called there is none to
    /// open.
    pub fn set_launcher(&mut self, program: impl Into<PathBuf>) {
        self.launcher.0 = Some(program.into());
    }

    /// Opens the launcher, unless one is open already.
    fn open_launcher(&mut self) {
        let (program, open) = &self.launcher;
        let open = open.is_some_and(|open| self.children.iter().any(|child| child.id() == open));
        let program = program.clone().filter(|_| !open);
        if let Some(program) = program {
            // It has the keyboard, whatever had it, while it is open.
            let roles = Roles::OVERLAY;
            self.launcher.1 = self.start(program.as_os_str(), &[], roles);
        }
    }

    /// Starts a program in the session, and returns the number of its
    /// process. One that cannot be started is a problem to tell the user
    /// of.
    ///
    /// A program that is to have `roles` is handed its connection to the
    /// compositor, already made and known to have them: as what it reads
    /// from, which it is told by `WAYLAND_SOCKET` in the way programs
    /// understand. An ordinary program finds the socket by its name and is
    /// given nothing to read.
    fn start(&mut self, program: &OsStr, arguments: &[String], roles: Roles) -> Option<u32> {
        self.launch(program, arguments, (roles, false))
    }

    /// Starts a program as [`Compositor::start`] does, and, if it is
    /// `heard`, listens to what it says, as the watcher of what records
    /// says it.
    fn launch(
        &mut self,
        program: &OsStr,
        arguments: &[String],
        (roles, heard): (Roles, bool),
    ) -> Option<u32> {
        let mut process = std::process::Command::new(program);
        if heard {
            process.stdout(Stdio::piped());
        }
        // The token says the window it opens was asked for.
        let ticket = self.ticket();
        process
            .args(arguments)
            .stdin(Stdio::null())
            .envs(self.session_environment())
            .env("XDG_ACTIVATION_TOKEN", &ticket)
            .env("DESKTOP_STARTUP_ID", &ticket);
        let trusted = (roles != Roles::NONE).then(UnixStream::pair);
        let handed = trusted.and_then(Result::ok).and_then(|(ours, theirs)| {
            let known = self.insert_client(ours, roles).ok();
            known.map(|_| theirs)
        });
        if let Some(theirs) = handed {
            process
                .stdin(OwnedFd::from(theirs))
                .env("WAYLAND_SOCKET", "0");
        }
        let started = match process.spawn() {
            Ok(mut child) => {
                self.wake_when_exited(&child);
                let process = child.id();
                if let Some(said) = child.stdout.take() {
                    self.listen_to(said);
                }
                self.children.push(child);
                Some(process)
            }
            Err(error) => {
                self.problems
                    .push(format!("cannot start `{}`: {error}", program.display()));
                None
            }
        };
        self.started(ticket, started);
        started
    }

    /// Says where the panel that comes with omorfos is: a name to look up
    /// in `PATH`, or a path. It is then started with the session, unless
    /// the configuration turns it off. Until this is called there is none
    /// to start.
    pub fn set_panel(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Panel, program.into());
    }

    /// Says where the wallpaper that comes with omorfos is: a name to look
    /// up in `PATH`, or a path. It is then started with the session, unless
    /// the configuration turns it off. Until this is called there is none
    /// to start.
    pub fn set_wallpaper(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Wallpaper, program.into());
    }

    /// Says where the program that shows notifications is: a name to look
    /// up in `PATH`, or a path. It is then started with the session,
    /// unless the configuration turns it off. Until this is called there
    /// is none to start.
    pub fn set_notifier(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Notifier, program.into());
    }

    /// Says where the program that keeps what is copied is, once the
    /// program it was copied from has closed: a name to look up in `PATH`,
    /// or a path. It is then started with the session, unless the
    /// configuration turns it off. Until this is called there is none to
    /// start.
    pub fn set_keeper(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Keeper, program.into());
    }

    /// Says where the program that watches what records from a microphone
    /// or a camera is: a name to look up in `PATH`, or a path. It is then
    /// started with the session, and nothing turns it off. Until this is
    /// called there is none to start.
    pub fn set_recorder(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Recorder, program.into());
    }

    /// Says where the program that answers the portals is: a name to look
    /// up in `PATH`, or a path. It is then started with the session,
    /// unless the configuration turns it off. Until this is called there
    /// is none to start.
    pub fn set_portal(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Portal, program.into());
    }

    /// Says where the program that watches over the session is, which
    /// locks it when logind asks and before the machine sleeps: a name to
    /// look up in `PATH`, or a path. It is then started with the session,
    /// and nothing turns it off. Until this is called there is none to
    /// start.
    pub fn set_session(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Session, program.into());
    }

    /// Has the programs the configuration lists to start with the session
    /// started through `program`, `systemd-run`, each in a scope that is
    /// part of the graphical session, so that it is stopped when the
    /// session ends. Until this is called each is started as it is.
    pub fn set_scoper(&mut self, program: impl Into<PathBuf>) {
        self.autostarted.scoper = Some(program.into());
    }

    /// Says where the program that asks for secrets is, polkit's agent for
    /// the session: a name to look up in `PATH`, or a path. It is then
    /// started with the session, with the role to ask in the sealed
    /// prompt, and nothing turns it off. Until this is called there is
    /// none to start.
    pub fn set_asker(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Asker, program.into());
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
    /// is wanted, then the programs the configuration lists to be started
    /// with the session, and then those the autostart entries of the user
    /// and the system list, where omorfos is the whole session. A
    /// backend calls this once it is ready for them: when the sockets are
    /// open and there is a screen to show them on.
    ///
    /// They are started once. Calling this again, or changing the list and
    /// reloading, starts nothing: what is already running would be started
    /// twice.
    pub fn autostart(&mut self) {
        if std::mem::replace(&mut self.autostarted.done, true) {
            return;
        }
        let config = Arc::clone(self.store.current());
        let mut components = std::mem::take(&mut self.components);
        for component in &mut components {
            if component.kind.wanted(&config.general) {
                let program = component.program.as_os_str();
                let heard = component.kind == Program::Recorder;
                let how = (component.kind.roles(), heard);
                component.process = self.launch(program, &[], how);
            }
        }
        self.components = components;
        // Each in a scope of its own, where there is what makes one, so
        // that it ends with the session: a program that is not the
        // compositor's client is told of nothing else that does.
        let scoper = self.autostarted.scoper.clone();
        for command in &config.general.autostart {
            let run = &command.run;
            let (program, arguments) = scoped(scoper.as_deref(), run.program(), run.arguments());
            self.start(program.as_ref(), &arguments, Roles::named(&command.may));
        }
        // Then what the desktop entries in `~/.config/autostart` and the
        // system's say is to start, which the program that watches over the
        // session reads and starts as ordinary programs, once.
        let session = self
            .components
            .iter()
            .find(|component| component.kind == Program::Session);
        let session = session.map(|component| component.program.clone());
        if let Some(session) = session {
            self.launch(
                session.as_os_str(),
                &["autostart".to_owned()],
                (Roles::NONE, false),
            );
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
                let program = component.program.as_os_str();
                let heard = component.kind == Program::Recorder;
                let how = (component.kind.roles(), heard);
                component.process = self.launch(program, &[], how);
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
        let mut veils = Veils::kept(&mut previous, &config.theme);
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
                    let private = self.desktop.private(*id);
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
                            veil: private.then(|| veils.veil(*id, frame.geometry.size)),
                            private,
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
        self.repoint();
        self.note_mark();
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
                private: false,
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
        // The notice is on the output that comes first, which is where a
        // session with one screen has it.
        let first = self.primary_head();
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
        self.repaint_switcher();
        let locked = self.is_locked();
        self.animation.fade_out(theme, &self.scene, locked);
        // Everything that changes colour, comes in or goes has been asked
        // after by now.
        self.animation.motion.end();
    }

    /// Gives the keyboard to whoever should have it now.
    fn sync_keyboard_focus(&mut self, focused: Option<WindowId>) {
        let sealed = self.sealed().map(|(_, surface, _)| surface);
        if self.is_locked() || sealed.is_some() {
            // The lock surface has the keyboard, or the prompt that asks for
            // a secret, or nothing does. Nothing below is consulted.
            let guarded = if self.is_locked() {
                self.lock_focus().cloned()
            } else {
                sealed
            };
            let focus = guarded.map(Focus::Surface);
            let keyboard = self.keyboard.clone();
            if keyboard.current_focus() != focus {
                keyboard.set_focus(self, focus, SERIAL_COUNTER.next_serial());
            }
            return;
        }
        if self.switcher_open() {
            // The switcher has the keyboard while it is open, so that the
            // window with focus does not see Alt go down and come up with
            // nothing between, which a browser takes as Alt pressed alone
            // and shows its menu bar for.
            let keyboard = self.keyboard.clone();
            if keyboard.current_focus().is_some() {
                keyboard.set_focus(self, None, SERIAL_COUNTER.next_serial());
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
                // Only what was started to may take the keyboard from
                // whatever has it.
                let roles = Self::roles(layer.wl_surface().client().as_ref());
                layer.cached_state().keyboard_interactivity == KeyboardInteractivity::Exclusive
                    && matches!(layer.layer(), Layer::Top | Layer::Overlay)
                    && roles.has(Roles::OVERLAY)
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

    /// Draws what `output` shows into `framebuffer`, and gives the
    /// programs taking pictures of it those that are due.
    ///
    /// `age` is how many frames old the framebuffer's contents are, or `0`
    /// if unknown, and lets the damage tracker redraw only what changed.
    /// `P` is what the renderer draws a picture of a window by itself in,
    /// to be read back: one as large as the window, and see-through where
    /// it is.
    ///
    /// # Errors
    ///
    /// Returns an error if the renderer fails.
    pub fn render<R, P>(
        &mut self,
        output: &Output,
        renderer: &mut R,
        framebuffer: &mut R::Framebuffer<'_>,
        tracker: &mut OutputDamageTracker,
        age: usize,
    ) -> Result<Drawn, RenderError<R::Error>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Bind<Dmabuf> + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let head = self.head(output);
        let view = View {
            origin: head.map(|head| head.area().loc).unwrap_or_default(),
            scale: head.map_or(Scale::ONE, |head| head.scale),
        };
        if self.is_locked() {
            // Nothing of the session is drawn while it is locked: only what
            // the locker shows, on black. Nothing else is even looked at.
            let elements = self.locked_elements(renderer, output, view);
            if let Some(head) = self.head(output) {
                head.blanked.set(true);
            }
            let drawn = tracker.render_output(renderer, framebuffer, age, &elements, BLACK);
            return drawn.map(|drawn| Drawn::of(drawn, Vec::new()));
        }
        // A pointer an application draws is drawn by itself for programs
        // following it, on the screen it is on.
        let here = self
            .pointer_head()
            .is_some_and(|head| head.output == *output);
        let drawing = self.pointer_to_draw().filter(|_| here).cloned();
        if let Some(surface) = drawing {
            self.draw_pointer::<R, P>(renderer, &surface, view.scale);
        }
        let desktop = self.config().theme.style(Part::Desktop, States::NONE);
        let behind = color(desktop.background);
        // A picture a program is to be given is drawn first, without the
        // mark and with the pointer only if the program asked for it, and
        // read back. What the screen shows is drawn over it after.
        let mut age = age;
        let mut before = Vec::new();
        for pass in self.capture.wanted(output) {
            let (elements, behind) = match pass {
                Pass::Screen { pointer } => {
                    let shown = Extras {
                        pointer,
                        mark: false,
                    };
                    (self.elements(renderer, output, view, shown), behind)
                }
                Pass::Window { window, pointer } => (
                    self.alone(renderer, window, (view.scale, pointer)),
                    Color32F::TRANSPARENT,
                ),
            };
            let due = self.capture.due(output, pass, &elements);
            // A picture asked for in the memory of the graphics card is
            // drawn there and nothing of it is read back; the rest are
            // read back from what the screen is drawn into.
            let (on_card, due) = self.capture.split(due);
            let scale = view.scale.factor();
            for (due, mut buffer) in on_card {
                let at = (due.size(), scale);
                let drawn = gpu::draw(renderer, &mut buffer, at, &elements, behind);
                self.capture.give(due, drawn, self.animation.clock);
            }
            // A window by itself is drawn in a picture of its own, as large
            // as it is, whatever the screen is; and so is a screen that is
            // turned, as what it is drawn into is turned with it, and the
            // pictures are to be the right way up.
            let turned = self
                .head(output)
                .is_some_and(|head| head.turned() != Transform::Normal);
            if matches!(pass, Pass::Window { .. }) || turned {
                self.read_alone::<R, P>(renderer, (&elements, behind), due, scale);
                continue;
            }
            if due.is_empty() {
                continue;
            }
            // If this cannot be drawn, neither can what follows, which is
            // where it is said.
            let drawn = tracker.render_output(renderer, framebuffer, age, &elements, behind);
            let changed = drawn
                .iter()
                .flat_map(|drawn| drawn.damage.into_iter().flatten());
            before.extend(changed.copied());
            age = 1;
            // Only what the pictures need is read back: what has changed,
            // and what their programs say is out of date in their buffers.
            // Red first is the one order every renderer can give.
            let part = Capture::reading(&due).unwrap_or_default();
            let (at, of) = ((part.loc.x, part.loc.y), (part.size.w, part.size.h));
            let read = renderer.copy_framebuffer(
                framebuffer,
                Rectangle::new(at.into(), of.into()),
                Fourcc::Abgr8888,
            );
            let read = read.ok();
            let pixels = read
                .as_ref()
                .and_then(|read| renderer.map_texture(read).ok());
            self.capture.hand(due, pixels, part, self.animation.clock);
        }
        let shown = Extras {
            pointer: true,
            mark: true,
        };
        let elements = self.elements(renderer, output, view, shown);
        let drawn = tracker.render_output(renderer, framebuffer, age, &elements, behind);
        drawn.map(|drawn| Drawn::of(drawn, before))
    }

    /// Where the mark that says the screen is captured is on `output`: in
    /// the corner of what the panels leave, so as not to cover the end of
    /// one along the top.
    fn mark_area(&self, output: &Output) -> Rectangle<i32, Logical> {
        let area = self.area_of(output);
        let free = layer_map_for_output(output).non_exclusive_zone();
        let right = area.loc.x + free.loc.x + free.size.w;
        let (width, height) = paint::capture_mark_size(self.signs(output));
        let left = right - capture::MARGIN - width.cast_signed();
        let top = area.loc.y + free.loc.y + capture::MARGIN;
        Rectangle::new(
            (left, top).into(),
            (width.cast_signed(), height.cast_signed()).into(),
        )
    }

    /// What the mark on `output` says when the pointer rests on it: which
    /// programs are taking pictures there, and of what.
    fn mark_says(&self, output: &Output) -> Vec<String> {
        let recorders = &self.capture.recording.now;
        let mut said = Vec::new();
        for (client, window) in self.capture.takers(output) {
            let pid = client.get_credentials(&self.display).ok();
            let pid = pid.and_then(|known| u32::try_from(known.pid).ok());
            let what = if window {
                "a window on this screen"
            } else {
                "this screen"
            };
            // A program that shares what it takes, as the portal does, is
            // named with what it shares it with, which PipeWire knows.
            let shown = recorders.iter().filter(|recorder| {
                recorder.kind == Recorded::Screen
                    && recorder.through.is_some_and(|through| Some(through) == pid)
            });
            let shown: Vec<String> = shown
                .map(|recorder| {
                    let (seer, sharer) = (program(Some(recorder.pid)), program(pid));
                    format!("{seer} is shown {what}, through {sharer}")
                })
                .collect();
            if shown.is_empty() {
                said.push(format!("{} is taking pictures of {what}", program(pid)));
            }
            said.extend(shown);
        }
        let screen = !said.is_empty();
        let recording = recorders.iter().filter_map(|recorder| {
            let what = match recorder.kind {
                Recorded::Microphone => "is recording from the microphone",
                Recorded::Sound => "is recording the sound that plays",
                Recorded::Camera => "is using the camera",
                Recorded::Screen => return None,
            };
            Some((
                recorder.kind,
                format!("{} {what}", program(Some(recorder.pid))),
            ))
        });
        let recording: Vec<(Recorded, String)> = recording.collect();
        let has = |kinds: &[Recorded]| recording.iter().any(|(kind, _)| kinds.contains(kind));
        let (sound, camera) = (
            has(&[Recorded::Microphone, Recorded::Sound]),
            has(&[Recorded::Camera]),
        );
        said.extend(recording.into_iter().map(|(_, line)| line));
        // What has stopped, and is still on the mark for a moment.
        let signs = self.signs(output);
        let ago = [
            (
                signs.screen && !screen,
                "Pictures of this screen were taken a moment ago",
            ),
            (signs.sound && !sound, "Sound was recorded a moment ago"),
            (signs.camera && !camera, "The camera was used a moment ago"),
        ];
        let ago = ago.into_iter().filter(|(stopped, _)| *stopped);
        said.extend(ago.map(|(_, line)| line.to_owned()));
        said
    }

    /// Brings what the mark says up to date with where the pointer is:
    /// shown while it rests on the mark of a screen being captured, and
    /// painted again only when what it says differs.
    fn note_mark(&mut self) {
        let point = self.pointer_location;
        let marked = self
            .heads
            .iter()
            .filter(|head| self.signs(&head.output).any());
        let mut under = marked.filter(|head| self.mark_area(&head.output).to_f64().contains(point));
        let wanted = under.next().map(|head| {
            let mark = self.mark_area(&head.output);
            (
                head.output.clone(),
                self.mark_says(&head.output),
                head.scale,
                mark,
            )
        });
        let theme = &self.store.current().theme;
        let had = self.hover.mark.take();
        self.hover.mark = wanted.map(|(output, says, scale, mark)| {
            let same =
                |note: &MarkNote| note.says == says && note.scale == scale && note.theme == *theme;
            had.filter(same).unwrap_or_else(|| {
                let canvas = paint::tooltip(&mut self.text, theme, &says, scale);
                let (width, height) = canvas.units();
                // Under the mark, at its right.
                let left = mark.loc.x + mark.size.w - width.cast_signed();
                let top = mark.loc.y + mark.size.h + 4;
                MarkNote {
                    output,
                    says,
                    scale,
                    theme: theme.clone(),
                    area: Rect::new(DomainPoint::new(left, top), DomainSize::new(width, height)),
                    pixels: Painted::new(&canvas),
                }
            })
        });
    }

    /// What the mark that says the screen is captured says, while the
    /// pointer rests on it: a line for each program taking pictures. Empty
    /// while it is not shown.
    #[must_use]
    pub fn mark_tooltip(&self) -> &[String] {
        self.hover
            .mark
            .as_ref()
            .map_or(&[], |note| note.says.as_slice())
    }

    /// Draws a window by itself, or a screen that is turned, as `elements`
    /// have it over `behind`, in a picture the right way up as large as the
    /// largest of those `due`, and gives them what they need of it.
    fn read_alone<R, P>(
        &mut self,
        renderer: &mut R,
        (elements, behind): (&[Element<R>], Color32F),
        due: Vec<Due>,
        scale: f64,
    ) where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let largest = |one: Size<i32, Physical>, other: Size<i32, Physical>| {
            Size::from((one.w.max(other.w), one.h.max(other.h)))
        };
        let Some(size) = due.iter().map(Due::size).reduce(largest) else {
            return;
        };
        let part = Capture::reading(&due).unwrap_or_default();
        let (at, of) = ((part.loc.x, part.loc.y), (part.size.w, part.size.h));
        let reading = Rectangle::new(at.into(), of.into());
        let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
        let picture = self.capture.alone::<R, P>(renderer, size);
        let target = picture.and_then(|picture| renderer.bind(picture).ok());
        let read = target.and_then(|mut target| {
            let drawn = tracker.render_output(renderer, &mut target, 0, elements, behind);
            let read = drawn
                .ok()
                .map(|_| renderer.copy_framebuffer(&target, reading, Fourcc::Abgr8888));
            read.and_then(Result::ok)
        });
        let pixels = read
            .as_ref()
            .and_then(|read| renderer.map_texture(read).ok());
        self.capture.hand(due, pixels, part, self.animation.clock);
    }

    /// A window by itself, from the top left corner of the screen, as a
    /// picture of it is taken: what it draws, without the frame omorfos
    /// puts round it, and the pointer over it if `pointer`. A blank, if a
    /// rule says it is private.
    fn alone<R>(
        &self,
        renderer: &mut R,
        window: WindowId,
        (scale, pointer): (Scale, bool),
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let corner = Point::<i32, Physical>::default();
        // The pointer is drawn where it is over the picture, which begins
        // where all the window draws does.
        let mut elements = Vec::new();
        let begins = self.pictured(window).filter(|_| pointer);
        if let Some((area, _)) = begins {
            let origin = area.loc;
            self.pointer_elements(renderer, View { origin, scale }, &mut elements);
        }
        if self.desktop.private(window) {
            let blank = self.capture.blank(window).into_iter();
            let blank = blank.map(|blank| {
                SolidColorRenderElement::from_buffer(blank, corner, 1.0, 1.0, Kind::Unspecified)
            });
            elements.extend(blank.map(Element::Solid));
            return elements;
        }
        let mapped = self.windows.iter().find(|mapped| mapped.id == window);
        let surfaces = mapped.into_iter().flat_map(|mapped| {
            // All that the window draws, the shadow it may draw round
            // what it says is the window included.
            let within = Point::<i32, Logical>::default() - mapped.window.bbox().loc;
            let at = within.to_f64().to_physical(scale.factor()).to_i32_round();
            let scale = smithay::utils::Scale::from(scale.factor());
            mapped
                .window
                .render_elements::<WaylandSurfaceRenderElement<R>>(renderer, at, scale, 1.0)
        });
        elements.extend(surfaces.map(Element::Surface));
        elements
    }

    /// What a locked screen shows: the pointer, over what the locker put
    /// there.
    fn locked_elements<R>(&self, renderer: &mut R, output: &Output, view: View) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let unspecified = Kind::Unspecified;
        let mut elements: Vec<Element<R>> = Vec::new();
        self.pointer_elements(renderer, view, &mut elements);
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
        elements
    }

    /// Everything an unlocked screen shows, from the top of the stack
    /// down, with or without what `shown` names.
    fn elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        shown: Extras,
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let mut elements: Vec<Element<R>> = Vec::new();
        let signs = self.signs(output);
        if shown.mark && signs.any() {
            // Over everything, the pointer too: nothing hides it; and over
            // that, what it says when the pointer rests on it.
            let tooltip = self
                .hover
                .mark
                .as_ref()
                .filter(|note| note.output == *output);
            let tooltip = tooltip.and_then(|note| {
                let at = Point::<i32, Logical>::from((note.area.origin.x, note.area.origin.y));
                note.pixels.element(renderer, at.to_f64(), view, 1.0)
            });
            elements.extend(tooltip);
            let at = self.mark_area(output).loc;
            let mark = self.capture.mark(view.scale, signs);
            elements.extend(mark.and_then(|mark| mark.element(renderer, at.to_f64(), view, 1.0)));
            // And round a screen that is shared, a line, which is no more in
            // a picture of it than the mark is.
            let corner = self.area_of(output).loc;
            for (at, side) in self.capture.edges(&Subject::Screen(output.clone())) {
                let at = view.pixel((corner + *at).to_f64());
                let scale = view.scale.factor();
                let side =
                    SolidColorRenderElement::from_buffer(side, at, scale, 1.0, Kind::Unspecified);
                elements.push(Element::Solid(side));
            }
        }
        if shown.pointer {
            self.pointer_elements(renderer, view, &mut elements);
        }
        // A prompt that asks for a secret is over all but the pointer and the
        // mark, and dims the rest; no input method types into it.
        self.sealed_elements(renderer, output, view, &mut elements);
        self.popup_elements(renderer, view, &mut elements);
        if let Some(notice) = &self.notice {
            let at = Point::<i32, Logical>::from((notice.area.origin.x, notice.area.origin.y));
            elements.extend(notice.pixels.element(
                renderer,
                at.to_f64(),
                view,
                self.animation.notice_opacity,
            ));
        }
        elements.extend(self.switcher_elements(renderer, output, view));
        for (at, panel) in self.menu_chrome.iter().flat_map(|chrome| &chrome.panels) {
            elements.extend(panel.element(
                renderer,
                at.to_f64(),
                view,
                self.animation.menu_opacity,
            ));
        }
        if let Some(shown) = &self.hover.palette.shown {
            // Its arrow stands above it, up to the titlebar.
            let up = shown.area.origin.offset(0, -palette::ARROW.cast_signed());
            let at = Point::<i32, Logical>::from((up.x, up.y));
            elements.extend(shown.pixels.element(renderer, at.to_f64(), view, 1.0));
        }
        self.gone_elements(renderer, view, (Level::Over, shown.mark), &mut elements);
        // From the top down: overlays, fullscreen windows, panels and
        // docks, ordinary windows, then whatever sits behind them. What
        // has gone from among each of them is over those that are left.
        let layers = [Layer::Overlay, Layer::Top, Layer::Bottom, Layer::Background];
        let frames = [Some(true), Some(false), None, None];
        for (layer, fullscreen) in layers.into_iter().zip(frames) {
            let seen = shown.mark;
            self.gone_elements(renderer, view, (Level::Layer(layer), seen), &mut elements);
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
                    // Under them, the windows offered in a part with nothing
                    // in it.
                    elements.extend(self.offer_titles(renderer, view));
                    elements.extend(self.offer_elements(renderer, view));
                }
                let frames = (Level::Frames(fullscreen), shown.mark);
                self.gone_elements(renderer, view, frames, &mut elements);
                self.frame_elements(renderer, view, (fullscreen, shown.mark), &mut elements);
            }
        }
        elements
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
        match (&self.shown, &self.pointer_images.shown) {
            (Look::Icon(_), Some(image)) => {
                let at = self.pointer_location - image.hotspot;
                elements.extend(image.picture.cursor(renderer, at, view));
            }
            (Look::Surface(surface), _) => {
                let at = self.pointer_location - cursor_hotspot(surface).to_f64();
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
    /// fading, the last to go first. A window a rule called private is
    /// not in a picture as it fades, any more than it was before: only
    /// where it is `seen`.
    fn gone_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        (level, seen): (Level, bool),
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let gone = self.animation.gone.iter().rev();
        let shown = |gone: &&Gone| gone.level == level && (seen || !gone.private);
        for gone in gone.filter(shown) {
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
        (fullscreen, seen): (bool, bool),
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
            if let Some(veil) = shown.veil.as_ref().filter(|_| !seen) {
                // In a picture a private window is a blank the size of
                // its frame: its titlebar says what it is as well.
                let at = Point::<i32, Logical>::from((shown.outer.origin.x, shown.outer.origin.y));
                let at = view.pixel(at.to_f64());
                let scale = view.scale.factor();
                let blank =
                    SolidColorRenderElement::from_buffer(veil, at, scale, 1.0, Kind::Unspecified);
                elements.push(Element::Solid(blank));
                continue;
            }
            // Round a window that is shared, a line over its frame, which
            // is no more in a picture than the one round a screen is.
            let corner = Point::<i32, Logical>::from((shown.outer.origin.x, shown.outer.origin.y));
            let lined = Subject::Window(shown.id);
            let sides = self.capture.edges(&lined).filter(|_| seen);
            for (at, side) in sides {
                let at = view.pixel((corner + *at).to_f64());
                let scale = view.scale.factor();
                let side =
                    SolidColorRenderElement::from_buffer(side, at, scale, 1.0, Kind::Unspecified);
                elements.push(Element::Solid(side));
            }
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
        // Asked before the panels are, which holds them until it is done.
        let sealed = self.sealed().map(|(_, surface, _)| surface);
        let layers = layer_map_for_output(output);
        let index = self.heads.iter().position(|head| head.output == *output);
        let index = index.unwrap_or_default();
        let wanted = |surface: &&LayerSurface| {
            let area = layers.layer_geometry(surface).unwrap_or_default();
            let sealed = sealed.as_ref() == Some(surface.wl_surface());
            self.standing(index, layer, area) == standing && !sealed
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
        self.send_offered_frames(output, elapsed);
        self.send_switched_frames(output, elapsed);
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
pub(crate) struct View {
    /// Where in the space the output's top-left corner is.
    origin: Point<i32, Logical>,
    pub(crate) scale: Scale,
}

impl View {
    /// The pixel of the output that a point of the space falls on.
    pub(crate) fn pixel(self, at: Point<f64, Logical>) -> Point<i32, Physical> {
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

/// What programs hand over pictures in the memory of the graphics card by.
#[derive(Debug)]
pub(crate) struct GpuBuffers {
    pub(crate) state: DmabufState,
    /// What they are offered by, once the backend has said which card.
    global: Option<DmabufGlobal>,
}

impl Default for GpuBuffers {
    fn default() -> Self {
        Self {
            state: DmabufState::new(),
            global: None,
        }
    }
}

/// The pictures of the pointer the compositor has loaded.
#[derive(Debug, Default)]
struct PointerImages {
    /// The picture for the shape being shown.
    shown: Option<PointerImage>,
    /// The ordinary pointer, kept to stand in for one an application draws
    /// itself until that has been drawn for programs following it.
    stand_in: Option<PointerImage>,
    /// The last pointer an application drew, as drawn for programs
    /// following it.
    drawn: Option<DrawnPointer>,
    /// Whether the application has changed that pointer since.
    stale: bool,
    /// How many have been loaded or drawn, which names the next: the first
    /// is 1.
    loaded: u64,
}

/// A pointer an application draws, drawn by itself for programs following
/// the pointer: what it is, at what scale, and the picture.
#[derive(Debug)]
struct DrawnPointer {
    surface: WlSurface,
    scale: Scale,
    given: Given,
}

/// A picture of the pointer as programs that take pictures of it are
/// given it.
#[derive(Debug)]
pub(crate) struct Given {
    /// The picture's pixels, row by row, as `Argb8888` with premultiplied
    /// alpha.
    pub(crate) bytes: Vec<u8>,
    /// How many pixels it is across and down.
    pub(crate) pixels: (u32, u32),
    /// The pixel that is the pointer's position.
    pub(crate) tip: (u32, u32),
    /// Which picture this is: no other loaded or drawn has the same.
    pub(crate) serial: u64,
}

/// The picture of a named pointer shape, and everything it depends on.
#[derive(Debug)]
pub(crate) struct PointerImage {
    icon: CursorIcon,
    scale: Scale,
    theme: omorfos_config::Cursor,
    picture: Painted,
    /// Where in the picture the pointer's position is.
    hotspot: Point<f64, Logical>,
    /// The picture as a program that takes pictures of the pointer is
    /// given it.
    given: Given,
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
        serial: u64,
    ) -> Self {
        let pixels = scale.pixels(theme.size.get().cast_signed()).unsigned_abs();
        let found = [icon, CursorIcon::Default]
            .into_iter()
            .find_map(|icon| themes.load(&theme.theme, icon, pixels));
        let image = found.unwrap_or_else(|| {
            let canvas = paint::pointer(scale);
            cursor::Image {
                pixels: canvas.bytes().to_vec(),
                size: (canvas.width(), canvas.height()),
                hotspot: (0, 0),
            }
        });
        let hotspot = (f64::from(image.hotspot.0), f64::from(image.hotspot.1));
        Self {
            icon,
            scale,
            theme: theme.clone(),
            picture: Painted::from_image(&image, scale),
            hotspot: Point::<f64, Physical>::from(hotspot).to_logical(scale.factor()),
            given: Given {
                bytes: image.pixels,
                pixels: image.size,
                tip: image.hotspot,
                serial,
            },
        }
    }
}

/// Where in the picture an application gave for the pointer its position is.
fn cursor_hotspot(surface: &WlSurface) -> Point<i32, Logical> {
    with_states(surface, |states| {
        let attributes = states.data_map.get::<Mutex<CursorImageAttributes>>();
        attributes
            .and_then(|attributes| attributes.lock().ok().map(|held| held.hotspot))
            .unwrap_or_default()
    })
}

render_elements! {
    /// Everything the compositor draws: client surfaces and its own
    /// decorations.
    pub(crate) Element<R> where R: ImportAll + ImportMem;
    Surface=WaylandSurfaceRenderElement<R>,
    Strip=Strip<WaylandSurfaceRenderElement<R>>,
    Ghost=Ghost<R>,
    Memory=MemoryRenderBufferRenderElement<R>,
    Solid=SolidColorRenderElement,
    Cropped=CropRenderElement<WaylandSurfaceRenderElement<R>>,
    Texture=TextureRenderElement<<R as RendererSuper>::TextureId>,
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

/// What the program running as the process numbered `pid` is called: the
/// name of its file, which it cannot choose as it can a title.
fn program(pid: Option<u32>) -> String {
    let file = pid.and_then(|pid| fs::read_link(format!("/proc/{pid}/exe")).ok());
    let name = file.as_deref().and_then(Path::file_name);
    name.map_or_else(
        || "A program".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Whether the programs to start with the session have been, and what
/// starts them in a scope of their own.
#[derive(Debug, Default)]
struct Autostarted {
    done: bool,
    scoper: Option<PathBuf>,
}

/// The program and arguments that start `program` with `arguments`: by
/// `scoper`, `systemd-run`, in a scope that is part of the graphical
/// session and stops with it, where there is one; and as it is otherwise.
/// It is the same process either way, which `systemd-run` becomes.
pub(crate) fn scoped(
    scoper: Option<&Path>,
    program: &str,
    arguments: &[String],
) -> (String, Vec<String>) {
    let Some(scoper) = scoper else {
        return (program.to_owned(), arguments.to_vec());
    };
    let through = [
        "--user",
        "--scope",
        "--quiet",
        "--collect",
        "--slice=app-graphical.slice",
        "--property=PartOf=graphical-session.target",
        "--property=After=graphical-session.target",
        "--",
        program,
    ];
    let mut all: Vec<String> = through.into_iter().map(str::to_owned).collect();
    all.extend_from_slice(arguments);
    (scoper.to_string_lossy().into_owned(), all)
}

/// What is said when the session is to be locked and nothing is set to
/// lock it with.
pub(crate) const NO_LOCKER: &str = "nothing is set to lock the session with: name a screen \
     locker as `locker` in config.yaml, such as `locker: [swaylock, -f]`";

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
    Wallpaper,
    Notifier,
    Portal,
    Keeper,
    Recorder,
    Session,
    Asker,
}

impl Program {
    /// Whether the configuration has it running.
    const fn wanted(self, general: &omorfos_config::General) -> bool {
        match self {
            Self::Panel => general.panel.enabled,
            Self::Wallpaper => general.wallpaper.enabled,
            Self::Notifier => general.notifications.enabled,
            Self::Portal => general.portals.enabled,
            Self::Keeper => general.clipboard.enabled,
            // The mark is not to be turned off, nor what it shows; nor is
            // locking before sleep, and without logind it ends at once.
            Self::Recorder | Self::Session | Self::Asker => true,
        }
    }

    /// What it may do. The program that answers the portals asks the user
    /// over everything, with the keyboard, and takes the picture that is
    /// given.
    const fn roles(self) -> Roles {
        match self {
            Self::Panel | Self::Wallpaper | Self::Notifier => Roles::PANEL,
            Self::Portal => Roles::PANEL.with(Roles::OVERLAY).with(Roles::CAPTURE),
            Self::Keeper => Roles::CLIPBOARD,
            // One speaks to PipeWire, and to the compositor only over the
            // pipe it says what it finds through; the other asks to lock
            // as anyone may, over the socket for scripts, and the locker
            // that is started has the role.
            Self::Recorder | Self::Session => Roles::NONE,
            // It asks for secrets, over a dimmed session, with the keyboard.
            Self::Asker => Roles::ASK,
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
    /// The arrangements a window can be put in, offered when it rests on
    /// the second button of a titlebar.
    pub(crate) palette: Palette,
    /// The surface of a program it was last said to be over, and where on
    /// it; and the time it last moved.
    pub(crate) pointed: (Focused, u32),
    /// What the mark that says a screen is captured says, while it rests
    /// on it.
    mark: Option<MarkNote>,
}

/// What the mark that says a screen is captured says, painted.
#[derive(Debug)]
struct MarkNote {
    output: Output,
    says: Vec<String>,
    scale: Scale,
    theme: Theme,
    /// Where it is shown, under the mark.
    area: Rect,
    pixels: Painted,
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
    /// Whether a rule called it private when it went.
    private: bool,
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
                private: shown.private,
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
            private: false,
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
pub(crate) struct Painted {
    pixels: MemoryRenderBuffer,
    /// The size on screen, in units of layout.
    size: Size<i32, Logical>,
    /// All of the pixels, which is the part to draw.
    whole: Rectangle<f64, Logical>,
}

impl Painted {
    /// Readies a canvas to be drawn.
    pub(crate) fn new(canvas: &paint::Canvas) -> Self {
        let pixels = (canvas.width().cast_signed(), canvas.height().cast_signed());
        let (width, height) = canvas.units();
        let size = (width.cast_signed(), height.cast_signed());
        Self::from_pixels(canvas.bytes(), pixels, size)
    }

    /// How wide it is on screen, in units of layout.
    pub(crate) fn width(&self) -> u32 {
        self.size.w.unsigned_abs()
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
    pub(crate) fn element<R>(
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
    pub(crate) surface: WlSurface,
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
    /// What is drawn in its place in a picture taken of the screen, if a
    /// rule says it is private. Kept, so that a picture after the first
    /// is not thought to differ where it is.
    veil: Option<SolidColorBuffer>,
    /// Whether a rule calls it private, which it still is once the veil
    /// it was drawn with has gone to the next frame.
    private: bool,
}

/// What stands for private windows in pictures taken of the screen, kept
/// from one laying out of the scene to the next so that none is something
/// new each time, which a program taking pictures would be told of.
struct Veils {
    kept: Vec<(WindowId, SolidColorBuffer)>,
    color: Color32F,
}

impl Veils {
    /// Takes what stood for the windows of a scene, to be given to those
    /// of the next. They are the colour of the mark of capture.
    fn kept(scene: &mut [Shown], theme: &Theme) -> Self {
        let kept = scene.iter_mut();
        let kept = kept.filter_map(|shown| shown.veil.take().map(|veil| (shown.id, veil)));
        Self {
            kept: kept.collect(),
            color: color(theme.style(Part::Capture, States::NONE).background),
        }
    }

    /// What stands for a window whose frame is of a size.
    fn veil(&mut self, id: WindowId, size: DomainSize) -> SolidColorBuffer {
        let kept = self.kept.iter().position(|(of, _)| *of == id);
        let kept = kept.map(|index| self.kept.swap_remove(index).1);
        let mut veil = kept.unwrap_or_default();
        veil.update(logical_size(size), self.color);
        veil
    }
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
