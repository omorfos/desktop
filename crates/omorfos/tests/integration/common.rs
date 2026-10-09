//! Test support: a compositor session and a scriptable Wayland client.
#![expect(
    clippy::unwrap_used,
    reason = "test support code fails the test on any error"
)]

use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use omorfos::Compositor;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{Bind, ExportMem};
use smithay::reexports::pixman::{FormatCode, Image};
use smithay::reexports::wayland_server::Display;
use smithay::utils::{Rectangle, Transform};
use tempfile::TempDir;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer,
    wl_data_source, wl_keyboard, wl_output, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
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
    pub(crate) const SHIFT: Keycode = Keycode::new(50);
    pub(crate) const ALT: Keycode = Keycode::new(64);
    pub(crate) const X: Keycode = Keycode::new(53);
    pub(crate) const SUPER: Keycode = Keycode::new(133);
}

/// A compositor with a software-rendered output and its own configuration
/// directory.
pub(crate) struct Session {
    display: Display<Compositor>,
    pub(crate) compositor: Compositor,
    renderer: PixmanRenderer,
    image: Image<'static, 'static>,
    tracker: OutputDamageTracker,
    pub(crate) config: TempDir,
    /// The time the compositor is told it is. Tests advance it by hand.
    pub(crate) now: Instant,
}

/// What a frame looked like.
pub(crate) struct Picture {
    pixels: Vec<u8>,
    /// Whether anything had to be redrawn.
    pub(crate) damaged: bool,
}

impl Picture {
    /// The colour at a position, as `0xAARRGGBB`.
    pub(crate) fn at(&self, x: i32, y: i32) -> u32 {
        let offset = usize::try_from((y * WIDTH + x) * 4).unwrap();
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
        Self::in_directory(config, None)
    }

    /// Starts a compositor reading configuration from `directory`, or from
    /// the temporary directory itself when `None`.
    pub(crate) fn in_directory(config: TempDir, directory: Option<&Path>) -> Self {
        let display = Display::new().unwrap();
        let directory = directory.unwrap_or(config.path()).to_owned();
        let compositor = Compositor::new(
            display.handle(),
            directory,
            (WIDTH, HEIGHT),
            Transform::Normal,
        )
        .unwrap();
        let tracker = OutputDamageTracker::from_output(compositor.output());
        let width = usize::try_from(WIDTH).unwrap();
        let height = usize::try_from(HEIGHT).unwrap();
        Self {
            display,
            compositor,
            renderer: PixmanRenderer::new().unwrap(),
            image: Image::new(FormatCode::A8R8G8B8, width, height, true).unwrap(),
            tracker,
            config,
            now: Instant::now(),
        }
    }

    /// Writes a configuration file.
    pub(crate) fn write_config(&self, name: &str, contents: &str) {
        std::fs::write(self.config.path().join(name), contents).unwrap();
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
        self.compositor.listen_ipc(&path).unwrap();
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
        self.compositor.insert_client(ours).unwrap();
        let mut client = Client::new(theirs);
        self.roundtrip(&mut client);
        self.roundtrip(&mut client);
        client
    }

    /// Runs the compositor's side once: handle requests, update, reply.
    pub(crate) fn step(&mut self) {
        self.display.dispatch_clients(&mut self.compositor).unwrap();
        self.compositor.refresh(self.now);
        self.display.flush_clients().unwrap();
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
        let mut target = self.renderer.bind(&mut self.image).unwrap();
        let result = self
            .compositor
            .render(&mut self.renderer, &mut target, &mut self.tracker, 1)
            .unwrap();
        let damaged = result.damage.is_some();
        let region = Rectangle::from_size((WIDTH, HEIGHT).into());
        let mapping = self
            .renderer
            .copy_framebuffer(&target, region, Fourcc::Argb8888)
            .unwrap();
        let pixels = self.renderer.map_texture(&mapping).unwrap().to_vec();
        Picture { pixels, damaged }
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
}

/// A toplevel window belonging to a [`Client`].
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
        let surface = self.surface();
        let shell = self.state.layer_shell.as_ref().unwrap();
        let role = shell.get_layer_surface(
            &surface,
            None,
            shape.layer,
            "test".to_owned(),
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
            "wl_data_device_manager" => {
                state.clipboards = Some(registry.bind(name, version.min(3), handle, ()));
            }
            "zwlr_layer_shell_v1" => {
                state.layer_shell = Some(registry.bind(name, version.min(4), handle, ()));
            }
            "zxdg_decoration_manager_v1" => {
                state.decorations = Some(registry.bind(name, version, handle, ()));
            }
            "wl_seat" => state.seat = Some(registry.bind(name, version, handle, ())),
            "wl_output" => {
                let _: wl_output::WlOutput = registry.bind(name, version, handle, ());
                state.outputs += 1;
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
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            surface.ack_configure(serial);
            state.events.push(Event::LayerConfigure {
                size: (width, height),
            });
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
delegate_noop!(ClientState: wl_data_device_manager::WlDataDeviceManager);
delegate_noop!(ClientState: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(ClientState: zxdg_decoration_manager_v1::ZxdgDecorationManagerV1);
delegate_noop!(ClientState: ignore wl_shm::WlShm);
delegate_noop!(ClientState: ignore wl_buffer::WlBuffer);
delegate_noop!(ClientState: ignore wl_surface::WlSurface);
delegate_noop!(ClientState: ignore wl_output::WlOutput);
