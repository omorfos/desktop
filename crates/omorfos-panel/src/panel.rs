//! The panel as a program: a bar on each screen, kept up to date.
//!
//! Nothing here decides what a bar looks like or what a click means; that
//! is [`Bar`]. This carries: what the compositor says goes in, pictures go
//! out, clicks go back.

use std::error::Error;
use std::io;
use std::mem;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use omorfos_config::{Store, Theme, Watcher};
use omorfos_ipc::{Client as Ipc, Reply, Request, State};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Motion, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::Bar;

/// Why the panel could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// The panel: a connection to the compositor as a Wayland client, two to
/// its own socket, and a bar for each screen.
///
/// It does nothing by itself. Whoever has it calls [`Panel::turn`] when
/// there may be something to do, or leaves it to [`Panel::serve`].
pub struct Panel {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
    /// The compositor's news of the session.
    news: Ipc,
    /// Tells when the theme may have changed. Absent when the configuration
    /// directory cannot be watched, such as when there is none.
    watcher: Option<Watcher>,
}

impl Panel {
    /// A panel on the Wayland connection `wayland`, themed from the
    /// configuration in `config`.
    ///
    /// `news` and `commands` are two connections to the compositor's own
    /// socket. The first is given over to hearing of the session, and what
    /// clicks ask for goes down the second.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the connections cannot be used.
    pub fn new(
        wayland: UnixStream,
        mut news: Ipc,
        commands: Ipc,
        config: &Path,
    ) -> Result<Self, Failure> {
        let subscribed = news.send(&Request::Subscribe);
        let prepared = subscribed
            .and_then(|()| news.set_nonblocking(true))
            .and_then(|()| commands.set_nonblocking(true));
        let connection = prepared.map_err(Failure::from).and_then(|()| {
            let connection = Connection::from_socket(wayland);
            connection.map_err(Failure::from)
        });
        connection.map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            Self {
                connection,
                queue,
                shell: Shell::new(Store::open(config), commands),
                news,
                watcher: Watcher::new(config).ok(),
            }
        })
    }

    /// Does whatever there is to do, without waiting: hears what the
    /// compositor has said, draws the bars that changed, and sends what
    /// there is to send.
    ///
    /// `time` is what the clock on each bar is to read, which is for
    /// whoever calls this to know. Nothing is drawn again while it stays
    /// the same. `now` is how long the panel has been running, which is
    /// what a button changing colour goes by.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone or stopped making
    /// sense. The panel is of no use after that.
    pub fn turn(&mut self, time: &str, now: Duration) -> Result<(), Failure> {
        self.shell.timed(time);
        self.shell.now = now;
        // A button on its way to another look is drawn again as it goes.
        self.shell.stale |= self.deadline().is_some_and(|due| due <= now);
        self.hear()
            .and_then(|()| self.learn().map_err(Failure::from))
            .and_then(|()| {
                let shown = self.shell.show(&self.queue.handle());
                shown.map_err(Failure::from)
            })
            .and_then(|()| self.connection.flush().map_err(Failure::from))
    }

    /// Runs the panel, asleep until there is something to do, until it
    /// cannot go on. Returns why.
    pub fn serve(&mut self) -> Failure {
        let started = Instant::now();
        loop {
            let (time, changes) = omorfos_shell::clock::now();
            let now = started.elapsed();
            if let Err(failure) = self.turn(&time, now) {
                return failure;
            }
            let drawn = self.deadline().map(|due| due.saturating_sub(now));
            let soonest = [Some(changes), drawn].into_iter().flatten().min();
            self.wait(soonest.unwrap_or(changes));
        }
    }

    /// When there will next be something to do without anything having
    /// happened, other than the time changing: when a button that is
    /// changing colour is due to be drawn again.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let faces = self
            .shell
            .screens
            .iter()
            .filter_map(|screen| screen.face.as_ref());
        faces.filter_map(|face| face.motion.deadline()).min()
    }

    /// Sleeps until one of the connections has something to read, the
    /// configuration changes, or `longest` has passed.
    fn wait(&self, longest: Duration) {
        let readable = PollFlags::IN;
        let mut sources = vec![
            PollFd::new(&self.connection, readable),
            PollFd::new(&self.news, readable),
            PollFd::new(&self.shell.commands, readable),
        ];
        let watcher = self.watcher.iter();
        sources.extend(watcher.map(|watcher| PollFd::new(watcher, readable)));
        // A wait that is interrupted is a wake-up like any other, and
        // anything worse shows in the turn that follows.
        let longest = Timespec::try_from(longest).ok();
        let _ = poll(&mut sources, longest.as_ref());
    }

    /// Reads what the compositor sent as a Wayland server and acts on it.
    fn hear(&mut self) -> Result<(), Failure> {
        // Whether reading with nothing to read counts as a failure depends
        // on how the library was built, so the outcome is not looked at
        // here. A real failure is kept by the connection and comes back
        // from everything after it, down to the flush that ends the turn.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        let handled = self.queue.dispatch_pending(&mut self.shell);
        handled.map(drop).map_err(Failure::from)
    }

    /// Takes in what arrived on the compositor's own socket, and the theme
    /// if it changed.
    fn learn(&mut self) -> io::Result<()> {
        let shell = &mut self.shell;
        let mut news = std::iter::from_fn(|| self.news.try_receive().transpose());
        let heard = news.try_for_each(|reply| reply.map(|reply| shell.told(reply)));
        // The answers to what clicks asked for. A window that had gone by
        // the time it was asked for is nothing to do anything about.
        let mut answers = std::iter::from_fn(|| shell.commands.try_receive().transpose());
        let answered = answers.try_for_each(|answer| answer.map(drop));
        let sent = mem::replace(&mut shell.sent, Ok(()));
        let watcher = self.watcher.as_mut();
        let changed = watcher.map_or(Ok(false), Watcher::drain);
        heard.and(answered).and(sent).and(changed).map(|changed| {
            if changed && shell.store.reload() {
                shell.rethemed();
            }
        })
    }
}

/// Runs a panel in the session whose Wayland socket is at `wayland` and
/// whose own socket is at `socket`, themed from `config`, until it cannot
/// go on. Returns why.
pub(crate) fn run(wayland: &Path, socket: &Path, config: &Path) -> Failure {
    let streams = omorfos_shell::connect(wayland).and_then(|wayland| {
        let news = Ipc::connect(socket);
        let both = news.and_then(|news| Ipc::connect(socket).map(|commands| (news, commands)));
        both.map(|(news, commands)| (wayland, news, commands))
    });
    let panel = streams
        .map_err(Failure::from)
        .and_then(|(wayland, news, commands)| Panel::new(wayland, news, commands, config));
    match panel {
        Ok(mut panel) => panel.serve(),
        Err(failure) => failure,
    }
}

/// What the Wayland connection's events act on: the screens and their
/// bars, and what is needed to draw them.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    pointer: Option<wl_pointer::WlPointer>,
    screens: Vec<Screen>,
    /// The screen whose bar the pointer is over, and how far along it.
    over: Option<(u32, f64)>,
    /// What the compositor last said of the session.
    state: State,
    /// What the clocks read.
    time: String,
    store: Store,
    text: Text,
    /// Where what clicks ask for is sent.
    commands: Ipc,
    /// Whether everything asked for so far could be sent.
    sent: io::Result<()>,
    /// How long the panel has been running, as of this turn.
    now: Duration,
    /// Whether anything happened that may change what a bar shows.
    stale: bool,
}

/// A screen, by the number the compositor announced it under.
struct Screen {
    global: u32,
    /// What the compositor calls it, once it has said.
    name: String,
    /// What its bar is put on.
    output: wl_output::WlOutput,
    /// Its bar, once there is everything needed to make one.
    face: Option<Face>,
}

/// A bar's surface on a screen.
struct Face {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    fraction: wp_fractional_scale_v1::WpFractionalScaleV1,
    /// How wide the compositor made it, in units of layout. Nothing until
    /// it has said.
    width: u32,
    /// How many pixels the screen has to a unit.
    scale: Scale,
    /// What is on it now.
    shown: Option<(Bar, Scale)>,
    /// The buttons that are on their way from one look to another.
    motion: Motion,
    /// The picture of that, which the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
}

/// What it takes to put a bar on a screen.
/// What the bar says of the radios: that the machine is in aeroplane mode,
/// all of them off; or which kinds are off; or nothing while none is.
fn radios(radios: &[omorfos_ipc::Radio]) -> String {
    if !radios.is_empty() && radios.iter().all(|radio| radio.off) {
        return "Aeroplane mode".to_owned();
    }
    let off: Vec<&str> = radios
        .iter()
        .filter(|radio| radio.off)
        .map(omorfos_ipc::Radio::called)
        .collect();
    match off.as_slice() {
        [] => String::new(),
        [one] => format!("{one} off"),
        [first @ .., last] => format!("{} and {last} off", first.join(", ")),
    }
}

/// What the bar says while the session is kept awake, as the user asked:
/// until when, if it ends. Nothing while it is not.
fn awake(awake: Option<omorfos_ipc::Awake>) -> String {
    let until = awake.map(|awake| awake.until.and_then(omorfos_shell::clock::at));
    match until {
        None => String::new(),
        Some(None) => "Awake".to_owned(),
        Some(Some(at)) => format!("Awake until {at}"),
    }
}

struct Tools<'a> {
    compositor: &'a wl_compositor::WlCompositor,
    shm: &'a wl_shm::WlShm,
    layers: &'a zwlr_layer_shell_v1::ZwlrLayerShellV1,
    viewporter: &'a wp_viewporter::WpViewporter,
    scales: &'a wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    handle: &'a QueueHandle<Shell>,
    state: &'a State,
    time: &'a str,
    /// What says the session is kept awake, while it is.
    awake: &'a str,
    /// What says which radios are off, while any is.
    radios: &'a str,
    now: Duration,
    theme: &'a Theme,
    text: &'a mut Text,
}

impl Shell {
    fn new(store: Store, commands: Ipc) -> Self {
        Self {
            compositor: None,
            shm: None,
            layers: None,
            viewporter: None,
            scales: None,
            pointer: None,
            screens: Vec::new(),
            over: None,
            state: State::default(),
            time: String::new(),
            store,
            text: Text::new(),
            commands,
            sent: Ok(()),
            now: Duration::ZERO,
            stale: false,
        }
    }

    /// Takes in a message from the compositor's news.
    fn told(&mut self, reply: Reply) {
        // Nothing but the state is expected, and nothing else is of use.
        if let Reply::State(state) = reply {
            self.state = state;
            self.stale = true;
        }
    }

    /// Takes in what the time is.
    fn timed(&mut self, time: &str) {
        if self.time != time {
            time.clone_into(&mut self.time);
            self.stale = true;
        }
    }

    /// Takes the bars down, to be put up again as the theme now has them.
    fn rethemed(&mut self) {
        let faces = self
            .screens
            .iter_mut()
            .filter_map(|screen| screen.face.take());
        faces.for_each(Face::close);
        self.stale = true;
    }

    /// Lets go of a screen that is no longer there.
    fn forget(&mut self, global: u32) {
        let gone = self
            .screens
            .extract_if(.., |screen| screen.global == global);
        gone.filter_map(|screen| screen.face).for_each(Face::close);
    }

    /// The bar's surface on a screen, if it has one.
    fn faces(&mut self, global: u32) -> impl Iterator<Item = &mut Face> {
        let screens = self.screens.iter_mut();
        let screens = screens.filter(move |screen| screen.global == global);
        screens.filter_map(|screen| screen.face.as_mut())
    }

    /// Notes how many pixels a screen has to a unit. Returns whether that
    /// is news.
    fn scaled(&mut self, global: u32, scale: Scale) -> bool {
        self.faces(global).fold(false, |changed, face| {
            let before = mem::replace(&mut face.scale, scale);
            changed || before != scale
        })
    }

    /// Asks the compositor for what the button under the pointer stands
    /// for.
    fn click(&mut self) {
        let request = self.over.and_then(|(global, along)| {
            let screens = self.screens.iter();
            let screens = screens.filter(|screen| screen.global == global);
            let faces = screens.filter_map(|screen| screen.face.as_ref());
            let mut bars = faces.filter_map(|face| face.shown.as_ref());
            bars.find_map(|(bar, _)| bar.click(whole(along)))
        });
        let sent = request.map_or(Ok(()), |request| self.commands.send(&request));
        self.sent = mem::replace(&mut self.sent, Ok(())).and(sent);
    }

    /// Puts a bar on every screen that has none, and draws those that no
    /// longer show what they should.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        if !mem::take(&mut self.stale) {
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
        let awake = awake(self.state.awake);
        let radios = radios(&self.state.radios);
        let mut tools = Tools {
            compositor,
            shm,
            layers,
            viewporter,
            scales,
            handle,
            state: &self.state,
            time: &self.time,
            awake: &awake,
            radios: &radios,
            now: self.now,
            theme: &self.store.current().theme,
            text: &mut self.text,
        };
        let mut screens = self.screens.iter_mut();
        screens.try_for_each(|screen| {
            let face = screen.face.take();
            let face = face.unwrap_or_else(|| Face::open(&tools, screen));
            let face = screen.face.insert(face);
            face.draw(&mut tools, &screen.name)
        })
    }
}

impl Face {
    /// Asks for a surface along the bottom of a screen, as tall as the
    /// theme makes a bar and with that much of the screen kept clear of
    /// windows.
    fn open(tools: &Tools<'_>, screen: &Screen) -> Self {
        let handle = tools.handle;
        let height = Bar::thickness(tools.theme);
        let surface = tools.compositor.create_surface(handle, ());
        let layer = tools.layers.get_layer_surface(
            &surface,
            Some(&screen.output),
            zwlr_layer_shell_v1::Layer::Top,
            "panel".to_owned(),
            handle,
            screen.global,
        );
        let edges = zwlr_layer_surface_v1::Anchor::Bottom
            | zwlr_layer_surface_v1::Anchor::Left
            | zwlr_layer_surface_v1::Anchor::Right;
        layer.set_anchor(edges);
        layer.set_size(0, height);
        layer.set_exclusive_zone(height.cast_signed());
        let viewport = tools.viewporter.get_viewport(&surface, handle, ());
        let scales = tools.scales;
        let fraction = scales.get_fractional_scale(&surface, handle, screen.global);
        // The compositor answers this with how wide the bar is to be.
        surface.commit();
        Self {
            surface,
            layer,
            viewport,
            fraction,
            width: 0,
            scale: Scale::ONE,
            shown: None,
            motion: Motion::new(),
            buffer: None,
        }
    }

    /// Gives the surface back.
    fn close(self) {
        self.fraction.destroy();
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
        self.buffer.iter().for_each(wl_buffer::WlBuffer::destroy);
    }

    /// Draws the bar of the screen called `name`, unless it already shows
    /// just that.
    fn draw(&mut self, tools: &mut Tools<'_>, name: &str) -> io::Result<()> {
        if self.width == 0 {
            return Ok(());
        }
        let (state, times) = (tools.state, (tools.time, tools.awake, tools.radios));
        let bar = Bar::new(state, name, self.width, times, tools.theme, tools.text);
        let wanted = (bar, self.scale);
        // The same bar is drawn again while a button on it changes colour.
        let again = self.motion.due(tools.now);
        if self.shown.as_ref() == Some(&wanted) && !again {
            return Ok(());
        }
        let (bar, scale) = &wanted;
        self.motion.begin(tools.now);
        let canvas = bar.paint(tools.text, tools.theme, *scale, &mut self.motion);
        self.motion.end();
        let size = (self.width.cast_signed(), bar.height().cast_signed());
        picture(tools.shm, &canvas, tools.handle).map(|buffer| {
            self.surface.attach(Some(&buffer), 0, 0);
            // However many pixels the picture has, it covers this much.
            self.viewport.set_destination(size.0, size.1);
            self.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            self.surface.commit();
            // The picture before this one is let go of when the compositor
            // says it has finished with it.
            self.buffer = Some(buffer);
            self.shown = Some(wanted);
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
        if let wl_registry::Event::GlobalRemove { name } = event {
            shell.forget(name);
        }
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
                "zwlr_layer_shell_v1" => {
                    shell.layers = Some(registry.bind(name, 1, handle, ()));
                }
                "wp_viewporter" => shell.viewporter = Some(registry.bind(name, 1, handle, ())),
                "wp_fractional_scale_manager_v1" => {
                    shell.scales = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_seat" => {
                    let _: wl_seat::WlSeat = registry.bind(name, version.min(5), handle, ());
                }
                "wl_output" => shell.screens.push(Screen {
                    global: name,
                    name: String::new(),
                    // From its fourth version an output says what it is
                    // called.
                    output: registry.bind(name, version.min(4), handle, name),
                    face: None,
                }),
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for Shell {
    fn event(
        shell: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            let screens = shell.screens.iter_mut();
            let named = screens.filter(|screen| screen.global == *global);
            named.for_each(|screen| screen.name.clone_from(&name));
            shell.stale = true;
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, u32> for Shell {
    fn event(
        shell: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure { serial, width, .. } = event {
            layer.ack_configure(serial);
            shell.faces(*global).for_each(|face| face.width = width);
            shell.stale = true;
            return;
        }
        // The only other thing it says is that the compositor closed it,
        // which it does when the screen goes.
        shell.forget(*global);
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, u32> for Shell {
    fn event(
        shell: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The scale comes in parts of 120 to one. Told the one it already
        // has, a bar has nothing to do over.
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event
            && shell.scaled(*global, Scale::new(f64::from(scale) / 120.0))
        {
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
                surface, surface_x, ..
            } => {
                let mut screens = shell.screens.iter();
                let entered = screens.find(|screen| {
                    let face = screen.face.as_ref();
                    face.is_some_and(|face| face.surface == surface)
                });
                shell.over = entered.map(|screen| (screen.global, surface_x));
            }
            wl_pointer::Event::Motion { surface_x, .. } => {
                shell.over = shell.over.map(|(global, _)| (global, surface_x));
            }
            wl_pointer::Event::Leave { .. } => shell.over = None,
            wl_pointer::Event::Button {
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => shell.click(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_says_which_radios_are_off_and_aeroplane_mode_when_all_are() {
        let radio = |kind: &str, off| omorfos_ipc::Radio {
            kind: kind.to_owned(),
            off,
            switched: false,
        };
        assert_eq!(radios(&[]), "");
        assert_eq!(
            radios(&[radio("wlan", false), radio("bluetooth", false)]),
            ""
        );
        assert_eq!(
            radios(&[radio("wlan", false), radio("bluetooth", true)]),
            "Bluetooth off"
        );
        let three = [
            radio("wlan", true),
            radio("bluetooth", true),
            radio("nfc", true),
            radio("gps", false),
        ];
        assert_eq!(radios(&three), "Wi-Fi, Bluetooth and NFC off");
        assert_eq!(
            radios(&[radio("wlan", true), radio("bluetooth", true)]),
            "Aeroplane mode"
        );
    }

    #[test]
    fn the_bar_says_the_session_is_kept_awake_and_until_when() {
        use omorfos_ipc::Awake;

        assert_eq!(awake(None), "");
        assert_eq!(awake(Some(Awake { until: None })), "Awake");
        let until = awake(Some(Awake {
            until: Some(1_761_400_800),
        }));
        assert!(until.starts_with("Awake until "), "{until}");
        let never = Some(Awake {
            until: Some(u64::MAX),
        });
        assert_eq!(awake(never), "Awake", "a time no calendar has is not said");
    }
}
