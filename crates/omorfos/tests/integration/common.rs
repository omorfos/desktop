//! Test support: a compositor session and a scriptable Wayland client.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::cell::Cell;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

use omorfos::Compositor;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{Bind, ExportMem};
use smithay::output::Output;
use smithay::reexports::calloop::channel;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::pixman::{FormatCode, Image};
use smithay::utils::{Rectangle, Transform};
use tempfile::TempDir;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer,
    wl_data_source, wl_keyboard, wl_output, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1 as listed_handle, ext_foreign_toplevel_list_v1 as toplevel_list,
};
use wayland_protocols::ext::idle_notify::v1::client::{
    ext_idle_notification_v1, ext_idle_notifier_v1,
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::idle_inhibit::zv1::client::{
    zwp_idle_inhibit_manager_v1, zwp_idle_inhibitor_v1,
};
use wayland_protocols::wp::primary_selection::zv1::client::{
    zwp_primary_selection_device_manager_v1 as primary_manager,
    zwp_primary_selection_device_v1 as primary_device,
    zwp_primary_selection_offer_v1 as primary_offer,
    zwp_primary_selection_source_v1 as primary_source,
};
use wayland_protocols::wp::single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1;
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::activation::v1::client::{xdg_activation_token_v1, xdg_activation_v1};
use wayland_protocols::xdg::decoration::zv1::client::{
    zxdg_decoration_manager_v1, zxdg_toplevel_decoration_v1,
};
use wayland_protocols::xdg::shell::client::{
    xdg_popup, xdg_positioner, xdg_surface, xdg_toplevel, xdg_wm_base,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

pub(crate) const WIDTH: i32 = 640;
pub(crate) const HEIGHT: i32 = 480;

/// Evdev key codes offset by 8, as XKB numbers them.
pub(crate) mod key {
    use smithay::input::keyboard::Keycode;

    pub(crate) const ESCAPE: Keycode = Keycode::new(9);
    pub(crate) const ONE: Keycode = Keycode::new(10);
    pub(crate) const TWO: Keycode = Keycode::new(11);
    pub(crate) const Q: Keycode = Keycode::new(24);
    pub(crate) const T: Keycode = Keycode::new(28);
    pub(crate) const RETURN: Keycode = Keycode::new(36);
    pub(crate) const A: Keycode = Keycode::new(38);
    pub(crate) const CONTROL: Keycode = Keycode::new(37);
    pub(crate) const SHIFT: Keycode = Keycode::new(50);
    pub(crate) const ALT: Keycode = Keycode::new(64);
    pub(crate) const X: Keycode = Keycode::new(53);
    pub(crate) const F2: Keycode = Keycode::new(68);
    pub(crate) const SUPER: Keycode = Keycode::new(133);
}

/// A compositor with a software-rendered output and its own configuration
/// directory.
pub(crate) struct Session {
    inner: omorfos::Session,
    renderer: PixmanRenderer,
    /// The outputs, each with the picture it is drawn into. Tests that do
    /// not care have one, [`WIDTH`] by [`HEIGHT`].
    monitors: Vec<Monitor>,
    pub(crate) config: TempDir,
    /// The time the compositor is told it is. Tests advance it by hand.
    pub(crate) now: Instant,
}

/// What a frame looked like.
/// An output drawn with the software renderer.
struct Monitor {
    output: Output,
    image: Image<'static, 'static>,
    tracker: OutputDamageTracker,
    size: (i32, i32),
}

pub(crate) struct Picture {
    pixels: Vec<u8>,
    width: i32,
    /// Whether anything had to be redrawn.
    pub(crate) damaged: bool,
}

impl Picture {
    /// The colour at a position, as `0xAARRGGBB`.
    pub(crate) fn at(&self, x: i32, y: i32) -> u32 {
        let offset = usize::try_from((y * self.width + x) * 4).unwrap();
        u32::from_le_bytes(self.pixels[offset..offset + 4].try_into().unwrap())
    }
}

impl Session {
    /// Starts a compositor with built-in defaults.
    pub(crate) fn new() -> Self {
        Self::with_config(&[])
    }

    /// Starts a compositor whose configuration directory holds `files`.
    pub(crate) fn with_config(files: &[(&str, &str)]) -> Self {
        let config = TempDir::new().unwrap();
        for (name, contents) in files {
            std::fs::write(config.path().join(name), contents).unwrap();
        }
        // Nothing takes time unless the test says so.
        let theme = files.iter().find(|(name, _)| *name == "theme.css");
        let theme = theme.map_or("", |(_, contents)| contents);
        std::fs::write(config.path().join("theme.css"), still(theme)).unwrap();
        Self::in_directory(config, None)
    }

    /// Starts a compositor reading configuration from `directory`, or from
    /// the temporary directory itself when `None`.
    pub(crate) fn in_directory(config: TempDir, directory: Option<&Path>) -> Self {
        let directory = directory.unwrap_or(config.path()).to_owned();
        let inner = omorfos::Session::new(directory).unwrap();
        let mut session = Self {
            inner,
            renderer: PixmanRenderer::new().unwrap(),
            monitors: Vec::new(),
            config,
            now: Instant::now(),
        };
        session.add_output((WIDTH, HEIGHT));
        session
    }

    /// Plugs in another output, to the right of those there are.
    pub(crate) fn add_output(&mut self, size: (i32, i32)) -> Output {
        let name = format!("test-{}", self.monitors.len());
        let compositor = self.inner.compositor_mut();
        let output = compositor.add_output(&name, size, Transform::Normal);
        let width = usize::try_from(size.0).unwrap();
        let height = usize::try_from(size.1).unwrap();
        self.monitors.push(Monitor {
            tracker: OutputDamageTracker::from_output(&output),
            image: Image::new(FormatCode::A8R8G8B8, width, height, true).unwrap(),
            output: output.clone(),
            size,
        });
        output
    }

    /// The first output, which is the only one unless a test adds more.
    pub(crate) fn output(&self) -> Output {
        self.monitors[0].output.clone()
    }

    /// Changes the size of the first output, in pixels.
    pub(crate) fn resize_output(&mut self, size: (i32, i32)) {
        let output = self.output();
        self.inner.compositor_mut().resize_output(&output, size);
    }

    /// Says what scale suits the first output.
    pub(crate) fn suggest_scale(&mut self, scale: omorfos::Scale) {
        let output = self.output();
        self.inner.compositor_mut().suggest_scale(&output, scale);
    }

    /// Tells what is on the first output that a frame was shown.
    pub(crate) fn send_frames(&mut self, elapsed: Duration) {
        let output = self.output();
        self.inner.compositor_mut().send_frames(&output, elapsed);
    }

    /// The compositor.
    pub(crate) fn compositor(&self) -> &Compositor {
        self.inner.compositor()
    }

    /// The compositor, to act on.
    pub(crate) fn compositor_mut(&mut self) -> &mut Compositor {
        self.inner.compositor_mut()
    }

    /// The session under test itself, for what the harness does not wrap.
    pub(crate) fn inner(&mut self) -> &mut omorfos::Session {
        &mut self.inner
    }

    /// Ends the session, keeping its configuration directory.
    pub(crate) fn end(self) -> TempDir {
        self.config
    }

    /// Installs a cursor theme called `name` whose shapes are plain
    /// squares, one per `(shape, size, colour)`, with the hotspot one pixel
    /// in from the corner. Returns where themes are then to be found.
    pub(crate) fn install_cursors(
        &self,
        name: &str,
        shapes: &[(&str, u32, u32)],
    ) -> omorfos::Themes {
        let themes = self.config.path().join("icons");
        let folder = themes.join(name).join("cursors");
        std::fs::create_dir_all(&folder).unwrap();
        for (shape, size, color) in shapes {
            let word = |value: u32| value.to_le_bytes();
            // One image: the file's header, a table of contents with one
            // entry, then the image's own header and its pixels.
            let header = [0x7275_6358, 16, 0x1_0000, 1, 0xfffd_0002, *size, 28];
            let image = [36, 0xfffd_0002, *size, 1, *size, *size, 1, 1, 0];
            let pixels = usize::try_from(size * size).unwrap();
            let mut bytes: Vec<u8> = header.into_iter().chain(image).flat_map(word).collect();
            bytes.extend(word(*color).repeat(pixels));
            std::fs::write(folder.join(shape), bytes).unwrap();
        }
        omorfos::Themes::new(vec![themes])
    }

    /// Adds a source that wakes the session each time something is sent to
    /// it.
    pub(crate) fn waker(&mut self) -> channel::Sender<()> {
        let (sender, receiver) = channel::channel();
        self.inner
            .handle()
            .insert_source(receiver, |_, (), _| {})
            .unwrap();
        sender
    }

    /// Runs the session as it runs in use, asleep until something happens,
    /// until `done` says so.
    ///
    /// The compositor is not stepped by hand here, so this only returns if
    /// whatever `done` waits for wakes the event loop by itself. If nothing
    /// has after half a minute the test fails, since that is what a missed
    /// wake-up looks like. The session has ended afterwards.
    pub(crate) fn run_until(&mut self, mut done: impl FnMut(&mut Compositor) -> bool) {
        let expired = Rc::new(Cell::new(false));
        let alarm = Rc::clone(&expired);
        self.inner
            .handle()
            .insert_source(
                Timer::from_duration(Duration::from_secs(30)),
                move |_, (), _| {
                    alarm.set(true);
                    TimeoutAction::Drop
                },
            )
            .unwrap();
        self.inner
            .run(|compositor| {
                if done(compositor) || expired.get() {
                    compositor.perform(&omorfos_config::Action::Quit);
                }
                Ok(())
            })
            .unwrap();
        assert!(!expired.get(), "the event loop was never woken");
    }

    /// Writes a configuration file. A theme changes colours at once and
    /// has things there at once, as the one the session started with does.
    pub(crate) fn write_config(&self, name: &str, contents: &str) {
        let contents = if name == "theme.css" {
            still(contents)
        } else {
            contents.to_owned()
        };
        std::fs::write(self.config.path().join(name), contents).unwrap();
    }

    /// Lets whatever is coming onto the screen, or changing colour, get
    /// there. Needed where the built-in theme is in use, which takes its
    /// time over both.
    pub(crate) fn settle(&mut self) {
        // What is new is first seen now, and is all there a second on.
        self.step();
        self.advance(Duration::from_secs(1));
    }

    /// Lets time pass for the compositor.
    pub(crate) fn advance(&mut self, by: Duration) {
        self.now += by;
        self.step();
    }

    /// Keeps the compositor running, with its clock moving, until
    /// `condition` holds.
    ///
    /// Some effects reach the compositor a little late: a client's socket
    /// closing, or the notification that a file was written. Both wait for
    /// the last copy of a file descriptor to be closed, and while tests run
    /// in parallel another test may be starting a process, which briefly
    /// holds copies of every descriptor in this one. Such effects have to be
    /// waited for rather than assumed.
    pub(crate) fn eventually(&mut self, condition: impl Fn(&mut Self) -> bool) {
        for _ in 0..5000 {
            if condition(self) {
                return;
            }
            self.advance(Duration::from_millis(200));
            std::thread::sleep(Duration::from_millis(1));
        }
        unreachable!("the expected state was never reached");
    }

    /// Starts the IPC socket in the session's directory and returns its path.
    pub(crate) fn listen(&mut self) -> std::path::PathBuf {
        let path = self.config.path().join("ipc.sock");
        self.compositor_mut().listen_ipc(&path).unwrap();
        path
    }

    /// Sends an IPC request and returns the compositor's answer.
    pub(crate) fn ask(
        &mut self,
        peer: &mut omorfos_ipc::Client,
        request: &omorfos_ipc::Request,
    ) -> omorfos_ipc::Reply {
        peer.send(request).unwrap();
        // Accepting the connection and answering may take a step each.
        self.step();
        self.step();
        peer.receive().unwrap()
    }

    /// Connects a new client.
    pub(crate) fn connect(&mut self) -> Client {
        let (ours, theirs) = UnixStream::pair().unwrap();
        self.compositor_mut().insert_client(ours).unwrap();
        self.adopt(theirs)
    }

    /// Makes a client of a connection the compositor already has the other
    /// end of.
    pub(crate) fn adopt(&mut self, stream: UnixStream) -> Client {
        let mut client = Client::new(stream);
        self.roundtrip(&mut client);
        self.roundtrip(&mut client);
        client
    }

    /// Runs the compositor's side once: handle requests, update, reply.
    pub(crate) fn step(&mut self) {
        self.inner.dispatch(Some(Duration::ZERO), self.now).unwrap();
    }

    /// Exchanges messages until the compositor has handled everything the
    /// client has sent and the client has seen every reply.
    pub(crate) fn roundtrip(&mut self, client: &mut Client) {
        client.state.synced = false;
        client.connection.display().sync(&client.queue.handle(), ());
        for _ in 0..100 {
            client.connection.flush().unwrap();
            self.step();
            client.receive();
            if client.state.synced {
                return;
            }
        }
        unreachable!("the compositor did not answer the client");
    }

    /// Opens a window filled with `color` (`0xAARRGGBB`) and waits until it
    /// is on screen.
    pub(crate) fn open(
        &mut self,
        client: &mut Client,
        app_id: &str,
        size: (i32, i32),
        color: u32,
    ) -> Toplevel {
        let window = client.toplevel(app_id, &format!("{app_id} window"));
        self.roundtrip(client);
        client.fill(&window.surface, size, color);
        self.roundtrip(client);
        self.roundtrip(client);
        window
    }

    /// Puts up a layer-shell surface of `size` filled with `color`, and
    /// waits until it is on screen.
    pub(crate) fn open_layer(
        &mut self,
        client: &mut Client,
        shape: &LayerShape,
        size: (i32, i32),
        color: u32,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        let (surface, role) = client.layer(shape);
        self.roundtrip(client);
        client.fill(&surface, size, color);
        self.roundtrip(client);
        self.roundtrip(client);
        (surface, role)
    }

    /// Like [`Session::open`], for a window that asks the compositor to draw
    /// its titlebar and borders.
    pub(crate) fn open_decorated(
        &mut self,
        client: &mut Client,
        app_id: &str,
        size: (i32, i32),
        color: u32,
    ) -> Toplevel {
        let window = client.toplevel_uncommitted(app_id, &format!("{app_id} window"));
        client.decoration(&window);
        window.surface.commit();
        self.roundtrip(client);
        client.fill(&window.surface, size, color);
        self.roundtrip(client);
        self.roundtrip(client);
        window
    }

    /// Draws a frame and returns what is on screen.
    pub(crate) fn draw(&mut self) -> Picture {
        self.draw_on(0)
    }

    /// Draws a frame on one of the outputs, counted in the order they were
    /// added, and returns what is on it.
    pub(crate) fn draw_on(&mut self, index: usize) -> Picture {
        let monitor = &mut self.monitors[index];
        let mut target = self.renderer.bind(&mut monitor.image).unwrap();
        let result = self
            .inner
            .compositor_mut()
            .render(
                &monitor.output,
                &mut self.renderer,
                &mut target,
                &mut monitor.tracker,
                1,
            )
            .unwrap();
        let damaged = result.damage.is_some();
        let width = monitor.size.0;
        let region = Rectangle::from_size(monitor.size.into());
        let mapping = self
            .renderer
            .copy_framebuffer(&target, region, Fourcc::Argb8888)
            .unwrap();
        let pixels = self.renderer.map_texture(&mapping).unwrap().to_vec();
        Picture {
            pixels,
            width,
            damaged,
        }
    }
}

/// Something the compositor told the client.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Event {
    Configure {
        size: (i32, i32),
        states: Vec<xdg_toplevel::State>,
    },
    Close,
    /// A layer-shell surface was told its size.
    LayerConfigure {
        size: (u32, u32),
    },
    /// The compositor said who draws the decoration: `true` for itself.
    Decoration {
        server_side: bool,
    },
    PopupConfigure {
        position: (i32, i32),
        size: (i32, i32),
    },
    PopupRepositioned(u32),
    /// The compositor dismissed a popup.
    PopupDone,
    PointerEnter {
        x: f64,
        y: f64,
    },
    PointerLeave,
    PointerMotion {
        x: f64,
        y: f64,
    },
    PointerButton {
        button: u32,
        pressed: bool,
    },
    PointerScroll {
        vertical: bool,
        value: f64,
    },
    KeyboardEnter,
    KeyboardLeave,
    Key {
        code: u32,
        pressed: bool,
    },
    FrameDone,
    /// A panel was told it has nowhere to be any more.
    LayerClosed,
    /// The session is locked, as this client asked.
    Locked,
    /// This client's request to lock the session was turned down.
    LockRefused,
    /// The size a lock surface has to be.
    LockSize(u32, u32),
    /// The user has been away for as long as this client asked to be told
    /// about.
    Idle,
    /// The user is back.
    Back,
}

/// The client's globals and everything it has been told.
#[derive(Default)]
pub(crate) struct ClientState {
    synced: bool,
    compositor: Option<wl_compositor::WlCompositor>,
    subcompositor: Option<wl_subcompositor::WlSubcompositor>,
    shm: Option<wl_shm::WlShm>,
    wm_base: Option<xdg_wm_base::XdgWmBase>,
    decorations: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    clipboards: Option<wl_data_device_manager::WlDataDeviceManager>,
    activation: Option<xdg_activation_v1::XdgActivationV1>,

    /// The windows the compositor lists for taskbars, as this client has
    /// been told about them, in the order they were announced.
    pub(crate) taskbar: Vec<Task>,
    primaries: Option<primary_manager::ZwpPrimarySelectionDeviceManagerV1>,
    fractions: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    single_pixels: Option<wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1>,
    shapes: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    lockers: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    idlers: Option<ext_idle_notifier_v1::ExtIdleNotifierV1>,
    inhibitors: Option<zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1>,
    /// The outputs the compositor offers, in the order they appeared.
    pub(crate) screens: Vec<wl_output::WlOutput>,
    /// What the client's surfaces were last told about how they are shown.
    pub(crate) told: Told,
    primary_device: Option<primary_device::ZwpPrimarySelectionDeviceV1>,
    /// What this client hands over when asked for the text it has selected.
    selected: String,
    /// The primary selection as last offered to this client.
    pub(crate) primary: Option<primary_offer::ZwpPrimarySelectionOfferV1>,
    /// The activation token most recently issued to this client.
    pub(crate) token: Option<String>,
    data_device: Option<wl_data_device::WlDataDevice>,
    /// What this client hands over when asked for what it copied.
    copied: String,
    /// The clipboard as last offered to this client, if it holds anything.
    pub(crate) selection: Option<wl_data_offer::WlDataOffer>,
    /// The formats the offered clipboard can be had in.
    pub(crate) formats: Vec<String>,
    seat: Option<wl_seat::WlSeat>,
    pub(crate) keyboard: Option<wl_keyboard::WlKeyboard>,
    pub(crate) pointer: Option<wl_pointer::WlPointer>,
    pub(crate) outputs: usize,
    /// The serial of the most recent input event, for requests that need one.
    pub(crate) serial: u32,
    pub(crate) events: Vec<Event>,
}

/// A Wayland client driven step by step from a test.
pub(crate) struct Client {
    connection: Connection,
    queue: EventQueue<ClientState>,
    pub(crate) state: ClientState,
    /// What the layer-shell surfaces it creates say they are.
    pub(crate) layer_name: &'static str,
}

/// A toplevel window belonging to a [`Client`].
/// What the compositor has told a client's surfaces about the screen. The
/// tests that look at this use one surface, so it is not kept per surface.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Told {
    /// The whole scale to draw buffers at.
    pub(crate) buffer_scale: Option<i32>,
    /// The exact scale, in units of 1/120, for a surface that asked.
    pub(crate) scale: Option<u32>,
    /// How many times a surface was told it is on an output.
    pub(crate) entered: u32,
}

pub(crate) struct Toplevel {
    pub(crate) surface: wl_surface::WlSurface,
    pub(crate) xdg_surface: xdg_surface::XdgSurface,
    pub(crate) role: xdg_toplevel::XdgToplevel,
}

impl Client {
    fn new(stream: UnixStream) -> Self {
        let connection = Connection::from_socket(stream).unwrap();
        let queue = connection.new_event_queue();
        connection.display().get_registry(&queue.handle(), ());
        Self {
            connection,
            queue,
            state: ClientState::default(),
            layer_name: "test",
        }
    }

    fn handle(&self) -> QueueHandle<ClientState> {
        self.queue.handle()
    }

    /// Reads and handles whatever the compositor has sent, without waiting.
    fn receive(&mut self) {
        if let Some(guard) = self.queue.prepare_read() {
            // Nothing to read is not an error here.
            let _ = guard.read();
        }
        self.queue.dispatch_pending(&mut self.state).unwrap();
    }

    /// Returns the events received so far and forgets them.
    pub(crate) fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.state.events)
    }

    /// The size and states of the most recent configure, forgetting the
    /// events received so far.
    pub(crate) fn last_configure(&mut self) -> ((i32, i32), Vec<xdg_toplevel::State>) {
        let configure = self
            .take_events()
            .into_iter()
            .rev()
            .find_map(|event| match event {
                Event::Configure { size, mut states } => {
                    states.sort_by_key(|state| u32::from(*state));
                    Some((size, states))
                }
                _ => None,
            });
        configure.unwrap()
    }

    /// Creates a surface with no role.
    pub(crate) fn surface(&self) -> wl_surface::WlSurface {
        self.state
            .compositor
            .as_ref()
            .unwrap()
            .create_surface(&self.handle(), ())
    }

    /// Creates a toplevel and makes its initial, content-less commit.
    pub(crate) fn toplevel(&self, app_id: &str, title: &str) -> Toplevel {
        let window = self.toplevel_uncommitted(app_id, title);
        window.surface.commit();
        window
    }

    /// Creates a layer-shell surface, such as a panel or a wallpaper, and
    /// makes its initial commit. A `size` of zero along an axis means "as
    /// wide (or tall) as the edges I am anchored to".
    pub(crate) fn layer(
        &self,
        shape: &LayerShape,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        self.layer_on(shape, None)
    }

    /// As [`Client::layer`], on an output of the client's choosing, counted
    /// in the order they appeared. Without one the compositor chooses.
    pub(crate) fn layer_on(
        &self,
        shape: &LayerShape,
        output: Option<usize>,
    ) -> (
        wl_surface::WlSurface,
        zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    ) {
        let surface = self.surface();
        let shell = self.state.layer_shell.as_ref().unwrap();
        let role = shell.get_layer_surface(
            &surface,
            output.map(|index| &self.state.screens[index]),
            shape.layer,
            self.layer_name.to_owned(),
            &self.handle(),
            (),
        );
        role.set_anchor(shape.anchor);
        role.set_size(shape.size.0, shape.size.1);
        role.set_exclusive_zone(shape.exclusive_zone);
        role.set_keyboard_interactivity(shape.keyboard);
        surface.commit();
        (surface, role)
    }

    /// Asks the compositor to negotiate who decorates a window.
    pub(crate) fn decoration(
        &self,
        window: &Toplevel,
    ) -> zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1 {
        let manager = self.state.decorations.as_ref().unwrap();
        manager.get_toplevel_decoration(&window.role, &self.handle(), ())
    }

    /// Creates a toplevel without committing it yet.
    pub(crate) fn toplevel_uncommitted(&self, app_id: &str, title: &str) -> Toplevel {
        let surface = self.surface();
        let wm_base = self.state.wm_base.as_ref().unwrap();
        let xdg_surface = wm_base.get_xdg_surface(&surface, &self.handle(), ());
        let toplevel = xdg_surface.get_toplevel(&self.handle(), ());
        toplevel.set_app_id(app_id.to_owned());
        toplevel.set_title(title.to_owned());
        Toplevel {
            surface,
            xdg_surface,
            role: toplevel,
        }
    }

    /// Gives a surface content: a buffer of one solid colour.
    pub(crate) fn fill(&self, surface: &wl_surface::WlSurface, size: (i32, i32), color: u32) {
        surface.attach(Some(&self.buffer(size, color)), 0, 0);
        surface.damage_buffer(0, 0, size.0, size.1);
        surface.commit();
    }

    /// Asks for the session to be locked.
    pub(crate) fn lock(&self) -> ext_session_lock_v1::ExtSessionLockV1 {
        let lockers = self.state.lockers.as_ref().unwrap();
        lockers.lock(&self.handle(), ())
    }

    /// Makes a surface the one shown while the session is locked.
    pub(crate) fn lock_surface(
        &self,
        lock: &ext_session_lock_v1::ExtSessionLockV1,
    ) -> wl_surface::WlSurface {
        self.lock_surface_on(lock, 0)
    }

    /// Makes a surface the one shown on one output while the session is
    /// locked. Outputs are counted in the order they appeared.
    pub(crate) fn lock_surface_on(
        &self,
        lock: &ext_session_lock_v1::ExtSessionLockV1,
        output: usize,
    ) -> wl_surface::WlSurface {
        let surface = self.surface();
        let output = &self.state.screens[output];
        lock.get_lock_surface(&surface, output, &self.handle(), ());
        surface
    }

    /// Asks to be told when the user has been away for `milliseconds`.
    pub(crate) fn watch_idle(&self, milliseconds: u32) {
        let idlers = self.state.idlers.as_ref().unwrap();
        idlers.get_idle_notification(milliseconds, self.seat(), &self.handle(), ());
    }

    /// Asks for the session not to go idle while `surface` is shown.
    pub(crate) fn inhibit_idle(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1 {
        let inhibitors = self.state.inhibitors.as_ref().unwrap();
        inhibitors.create_inhibitor(surface, &self.handle(), ())
    }

    /// Asks for the pointer to be drawn as `picture` while it is over this
    /// client, with `hotspot` the place in it that is the position, or to
    /// be hidden.
    pub(crate) fn set_cursor(&self, picture: Option<&wl_surface::WlSurface>, hotspot: (i32, i32)) {
        let pointer = self.state.pointer.as_ref().unwrap();
        pointer.set_cursor(self.state.serial, picture, hotspot.0, hotspot.1);
    }

    /// Asks for the pointer to be one of the named shapes while it is over
    /// this client.
    pub(crate) fn set_cursor_shape(&self, shape: wp_cursor_shape_device_v1::Shape) {
        let pointer = self.state.pointer.as_ref().unwrap();
        let shapes = self.state.shapes.as_ref().unwrap();
        let device = shapes.get_pointer(pointer, &self.handle(), ());
        device.set_shape(self.state.serial, shape);
    }

    /// Asks to be told the exact scale `surface` is shown at.
    pub(crate) fn watch_scale(&self, surface: &wl_surface::WlSurface) {
        let fractions = self.state.fractions.as_ref().unwrap();
        fractions.get_fractional_scale(surface, &self.handle(), ());
    }

    /// Fills `surface` with a buffer of `pixels`, to be shown at `size`:
    /// what an application does to draw sharply at a scale.
    pub(crate) fn fill_scaled(
        &self,
        surface: &wl_surface::WlSurface,
        pixels: (i32, i32),
        size: (i32, i32),
        color: u32,
    ) {
        let viewporter = self.state.viewporter.as_ref().unwrap();
        let viewport = viewporter.get_viewport(surface, &self.handle(), ());
        viewport.set_destination(size.0, size.1);
        self.fill(surface, pixels, color);
    }

    /// Gives a surface content of one colour (`0xAARRGGBB`) by way of a
    /// buffer that is a single pixel of it, stretched to `size`: what a
    /// toolkit does for a plain background.
    pub(crate) fn fill_plain(&self, surface: &wl_surface::WlSurface, size: (i32, i32), color: u32) {
        let viewporter = self.state.viewporter.as_ref().unwrap();
        let viewport = viewporter.get_viewport(surface, &self.handle(), ());
        viewport.set_destination(size.0, size.1);
        // Each channel is given as a share of all thirty-two bits.
        let [alpha, red, green, blue] = color
            .to_be_bytes()
            .map(|byte| u32::from(byte) * 0x0101_0101);
        let pixels = self.state.single_pixels.as_ref().unwrap();
        let buffer = pixels.create_u32_rgba_buffer(red, green, blue, alpha, &self.handle(), ());
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, 1, 1);
        surface.commit();
    }

    fn buffer(&self, (width, height): (i32, i32), color: u32) -> wl_buffer::WlBuffer {
        let pixels = usize::try_from(width * height).unwrap();
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(&color.to_le_bytes().repeat(pixels)).unwrap();
        let shm = self.state.shm.as_ref().unwrap();
        let pool = shm.create_pool(file.as_fd(), width * height * 4, &self.handle(), ());
        pool.create_buffer(
            0,
            width,
            height,
            width * 4,
            wl_shm::Format::Argb8888,
            &self.handle(),
            (),
        )
    }

    /// Attaches a filled subsurface to `parent` at `position`.
    pub(crate) fn subsurface(
        &self,
        parent: &wl_surface::WlSurface,
        position: (i32, i32),
        size: (i32, i32),
        color: u32,
    ) -> wl_surface::WlSurface {
        let surface = self.surface();
        let subcompositor = self.state.subcompositor.as_ref().unwrap();
        let subsurface = subcompositor.get_subsurface(&surface, parent, &self.handle(), ());
        subsurface.set_position(position.0, position.1);
        subsurface.set_desync();
        self.fill(&surface, size, color);
        parent.commit();
        surface
    }

    /// Creates a popup on `parent` and makes its initial commit. Returns the
    /// popup and the positioner used, for repositioning later.
    pub(crate) fn popup(
        &self,
        parent: &Toplevel,
        position: (i32, i32),
        size: (i32, i32),
    ) -> (
        wl_surface::WlSurface,
        xdg_popup::XdgPopup,
        xdg_positioner::XdgPositioner,
    ) {
        let wm_base = self.state.wm_base.as_ref().unwrap();
        let positioner = wm_base.create_positioner(&self.handle(), ());
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(position.0, position.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::TopLeft);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        let surface = self.surface();
        let xdg_surface = wm_base.get_xdg_surface(&surface, &self.handle(), ());
        let popup =
            xdg_surface.get_popup(Some(&parent.xdg_surface), &positioner, &self.handle(), ());
        surface.commit();
        (surface, popup, positioner)
    }

    /// Creates a popup on a layer-shell surface, such as a panel's menu,
    /// and makes its initial commit.
    pub(crate) fn layer_popup(
        &self,
        parent: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        position: (i32, i32),
        size: (i32, i32),
    ) -> (wl_surface::WlSurface, xdg_popup::XdgPopup) {
        let wm_base = self.state.wm_base.as_ref().unwrap();
        let positioner = wm_base.create_positioner(&self.handle(), ());
        positioner.set_size(size.0, size.1);
        positioner.set_anchor_rect(position.0, position.1, 1, 1);
        positioner.set_anchor(xdg_positioner::Anchor::TopLeft);
        positioner.set_gravity(xdg_positioner::Gravity::BottomRight);
        let surface = self.surface();
        let xdg_surface = wm_base.get_xdg_surface(&surface, &self.handle(), ());
        let popup = xdg_surface.get_popup(None, &positioner, &self.handle(), ());
        parent.get_popup(&popup);
        surface.commit();
        (surface, popup)
    }

    /// Makes `text` the primary selection, as selecting it with the mouse
    /// does.
    pub(crate) fn select(&mut self, text: &str) {
        text.clone_into(&mut self.state.selected);
        let source = self
            .state
            .primaries
            .as_ref()
            .unwrap()
            .create_source(&self.handle(), ());
        source.offer(TEXT.to_owned());
        self.state
            .primary_device
            .as_ref()
            .unwrap()
            .set_selection(Some(&source), self.state.serial);
    }

    /// Asks for the primary selection, as a middle click does. Returns
    /// where it will arrive, or `None` if nothing is selected.
    pub(crate) fn paste_selection(&self) -> Option<std::io::PipeReader> {
        let offer = self.state.primary.as_ref()?;
        let (reader, writer) = std::io::pipe().unwrap();
        offer.receive(TEXT.to_owned(), writer.as_fd());
        Some(reader)
    }

    /// Asks for an activation token: the right to hand focus to a window.
    /// `from_input` ties it to the last thing the user did in this client.
    /// The token arrives in [`ClientState::token`].
    pub(crate) fn request_token(&self, from_input: bool) {
        let activation = self.state.activation.as_ref().unwrap();
        let token = activation.get_activation_token(&self.handle(), ());
        if from_input {
            token.set_serial(self.state.serial, self.seat());
        }
        token.commit();
    }

    /// Asks for `surface`'s window to be focused, presenting `token`.
    pub(crate) fn activate(&self, token: &str, surface: &wl_surface::WlSurface) {
        self.state
            .activation
            .as_ref()
            .unwrap()
            .activate(token.to_owned(), surface);
    }

    /// Puts `text` on the clipboard, as an application does on Ctrl+C.
    pub(crate) fn copy(&mut self, text: &str) {
        text.clone_into(&mut self.state.copied);
        let source = self
            .state
            .clipboards
            .as_ref()
            .unwrap()
            .create_data_source(&self.handle(), ());
        source.offer(TEXT.to_owned());
        self.state
            .data_device
            .as_ref()
            .unwrap()
            .set_selection(Some(&source), self.state.serial);
    }

    /// Asks for the clipboard's text, as an application does on Ctrl+V.
    /// Returns where it will arrive, or `None` if the clipboard is empty.
    pub(crate) fn paste(&self) -> Option<std::io::PipeReader> {
        let offer = self.state.selection.as_ref()?;
        let (reader, writer) = std::io::pipe().unwrap();
        offer.receive(TEXT.to_owned(), writer.as_fd());
        Some(reader)
    }

    /// Limits where a surface accepts the pointer to one rectangle.
    pub(crate) fn input_region(
        &self,
        surface: &wl_surface::WlSurface,
        (x, y, width, height): (i32, i32, i32, i32),
    ) {
        let region = self
            .state
            .compositor
            .as_ref()
            .unwrap()
            .create_region(&self.handle(), ());
        region.add(x, y, width, height);
        surface.set_input_region(Some(&region));
        surface.commit();
    }

    /// Asks to be told when the next frame has been shown.
    pub(crate) fn request_frame(&self, surface: &wl_surface::WlSurface) {
        surface.frame(&self.handle(), Frame);
        surface.commit();
    }

    /// The seat, for requests that take one.
    pub(crate) fn seat(&self) -> &wl_seat::WlSeat {
        self.state.seat.as_ref().unwrap()
    }
}

/// A window as a taskbar sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Task {
    handle: listed_handle::ExtForeignToplevelHandleV1,
    pub(crate) app_id: String,
    pub(crate) title: String,
    pub(crate) closed: bool,
}

/// The format the test client copies and pastes.
const TEXT: &str = "text/plain;charset=utf-8";

/// How a layer-shell surface asks to be placed.
pub(crate) struct LayerShape {
    pub(crate) layer: zwlr_layer_shell_v1::Layer,
    pub(crate) anchor: zwlr_layer_surface_v1::Anchor,
    pub(crate) size: (u32, u32),
    pub(crate) exclusive_zone: i32,
    pub(crate) keyboard: zwlr_layer_surface_v1::KeyboardInteractivity,
}

/// Marks a `wl_callback` as a frame callback rather than a sync.
struct Frame;

impl Dispatch<wl_registry::WlRegistry, ()> for ClientState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            return;
        };
        match interface.as_str() {
            "wl_compositor" => state.compositor = Some(registry.bind(name, version, handle, ())),
            "wl_subcompositor" => {
                state.subcompositor = Some(registry.bind(name, version, handle, ()));
            }
            "wl_shm" => state.shm = Some(registry.bind(name, version, handle, ())),
            "xdg_wm_base" => state.wm_base = Some(registry.bind(name, version, handle, ())),
            "ext_foreign_toplevel_list_v1" => {
                let _: toplevel_list::ExtForeignToplevelListV1 =
                    registry.bind(name, version, handle, ());
            }
            "zwp_primary_selection_device_manager_v1" => {
                state.primaries = Some(registry.bind(name, version, handle, ()));
            }
            "xdg_activation_v1" => {
                state.activation = Some(registry.bind(name, version, handle, ()));
            }
            "wl_data_device_manager" => {
                state.clipboards = Some(registry.bind(name, version.min(3), handle, ()));
            }
            "zwlr_layer_shell_v1" => {
                state.layer_shell = Some(registry.bind(name, version.min(4), handle, ()));
            }
            "zxdg_decoration_manager_v1" => {
                state.decorations = Some(registry.bind(name, version, handle, ()));
            }
            "wp_fractional_scale_manager_v1" => {
                state.fractions = Some(registry.bind(name, version, handle, ()));
            }
            "wp_viewporter" => state.viewporter = Some(registry.bind(name, version, handle, ())),
            "wp_single_pixel_buffer_manager_v1" => {
                state.single_pixels = Some(registry.bind(name, version, handle, ()));
            }
            "wp_cursor_shape_manager_v1" => {
                state.shapes = Some(registry.bind(name, version, handle, ()));
            }
            "wl_seat" => state.seat = Some(registry.bind(name, version, handle, ())),
            "wl_output" => {
                state.screens.push(registry.bind(name, version, handle, ()));
                state.outputs += 1;
            }
            "ext_session_lock_manager_v1" => {
                state.lockers = Some(registry.bind(name, version, handle, ()));
            }
            "ext_idle_notifier_v1" => {
                state.idlers = Some(registry.bind(name, version, handle, ()));
            }
            "zwp_idle_inhibit_manager_v1" => {
                state.inhibitors = Some(registry.bind(name, version, handle, ()));
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.synced = true;
    }
}

impl Dispatch<wl_callback::WlCallback, Frame> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        _: &Frame,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        state.events.push(Event::FrameDone);
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for ClientState {
    fn event(
        state: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { .. } = event {
            state
                .keyboard
                .get_or_insert_with(|| seat.get_keyboard(handle, ()));
            let clipboards = state.clipboards.as_ref().unwrap();
            state
                .data_device
                .get_or_insert_with(|| clipboards.get_data_device(seat, handle, ()));
            let primaries = state.primaries.as_ref().unwrap();
            state
                .primary_device
                .get_or_insert_with(|| primaries.get_device(seat, handle, ()));
            state
                .pointer
                .get_or_insert_with(|| seat.get_pointer(handle, ()));
        }
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for ClientState {
    fn event(
        _: &mut Self,
        wm_base: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            wm_base.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, ()> for ClientState {
    fn event(
        _: &mut Self,
        surface: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            surface.ack_configure(serial);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure {
                width,
                height,
                states,
            } => {
                let states = states
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|raw| u32::from_ne_bytes(*raw))
                    .filter_map(|raw| xdg_toplevel::State::try_from(raw).ok())
                    .collect();
                state.events.push(Event::Configure {
                    size: (width, height),
                    states,
                });
            }
            xdg_toplevel::Event::Close => state.events.push(Event::Close),
            _ => {}
        }
    }
}

impl Dispatch<zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &zxdg_toplevel_decoration_v1::ZxdgToplevelDecorationV1,
        event: zxdg_toplevel_decoration_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_toplevel_decoration_v1::Event::Configure { mode } = event {
            let server_side = mode == WEnum::Value(zxdg_toplevel_decoration_v1::Mode::ServerSide);
            state.events.push(Event::Decoration { server_side });
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        surface: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                surface.ack_configure(serial);
                state.events.push(Event::LayerConfigure {
                    size: (width, height),
                });
            }
            zwlr_layer_surface_v1::Event::Closed => state.events.push(Event::LayerClosed),
            _ => {}
        }
    }
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::DataOffer { .. } => state.formats.clear(),
            wl_data_device::Event::Selection { id } => state.selection = id,
            _ => {}
        }
    }

    wayland_client::event_created_child!(ClientState, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wl_data_offer::WlDataOffer, ()),
    ]);
}

impl Dispatch<wl_data_offer::WlDataOffer, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            state.formats.push(mime_type);
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_source::Event::Send { fd, .. } = event {
            // Hand over what was copied and close the pipe by dropping it.
            std::fs::File::from(fd)
                .write_all(state.copied.as_bytes())
                .unwrap();
        }
    }
}

impl Dispatch<primary_device::ZwpPrimarySelectionDeviceV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &primary_device::ZwpPrimarySelectionDeviceV1,
        event: primary_device::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let primary_device::Event::Selection { id } = event {
            state.primary = id;
        }
    }

    wayland_client::event_created_child!(ClientState, primary_device::ZwpPrimarySelectionDeviceV1, [
        primary_device::EVT_DATA_OFFER_OPCODE => (primary_offer::ZwpPrimarySelectionOfferV1, ()),
    ]);
}

impl Dispatch<primary_source::ZwpPrimarySelectionSourceV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &primary_source::ZwpPrimarySelectionSourceV1,
        event: primary_source::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let primary_source::Event::Send { fd, .. } = event {
            std::fs::File::from(fd)
                .write_all(state.selected.as_bytes())
                .unwrap();
        }
    }
}

impl Dispatch<toplevel_list::ExtForeignToplevelListV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &toplevel_list::ExtForeignToplevelListV1,
        event: toplevel_list::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let toplevel_list::Event::Toplevel { toplevel } = event {
            state.taskbar.push(Task {
                handle: toplevel,
                app_id: String::new(),
                title: String::new(),
                closed: false,
            });
        }
    }

    wayland_client::event_created_child!(ClientState, toplevel_list::ExtForeignToplevelListV1, [
        toplevel_list::EVT_TOPLEVEL_OPCODE => (listed_handle::ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<listed_handle::ExtForeignToplevelHandleV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        handle: &listed_handle::ExtForeignToplevelHandleV1,
        event: listed_handle::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let task = state
            .taskbar
            .iter_mut()
            .find(|task| task.handle == *handle)
            .unwrap();
        match event {
            listed_handle::Event::Title { title } => task.title = title,
            listed_handle::Event::AppId { app_id } => task.app_id = app_id,
            listed_handle::Event::Closed => task.closed = true,
            _ => {}
        }
    }
}

impl Dispatch<xdg_activation_token_v1::XdgActivationTokenV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &xdg_activation_token_v1::XdgActivationTokenV1,
        event: xdg_activation_token_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_activation_token_v1::Event::Done { token } = event {
            state.token = Some(token);
        }
    }
}

impl Dispatch<xdg_popup::XdgPopup, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &xdg_popup::XdgPopup,
        event: xdg_popup::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_popup::Event::Configure {
                x,
                y,
                width,
                height,
            } => {
                state.events.push(Event::PopupConfigure {
                    position: (x, y),
                    size: (width, height),
                });
            }
            xdg_popup::Event::Repositioned { token } => {
                state.events.push(Event::PopupRepositioned(token));
            }
            xdg_popup::Event::PopupDone => state.events.push(Event::PopupDone),
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                state.serial = serial;
                state.events.push(Event::PointerEnter {
                    x: surface_x,
                    y: surface_y,
                });
            }
            wl_pointer::Event::Leave { .. } => state.events.push(Event::PointerLeave),
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                state.events.push(Event::PointerMotion {
                    x: surface_x,
                    y: surface_y,
                });
            }
            wl_pointer::Event::Button {
                serial,
                button,
                state: button_state,
                ..
            } => {
                state.serial = serial;
                let pressed = button_state == WEnum::Value(wl_pointer::ButtonState::Pressed);
                state.events.push(Event::PointerButton { button, pressed });
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                let vertical = axis == WEnum::Value(wl_pointer::Axis::VerticalScroll);
                state.events.push(Event::PointerScroll { vertical, value });
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { serial, .. } => {
                state.serial = serial;
                state.events.push(Event::KeyboardEnter);
            }
            wl_keyboard::Event::Leave { .. } => state.events.push(Event::KeyboardLeave),
            wl_keyboard::Event::Key {
                serial,
                key,
                state: key_state,
                ..
            } => {
                state.serial = serial;
                let pressed = key_state == WEnum::Value(wl_keyboard::KeyState::Pressed);
                state.events.push(Event::Key { code: key, pressed });
            }
            _ => {}
        }
    }
}

delegate_noop!(ClientState: wl_compositor::WlCompositor);
delegate_noop!(ClientState: wl_subcompositor::WlSubcompositor);
delegate_noop!(ClientState: wl_subsurface::WlSubsurface);
delegate_noop!(ClientState: wl_shm_pool::WlShmPool);
delegate_noop!(ClientState: wl_region::WlRegion);
delegate_noop!(ClientState: xdg_positioner::XdgPositioner);
delegate_noop!(ClientState: primary_manager::ZwpPrimarySelectionDeviceManagerV1);
delegate_noop!(ClientState: ignore primary_offer::ZwpPrimarySelectionOfferV1);
delegate_noop!(ClientState: xdg_activation_v1::XdgActivationV1);
delegate_noop!(ClientState: wl_data_device_manager::WlDataDeviceManager);
delegate_noop!(ClientState: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(ClientState: zxdg_decoration_manager_v1::ZxdgDecorationManagerV1);
delegate_noop!(ClientState: ignore wl_shm::WlShm);
delegate_noop!(ClientState: ignore wl_buffer::WlBuffer);
delegate_noop!(ClientState: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
delegate_noop!(ClientState: wp_viewporter::WpViewporter);
delegate_noop!(ClientState: wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1);
delegate_noop!(ClientState: ext_session_lock_manager_v1::ExtSessionLockManagerV1);
delegate_noop!(ClientState: ext_idle_notifier_v1::ExtIdleNotifierV1);
delegate_noop!(ClientState: zwp_idle_inhibit_manager_v1::ZwpIdleInhibitManagerV1);
delegate_noop!(ClientState: zwp_idle_inhibitor_v1::ZwpIdleInhibitorV1);

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_session_lock_v1::Event::Locked => state.events.push(Event::Locked),
            ext_session_lock_v1::Event::Finished => state.events.push(Event::LockRefused),
            _ => {}
        }
    }
}

impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        surface: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            surface.ack_configure(serial);
            state.events.push(Event::LockSize(width, height));
        }
    }
}

impl Dispatch<ext_idle_notification_v1::ExtIdleNotificationV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &ext_idle_notification_v1::ExtIdleNotificationV1,
        event: ext_idle_notification_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_idle_notification_v1::Event::Idled => state.events.push(Event::Idle),
            ext_idle_notification_v1::Event::Resumed => state.events.push(Event::Back),
            _ => {}
        }
    }
}

delegate_noop!(ClientState: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
delegate_noop!(ClientState: wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);
delegate_noop!(ClientState: wp_viewport::WpViewport);

impl Dispatch<wl_surface::WlSurface, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wl_surface::WlSurface,
        event: wl_surface::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::Enter { .. } => state.told.entered += 1,
            wl_surface::Event::PreferredBufferScale { factor } => {
                state.told.buffer_scale = Some(factor);
            }
            _ => {}
        }
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for ClientState {
    fn event(
        state: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.told.scale = Some(scale);
        }
    }
}
delegate_noop!(ClientState: ignore wl_output::WlOutput);

/// A theme with a rule after it that has every part change colour, and
/// come onto the screen, at once. A theme that says how long some part
/// takes outranks it for that part.
///
/// A theme that is broken is not read at all, and the built-in one, which
/// takes its time, is used: a test of that lets time pass.
fn still(theme: &str) -> String {
    format!("{theme}\n* {{ transition: none; }}\n")
}

/// Whether a colour on the screen is about half way from red to blue, as
/// something changing from the one to the other is in the middle of it.
pub(crate) fn half_way(color: u32) -> bool {
    let [_, red, _, blue] = color.to_be_bytes();
    (100..160).contains(&red) && (100..160).contains(&blue)
}

/// Whether a colour on the screen is somewhere between two others, and
/// neither of them, as something half there is over what is behind it.
pub(crate) fn between(color: u32, one: u32, other: u32) -> bool {
    let channels = color.to_be_bytes().into_iter();
    let ends = one.to_be_bytes().into_iter().zip(other.to_be_bytes());
    let within = channels
        .zip(ends)
        .all(|(channel, (one, other))| (one.min(other)..=one.max(other)).contains(&channel));
    within && color != one && color != other
}
