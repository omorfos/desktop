//! The launcher as a program: its box on the screen, the keyboard, and the
//! request that starts what was picked.
//!
//! Nothing here decides what the launcher shows or what a key means; that
//! is [`Picker`]. This carries: keys and the pointer go in, pictures go
//! out, and one request goes to the compositor at the end.

use std::error::Error;
use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use omorfos_config::{Config, Part, States, Store, Theme};
use omorfos_i18n::{LAUNCHER, Words};
use omorfos_ipc::Client as Ipc;
use omorfos_shell::sheet::Sheet;
use omorfos_shell::typing::{Key as Typed, Typing};
use omorfos_shell::whole;
use omorfos_ui::field::{Edit, Way};
use omorfos_ui::{Blink, Canvas, Motion, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_data_device, wl_data_device_manager, wl_data_offer, wl_keyboard,
    wl_pointer, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::pasting::{self, Pasting};
use crate::{App, Icons, Key, Numbers, Outcome, Picker};

/// Why the launcher could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// How far a notch of a wheel scrolls, as the compositor says it: a row
/// of the list. A touchpad says less at a time.
const NOTCH: f64 = 15.0;

/// The most rows one turn of the wheel goes by, more than the list ever
/// shows.
const MOST_SCROLLED: f64 = 64.0;

/// The most times a held key is taken again in one turn. A program that
/// was kept from running for a while does not then type a screenful.
const MOST_REPEATS: usize = 8;

/// The launcher: a connection to the compositor as a Wayland client, one to
/// its own socket, and the box on the screen.
///
/// It does nothing by itself. Whoever has it calls [`Launcher::turn`] when
/// there may be something to do, or leaves it to [`Launcher::serve`].
pub struct Launcher {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
}

impl Launcher {
    /// A launcher on the Wayland connection `wayland`, offering `apps` with
    /// their pictures from `icons`, themed and set up from the
    /// configuration in `config`, a file of it that does not load as the
    /// compositor keeps it in `kept`, if that says where, reading sums as
    /// `numbers`. What is picked
    /// is asked for over `commands`, a connection to the compositor's own
    /// socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        commands: Ipc,
        (config, kept): (&Path, Option<&Path>),
        apps: Vec<App>,
        icons: Icons,
        numbers: Numbers,
    ) -> Result<Self, Failure> {
        let connection = Connection::from_socket(wayland);
        connection.map_err(Failure::from).map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            let store = Store::open(config).starting_from(kept);
            let config = store.current();
            let picker = Picker::new(apps, config.general.launcher, numbers);
            let mut icons = icons;
            icons.set_theme(&config.general.icons.theme);
            Self {
                connection,
                queue,
                shell: Shell::new(config.theme.clone(), commands, picker, icons)
                    .blinking(blinks(config))
                    .reading_as(config.keys.ordinary()),
            }
        })
    }

    /// The same launcher showing every program as a grid, each one's icon
    /// over its name, if `grid`, rather than as a list.
    #[must_use]
    pub const fn in_grid(mut self, grid: bool) -> Self {
        self.shell.picker.set_grid(grid);
        self
    }

    /// Does whatever there is to do, without waiting, at the time `now`,
    /// counted from when the program started: takes in the keys the
    /// compositor reports, takes a key that is being held again if it is
    /// time to, and draws the launcher if it changed. Returns whether the
    /// launcher is done with: a program was picked, or it was dismissed.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone or stopped making
    /// sense.
    pub fn turn(&mut self, now: Duration) -> Result<bool, Failure> {
        self.shell.now = now;
        // Whether reading with nothing to read counts as a failure depends
        // on how the library was built, so the outcome is not looked at
        // here. A real failure is kept by the connection and comes back
        // from everything after it, down to the flush that ends the turn.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        let handled = self.queue.dispatch_pending(&mut self.shell);
        let handled = handled.map(drop).map_err(Failure::from);
        self.shell.take_paste();
        self.shell.repeat(now);
        // A row on its way to another look is drawn again as it goes, and
        // the cursor as it blinks.
        self.shell.stale |= self.shell.motion.due(now);
        self.shell.stale |= self.shell.blink.changed(now);
        handled
            .and_then(|()| {
                let shown = self.shell.show(&self.queue.handle());
                shown.map_err(Failure::from)
            })
            .and_then(|()| self.connection.flush().map_err(Failure::from))
            .and_then(|()| {
                let sent = std::mem::replace(&mut self.shell.sent, Ok(()));
                sent.map_err(Failure::from)
            })
            .map(|()| self.shell.done)
    }

    /// Runs the launcher, asleep until there is something to do, until it
    /// is done with.
    ///
    /// # Errors
    ///
    /// As for [`Launcher::turn`].
    pub fn serve(&mut self) -> Result<(), Failure> {
        let started = Instant::now();
        loop {
            let now = started.elapsed();
            if self.turn(now)? {
                return Ok(());
            }
            // What is being pasted, as it arrives, wakes it too; with
            // nothing being pasted, the connection is waited on twice.
            let pasting = self.shell.pasting.as_ref().map(Pasting::source);
            let pasting = pasting.unwrap_or_else(|| self.connection.as_fd());
            let mut sources = [
                PollFd::new(&self.connection, PollFlags::IN),
                PollFd::new(&pasting, PollFlags::IN),
            ];
            let longest = self.deadline().map(|due| due.saturating_sub(now));
            let longest = longest.and_then(|longest| Timespec::try_from(longest).ok());
            // A wait that is interrupted is a wake-up like any other, and
            // anything worse shows in the turn that follows.
            let _ = poll(&mut sources, longest.as_ref());
        }
    }

    /// When there will next be something to do without anything having
    /// happened: when a key that is being held is due to be taken again,
    /// a row that is changing colour is due to be drawn again, or the
    /// cursor is due to blink.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let held = self.shell.held.map(|(_, due)| due);
        let drawn = self.shell.motion.deadline();
        let blinks = self.shell.blink.deadline(self.shell.now);
        held.into_iter().chain(drawn).chain(blinks).min()
    }
}

/// Whether the field's cursor blinks under `config`: as the launcher moves,
/// unless motion is to be reduced, or the theme has the launcher move
/// nothing, which is how a theme turns movement off.
fn blinks(config: &Config) -> bool {
    let moves = config.theme.style(Part::Launcher, States::NONE);
    moves.transition.duration > 0 && !config.general.appearance.reduce_motion
}

/// Runs a launcher offering `apps`, with their pictures from `icons`, in
/// the session whose Wayland socket is at `wayland` and whose own socket is
/// at `socket`, themed and set up from the configuration and what is kept
/// of it, `config`, reading sums as `numbers`; as a grid, if `grid`.
pub(crate) fn run(
    wayland: &Path,
    socket: &Path,
    config: (&Path, Option<&Path>),
    (apps, icons, numbers): (Vec<App>, Icons, Numbers),
    grid: bool,
) -> Result<(), Failure> {
    let streams = omorfos_shell::connect(wayland);
    let streams = streams.and_then(|wayland| Ipc::connect(socket).map(|ipc| (wayland, ipc)));
    let launcher = streams
        .map_err(Failure::from)
        .and_then(|(wayland, commands)| {
            let launcher = Launcher::new(wayland, commands, config, apps, icons, numbers);
            launcher.map(|launcher| launcher.in_grid(grid))
        });
    launcher.and_then(|mut launcher| launcher.serve())
}

/// What the Wayland connection's events act on.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    clipboards: Option<wl_data_device_manager::WlDataDeviceManager>,
    /// What the clipboard is offered to it through, while it has the
    /// keyboard, as to any program that has it.
    clipboard: Option<wl_data_device::WlDataDevice>,
    /// What is on the clipboard, as last offered, with the forms it can be
    /// had in.
    copied: Option<wl_data_offer::WlDataOffer>,
    /// What is being pasted, as far as it has arrived.
    pasting: Option<Pasting>,
    /// What covers the rest of the screen, to dim it, and for the
    /// compositor to close the launcher at a press on it.
    backdrop: Option<Backdrop>,
    /// The launcher's box, once there is everything needed to make it.
    face: Option<Face>,
    picker: Picker,
    icons: Icons,
    theme: Theme,
    text: Text,
    /// What it says, in the user's language.
    words: Words,
    /// The keyboard as it is, for telling what a key types.
    typing: Typing,
    /// What the time is, as whoever runs the launcher last said.
    now: Duration,
    /// How long a key is held before it is taken again, and how long
    /// between each time after that, if held keys repeat at all.
    repeating: Option<(Duration, Duration)>,
    /// The key that is being held, and when it is next taken again.
    held: Option<(u32, Duration)>,
    /// Where on the launcher the pointer is.
    over: (i32, i32),
    /// How far it has been scrolled since it last went by a row, downwards.
    scrolled: f64,
    /// Where what was picked is asked for.
    commands: Ipc,
    /// Whether it could be asked for.
    sent: io::Result<()>,
    /// The rows that are on their way from one look to another.
    motion: Motion,
    /// The field's cursor blinking, from the last key or click.
    blink: Blink<Duration>,
    /// Whether the launcher may look different from what is on the screen.
    stale: bool,
    /// Whether the launcher is done with.
    done: bool,
}

/// Which of the launcher's two surfaces something is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Backdrop,
    Box,
}

/// The surface behind the launcher's box, as large as the screen.
struct Backdrop {
    sheet: Sheet,
    /// How large the compositor has said it is, once it has.
    size: Option<(u32, u32)>,
    /// The size it was last drawn at.
    drawn: Option<(u32, u32)>,
}

impl Backdrop {
    /// Draws it in the theme's colour for it, if it has a size it has not
    /// been drawn at.
    fn draw(
        &mut self,
        shm: &wl_shm::WlShm,
        theme: &Theme,
        handle: &QueueHandle<Shell>,
    ) -> io::Result<()> {
        let Some((width, height)) = self.size.filter(|size| self.drawn != Some(*size)) else {
            return Ok(());
        };
        let shade = theme.style(Part::LauncherBackdrop, States::NONE).ground();
        // Nothing at all is the same at any size, so one pixel of it will
        // do. A colour is drawn at its full size: stretched from less, it
        // would fade towards the edges of the screen.
        let (wide, tall) = if shade.alpha == 0 {
            (1, 1)
        } else {
            (width, height)
        };
        let mut canvas = Canvas::new(wide, tall);
        canvas.fill((0, 0, wide, tall), shade);
        let put = self.sheet.put(shm, &canvas, (width, height), handle);
        put.map(|()| self.drawn = self.size)
    }
}

/// The launcher's surface.
struct Face {
    sheet: Sheet,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    /// How large it was last asked to be, and how far from the top of
    /// the screen.
    size: (u32, u32),
    top: u32,
    /// Whether the compositor has said the surface may be drawn on.
    ready: bool,
    /// How many pixels the screen has to a unit.
    scale: Scale,
}

impl Shell {
    /// The same, its field reading copying, pasting and the rest from Ctrl
    /// if the keys are the `ordinary` way's, or else from Super (ADR 101).
    fn reading_as(mut self, ordinary: bool) -> Self {
        self.typing.read_as(ordinary);
        self
    }

    fn new(theme: Theme, commands: Ipc, picker: Picker, icons: Icons) -> Self {
        Self {
            compositor: None,
            shm: None,
            layers: None,
            viewporter: None,
            scales: None,
            keyboard: None,
            pointer: None,
            clipboards: None,
            clipboard: None,
            copied: None,
            pasting: None,
            backdrop: None,
            face: None,
            picker,
            icons,
            theme,
            text: Text::new(),
            words: Words::of(&LAUNCHER),
            typing: Typing::default(),
            now: Duration::ZERO,
            repeating: None,
            held: None,
            over: (-1, -1),
            scrolled: 0.0,
            commands,
            sent: Ok(()),
            motion: Motion::new(),
            blink: Blink::new(Duration::ZERO, true),
            // Worth trying to show from the very first turn.
            stale: true,
            done: false,
        }
    }

    /// The same, its cursor blinking, if it is to blink.
    const fn blinking(mut self, blinks: bool) -> Self {
        self.blink = Blink::new(Duration::ZERO, !blinks);
        self
    }

    /// Acts on what a key or a click came to, showing the cursor at once,
    /// and blinking from there.
    fn take(&mut self, outcome: Outcome) {
        self.blink.restart(self.now);
        match outcome {
            Outcome::Stay => self.stale = true,
            Outcome::Close => self.done = true,
            Outcome::Start(request) => {
                self.sent = self.commands.send(&request);
                self.done = true;
            }
            Outcome::Ask(request) => {
                self.sent = self.commands.send(&request);
                self.stale = true;
            }
        }
    }

    /// Takes in where the pointer now is on the surface it is on.
    fn moved(&mut self, x: f64, y: f64) {
        self.over = (whole(x), whole(y));
        let (x, y) = self.over;
        // Held down since it was pressed in the field, it chooses what it
        // goes over.
        let (theme, text) = (&self.theme, &mut self.text);
        self.stale |= self.picker.drag(x, y, theme, text);
        self.stale |= self.picker.point(x, y, &self.theme);
    }

    /// Takes in a scroll of the wheel or the touchpad, `by` so far downwards:
    /// the list scrolled a row for each notch, the chosen row kept as it is
    /// while it is in sight, and what is less than a notch kept for the
    /// next.
    fn scroll(&mut self, by: f64) {
        let scrolled = self.scrolled + by;
        let whole = (scrolled / NOTCH).trunc();
        let mut rows = whole.clamp(-MOST_SCROLLED, MOST_SCROLLED);
        // Beyond the most, nothing is kept: it went past the end anyway.
        self.scrolled = if whole.abs() > MOST_SCROLLED {
            0.0
        } else {
            scrolled % NOTCH
        };
        while rows >= 1.0 {
            self.stale |= self.picker.scroll(1);
            rows -= 1.0;
        }
        while rows <= -1.0 {
            self.stale |= self.picker.scroll(-1);
            rows += 1.0;
        }
    }

    /// Does what `key` does to the list.
    fn take_key(&mut self, key: Key) {
        let outcome = self.picker.key(key);
        self.take(outcome);
    }

    /// Asks for what is on the clipboard, as text, if it can be had so; it
    /// is typed once it has arrived. Pasting again before then pastes the
    /// later.
    fn paste(&mut self) {
        let copied = self.copied.as_ref();
        let forms = copied.and_then(|copied| copied.data::<Forms>());
        let forms = forms.map_or_default(Forms::taken);
        let text = pasting::TEXTS
            .into_iter()
            .find(|text| forms.iter().any(|form| form == text));
        // A pipe that cannot be made is nothing pasted.
        let piped = copied
            .zip(text)
            .and_then(|asked| io::pipe().ok().map(|pipe| (asked, pipe)));
        let asked = piped.map(|((copied, text), (from, to))| {
            copied.receive(text.to_owned(), to.as_fd());
            Pasting::new(from)
        });
        self.pasting = asked;
    }

    /// Types what was pasted once it has all arrived.
    fn take_paste(&mut self) {
        let typed = self.pasting.as_mut().and_then(Pasting::take);
        if let Some(typed) = typed {
            self.pasting = None;
            self.take_key(Key::Text(typed));
        }
    }

    /// Takes in the keyboard layout the compositor sent.
    fn learn_layout(&mut self, layout: OwnedFd, size: u32) {
        self.typing.learn_layout(layout, size);
        // A launcher that cannot be typed in, nor dismissed with a key, is
        // worse than none.
        self.done |= !self.typing.known();
    }

    /// Takes in a key going down: it is taken once now, and again and
    /// again while it stays down, if it is a key that repeats.
    fn pressed(&mut self, code: u32) {
        self.press(code);
        let repeats = self.typing.repeats(code);
        let repeating = self.repeating.filter(|_| repeats);
        let held = repeating.map(|(delay, _)| (code, self.now.saturating_add(delay)));
        // A key that does not repeat, such as Shift, going down leaves the
        // one that is being held as it is.
        self.held = held.or(self.held);
    }

    /// Takes in a key coming up.
    fn released(&mut self, code: u32) {
        self.held = self.held.filter(|(held, _)| *held != code);
    }

    /// Takes the key that is being held again, as often as is due by
    /// `now`.
    fn repeat(&mut self, now: Duration) {
        let interval = self
            .repeating
            .map_or(Duration::MAX, |(_, interval)| interval);
        for _ in 0..MOST_REPEATS {
            let Some((code, due)) = self.held.filter(|(_, due)| *due <= now && !self.done) else {
                return;
            };
            self.press(code);
            self.held = Some((code, due.saturating_add(interval)));
        }
        // Too far behind to catch up: carry on from now.
        self.held = self
            .held
            .map(|(code, _)| (code, now.saturating_add(interval)));
    }

    /// Takes a key that was pressed, by its number.
    fn press(&mut self, code: u32) {
        // Read as every field of the shell reads keys, Ctrl and A, or Super
        // and A, choosing all that is typed.
        let key = match self.typing.pressed(code) {
            Typed::Enter => Some(Key::Enter),
            Typed::Escape => Some(Key::Escape),
            Typed::Back(reach) => Some(Key::Edit(Edit::Remove {
                way: Way::Back,
                reach,
            })),
            Typed::Delete(reach) => Some(Key::Edit(Edit::Remove {
                way: Way::On,
                reach,
            })),
            Typed::Move {
                way,
                reach,
                choosing,
            } => Some(Key::Edit(Edit::Move {
                way,
                reach,
                choosing,
            })),
            Typed::Undo => Some(Key::Edit(Edit::Undo)),
            Typed::Redo => Some(Key::Edit(Edit::Redo)),
            Typed::Copy => Some(Key::Copy),
            Typed::Cut => Some(Key::Cut),
            Typed::Paste => {
                self.paste();
                None
            }
            Typed::All => Some(Key::All),
            Typed::Up => Some(Key::Up),
            Typed::Down => Some(Key::Down),
            Typed::Typed(typed) => Some(Key::Text(typed)),
            Typed::Next | Typed::Previous | Typed::Mode(_) | Typed::Nothing => None,
        };
        // A key that types nothing, such as Shift, is no key to the launcher.
        if let Some(key) = key {
            self.take_key(key);
        }
    }

    /// Puts the launcher on the screen if it is not there, and draws it if
    /// it no longer shows what it should.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        if !std::mem::take(&mut self.stale) {
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
        // The backdrop first, so that the box is put over it.
        let backdrop = self.backdrop.take().unwrap_or_else(|| {
            let surface = compositor.create_surface(handle, ());
            let layer = layers.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Overlay,
                "launcher-backdrop".to_owned(),
                handle,
                Which::Backdrop,
            );
            // Every edge of the screen, panels and all.
            layer.set_anchor(zwlr_layer_surface_v1::Anchor::all());
            layer.set_exclusive_zone(-1);
            // It takes no pointer: what is behind it is pointed at and
            // scrolled through it, and the compositor closes the launcher
            // at a press there.
            let nowhere = compositor.create_region(handle, ());
            surface.set_input_region(Some(&nowhere));
            nowhere.destroy();
            let viewport = viewporter.get_viewport(&surface, handle, ());
            surface.commit();
            Backdrop {
                sheet: Sheet::new(surface, viewport),
                size: None,
                drawn: None,
            }
        });
        let behind = self.backdrop.insert(backdrop);
        let screen = behind.size.map(|(_, height)| height);
        let behind = behind.draw(shm, &self.theme, handle);
        // Until the backdrop is told how tall the screen is, the box cannot
        // be told where to stand, nor how many rows it has room for.
        let Some(screen) = screen else {
            return behind;
        };
        self.picker.fit(&self.theme, screen);
        let (width, height) = self.picker.size(&self.theme);
        let top = self.picker.top(&self.theme, screen);
        let face = self.face.take().unwrap_or_else(|| {
            let surface = compositor.create_surface(handle, ());
            // On the screen the compositor chooses, which is the one the
            // pointer is on, above everything, with the keyboard to itself.
            let layer = layers.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Overlay,
                "launcher".to_owned(),
                handle,
                Which::Box,
            );
            // Its top stays where it is put, and it grows and shrinks
            // downwards, so that what is typed does not move; it is put
            // from the very top of the screen, panels and all, as the
            // backdrop covers it.
            layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top);
            layer.set_exclusive_zone(-1);
            layer.set_margin(top.cast_signed(), 0, 0, 0);
            layer.set_size(width, height);
            let exclusive = zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive;
            layer.set_keyboard_interactivity(exclusive);
            let viewport = viewporter.get_viewport(&surface, handle, ());
            scales.get_fractional_scale(&surface, handle, ());
            // The compositor answers this by saying it may be drawn on.
            surface.commit();
            Face {
                sheet: Sheet::new(surface, viewport),
                layer,
                size: (width, height),
                top,
                ready: false,
                scale: Scale::ONE,
            }
        });
        let face = self.face.insert(face);
        if !face.ready {
            return behind;
        }
        // As many rows as match, and the screen it is on may have changed.
        if (face.size, face.top) != ((width, height), top) {
            face.layer.set_size(width, height);
            face.layer.set_margin(top.cast_signed(), 0, 0, 0);
            (face.size, face.top) = ((width, height), top);
        }
        let (text, icons) = ((&mut self.text, &self.words), &mut self.icons);
        self.motion.begin(self.now);
        let look = (&self.theme, face.scale, self.blink.draw(self.now));
        let canvas = self.picker.paint(text, look, icons, &mut self.motion);
        self.motion.end();
        behind.and(face.sheet.put(shm, &canvas, (width, height), handle))
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
        // A registry also says when something goes, which is of no
        // interest to a program this short-lived.
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
                "wl_data_device_manager" => {
                    shell.clipboards = Some(registry.bind(name, version.min(3), handle, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, Which> for Shell {
    fn event(
        shell: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        which: &Which,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            layer.ack_configure(serial);
            let backdrop = shell.backdrop.iter_mut();
            let backdrop = backdrop.filter(|_| *which == Which::Backdrop);
            backdrop.for_each(|backdrop| backdrop.size = Some((width, height)));
            let face = shell.face.iter_mut().filter(|_| *which == Which::Box);
            face.for_each(|face| face.ready = true);
            shell.stale = true;
            return;
        }
        // The only other thing either says is that the compositor closed
        // it.
        shell.done = true;
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
        // The scale comes in parts of 120 to one. Told the one it already
        // has, the launcher has nothing to do over.
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event
            && let scale = Scale::from_numerator(scale)
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
            let typing = capabilities.contains(wl_seat::Capability::Keyboard);
            let keyboard = shell.keyboard.take();
            shell.keyboard = keyboard.or_else(|| typing.then(|| seat.get_keyboard(handle, ())));
            let pointing = capabilities.contains(wl_seat::Capability::Pointer);
            let pointer = shell.pointer.take();
            shell.pointer = pointer.or_else(|| pointing.then(|| seat.get_pointer(handle, ())));
            // Announced with the seat, as every global is at the start.
            let clipboards = shell.clipboards.as_ref();
            let clipboard = shell.clipboard.take();
            shell.clipboard = clipboard.or_else(|| {
                clipboards.map(|clipboards| clipboards.get_data_device(seat, handle, ()))
            });
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
            wl_keyboard::Event::Keymap { fd, size, .. } => shell.learn_layout(fd, size),
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
            } => shell.released(key),
            wl_keyboard::Event::RepeatInfo { rate, delay } => {
                // So many times a second, after so many milliseconds. No
                // times a second is not at all.
                let delay = Duration::from_millis(u64::from(delay.unsigned_abs()));
                let rate = u32::try_from(rate).ok().filter(|rate| *rate > 0);
                shell.repeating = rate.map(|rate| (delay, Duration::from_secs(1) / rate));
            }
            // The keyboard went elsewhere: something else wants the user.
            wl_keyboard::Event::Leave { .. } => shell.done = true,
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
        let (x, y) = shell.over;
        match event {
            // Only ever on the box: the backdrop takes no pointer.
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
            wl_pointer::Event::Axis {
                axis: WEnum::Value(wl_pointer::Axis::VerticalScroll),
                value,
                ..
            } => shell.scroll(value),
            wl_pointer::Event::Button {
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                let outcome = shell.picker.click(x, y, &shell.theme, &mut shell.text);
                shell.take(outcome);
            }
            wl_pointer::Event::Button {
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Released),
                ..
            } => shell.picker.release(),
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

/// The forms what is on the clipboard can be had in, as they are offered.
#[derive(Debug, Default)]
struct Forms(std::sync::Mutex<Vec<String>>);

impl Forms {
    /// The forms offered so far.
    fn taken(&self) -> Vec<String> {
        self.0.lock().map_or_default(|forms| forms.clone())
    }
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for Shell {
    fn event(
        shell: &mut Self,
        _: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Of what is dragged over it, nothing is taken.
        if let wl_data_device::Event::Selection { id } = event {
            shell.copied = id;
        }
    }

    wayland_client::event_created_child!(Shell, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wl_data_offer::WlDataOffer, Forms::default()),
    ]);
}

impl Dispatch<wl_data_offer::WlDataOffer, Forms> for Shell {
    fn event(
        _: &mut Self,
        _: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        forms: &Forms,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            // Only this thread ever holds it.
            let _ = forms.0.lock().map(|mut forms| forms.push(mime_type));
        }
    }
}

delegate_noop!(Shell: wl_compositor::WlCompositor);
delegate_noop!(Shell: wl_region::WlRegion);
delegate_noop!(Shell: wl_data_device_manager::WlDataDeviceManager);
delegate_noop!(Shell: ignore wl_shm::WlShm);
delegate_noop!(Shell: wl_shm_pool::WlShmPool);
delegate_noop!(Shell: ignore wl_surface::WlSurface);
delegate_noop!(Shell: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Shell: wp_viewporter::WpViewporter);
delegate_noop!(Shell: wp_viewport::WpViewport);
delegate_noop!(Shell: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
