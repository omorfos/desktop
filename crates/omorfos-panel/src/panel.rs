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
use omorfos_i18n::{PANEL, Value, Words};
use omorfos_ipc::{Client as Ipc, Reply, Request, State, Steady};
use omorfos_shell::month::{self, Month};
use omorfos_shell::popover::{Hanging, Popovers, Tip};
use omorfos_shell::sheet::Sheet;
use omorfos_shell::tooltip::Resting;
use omorfos_shell::whole;
use omorfos_ui::{Area, Motion, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_region, wl_registry, wl_seat,
    wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{
    Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_dispatch, delegate_noop,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::shell::client::{xdg_popup, xdg_positioner, xdg_surface, xdg_wm_base};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::{Bar, Click, Hint};

/// Why the panel could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// Escape, and the arrows left and right, as Linux numbers keys, which is
/// the same on every layout.
const ESCAPE: u32 = 1;
const LEFT: u32 = 105;
const RIGHT: u32 = 106;

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
    news: Steady,
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
    /// clicks ask for goes down the second. Each is made again at `socket`
    /// once it closes, where that is given, as the compositor is started
    /// anew under the relay; otherwise its closing is the end of the panel.
    ///
    /// # Errors
    ///
    /// Returns an error if any of the connections cannot be used.
    pub fn new(
        wayland: UnixStream,
        (news, commands): (Ipc, Ipc),
        config: &Path,
        socket: Option<&Path>,
    ) -> Result<Self, Failure> {
        let steady =
            |client, first| Steady::new(client, first, socket.map(Path::to_owned), Duration::ZERO);
        let news = steady(news, Some(Request::Subscribe));
        let both = news.and_then(|news| steady(commands, None).map(|commands| (news, commands)));
        let connection = both.map_err(Failure::from).and_then(|both| {
            let connection = Connection::from_socket(wayland);
            connection
                .map(|connection| (connection, both))
                .map_err(Failure::from)
        });
        connection.map(|(connection, (news, commands))| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            let store = Store::open(config);
            let watcher = Watcher::of(&store).ok();
            Self {
                connection,
                queue,
                shell: Shell::new(store, commands),
                news,
                watcher,
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
        // So is a tooltip that has been waited for long enough.
        self.shell.stale |= self.shell.resting.at(now);
        self.hear()
            .and_then(|()| self.learn().map_err(Failure::from))
            .and_then(|()| {
                let shown = self.shell.show(&self.queue.handle());
                shown.map_err(Failure::from)
            })
            .and_then(|()| self.connection.flush().map_err(Failure::from))
    }

    /// Says what day it is, which the month opens on: for whoever calls
    /// [`Panel::turn`] to know, as the time is.
    pub fn date(&mut self, today: month::Date) {
        self.shell.today = today;
    }

    /// Runs the panel, asleep until there is something to do, until it
    /// cannot go on. Returns why.
    pub fn serve(&mut self) -> Failure {
        let started = Instant::now();
        loop {
            let (time, changes) = omorfos_shell::clock::now();
            self.date(omorfos_shell::clock::today());
            let now = started.elapsed();
            if let Err(failure) = self.turn(&time, now) {
                return failure;
            }
            let drawn = self.deadline().map(|due| due.saturating_sub(now));
            // While the compositor is away, until it is tried again.
            let again = [self.news.until(now), self.shell.commands.until(now)];
            let soonest = [Some(changes), drawn].into_iter().chain(again);
            let soonest = soonest.flatten().min();
            self.wait(soonest.unwrap_or(changes));
        }
    }

    /// When there will next be something to do without anything having
    /// happened, other than the time changing: when a button that is
    /// changing colour is due to be drawn again, a tooltip to be shown, or
    /// the next step of day turning into night.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let faces = self
            .shell
            .screens
            .iter()
            .filter_map(|screen| screen.face.as_ref());
        let moving = faces.filter_map(|face| face.motion.deadline());
        let tip = self.shell.resting.deadline();
        moving.chain(tip).chain(self.shell.store.turns()).min()
    }

    /// Sleeps until one of the connections has something to read, the
    /// configuration changes, or `longest` has passed.
    fn wait(&self, longest: Duration) {
        let readable = PollFlags::IN;
        let mut sources = vec![PollFd::new(&self.connection, readable)];
        let steady = [self.news.waited(), self.shell.commands.waited()];
        let steady = steady.into_iter().flatten();
        sources.extend(steady.map(|steady| PollFd::from_borrowed_fd(steady, readable)));
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
        let now = shell.now;
        let heard = self.news.hear(now, |reply| shell.told(reply));
        // The answers to what clicks asked for. A window that had gone by
        // the time it was asked for is nothing to do anything about.
        let answered = shell.commands.hear(now, drop);
        let sent = mem::replace(&mut shell.sent, Ok(()));
        let watcher = self.watcher.as_mut();
        let changed = watcher.map_or(Ok(false), |watcher| watcher.refresh(&mut shell.store));
        // Day turning into night, or night into day, changes only colours,
        // a step at a time: the bars are drawn again, not opened anew.
        shell.stale |= shell.store.at(now);
        heard.and(answered).and(sent).and(changed).map(|changed| {
            if changed {
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
        .and_then(|(wayland, news, commands)| {
            Panel::new(wayland, (news, commands), config, Some(socket))
        });
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
    keyboard: Option<wl_keyboard::WlKeyboard>,
    /// The seat, which a popover takes the click that opens it from.
    seat: Option<wl_seat::WlSeat>,
    screens: Vec<Screen>,
    /// The screen whose bar the pointer is over, and how far along it.
    over: Option<(u32, f64)>,
    /// What hangs from a bar.
    popovers: Popovers,
    /// The month open, and the screen whose clock it hangs from.
    month: Option<(u32, Month)>,
    /// Where on the month the pointer is, while it is on it.
    on_month: Option<(i32, i32)>,
    /// Which tooltip the pointer rests on, and whether it is to be shown.
    resting: Resting<Tipped>,
    /// The tooltip shown.
    tipped: Option<Tipped>,
    /// What the compositor last said of the session.
    state: State,
    /// What the clocks read.
    time: String,
    /// What day it is, which the month opens on.
    today: month::Date,
    /// What it says, in the user's language (ADR 79).
    words: Words,
    store: Store,
    text: Text,
    /// Where what clicks ask for is sent.
    commands: Steady,
    /// Whether everything asked for so far could be sent.
    sent: io::Result<()>,
    /// How long the panel has been running, as of this turn.
    now: Duration,
    /// Whether anything happened that may change what a bar shows.
    stale: bool,
}

/// A tooltip on a bar: of the bar of which screen, by the number the
/// compositor announced it under; what it hangs from there; and what it
/// says, a line at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tipped {
    global: u32,
    under: Area,
    lines: Vec<String>,
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
    sheet: Sheet,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    fraction: wp_fractional_scale_v1::WpFractionalScaleV1,
    /// How wide the compositor made it, in units of layout. Nothing until
    /// it has said.
    width: u32,
    /// How many pixels the screen has to a unit.
    scale: Scale,
    /// What is on it now.
    shown: Option<(Bar, Scale, Theme)>,
    /// The buttons that are on their way from one look to another.
    motion: Motion,
}

/// What it takes to put a bar on a screen.
/// What the bar says of the radios: that the machine is in aeroplane mode,
/// all of them off; or which kinds are off; or nothing while none is.
fn radios(words: &Words, radios: &[omorfos_ipc::Radio]) -> String {
    if !radios.is_empty() && radios.iter().all(|radio| radio.off) {
        return words.say("aeroplane-mode", &[]);
    }
    let off: Vec<String> = radios
        .iter()
        .filter(|radio| radio.off)
        .map(|radio| words.say("radio", &[("kind", Value::Words(&radio.kind))]))
        .collect();
    let named = match off.as_slice() {
        [] => return String::new(),
        [one] => one.clone(),
        [first @ .., last] => {
            let first = first.join(", ");
            let both = [
                ("first", Value::Words(&first)),
                ("last", Value::Words(last)),
            ];
            words.say("and", &both)
        }
    };
    let count = i64::try_from(off.len()).unwrap_or(i64::MAX);
    let given = [
        ("radios", Value::Words(&named)),
        ("count", Value::Number(count)),
    ];
    words.say("radios-off", &given)
}

/// What the bar says while the session is kept awake, as the user asked:
/// until when, if it ends. Nothing while it is not.
fn awake(words: &Words, awake: Option<omorfos_ipc::Awake>) -> String {
    let until = awake.map(|awake| awake.until.and_then(omorfos_shell::clock::at));
    match until {
        None => String::new(),
        Some(None) => words.say("awake", &[]),
        Some(Some(at)) => words.say("awake-until", &[("at", Value::Words(&at))]),
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
    fn new(store: Store, commands: Steady) -> Self {
        Self {
            compositor: None,
            shm: None,
            layers: None,
            viewporter: None,
            scales: None,
            pointer: None,
            keyboard: None,
            seat: None,
            screens: Vec::new(),
            over: None,
            popovers: Popovers::default(),
            month: None,
            on_month: None,
            resting: Resting::new(),
            tipped: None,
            state: State::default(),
            time: String::new(),
            today: month::Date::default(),
            words: Words::of(&PANEL),
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
        // The month hangs from a bar, and goes before it: it opens again
        // as large as the theme now makes it.
        self.month = None;
        self.popovers.close();
        // A tooltip too, and it waits to be rested on anew.
        self.popovers.untip();
        self.tipped = None;
        self.resting = Resting::new();
        let faces = self
            .screens
            .iter_mut()
            .filter_map(|screen| screen.face.take());
        faces.for_each(Face::close);
        self.stale = true;
    }

    /// Lets go of a screen that is no longer there, and of the tooltip,
    /// which may hang from its bar.
    fn forget(&mut self, global: u32) {
        self.popovers.untip();
        self.tipped = None;
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

    /// Does what a click at `serial` on what is under the pointer comes
    /// to: on the month, what it does there; on a bar, what its button
    /// asks of the compositor, or the month opened under the clock, or
    /// closed if it is open.
    fn click(&mut self, serial: u32, handle: &QueueHandle<Self>) {
        // What was clicked shows no tooltip until the pointer leaves it.
        self.stale |= self.resting.clicked();
        let theme = self.store.current().theme.clone();
        if let Some((x, y)) = self.on_month {
            let months = self.month.iter_mut();
            self.stale |= months.fold(false, |turned, (_, month)| {
                turned | month.click(x, y, &theme)
            });
            return;
        }
        let clicked = self.over.and_then(|(global, along)| {
            let screens = self.screens.iter();
            let screens = screens.filter(|screen| screen.global == global);
            let faces = screens.filter_map(|screen| screen.face.as_ref());
            let mut bars = faces.filter_map(|face| face.shown.as_ref());
            let clicked = bars.find_map(|(bar, ..)| bar.click(whole(along)));
            clicked.map(|clicked| (global, clicked))
        });
        match clicked {
            Some((_, Click::Ask(request))) => {
                let sent = self.commands.send(&request, self.now);
                self.sent = mem::replace(&mut self.sent, Ok(())).and(sent);
            }
            Some((global, Click::Month(under))) => {
                self.month_under(global, under, (serial, handle));
            }
            None => {}
        }
    }

    /// Opens the month under the clock at `under` on the bar of `global`,
    /// taking the click at `serial`; or, open, closes it.
    fn month_under(
        &mut self,
        global: u32,
        under: Area,
        (serial, handle): (u32, &QueueHandle<Self>),
    ) {
        self.stale = true;
        if self.month.take().is_some() {
            self.popovers.close();
            return;
        }
        let theme = &self.store.current().theme;
        let month = Month::new(said(&self.words), self.today);
        let screens = self.screens.iter();
        let face = screens
            .filter(|screen| screen.global == global)
            .find_map(|screen| screen.face.as_ref());
        let tools = self.compositor.as_ref().zip(self.viewporter.as_ref());
        let opened =
            face.zip(tools)
                .zip(self.seat.as_ref())
                .is_some_and(|((face, tools), seat)| {
                    let hanging = Hanging {
                        from: &face.layer,
                        under,
                        size: Month::size(theme),
                        click: (seat, serial),
                    };
                    self.popovers.open(tools, hanging, handle)
                });
        self.month = opened.then_some((global, month));
    }

    /// Takes in where the pointer is: on the month, while it is on it,
    /// and on what has a tooltip on a bar.
    fn pointed(&mut self) {
        let theme = &self.store.current().theme;
        let (x, y) = self.on_month.unwrap_or((-1, -1));
        let months = self.month.iter_mut();
        self.stale |= months.fold(false, |changed, (_, month)| {
            changed | month.point(x, y, theme)
        });
        let hovered = self.hovered();
        self.stale |= self.resting.over(hovered, self.now);
    }

    /// The tooltip of what the pointer is over on a bar, if it has one.
    /// None while the month is open.
    fn hovered(&self) -> Option<Tipped> {
        let (global, along) = self.over.filter(|_| self.month.is_none())?;
        let screens = self.screens.iter();
        let screens = screens.filter(|screen| screen.global == global);
        let faces = screens.filter_map(|screen| screen.face.as_ref());
        let mut bars = faces.filter_map(|face| face.shown.as_ref());
        let (under, hint) = bars.find_map(|(bar, ..)| bar.hint(whole(along)))?;
        let lines = vec![says(&self.words, self.today, hint)];
        Some(Tipped {
            global,
            under,
            lines,
        })
    }

    /// Takes a key that was pressed, by Linux's number for it, while the
    /// month has the keyboard: Escape closes it, and the arrows turn it.
    fn press(&mut self, key: u32) {
        self.stale = true;
        if key == ESCAPE {
            self.month = None;
            self.popovers.close();
        }
        let turn = [(LEFT, false), (RIGHT, true)]
            .into_iter()
            .find(|(arrow, _)| *arrow == key);
        let months = self.month.iter_mut();
        months
            .zip(turn)
            .for_each(|((_, month), (_, after))| month.turn(after));
    }

    /// Puts a bar on every screen that has none, and draws those that no
    /// longer show what they should.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        let popover = self.popovers.changed();
        if !mem::take(&mut self.stale) && !popover {
            return Ok(());
        }
        // Taken away by the compositor, at a click elsewhere.
        if !self.popovers.is_open() {
            self.month = None;
        }
        let globals = self.compositor.as_ref().zip(self.shm.as_ref());
        let globals = globals.zip(self.layers.as_ref());
        let globals = globals.zip(self.viewporter.as_ref().zip(self.scales.as_ref()));
        // Until the compositor has offered everything, there is nothing to
        // draw with. Each offer makes this worth trying again.
        let Some((((compositor, shm), layers), (viewporter, scales))) = globals else {
            return Ok(());
        };
        let awake = awake(&self.words, self.state.awake);
        let radios = radios(&self.words, &self.state.radios);
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
        let shm = shm.clone();
        let mut screens = self.screens.iter_mut();
        let drawn = screens.try_for_each(|screen| {
            let face = screen.face.take();
            let face = face.unwrap_or_else(|| Face::open(&tools, screen));
            let face = screen.face.insert(face);
            face.draw(&mut tools, &screen.name)
        });
        drawn
            .and_then(|()| self.show_month(&shm, handle))
            .and_then(|()| self.show_tip(&shm, handle))
    }

    /// Draws the month, if it is open, with as many pixels to a unit as its
    /// screen has, under where its clock now is.
    fn show_month(&mut self, shm: &wl_shm::WlShm, handle: &QueueHandle<Self>) -> io::Result<()> {
        let theme = &self.store.current().theme;
        let Some((global, month)) = &self.month else {
            return Ok(());
        };
        let screens = self.screens.iter();
        let face = screens
            .filter(|screen| screen.global == *global)
            .find_map(|screen| screen.face.as_ref());
        let shown = face.and_then(|face| face.shown.as_ref());
        let clock = shown.and_then(|(bar, ..)| bar.clock());
        self.popovers.hang(clock, handle);
        let scale = face.map_or(Scale::ONE, |face| face.scale);
        let canvas = month.paint(&mut self.text, theme, scale);
        self.popovers.draw(shm, &canvas, handle)
    }

    /// Shows the tooltip the pointer has rested on long enough, if any,
    /// with as many pixels to a unit as its screen has, and takes away one
    /// no longer wanted.
    fn show_tip(&mut self, shm: &wl_shm::WlShm, handle: &QueueHandle<Self>) -> io::Result<()> {
        // What is under the pointer may have changed under it, as a bar
        // drawn anew does.
        let hovered = self.hovered();
        self.resting.over(hovered, self.now);
        let wanted = self.resting.shown().cloned();
        let anew = wanted != self.tipped;
        if anew {
            self.popovers.untip();
        }
        self.tipped = wanted;
        let Some(tipped) = &self.tipped else {
            return Ok(());
        };
        let theme = &self.store.current().theme;
        let screens = self.screens.iter();
        let face = screens
            .filter(|screen| screen.global == tipped.global)
            .find_map(|screen| screen.face.as_ref());
        let scale = face.map_or(Scale::ONE, |face| face.scale);
        let canvas = omorfos_ui::tooltip(&mut self.text, theme, &tipped.lines, scale);
        let tools = self.compositor.as_ref().zip(self.viewporter.as_ref());
        if let Some((face, tools)) = face.zip(tools).filter(|_| anew) {
            let tip = Tip {
                from: &face.layer,
                under: tipped.under,
                size: canvas.units(),
            };
            self.popovers.tip(tools, tip, handle);
        }
        self.popovers.draw_tip(shm, &canvas, handle)
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
            sheet: Sheet::new(surface, viewport),
            layer,
            fraction,
            width: 0,
            scale: Scale::ONE,
            shown: None,
            motion: Motion::new(),
        }
    }

    /// Gives the surface back.
    fn close(self) {
        self.fraction.destroy();
        self.layer.destroy();
        self.sheet.destroy();
    }

    /// Draws the bar of the screen called `name`, unless it already shows
    /// just that.
    fn draw(&mut self, tools: &mut Tools<'_>, name: &str) -> io::Result<()> {
        if self.width == 0 {
            return Ok(());
        }
        let (state, times) = (tools.state, (tools.time, tools.awake, tools.radios));
        let bar = Bar::new(state, name, self.width, times, tools.theme, tools.text);
        // In the colours it is drawn in, which may change alone.
        let wanted = (bar, self.scale, tools.theme.clone());
        // The same bar is drawn again while a button on it changes colour.
        let again = self.motion.due(tools.now);
        if self.shown.as_ref() == Some(&wanted) && !again {
            return Ok(());
        }
        let (bar, scale, _) = &wanted;
        self.motion.begin(tools.now);
        let canvas = bar.paint(tools.text, tools.theme, *scale, &mut self.motion);
        self.motion.end();
        let size = (self.width, bar.height());
        let put = self.sheet.put(tools.shm, &canvas, size, tools.handle);
        put.map(|()| self.shown = Some(wanted))
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
                "wl_seat" => shell.seat = Some(registry.bind(name, version.min(5), handle, ())),
                // The third version can place a popover anew.
                "xdg_wm_base" => {
                    shell
                        .popovers
                        .offered(registry.bind(name, version.min(3), handle, ()));
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
            && shell.scaled(*global, Scale::from_numerator(scale))
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
            // The keyboard is the month's while it is open, and nothing's
            // else of the panel's.
            let typing = capabilities.contains(wl_seat::Capability::Keyboard);
            let keyboard = shell.keyboard.take();
            shell.keyboard = keyboard.or_else(|| typing.then(|| seat.get_keyboard(handle, ())));
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
        handle: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let mut screens = shell.screens.iter();
                let entered = screens.find(|screen| {
                    let face = screen.face.as_ref();
                    face.is_some_and(|face| *face.sheet.surface() == surface)
                });
                shell.over = entered.map(|screen| (screen.global, surface_x));
                let on = shell.popovers.is(&surface);
                shell.on_month = on.then(|| (whole(surface_x), whole(surface_y)));
                shell.pointed();
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                shell.over = shell.over.map(|(global, _)| (global, surface_x));
                let on = shell.on_month.map(|_| (whole(surface_x), whole(surface_y)));
                shell.on_month = on;
                shell.pointed();
            }
            wl_pointer::Event::Leave { .. } => {
                shell.over = None;
                shell.on_month = None;
                shell.pointed();
            }
            wl_pointer::Event::Button {
                serial,
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => shell.click(serial, handle),
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Shell {
    fn event(
        shell: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Only what a key is matters, by its number: no layout is needed
        // for Escape and the arrows, and the layout the compositor sends
        // is let go of, closing it, with the event.
        if let wl_keyboard::Event::Key {
            key,
            state: WEnum::Value(wl_keyboard::KeyState::Pressed),
            ..
        } = event
        {
            shell.press(key);
        }
    }
}

impl AsMut<Popovers> for Shell {
    fn as_mut(&mut self) -> &mut Popovers {
        &mut self.popovers
    }
}

delegate_dispatch!(Shell: [xdg_surface::XdgSurface: ()] => Popovers);
delegate_dispatch!(Shell: [xdg_popup::XdgPopup: ()] => Popovers);
// What makes popovers says nothing but to be pinged, which omorfos does
// not do to its components; a positioner says nothing at all.
delegate_noop!(Shell: ignore xdg_wm_base::XdgWmBase);
delegate_noop!(Shell: xdg_positioner::XdgPositioner);

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
delegate_noop!(Shell: wl_region::WlRegion);
delegate_noop!(Shell: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Shell: wp_viewporter::WpViewporter);
delegate_noop!(Shell: wp_viewport::WpViewport);
delegate_noop!(Shell: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);

/// What the month says, in the panel's words.
fn said(words: &Words) -> month::Said {
    let say = |name: &str| words.say(name, &[]);
    month::Said {
        months: [
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
        ]
        .map(say),
        weekdays: [
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
        ]
        .map(|day| say(&format!("{day}-short"))),
    }
}

/// What a tooltip on the bar says, in the panel's words, `today` being
/// the date.
fn says(words: &Words, today: month::Date, hint: Hint) -> String {
    match hint {
        Hint::Date => date(words, today),
        Hint::Awake => words.say("awake-tip", &[]),
        Hint::Whole(label) => label,
    }
}

/// The days of the week from Monday, and the months, as the panel's
/// messages choose by them.
const WEEKDAYS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];
const MONTHS: [&str; 12] = [
    "january",
    "february",
    "march",
    "april",
    "may",
    "june",
    "july",
    "august",
    "september",
    "october",
    "november",
    "december",
];

/// The whole of `today`, as "Saturday 14 November 2026".
fn date(words: &Words, today: month::Date) -> String {
    let weekday = usize::from(today.weekday().to_monday_zero_offset().unsigned_abs());
    let month = usize::from(today.month().unsigned_abs()).saturating_sub(1);
    let weekday = WEEKDAYS.get(weekday).copied().unwrap_or_default();
    let month = MONTHS.get(month).copied().unwrap_or_default();
    let (day, year) = (today.day().to_string(), today.year().to_string());
    let given = [
        ("weekday", Value::Words(weekday)),
        ("day", Value::Words(&day)),
        ("month", Value::Words(month)),
        ("year", Value::Words(&year)),
    ];
    words.say("date", &given)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tooltip_on_the_bar_says_the_date_what_a_click_does_or_a_whole_label() {
        let english = Words::in_languages(&PANEL, &[]);
        let saturday = month::Date::new(2026, 11, 14).unwrap();
        assert_eq!(
            says(&english, saturday, Hint::Date),
            "Saturday 14 November 2026"
        );
        let sunday = month::Date::new(2027, 2, 28).unwrap();
        assert_eq!(
            says(&english, sunday, Hint::Date),
            "Sunday 28 February 2027"
        );
        assert_eq!(
            says(&english, saturday, Hint::Awake),
            "Click to let the screen sleep again"
        );
        let whole = Hint::Whole("All of a title".to_owned());
        assert_eq!(says(&english, saturday, whole), "All of a title");
        // In Polish the month is said as a date has it, not as a heading.
        let polish = Words::in_languages(&PANEL, &["pl".to_owned()]);
        assert_eq!(
            says(&polish, saturday, Hint::Date),
            "sobota, 14 listopada 2026"
        );
    }

    #[test]
    fn the_bar_says_which_radios_are_off_and_aeroplane_mode_when_all_are() {
        let radio = |kind: &str, off| omorfos_ipc::Radio {
            kind: kind.to_owned(),
            off,
            switched: false,
        };
        let english = Words::in_languages(&PANEL, &[]);
        let radios = |given: &[omorfos_ipc::Radio]| radios(&english, given);
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
        assert_eq!(
            radios(&[radio("laser", true), radio("wlan", false)]),
            "A radio off"
        );
        // In the user's language, where it has the words.
        let polish = Words::in_languages(&PANEL, &["pl".to_owned()]);
        assert_eq!(
            super::radios(&polish, &three),
            "Wyłączone: Wi-Fi, Bluetooth i NFC"
        );
        let all = [radio("wlan", true)];
        assert_eq!(super::radios(&polish, &all), "Tryb samolotowy");
    }

    #[test]
    fn the_bar_says_the_session_is_kept_awake_and_until_when() {
        use omorfos_ipc::Awake;

        let english = Words::in_languages(&PANEL, &[]);
        let awake = |given| awake(&english, given);
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
        let polish = Words::in_languages(&PANEL, &["pl".to_owned()]);
        let until = super::awake(
            &polish,
            Some(Awake {
                until: Some(1_761_400_800),
            }),
        );
        assert!(until.starts_with("Nie zasypia do "), "{until}");
    }
}
