//! The notifications as a program: their place on the screen, the pointer,
//! and the time.
//!
//! Nothing here decides which notifications are showing or how they look;
//! that is [`Stack`]. This carries: what the bus hears goes in, pictures go
//! out, clicks come back, and programs are told what became of what they
//! sent.

use std::io;
use std::mem;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use omorfos_config::{Store, Watcher};
use omorfos_icons::Icons;
use omorfos_ipc::{Client as Ipc, Reply, Request};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Motion, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::{Bus, Event, Failure, Reason, Stack};

/// The left and right buttons of a mouse, as Linux numbers them.
const LEFT_BUTTON: u32 = 0x110;
const RIGHT_BUTTON: u32 = 0x111;

/// The notifications: a connection to the compositor as a Wayland client,
/// the service on the bus, and what is showing.
///
/// It does nothing by itself. Whoever has it calls [`Notifier::turn`] when
/// there may be something to do, or leaves it to [`Notifier::serve`].
pub struct Notifier {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
    bus: Bus,
    /// Where the compositor says what is wrong with the session, which is
    /// shown among the notifications. Absent outside a session that has
    /// it, where the compositor shows that itself.
    news: Option<Ipc>,
    /// Tells when the theme may have changed. Absent when the configuration
    /// directory cannot be watched, such as when there is none.
    watcher: Option<Watcher>,
}

impl Notifier {
    /// Notifications on the Wayland connection `wayland`, heard of over
    /// `bus`, themed and set up from the configuration in `config`, with
    /// the pictures that are named looked up in `icons`.
    ///
    /// `news` is a connection to the compositor's own socket, on which it
    /// is asked to say what is wrong with the session, so that it can be
    /// shown among the notifications. Without one, or if it cannot be
    /// asked, the compositor goes on showing that itself.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        bus: Bus,
        news: Option<Ipc>,
        config: &Path,
        icons: Icons,
    ) -> Result<Self, Failure> {
        let news = news.and_then(|mut news| {
            let asked = news.set_nonblocking(true);
            let asked = asked.and_then(|()| news.send(&Request::ShowProblems));
            asked.ok().map(|()| news)
        });
        let connection = Connection::from_socket(wayland);
        connection.map_err(Failure::from).map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            Self {
                connection,
                queue,
                shell: Shell::new(Store::open(config), icons),
                bus,
                news,
                watcher: Watcher::new(config).ok(),
            }
        })
    }

    /// Does whatever there is to do, without waiting, at the time `now`,
    /// counted from when the program started: takes in what programs sent,
    /// lets go of the notifications whose time is up, draws what changed,
    /// and tells programs what became of theirs.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor or the bus has gone. The
    /// program is of no use after that.
    pub fn turn(&mut self, now: Duration) -> Result<(), Failure> {
        // Whether reading with nothing to read counts as a failure depends
        // on how the library was built, so the outcome is not looked at
        // here. A real failure is kept by the connection and comes back
        // from everything after it, down to the flush that ends the turn.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        let handled = self.queue.dispatch_pending(&mut self.shell);
        let handled = handled.map(drop).map_err(Failure::from);
        // A notification on its way to another look is drawn again as it
        // goes.
        self.shell.stale |= self.shell.motion.due(now);
        handled
            .and_then(|()| self.learn(now))
            .and_then(|()| self.hear(now))
            .and_then(|()| {
                let shown = self.shell.show(&self.queue.handle(), now);
                shown.map_err(Failure::from)
            })
            .and_then(|()| self.connection.flush().map_err(Failure::from))
    }

    /// Takes in what the compositor has said is wrong with the session.
    fn learn(&mut self, now: Duration) -> Result<(), Failure> {
        let shell = &mut self.shell;
        let heard = self.news.as_mut().map_or(Ok(()), |news| {
            let mut said = std::iter::from_fn(|| news.try_receive().transpose());
            said.try_for_each(|reply| reply.map(|reply| shell.told(&reply, now)))
        });
        heard.map_err(Failure::from)
    }

    /// When there will next be something to do without anything having
    /// happened: when the next notification's time runs out, or one that
    /// is changing colour is due to be drawn again.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let gone = self.shell.stack.deadline();
        let drawn = self.shell.motion.deadline();
        gone.into_iter().chain(drawn).min()
    }

    /// Runs the notifications, asleep until there is something to do,
    /// until they cannot go on. Returns why.
    pub fn serve(&mut self) -> Failure {
        let started = Instant::now();
        loop {
            let now = started.elapsed();
            if let Err(failure) = self.turn(now) {
                return failure;
            }
            let mut sources = vec![
                PollFd::new(&self.connection, PollFlags::IN),
                PollFd::new(&self.bus, PollFlags::IN),
            ];
            let watcher = self.watcher.iter();
            sources.extend(watcher.map(|watcher| PollFd::new(watcher, PollFlags::IN)));
            let news = self.news.iter();
            sources.extend(news.map(|news| PollFd::new(news, PollFlags::IN)));
            let longest = self.deadline().map(|deadline| deadline.saturating_sub(now));
            let longest = longest.and_then(|longest| Timespec::try_from(longest).ok());
            // A wait that is interrupted is a wake-up like any other, and
            // anything worse shows in the turn that follows.
            let _ = poll(&mut sources, longest.as_ref());
        }
    }

    /// Takes in what programs sent and what time has done, and tells
    /// programs what is no longer showing.
    fn hear(&mut self, now: Duration) -> Result<(), Failure> {
        let shell = &mut self.shell;
        let mut lost = false;
        for event in self.bus.events() {
            match event {
                Event::Show(note) => {
                    let crowded = shell.stack.show(note, now);
                    shell.gone.extend(crowded.map(|id| (id, Reason::Expired)));
                }
                Event::Close(id) => {
                    let closed = shell.stack.close(id);
                    shell.gone.extend(closed.then_some((id, Reason::Closed)));
                }
                Event::Lost => lost = true,
            }
            shell.stale = true;
        }
        // What omorfos itself said goes too, and no program is told.
        let before = shell.stack.len();
        let expired = shell.stack.expire(now);
        shell.stale |= shell.stack.len() != before;
        shell
            .gone
            .extend(expired.into_iter().map(|id| (id, Reason::Expired)));

        let watcher = self.watcher.as_mut();
        let changed = watcher.map_or(Ok(false), Watcher::drain);
        let reloaded = changed.map(|changed| {
            if changed && shell.store.reload() {
                shell.settle();
            }
        });
        // A click is told of before the notification's going is.
        let mut clicked = mem::take(&mut shell.clicked).into_iter();
        let clicked = clicked.try_for_each(|id| self.bus.clicked(id));
        let mut gone = mem::take(&mut shell.gone).into_iter();
        let told = gone.try_for_each(|(id, reason)| self.bus.closed(id, reason));
        let told = clicked.and(told);
        let bus = if lost {
            Err("the bus that notifications are sent over has gone".into())
        } else {
            Ok(())
        };
        reloaded.map_err(Failure::from).and(told).and(bus)
    }
}

/// Shows notifications in the session whose Wayland socket is at `wayland`
/// and whose bus is at `address`, or the session's own if there is none
/// given, until it cannot go on. Returns why.
pub(crate) fn run(
    wayland: &Path,
    address: Option<&str>,
    socket: Option<&Path>,
    config: &Path,
    icons: Icons,
) -> Failure {
    let wayland = omorfos_shell::connect(wayland).map_err(Failure::from);
    let joined = wayland.and_then(|wayland| Bus::open(address).map(|bus| (wayland, bus)));
    // A session without the compositor's own socket shows what is wrong
    // itself.
    let news = socket.and_then(|socket| Ipc::connect(socket).ok());
    let notifier =
        joined.and_then(|(wayland, bus)| Notifier::new(wayland, bus, news, config, icons));
    match notifier {
        Ok(mut notifier) => notifier.serve(),
        Err(failure) => failure,
    }
}

/// What the Wayland connection's events act on.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    pointer: Option<wl_pointer::WlPointer>,
    /// The surface the notifications are on, while there are any.
    face: Option<Face>,
    stack: Stack,
    store: Store,
    text: Text,
    icons: Icons,
    /// Where on the surface the pointer is.
    over: (i32, i32),
    /// The notifications that were clicked and whose senders have yet to
    /// be told.
    clicked: Vec<u32>,
    /// The notifications that are no longer showing and that programs have
    /// yet to be told of, with why.
    gone: Vec<(u32, Reason)>,
    /// The notifications that are on their way from one look to another.
    motion: Motion,
    /// Whether what is showing may differ from what is on the screen.
    stale: bool,
}

/// The surface the notifications are on.
struct Face {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    fraction: wp_fractional_scale_v1::WpFractionalScaleV1,
    /// How large the compositor has been asked to make it.
    asked: (u32, u32),
    /// Whether the compositor has agreed to that size.
    ready: bool,
    /// How many pixels the screen has to a unit.
    scale: Scale,
    /// The picture on it, which the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
}

impl Face {
    /// Gives the surface back.
    fn close(self) {
        self.fraction.destroy();
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
        self.buffer.iter().for_each(wl_buffer::WlBuffer::destroy);
    }
}

impl Shell {
    fn new(store: Store, icons: Icons) -> Self {
        let usual = store.current().general.notifications.timeout.get();
        Self {
            compositor: None,
            shm: None,
            layers: None,
            viewporter: None,
            scales: None,
            pointer: None,
            face: None,
            stack: Stack::new(usual),
            store,
            text: Text::new(),
            icons,
            over: (-1, -1),
            clicked: Vec::new(),
            gone: Vec::new(),
            motion: Motion::new(),
            stale: false,
        }
    }

    /// Takes in a configuration that changed.
    fn settle(&mut self) {
        let usual = self.store.current().general.notifications.timeout.get();
        self.stack.set_usual(usual);
        self.stale = true;
    }

    /// Takes in something the compositor said on its own socket. Only the
    /// state of the session is of any use here, for what it says is wrong.
    fn told(&mut self, reply: &Reply, now: Duration) {
        if let Reply::State(state) = reply {
            self.stale |= self.stack.tell(&state.problems, &state.events, now);
        }
    }

    /// Takes in where the pointer now is.
    fn moved(&mut self, x: f64, y: f64) {
        self.over = (whole(x), whole(y));
        let theme = &self.store.current().theme;
        self.stale |= self.stack.point(self.over.0, self.over.1, theme);
    }

    /// Puts the notifications on the screen, takes the surface away when
    /// there are none, and draws them if they no longer show what they
    /// should.
    fn show(&mut self, handle: &QueueHandle<Self>, now: Duration) -> io::Result<()> {
        if !mem::take(&mut self.stale) {
            return Ok(());
        }
        if self.stack.is_empty() {
            self.face.take().into_iter().for_each(Face::close);
            return Ok(());
        }
        let globals = self.compositor.as_ref().zip(self.shm.as_ref());
        let globals = globals.zip(self.layers.as_ref());
        let globals = globals.zip(self.viewporter.as_ref().zip(self.scales.as_ref()));
        // Until the compositor has offered everything, there is nothing to
        // draw with. Each offer makes this worth trying again.
        let Some((((compositor, shm), layers), (viewporter, scales))) = globals else {
            return Ok(());
        };
        let theme = &self.store.current().theme;
        self.stack.fit(theme, &mut self.text);
        let size = self.stack.size(theme);
        let face = self.face.take().unwrap_or_else(|| {
            let surface = compositor.create_surface(handle, ());
            // In the top right-hand corner of the screen the compositor
            // chooses, which is the one the pointer is on, above
            // everything, and never with the keyboard.
            let layer = layers.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Overlay,
                "notification".to_owned(),
                handle,
                (),
            );
            let corner = zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Right;
            layer.set_anchor(corner);
            let viewport = viewporter.get_viewport(&surface, handle, ());
            let fraction = scales.get_fractional_scale(&surface, handle, ());
            Face {
                surface,
                layer,
                viewport,
                fraction,
                asked: (0, 0),
                ready: false,
                scale: Scale::ONE,
                buffer: None,
            }
        });
        let face = self.face.insert(face);
        if face.asked != size {
            // The compositor answers this by agreeing to the size, and
            // only then is there a surface of that size to draw on.
            face.layer.set_size(size.0, size.1);
            face.surface.commit();
            face.asked = size;
            face.ready = false;
        }
        if !face.ready {
            return Ok(());
        }
        let (text, icons) = (&mut self.text, &mut self.icons);
        self.motion.begin(now);
        let canvas = self
            .stack
            .paint(text, theme, face.scale, icons, &mut self.motion);
        self.motion.end();
        picture(shm, &canvas, handle).map(|buffer| {
            face.surface.attach(Some(&buffer), 0, 0);
            // However many pixels the picture has, it covers this much.
            face.viewport
                .set_destination(size.0.cast_signed(), size.1.cast_signed());
            face.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            face.surface.commit();
            // The picture before this one is let go of when the compositor
            // says it has finished with it.
            face.buffer = Some(buffer);
        })
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Shell {
    fn event(
        shell: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        // A registry also says when something goes. If that is the screen
        // the notifications are on, the compositor closes their surface,
        // which is heard of there.
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            shell.stale = true;
            match interface.as_str() {
                "wl_compositor" => {
                    shell.compositor = Some(registry.bind(name, version.min(4), handle, ()));
                }
                "wl_shm" => shell.shm = Some(registry.bind(name, 1, handle, ())),
                "zwlr_layer_shell_v1" => shell.layers = Some(registry.bind(name, 1, handle, ())),
                "wp_viewporter" => shell.viewporter = Some(registry.bind(name, 1, handle, ())),
                "wp_fractional_scale_manager_v1" => {
                    shell.scales = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_seat" => {
                    let _: wl_seat::WlSeat = registry.bind(name, version.min(5), handle, ());
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for Shell {
    fn event(
        shell: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        shell.stale = true;
        if let zwlr_layer_surface_v1::Event::Configure { serial, .. } = event {
            layer.ack_configure(serial);
            shell.face.iter_mut().for_each(|face| face.ready = true);
            return;
        }
        // The only other thing it says is that the compositor closed it,
        // as when its screen goes. What is showing is put up again on
        // another.
        shell.face.take().into_iter().for_each(Face::close);
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, ()> for Shell {
    fn event(
        shell: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The scale comes in parts of 120 to one. Told the one they
        // already have, the notifications have nothing to do over.
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event
            && let scale = Scale::new(f64::from(scale) / 120.0)
            && shell.face.iter().any(|face| face.scale != scale)
        {
            shell.face.iter_mut().for_each(|face| face.scale = scale);
            shell.stale = true;
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Shell {
    fn event(
        shell: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(capabilities),
        } = event
        {
            let pointing = capabilities.contains(wl_seat::Capability::Pointer);
            let pointer = shell.pointer.take();
            shell.pointer = pointer.or_else(|| pointing.then(|| seat.get_pointer(handle, ())));
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Shell {
    fn event(
        shell: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface_x,
                surface_y,
                ..
            }
            | wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => shell.moved(surface_x, surface_y),
            wl_pointer::Event::Leave { .. } => shell.stale |= shell.stack.leave(),
            wl_pointer::Event::Button {
                button: button @ (LEFT_BUTTON | RIGHT_BUTTON),
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                // Either button dismisses the notification. The left one
                // is also the click its sender may be waiting to hear of;
                // the right one is a way to be rid of it without that.
                let theme = &shell.store.current().theme;
                let dismissed = shell.stack.click(shell.over.0, shell.over.1, theme);
                let acted = dismissed.filter(|(_, acts)| *acts && button == LEFT_BUTTON);
                shell.clicked.extend(acted.map(|(id, _)| id));
                let gone = dismissed.map(|(id, _)| (id, Reason::Dismissed));
                shell.gone.extend(gone);
                shell.stale = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for Shell {
    fn event(
        _: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // All a buffer ever says is that the compositor has finished with
        // it, which it does once a newer picture has taken its place.
        buffer.destroy();
    }
}

delegate_noop!(Shell: wl_compositor::WlCompositor);
delegate_noop!(Shell: ignore wl_shm::WlShm);
delegate_noop!(Shell: wl_shm_pool::WlShmPool);
delegate_noop!(Shell: ignore wl_surface::WlSurface);
delegate_noop!(Shell: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Shell: wp_viewporter::WpViewporter);
delegate_noop!(Shell: wp_viewport::WpViewport);
delegate_noop!(Shell: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
