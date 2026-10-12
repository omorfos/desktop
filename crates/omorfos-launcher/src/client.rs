//! The launcher as a program: its box on the screen, the keyboard, and the
//! request that starts what was picked.
//!
//! Nothing here decides what the launcher shows or what a key means; that
//! is [`Picker`]. This carries: keys and the pointer go in, pictures go
//! out, and one request goes to the compositor at the end.

use std::error::Error;
use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use omorfos_config::{Part, States, Store, Theme};
use omorfos_ipc::Client as Ipc;
use omorfos_shell::{picture, whole};
use omorfos_ui::{Canvas, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
    wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use xkbcommon::xkb;

use crate::{App, Icons, Key, Outcome, Picker};

/// Why the launcher could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// The most times a held key is taken again in one turn. A program that
/// was kept from running for a while does not then type a screenful.
const MOST_REPEATS: usize = 8;

/// The most of a keyboard layout that is read, in bytes. Real ones are
/// some tens of kilobytes.
const LARGEST_KEYMAP: u32 = 1024 * 1024;

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
    /// configuration in `config`. What is picked is asked for over
    /// `commands`, a connection to the compositor's own socket.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        commands: Ipc,
        config: &Path,
        apps: Vec<App>,
        icons: Icons,
    ) -> Result<Self, Failure> {
        let connection = Connection::from_socket(wayland);
        connection.map_err(Failure::from).map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            let store = Store::open(config);
            let config = store.current();
            let picker = Picker::new(apps, config.general.launcher);
            Self {
                connection,
                queue,
                shell: Shell::new(config.theme.clone(), commands, picker, icons),
            }
        })
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
        self.shell.repeat(now);
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
            let mut sources = [PollFd::new(&self.connection, PollFlags::IN)];
            let longest = self.deadline().map(|due| due.saturating_sub(now));
            let longest = longest.and_then(|longest| Timespec::try_from(longest).ok());
            // A wait that is interrupted is a wake-up like any other, and
            // anything worse shows in the turn that follows.
            let _ = poll(&mut sources, longest.as_ref());
        }
    }

    /// When there will next be something to do without anything having
    /// happened: when a key that is being held is due to be taken again.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.shell.held.map(|(_, due)| due)
    }
}

/// Runs a launcher offering `apps`, with their pictures from `icons`, in
/// the session whose Wayland socket is at `wayland` and whose own socket is
/// at `socket`, themed and set up from `config`.
pub(crate) fn run(
    wayland: &Path,
    socket: &Path,
    config: &Path,
    (apps, icons): (Vec<App>, Icons),
) -> Result<(), Failure> {
    let streams = UnixStream::connect(wayland);
    let streams = streams.and_then(|wayland| Ipc::connect(socket).map(|ipc| (wayland, ipc)));
    let launcher = streams
        .map_err(Failure::from)
        .and_then(|(wayland, commands)| Launcher::new(wayland, commands, config, apps, icons));
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
    /// What covers the rest of the screen, so that a click beside the
    /// launcher is the launcher's to hear of.
    backdrop: Option<Backdrop>,
    /// The launcher's box, once there is everything needed to make it.
    face: Option<Face>,
    /// Whether the pointer is on the box, and not on what is behind it.
    on_box: bool,
    picker: Picker,
    icons: Icons,
    theme: Theme,
    text: Text,
    /// The keyboard layout, for telling what a key types.
    layouts: xkb::Context,
    typing: Option<xkb::State>,
    /// What the time is, as whoever runs the launcher last said.
    now: Duration,
    /// How long a key is held before it is taken again, and how long
    /// between each time after that, if held keys repeat at all.
    repeating: Option<(Duration, Duration)>,
    /// The key that is being held, and when it is next taken again.
    held: Option<(u32, Duration)>,
    /// Where on the launcher the pointer is.
    over: (i32, i32),
    /// Where what was picked is asked for.
    commands: Ipc,
    /// Whether it could be asked for.
    sent: io::Result<()>,
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
    surface: wl_surface::WlSurface,
    viewport: wp_viewport::WpViewport,
    /// How large the compositor has said it is, once it has.
    size: Option<(u32, u32)>,
    /// The size it was last drawn at.
    drawn: Option<(u32, u32)>,
    buffer: Option<wl_buffer::WlBuffer>,
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
        let shade = theme.style(Part::LauncherBackdrop, States::NONE).background;
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
        picture(shm, &canvas, handle).map(|buffer| {
            self.surface.attach(Some(&buffer), 0, 0);
            self.viewport
                .set_destination(width.cast_signed(), height.cast_signed());
            self.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            self.surface.commit();
            self.buffer = Some(buffer);
            self.drawn = self.size;
        })
    }
}

/// The launcher's surface.
struct Face {
    surface: wl_surface::WlSurface,
    viewport: wp_viewport::WpViewport,
    /// Whether the compositor has said the surface may be drawn on.
    ready: bool,
    /// How many pixels the screen has to a unit.
    scale: Scale,
    /// The picture on it, which the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
}

impl Shell {
    fn new(theme: Theme, commands: Ipc, picker: Picker, icons: Icons) -> Self {
        Self {
            compositor: None,
            shm: None,
            layers: None,
            viewporter: None,
            scales: None,
            keyboard: None,
            pointer: None,
            backdrop: None,
            face: None,
            on_box: false,
            picker,
            icons,
            theme,
            text: Text::new(),
            layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            typing: None,
            now: Duration::ZERO,
            repeating: None,
            held: None,
            over: (-1, -1),
            commands,
            sent: Ok(()),
            // Worth trying to show from the very first turn.
            stale: true,
            done: false,
        }
    }

    /// Acts on what a key or a click came to.
    fn take(&mut self, outcome: Outcome) {
        match outcome {
            Outcome::Stay => self.stale = true,
            Outcome::Close => self.done = true,
            Outcome::Start(request) => {
                self.sent = self.commands.send(&request);
                self.done = true;
            }
        }
    }

    /// Takes in where the pointer now is on the surface it is on.
    fn moved(&mut self, x: f64, y: f64) {
        self.over = (whole(x), whole(y));
        let (x, y) = self.over;
        // Only on the box do places mean anything to the list.
        self.stale |= self.on_box && self.picker.point(x, y, &self.theme);
    }

    /// Takes in the keyboard layout the compositor sent.
    fn learn_layout(&mut self, layout: OwnedFd, size: u32) {
        let length = usize::try_from(size.min(LARGEST_KEYMAP)).unwrap_or(0);
        let mut bytes = vec![0; length];
        // Read from the start whatever else has read it: the compositor
        // hands every program the same file.
        let read = File::from(layout).read_exact_at(&mut bytes, 0);
        let text = read.ok().and_then(|()| String::from_utf8(bytes).ok());
        let keymap = text.and_then(|text| {
            let text = text.trim_end_matches('\0').to_owned();
            let (format, flags) = (xkb::KEYMAP_FORMAT_TEXT_V1, xkb::KEYMAP_COMPILE_NO_FLAGS);
            xkb::Keymap::new_from_string(&self.layouts, text, format, flags)
        });
        self.typing = keymap.as_ref().map(xkb::State::new);
        // A launcher that cannot be typed in, nor dismissed with a key, is
        // worse than none.
        self.done |= self.typing.is_none();
    }

    /// Takes in a key going down: it is taken once now, and again and
    /// again while it stays down, if it is a key that repeats.
    fn pressed(&mut self, code: u32) {
        self.press(code);
        let repeats = self.typing.as_ref().is_some_and(|typing| {
            let layout = typing.get_keymap();
            layout.key_repeats(xkb::Keycode::new(code.saturating_add(8)))
        });
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
        // The numbers the layout knows keys by are eight higher.
        let code = xkb::Keycode::new(code.saturating_add(8));
        let key = self
            .typing
            .as_ref()
            .and_then(|typing| match typing.key_get_one_sym(code) {
                xkb::Keysym::Escape => Some(Key::Escape),
                xkb::Keysym::Return | xkb::Keysym::KP_Enter => Some(Key::Enter),
                xkb::Keysym::BackSpace => Some(Key::Backspace),
                xkb::Keysym::Up => Some(Key::Up),
                xkb::Keysym::Down => Some(Key::Down),
                _ => {
                    let typed = typing.key_get_utf8(code);
                    (!typed.is_empty()).then_some(Key::Text(typed))
                }
            });
        // A key that types nothing, such as Shift, is no key to the launcher.
        if let Some(key) = key {
            let outcome = self.picker.key(key);
            self.take(outcome);
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
            let viewport = viewporter.get_viewport(&surface, handle, ());
            surface.commit();
            Backdrop {
                surface,
                viewport,
                size: None,
                drawn: None,
                buffer: None,
            }
        });
        let behind = self.backdrop.insert(backdrop);
        let behind = behind.draw(shm, &self.theme, handle);
        let (width, height) = Picker::size(&self.theme);
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
            layer.set_size(width, height);
            let exclusive = zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive;
            layer.set_keyboard_interactivity(exclusive);
            let viewport = viewporter.get_viewport(&surface, handle, ());
            scales.get_fractional_scale(&surface, handle, ());
            // The compositor answers this by saying it may be drawn on.
            surface.commit();
            Face {
                surface,
                viewport,
                ready: false,
                scale: Scale::ONE,
                buffer: None,
            }
        });
        let face = self.face.insert(face);
        if !face.ready {
            return behind;
        }
        let (text, icons) = (&mut self.text, &mut self.icons);
        let canvas = self.picker.paint(text, &self.theme, face.scale, icons);
        behind.and(picture(shm, &canvas, handle)).map(|buffer| {
            face.surface.attach(Some(&buffer), 0, 0);
            // However many pixels the picture has, it covers this much.
            face.viewport
                .set_destination(width.cast_signed(), height.cast_signed());
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
            let typing = capabilities.contains(wl_seat::Capability::Keyboard);
            let keyboard = shell.keyboard.take();
            shell.keyboard = keyboard.or_else(|| typing.then(|| seat.get_keyboard(handle, ())));
            let pointing = capabilities.contains(wl_seat::Capability::Pointer);
            let pointer = shell.pointer.take();
            shell.pointer = pointer.or_else(|| pointing.then(|| seat.get_pointer(handle, ())));
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
                let typing = shell.typing.iter_mut();
                typing.for_each(|typing| {
                    typing.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group);
                });
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
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let mut faces = shell.face.iter();
                shell.on_box = faces.any(|face| face.surface == surface);
                shell.moved(surface_x, surface_y);
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => shell.moved(surface_x, surface_y),
            wl_pointer::Event::Button {
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => {
                // A click beside the box is a way of saying never mind.
                let outcome = if shell.on_box {
                    shell.picker.click(x, y, &shell.theme)
                } else {
                    Outcome::Close
                };
                shell.take(outcome);
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
