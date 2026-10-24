//! The lock screen as a Wayland client: the session locked, a surface on
//! every screen with the time, and once a key or a button is pressed who
//! is to unlock it, a field for the password and what each way of
//! unlocking says; and the session unlocked once one says yes.

use std::error::Error;
use std::ffi::OsString;
use std::io;
use std::mem;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use omorfos_config::{Color, Fit, Part, States, Store, Style, Theme, Transition};
use omorfos_shell::picture;
use omorfos_shell::said::Said;
use omorfos_shell::shake::{FRAME, SHAKING, shaken};
use omorfos_shell::typing::{Key, Repeat, Typing};
use omorfos_ui::{Canvas, Image, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::pipe::{PipeFlags, pipe_with};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry,
    wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

use crate::User;
use crate::power::{Act, Button, Power, Told};
use crate::ways::{Came, Way, Ways};

/// Why the lock screen could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The longest secret taken in, in characters.
const LONGEST: usize = 1024;

/// How large the time is drawn, in points.
const CLOCK: u32 = 64;

/// How wide the field is, and how tall each line of the lock screen is, in
/// units of layout.
const FIELD: (u32, u32) = (320, 36);
const LINE: u32 = 28;

/// How large the face is, in units of layout, and in pixels as it is read:
/// enough for a screen of three pixels to a unit.
const FACE: u32 = 96;
const FACE_READ: u32 = FACE * 3;

/// How large the name is drawn, in points, and the initials in a face of
/// none's own.
const NAME: u32 = 20;
const INITIALS: u32 = 32;

/// The room kept clear at each side of the screen, in units of layout:
/// "far", as the design has it.
const MARGIN: u32 = 24;

/// How tall a power button is, the room inside it beside its label, the
/// room between two, and below them, in units of layout.
const BUTTON: u32 = 32;
const PADDING: u32 = 16;
const GAP: u32 = 12;
const BOTTOM: u32 = 24;

/// How far below its place what comes into view starts, in units of
/// layout.
const RISE: f32 = 16.0;

/// How long the lock screen waits for a key before it shows only the time
/// again, emptying the field.
const REST: Duration = Duration::from_secs(30);

/// The lock screen: a connection to the compositor as a Wayland client,
/// and the helpers asking PAM.
pub struct Lock {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
    /// What the helpers and `omorfos-session` said, and what wakes the
    /// lock screen when they do.
    heard: Receiver<(Way, Said)>,
    told: Receiver<Told>,
    senders: Senders,
    woken: OwnedFd,
}

/// What the helpers and `omorfos-session` say through, and wake the lock
/// screen by.
struct Senders {
    heard: Sender<(Way, Said)>,
    told: Sender<Told>,
    waker: Waker,
}

/// What wakes the lock screen from another thread.
#[derive(Clone)]
struct Waker(Arc<OwnedFd>);

impl Waker {
    fn wake(&self) {
        // A wake-up already waiting is as good.
        let _ = rustix::io::write(&*self.0, &[1]);
    }
}

impl Lock {
    /// A lock screen on the Wayland connection `wayland`, looking as the
    /// configuration in `config` has it, for the user called `user`, asking
    /// each of `ways` by a helper that is `helper`. Their face is the
    /// first of theirs that reads as a picture; with none, there is none.
    /// With `power`, `omorfos-session` and the words it is always asked
    /// with first, it offers to sleep, restart, shut down and log out.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        config: &Path,
        (helper, user): (PathBuf, User),
        ways: &[Way],
        power: Option<(PathBuf, Vec<OsString>)>,
    ) -> Result<Self, Failure> {
        let room = (FACE_READ, FACE_READ);
        let face = user.faces.iter().find_map(|path| {
            let read = omorfos_wallpaper::picture::read(path, Fit::Fill, room);
            read.ok().map(|face| face.circled())
        });
        let connection = Connection::from_socket(wayland).map_err(Failure::from);
        let piped = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(Failure::from);
        connection.and_then(|connection| {
            piped.map(|(woken, waking)| {
                let queue = connection.new_event_queue();
                connection.display().get_registry(&queue.handle(), ());
                let (sender, heard) = mpsc::channel();
                let (teller, told) = mpsc::channel();
                let mut shell = Shell::new(
                    Store::open(config),
                    (user.called, face),
                    Ways::new(ways, helper, user.name),
                );
                shell.power = power.map(Power::new);
                Self {
                    connection,
                    queue,
                    shell,
                    heard,
                    told,
                    senders: Senders {
                        heard: sender,
                        told: teller,
                        waker: Waker(Arc::new(waking)),
                    },
                    woken,
                }
            })
        })
    }

    /// Does whatever there is to do, without waiting: hears the compositor
    /// and the helpers, locks the session once it can, puts a surface on
    /// every screen, starts the helpers that are due, and draws. Returns
    /// whether the session has been unlocked, which is when the lock screen
    /// is done.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone, or would not have the
    /// session locked by this program.
    pub fn turn(&mut self, now: Instant) -> Result<bool, Failure> {
        // Whether reading with nothing to read counts as a failure depends
        // on how the library was built; a real failure comes back from the
        // flush that ends the turn.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        self.shell.now = now;
        let dispatched = self.queue.dispatch_pending(&mut self.shell);
        dispatched
            .map_err(Failure::from)
            .and_then(|_| self.after(now))
    }

    /// The rest of a turn, once the compositor has been heard.
    fn after(&mut self, now: Instant) -> Result<bool, Failure> {
        let mut drained = [0; 64];
        while rustix::io::read(&self.woken, &mut drained).is_ok_and(|read| read > 0) {}
        let heard: Vec<(Way, Said)> = self.heard.try_iter().collect();
        for said in heard {
            // A finger that did not match is shown, if only the time was.
            if matches!(said.1, Said::Error(_)) {
                self.shell.wake();
            }
            match self.shell.ways.heard(said, now) {
                Came::Right => self.shell.held = Held::Unlocking,
                Came::Wrong => self.shell.shake(),
                Came::Going => {}
            }
            self.shell.stale = true;
        }
        if let Some(power) = self.shell.power.as_mut() {
            for told in self.told.try_iter() {
                power.told(told);
                self.shell.stale = true;
            }
        }
        let refused = (self.shell.held == Held::Refused).then_some(());
        let lockable = refused.map_or(Ok(()), |()| {
            Err(Failure::from(
                "the session is being locked by another program",
            ))
        });
        let handle = self.queue.handle();
        let gone_on = lockable.and_then(|()| {
            let gone_on = self.shell.go_on(now, &handle, &self.senders);
            gone_on.map_err(Failure::from)
        });
        gone_on.and_then(|done| {
            self.connection
                .flush()
                .map(|()| done)
                .map_err(Failure::from)
        })
    }

    /// When there will next be something to do without anything having
    /// happened: the time changing, a helper due to be asked again, the
    /// lock screen due to show only the time again, or a key held due to be
    /// done again.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        let clock = self.shell.clock.as_ref().map(|(_, changes)| *changes);
        let rest = match self.shell.view {
            View::Awake { until } => until,
            View::Resting => None,
        };
        let repeat = self.shell.repeat.deadline();
        clock
            .into_iter()
            .chain(self.shell.again)
            .chain(rest)
            .chain(repeat)
            .min()
    }

    /// Runs the lock screen, asleep until there is something to do, until
    /// the session is unlocked, or it cannot go on, which it says why.
    ///
    /// # Errors
    ///
    /// Returns why it could not go on.
    pub fn serve(&mut self) -> Result<(), Failure> {
        loop {
            match self.turn(Instant::now()) {
                Ok(false) => {}
                ended => return ended.map(drop),
            }
            let longest = self
                .deadline()
                .map(|due| due.saturating_duration_since(Instant::now()));
            let longest = longest.and_then(|longest| Timespec::try_from(longest).ok());
            let readable = PollFlags::IN;
            let mut sources = [
                PollFd::new(&self.connection, readable),
                PollFd::new(&self.woken, readable),
            ];
            // A wait that is interrupted is a wake-up like any other.
            let _ = poll(&mut sources, longest.as_ref());
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // No helper outlives the lock screen.
        self.shell.ways.stop();
    }
}

/// What the Wayland connection's events act on.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    manager: Option<ext_session_lock_manager_v1::ExtSessionLockManagerV1>,
    /// The session lock, once asked for, and how far it has got.
    lock: Option<ext_session_lock_v1::ExtSessionLockV1>,
    held: Held,
    screens: Vec<Screen>,
    store: Store,
    text: Text,
    typing: Typing,
    typed: String,
    /// Whether all that is typed is chosen, as Ctrl and A choose it, to be
    /// typed over or taken out at once.
    marked: bool,
    /// The key held down, done again as keys repeat.
    repeat: Repeat,
    /// The time of the turn being taken, which keys going down are timed
    /// by.
    now: Instant,
    ways: Ways,
    /// The power buttons, if they are offered.
    power: Option<Power>,
    /// Where the pointer is: on which screen, and where on it, in units of
    /// layout; the button it is over; and one pressed, to be done.
    pointer: Option<(u32, (f64, f64))>,
    hovered: Option<Button>,
    chosen: Option<Button>,
    /// The button the keyboard is on, reached with Tab; on none, it is on
    /// the field.
    focused: Option<Button>,
    /// What the user is called, and their face, cut to a circle.
    called: String,
    face: Option<Image>,
    /// Whether only the time is shown.
    view: View,
    /// While who is to unlock comes into view, or goes out of it: when the
    /// first frame of that was shown, once one has been, and how long it
    /// has moved for since, in milliseconds, as the compositor tells the
    /// time of each.
    moving: Option<(Option<u32>, u32)>,
    /// How much of it was in view when it last began to come or go, so that
    /// turning back halfway does not jump.
    from: f32,
    /// What the clock reads, and when it reads differently.
    clock: Option<(String, Instant)>,
    /// When a helper is next due to be asked again.
    again: Option<Instant>,
    /// While the field shakes, after a wrong password: when the first frame
    /// of it was shown, once one has been, and how long it has shaken for
    /// since, in milliseconds, as the compositor tells the time of each.
    shaking: Option<(Option<u32>, u32)>,
    /// Whether what is on the screens may not be what should be.
    stale: bool,
}

/// What the lock screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    /// Only the time, until a key or a button is pressed.
    Resting,
    /// Who is to unlock, the field and what each way says as well, until
    /// it has been left alone for a while: when that is, once a turn has
    /// said, after the last key.
    Awake { until: Option<Instant> },
}

/// How far the session lock has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    /// Asked for, or to be, and not yet said to be locked.
    Asking,
    /// Locked, as the compositor says.
    Locked,
    /// Not to be had: the compositor would not have the session locked by
    /// this program, as when another holds it.
    Refused,
    /// A way has said the user is who they say, and it is to be unlocked.
    Unlocking,
}

/// A screen, and the lock surface on it.
struct Screen {
    global: u32,
    output: wl_output::WlOutput,
    face: Option<Face>,
}

/// A lock surface.
struct Face {
    surface: wl_surface::WlSurface,
    viewport: wp_viewport::WpViewport,
    /// How large the compositor made it, in units of layout, once it has
    /// said; and how many pixels the screen has to a unit.
    size: (u32, u32),
    scale: Scale,
    /// The picture the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
    /// The buttons drawn on it, and where.
    buttons: Vec<(Button, Area)>,
}

/// An area in units of layout: from the left, from the top, across and
/// down.
type Area = (i32, i32, u32, u32);

impl Shell {
    fn new(store: Store, (called, face): (String, Option<Image>), ways: Ways) -> Self {
        Self {
            compositor: None,
            shm: None,
            viewporter: None,
            scales: None,
            manager: None,
            lock: None,
            held: Held::Asking,
            screens: Vec::new(),
            store,
            text: Text::new(),
            typing: Typing::default(),
            typed: String::new(),
            marked: false,
            repeat: Repeat::default(),
            now: Instant::now(),
            ways,
            power: None,
            pointer: None,
            hovered: None,
            chosen: None,
            focused: None,
            called,
            face,
            view: View::Resting,
            moving: None,
            from: 0.0,
            clock: None,
            again: None,
            shaking: None,
            stale: true,
        }
    }

    /// Locks the session once the compositor offers to, puts a surface on
    /// each screen, asks the ways once it is locked, and draws; or unlocks
    /// it. Returns whether it has been unlocked.
    fn go_on(
        &mut self,
        now: Instant,
        handle: &QueueHandle<Self>,
        senders: &Senders,
    ) -> io::Result<bool> {
        if self.held == Held::Unlocking {
            let lock = self.lock.take();
            lock.iter()
                .for_each(ext_session_lock_v1::ExtSessionLockV1::unlock_and_destroy);
            wipe(&mut self.typed);
            self.ways.stop();
            return Ok(true);
        }
        if self.lock.is_none() {
            self.lock = self
                .manager
                .as_ref()
                .map(|manager| manager.lock(handle, ()));
        }
        // A key held is done again when it is due, as it would be pressed
        // again, and puts off showing only the time as a key does.
        if let Some(code) = self.repeat.due(now) {
            let key = self.typing.pressed(code);
            self.key(key);
            self.view = View::Awake { until: None };
        }
        match self.view {
            View::Awake { until: None } => {
                self.view = View::Awake {
                    until: Some(now + REST),
                }
            }
            View::Awake { until: Some(until) } if until <= now => self.rest(),
            View::Awake { .. } | View::Resting => {}
        }
        if self.held == Held::Locked {
            let wake = senders.waker.clone();
            let heard_by = (senders.heard.clone(), move || wake.wake());
            self.again = self.ways.start(now, &heard_by);
            let wake = senders.waker.clone();
            let told_by = (senders.told.clone(), move || wake.wake());
            let chosen = self.chosen.take();
            if let Some(power) = self.power.as_mut() {
                if let Some(button) = chosen {
                    let asked = power.confirming;
                    power.pressed(button, &told_by);
                    self.focused = match (asked, power.confirming) {
                        // Asked about, the keyboard is on Cancel, so that
                        // Enter pressed again loses nothing.
                        (_, Some(_)) => Some(Button::No),
                        // Said no to from the keyboard, it is back on what
                        // was asked about.
                        (Some(act), None) if button == Button::No && self.focused.is_some() => {
                            Some(Button::Act(act))
                        }
                        // Said yes to, it is back on the field; done at
                        // once, it stays.
                        _ => self.focused.filter(|_| asked.is_none()),
                    };
                }
                // What is offered is asked once the buttons are to be seen.
                if self.view != View::Resting {
                    power.ask(&told_by);
                }
            }
        }
        if self
            .clock
            .as_ref()
            .is_none_or(|(_, changes)| *changes <= now)
        {
            let (time, changes) = omorfos_shell::clock::now();
            self.clock = Some((time, now + changes));
            self.stale = true;
        }
        self.show(handle).map(|()| false)
    }

    /// Puts a lock surface on every screen that has none, and draws those
    /// that are to be drawn again.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        let globals = self.compositor.as_ref().zip(self.shm.as_ref());
        let globals = globals.zip(self.viewporter.as_ref().zip(self.scales.as_ref()));
        let globals = globals.zip(self.lock.as_ref());
        let Some((((compositor, shm), (viewporter, scales)), lock)) = globals else {
            return Ok(());
        };
        for screen in &mut self.screens {
            if screen.face.is_none() {
                let surface = compositor.create_surface(handle, ());
                // What the compositor configures and tells the scale of: the
                // surface's role and its scale, which nothing else is asked.
                lock.get_lock_surface(&surface, &screen.output, handle, screen.global);
                scales.get_fractional_scale(&surface, handle, screen.global);
                let face = Face {
                    viewport: viewporter.get_viewport(&surface, handle, ()),
                    surface,
                    size: (0, 0),
                    scale: Scale::ONE,
                    buffer: None,
                    buttons: Vec::new(),
                };
                screen.face = Some(face);
            }
        }
        if !mem::take(&mut self.stale) {
            return Ok(());
        }
        let theme = self.store.current().theme.clone();
        let time = self.clock.as_ref().map_or("", |(time, _)| time.as_str());
        let ways = &self.ways;
        // Shaking, the field is drawn again as each frame is shown, until it
        // has shaken for as long as it does.
        let elapsed = self.shaking.map(|(_, elapsed)| elapsed);
        self.shaking = self.shaking.filter(|(_, elapsed)| *elapsed < SHAKING);
        let shift = elapsed.map_or(0, shaken);
        // Coming into view or going, likewise, for as long as the theme
        // says.
        let moves = theme.style(Part::Question, States::NONE).transition;
        let share = self.share(moves);
        let lasts = moves.lasts();
        let done = |elapsed: &u32| Duration::from_millis(u64::from(*elapsed)) >= lasts;
        self.moving = self.moving.filter(|(_, elapsed)| !done(elapsed));
        let who = (self.called.as_str(), self.face.as_ref());
        let power = (self.power.as_ref(), self.hovered, self.focus());
        let faces = self
            .screens
            .iter_mut()
            .filter_map(|screen| screen.face.as_mut());
        let mut sized = faces.filter(|face| face.size.0 > 0 && face.size.1 > 0);
        sized.try_for_each(|face| {
            let shown = Shown {
                time,
                who,
                typed: (self.typed.chars().count(), self.marked),
                ways,
                shift,
                share,
                power,
            };
            let (canvas, buttons) = paint(&theme, &mut self.text, &shown, face.size, face.scale);
            face.buttons = buttons;
            picture(shm, &canvas, handle).map(|buffer| {
                face.surface.attach(Some(&buffer), 0, 0);
                // Told when each drawing is shown: shaking, that is when the
                // next is due; otherwise it says nothing to do.
                face.surface.frame(handle, ());
                let (width, height) = (face.size.0.cast_signed(), face.size.1.cast_signed());
                face.viewport.set_destination(width, height);
                face.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
                face.surface.commit();
                face.buffer = Some(buffer);
            })
        })
    }

    /// Acknowledges the size the lock surface on the screen numbered
    /// `global` is given, and notes it. Returns whether that is news.
    fn sized(
        &mut self,
        lock: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        serial: u32,
        global: u32,
        size: (u32, u32),
    ) -> bool {
        lock.ack_configure(serial);
        let screens = self
            .screens
            .iter_mut()
            .filter(|screen| screen.global == global);
        let mut news = false;
        for face in screens.filter_map(|screen| screen.face.as_mut()) {
            news |= mem::replace(&mut face.size, size) != size;
        }
        news
    }

    /// Notes how many pixels the screen numbered `global` has to a unit.
    /// Returns whether that is news.
    fn scaled(&mut self, global: u32, scale: Scale) -> bool {
        let screens = self
            .screens
            .iter_mut()
            .filter(|screen| screen.global == global);
        let mut news = false;
        for face in screens.filter_map(|screen| screen.face.as_mut()) {
            news |= mem::replace(&mut face.scale, scale) != scale;
        }
        news
    }

    /// Shows who is to unlock, if only the time was shown, coming into view
    /// as the theme has things move; and puts off showing only the time
    /// again. Returns whether only the time was shown.
    fn wake(&mut self) -> bool {
        let resting = self.view == View::Resting;
        if resting {
            self.move_from_here();
        }
        // What may be done may have changed while only the time showed.
        if resting {
            self.power.iter_mut().for_each(Power::refresh);
        }
        self.view = View::Awake { until: None };
        self.stale = true;
        resting
    }

    /// Shows only the time again, the rest going out of view as the theme
    /// has things move, and forgets what was typed, what was being asked
    /// about and the button the keyboard was on.
    fn rest(&mut self) {
        self.move_from_here();
        self.view = View::Resting;
        self.repeat.stop();
        self.empty();
        self.power.iter_mut().for_each(Power::forget);
        self.focused = None;
        self.stale = true;
    }

    /// How much of who is to unlock, the field and the rest is in view, as
    /// it moves as `transition` says: from 0, only the time, to 1, all of it.
    fn share(&self, transition: Transition) -> f32 {
        let along = self
            .moving
            .map(|(_, elapsed)| transition.progress(Duration::from_millis(u64::from(elapsed))));
        match self.view {
            View::Resting => along.map_or(0.0, |along| self.from * (1.0 - along)),
            View::Awake { .. } => {
                along.map_or(1.0, |along| (1.0 - self.from).mul_add(along, self.from))
            }
        }
    }

    /// Has the view begin to move the other way from where it is, unless the
    /// theme has nothing move.
    fn move_from_here(&mut self) {
        let theme = &self.store.current().theme;
        let moves = theme.style(Part::Question, States::NONE).transition;
        if moves.duration > 0 {
            self.from = self.share(moves);
            self.moving = Some((None, 0));
        }
    }

    /// The button the pointer is over, if any.
    fn over(&self) -> Option<Button> {
        self.pointer.and_then(|(on, (x, y))| {
            let screens = self.screens.iter().filter(|screen| screen.global == on);
            let mut buttons = screens
                .filter_map(|screen| screen.face.as_ref())
                .flat_map(|face| &face.buttons);
            let inside = |(left, top, across, down): &Area| {
                let (left, top) = (f64::from(*left), f64::from(*top));
                (left..left + f64::from(*across)).contains(&x)
                    && (top..top + f64::from(*down)).contains(&y)
            };
            buttons
                .find(|(_, area)| inside(area))
                .map(|(button, _)| *button)
        })
    }

    /// The button the keyboard is on, if it is one of those shown; else it
    /// is on the field.
    fn focus(&self) -> Option<Button> {
        let buttons = self.power.as_ref().map(Power::buttons);
        let shown = |button: &Button| buttons.as_ref().is_some_and(|all| all.contains(button));
        self.focused.filter(shown)
    }

    /// Moves the keyboard on from the field to each button in turn, and
    /// round to the field again; or `back`.
    fn tab(&mut self, back: bool) {
        let buttons = self.power.as_ref().map(Power::buttons).unwrap_or_default();
        let places = buttons.len() + 1;
        let focus = self.focus();
        let on = buttons.iter().position(|button| Some(*button) == focus);
        // The field is the place after the last button.
        let on = on.unwrap_or(buttons.len());
        let to = (if back { on + places - 1 } else { on + 1 }) % places;
        self.focused = buttons.get(to).copied();
    }

    /// Notes the button the pointer is over, drawn as hovered.
    fn hover(&mut self) {
        let over = self.over();
        if over != self.hovered {
            self.hovered = over;
            self.stale = true;
        }
    }

    /// Shakes the field, as a wrong password does, unless the theme has
    /// nothing move.
    fn shake(&mut self) {
        let theme = &self.store.current().theme;
        let moves = theme.style(Part::Question, States::NONE).transition;
        if moves.duration > 0 {
            self.shaking = Some((None, 0));
        }
    }

    /// Takes in a key going down: what it types goes in the field, `⌫`
    /// takes the last character out, Escape shows only the time again, and
    /// empties it, and Enter hands it
    /// to the password's helper if it is asking. Tab and Shift with it move
    /// the keyboard to each button and back to the field, and Enter on a
    /// button presses it. While only the time is shown, a key shows the
    /// rest and does nothing else, so that a key pressed to wake the screen
    /// is not taken for part of the password.
    fn pressed(&mut self, code: u32) {
        if self.wake() {
            return;
        }
        let key = self.typing.pressed(code);
        // Only what types, or takes out, is done again while it is held:
        // not Enter, Escape or Tab, which do something once.
        let again = matches!(key, Key::Typed(_) | Key::Back) && self.typing.repeats(code);
        self.repeat.pressed(code, again, self.now);
        self.key(key);
    }

    /// Does what `key` does, going down or held.
    fn key(&mut self, key: Key) {
        // What types, takes out or chooses is for the field.
        if matches!(key, Key::Typed(_) | Key::Back | Key::All) {
            self.focused = None;
        }
        match key {
            Key::Enter => match self.focus() {
                Some(button) => self.chosen = Some(button),
                None => {
                    if self.ways.answer(&self.typed) {
                        self.empty();
                    }
                }
            },
            // Escape says no to what is being asked about, if anything is,
            // and else shows only the time, as being left alone does.
            Key::Escape => {
                let power = self.power.as_ref();
                if power.is_some_and(|power| power.confirming.is_some()) {
                    self.chosen = Some(Button::No);
                } else {
                    self.rest();
                }
            }
            Key::Next => self.tab(false),
            Key::Previous => self.tab(true),
            Key::All => self.marked = !self.typed.is_empty(),
            // All of it, if all is chosen.
            Key::Back if self.marked => self.empty(),
            Key::Back => {
                self.typed.pop();
            }
            Key::Typed(typed) => {
                // In place of all of it, if all is chosen.
                if self.marked {
                    self.empty();
                }
                // No more than a secret may be is kept.
                let room = LONGEST.saturating_sub(self.typed.chars().count());
                self.typed.extend(typed.chars().take(room));
            }
            Key::Nothing => {}
        }
        self.stale = true;
    }

    /// Empties the field, wiping what was typed.
    fn empty(&mut self) {
        wipe(&mut self.typed);
        self.marked = false;
    }
}

/// Wipes what was typed, so that it is not left in memory.
fn wipe(typed: &mut String) {
    let mut bytes = mem::take(typed).into_bytes();
    bytes.fill(0);
}

/// What a screen shows.
struct Shown<'a> {
    /// The time.
    time: &'a str,
    /// What the user is called, and their face.
    who: (&'a str, Option<&'a Image>),
    /// How many characters are typed in the field, and whether all of them
    /// are chosen.
    typed: (usize, bool),
    ways: &'a Ways,
    /// How far the field is moved across as it shakes, in units of layout.
    shift: i32,
    /// How much of who is to unlock, the field and what the ways say is in
    /// view: from 0, only the time, to 1, all of it.
    share: f32,
    /// The power buttons, if offered, the one the pointer is over and the
    /// one the keyboard is on.
    power: (Option<&'a Power>, Option<Button>, Option<Button>),
}

/// Paints a screen `size` units large at `scale`: the time, in the middle
/// while it is all there is; and as the rest comes into view, rising into
/// place and out of nothing, who is to unlock, the field with a dot for
/// each character typed, what the password's helper said under it, and
/// what each other way says beneath; and at the bottom, the power buttons,
/// which it returns with where each is.
fn paint(
    theme: &Theme,
    text: &mut Text,
    shown: &Shown,
    size: (u32, u32),
    scale: Scale,
) -> (Canvas, Vec<(Button, Area)>) {
    let (width, height) = size;
    let share = shown.share;
    let mut canvas = Canvas::scaled(width, height, scale);
    let behind = theme.style(Part::Desktop, States::NONE).background;
    canvas.fill((0, 0, width, height), behind);
    let middle = |length: u32| (width.saturating_sub(length) / 2).cast_signed();
    // Who is to unlock is under the time, and the field under them: all of
    // it in the middle, and the time alone in the middle before it.
    // With the power buttons, in what is left above them.
    let all = CLOCK * 2 + 16 + FACE + 8 + LINE + 12 + FIELD.1;
    let row = if shown.power.0.is_some() {
        BUTTON + BOTTOM
    } else {
        0
    };
    let above = height.saturating_sub(row);
    let alone = f32::from(u16::try_from((height / 2).saturating_sub(CLOCK)).unwrap_or(0));
    let together = f32::from(u16::try_from((above / 2).saturating_sub(all / 2)).unwrap_or(0));
    let mut top = whole((together - alone).mul_add(share, alone));
    // Text is drawn in the middle of the screen, whole if it fits between
    // its margins, and cut short only if it does not.
    let (left, across) = (MARGIN.cast_signed(), width.saturating_sub(MARGIN * 2));
    // The time, large.
    let mut font = theme.font(Part::Clock, States::NONE).clone();
    font.size = omorfos_config::Bounded::clamped(CLOCK);
    let ink = theme.style(Part::Clock, States::NONE).color;
    let area = (left, top, across, CLOCK * 2);
    text.draw_centered(&mut canvas, shown.time, area, ink, &font);
    if share <= 0.0 {
        return (canvas, Vec::new());
    }
    top += (CLOCK * 2 + 16).cast_signed() + whole(RISE * (1.0 - share));
    let faint = |color: Color| Color::rgba(0, 0, 0, 0).toward(color, share);
    // Who is to unlock: their face, and what they are called.
    let (called, face) = shown.who;
    let area = (middle(FACE), top, FACE, FACE);
    match face {
        Some(face) if share < 1.0 => canvas.draw_image(area, &face.faded(share)),
        Some(face) => canvas.draw_image(area, face),
        // Without a face of their own, a disc as the prompt for a secret
        // looks, with their initials in it.
        None => {
            let look = theme.style(Part::Question, States::NONE);
            let circle = Style {
                radius: FACE / 2,
                ..look
            };
            let circle = circle.blended(&Style::NONE, share);
            canvas.draw_box(area, &circle);
            let mut font = theme.font(Part::Question, States::NONE).clone();
            font.size = omorfos_config::Bounded::clamped(INITIALS);
            text.draw_centered(&mut canvas, &initials(called), area, circle.color, &font);
        }
    }
    top += (FACE + 8).cast_signed();
    let said = faint(theme.style(Part::Question, States::NONE).color);
    let mut font = theme.font(Part::Question, States::NONE).clone();
    font.size = omorfos_config::Bounded::clamped(NAME);
    text.draw_centered(&mut canvas, called, (left, top, across, LINE), said, &font);
    top += (LINE + 12).cast_signed();
    // The field, with a dot for each character typed.
    let ways = shown.ways;
    let password = ways
        .asking
        .iter()
        .find(|asking| asking.way == Way::Password);
    // The field looks focused while the keyboard is on it and it asks.
    let asks = password.is_some_and(|password| password.asks) && shown.power.2.is_none();
    let area = (middle(FIELD.0) + shown.shift, top, FIELD.0, FIELD.1);
    paint_field(
        theme,
        (&mut canvas, text),
        (area, share),
        (shown.typed, asks),
    );
    top += FIELD.1.cast_signed() + 8;
    // What each way says: the password's first, under the field. Another
    // is shown only once it has said something, as a key does once it is
    // plugged in, so that the screen does not tell anyone what else would
    // unlock it.
    let font = theme.font(Part::Question, States::NONE);
    let lines = ways
        .asking
        .iter()
        .filter(|asking| !asking.gone && (asking.way == Way::Password || !asking.said.is_empty()));
    for asking in lines {
        let area = (left, top, across, LINE);
        text.draw_centered(&mut canvas, &asking.said, area, said, font);
        top += LINE.cast_signed();
    }
    let buttons = shown.power.0.map_or_else(Vec::new, |power| {
        let painting = (&mut canvas, text);
        let (_, hovered, focused) = shown.power;
        paint_power(theme, painting, (power, hovered, focused), (size, share))
    });
    (canvas, buttons)
}

/// Paints the field for the password in `area`, `share` of it in view:
/// focused if it `asks`, with a dot for each of the characters typed, on
/// what the theme marks them with if all are chosen, and a line where the
/// next goes while it is typed into.
fn paint_field(
    theme: &Theme,
    (canvas, text): (&mut Canvas, &mut Text),
    (area, share): (Area, f32),
    ((typed, marked), asks): ((usize, bool), bool),
) {
    let states = if asks {
        States::NONE.with(omorfos_config::State::Focused)
    } else {
        States::NONE
    };
    let field = theme
        .style(Part::QuestionInput, states)
        .blended(&Style::NONE, share);
    canvas.draw_box(area, &field);
    let dots = "\u{2022}".repeat(typed);
    let font = theme.font(Part::QuestionInput, states);
    let inside = omorfos_ui::inset(area, field.inset());
    let (left_in, top_in, room, tall) = inside;
    let wide = text.measure(&dots, font).min(room.saturating_sub(1));
    let ink = if marked {
        // All of it chosen: the dots on what the theme marks it with.
        let chosen = theme
            .style(
                Part::QuestionInput,
                states.with(omorfos_config::State::Active),
            )
            .blended(&Style::NONE, share);
        canvas.fill((left_in, top_in, wide, tall), chosen.background);
        chosen.color
    } else {
        field.color
    };
    text.draw(canvas, &dots, inside, ink, font);
    // A line where the next character will go, while the field is typed
    // into, which stays in the field however much has been typed.
    if asks && !marked {
        let margin = tall / 5;
        let caret = (
            left_in.saturating_add_unsigned(wide),
            top_in.saturating_add_unsigned(margin),
            1,
            tall.saturating_sub(margin * 2),
        );
        canvas.fill(caret, field.color);
    }
}

/// Paints the power buttons along the bottom of a screen `size` units
/// large, `share` of them in view, the one `hovered` and the one `focused`
/// as such; and above
/// them what is being asked about, why what was asked last failed, and who
/// keeps the machine from sleeping or turning off. Returns where each
/// button is.
fn paint_power(
    theme: &Theme,
    (canvas, text): (&mut Canvas, &mut Text),
    (power, hovered, focused): (&Power, Option<Button>, Option<Button>),
    ((width, height), share): ((u32, u32), f32),
) -> Vec<(Button, Area)> {
    let faint = |color: Color| Color::rgba(0, 0, 0, 0).toward(color, share);
    let said = faint(theme.style(Part::Question, States::NONE).color);
    let font = theme.font(Part::Question, States::NONE);
    let row = height.saturating_sub(BOTTOM + BUTTON).cast_signed();
    let asked = power.confirming.and_then(Act::question);
    let failed = Some(power.failed.as_str()).filter(|failed| !failed.is_empty());
    let held = power.offered.held.iter().map(String::as_str);
    let lines: Vec<&str> = held.chain(asked).chain(failed).collect();
    let mut top = row - 8 - (LINE * u32::try_from(lines.len()).unwrap_or(0)).cast_signed();
    let (left, across) = (MARGIN.cast_signed(), width.saturating_sub(MARGIN * 2));
    for line in lines {
        text.draw_centered(canvas, line, (left, top, across, LINE), said, font);
        top += LINE.cast_signed();
    }
    let buttons = power.buttons();
    let font = theme.font(Part::QuestionButton, States::NONE);
    let wide: Vec<u32> = buttons
        .iter()
        .map(|button| text.measure(button.label(), font) + PADDING * 2)
        .collect();
    let gaps = GAP * u32::try_from(wide.len().saturating_sub(1)).unwrap_or(0);
    let all = wide.iter().sum::<u32>() + gaps;
    let mut along = (width.saturating_sub(all) / 2).cast_signed();
    let mut placed = Vec::new();
    for (button, wide) in buttons.into_iter().zip(wide) {
        let mut states = States::NONE;
        if matches!(button, Button::Yes(_)) {
            states = states.with(omorfos_config::State::Active);
        }
        if hovered == Some(button) {
            states = states.with(omorfos_config::State::Hovered);
        }
        if focused == Some(button) {
            states = states.with(omorfos_config::State::Focused);
        }
        let style = theme
            .style(Part::QuestionButton, states)
            .blended(&Style::NONE, share);
        let area = (along, row, wide, BUTTON);
        canvas.draw_box(area, &style);
        let font = theme.font(Part::QuestionButton, states);
        text.draw_centered(canvas, button.label(), area, style.color, font);
        placed.push((button, area));
        along += (wide + GAP).cast_signed();
    }
    placed
}

/// The initials of what someone is called: the first letters of their
/// first and last names, or of their one name.
fn initials(called: &str) -> String {
    let mut names = called.split_whitespace();
    let first = names.next();
    let names = first.into_iter().chain(names.last());
    let letters = names.filter_map(|name| name.chars().next());
    letters.flat_map(char::to_uppercase).collect()
}

/// The nearest whole number of units to `units`.
fn whole(units: f32) -> i32 {
    omorfos_shell::whole(f64::from(units) + 0.5)
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
        let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        else {
            // Only screens are taken away. The lock surface on one that has
            // gone shows nowhere, and the compositor keeps the session
            // locked on the rest.
            return;
        };
        shell.stale = true;
        match interface.as_str() {
            "wl_compositor" => {
                shell.compositor = Some(registry.bind(name, version.min(4), handle, ()));
            }
            "wl_shm" => shell.shm = Some(registry.bind(name, 1, handle, ())),
            "wp_viewporter" => shell.viewporter = Some(registry.bind(name, 1, handle, ())),
            "wp_fractional_scale_manager_v1" => {
                shell.scales = Some(registry.bind(name, 1, handle, ()));
            }
            "ext_session_lock_manager_v1" => {
                shell.manager = Some(registry.bind(name, 1, handle, ()));
            }
            "wl_seat" => {
                registry.bind::<wl_seat::WlSeat, _, _>(name, version.min(5), handle, ());
            }
            "wl_output" => shell.screens.push(Screen {
                global: name,
                output: registry.bind(name, version.min(4), handle, ()),
                face: None,
            }),
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Shell {
    fn event(
        _: &mut Self,
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
            let typing = capabilities.contains(wl_seat::Capability::Keyboard);
            let _ = typing.then(|| seat.get_keyboard(handle, ()));
            let pointing = capabilities.contains(wl_seat::Capability::Pointer);
            let _ = pointing.then(|| seat.get_pointer(handle, ()));
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
        match event {
            wl_keyboard::Event::Keymap { fd, size, .. } => shell.typing.learn_layout(fd, size),
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                let held = (mods_depressed, mods_latched, mods_locked, group);
                shell.typing.modifiers(held);
            }
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => shell.pressed(key),
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Released),
                ..
            } => shell.repeat.released(key),
            wl_keyboard::Event::RepeatInfo { rate, delay } => shell.repeat.learn((rate, delay)),
            wl_keyboard::Event::Leave { .. } => shell.repeat.stop(),
            _ => {}
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
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let mut screens = shell.screens.iter();
                let on = screens.find(|screen| {
                    let face = screen.face.as_ref();
                    face.is_some_and(|face| face.surface == surface)
                });
                shell.pointer = on.map(|screen| (screen.global, (surface_x, surface_y)));
                shell.hover();
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                let moved = shell.pointer.map(|(on, _)| (on, (surface_x, surface_y)));
                shell.pointer = moved;
                shell.hover();
            }
            wl_pointer::Event::Leave { .. } => {
                shell.pointer = None;
                shell.hover();
            }
            // A button pressed shows who is to unlock, as a key does; and
            // once they are shown, presses the button it is over.
            wl_pointer::Event::Button {
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                let woke = shell.wake();
                shell.chosen = shell.over().filter(|_| !woke);
            }
            _ => {}
        }
    }
}

impl Dispatch<ext_session_lock_v1::ExtSessionLockV1, ()> for Shell {
    fn event(
        shell: &mut Self,
        _: &ext_session_lock_v1::ExtSessionLockV1,
        event: ext_session_lock_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // A way is asked only once the session is locked, so nothing is
        // heard of it after one has said yes.
        shell.held = match event {
            ext_session_lock_v1::Event::Locked => Held::Locked,
            _ => Held::Refused,
        };
        // What was told before is drawn now.
        shell.stale = true;
    }
}

impl Dispatch<ext_session_lock_surface_v1::ExtSessionLockSurfaceV1, u32> for Shell {
    fn event(
        shell: &mut Self,
        lock: &ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
        event: ext_session_lock_surface_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // What a lock surface is told before the session is locked is drawn
        // once it is, all at once; after, as it comes.
        if let ext_session_lock_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
            && shell.sized(lock, serial, *global, (width, height))
            && shell.held == Held::Locked
        {
            shell.stale = true;
        }
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
        // As for its size: drawn at once while locked, and else once it is.
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event
            && shell.scaled(*global, Scale::new(f64::from(scale) / 120.0))
            && shell.held == Held::Locked
        {
            shell.stale = true;
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for Shell {
    fn event(
        shell: &mut Self,
        _: &wl_callback::WlCallback,
        event: wl_callback::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // A drawing was shown, at the time it says; shaking, or coming into
        // view or going, the next of it is due.
        if let wl_callback::Event::Done { callback_data: now } = event
            && (shell.shaking.is_some() || shell.moving.is_some())
        {
            for moving in [&mut shell.shaking, &mut shell.moving] {
                if let Some((began, elapsed)) = moving.as_mut() {
                    // It was last shown as it was a frame before the first
                    // of the move, which is so far into it; time does not
                    // go back.
                    let began = *began.get_or_insert(now.saturating_sub(FRAME));
                    *elapsed = now.saturating_sub(began);
                    shell.stale = true;
                }
            }
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
        // All a buffer says is that the compositor has finished with it.
        buffer.destroy();
    }
}

delegate_noop!(Shell: wl_compositor::WlCompositor);
delegate_noop!(Shell: ignore wl_shm::WlShm);
delegate_noop!(Shell: wl_shm_pool::WlShmPool);
delegate_noop!(Shell: ignore wl_surface::WlSurface);
delegate_noop!(Shell: ignore wl_output::WlOutput);
delegate_noop!(Shell: wp_viewporter::WpViewporter);
delegate_noop!(Shell: wp_viewport::WpViewport);
delegate_noop!(Shell: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
delegate_noop!(Shell: ext_session_lock_manager_v1::ExtSessionLockManagerV1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_initials_are_of_the_first_and_last_names_or_of_the_one() {
        assert_eq!(initials("Some Middle One"), "SO");
        assert_eq!(initials("  someone  "), "S");
        assert_eq!(initials("łukasz żak"), "ŁŻ");
        assert_eq!(initials(""), "");
    }
}
