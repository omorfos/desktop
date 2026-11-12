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
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use omorfos_brand::pointer::Shape;
use omorfos_config::{
    Action, Config, Debouncer, Font, Half, Part, Scheme, State, States, Store, Style, Theme,
    Watcher, WorkspaceNumber,
};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{
    Button, Decoration, Desktop, Frame, Home, Item, Mark, Menu, MenuMetrics, Monitor, Place,
    Screen, Showing, Sign, Standing, System, WindowId, WorkspaceId,
};
use omorfos_ui::{Area, Seen};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::process::{Pid, PidfdFlags, Signal, kill_process, pidfd_open};
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
use smithay::backend::renderer::element::utils::{
    CropRenderElement, Relocate, RelocateRenderElement, RescaleRenderElement,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind, render_elements};
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::backend::renderer::{
    Color32F, ExportMem, ImportAll, ImportMem, Offscreen, Renderer, RendererSuper, Texture,
};
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::desktop::{LayerMap, LayerSurface, PopupManager, Space, Window, layer_map_for_output};
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
    ClockSource, IsAlive, Logical, Monotonic, Physical, Point, Rectangle, SERIAL_COUNTER, Size,
    Transform,
};
use smithay::wayland::alpha_modifier::AlphaModifierState;
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
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::security_context::SecurityContextState;
use smithay::wayland::selection::SelectionTarget;
use smithay::wayland::selection::data_device::{DataDeviceState, set_data_device_selection};
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
use smithay::wayland::xdg_foreign::XdgForeignState;
use smithay::xwayland::X11Surface;

use crate::Scale;
use crate::asked::Asked;
use crate::backdrops::Backdrops;
use crate::bridge::{self, Bindings};
use crate::capture::{self, Capture, Due, Gpu, Pass, Subject};
use crate::corners::{self, Names, Strip};
use crate::cursor::{self, Themes};
use crate::fading::Fade;
use crate::frames::Frames;
use crate::gestures::Gestures;
use crate::ghost::{Ghost, Noted, Noting};
use crate::glass::{self, Frosted, Frosting, Glass, Pane, Scene};
use crate::gpu;
use crate::handlers::{self, Offer};
use crate::hold::Holder;
use crate::holding::Holding;
use crate::icons::WindowIcons;
use crate::input::Drag;
use crate::input::Focused;
use crate::ipc::Server;
use crate::kept::Parts;
use crate::lock::{Awake, Lock, Presence};
use crate::night::Night;
use crate::outputs::{Aside, Head};
use crate::paint::{self, AboutButton, Motion, Text};
use crate::palette::{self, Palette};
use crate::presentation::Feedback;
use crate::recording::Recorded;
use crate::restarts::Restarts;
use crate::reveal::{Placing, Reveal};
use crate::roles::{Refused, Roles};
use crate::scaling::{self, Boxed, Stretch};
use crate::sealed::Seal;
use crate::sent::Way;
use crate::shadows::{Shades, Shadowing};
use crate::standing::Interim;
use crate::switcher::Switch;
use crate::system;
use crate::tiles::Lines;
use crate::typing::Typing;
use crate::versions::{InUse, Own, Versions};
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
    /// The programs to be told when it reaches the screen.
    pub feedback: Feedback,
}

impl Drawn {
    /// What the last drawing of a screen came to, with what the drawings
    /// before it in the same turn changed, and who is to be told when it
    /// is shown.
    fn of(
        last: RenderOutputResult<'_>,
        mut before: Vec<Rectangle<i32, Physical>>,
        feedback: Feedback,
    ) -> Self {
        let damage = if before.is_empty() {
            last.damage.cloned()
        } else {
            before.extend(last.damage.into_iter().flatten().copied());
            Some(before)
        };
        Self {
            damage,
            sync: last.sync,
            feedback,
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
#[derive(Debug)]
pub struct ClientState {
    pub(crate) compositor: CompositorClientState,
    /// What the client may do that an ordinary program may not.
    pub(crate) roles: Roles,
    /// Whether the client came in through a sandbox, which told the
    /// compositor so itself.
    pub(crate) sandboxed: bool,
    /// Who it is, as the relay said, where it came through the relay,
    /// whose own the connection is (ADR 61): its process, and the number
    /// the relay gave it, which the relay keeps across compositors.
    pub(crate) relayed: Option<omorfos_ipc::relay::Program>,
    /// Which program it is, as the log names it, once it is known; and
    /// where it is said that it was disconnected for breaking the protocol.
    named: OnceLock<String>,
    told: Sender<String>,
}

impl wayland_server::backend::ClientData for ClientState {
    fn disconnected(
        &self,
        _: wayland_server::backend::ClientId,
        reason: wayland_server::backend::DisconnectReason,
    ) {
        // A program that left, or ended, is nothing to say. One sent away
        // for asking what the protocol does not allow is, as it may then
        // look to the user as though it crashed by itself.
        if let wayland_server::backend::DisconnectReason::ProtocolError(error) = reason {
            let named = self.named.get().map_or("A program", String::as_str);
            let _ = self.told.send(format!(
                "{named} was disconnected for asking what the Wayland protocol does not allow: \
                 {}@{}: {}",
                error.object_interface, error.object_id, error.message
            ));
        }
    }
}

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
    /// What the relay keeps where it is by, for a window of a program it
    /// keeps (ADR 61, 3c): the number it gave the program, and the
    /// program's number for the window's toplevel.
    pub(crate) kept_by: Option<(u32, u32)>,
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
    pub(crate) protocols: Protocols,
    pub(crate) seat: Seat<Self>,
    pub(crate) keyboard: KeyboardHandle<Self>,
    pub(crate) pointer: PointerHandle<Self>,
    pub(crate) space: Space<Window>,
    pub(crate) popups: PopupManager,
    /// The outputs, left to right.
    pub(crate) heads: Vec<Head>,
    /// The outputs that stand nowhere of their own.
    pub(crate) aside: Aside,
    /// Pictures of the screen being taken by programs that may.
    pub(crate) capture: Capture,
    _surfaces: Offered,
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
    /// The notice of what is wrong, and "About".
    pub(crate) cards: Cards,
    /// How the screens stand for now, as the display key has them.
    pub(crate) interim: Interim,
    pub(crate) text: Text,

    pub(crate) store: Store,
    watcher: Option<Watcher>,
    debouncer: Debouncer,
    /// New versions of the programs that come with omorfos, being taken
    /// up.
    updates: Updates,
    pub(crate) bindings: Bindings,
    pub(crate) problems: Vec<String>,
    /// What has happened lately that the user might want to know of and
    /// need do nothing about, and when.
    events: Vec<(String, Instant)>,
    /// How warm night light has the screens.
    pub(crate) night: Night,

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
    /// The input method, and what is known of it.
    pub(crate) typing: Typing,
    /// What is drawn round a prompt that asks for a secret.
    pub(crate) seal: Seal,
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
    pub(crate) autostarted: Autostarted,
    /// The programs that come with omorfos and run for the whole session,
    /// once a backend has said where each is: the panel first.
    components: Vec<Component>,
    /// The programs started when asked for, rather than with the session.
    on_demand: OnDemand,
    /// What programs have been refused for want of a role.
    pub(crate) refused: Refused,
    /// What is changing colour or coming in, and the time it goes by.
    pub(crate) animation: Animation,
    pub(crate) ipc: Option<Server>,
    /// What the peer that speaks for the machine's radios last said of
    /// them, while it is connected.
    pub(crate) radios: Vec<omorfos_ipc::Radio>,
    /// The event loop, to be woken by what the compositor waits for.
    pub(crate) event_loop: LoopHandle<'static, Self>,
    socket_name: Option<OsString>,
    pub(crate) running: bool,
    /// `omorfos-hold`, where the compositor was started by it.
    pub(crate) holder: Option<Holder>,
}

/// What a surface may be told or tell beyond the core protocol, which is
/// offered and needs nothing more of the compositor: the renderer heeds it
/// where surfaces are drawn.
#[derive(Debug)]
struct Offered {
    _fractional_scale: FractionalScaleManagerState,
    _viewporter: ViewporterState,
    _single_pixels: SinglePixelBufferState,
    /// Lets a program make a surface fainter as a whole, as the wallpaper
    /// does to fade one picture into the next.
    _alpha: AlphaModifierState,
}

impl Offered {
    fn new(display: &DisplayHandle) -> Self {
        Self {
            _fractional_scale: FractionalScaleManagerState::new::<Compositor>(display),
            _viewporter: ViewporterState::new::<Compositor>(display),
            _single_pixels: SinglePixelBufferState::new::<Compositor>(display),
            _alpha: AlphaModifierState::new::<Compositor>(display),
        }
    }
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
                    _surfaces: Offered::new(&display),
                    _sandboxes: SecurityContextState::new::<Self, _>(&display, Self::outside),
                    _cursor_shapes: CursorShapeManagerState::new::<Self>(&display),
                    capture: Capture::offer(&display),
                    asked: Asked::new(&display),
                    switch: Switch::offer(&display),
                    typing: Typing::offer(&display),
                    seal: Seal::default(),
                    protocols: Protocols::offer(&display),
                    clipboards: DataControlState::new::<Self, _>(&display, None, Self::copies),
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
                    aside: Aside::default(),
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
                    cards: Cards::default(),
                    interim: Interim::default(),
                    text: Text::new(),
                    store,
                    watcher,
                    debouncer: Debouncer::new(RELOAD_DELAY),
                    updates: Updates::default(),
                    bindings: Bindings::new([]),
                    problems: Vec::new(),
                    events: Vec::new(),
                    night: Night::default(),
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
                    on_demand: OnDemand::default(),
                    refused: Refused::default(),
                    animation: Animation::new(),
                    ipc: None,
                    radios: Vec::new(),
                    event_loop,
                    socket_name: None,
                    running: true,
                    holder: None,
                };
                compositor.apply_config(startup_problems);
                compositor.wake_on_changes();
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
        self.insert(stream, roles, false, None)
    }

    /// Accepts a client that connected through a sandbox. It has no role
    /// and is not shown what only a role allows.
    pub(crate) fn insert_sandboxed(&mut self, stream: UnixStream) -> io::Result<Client> {
        self.insert(stream, Roles::default(), true, None)
    }

    /// Accepts a program the relay connected, `stream` being its connection
    /// to the compositor, which is the relay's own: who the program is is
    /// what the relay said, `program`, not what the connection says.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be taken.
    pub fn insert_relayed(
        &mut self,
        stream: UnixStream,
        program: omorfos_ipc::relay::Program,
    ) -> io::Result<Client> {
        self.insert(stream, Roles::default(), false, Some(program))
    }

    /// Accepts a program the relay connected through a sandbox, as
    /// [`Compositor::insert_relayed`] accepts a program: inside the
    /// sandbox, with no role, and not shown what only a role allows.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be taken.
    pub fn insert_relayed_sandboxed(
        &mut self,
        stream: UnixStream,
        program: omorfos_ipc::relay::Program,
    ) -> io::Result<Client> {
        self.insert(stream, Roles::default(), true, Some(program))
    }

    /// Accepts a part of the session the relay kept, as
    /// [`Compositor::insert_relayed`] accepts a program: with the roles
    /// the compositor that started it gave it, `roles`, as the relay says,
    /// which is to be trusted as what it says of who a program is is. One
    /// of the programs that come with omorfos is known by what the
    /// compositor before called it, which the relay handed back first, or
    /// where it did not say, as the keeper by its role, as each was handed
    /// before they were called anything; it is then not started again, and
    /// is watched over as if this had started it. So is the locker, by what
    /// it was called. One the configuration started is the user's, and none
    /// of these.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be taken.
    pub fn insert_part(
        &mut self,
        stream: UnixStream,
        program: omorfos_ipc::relay::Program,
        roles: u8,
    ) -> io::Result<Client> {
        let users = roles & USERS != 0;
        let roles = Roles::from_bits(roles & !USERS);
        let inserted = self.insert(stream, roles, false, Some(program));
        let known = self.autostarted.parts.known.get(&program.pid).cloned();
        let keeper = roles
            .has(Roles::CLIPBOARD)
            .then(|| Program::Keeper.called());
        let called = known.as_deref().or(keeper.flatten()).filter(|_| !users);
        inserted.inspect(|_| {
            if called == Some(LOCKER) {
                self.on_demand.locker.0 = Some(program.pid);
                self.adopt(program.pid);
                return;
            }
            let mut components = self.components.iter_mut();
            let found = components.find(|component| component.kind.called() == called);
            if let Some(component) = found.filter(|_| called.is_some()) {
                component.process = Some(program.pid);
                self.adopt(program.pid);
            }
        })
    }

    /// Watches over `process`, a part of the session a compositor before
    /// started, which is not this one's child, for when it ends.
    fn adopt(&mut self, process: u32) {
        let pid = Pid::from_raw(process.cast_signed());
        let opened = pid.and_then(|pid| pidfd_open(pid, PidfdFlags::empty()).ok());
        // Woken once, as when a child exits.
        let woken = opened.as_ref().and_then(|opened| opened.try_clone().ok());
        let exited = |_, _: &mut _, _: &mut Self| Ok(PostAction::Remove);
        let _ = woken
            .map(|woken| Generic::new(woken, Interest::READ, Trigger::Level))
            .and_then(|woken| self.event_loop.insert_source(woken, exited).ok());
        // One that has ended already is found to have at once.
        self.autostarted.parts.adopted.push((process, opened));
    }

    /// What each part of the session the relay keeps is called, as the
    /// next compositor is to know it by, and its process: the locker among
    /// them, while it runs.
    pub(crate) fn kept_parts(&self) -> Vec<(&'static str, u32)> {
        let kept = self.components.iter();
        let kept = kept.filter_map(|component| component.kind.called().zip(component.process));
        let locker = self.on_demand.locker.0.map(|process| (LOCKER, process));
        kept.chain(locker).collect()
    }

    /// Hands the relay, over `relay`, what is to be kept as programs are:
    /// the keeper of the clipboard, as it is started (ADR 61).
    pub fn relay_parts(&mut self, relay: OwnedFd) {
        self.autostarted.parts.relay = Some(relay);
    }

    /// Takes in that the relay has handed over every program it kept: each
    /// part of the session it keeps is started now, unless it was among
    /// them, and so is the locker, if the session came up locked with none
    /// to lock it again.
    pub fn relay_kept(&mut self) {
        self.autostarted.parts.kept = true;
        if self.is_held_by_none() && self.on_demand.locker.0.is_none() {
            self.start_locker();
        }
        if !self.autostarted.done {
            return;
        }
        let config = Arc::clone(self.store.current());
        let mut components = std::mem::take(&mut self.components);
        let missing = components.iter_mut().filter(|component| {
            component.kind.kept()
                && component.process.is_none()
                && component.kind.wanted(&config.general)
        });
        for part in missing {
            let how = (part.kind.roles(), false, Relaying::Own);
            part.process = self.launch(part.program.as_os_str(), &[], how, None);
        }
        self.components = components;
    }

    /// Accepts a client with `roles`, through a sandbox or not, and notes
    /// which program it is, for the log to name it by: its process is
    /// `relayed`, where the relay said so.
    fn insert(
        &mut self,
        stream: UnixStream,
        roles: Roles,
        sandboxed: bool,
        relayed: Option<omorfos_ipc::relay::Program>,
    ) -> io::Result<Client> {
        let state = Arc::new(ClientState {
            compositor: CompositorClientState::default(),
            roles,
            sandboxed,
            relayed,
            named: OnceLock::new(),
            told: self.refused.disconnected.0.clone(),
        });
        let given: Arc<dyn wayland_server::backend::ClientData> = Arc::<ClientState>::clone(&state);
        let inserted = self.display.insert_client(stream, given);
        inserted.inspect(|client| {
            let pid = self.process(client);
            let named = format!("{}, process {}", program(pid), pid.unwrap_or_default());
            let _ = state.named.set(named);
        })
    }

    /// The process of the program `client` is: as the relay said, where it
    /// came through the relay, or as its connection says.
    pub(crate) fn process(&self, client: &Client) -> Option<u32> {
        let relayed = client
            .get_data::<ClientState>()
            .and_then(|state| state.relayed);
        let relayed = relayed.map(|program| program.pid);
        relayed.or_else(|| {
            let credentials = client.get_credentials(&self.display).ok();
            credentials.and_then(|known| u32::try_from(known.pid).ok())
        })
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
        self.autostarted.environment.1 = false;
        self.pointer_images = PointerImages::default();
    }

    /// Says where programs install what they share, desktop entries and
    /// icons among it, the user's own first (see
    /// [`omorfos_icons::data_directories`]). Until this is called windows
    /// have only the icons their programs give as pixels.
    pub fn set_data_directories(&mut self, data: Vec<PathBuf>) {
        self.switch.icons = WindowIcons::new(data);
        let theme = &self.store.current().general.icons.theme;
        self.switch.icons.set_theme(theme);
    }

    /// Works out what the pointer looks like now and readies its picture.
    fn update_pointer(&mut self) {
        if !self.draws_pointer {
            return;
        }
        self.shown = self.cursor_wanted();
        let config = self.store.current();
        let (theme, scheme) = (&config.general.cursor, config.scheme);
        let scale = self.pointer_scale();
        let images = &mut self.pointer_images;
        let loaded = &mut images.loaded;
        let mut ready = |kept: Option<PointerImage>, icon| {
            let current = kept.filter(|image| {
                let looks = image.theme == *theme && image.scheme == scheme;
                image.icon == icon && image.scale == scale && looks
            });
            current.unwrap_or_else(|| {
                *loaded += 1;
                PointerImage::load(&self.themes, (theme, scheme), icon, scale, *loaded)
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

    /// What the log is to be told that it has not been yet, once each: the
    /// programs that were disconnected for breaking the protocol, and why.
    pub fn take_logged(&mut self) -> Vec<String> {
        self.refused.disconnected.1.try_iter().collect()
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

    /// Has the event loop woken when the configuration changes, or a new
    /// version of a program is installed.
    fn wake_on_changes(&self) {
        let watcher = self.watcher.as_ref().map(AsFd::as_fd);
        for source in watcher.into_iter().chain(self.updates.versions.source()) {
            self.wake_on(source);
        }
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
        // And so are the session to be locked and the machine put to sleep,
        // and the end of its being kept awake.
        let away = self.away_at().into_iter().flatten().min();
        let awake = self.presence.awake.and_then(Awake::until);
        // What is changing colour is drawn again a moment later.
        let motions = [&self.animation.motion, &self.animation.portrayal];
        let due = motions.into_iter().filter_map(Motion::deadline).min();
        let moving = self.animation.epoch.zip(due);
        let moving = moving.map(|(epoch, due)| epoch + due);
        // So is a slide of the workspaces that is settling.
        // As is the queue of the switcher, while it turns.
        let settling = self.protocols.gestures.slide.settling() || self.switcher_turning();
        let sliding = self.animation.epoch.filter(|_| settling);
        let sliding = sliding.map(|epoch| epoch + self.animation.clock + Motion::FRAME);
        // And what the screens showed before, while it gives way, a
        // window's last picture while it gives way to its new one, and a
        // window or a line on its way to where it was sent.
        let giving = self
            .scene
            .iter()
            .any(|shown| shown.giving.is_some() || shown.going())
            || self.hover.lines.going(self.animation.clock);
        let fading = self.animation.epoch.filter(|_| self.is_fading() || giving);
        let fading = fading.map(|epoch| epoch + self.animation.clock + Motion::FRAME);
        // As are the screens, while night light fades from one temperature
        // to the next.
        let warming = self.animation.epoch.filter(|_| self.night_fading());
        let warming = warming.map(|epoch| epoch + self.animation.clock + Motion::FRAME);
        // What has happened is said for a while and then no longer.
        let said = self.events.iter().map(|(_, at)| *at + EVENT_LASTS).min();
        // And the switcher, once its keys have been held long enough.
        let showing = self.animation.epoch.zip(self.switcher_due());
        let showing = showing.map(|(epoch, due)| epoch + due);
        // As is the next step of day turning into night.
        let turning = self.store.turns().zip(self.animation.epoch);
        let turning = turning.map(|(due, epoch)| epoch + due);
        [
            turning,
            warming,
            showing,
            self.night_due(Instant::now(), SystemTime::now()),
            self.about_due(),
            self.debouncer.deadline(),
            self.ipc.as_ref().and_then(|server| server.retry),
            self.x11.deadline(),
            self.hover.reveal.deadline(),
            self.hover.palette.deadline(),
            self.capture.deadline(),
            self.capture.recording.deadline(),
            self.updates.versions.due(),
            self.updates.retiring.iter().map(|(_, by)| *by).min(),
            self.updates.next,
            self.shown_by(),
            self.interim.deadline(),
            match self.on_demand.ending {
                Ending::By(by) => Some(by),
                Ending::Not | Ending::Asked => None,
            },
            screens_off,
            away,
            awake,
            moving,
            sliding,
            fading,
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
        // Day turning into night, or night into day, is drawn over a minute;
        // whatever compares the theme it was drawn in is drawn again.
        let _ = self.store.at(self.animation.clock);
        self.light_the_night_if_due(SystemTime::now());
        self.settle_recording(now);
        self.settle_capture(now);
        self.wait_for_relay(now);
        self.fade();
        // Collect children that have exited so they do not linger as
        // zombies, noting those that did not exit as they should have.
        let (mut failed, mut ended) = (Vec::new(), Vec::new());
        self.children.retain_mut(|child| {
            // One that cannot even be asked is taken to have failed.
            let waited = child.try_wait();
            let exited = waited.map_or(Some(false), |status| status.map(|status| status.success()));
            if exited == Some(false) {
                failed.push(child.id());
            }
            if exited.is_some() {
                ended.push(child.id());
            }
            exited.is_none()
        });
        // So are the parts kept from a compositor before, which are not its
        // children: how each ended cannot be known, and is taken to have
        // been a failure.
        let adopted = &mut self.autostarted.parts.adopted;
        let gone = adopted.extract_if(.., |(_, opened)| opened.as_ref().is_none_or(has_ended));
        for (process, _) in gone {
            failed.push(process);
            ended.push(process);
        }
        self.forget_ended();
        self.end(now);
        self.revive(&failed, now);
        self.relock(&ended, now);
        self.await_kept_locker(now);
        self.take_up(&ended, now);
        self.serve_ipc();
        self.manage_x11(now);
        self.reveal(now);
        self.offer_arrangements(now);
        self.settle_slide();
        self.sync();
        self.note_use();
        self.rest_or_wake(now);
        self.take_up_own(now);
        self.time_screens(now);
        // With the screens off a lock is shown without drawing, and is
        // said to be before the state is.
        self.confirm_lock();
        self.update_seal();
        self.space.refresh();
        self.popups.cleanup();
        self.publish_ipc(now);
        self.tell_holder();
        self.tell_relay();
    }

    // --- Configuration -------------------------------------------------------

    /// Reads the configuration again and applies it.
    pub fn reload_config(&mut self) {
        self.store.reload();
        self.apply_config(Vec::new());
    }

    fn apply_config(&mut self, mut problems: Vec<String>) {
        let config = Arc::clone(self.store.current());
        // Read again when the scheme may change, if it goes by the time of
        // day, as though a file had changed.
        let due = self.store.due();
        self.watcher
            .iter_mut()
            .for_each(|watcher| watcher.wake_at(due));
        self.switch.icons.set_theme(&config.general.icons.theme);
        self.light_the_night(&jiff::Zoned::now());
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
        self.tell_backdrops();
        // The pointer's theme may be another, by a file or by the scheme.
        self.autostarted.environment.1 = false;
    }

    // --- Actions -------------------------------------------------------------

    /// Goes to another workspace of the output the pointer is on, by `go`,
    /// which is given the one shown and says whether it went forwards; and
    /// slides in the one it went to.
    fn go_to_workspace(&mut self, go: impl FnOnce(&mut Desktop, usize) -> bool) {
        self.desktop.turn_to(self.pointer_position());
        let was = self.desktop.active_workspace();
        let forwards = go(&mut self.desktop, was);
        self.slide_in(was, forwards);
    }

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
                let to = bridge::workspace_index(*number);
                self.go_to_workspace(|desktop, was| {
                    desktop.switch_workspace(to);
                    desktop.active_workspace() > was
                });
            }
            Action::NewWorkspace => self.go_to_workspace(|desktop, _| {
                desktop.new_workspace();
                true
            }),
            Action::NextWorkspace => self.go_to_workspace(|desktop, _| {
                desktop.next_workspace();
                true
            }),
            Action::PreviousWorkspace => self.go_to_workspace(|desktop, _| {
                desktop.previous_workspace();
                false
            }),
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
            Action::About => {
                // Asked again, it goes; and it is not shown on a locked
                // session.
                let open = self.cards.about.asked.is_none() && !self.is_locked();
                self.cards.about.asked = open.then_some(self.pointer_location);
                // What the computer is, as it opens.
                if open {
                    let said = system::said(Path::new("/"), self.monitors());
                    self.cards.about.system = System::of(&said);
                    self.cards.about.copied = None;
                }
            }
            Action::Lock => self.start_locker(),
            Action::StayAwake => self.stay_awake(None),
            Action::StayAwakeFor(seconds) => self.stay_awake(Some(seconds.get())),
            Action::NightLight => self.switch_night_light(),
            Action::NextLayout => {
                let keyboard = self.keyboard.clone();
                keyboard.with_xkb_state(self, |mut xkb| xkb.cycle_next_layout());
            }
            Action::NextScreens => self.next_screens(),
            Action::ReloadConfig => self.reload_config(),
            Action::RestartCompositor => self.restart_compositor(),
            Action::Quit => self.quit(),
        }
    }

    /// Opens the root menu at the pointer.
    pub(crate) fn open_menu(&mut self) {
        let config = Arc::clone(self.store.current());
        let items = bridge::menu_items(&config.menu.items, self.toggled());
        self.open_menu_of(items);
    }

    /// What is on, of what the rows of a menu turn on and off: how the
    /// window with focus stands, and whether the session is kept awake.
    fn toggled(&self) -> bridge::Toggled {
        let focused = self.desktop.focused();
        let frame = self
            .desktop
            .frames()
            .find(|frame| Some(frame.window().id) == focused)
            .map(|frame| (frame.standing, frame.maximized));
        let (standing, maximized) = frame.unwrap_or_default();
        bridge::Toggled {
            standing,
            maximized,
            awake: self.presence.awake.is_some(),
        }
    }

    /// Opens the menu of the window that has focus, at the pointer: what
    /// can be done with it, each thing done as its key binding would do
    /// it. A right click on a titlebar opens it, having given that window
    /// focus.
    pub(crate) fn open_window_menu(&mut self) {
        // Each says what choosing it does, the same whichever way things
        // are; those that turn something on and off are ticked while it is.
        let on = self.toggled();
        let entry = |label: &str, action: Action| Item::Entry {
            label: label.to_owned(),
            mark: bridge::mark(&action, on),
            action,
        };
        let here = self.desktop.current_output();
        let places = || {
            let places = self.desktop.workspaces();
            places.filter(|place| place.output == here)
        };
        let desktops = places().filter(|place| place.desktop.is_some());
        let send = (1..).zip(desktops).map(|(number, place)| Item::Entry {
            label: self.workspace_name(&place),
            action: Action::MoveToWorkspace(WorkspaceNumber::clamped(number)),
            mark: Mark::Blank,
        });
        // A workspace made for windows takes another where it has room,
        // unless it is where this window is already.
        let own = self.desktop.focused();
        let joinable =
            |place: &Place| place.room && !own.is_some_and(|id| place.tiles.contains(&id));
        let join = places().filter(joinable).map(|place| Item::Entry {
            label: format!("Beside {}", self.workspace_name(&place)),
            action: Action::JoinWorkspace(place.id.get()),
            mark: Mark::Blank,
        });
        let send = send.chain(join).chain([
            Item::Separator,
            entry("New workspace", Action::MoveToNewWorkspace),
        ]);
        let items = vec![
            entry("Close", Action::Close),
            entry("Maximize", Action::Maximize),
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
                mark: Mark::Sign(Sign::Tile),
            },
            Item::Separator,
            entry("Shade", Action::Shade),
            entry("Keep above", Action::KeepAbove),
            entry("Show on every workspace", Action::Stick),
            Item::Separator,
            Item::Submenu {
                label: "Send to workspace".to_owned(),
                items: send.collect(),
                mark: Mark::Sign(Sign::Send),
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
        // Rows begin inside the menu's border and padding, as in CSS.
        let room = config.theme.style(Part::Menu, States::NONE).inset();
        let metrics = MenuMetrics {
            width: MENU_WIDTH,
            row: config.theme.style(Part::MenuItem, States::NONE).height,
            separator: MENU_SEPARATOR,
            above: room.top,
            below: room.bottom,
            overlap: MENU_OVERLAP,
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
        let program: &OsStr = command.program().as_ref();
        let named = omorfos_journal::name(program);
        // One given a role is kept by the relay as the user's.
        let how = (roles, false, Relaying::Users);
        self.launch(program, command.arguments(), how, Some(&named));
    }

    /// Starts the program the configuration names to lock the session,
    /// which is let lock by having been started for it. With none named
    /// there is nothing to lock with, which the user is told.
    pub(crate) fn start_locker(&mut self) {
        // Locked by hand, the session is not kept awake any longer.
        self.presence.awake = None;
        let config = Arc::clone(self.store.current());
        match &config.general.locker {
            Some(locker) => {
                // Kept by the relay as programs are, where there is one, so
                // that a lock outlives the compositor (ADR 61).
                let program: &OsStr = locker.program().as_ref();
                let named = omorfos_journal::name(program);
                let how = (Roles::LOCK, false, Relaying::Own);
                let started = self.launch(program, locker.arguments(), how, Some(&named));
                self.on_demand.locker.0 = started;
            }
            None => self.refused.say(NO_LOCKER),
        }
    }

    /// Starts the locker anew, ending the one a compositor before started,
    /// which the relay kept, if that has not locked the session again within
    /// [`RELOCKING`] of `now`: the session would stay black.
    fn await_kept_locker(&mut self, now: Instant) {
        let adopted = &self.autostarted.parts.adopted;
        let kept = self.on_demand.locker.0.filter(|locker| {
            let mut adopted = adopted.iter();
            adopted.any(|(adopted, _)| adopted == locker)
        });
        let Some(kept) = kept.filter(|_| self.is_held_by_none()) else {
            self.on_demand.relocking = None;
            return;
        };
        let since = *self.on_demand.relocking.get_or_insert(now);
        if now.saturating_duration_since(since) >= RELOCKING {
            let kept = Pid::from_raw(kept.cast_signed());
            let _ = kept.map(|kept| kill_process(kept, Signal::TERM));
            self.on_demand.relocking = None;
            self.start_locker();
        }
    }

    /// Starts the locker again if it ended, of those that `ended`, while the
    /// session is still locked, which a locker that crashed leaves it; at
    /// most three times in a minute, after which the screen stays black and
    /// locked, and the user is told how to unlock it from elsewhere.
    fn relock(&mut self, ended: &[u32], now: Instant) {
        let gone = self
            .on_demand
            .locker
            .0
            .take_if(|locker| ended.contains(locker));
        if gone.is_none() || !self.is_locked() {
            return;
        }
        if self.on_demand.locker.1.allow(now) {
            self.start_locker();
        } else {
            self.problems.push(
                "The locker keeps stopping with the session locked: from another virtual \
                 terminal, `omorfosctl action lock` starts it again"
                    .to_owned(),
            );
        }
    }

    /// Ends the session: asks the programs started in it to end first, so
    /// that they end as they would be asked to by any desktop, and not
    /// with the compositor gone from under them, which they say. With
    /// none, or asked again while they are waited for, at once.
    fn quit(&mut self) {
        let asked = self.on_demand.ending == Ending::Not;
        for process in self.programs() {
            let _ = kill_process(process, Signal::TERM);
        }
        let waiting = asked && self.programs().next().is_some();
        if asked {
            self.on_demand.ending = Ending::Asked;
        }
        self.running = waiting;
    }

    /// The programs started in the session that are asked to end before it
    /// does: not those that come with omorfos, which end when the
    /// compositor does, nor the locker, which would be taken for one that
    /// crashed.
    fn programs(&self) -> impl Iterator<Item = Pid> {
        let ours = self
            .components
            .iter()
            .filter_map(|component| component.process);
        let ours: Vec<u32> = ours.chain(self.on_demand.locker.0).collect();
        let theirs = self
            .children
            .iter()
            .filter(move |child| !ours.contains(&child.id()));
        theirs.map(Pid::from_child)
    }

    /// Ends the session once the programs asked to end have, or have been
    /// given long enough to.
    fn end(&mut self, now: Instant) {
        let by = match self.on_demand.ending {
            Ending::Not => return,
            Ending::Asked => now + ENDING,
            Ending::By(by) => by,
        };
        self.on_demand.ending = Ending::By(by);
        self.running &= by > now && self.programs().next().is_some();
    }

    /// Says where the launcher that comes with omorfos is: a name to look
    /// up in `PATH`, or a path. Until this is called there is none to
    /// open.
    pub fn set_launcher(&mut self, program: impl Into<PathBuf>) {
        self.on_demand.launcher.0 = Some(program.into());
    }

    /// Opens the launcher, unless one is open already.
    fn open_launcher(&mut self) {
        let (program, open) = &self.on_demand.launcher;
        let open = open.is_some_and(|open| self.children.iter().any(|child| child.id() == open));
        let program = program.clone().filter(|_| !open);
        if let Some(program) = program {
            // It has the keyboard, whatever had it, while it is open.
            let roles = Roles::OVERLAY;
            self.on_demand.launcher.1 = self.start(program.as_os_str(), &[], roles);
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
        let named = omorfos_journal::name(program);
        self.launch(
            program,
            arguments,
            (roles, false, Relaying::No),
            Some(&named),
        )
    }

    /// Starts a program as [`Compositor::start`] does, and, if it is
    /// `heard`, listens to what it says, as the watcher of what records
    /// says it. One to be `kept` by the relay as programs are, where there
    /// is one, has its connection handed to the relay, with its roles and
    /// whose it is, which hands it back.
    fn launch(
        &mut self,
        program: &OsStr,
        arguments: &[String],
        (roles, heard, kept): (Roles, bool, Relaying),
        journalled: Option<&str>,
    ) -> Option<u32> {
        let mut process = std::process::Command::new(program);
        if heard {
            process.stdout(Stdio::piped());
        }
        // What a program writes is kept in the journal under its name,
        // where there is one, and not in the compositor's own log.
        let socket = self.autostarted.journal.as_deref();
        let stream = socket.zip(journalled);
        let stream = stream.and_then(|(socket, named)| omorfos_journal::stream(socket, named));
        let both = stream.and_then(|out| out.try_clone().ok().map(|err| (out, err)));
        if let Some((out, err)) = both {
            process.stdout(out).stderr(err);
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
        let relayed = kept != Relaying::No && self.autostarted.parts.relay.is_some();
        let mut relaying = None;
        let handed = trusted.and_then(Result::ok).and_then(|(ours, theirs)| {
            if relayed {
                relaying = Some(ours);
                return Some(theirs);
            }
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
                if let Some(ours) = relaying {
                    self.relay_part(&ours, process, roles, kept == Relaying::Users);
                }
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

    /// Hands the relay `ours`, the connection of the part of the session
    /// just started as `process` with `roles`, to be kept as programs are,
    /// marked as the `users` where the configuration started it. A relay
    /// that cannot be told has gone, and the compositor steps down with it,
    /// for the next to start the part again.
    fn relay_part(&self, ours: &UnixStream, process: u32, roles: Roles, users: bool) {
        let mark = if users { USERS } else { 0 };
        let started = omorfos_ipc::relay::Started {
            pid: process,
            roles: roles.bits() | mark,
        };
        let hand = omorfos_ipc::relay::Hand::Started(started);
        let relay = self.autostarted.parts.relay.as_ref();
        let _ = relay.map(|relay| omorfos_ipc::relay::send(relay.as_fd(), &hand, &[ours.as_fd()]));
    }

    /// Says where the panel that comes with omorfos is: a name to look up
    /// in `PATH`, or a path. It is then started with the session, unless
    /// the configuration turns it off. Until this is called there is none
    /// to start.
    pub fn set_panel(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Panel, program.into());
    }

    /// What glass is to be blurred by on `renderer`: `made`, if it could be
    /// made and blurs a picture as the software blur does, within rounding.
    /// Else glass is drawn solid for the session, and the user is told why,
    /// as of `now`, once; that is returned, for the log.
    ///
    /// # Errors
    ///
    /// Returns what the user was told if `made` could not be made or does
    /// not blur as it should.
    pub fn frosting<R, P, F>(
        &mut self,
        renderer: &mut R,
        made: Result<F, impl std::fmt::Display>,
        now: Instant,
    ) -> Result<F, String>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        F: Frosting<R, P>,
    {
        let tried = made
            .map_err(|error| format!("could not be made ({error})"))
            .and_then(|mut frosting| glass::check(renderer, &mut frosting).map(|()| frosting));
        tried.map_err(|why| {
            let said = format!("Glass is drawn solid: the blur of this graphics card {why}");
            self.happened(&said, now);
            said
        })
    }

    /// How many times what is behind glass has been blurred since the
    /// compositor started.
    #[must_use]
    pub const fn blurs(&self) -> u64 {
        self.animation.glass.blurs()
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

    /// Has what the programs started in the session write kept in the
    /// journal that takes streams at `socket`, each under its name, and not
    /// in the compositor's own log. Until this is called, or where there is
    /// no journal there, they write where the compositor does. Its own
    /// programs, the panel and the rest, write there all the same.
    pub fn set_journal(&mut self, socket: impl Into<PathBuf>) {
        self.autostarted.journal = Some(socket.into());
    }

    /// Says where the program that asks for secrets is, polkit's agent for
    /// the session: a name to look up in `PATH`, or a path. It is then
    /// started with the session, with the role to ask in the sealed
    /// prompt, and nothing turns it off. Until this is called there is
    /// none to start.
    pub fn set_asker(&mut self, program: impl Into<PathBuf>) {
        self.set_component(Program::Asker, program.into());
    }

    /// Says where the compositor's own program is, for its new version to
    /// be taken up once it is installed, at a moment the session is not in
    /// use, where the relay keeps the programs through it; and for the user
    /// to be told it is used once they log in again where nothing does.
    pub fn set_own(&mut self, program: impl Into<PathBuf>) {
        self.updates.own = Some(program.into());
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
            newer: false,
            after: None,
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
            // The keeper, relayed, waits for the relay to have handed over
            // what it kept, which may be the keeper.
            let waits = component.kind.kept()
                && self.autostarted.parts.relay.is_some()
                && !self.autostarted.parts.kept;
            let running = component.process.is_some();
            if component.kind.wanted(&config.general) && !waits && !running {
                let program = component.program.as_os_str();
                let heard = component.kind == Program::Recorder;
                let how = (component.kind.roles(), heard, component.kind.relaying());
                component.process = self.launch(program, &[], how, None);
            }
        }
        self.components = components;
        // From now on, a new version of any of them is taken up.
        let programs = self
            .components
            .iter()
            .map(|component| component.program.as_path());
        let own = self.updates.own.as_deref();
        self.updates.versions.follow(programs.chain(own));
        // A compositor started again in the same login leaves alone what
        // the first started, which may well be running still.
        if self.started_again() {
            return;
        }
        // Each in a scope of its own, where there is what makes one, so
        // that it ends with the session: a program that is not the
        // compositor's client is told of nothing else that does.
        let scoper = self.autostarted.scoper.clone();
        for command in &config.general.autostart {
            let run = &command.run;
            let (program, arguments) = scoped(scoper.as_deref(), run.program(), run.arguments());
            // Known by its own name, whatever starts it.
            let named = omorfos_journal::name(run.program().as_ref());
            // One given a role is kept by the relay as the user's.
            let roles = (Roles::named(&command.may), false, Relaying::Users);
            self.launch(program.as_ref(), &arguments, roles, Some(&named));
        }
        // Then what the desktop entries in `~/.config/autostart` and the
        // system's say is to start, which the program that watches over the
        // session reads and starts as ordinary programs, once.
        self.ask_session(&["autostart"]);
    }

    /// Asks the program that watches over the session to do `request`, as
    /// a key or the menu does. Without one there is nobody to ask.
    pub(crate) fn ask_session(&mut self, request: &[&str]) {
        let session = self
            .components
            .iter()
            .find(|component| component.kind == Program::Session);
        let session = session.map(|component| component.program.clone());
        if let Some(session) = session {
            let request: Vec<String> = request.iter().map(|word| (*word).to_owned()).collect();
            self.launch(
                session.as_os_str(),
                &request,
                (Roles::NONE, false, Relaying::No),
                None,
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
                let how = (component.kind.roles(), heard, component.kind.relaying());
                component.process = self.launch(program, &[], how, None);
            } else {
                self.problems.push(format!(
                    "`{}` keeps stopping, and will not be started again until you log in",
                    component.program.display()
                ));
            }
        }
        self.components = components;
    }

    /// Takes up the compositor's own new version, if one has been heard of,
    /// by stepping down to be started again as it: where the relay keeps
    /// the programs through it, once it has handed over all it kept, at a
    /// moment the session is not in use, or regardless once it has waited
    /// as long as the configuration lets it; but never while a program
    /// written for X11, which would close, has a window, nor while the
    /// screens are off, which the next would turn on. Where nothing
    /// keeps the programs they would close, and the user is told it is used
    /// once they log in again (ADR 60).
    fn take_up_own(&mut self, now: Instant) {
        self.say_if_updated(now);
        let updates = &mut self.updates;
        updates.next = None;
        let Some(since) = updates.newer else {
            return;
        };
        let parts = &self.autostarted.parts;
        if parts.relay.is_none() {
            if !std::mem::replace(&mut updates.told, true) {
                self.problems.push(LOG_IN_AGAIN.to_owned());
            }
            return;
        }
        if !parts.kept || self.stepping_down() {
            return;
        }
        let recorded = [
            Recorded::Microphone,
            Recorded::Sound,
            Recorded::Camera,
            Recorded::Screen,
        ];
        let in_use = InUse {
            fullscreen: self.desktop.frames().any(|frame| frame.fullscreen),
            recording: self.capture.recording.any(&recorded) || self.capture.sharing(),
            playing: self.presence.kept_awake
                || self.kept_from().sleep
                || self.capture.recording.playing,
            open: self.sealed().is_some() || self.on_demand.launcher.1.is_some(),
            input: self.last_active(),
            x11: (self.windows.iter()).any(|mapped| matches!(mapped.role, Role::X11(_))),
            dark: !self.screens_on(),
        };
        // The user back, it waits as long again, so as not to be taken up
        // regardless at the first key.
        let since = if in_use.dark { now } else { since };
        self.updates.newer = Some(since);
        let wait = self.config().general.update.wait.get();
        let wait = (wait > 0).then(|| Duration::from_secs(u64::from(wait)));
        self.updates.next = in_use.next(since, wait, now);
        match in_use.own(since, wait, now) {
            Own::Waits => {}
            Own::WaitsForX11 => {
                if !std::mem::replace(&mut self.updates.told, true) {
                    self.problems.push(WAITS_FOR_X11.to_owned());
                }
            }
            Own::TakenUp { regardless } => {
                self.updates.newer = None;
                self.step_down(regardless);
            }
        }
    }

    /// Runs the compositor's own new version, installed at `own` and heard
    /// of `now`, to see that it starts at all before it is taken up: asked
    /// only for its version, it ends at once, touching nothing. One that
    /// cannot even be started is said not to start at once.
    fn check_own(&mut self, own: &Path, now: Instant) {
        // What was checked before, replaced since, is of no use; it is
        // collected as any child is.
        if let Some((mut before, _)) = self.updates.checking.take() {
            let _ = before.kill();
            self.children.push(before);
        }
        let checking = std::process::Command::new(own)
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match checking {
            Ok(child) => {
                self.wake_when_exited(&child);
                self.updates.checking = Some((child, now));
            }
            Err(error) => self.not_starting(own, &error.to_string()),
        }
    }

    /// Takes in how the check of the compositor's own new version went,
    /// once it has ended: one that started is to be taken up, as from when
    /// it was heard of; one that did not is said not to, and the version
    /// running stays until another is installed.
    fn checked_own(&mut self) {
        let checking = self.updates.checking.as_mut();
        let ended = checking.and_then(|(child, _)| child.try_wait().ok().flatten());
        let Some(status) = ended else {
            return;
        };
        let heard = self.updates.checking.take().map(|(_, heard)| heard);
        if status.success() {
            self.updates.newer = self.updates.newer.or(heard);
        } else {
            let own = self.updates.own.clone().unwrap_or_default();
            self.not_starting(&own, &status.to_string());
        }
    }

    /// Tells the user that the compositor's own new version, at `own`, does
    /// not start, for `why`, and is not taken up.
    fn not_starting(&mut self, own: &Path, why: &str) {
        self.problems.push(format!(
            "the new version of omorfos, `{}`, does not start ({why}), so the one running \
             stays until another is installed",
            own.display()
        ));
    }

    /// Takes up the new versions of the programs that come with omorfos
    /// that have been installed, by `now`, of those that run for the whole
    /// session: each is asked to end, and its new version started once it
    /// has, as it holds names the new one is to have. The keeper of the
    /// clipboard is the other way round, for the new one to take what the
    /// old keeps; and what shares a screen, or asks for a secret, is left
    /// until it no longer does. `ended` are the processes that have just
    /// ended.
    ///
    /// The compositor's own new version is noted, from when it was first
    /// heard of, for [`Compositor::take_up_own`].
    fn take_up(&mut self, ended: &[u32], now: Instant) {
        self.updates.versions.hear(now);
        let settled = self.updates.versions.settled(now);
        let own = self.updates.own.clone();
        if let Some(own) = own.filter(|own| settled.contains(own)) {
            self.check_own(&own, now);
        }
        self.checked_own();
        let retired = self.updates.retiring.extract_if(.., |(_, by)| *by <= now);
        let retired = retired.filter_map(|(process, _)| Pid::from_raw(process.cast_signed()));
        for process in retired {
            // One that has gone already is nothing to ask.
            let _ = kill_process(process, Signal::TERM);
        }
        let config = Arc::clone(self.store.current());
        let started = self.autostarted.done;
        let sharing = self.capture.sharing();
        let asking = self.sealed().is_some();
        let mut components = std::mem::take(&mut self.components);
        for component in &mut components {
            component.newer |= settled.contains(&component.program);
            let heard = component.kind == Program::Recorder;
            let how = (component.kind.roles(), heard, component.kind.relaying());
            let program = component.program.clone();
            if component
                .after
                .take_if(|after| ended.contains(after))
                .is_some()
            {
                component.process = self.launch(program.as_os_str(), &[], how, None);
            }
            let busy = match component.kind {
                Program::Portal => sharing,
                Program::Asker => asking,
                _ => false,
            };
            let due = component.newer && component.after.is_none() && started && !busy;
            if !due || !component.kind.wanted(&config.general) {
                continue;
            }
            component.newer = false;
            // A new version is not held to how often the one before stopped.
            component.restarts = Restarts::default();
            match component.process.take() {
                Some(old) if component.kind == Program::Keeper => {
                    self.updates.retiring.push((old, now + HANDOVER));
                    component.process = self.launch(program.as_os_str(), &[], how, None);
                }
                Some(old) => {
                    let old_one = Pid::from_raw(old.cast_signed());
                    let _ = old_one.map(|old_one| kill_process(old_one, Signal::TERM));
                    component.after = Some(old);
                }
                None => component.process = self.launch(program.as_os_str(), &[], how, None),
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
        let config = self.store.current();
        let cursor = &config.general.cursor;
        let display = self.socket_name.clone();
        let socket = self.ipc_path().map(|path| path.as_os_str().to_owned());
        let fixed = [
            ("XDG_SESSION_TYPE", OsString::from("wayland")),
            ("XDG_CURRENT_DESKTOP", OsString::from(DESKTOP_NAME)),
            (
                "XCURSOR_THEME",
                OsString::from(&*cursor.theme_in(config.scheme)),
            ),
            (
                "XCURSOR_SIZE",
                OsString::from(cursor.size.get().to_string()),
            ),
            // Qt connects again, to the same socket, when the compositor
            // is started again under the holder (ADR 60), but only if told.
            ("QT_WAYLAND_RECONNECT", OsString::from("1")),
        ];
        let located = [
            ("WAYLAND_DISPLAY", display),
            ("OMORFOS_SOCKET", socket),
            ("DISPLAY", self.x11.display()),
            // Where the compositor found cursor themes, which may be where
            // programs that read them would not look, as /usr/local.
            ("XCURSOR_PATH", self.themes.search_path()),
        ];
        let located = located
            .into_iter()
            .filter_map(|(name, value)| value.map(|value| (name, value)));
        located.chain(fixed).collect()
    }

    /// The session's environment, if D-Bus and systemd are to be told it:
    /// the first time, and whenever it has changed since, as when the X
    /// server has started or the pointer's theme is another. It is then
    /// taken to have been told.
    pub fn environment_to_tell(&mut self) -> Option<Vec<(&'static str, OsString)>> {
        if std::mem::replace(&mut self.autostarted.environment.1, true) {
            return None;
        }
        let now = self.session_environment();
        let told = &mut self.autostarted.environment.0;
        (told.as_ref() != Some(&now)).then(|| told.insert(now).clone())
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
        let mut frames: Vec<FrameInfo> = self
            .desktop
            .frames()
            .map(|frame| FrameInfo::of(&frame, scale_for(frame.geometry), over))
            .collect();
        let config = Arc::clone(self.store.current());
        let mut previous = std::mem::take(&mut self.scene);
        let mut veils = Veils::kept(&mut previous, &config.theme);
        // What slides into view with the workspace it is on was there all
        // along, and does not come in afresh.
        let sliding = self.desktop.slid() != 0;
        let changes = config.theme.style(Part::Window, States::NONE).transition;
        let (moves, held) = self.moving(changes, sliding);
        let text = &mut self.text;
        let motion = &mut self.animation.motion;
        motion.begin(self.animation.clock);
        let shades = self.animation.shades.begun();

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
                .iter_mut()
                .enumerate()
                .find(|(_, frame)| frame.shown == *id);
            let placed = frame.as_ref().and_then(|(_, frame)| frame.placed());
            role.arrange(Some(*id) == focused, placed, window.geometry().size);
            match frame {
                Some((order, frame)) => {
                    let before = previous.iter_mut().find(|shown| shown.id == *id);
                    let shows = window.geometry().size;
                    let clock = self.animation.clock;
                    let (way, sizes) = frame.travel(before.as_deref(), shows, (clock, moves));
                    let noted = (&self.animation.drawn, surface);
                    let (stretch, giving) =
                        stretching(role, sizes, before, noted, (changes, clock, held));
                    let chrome = frame.decoration.map(|decoration| {
                        let cached = painted(&mut previous, &frame.tabs);
                        let tools = (&mut *text, &mut *motion, &mut *shades);
                        Chrome::paint(tools, &config.theme, cached, frame, decoration)
                    });
                    let origin = (frame.client.origin.x, frame.client.origin.y).into();
                    let private = self.desktop.private(*id);
                    let blurred = self.desktop.blurred(*id);
                    let seen = presence(motion, &config.theme, (&frame.tabs, *id), sliding);
                    scene.push((
                        order,
                        Shown {
                            id: *id,
                            seen,
                            shaded: frame.standing.shaded,
                            over: frame.home != Home::Desktop,
                            window: window.clone(),
                            surface: surface.clone(),
                            outer: frame.geometry,
                            origin,
                            chrome,
                            veil: private.then(|| veils.veil(*id, frame.geometry.size, blurred)),
                            private,
                            on: frame.on,
                            stretch,
                            giving,
                            way,
                        },
                    ));
                }
                None => self.space.unmap_elem(window),
            }
        }
        // Mapping raises an element, so mapping bottom to top sets the order.
        scene.sort_by_key(|(order, _)| *order);
        self.scene = scene.into_iter().map(|(_, shown)| shown).collect();
        self.lay_lines(moves);
        for shown in &self.scene {
            self.space
                .map_element(shown.window.clone(), shown.origin, false);
        }
        self.animation.leave(previous, &self.scene);
        self.portray(&config.theme);

        self.fade_layers(&config.theme);
        self.repaint_overlays();
        self.repoint();
        self.note_mark();
        self.update_pointer();
        self.announce_scale();
        self.update_idle_inhibition();
        self.sync_keyboard_focus(focused);
    }

    /// How the frames whose places have changed since the last sync got
    /// there: whether they were sent there, to be drawn easing there,
    /// where windows change over a time, `changes`, motion is not to be
    /// reduced, and nothing slides, what slides with its workspace being
    /// carried, not sent; and whether one is held by hand, by its edge or
    /// by the line between it and another.
    fn moving(&mut self, changes: omorfos_config::Transition, sliding: bool) -> (bool, bool) {
        let sent = self.desktop.sent() != self.animation.sent;
        self.animation.sent = self.desktop.sent();
        let reduced = self.config().general.appearance.reduce_motion;
        let held = matches!(self.drag, Some(Drag::Resize { .. } | Drag::Line { .. }));
        let eased = sent && !sliding && !changes.lasts().is_zero() && !reduced;
        (eased, held)
    }

    /// Paints the frames the windows pictures are taken of by themselves
    /// are drawn in, as each would be shown wherever it is, keeping what
    /// was painted while it would look the same. Once the scene is laid
    /// out, with the shadows in use begun.
    fn portray(&mut self, theme: &Theme) {
        let over = self.hover.button;
        let pictured: Vec<(FrameInfo, Window, WlSurface)> = self
            .capture
            .windows()
            .filter_map(|window| {
                let mapped = self.windows.iter().find(|mapped| mapped.id == window);
                mapped.zip(self.desktop.frame_of(window))
            })
            .map(|(mapped, frame)| {
                let scale = self.portrait_head(frame.geometry).map(|head| head.scale);
                let frame = FrameInfo::of(&frame, scale.unwrap_or(Scale::ONE), over);
                (frame, mapped.window.clone(), mapped.surface.clone())
            })
            .collect();
        let mut kept = std::mem::take(&mut self.animation.portraits);
        let animation = &mut self.animation;
        let (text, shades) = (&mut self.text, &mut animation.shades);
        let (portrayal, portraits) = (&mut animation.portrayal, &mut animation.portraits);
        portrayal.begin(animation.clock);
        for (frame, window, surface) in pictured {
            let id = frame.shown;
            // Once, however many programs take pictures of it.
            if portraits.iter().any(|portrait| portrait.id == id) {
                continue;
            }
            let cached = kept.iter().position(|portrait| portrait.id == id);
            let cached = cached.and_then(|index| kept.swap_remove(index).chrome);
            let chrome = frame.decoration.map(|decoration| {
                let tools = (&mut *text, &mut *portrayal, &mut *shades);
                Chrome::paint(tools, theme, cached, &frame, decoration)
            });
            portraits.push(Shown {
                id,
                over: false,
                window,
                surface,
                outer: frame.geometry,
                origin: (frame.client.origin.x, frame.client.origin.y).into(),
                // Nothing fades into a picture.
                seen: Seen::WHOLE,
                shaded: frame.standing.shaded,
                chrome,
                veil: None,
                private: false,
                on: frame.on,
                stretch: None,
                giving: None,
                way: Way::at(frame.geometry),
            });
        }
        portrayal.end();
    }

    /// Works out how much there is to see of each panel that a shell
    /// component has put up or taken down: one that is named after a part
    /// of the theme comes in, and goes, over the time that part's colours
    /// take to change, but for a question. A panel of any other program is
    /// there at once and gone at once.
    fn fade_layers(&mut self, theme: &Theme) {
        let animation = &mut self.animation;
        std::mem::swap(&mut animation.layers, &mut animation.before);
        animation.layers.clear();
        for (index, head) in self.heads.iter().enumerate() {
            let layers = layer_map_for_output(&head.output);
            let height = head.area().size.h;
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
                let present = part.map(|part| {
                    let style = theme.style(part, States::NONE);
                    let had = animation
                        .before
                        .iter_mut()
                        .find(|was| was.surface == *surface);
                    let had = had.and_then(|was| was.shadow.take());
                    let placed = layers.layer_geometry(layer).unwrap_or_default();
                    let off = self
                        .hover
                        .reveal
                        .off(index, (layer.layer(), placed), height);
                    let at = layer_origin(&layers, layer) + head.area().loc + Point::from((0, off));
                    let size = placed.size;
                    let area = (at.x, at.y, size.w.unsigned_abs(), size.h.unsigned_abs());
                    let shadow = animation.shades.cast(&style, head.scale, area, had);
                    let seen = if fades(part) {
                        animation.motion.presence(theme, part, key)
                    } else {
                        Seen::WHOLE
                    };
                    Present {
                        surface: surface.clone(),
                        part,
                        layer: layer.layer(),
                        seen,
                        shadow,
                        place: Rectangle::new(at, size),
                    }
                });
                animation.layers.extend(present);
            }
        }
        let mut before = std::mem::take(&mut animation.before);
        for was in before.drain(..) {
            let mut now = animation.layers.iter();
            let still = now.any(|now| now.surface == was.surface);
            let pieces = (!still && fades(was.part)).then(|| animation.noted(&was.surface));
            let gone = pieces.flatten().map(|pieces| Gone {
                part: was.part,
                since: animation.clock,
                from: was.seen,
                seen: was.seen,
                level: Level::Layer(was.layer),
                what: What::Surface(None, pieces, was.shadow, Some(was.place)),
                private: false,
            });
            animation.gone.extend(gone);
        }
        animation.before = before;
    }

    /// Brings "About" up to date: shown in the middle of the screen it was
    /// asked for on, or of the first if that has gone, while it is to be,
    /// and painted again only if how it would look has changed.
    fn repaint_about(&mut self, config: &Config) {
        let (theme, scheme) = (&config.theme, config.scheme);
        let asked = self.cards.about.asked;
        let screen = asked.and_then(|at| self.head_at(at).or(self.primary_head()));
        let within = screen.map(Head::area).unwrap_or_default();
        let scale = screen.map_or(Scale::ONE, |screen| screen.scale);
        let wanted = screen.is_some();
        // One that is no longer wanted fades out as it was; one painted
        // again is scrolled as far as it was.
        let had = self.cards.about.shown.take();
        let scrolled = had.as_ref().map_or(0.0, |about| about.scrolled);
        let pointer = self.pointer_position();
        let under = had.as_ref().and_then(|about| about.button_at(pointer));
        let hovered = under == Some(AboutButton::Close);
        let (had, closed) = if wanted { (had, None) } else { (None, had) };
        let closed = closed.map(|about| {
            self.animation
                .closing(Part::About, What::About(Box::new(about)))
        });
        self.animation.gone.extend(closed);
        let current = had.filter(|about| {
            about.theme == *theme
                && about.scheme == scheme
                && about.within == within
                && about.scale == scale
                && about.hovered == hovered
        });
        self.cards.about.shown = match (current, wanted) {
            (Some(about), _) => Some(about),
            (None, true) => {
                let tools = (&mut self.text, &mut self.animation.shades);
                let what = (theme, scheme, &self.cards.about.system);
                let mut about = About::paint(tools, what, within, (scale, hovered));
                about.scroll(scrolled);
                Some(about)
            }
            (None, false) => None,
        };
        // Its button that copies: under the pointer, or, for a while once
        // it has copied, saying so, with a tick drawn on it.
        let clock = self.animation.clock;
        let since = self.cards.about.copied.map(|at| clock.saturating_sub(at));
        let since = since.filter(|since| *since < COPIED_FOR);
        self.cards.about.copied = self.cards.about.copied.filter(|_| since.is_some());
        let tick = since.map(|since| (since.as_secs_f64() / TICK_DRAWN.as_secs_f64()).min(1.0));
        let states = States::NONE
            .when(State::Hovered, under == Some(AboutButton::Copy))
            .when(State::Active, tick.is_some());
        if let Some(about) = self.cards.about.shown.as_mut() {
            let style = self
                .animation
                .motion
                .style(theme, Part::AboutButton, 0, states);
            let font = theme.font(Part::AboutButton, states);
            about.dress(&mut self.text, (style, font), tick);
        }
        let about = self.cards.about.shown.as_ref();
        let share = about.map(|_| self.animation.motion.presence(theme, Part::About, 0));
        self.animation.about_seen = share.unwrap_or(Seen::WHOLE);
    }

    /// When "About" is to be drawn again of itself: a frame on, while the
    /// tick on its button that copies is being drawn, then when the button
    /// is to be as it was.
    fn about_due(&self) -> Option<Instant> {
        let at = self.cards.about.copied?;
        let clock = self.animation.clock;
        let next = if clock.saturating_sub(at) < TICK_DRAWN {
            clock + Motion::FRAME
        } else {
            at + COPIED_FOR
        };
        self.animation.epoch.map(|epoch| epoch + next)
    }

    /// Puts what the computer is, as "About" shows it, on the clipboard as
    /// text, for any program to paste: those written for X11 are told of
    /// it as well, since the compositor setting it is not a program doing
    /// so, which is what tells them otherwise.
    pub(crate) fn copy_about(&mut self) {
        let text = self.cards.about.system.text(env!("CARGO_PKG_VERSION"));
        let kinds: Vec<String> = handlers::TEXT.map(str::to_owned).into();
        let offer = Offer::Text(text.into());
        set_data_device_selection(&self.display, &self.seat, kinds.clone(), offer);
        // Its button says so for a while.
        self.cards.about.copied = Some(self.animation.clock);
        self.offer_to_x11(SelectionTarget::Clipboard, Some(kinds));
    }

    /// The screens that stand in the space, as "About" tells of them.
    fn monitors(&self) -> Vec<Monitor> {
        let monitor = |head: &Head| {
            let pixels = head.pixels();
            let refresh = head.output.current_mode().map_or(0, |mode| mode.refresh);
            Monitor {
                name: head.output.name(),
                pixels: (pixels.w, pixels.h),
                refresh,
                scale: head.scale.factor(),
            }
        };
        self.heads.iter().map(monitor).collect()
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
            self.cards.dismissed.clear();
        }
        // What is wrong, and what has just happened, is shown here only if
        // no program is showing it: the notifications that come with
        // omorfos do, among what other programs have to say, where neither
        // covers the other.
        let wanted = !said.is_empty() && *said != *self.cards.dismissed && !shown_elsewhere;
        // The notice is on the output that comes first, which is where a
        // session with one screen has it.
        let first = self.primary_head();
        let within = first.map(Head::area).unwrap_or_default();
        let scale = first.map_or(Scale::ONE, |head| head.scale);
        // One that is no longer wanted fades out as it was.
        let had = self.cards.notice.take();
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
        self.cards.notice = match (current, wanted) {
            (Some(notice), _) => Some(notice),
            (None, true) => {
                let tools = (&mut self.text, &mut self.animation.shades);
                Some(Notice::paint(tools, theme, &said, within, scale))
            }
            (None, false) => None,
        };
        let notice = self.cards.notice.as_ref();
        let share = notice.map(|_| self.animation.motion.presence(theme, Part::Notice, 0));
        self.animation.notice_seen = share.unwrap_or(Seen::WHOLE);
        self.repaint_about(&config);

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
        let unchanged = had.as_ref().is_some_and(|chrome| {
            menu == Some(&chrome.menu)
                && Some(&chrome.looks) == looks.as_ref()
                && chrome.theme == *theme
                && chrome.scale == scale
        });
        self.menu_chrome = match (had, menu.zip(looks)) {
            (Some(chrome), _) if unchanged => Some(chrome),
            // Painted anew from what it was, whose shadows it keeps.
            (had, Some((menu, looks))) => {
                let tools = (
                    &mut self.text,
                    &mut self.animation.shades,
                    &mut self.switch.icons,
                );
                Some(MenuChrome::paint(tools, theme, (menu, looks), scale, had))
            }
            (_, None) => None,
        };
        let share = menu.map(|_| self.animation.motion.presence(theme, Part::Menu, 0));
        self.animation.menu_seen = share.unwrap_or(Seen::WHOLE);
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
    /// it is; and what is behind glass, to be blurred by `frosting`. Glass
    /// is solid without one.
    ///
    /// A frame that drew something is counted, with how long it took and
    /// how many blurs it made, for the socket to tell.
    ///
    /// # Errors
    ///
    /// Returns an error if the renderer fails.
    pub fn render<R, P>(
        &mut self,
        output: &Output,
        renderer: &mut R,
        framebuffer: &mut R::Framebuffer<'_>,
        tracking: (&mut OutputDamageTracker, usize),
        frosting: Option<&mut (dyn Frosting<R, P> + '_)>,
    ) -> Result<Drawn, RenderError<R::Error>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Bind<Dmabuf> + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let (started, blurs) = (Instant::now(), self.blurs());
        let drawn = self.render_frame(output, renderer, framebuffer, tracking, frosting);
        let made = self.blurs().saturating_sub(blurs);
        let counted = drawn.as_ref().ok().filter(|drawn| drawn.damage.is_some());
        if counted.is_some() {
            self.animation
                .frames
                .drew(&output.name(), started.elapsed(), made);
        }
        drawn
    }

    /// What [`Compositor::render`] draws.
    fn render_frame<R, P>(
        &mut self,
        output: &Output,
        renderer: &mut R,
        framebuffer: &mut R::Framebuffer<'_>,
        (tracker, age): (&mut OutputDamageTracker, usize),
        mut frosting: Option<&mut (dyn Frosting<R, P> + '_)>,
    ) -> Result<Drawn, RenderError<R::Error>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Bind<Dmabuf> + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        if let Some(nothing) = self.hidden(output) {
            return Ok(nothing);
        }
        if self.is_dark(output) {
            // Nothing is drawn on a screen that is off, by a backend that can
            // turn it off; one that cannot is shown black.
            let none: &[Element<R>] = &[];
            let drawn = tracker.render_output(renderer, framebuffer, age, none, BLACK);
            return self.drawn(output, output, drawn, Vec::new());
        }
        let mirror = self
            .aside
            .mirrors
            .iter()
            .position(|mirror| mirror.head.output == *output);
        if let Some(mirror) = mirror {
            let target = (framebuffer, tracker, age);
            return self.render_mirror::<R, P>(mirror, renderer, target, frosting);
        }
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
            return self.drawn(output, output, drawn, Vec::new());
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
        let behind = color(desktop.ground());
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
                    let frosting = frosting.as_deref_mut();
                    let scene = self.scene::<R, P>(renderer, output, (view, shown), frosting);
                    (scene, behind)
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
            self.draw_on_card(renderer, on_card, (&elements, behind), scale);
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
        let elements = self.scene::<R, P>(renderer, output, (view, shown), frosting);
        let drawn = tracker.render_output(renderer, framebuffer, age, &elements, behind);
        self.drawn(output, output, drawn, before)
    }

    /// Everything `output` shows, with or without what `shown` names, from
    /// the top down, and the glass under each part the theme makes glass,
    /// blurred by `frosting`.
    fn scene<R, P>(
        &mut self,
        renderer: &mut R,
        output: &Output,
        (view, shown): (View, Extras),
        frosting: Option<&mut (dyn Frosting<R, P> + '_)>,
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let scene = self.elements(renderer, output, view, shown);
        let desktop = self.config().theme.style(Part::Desktop, States::NONE);
        let behind = color(desktop.ground());
        let size = self.head(output).map(Head::upright).unwrap_or_default();
        let name = output.name();
        let screen = (name.as_str(), size, view.scale.factor());
        let solid = self.config().general.appearance.reduce_transparency;
        let glass = &mut self.animation.glass;
        glass.frost::<R, P>(renderer, screen, scene, (behind, solid), frosting)
    }

    /// What drawing `output` came to, with what was drawn before it in the
    /// same turn, and who is to be told when it is shown. `shows` is the
    /// output whose surfaces were drawn, which is another for a mirror.
    fn drawn<E>(
        &self,
        output: &Output,
        shows: &Output,
        drawn: Result<RenderOutputResult<'_>, E>,
        before: Vec<Rectangle<i32, Physical>>,
    ) -> Result<Drawn, E> {
        drawn.map(|drawn| {
            let feedback = self.feedback(output, shows, &drawn.states);
            Drawn::of(drawn, before, feedback)
        })
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
            let pid = self.process(&client);
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
                let style = theme.style(Part::Tooltip, States::NONE);
                let area = (left, top, width, height);
                let shadow = self.animation.shades.cast(&style, scale, area, None);
                MarkNote {
                    shadow,
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

    /// Draws `elements` over `behind` straight into each buffer of the
    /// graphics card in `on_card`, a picture at `scale` that is due, and
    /// gives it to its program, or says it could not be drawn.
    fn draw_on_card<R>(
        &mut self,
        renderer: &mut R,
        on_card: Vec<(Due, Dmabuf)>,
        (elements, behind): (&[Element<R>], Color32F),
        scale: f64,
    ) where
        R: Renderer + ImportAll + ImportMem + Bind<Dmabuf>,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        for (due, mut buffer) in on_card {
            let at = (due.size(), scale);
            let drawn = gpu::draw(renderer, &mut buffer, at, elements, behind);
            self.capture.give(due, drawn, self.animation.clock);
        }
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
    /// picture of it is taken: in its frame, as it would be shown, with the
    /// shadow it casts, and the pointer over it if `pointer`. A blank, if a
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
        // where all of the window does, its frame and its shadow too.
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
        // All of it, as it would be shown: its frame, what it draws, the
        // shadow it may draw round what it says is the window included,
        // and the shadow it casts; from the corner of all that.
        let portraits = self.animation.portraits.iter();
        let portrait = portraits.filter(|portrait| portrait.id == window);
        let portrayed = portrait.filter_map(|portrait| {
            let area = self.portrayed(window).map(|(area, _)| area);
            area.map(|area| (portrait, area))
        });
        for (portrait, area) in portrayed {
            let view = View {
                origin: area.loc,
                scale,
            };
            let chrome = portrait.chrome.as_ref();
            if let Some(chrome) = chrome {
                // What is behind it is not in a picture of it alone, nor is
                // its titlebar's glass.
                let drawn = (&mut elements, &mut Vec::new());
                Self::chrome_elements(renderer, view, chrome, 1.0, drawn);
            }
            if !portrait.shaded {
                self.window_elements(renderer, view, (portrait, false, false), &mut elements);
            }
            let shadow = chrome.and_then(|chrome| chrome.shadow.as_ref());
            if let Some(shadow) = shadow {
                shadow.elements(renderer, 1.0, view, &mut elements);
            }
        }
        elements
    }

    /// Draws on the output that is mirror `at` what the output it mirrors
    /// shows: drawn as for that one, made as large as fits and put in the
    /// middle, with black either side where the two are not of a shape.
    fn render_mirror<R, P>(
        &mut self,
        at: usize,
        renderer: &mut R,
        (framebuffer, tracker, age): (&mut R::Framebuffer<'_>, &mut OutputDamageTracker, usize),
        frosting: Option<&mut (dyn Frosting<R, P> + '_)>,
    ) -> Result<Drawn, RenderError<R::Error>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let source = self.aside.mirrors[at].source.clone();
        let room = self.aside.mirrors[at].head.upright();
        let head = self.head(&source);
        let view = View {
            origin: head.map(|head| head.area().loc).unwrap_or_default(),
            scale: head.map_or(Scale::ONE, |head| head.scale),
        };
        let picture = head.map(Head::upright).unwrap_or_default();
        let (zoom, shown) = fitted(picture, room);
        let mirror = &mut self.aside.mirrors[at];
        let mut elements: Vec<Mirrored<R>> = Vec::new();
        for (buffer, bar) in mirror.bars.iter_mut().zip(bars(shown, room)) {
            buffer.resize((bar.size.w, bar.size.h));
            let bar =
                SolidColorRenderElement::from_buffer(buffer, bar.loc, 1.0, 1.0, Kind::Unspecified);
            elements.push(Mirrored::Bar(bar));
        }
        let locked = self.is_locked();
        let (drawn, behind) = if locked {
            self.aside.mirrors[at].head.blanked.set(true);
            (self.locked_elements(renderer, &source, view), BLACK)
        } else {
            let shown = Extras {
                pointer: true,
                mark: true,
            };
            let desktop = self.config().theme.style(Part::Desktop, States::NONE);
            (
                self.scene::<R, P>(renderer, &source, (view, shown), frosting),
                color(desktop.ground()),
            )
        };
        elements.extend(drawn.into_iter().map(|element| {
            let made = RescaleRenderElement::from_element(element, (0, 0).into(), zoom);
            Mirrored::Shown(RelocateRenderElement::from_element(
                made,
                shown.loc,
                Relocate::Relative,
            ))
        }));
        let drawn = tracker.render_output(renderer, framebuffer, age, &elements, behind);
        let output = self.aside.mirrors[at].head.output.clone();
        self.drawn(&output, &source, drawn, Vec::new())
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

    /// While a program takes pictures of `output`, the mark that says so,
    /// over everything, the pointer too: nothing hides it; over that, what
    /// it says while the pointer rests on it, with its glass and its
    /// shadow; and a line round the screen if it is shared.
    fn mark_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let signs = self.signs(output);
        if !signs.any() {
            return;
        }
        // Over everything, the pointer too: nothing hides it; and over
        // that, what it says when the pointer rests on it.
        let tooltip = self
            .hover
            .mark
            .as_ref()
            .filter(|note| note.output == *output);
        let mut glass = None;
        if let Some(note) = tooltip {
            let at = Point::<i32, Logical>::from((note.area.origin.x, note.area.origin.y));
            elements.extend(note.pixels.element(renderer, at.to_f64(), view, 1.0));
            // Its glass, if the theme makes it glass, is under it.
            let style = self.config().theme.style(Part::Tooltip, States::NONE);
            let size = Size::from((
                note.area.size.width.cast_signed(),
                note.area.size.height.cast_signed(),
            ));
            let place = (Rectangle::new(at, size), view);
            let key = glass::key(&"the mark's tooltip");
            glass = Pane::of(&style, place, (key, elements.len(), 0), 1.0);
        }
        let at = self.mark_area(output).loc;
        let mark = self.capture.mark(view.scale, signs);
        elements.extend(mark.and_then(|mark| mark.element(renderer, at.to_f64(), view, 1.0)));
        // Its shadow is under the mark too, which nothing darkens.
        let shadow = tooltip.and_then(|note| note.shadow.as_ref());
        if let Some(shadow) = shadow {
            shadow.elements(renderer, 1.0, view, elements);
        }
        // What is behind its glass begins under its shadow.
        panes.extend(glass.map(|pane| pane.under(elements.len())));
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

    /// Everything an unlocked screen shows, from the top of the stack
    /// down, with or without what `shown` names.
    fn elements<R>(&self, renderer: &mut R, output: &Output, view: View, shown: Extras) -> Scene<R>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let mut elements: Vec<Element<R>> = Vec::new();
        let mut panes = Vec::new();
        let mut desktop = 0;
        if shown.mark {
            self.fading_elements(renderer, output, view, &mut elements);
            self.mark_elements(renderer, output, view, (&mut elements, &mut panes));
        }
        if shown.pointer {
            self.pointer_elements(renderer, view, &mut elements);
        }
        // A prompt that asks for a secret is over all but the pointer and the
        // mark, and dims the rest; no input method types into it.
        self.sealed_elements(renderer, output, view, (&mut elements, &mut panes));
        self.popup_elements(renderer, view, &mut elements);
        // Whether to keep the screens as they are is asked over everything
        // but the pointer.
        let asking = self.interim.prompts.iter();
        let asking = asking.filter(|prompt| prompt.output == *output);
        for prompt in asking {
            let at = prompt.at.to_f64();
            elements.extend(prompt.pixels.element(renderer, at, view, 1.0));
        }
        if let Some(about) = &self.cards.about.shown {
            let seen = self.animation.about_seen;
            let drawn = (&mut elements, &mut panes);
            Self::sized(view, seen, drawn, |drawn| {
                about.elements(renderer, view, seen.opacity, drawn);
            });
        }
        if let Some(notice) = &self.cards.notice {
            let seen = self.animation.notice_seen;
            let drawn = (&mut elements, &mut panes);
            Self::sized(view, seen, drawn, |drawn| {
                notice.elements(renderer, view, seen.opacity, drawn);
            });
        }
        self.switcher_elements(renderer, output, view, (&mut elements, &mut panes));
        if let Some(chrome) = &self.menu_chrome {
            let seen = self.animation.menu_seen;
            let drawn = (&mut elements, &mut panes);
            Self::sized(view, seen, drawn, |drawn| {
                chrome.elements(renderer, view, seen.opacity, drawn);
            });
        }
        self.palette_elements(renderer, view, (&mut elements, &mut panes));
        self.gone_elements(
            renderer,
            view,
            (Level::Over, shown.mark),
            (&mut elements, &mut panes),
        );
        // From the top down: overlays, fullscreen windows, panels and
        // docks, ordinary windows, then whatever sits behind them. What
        // has gone from among each of them is over those that are left.
        let layers = [Layer::Overlay, Layer::Top, Layer::Bottom, Layer::Background];
        let frames = [Some(true), Some(false), None, None];
        for (layer, fullscreen) in layers.into_iter().zip(frames) {
            let seen = shown.mark;
            // Behind everything else, the desktop: its colour and what is
            // drawn on it.
            if layer == Layer::Background {
                desktop = elements.len();
            }
            let drawn = (&mut elements, &mut panes);
            self.gone_elements(renderer, view, (Level::Layer(layer), seen), drawn);
            self.layer_elements(
                renderer,
                output,
                view,
                (layer, Placing::Usual),
                (&mut elements, &mut panes),
            );
            if let Some(fullscreen) = fullscreen {
                if fullscreen {
                    // Menus and tooltips of programs written for X11 are
                    // over every window, and under what is over all.
                    self.unmanaged_elements(renderer, view, shown.mark, &mut elements);
                    // Panels brought back over a fullscreen window.
                    let raised = (Layer::Top, Placing::Raised);
                    let drawn = (&mut elements, &mut panes);
                    self.layer_elements(renderer, output, view, raised, drawn);
                    // A private window offered in a part with nothing in it
                    // shown by its icon, over its blank.
                    elements.extend(self.offer_icons(renderer, view));
                    // The line between windows that share a workspace.
                    let lines = self.lines(|at| view.pixel(at), view.scale.factor());
                    elements.extend(lines.map(Element::Solid));
                    // Under them, the windows offered in a part with nothing
                    // in it.
                    elements.extend(self.offer_titles(renderer, view));
                    elements.extend(self.offer_elements(renderer, view));
                }
                let frames = (Level::Frames(fullscreen), shown.mark);
                self.gone_elements(renderer, view, frames, (&mut elements, &mut panes));
                let drawn = (&mut elements, &mut panes);
                self.frame_elements(renderer, view, (fullscreen, shown.mark), drawn);
            }
        }
        Scene {
            elements,
            panes,
            desktop,
        }
    }

    /// Adds the pictures of arrangements, if they are up, the shadow they
    /// cast, and their glass if the theme makes menus glass, as they are
    /// drawn as one.
    fn palette_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let Some(shown) = &self.hover.palette.shown else {
            return;
        };
        // Its arrow stands above it, up to the titlebar.
        let up = shown.area.origin.offset(0, -palette::ARROW.cast_signed());
        let at = Point::<i32, Logical>::from((up.x, up.y));
        elements.extend(shown.pixels.element(renderer, at.to_f64(), view, 1.0));
        let glass = elements.len();
        if let Some(shadow) = &shown.shadow {
            shadow.elements(renderer, 1.0, view, elements);
        }
        // Its glass is under the panel, below the arrow.
        let style = self.config().theme.style(Part::Menu, States::NONE);
        let area = logical(shown.area);
        let key = glass::key(&"palette");
        let pane = Pane::of(&style, (area, view), (key, glass, elements.len()), 1.0);
        panes.extend(pane);
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
    fn unmanaged_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        seen: bool,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        // In a picture, the menus of a program with a window a rule calls
        // private are left out with it, since they may say what is in it.
        // A program is known by the process it says each window is of.
        let private = |menu: &X11Surface| {
            let process = menu.pid().filter(|_| !seen);
            process.is_some_and(|process| {
                let mut windows = self.windows.iter();
                windows.any(|mapped| match &mapped.role {
                    Role::X11(window) => {
                        window.pid() == Some(process) && self.desktop.private(mapped.id)
                    }
                    Role::Xdg(_) => false,
                })
            })
        };
        let menus = self.x11.unmanaged().rev();
        for (window, surface) in menus.filter(|(menu, _)| !private(menu)) {
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
    /// fading, the last to go first, with its glass if it was glass. A
    /// window a rule called private is not in a picture as it fades, any
    /// more than it was before: only where it is `seen`.
    fn gone_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        (level, seen): (Level, bool),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let gone = self.animation.gone.iter().rev();
        let shown = |gone: &&Gone| gone.level == level && (seen || !gone.private);
        let theme = &self.config().theme;
        for gone in gone.filter(shown) {
            let opacity = gone.seen.opacity;
            let first = elements.len();
            // Its glass fades with it, kept apart from that of anything like
            // it that has come since.
            let mut glass = Vec::new();
            match &gone.what {
                What::Surface(chrome, pieces, cast, place) => {
                    if let Some(chrome) = chrome {
                        let drawn = (&mut *elements, &mut glass);
                        Self::chrome_elements(renderer, view, chrome, opacity, drawn);
                    }
                    let pieces = pieces.elements(view.origin, view.scale.factor(), opacity);
                    elements.extend(pieces.map(Element::Ghost));
                    let style = theme.style(gone.part, States::NONE);
                    let laid = place.and_then(|place| {
                        // Two that went together are told apart by where.
                        let (loc, size) = (place.loc, place.size);
                        let key = glass::key(&("gone", loc.x, loc.y, size.w, size.h));
                        let at = (key, elements.len(), 0);
                        Pane::of(&style, (place, view), at, opacity)
                    });
                    glass.extend(laid);
                    let shadow = chrome.as_ref().and_then(|chrome| chrome.shadow.as_ref());
                    if let Some(shadow) = shadow.or(cast.as_ref()) {
                        shadow.elements(renderer, opacity, view, elements);
                    }
                    let under = elements.len();
                    glass = glass.into_iter().map(|pane| pane.under(under)).collect();
                }
                What::Menu(chrome) => {
                    chrome.elements(renderer, view, opacity, (elements, &mut glass));
                }
                What::Notice(notice) => {
                    notice.elements(renderer, view, opacity, (elements, &mut glass));
                }
                What::About(about) => {
                    about.elements(renderer, view, opacity, (elements, &mut glass));
                }
            }
            let factor = view.scale.factor().into();
            let around =
                scaling::around(gone.seen.scale, || scaling::middle(elements, first, factor));
            scaling::scale(elements, first, around);
            scaling::scale_panes(&mut glass, 0, around);
            let apart = |pane: Pane| pane.apart(&(gone.since, gone.part.name()));
            panes.extend(glass.into_iter().map(apart));
        }
    }

    /// Has `draw` add a part's elements and its glass, and then has them
    /// drawn as large as `seen` says, about the middle of what it drew: a
    /// part on its way in.
    fn sized<R>(
        view: View,
        seen: Seen,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
        draw: impl FnOnce((&mut Vec<Element<R>>, &mut Vec<Pane>)),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let (first, glass) = (elements.len(), panes.len());
        draw((&mut *elements, &mut *panes));
        let factor = view.scale.factor().into();
        let around = scaling::around(seen.scale, || scaling::middle(elements, first, factor));
        scaling::scale(elements, first, around);
        scaling::scale_panes(panes, glass, around);
    }

    /// Adds the frames that are, or are not, fullscreen, topmost first.
    fn frame_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        (fullscreen, seen): (bool, bool),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
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
            let veil = shown.veil.as_ref().filter(|_| !seen);
            if let Some(Veil::Blank(veil)) = veil {
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
            let first = elements.len();
            // Or it is under glass that blurs it, frame and all, and is
            // blank where glass cannot blur.
            if let Some(Veil::Blurred) = veil {
                let theme = &self.config().theme;
                let blank = theme.style(Part::Capture, States::NONE).ground();
                let key = glass::key(&("private", shown.id));
                let area = (logical(shown.outer), view);
                panes.push(Pane::veiling(area, (PRIVATE_BLUR, blank), (key, first)));
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
            let mut glass = Vec::new();
            if let Some(chrome) = &shown.chrome {
                let drawn = (&mut *elements, &mut glass);
                Self::chrome_elements(renderer, view, chrome, shown.seen.opacity, drawn);
            }
            if shown.shaded {
                // Rolled up into its titlebar, none of the window is
                // drawn, and none of it is left to fade if it goes.
                let nothing = self.noting::<R::TextureId>(view, Part::Window);
                self.animation.note(&shown.surface, nothing.done());
            } else {
                // Under glass that blurs it, its menus are left out: they
                // reach past its frame, where nothing blurs them.
                let alone = matches!(veil, Some(Veil::Blurred));
                self.window_elements(renderer, view, (shown, true, alone), elements);
                // What is behind it, blurred where its program asks, is
                // under it.
                glass.extend(self.window_backdrop(view, shown, elements.len()));
            }
            // Under it all, its shadow.
            let shadow = shown
                .chrome
                .as_ref()
                .and_then(|chrome| chrome.shadow.as_ref());
            if let Some(shadow) = shadow {
                shadow.elements(renderer, shown.seen.opacity, view, elements);
            }
            let factor = view.scale.factor().into();
            let around = scaling::around(shown.seen.scale, || {
                scaling::middle(elements, first, factor)
            });
            scaling::scale(elements, first, around);
            scaling::scale_panes(&mut glass, 0, around);
            // What is behind its glass begins under its shadow.
            let under = elements.len();
            panes.extend(glass.into_iter().map(|pane| pane.under(under)));
        }
    }

    /// Adds a frame's decoration: its titlebar, with its glass if the theme
    /// makes it glass, its borders and where they turn the bottom corners.
    fn chrome_elements<R>(
        renderer: &mut R,
        view: View,
        chrome: &Chrome,
        opacity: f32,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let bar = chrome.decoration.titlebar();
        let at = Point::<i32, Logical>::from((bar.origin.x, bar.origin.y));
        let titlebar = chrome
            .titlebar
            .element(renderer, at.to_f64(), view, opacity);
        elements.extend(titlebar);
        // Its glass is under it, and what is written on it over that,
        // rounded above as it is.
        let looks = &chrome.appearance.looks;
        let area = (Rectangle::new(at, logical_size(bar.size)), view);
        let key = glass::key(&("titlebar", &chrome.tabs));
        let glass = Pane::of(&looks.titlebar, area, (key, elements.len(), 0), opacity);
        let radius = view.scale.pixels(looks.window.radius.cast_signed());
        panes.extend(glass.map(|pane| pane.rounded_above(radius)));
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

    /// Adds what a window itself shows: its pop-ups whole, unless it is
    /// to be drawn `alone`, and its own picture, which is cut to the frame
    /// if the frame's bottom corners are rounded. What it is drawn from is
    /// noted if `noted`, to fade it out by when it goes.
    fn window_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        (shown, noted, alone): (&Shown, bool, bool),
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let scale = smithay::utils::Scale::from(view.scale.factor());
        let mut noting = if noted {
            self.noting(view, Part::Window)
        } else {
            Noting::new(false, view.origin, view.scale.factor())
        };
        // Stretched, it is noted as it is drawn, to fade into the picture
        // its program draws next; over that, the picture it had before,
        // while it does.
        let about = view.pixel(shown.origin.to_f64());
        if let Some(Stretched { by, .. }) = shown.stretch {
            noting.stretched(shown.origin, by);
        }
        if let Some(giving) = &shown.giving {
            let changes = self
                .config()
                .theme
                .style(Part::Window, States::NONE)
                .transition;
            let share = changes.progress(self.animation.clock.saturating_sub(giving.since));
            let opacity = (1.0 - share) * shown.seen.opacity;
            let pieces = giving
                .pieces
                .elements(view.origin, view.scale.factor(), opacity);
            elements.extend(pieces.map(Element::Ghost));
        }
        // A window's surface may start above and left of its visible
        // geometry, where it draws its own shadows.
        let location = shown.origin - shown.window.geometry().loc;
        let rounded = shown.chrome.as_ref().filter(|chrome| chrome.radius > 0);
        let first = elements.len();
        let Some(chrome) = rounded else {
            let at = view.pixel(location.to_f64());
            let opacity = shown.seen.opacity;
            let surfaces: Vec<WaylandSurfaceRenderElement<R>> = if alone {
                let kind = Kind::Unspecified;
                render_elements_from_surface_tree(
                    renderer,
                    &shown.surface,
                    at,
                    scale,
                    opacity,
                    kind,
                )
            } else {
                shown.window.render_elements(renderer, at, scale, opacity)
            };
            for surface in &surfaces {
                noting.add(surface, noting.paint(surface.texture()));
            }
            elements.extend(surfaces.into_iter().map(Element::Surface));
            if let Some(Stretched { by, .. }) = shown.stretch {
                scaling::stretch(elements, first, about, by);
            }
            self.animation.note(&shown.surface, noting.done());
            return;
        };
        // Pop-ups reach beyond the frame and are not cut to it.
        let popups = PopupManager::popups_for_surface(&shown.surface);
        for (popup, offset) in popups.filter(|_| !alone) {
            let at = location + shown.window.geometry().loc + offset - popup.geometry().loc;
            let picture: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
                renderer,
                popup.wl_surface(),
                view.pixel(at.to_f64()),
                scale,
                shown.seen.opacity,
                Kind::Unspecified,
            );
            for surface in &picture {
                noting.add(surface, noting.paint(surface.texture()));
            }
            elements.extend(picture.into_iter().map(Element::Surface));
        }
        // What lands on each strip of the frame, once stretched.
        let first = elements.len();
        let cut = Self::cut(view, chrome).into_iter();
        let cut = cut.map(|area| {
            let by = shown.stretch.map_or((1.0, 1.0), |stretched| stretched.by);
            scaling::unstretched(area, about, by)
        });
        for (strip, area) in cut.enumerate() {
            let whole: Vec<WaylandSurfaceRenderElement<R>> = render_elements_from_surface_tree(
                renderer,
                &shown.surface,
                view.pixel(location.to_f64()),
                scale,
                shown.seen.opacity,
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
        if let Some(Stretched { by, .. }) = shown.stretch {
            scaling::stretch(elements, first, about, by);
        }
        self.animation.note(&shown.surface, noting.done());
    }

    /// What of a window in a frame with rounded corners is drawn, in
    /// pixels of the output: its own area, cut where the frame's bottom
    /// corners turn.
    fn cut(view: View, chrome: &Chrome) -> Vec<Rectangle<i32, Physical>> {
        let pixels = |area: Rect| {
            let corner = Point::<i32, Logical>::from((area.origin.x, area.origin.y));
            view.pixels(Rectangle::new(corner, logical_size(area.size)))
        };
        corners::strips(
            pixels(chrome.decoration.outer()),
            pixels(chrome.decoration.client()),
            view.scale.pixels(chrome.radius.cast_signed()),
        )
    }

    /// The glass a window's program asked for behind it, cut to the window
    /// proper, and not the shadow a program that draws its own frame draws
    /// round it on the same surface, and to its frame's rounded corners if
    /// it has them, to go at `at` in what is drawn.
    fn window_backdrop(&self, view: View, shown: &Shown, at: usize) -> Option<Pane> {
        let geometry = shown.window.geometry();
        let origin = shown.origin - geometry.loc;
        let rounded = shown.chrome.as_ref().filter(|chrome| chrome.radius > 0);
        let proper = || vec![view.pixels(Rectangle::new(shown.origin, geometry.size))];
        let bounds = rounded.map_or_else(proper, |chrome| Self::cut(view, chrome));
        self.backdrop(
            &shown.surface,
            (origin, view),
            Some(&bounds),
            (at, shown.seen.opacity),
        )
    }

    /// Begins noting what something is drawn from, if it is a `part` that
    /// takes time to go.
    fn noting<T: Clone + 'static>(&self, view: View, part: Part) -> Noting<T> {
        let closing = States::NONE.with(State::Closing);
        let going = self.config().theme.style(part, closing).transition;
        Noting::new(!going.lasts().is_zero(), view.origin, view.scale.factor())
    }

    /// Adds the layer-shell surfaces of one layer that stand as asked,
    /// topmost first, and under them all the shadows they cast: those on
    /// one layer stand at one height, so that the shadow of one falls on
    /// what is under the layer and not on another, as the shadow of one
    /// notification does not on the next.
    fn layer_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        (layer, standing): (Layer, Placing),
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
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
            // What is on its way out is drawn with what has been brought
            // back.
            let placing = self.standing(index, layer, area);
            let going = placing == Placing::Going && standing == Placing::Raised;
            (placing == standing || going) && !sealed
        };
        let height = self.heads.get(index).map_or(0, |head| head.area().size.h);
        let mut shadows = Vec::new();
        let mut glass = Vec::new();
        for surface in layers.layers_on(layer).rev().filter(wanted) {
            // The wallpaper goes with the workspaces as they slide, drawn
            // once for each the output draws; whatever else is on a layer
            // stays where it is.
            let carried = layer == Layer::Background;
            let still = std::iter::once(0).filter(|_| !carried);
            let shifts = self.desktop.shifts(index).filter(|_| carried);
            for across in still.chain(shifts) {
                let (first, cast, laid) = (elements.len(), shadows.len(), glass.len());
                // A panel is placed within its own output, and drawn as far
                // from there as it has to come yet.
                let area = layers.layer_geometry(surface).unwrap_or_default();
                let off = self.hover.reveal.off(index, (layer, area), height);
                let at = layer_origin(&layers, surface) + view.origin + Point::from((across, off));
                // A panel of a shell component may be coming in, and what it
                // is drawn from is noted, to fade it out by when it goes.
                let mut known = self.animation.layers.iter();
                let known = known.find(|known| known.surface == *surface.wl_surface());
                let surfaces = surface.render_elements::<WaylandSurfaceRenderElement<R>>(
                    renderer,
                    view.pixel(at.to_f64()),
                    smithay::utils::Scale::from(view.scale.factor()),
                    known.map_or(1.0, |known| known.seen.opacity),
                );
                if let Some(known) = known.filter(|known| fades(known.part)) {
                    let mut noting = self.noting(view, known.part);
                    for surface in &surfaces {
                        noting.add(surface, noting.paint(surface.texture()));
                    }
                    self.animation.note(&known.surface, noting.done());
                }
                elements.extend(surfaces.into_iter().map(Element::Surface));
                // The shadow its part casts, and its glass, which is under it and
                // over the shadows of the others.
                let shadow =
                    known.and_then(|known| Some(known.seen.opacity).zip(known.shadow.as_ref()));
                if let Some((share, shadow)) = shadow {
                    shadow.elements(renderer, share, view, &mut shadows);
                }
                let size = layers.layer_geometry(surface).unwrap_or_default().size;
                let place = Rectangle::new(at, size);
                let theme = &self.config().theme;
                // A panel its part makes glass has that, whatever its program
                // asks; else what is behind it is blurred where it asks.
                let share = known.map_or(1.0, |known| known.seen.opacity);
                let pane = known.and_then(|known| {
                    let style = theme.style(known.part, States::NONE);
                    let key = glass::key(&known.surface.id());
                    let at = (key, elements.len(), 0);
                    Pane::of(&style, (place, view), at, share)
                });
                glass.extend(pane.or_else(|| {
                    let placed = (at, view);
                    self.backdrop(surface.wl_surface(), placed, None, (elements.len(), share))
                }));
                // Its shadow is made larger or smaller with it, though it is
                // drawn under all of them.
                let scale = known.map_or(1.0, |known| known.seen.scale);
                let around = scaling::around(scale, || {
                    let middle = at + Point::from((size.w / 2, size.h / 2));
                    view.pixel(middle.to_f64())
                });
                scaling::scale(elements, first, around);
                scaling::scale(&mut shadows, cast, around);
                scaling::scale_panes(&mut glass, laid, around);
            }
        }
        elements.append(&mut shadows);
        // What is behind each begins under all of their shadows.
        let under = elements.len();
        panes.extend(glass.into_iter().map(|pane| pane.under(under)));
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
    pub(crate) origin: Point<i32, Logical>,
    pub(crate) scale: Scale,
}

impl View {
    /// The pixel of the output that a point of the space falls on.
    pub(crate) fn pixel(self, at: Point<f64, Logical>) -> Point<i32, Physical> {
        let within = at - self.origin.to_f64();
        within.to_physical(self.scale.factor()).to_i32_round()
    }

    /// The pixels of the output that an area of the space covers.
    pub(crate) fn pixels(self, area: Rectangle<i32, Logical>) -> Rectangle<i32, Physical> {
        let from = self.pixel(area.loc.to_f64());
        let to = self.pixel((area.loc + area.size.to_point()).to_f64());
        Rectangle::new(from, (to - from).to_size())
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
    /// The scheme omorfos's own pointer is drawn in.
    scheme: Scheme,
    picture: Painted,
    /// Where in the picture the pointer's position is.
    hotspot: Point<f64, Logical>,
    /// The picture as a program that takes pictures of the pointer is
    /// given it.
    given: Given,
}

impl PointerImage {
    /// Draws omorfos's own pointer in `scheme`, or finds the shape in the
    /// cursor theme, at the size the scale calls for. A shape no theme has
    /// is shown as the theme's ordinary pointer, and if there is not even
    /// that, as omorfos's own.
    fn load(
        themes: &Themes,
        (theme, scheme): (&omorfos_config::Cursor, Scheme),
        icon: CursorIcon,
        scale: Scale,
        serial: u64,
    ) -> Self {
        let pixels = scale.pixels(theme.size.get().cast_signed()).unsigned_abs();
        let named = (!theme.is_own()).then(|| {
            [icon, CursorIcon::Default]
                .into_iter()
                .find_map(|icon| themes.load(&theme.theme, icon, pixels))
        });
        let image = named.flatten().or_else(|| {
            // Every shape a program can name is one of omorfos's.
            let shape = Shape::named(icon.name()).unwrap_or(Shape::Default);
            let scheme = match scheme {
                Scheme::Dark => omorfos_brand::Scheme::Dark,
                Scheme::Light => omorfos_brand::Scheme::Light,
            };
            let drawn = omorfos_brand::pointer::pointer(shape, scheme, pixels);
            drawn.map(|drawn| cursor::Image {
                pixels: drawn.pixels,
                size: (drawn.across, drawn.across),
                hotspot: drawn.hotspot,
            })
        });
        // Nothing, at a size of no pixels.
        let image = image.unwrap_or_default();
        let hotspot = (f64::from(image.hotspot.0), f64::from(image.hotspot.1));
        Self {
            icon,
            scale,
            theme: theme.clone(),
            scheme,
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
    /// What a screen that mirrors another shows: what that one shows, made
    /// another size and moved, and the bars beside it.
    pub(crate) Mirrored<R> where R: ImportAll + ImportMem;
    Shown=RelocateRenderElement<RescaleRenderElement<Element<R>>>,
    Bar=SolidColorRenderElement,
}

/// How much larger a picture `picture` pixels large is made to fit on a
/// screen of `room`, and where on it it then is.
fn fitted(
    picture: Size<i32, Physical>,
    room: Size<i32, Physical>,
) -> (f64, Rectangle<i32, Physical>) {
    let along = |room: i32, picture: i32| f64::from(room) / f64::from(picture.max(1));
    let zoom = along(room.w, picture.w).min(along(room.h, picture.h));
    #[expect(
        clippy::cast_possible_truncation,
        reason = "no larger than the screen it is made to fit"
    )]
    let made = |side: i32| (f64::from(side) * zoom).round() as i32;
    let size = Size::from((made(picture.w), made(picture.h)));
    let corner = Point::from(((room.w - size.w) / 2, (room.h - size.h) / 2));
    (zoom, Rectangle::new(corner, size))
}

/// What is left of a screen of `room` either side of `shown`: to its left
/// and right, the screen's height, and above and below it, its width.
fn bars(
    shown: Rectangle<i32, Physical>,
    room: Size<i32, Physical>,
) -> [Rectangle<i32, Physical>; 4] {
    let (left, top) = (shown.loc.x, shown.loc.y);
    let (right, bottom) = (left + shown.size.w, top + shown.size.h);
    let bar =
        |x: i32, y: i32, w: i32, h: i32| Rectangle::new((x, y).into(), (w.max(0), h.max(0)).into());
    [
        bar(0, 0, left, room.h),
        bar(right, 0, room.w - right, room.h),
        bar(left, 0, shown.size.w, top),
        bar(left, bottom, shown.size.w, room.h - bottom),
    ]
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
    Frosted=Strip<Frosted<R>>,
    Tinted=Strip<SolidColorRenderElement>,
    Scaled=RescaleRenderElement<Boxed<R>>,
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
    shadow: Option<Shadowing>,
}

impl Notice {
    fn paint(
        (text, shades): (&mut Text, &mut Shades),
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
        let style = theme.style(Part::Notice, States::NONE);
        let placed = (area.origin.x, area.origin.y, painted_width, painted_height);
        Self {
            problems: problems.to_vec(),
            theme: theme.clone(),
            within,
            scale,
            area,
            pixels: Painted::new(&canvas),
            shadow: shades.cast(&style, scale, placed, None),
        }
    }

    /// The notice, then the shadow under it, as see-through as `opacity`
    /// says; and its glass, if the theme makes it glass.
    fn elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        opacity: f32,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let at = Point::<i32, Logical>::from((self.area.origin.x, self.area.origin.y));
        elements.extend(self.pixels.element(renderer, at.to_f64(), view, opacity));
        let glass = elements.len();
        if let Some(shadow) = &self.shadow {
            shadow.elements(renderer, opacity, view, elements);
        }
        let style = self.theme.style(Part::Notice, States::NONE);
        let at = (glass::key(&"notice"), glass, elements.len());
        panes.extend(Pane::of(&style, (logical(self.area), view), at, opacity));
    }
}

/// The widest "About" gets, in units of layout.
const ABOUT_WIDTH: u32 = 640;

/// What `CREDITS.md` said as omorfos was built, for "About" to show.
const CREDITS: &str = include_str!("../../../CREDITS.md");

/// The cards the compositor shows over everything: the notice of what is
/// wrong, and "About".
#[derive(Debug, Default)]
pub(crate) struct Cards {
    /// The notice of what is wrong, while there is something.
    pub(crate) notice: Option<Notice>,
    /// The problems the user has clicked away, hidden until they change.
    pub(crate) dismissed: Vec<String>,
    /// "About".
    pub(crate) about: AboutCard,
}

/// "About": whether it is to be shown, what the computer was as it was
/// asked for, and it as it is shown.
#[derive(Debug, Default)]
pub(crate) struct AboutCard {
    /// Where the pointer was when it was asked for, while it is to be
    /// shown, on the screen that was there.
    pub(crate) asked: Option<Point<f64, Logical>>,
    /// What the computer was as it was asked for: read once, and not
    /// again each time it is painted.
    pub(crate) system: System,
    /// When it last copied, on the clock of what moves, while its button
    /// still says so.
    pub(crate) copied: Option<Duration>,
    pub(crate) shown: Option<About>,
}

/// How long the tick takes to be drawn on the button of "About" that
/// copies, and how long the button says it has copied before it is as
/// it was.
const TICK_DRAWN: Duration = Duration::from_millis(300);
const COPIED_FOR: Duration = Duration::from_millis(1500);

/// "About omorfos", as it is shown.
#[derive(Debug)]
pub(crate) struct About {
    theme: Theme,
    scheme: Scheme,
    /// The output it is in the middle of.
    within: Rectangle<i32, Logical>,
    scale: Scale,
    /// Where it is, and its buttons: the one that closes it and the one
    /// that copies what the computer is.
    pub(crate) area: Rect,
    close: Rect,
    copy: Rect,
    /// Whether the pointer was on the button that closes it as it was
    /// painted.
    hovered: bool,
    pixels: Painted,
    shadow: Option<Shadowing>,
    /// The button that copies, drawn apart, as it changes: how it looked,
    /// and how much of its tick there was, when it was last painted.
    button: Option<(Style, Option<f64>, Painted)>,
    /// What it lists, all of it, and where on the screen it is shown.
    list: Painted,
    shown: Rect,
    /// How far down what it lists has been scrolled, in units of layout.
    scrolled: f64,
}

impl About {
    fn paint(
        (text, shades): (&mut Text, &mut Shades),
        (theme, scheme, system): (&Theme, Scheme, &System),
        within: Rectangle<i32, Logical>,
        (scale, hovered): (Scale, bool),
    ) -> Self {
        let (wide, high) = (within.size.w.unsigned_abs(), within.size.h.unsigned_abs());
        let width = ABOUT_WIDTH.min(wide.saturating_sub(NOTICE_MARGIN * 2));
        // The mark as large as it is drawn, in pixels, in the scheme.
        let side = scale.pixels(paint::ABOUT_MARK.cast_signed()).unsigned_abs();
        let brand = match scheme {
            Scheme::Dark => omorfos_brand::Scheme::Dark,
            Scheme::Light => omorfos_brand::Scheme::Light,
        };
        let drawn = omorfos_brand::draw(&omorfos_brand::mark(brand), (side, side));
        let mark = drawn.and_then(|png| omorfos_icons::decode_png(&png));
        let credits = omorfos_core::credits(CREDITS);
        let style = theme.style(Part::About, States::NONE);
        let room = width.saturating_sub(style.inset().across());
        let list = paint::about_list(text, theme, (system, &credits), (room, scale));
        // As much of it as there is room for on the screen, within the
        // margins, which is all of it on most screens.
        let tallest = high.saturating_sub(NOTICE_MARGIN * 2);
        let room = tallest.saturating_sub(paint::about_head(theme));
        let length = list.units().1.min(room);
        let version = env!("CARGO_PKG_VERSION");
        let said = (mark.as_slice(), version);
        let looks = (width, length, scale, hovered.then_some(AboutButton::Close));
        let copy = paint::about_copy_size(text, theme);
        let (canvas, (close, copy), shown) = paint::about(text, theme, said, looks, copy);
        let (across, down) = canvas.units();
        let left = wide.saturating_sub(across) / 2;
        let top = high.saturating_sub(down) / 2;
        let area = Rect::new(
            DomainPoint::new(
                within.loc.x + left.cast_signed(),
                within.loc.y + top.cast_signed(),
            ),
            DomainSize::new(across, down),
        );
        let on = |(x, y, wide, high): Area| {
            Rect::new(
                DomainPoint::new(area.origin.x + x, area.origin.y + y),
                DomainSize::new(wide, high),
            )
        };
        let placed = (area.origin.x, area.origin.y, across, down);
        Self {
            theme: theme.clone(),
            scheme,
            within,
            scale,
            area,
            close: on(close),
            copy: on(copy),
            hovered,
            pixels: Painted::new(&canvas),
            shadow: shades.cast(&style, scale, placed, None),
            button: None,
            list: Painted::new(&list),
            shown: on(shown),
            scrolled: 0.0,
        }
    }

    /// Paints its button that copies again if it is to look otherwise:
    /// as `look` says, with as much of its tick as `tick` says once it has
    /// copied.
    fn dress(&mut self, text: &mut Text, look: (Style, &Font), tick: Option<f64>) {
        let (style, font) = look;
        let same = self
            .button
            .as_ref()
            .is_some_and(|(was, ticked, _)| *was == style && *ticked == tick);
        if same {
            return;
        }
        let size = (self.copy.size.width, self.copy.size.height);
        let canvas = paint::about_copy(text, (&style, font), (size, self.scale), tick);
        self.button = Some((style, tick, Painted::new(&canvas)));
    }

    /// The button of it at `point`, if there is one there.
    pub(crate) fn button_at(&self, point: DomainPoint) -> Option<AboutButton> {
        let buttons = [
            (AboutButton::Close, self.close),
            (AboutButton::Copy, self.copy),
        ];
        let mut under = buttons.into_iter().filter(|(_, at)| at.contains(point));
        under.next().map(|(button, _)| button)
    }

    /// Scrolls what it lists `by` units of layout further down, or up if
    /// less than nothing: no further than its end, nor back past its top.
    pub(crate) fn scroll(&mut self, by: f64) {
        let hidden = self.list.height().saturating_sub(self.shown.size.height);
        self.scrolled = (self.scrolled + by).clamp(0.0, f64::from(hidden));
    }

    /// How far a page of what it lists is: all that is shown of it, less a
    /// line, so that the last line of one page is the first of the next.
    pub(crate) fn page(&self) -> f64 {
        let lines = self.shown.size.height.saturating_sub(paint::ABOUT_ROW);
        f64::from(lines.max(paint::ABOUT_ROW))
    }

    /// It, then the shadow under it, as see-through as `opacity` says;
    /// and its glass, if the theme makes it glass.
    fn elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        opacity: f32,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let shown = self.shown;
        let at = Point::<i32, Logical>::from((shown.origin.x, shown.origin.y));
        let rows = (self.scrolled, shown.size.height);
        elements.extend(self.list.rows(renderer, at.to_f64(), rows, view, opacity));
        let copy = Point::<i32, Logical>::from((self.copy.origin.x, self.copy.origin.y));
        let button = self.button.as_ref().map(|(.., button)| button);
        let button =
            button.and_then(|button| button.element(renderer, copy.to_f64(), view, opacity));
        elements.extend(button);
        let at = Point::<i32, Logical>::from((self.area.origin.x, self.area.origin.y));
        elements.extend(self.pixels.element(renderer, at.to_f64(), view, opacity));
        let glass = elements.len();
        if let Some(shadow) = &self.shadow {
            shadow.elements(renderer, opacity, view, elements);
        }
        let style = self.theme.style(Part::About, States::NONE);
        let at = (glass::key(&"about"), glass, elements.len());
        panes.extend(Pane::of(&style, (logical(self.area), view), at, opacity));
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

/// The programs the compositor starts when asked for, rather than with the
/// session.
#[derive(Debug, Default)]
struct OnDemand {
    /// The launcher, once a backend has said where it is, and the one that
    /// is open, by the number of its process.
    launcher: (Option<PathBuf>, Option<u32>),
    /// The locker last started, while it runs, and when it was started
    /// again after ending with the session still locked.
    locker: (Option<u32>, Restarts),
    /// Since when the locker a compositor before started, which the relay
    /// kept, has been waited for to lock the session again.
    relocking: Option<Instant>,
    /// Whether the session is to end, and the programs started in it are
    /// being waited for.
    ending: Ending,
}

/// How far the session has got towards ending.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// It is not to end.
    #[default]
    Not,
    /// It is to end: the programs started in it have been asked to end
    /// first.
    Asked,
    /// It ends once they have, or by then without them.
    By(Instant),
}

/// How long the programs started in the session are given to end, once
/// asked to, before the session ends without them.
const ENDING: Duration = Duration::from_secs(2);

/// Whether the programs to start with the session have been, what starts
/// them in a scope of their own, and where what they write is kept.
#[derive(Debug, Default)]
pub(crate) struct Autostarted {
    done: bool,
    /// What is handed to the relay to be kept as programs are, and what it
    /// hands back.
    pub(crate) parts: Parts,
    scoper: Option<PathBuf>,
    /// Where the journal takes what programs write, once known.
    journal: Option<PathBuf>,
    /// The session's environment as D-Bus and systemd were last told it,
    /// for the programs they start; and whether it is still as it was.
    pub(crate) environment: (Option<Vec<(&'static str, OsString)>>, bool),
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

/// What the locker is called to the next compositor, where the relay
/// keeps it.
const LOCKER: &str = "locker";

/// How long the locker a compositor before started, which the relay kept, is
/// given to lock the session again, before it is taken to be stuck.
const RELOCKING: Duration = Duration::from_secs(5);

/// What is said when the session is to be locked and nothing is set to
/// lock it with.
pub(crate) const NO_LOCKER: &str = "nothing is set to lock the session with: name a screen \
     locker as `locker` in config.yaml, such as `locker: [swaylock, -f]`";

/// The width of a menu panel, in pixels: room for the longest of the
/// labels of a window's menu, "Show on every workspace", after its sign.
const MENU_WIDTH: u32 = 240;
/// How far a submenu lies over the edge of the menu it opens from: a
/// little, so that the two are seen to belong together.
const MENU_OVERLAP: u32 = 4;
/// The height of a separator row in a menu.
const MENU_SEPARATOR: u32 = 9;

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

    /// Whether it is kept by the relay as programs are, where there is one,
    /// rather than ending with the compositor: see [`KEPT`].
    fn kept(self) -> bool {
        self.called().is_some()
    }

    /// What it is called to the next compositor, if it is kept by the
    /// relay.
    fn called(self) -> Option<&'static str> {
        let kept = KEPT.iter().find(|(kind, _)| *kind == self);
        kept.map(|(_, called)| *called)
    }

    /// How it is kept by the relay, as [`Program::kept`] says.
    fn relaying(self) -> Relaying {
        if self.kept() {
            Relaying::Own
        } else {
            Relaying::No
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

/// The programs that come with omorfos and run for the whole session that
/// are kept by the relay as programs are, where there is one, rather than
/// ending with the compositor, and what each is called to the next, which
/// knows it again by that (ADR 61, 3b and 3d): those on the screen, so that
/// none is seen to go and come back, and what keeps the clipboard, so that
/// what was copied outlives a restart. Not what asks for a secret, as a
/// prompt kept through a change of compositor would not be known to the
/// next as one it had shown; nor those not connected to it as a Wayland
/// program is.
const KEPT: [(Program, &str); 5] = [
    (Program::Panel, "panel"),
    (Program::Wallpaper, "wallpaper"),
    (Program::Notifier, "notifier"),
    (Program::Portal, "portal"),
    (Program::Keeper, "keeper"),
];

/// Whether a program the compositor starts is kept by the relay as programs
/// are, where there is one, and as whose (ADR 61, 3d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relaying {
    /// Not: it ends with the compositor.
    No,
    /// As one of the programs that come with omorfos.
    Own,
    /// As the user's: one the configuration starts with roles.
    Users,
}

/// Whether the process `opened` stands for has ended: what stands for it is
/// readable from then on.
fn has_ended(opened: &OwnedFd) -> bool {
    let mut polled = [PollFd::new(opened, PollFlags::IN)];
    let _ = poll(&mut polled, Some(&Timespec::default()));
    !polled[0].revents().is_empty()
}

/// What the relay is told, beside a part's roles, of one the configuration
/// started: that it is the user's, which the next compositor is not to take
/// for one of those that come with omorfos, such as the keeper of the
/// clipboard. A part without it is one of those, as each was handed before
/// there was this.
const USERS: u8 = 1 << 7;

/// A program that comes with omorfos and runs for the whole session.
#[derive(Debug)]
struct Component {
    kind: Program,
    /// A name to look up in `PATH`, or a path.
    program: PathBuf,
    /// The number of its process, while it is running.
    process: Option<u32>,
    restarts: Restarts,
    /// Whether a new version of it has been installed, and is to be taken
    /// up as soon as it may be.
    newer: bool,
    /// The process of the version before, asked to end, which the new one
    /// is started once it has.
    after: Option<u32>,
}

/// How long the keeper of the clipboard that a new version takes the place
/// of is left running, for the new one to take what it keeps: longer than
/// a keeper waits for what it reads.
const HANDOVER: Duration = Duration::from_secs(3);

/// What the user is told of the compositor's own new version where nothing
/// keeps the programs through its being taken up.
const LOG_IN_AGAIN: &str =
    "omorfos has been updated, and the new version is used once you log in again";

/// What the user is told of it while a program written for X11 has a
/// window, which would close as it is taken up.
const WAITS_FOR_X11: &str = "omorfos has been updated, and the new version is taken up once \
     the programs written for X11 have closed, or once you log in again";

/// New versions of the programs that come with omorfos, heard of and being
/// taken up.
#[derive(Debug)]
struct Updates {
    versions: Versions,
    /// The compositor's own program, taken up by its stepping down where
    /// the relay keeps its programs.
    own: Option<PathBuf>,
    /// When its new version was first heard of, or the screens were last
    /// off since, while it is yet to be taken up.
    newer: Option<Instant>,
    /// Whether the user has been told it waits: for them to log in again,
    /// where nothing keeps the programs, or for the programs written for
    /// X11 to close.
    told: bool,
    /// When what is to become of it may next change by the time alone.
    next: Option<Instant>,
    /// The processes of versions before that are to be asked to end, and
    /// when.
    retiring: Vec<(u32, Instant)>,
    /// The compositor's own new version, run to see that it starts at all,
    /// with when it was first heard of, until it has ended.
    checking: Option<(Child, Instant)>,
}

impl Default for Updates {
    fn default() -> Self {
        Self {
            versions: Versions::new(),
            own: None,
            newer: None,
            told: false,
            next: None,
            retiring: Vec::new(),
            checking: None,
        }
    }
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
    shadow: Option<Shadowing>,
}

/// What is changing colour, coming in or going.
#[derive(Debug)]
pub(crate) struct Animation {
    /// The frames and menu rows that are on their way from one look to
    /// another, and what has just come into the picture.
    motion: Motion,
    /// The same of the frames windows are pictured in by themselves, kept
    /// apart: a tab behind another is shown in front in its own picture.
    portrayal: Motion,
    /// The windows pictures are being taken of by themselves, each in its
    /// frame as it would be shown, wherever it is, as of the last sync.
    portraits: Vec<Shown>,
    /// The time they go by: how long it is since the first
    /// [`Compositor::refresh`], and when that was.
    clock: Duration,
    epoch: Option<Instant>,
    /// How much of the menu and of the notice there is to see while each
    /// is coming in, and how large each is.
    menu_seen: Seen,
    notice_seen: Seen,
    about_seen: Seen,
    /// The panels of shell components that are on the screen, and those
    /// that were when this was last worked out.
    layers: Vec<Present>,
    before: Vec<Present>,
    /// What each window and panel was last drawn from, noted while
    /// drawing, which does not change anything else.
    drawn: RefCell<Vec<(WlSurface, Noted)>>,
    /// What has gone from the screen and is still fading, oldest first.
    gone: Vec<Gone>,
    /// What the screens showed as the compositor was started, to fade.
    pub(crate) fading: Vec<Fade>,
    /// The pictures of the shadows cast.
    pub(crate) shades: Shades,
    /// The glass under parts, and what it keeps between frames.
    pub(crate) glass: Glass,
    /// How each screen's frames are drawn, for the socket to tell.
    pub(crate) frames: Frames,
    /// How many times the desktop had sent frames somewhere as of the
    /// last sync: a frame whose place has changed since, while it has, is
    /// drawn on its way there.
    sent: u64,
}

/// A panel of a shell component that is on the screen.
#[derive(Debug)]
struct Present {
    surface: WlSurface,
    /// The part of the theme it is named after.
    part: Part,
    layer: Layer,
    /// How much of it there is to see, and how large it is.
    seen: Seen,
    /// The shadow it casts, if the theme gives its part one.
    shadow: Option<Shadowing>,
    /// Where it is on the screens, for its glass as it fades out.
    place: Rectangle<i32, Logical>,
}

/// Whether a panel named after `part` comes and goes over the time the
/// part's colours take to change. A question does not: what it leads to
/// may be a picture of the screen, which it is not to be in.
fn fades(part: Part) -> bool {
    part != Part::Question
}

/// Something that has gone from the screen, while it fades.
#[derive(Debug)]
struct Gone {
    /// The part of the theme whose time it takes.
    part: Part,
    /// When it went, and how much of it there was to see then and how
    /// large it was.
    since: Duration,
    from: Seen,
    /// How much of it there is to see now, and how large it is.
    seen: Seen,
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
    /// A window, with the decoration it had and the shadow it cast; or a
    /// panel, which has no decoration, with the shadow it cast and where
    /// it was, for its glass.
    Surface(
        Option<Box<Chrome>>,
        Noted,
        Option<Shadowing>,
        Option<Rectangle<i32, Logical>>,
    ),
    Menu(MenuChrome),
    Notice(Notice),
    About(Box<About>),
}

impl Animation {
    fn new() -> Self {
        Self {
            motion: Motion::new(),
            portrayal: Motion::new(),
            portraits: Vec::new(),
            clock: Duration::ZERO,
            epoch: None,
            menu_seen: Seen::WHOLE,
            notice_seen: Seen::WHOLE,
            about_seen: Seen::WHOLE,
            layers: Vec::new(),
            before: Vec::new(),
            drawn: RefCell::new(Vec::new()),
            gone: Vec::new(),
            fading: Vec::new(),
            shades: Shades::default(),
            glass: Glass::default(),
            frames: Frames::default(),
            sent: 0,
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
        taken(&self.drawn, surface)
    }

    /// Has the windows that were on the screen and no longer are go on
    /// being drawn, as they last were, while they fade.
    fn leave(&mut self, previous: Vec<Shown>, scene: &[Shown]) {
        for shown in previous {
            // What slid from the screen with its workspace has not gone.
            let beside = shown.on == Showing::Beside;
            let still = beside || scene.iter().any(|now| now.id == shown.id);
            let pieces = (!still).then(|| self.noted(&shown.surface));
            let gone = pieces.flatten().map(|pieces| Gone {
                part: Part::Window,
                since: self.clock,
                from: shown.seen,
                seen: shown.seen,
                level: Level::Frames(shown.over),
                private: shown.private,
                what: What::Surface(shown.chrome.map(Box::new), pieces, None, None),
            });
            self.gone.extend(gone);
        }
    }

    /// The menu or the notice, now that it has been closed.
    fn closing(&self, part: Part, what: What) -> Gone {
        // How much of it there was to see was worked out when it was
        // last there.
        let from = match what {
            What::Notice(_) => self.notice_seen,
            What::About(_) => self.about_seen,
            What::Menu(_) | What::Surface(..) => self.menu_seen,
        };
        Gone {
            part,
            since: self.clock,
            from,
            seen: from,
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
        let motion = &mut self.motion;
        self.gone.retain_mut(|gone| {
            // What went while it was still coming leaves from as much of it
            // as had come.
            let left = motion.leaving(theme, gone.part, gone.since);
            gone.seen = left.map_or(gone.seen, |left| gone.from.and(left));
            left.is_some() && !locked
        });
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
    /// The shadow each casts, if the theme gives menus one.
    shadows: Vec<Shadowing>,
}

impl MenuChrome {
    /// Paints `menu`, its rows as `looks` says and their signs and icons
    /// from `icons`, keeping the shadows of what it was painted as before,
    /// `had`, where they are the same.
    fn paint(
        (text, shades, icons): (&mut Text, &mut Shades, &mut WindowIcons),
        theme: &Theme,
        (menu, looks): (&Menu<Action>, Vec<Vec<Style>>),
        scale: Scale,
        had: Option<Self>,
    ) -> Self {
        let panels: Vec<(Point<i32, Logical>, Painted)> = menu
            .panels()
            .iter()
            .zip(&looks)
            .map(|(panel, looks)| {
                let tools = (&mut *text, &mut *icons);
                let canvas = paint::menu_panel(tools, theme, menu, panel, looks, scale);
                let origin = menu.panel_rect(panel).origin;
                (Point::from((origin.x, origin.y)), Painted::new(&canvas))
            })
            .collect();
        let style = theme.style(Part::Menu, States::NONE);
        let mut had = had.map(|had| had.shadows).unwrap_or_default().into_iter();
        let shadows = panels.iter().filter_map(|(at, panel)| {
            let area = (at.x, at.y, panel.width(), panel.height());
            shades.cast(&style, scale, area, had.next())
        });
        Self {
            menu: menu.clone(),
            looks,
            theme: theme.clone(),
            scale,
            shadows: shadows.collect(),
            panels,
        }
    }

    /// Its panels, then the shadows under them, as see-through as `opacity`
    /// says; and the glass of each, if the theme makes menus glass.
    fn elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        opacity: f32,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let mut glass = Vec::new();
        let style = self.theme.style(Part::Menu, States::NONE);
        for (index, (at, panel)) in self.panels.iter().enumerate() {
            elements.extend(panel.element(renderer, at.to_f64(), view, opacity));
            let size = Size::from((panel.width().cast_signed(), panel.height().cast_signed()));
            let place = (Rectangle::new(*at, size), view);
            let key = glass::key(&("menu", index));
            glass.extend(Pane::of(&style, place, (key, elements.len(), 0), opacity));
        }
        for shadow in &self.shadows {
            shadow.elements(renderer, opacity, view, elements);
        }
        let under = elements.len();
        panes.extend(glass.into_iter().map(|pane| pane.under(under)));
    }
}

/// Protocols offered whose state the compositor only has to keep, or to
/// hand to Smithay when it asks, and the little it keeps of some.
#[derive(Debug)]
pub(crate) struct Protocols {
    _decorations: XdgDecorationState,
    _idle_inhibit: IdleInhibitManagerState,
    /// Windows a program shares with another by a handle, for a dialog of
    /// one to belong to a window of the other, as a portal's does.
    pub(crate) foreign: XdgForeignState,
    _presentation: PresentationState,
    /// Programs holding the pointer, and what decides whether they may.
    pub(crate) holding: Holding,
    /// Gestures on a touchpad, and the swipe of ours under way.
    pub(crate) gestures: Gestures,
    /// What is behind programs, blurred where they ask.
    pub(crate) backdrops: Backdrops,
}

impl Protocols {
    /// Offers them on `display`.
    fn offer(display: &DisplayHandle) -> Self {
        Self {
            _decorations: XdgDecorationState::new::<Compositor>(display),
            _idle_inhibit: IdleInhibitManagerState::new::<Compositor>(display),
            foreign: XdgForeignState::new::<Compositor>(display),
            _presentation: PresentationState::new::<Compositor>(display, Monotonic::ID as u32),
            holding: Holding::offer(display),
            gestures: Gestures::offer(display),
            backdrops: Backdrops::offer(display),
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

    /// How tall it is on screen, in units of layout.
    pub(crate) fn height(&self) -> u32 {
        self.size.h.unsigned_abs()
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
        Self::from_bytes(bytes, Fourcc::Argb8888, pixels, size.into())
    }

    /// Readies `bytes`, a picture of `pixels` across and down in `format`,
    /// to be drawn `size` large.
    pub(crate) fn from_bytes(
        bytes: &[u8],
        format: Fourcc,
        pixels: (i32, i32),
        size: Size<i32, Logical>,
    ) -> Self {
        Self {
            pixels: MemoryRenderBuffer::from_slice(
                bytes,
                format,
                pixels,
                1,
                Transform::Normal,
                None,
            ),
            size,
            whole: Rectangle::from_size(Size::from(pixels).to_f64()),
        }
    }

    /// The same painting, of which only what is within a border `border`
    /// pixels wide is drawn.
    pub(crate) fn within(mut self, border: i32) -> Self {
        let inset = f64::from(border);
        let (width, height) = (self.whole.size.w, self.whole.size.h);
        let size = Size::from((width - 2.0 * inset, height - 2.0 * inset));
        self.whole = Rectangle::new((inset, inset).into(), size);
        self
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
        self.placed(
            renderer,
            (at, self.size),
            view,
            (opacity, Kind::Unspecified),
        )
    }

    /// The rows of the painting from `from` units down from its top and
    /// `tall` units tall, placed at `at` as [`Painted::element`] places all
    /// of it. Where they begin is a whole pixel, so that what is drawn is
    /// as sharp as all of it.
    pub(crate) fn rows<R>(
        &self,
        renderer: &mut R,
        at: Point<f64, Logical>,
        (from, tall): (f64, u32),
        view: View,
        opacity: f32,
    ) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let across = self.whole.size.w;
        let pixels = self.whole.size.h / f64::from(self.size.h.max(1));
        let top = (from * pixels).round();
        let part = Rectangle::new((0.0, top).into(), (across, f64::from(tall) * pixels).into());
        MemoryRenderBufferRenderElement::from_buffer(
            renderer,
            view.pixel(at).to_f64(),
            &self.pixels,
            Some(opacity),
            Some(part),
            Some((self.size.w, tall.cast_signed()).into()),
            Kind::Unspecified,
        )
        .ok()
        .map(Element::Memory)
    }

    /// The painting placed at `at` as [`Painted::element`] places it, but
    /// made `size` large on screen, whatever it was readied to be.
    pub(crate) fn stretched<R>(
        &self,
        renderer: &mut R,
        (at, size): (Point<f64, Logical>, Size<i32, Logical>),
        view: View,
        opacity: f32,
    ) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        self.placed(renderer, (at, size), view, (opacity, Kind::Unspecified))
    }

    /// The painting as the pointer, which a backend may show by other
    /// means than drawing it.
    fn cursor<R>(&self, renderer: &mut R, at: Point<f64, Logical>, view: View) -> Option<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        self.placed(renderer, (at, self.size), view, (1.0, Kind::Cursor))
    }

    fn placed<R>(
        &self,
        renderer: &mut R,
        (at, size): (Point<f64, Logical>, Size<i32, Logical>),
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
            Some(size),
            kind,
        )
        .ok()
        .map(Element::Memory)
    }
}

/// Where a layer surface itself is on its output, in units of layout.
///
/// What Smithay calls its geometry is where all of it is, its subsurfaces
/// too, and begins further left or up than the surface when one of them
/// reaches out past it, as a line of text wider than the screen does: taken
/// for where the surface is, it put all of it that much out of place.
pub(crate) fn layer_origin(layers: &LayerMap, layer: &LayerSurface) -> Point<i32, Logical> {
    let all = layers.layer_geometry(layer).unwrap_or_default();
    all.loc - layer.bbox().loc
}

/// How much there is to see of the window `id` of a frame with `tabs`, and
/// how large it is.
/// Every window of the frame is in the picture, so that showing another of
/// its tabs is not a coming in; and what is `sliding` into view with its
/// workspace was there all along.
fn presence(
    motion: &mut Motion,
    theme: &Theme,
    (tabs, id): (&[WindowId], WindowId),
    sliding: bool,
) -> Seen {
    let mut seen = Seen::WHOLE;
    for tab in tabs {
        if sliding {
            motion.arrived(Part::Window, tab.raw());
        }
        let now = motion.presence(theme, Part::Window, tab.raw());
        if *tab == id {
            seen = now;
        }
    }
    seen
}

/// How a window's picture is drawn, its program having been told the
/// first of `sizes`, its frame being drawn the second and what its program
/// shows the third, as `before` it was drawn last: stretched to fill the
/// frame while the frame is on its way somewhere or the program is behind
/// with it, where windows change over a time, `changes`; and, once the
/// program has caught up with a frame that is still, with the picture it
/// had while it was, which is in what was `noted` of what was drawn,
/// fading into the new one from `now`. Unless the frame was `held` by hand
/// while it was stretched, which it is said to have been until it has
/// caught up: then the program draws at each step, and each picture
/// fading would leave the one before it over the next, which over a
/// window that can be seen through shows as one that cannot.
fn stretching(
    role: &Role,
    (told, wanted, drawn): Sizes,
    before: Option<&mut Shown>,
    noted: (&RefCell<Vec<(WlSurface, Noted)>>, &WlSurface),
    (changes, now, held): (omorfos_config::Transition, Duration, bool),
) -> (Option<Stretched>, Option<Giving>) {
    // A picture that was stretched only on its way, or while it was held,
    // is the new one already.
    let before = before.map(|before| (before.stretch, before.giving.take(), before.going()));
    let (was, giving, going) = before.unwrap_or_default();
    let was_held = was.is_some_and(|was| was.held);
    let behind = wanted != told || role.behind(told, drawn);
    let stretch = (behind && !changes.lasts().is_zero()).then(|| {
        let across = f64::from(wanted.w) / f64::from(drawn.w.max(1));
        let by = (across, f64::from(wanted.h) / f64::from(drawn.h.max(1)));
        Stretched {
            by,
            held: held || was_held,
        }
    });
    let caught_up = was.is_some() && stretch.is_none() && !going && !was_held;
    let begun = caught_up.then(|| taken(noted.0, noted.1)).flatten();
    let begun = begun.map(|pieces| Giving { pieces, since: now });
    let lasting = |giving: &Giving| now.saturating_sub(giving.since) < changes.lasts();
    (stretch, begun.or(giving).filter(lasting))
}

/// Takes the decoration painted for a frame of `tabs` out of what was
/// shown before, if one was.
fn painted(before: &mut Vec<Shown>, tabs: &[WindowId]) -> Option<Chrome> {
    let found = before
        .iter()
        .position(|shown| shown.chrome.as_ref().is_some_and(|c| c.tabs == tabs));
    found.and_then(|index| before.swap_remove(index).chrome)
}

/// Takes what a surface was last drawn from out of what was noted of
/// everything drawn, if it ever was, for its pieces to be drawn for
/// themselves.
fn taken(drawn: &RefCell<Vec<(WlSurface, Noted)>>, surface: &WlSurface) -> Option<Noted> {
    let mut drawn = drawn.borrow_mut();
    let found = drawn.iter().position(|(known, _)| known == surface);
    found.map(|index| drawn.swap_remove(index).1.left())
}

/// How large a window is: as its program was told, as its frame is drawn,
/// and as its program shows it.
type Sizes = (Size<i32, Logical>, Size<i32, Logical>, Size<i32, Logical>);

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
    /// How much of it there is to see, and how large it is, while it is
    /// coming in.
    seen: Seen,
    /// Whether it is rolled up into its titlebar, with none of the window
    /// showing.
    pub(crate) shaded: bool,
    chrome: Option<Chrome>,
    /// What is drawn in its place in a picture taken of the screen, if a
    /// rule says it is private. Kept, so that a picture after the first
    /// is not thought to differ where it is.
    veil: Option<Veil>,
    /// Whether a rule calls it private, which it still is once the veil
    /// it was drawn with has gone to the next frame.
    private: bool,
    /// Which of the workspaces drawn on its output it is on: one beside
    /// the one shown, while they slide, is gone from without fading once
    /// they settle.
    on: Showing,
    /// How its picture is stretched to fill its frame, while its program
    /// has yet to draw the size the frame has.
    stretch: Option<Stretched>,
    /// The picture it had while it was stretched, fading into the one its
    /// program has drawn since.
    giving: Option<Giving>,
    /// Where its frame is drawn, on its way to where it was sent or there.
    way: Way,
}

/// How a window's picture is stretched to fill its frame, and whether its
/// frame has been held by hand since it was: one that has is not faded
/// into the next.
#[derive(Debug, Clone, Copy)]
struct Stretched {
    by: Stretch,
    held: bool,
}

/// A window's last picture before its program drew the size its frame
/// has, and when it began to give way to the new one.
#[derive(Debug)]
struct Giving {
    pieces: Noted,
    since: Duration,
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
        let kept = scene
            .iter_mut()
            .filter_map(|shown| match shown.veil.take() {
                Some(Veil::Blank(veil)) => Some((shown.id, veil)),
                Some(Veil::Blurred) | None => None,
            });
        Self {
            kept: kept.collect(),
            color: color(theme.style(Part::Capture, States::NONE).ground()),
        }
    }

    /// What stands for a window whose frame is of a size: blurred if it
    /// is to be, else a blank.
    fn veil(&mut self, id: WindowId, size: DomainSize, blurred: bool) -> Veil {
        if blurred {
            return Veil::Blurred;
        }
        let kept = self.kept.iter().position(|(of, _)| *of == id);
        let kept = kept.map(|index| self.kept.swap_remove(index).1);
        let mut veil = kept.unwrap_or_default();
        veil.update(logical_size(size), self.color);
        Veil::Blank(veil)
    }
}

/// What stands for a private window in a picture of the screen.
#[derive(Debug)]
enum Veil {
    /// A blank the size of its frame, kept from one frame to the next.
    Blank(SolidColorBuffer),
    /// Its frame as it is, under glass that blurs it.
    Blurred,
}

/// How far a private window to be blurred in pictures is blurred, in units
/// of layout: twice what the thinnest glass is, which leaves nothing that
/// is written readable, however large.
pub(crate) const PRIVATE_BLUR: u32 = 32;

impl Shown {
    /// Whether its frame was drawn on its way to where it was sent.
    fn going(&self) -> bool {
        self.outer != self.way.to()
    }

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
    /// The shadow it casts, if the theme gives it one.
    shadow: Option<Shadowing>,
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
        (text, motion, shades): (&mut Text, &mut Motion, &mut Shades),
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
        let window = &appearance.looks.window;
        let shade = shades.of(window.shadow, window.radius, scale);
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
                shadow: shade.map(Shadowing::new),
            }
        });
        for (buffer, border) in chrome.borders.iter_mut().zip(straight) {
            // Updating with an unchanged size and colour is not damage.
            buffer.update(logical_size(border.size), color(outline));
        }
        chrome.tabs.clone_from(&frame.tabs);
        chrome.decoration = decoration;
        chrome.radius = radius;
        // A window as large as it can be, or larger, casts none.
        let outer = frame.geometry;
        let area = (
            outer.origin.x,
            outer.origin.y,
            outer.size.width,
            outer.size.height,
        );
        let cast = !frame.fullscreen && !frame.maximized;
        if let Some(shadow) = chrome.shadow.as_mut() {
            shadow.place(cast.then_some(area));
        }
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
    /// Which of the workspaces drawn on its output it is on.
    on: Showing,
}

impl FrameInfo {
    /// Draws it on its way from where it was drawn `before` to where it
    /// is, if it `moves`, as of `now`: saying how, and how large its window
    /// is where it is, as it is drawn, and as its program `shows` it.
    fn travel(
        &mut self,
        before: Option<&Shown>,
        shows: Size<i32, Logical>,
        (now, moves): (Duration, bool),
    ) -> (Way, Sizes) {
        let mut way = before.map_or(Way::at(self.geometry), |before| before.way);
        way.go(self.geometry, now, moves);
        let told = logical_size(self.client.size);
        self.ease(way.now(now));
        (way, (told, logical_size(self.client.size), shows))
    }

    /// Draws it at `to`, on its way to where it is: what is inside the
    /// frame as far in from each edge as it is there.
    fn ease(&mut self, to: Rect) {
        let from = self.geometry;
        self.client = self.client.carried(from, to);
        self.decoration = self
            .decoration
            .map(|decoration| decoration.at(decoration.outer().carried(from, to)));
        self.geometry = to;
    }

    /// Where its window is put, and in what state. One rolled up into its
    /// titlebar is left the size it was: it is not shown, but it is not
    /// made to be nothing.
    fn placed(&self) -> Option<Placed> {
        (!self.standing.shaded).then(|| Placed {
            client: self.client,
            maximized: self.maximized,
            // A window that shares its workspace fills a part of the
            // screen and is not fullscreen.
            fullscreen: self.fullscreen && self.home == Home::Solo,
            tiled: self.fullscreen && self.home == Home::Shared,
        })
    }

    /// What is known of a frame, drawn at `scale`, with the pointer `over`
    /// a button of some frame's titlebar or of none.
    fn of(frame: &Frame<'_>, scale: Scale, over: Option<(WindowId, Button)>) -> Self {
        let shown = frame.window().id;
        let over = over.filter(|(window, _)| *window == shown);
        Self {
            hovered: over.map(|(_, button)| button),
            standing: frame.standing,
            on: frame.on,
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

/// Where `area` is in the space.
pub(crate) fn logical(area: Rect) -> Rectangle<i32, Logical> {
    let corner = Point::from((area.origin.x, area.origin.y));
    Rectangle::new(corner, logical_size(area.size))
}

pub(crate) fn logical_size(size: DomainSize) -> Size<i32, Logical> {
    let clamp = |length: u32| i32::try_from(length).unwrap_or(i32::MAX);
    (clamp(size.width), clamp(size.height)).into()
}
