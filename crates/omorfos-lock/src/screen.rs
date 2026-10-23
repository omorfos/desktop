//! The lock screen as a Wayland client: the session locked, a surface on
//! every screen with the time, a field for the password and what each way
//! of unlocking says, and the session unlocked once one says yes.

use std::error::Error;
use std::io;
use std::mem;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Instant;

use omorfos_config::{Part, States, Store, Theme};
use omorfos_shell::picture;
use omorfos_shell::said::Said;
use omorfos_shell::shake::{FRAME, SHAKING, shaken};
use omorfos_shell::typing::{Key, Typing};
use omorfos_ui::{Canvas, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::pipe::{PipeFlags, pipe_with};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_output, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1, ext_session_lock_surface_v1, ext_session_lock_v1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};

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

/// The lock screen: a connection to the compositor as a Wayland client,
/// and the helpers asking PAM.
pub struct Lock {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
    /// What the helpers said, and what wakes the lock screen when they do.
    heard: Receiver<(Way, Said)>,
    heard_by: (Sender<(Way, Said)>, Waker),
    woken: OwnedFd,
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
    /// each of `ways` by a helper that is `helper`.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        config: &Path,
        (helper, user): (PathBuf, String),
        ways: &[Way],
    ) -> Result<Self, Failure> {
        let connection = Connection::from_socket(wayland).map_err(Failure::from);
        let piped = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(Failure::from);
        connection.and_then(|connection| {
            piped.map(|(woken, waking)| {
                let queue = connection.new_event_queue();
                connection.display().get_registry(&queue.handle(), ());
                let (sender, heard) = mpsc::channel();
                Self {
                    connection,
                    queue,
                    shell: Shell::new(Store::open(config), Ways::new(ways, helper, user)),
                    heard,
                    heard_by: (sender, Waker(Arc::new(waking))),
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
            match self.shell.ways.heard(said, now) {
                Came::Right => self.shell.held = Held::Unlocking,
                Came::Wrong => self.shell.shake(),
                Came::Going => {}
            }
            self.shell.stale = true;
        }
        let refused = (self.shell.held == Held::Refused).then_some(());
        let lockable = refused.map_or(Ok(()), |()| {
            Err(Failure::from(
                "the session is being locked by another program",
            ))
        });
        let handle = self.queue.handle();
        let gone_on = lockable.and_then(|()| {
            let gone_on = self.shell.go_on(now, &handle, &self.heard_by);
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
    /// happened: the time changing, or a helper due to be asked again.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        let clock = self.shell.clock.as_ref().map(|(_, changes)| *changes);
        clock.into_iter().chain(self.shell.again).min()
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
    ways: Ways,
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
}

impl Shell {
    fn new(store: Store, ways: Ways) -> Self {
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
            ways,
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
        heard_by: &(Sender<(Way, Said)>, Waker),
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
        if self.held == Held::Locked {
            let wake = heard_by.1.clone();
            let heard_by = (heard_by.0.clone(), move || wake.wake());
            self.again = self.ways.start(now, &heard_by);
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
        let faces = self
            .screens
            .iter_mut()
            .filter_map(|screen| screen.face.as_mut());
        let mut sized = faces.filter(|face| face.size.0 > 0 && face.size.1 > 0);
        sized.try_for_each(|face| {
            let shown = (time, self.typed.chars().count(), ways, shift);
            let canvas = paint(&theme, &mut self.text, shown, face.size, face.scale);
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
    /// takes the last character out, Escape empties it, and Enter hands it
    /// to the password's helper if it is asking.
    fn pressed(&mut self, code: u32) {
        match self.typing.pressed(code) {
            Key::Enter => {
                if self.ways.answer(&self.typed) {
                    wipe(&mut self.typed);
                }
            }
            Key::Escape => wipe(&mut self.typed),
            Key::Back => {
                self.typed.pop();
            }
            Key::Typed(typed) => {
                // No more than a secret may be is kept.
                let room = LONGEST.saturating_sub(self.typed.chars().count());
                self.typed.extend(typed.chars().take(room));
            }
            Key::Nothing => {}
        }
        self.stale = true;
    }
}

/// Wipes what was typed, so that it is not left in memory.
fn wipe(typed: &mut String) {
    let mut bytes = mem::take(typed).into_bytes();
    bytes.fill(0);
}

/// Paints a screen `size` units large at `scale`: the time, the field with
/// a dot for each of `typed` characters, moved `shift` across as it shakes,
/// what the password's helper said under it, and what each other way says
/// or offers beneath.
fn paint(
    theme: &Theme,
    text: &mut Text,
    (time, typed, ways, shift): (&str, usize, &Ways, i32),
    size: (u32, u32),
    scale: Scale,
) -> Canvas {
    let (width, height) = size;
    let mut canvas = Canvas::scaled(width, height, scale);
    let behind = theme.style(Part::Desktop, States::NONE).background;
    canvas.fill((0, 0, width, height), behind);
    let middle = |length: u32| (width.saturating_sub(length) / 2).cast_signed();
    let mut top = (height / 2).saturating_sub(CLOCK * 2).cast_signed();
    // The time, large.
    let mut font = theme.font(Part::Clock, States::NONE).clone();
    font.size = omorfos_config::Bounded::clamped(CLOCK);
    let ink = theme.style(Part::Clock, States::NONE).color;
    let wide = text.measure(time, &font);
    text.draw(
        &mut canvas,
        time,
        (middle(wide), top, wide, CLOCK * 2),
        ink,
        &font,
    );
    top += (CLOCK * 2).cast_signed();
    // The field, with a dot for each character typed.
    let password = ways
        .asking
        .iter()
        .find(|asking| asking.way == Way::Password);
    let asks = password.is_some_and(|password| password.asks);
    let states = if asks {
        States::NONE.with(omorfos_config::State::Focused)
    } else {
        States::NONE
    };
    let field = theme.style(Part::QuestionInput, states);
    let area = (middle(FIELD.0) + shift, top, FIELD.0, FIELD.1);
    canvas.draw_box(area, &field);
    let dots = "\u{2022}".repeat(typed);
    let font = theme.font(Part::QuestionInput, states);
    let inside = omorfos_ui::inset(area, field.inset());
    text.draw(&mut canvas, &dots, inside, field.color, font);
    top += FIELD.1.cast_signed() + 8;
    // What each way says: the password's first, under the field.
    let said = theme.style(Part::Question, States::NONE).color;
    let font = theme.font(Part::Question, States::NONE);
    for asking in ways.asking.iter().filter(|asking| !asking.gone) {
        let line = match (asking.way, asking.said.as_str()) {
            (Way::Fingerprint, "") => "Or a finger on the reader".to_owned(),
            (Way::Key, "") => "Or a security key".to_owned(),
            (_, said) => said.to_owned(),
        };
        let wide = text.measure(&line, font).min(width);
        text.draw(
            &mut canvas,
            &line,
            (middle(wide), top, wide, LINE),
            said,
            font,
        );
        top += LINE.cast_signed();
    }
    canvas
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
        // A drawing was shown, at the time it says; shaking, the next of the
        // shake is due.
        if let wl_callback::Event::Done { callback_data: now } = event
            && let Some((began, elapsed)) = shell.shaking.as_mut()
        {
            // The field was last shown at rest a frame before the first of
            // the shake, which is so far into it; time does not go back.
            let began = *began.get_or_insert(now.saturating_sub(FRAME));
            *elapsed = now.saturating_sub(began);
            shell.stale = true;
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
