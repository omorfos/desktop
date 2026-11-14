//! The card as a program: its box on the screen, the keyboard and the
//! pointer, and the choices saved as they are made.
//!
//! Nothing here decides what the card shows or what a key means; that is
//! [`Card`]. This carries: keys and the pointer go in, pictures go out,
//! and a choice goes to be saved.

use std::error::Error;
use std::fs::File;
use std::io;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use omorfos_config::{Part, States, Theme};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Canvas, Scale, Text};
use rustix::event::{PollFd, PollFlags, poll};
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

use crate::{Card, Choice, Key, Outcome};

/// Why the card could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// What saves a choice, or says why it could not.
pub type Save = Box<dyn FnMut(Choice) -> Result<(), String>>;

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// The most of a keyboard layout that is read, in bytes. Real ones are
/// some tens of kilobytes.
const LARGEST_KEYMAP: u32 = 1024 * 1024;

/// The card: a connection to the compositor as a Wayland client, and the
/// box on the screen.
///
/// It does nothing by itself. Whoever has it calls [`Welcome::turn`] when
/// there may be something to do, or leaves it to [`Welcome::serve`].
pub struct Welcome {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
}

impl Welcome {
    /// The card `card`, on the Wayland connection `wayland`, drawn under
    /// `theme`; what is chosen on it goes to `save`, and once it is closed
    /// `marked` says so.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        card: Card,
        (theme, save): (&Theme, Save),
        marked: &Path,
    ) -> Result<Self, Failure> {
        let connection = Connection::from_socket(wayland).map_err(Failure::from);
        connection.map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            Self {
                connection,
                queue,
                shell: Shell::new(card, theme.clone(), save, marked.to_owned()),
            }
        })
    }

    /// Does whatever there is to do, without waiting: takes in what the
    /// compositor said, and draws the card if it changed. Returns whether
    /// it is done with.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone or stopped making
    /// sense.
    pub fn turn(&mut self) -> Result<bool, Failure> {
        // As the launcher does: whether reading with nothing to read counts
        // as a failure depends on how the library was built.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        let handled = self.queue.dispatch_pending(&mut self.shell);
        handled
            .map(drop)
            .map_err(Failure::from)
            .and_then(|()| self.shell.show(&self.queue.handle()).map_err(Failure::from))
            .and_then(|()| self.connection.flush().map_err(Failure::from))
            .map(|()| self.shell.done)
    }

    /// Runs the card, asleep until there is something to do, until it is
    /// done with.
    ///
    /// # Errors
    ///
    /// As for [`Welcome::turn`].
    pub fn serve(&mut self) -> Result<(), Failure> {
        while !self.turn()? {
            let mut sources = [PollFd::new(&self.connection, PollFlags::IN)];
            // A wait that is interrupted is a wake-up like any other, and
            // anything worse shows in the turn that follows.
            let _ = poll(&mut sources, None);
        }
        Ok(())
    }
}

/// Shows `card` in the session whose Wayland socket is at `wayland`, until
/// it is closed.
pub(crate) fn run(
    wayland: &Path,
    card: Card,
    (theme, save): (&Theme, Save),
    marked: &Path,
) -> Result<(), Failure> {
    let stream = omorfos_shell::connect(wayland).map_err(Failure::from);
    let welcome = stream.and_then(|stream| Welcome::new(stream, card, (theme, save), marked));
    welcome.and_then(|mut welcome| welcome.serve())
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
    /// What covers the rest of the screen, so that what is behind the card
    /// is not used while it is open.
    backdrop: Option<Backdrop>,
    /// The card's box, once there is everything needed to make it.
    face: Option<Face>,
    /// Whether the pointer is on the box, and not on what is behind it.
    on_box: bool,
    card: Card,
    theme: Theme,
    text: Text,
    save: Save,
    /// Where it is marked as closed.
    marked: PathBuf,
    /// The keyboard layout, for telling what a key is.
    layouts: xkb::Context,
    typing: Option<xkb::State>,
    /// Where on the card the pointer is.
    over: (i32, i32),
    /// Whether the card may look different from what is on the screen.
    stale: bool,
    /// Whether the card is done with.
    done: bool,
}

/// Which of the card's two surfaces something is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Backdrop,
    Box,
}

/// The surface behind the card, as large as the screen.
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
    /// Draws it in the theme's colour for the launcher's backdrop, if it
    /// has a size it has not been drawn at.
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
        // As the launcher's: nothing at all is the same at any size.
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

/// The card's surface.
struct Face {
    surface: wl_surface::WlSurface,
    viewport: wp_viewport::WpViewport,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    /// Whether the compositor has said the surface may be drawn on.
    ready: bool,
    /// How many pixels the screen has to a unit.
    scale: Scale,
    /// The picture on it, which the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
}

impl Shell {
    fn new(card: Card, theme: Theme, save: Save, marked: PathBuf) -> Self {
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
            card,
            theme,
            text: Text::new(),
            save,
            marked,
            layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            typing: None,
            over: (-1, -1),
            // Worth trying to show from the very first turn.
            stale: true,
            done: false,
        }
    }

    /// Acts on what a key or a click came to.
    fn take(&mut self, outcome: Outcome) {
        self.stale = true;
        match outcome {
            Outcome::Stay => {}
            Outcome::Choose(choice) => match (self.save)(choice) {
                Ok(()) => self.card.saved(choice),
                Err(why) => self.card.refused(why),
            },
            Outcome::Close => {
                crate::mark(&self.marked);
                self.done = true;
            }
        }
    }

    /// Takes in where the pointer now is on the surface it is on.
    fn moved(&mut self, x: f64, y: f64) {
        self.over = (whole(x), whole(y));
        let (x, y) = self.over;
        self.stale |= self.on_box && self.card.point(x, y, &self.theme);
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
                xkb::Keysym::Return | xkb::Keysym::KP_Enter | xkb::Keysym::space => {
                    Some(Key::Enter)
                }
                xkb::Keysym::Tab => Some(Key::Next),
                // What Tab is with Shift held.
                xkb::Keysym::ISO_Left_Tab => Some(Key::Back),
                xkb::Keysym::Left => Some(Key::Left),
                xkb::Keysym::Right => Some(Key::Right),
                xkb::Keysym::Up => Some(Key::Up),
                xkb::Keysym::Down => Some(Key::Down),
                _ => None,
            });
        if let Some(key) = key {
            let outcome = self.card.key(key);
            self.take(outcome);
        }
    }

    /// Puts the card on the screen if it is not there, and draws it if it
    /// no longer shows what it should.
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
            let overlay = zwlr_layer_shell_v1::Layer::Overlay;
            let layer = layers.get_layer_surface(
                &surface,
                None,
                overlay,
                "welcome-backdrop".to_owned(),
                handle,
                Which::Backdrop,
            );
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
        let behind = self
            .backdrop
            .insert(backdrop)
            .draw(shm, &self.theme, handle);
        let (width, height) = Card::size(&self.theme);
        let face = self.face.take().unwrap_or_else(|| {
            let surface = compositor.create_surface(handle, ());
            // On the screen the compositor chooses, above everything, with
            // the keyboard to itself.
            let overlay = zwlr_layer_shell_v1::Layer::Overlay;
            let layer = layers.get_layer_surface(
                &surface,
                None,
                overlay,
                "welcome".to_owned(),
                handle,
                Which::Box,
            );
            let exclusive = zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive;
            layer.set_keyboard_interactivity(exclusive);
            let viewport = viewporter.get_viewport(&surface, handle, ());
            scales.get_fractional_scale(&surface, handle, ());
            Face {
                surface,
                viewport,
                layer,
                ready: false,
                scale: Scale::ONE,
                buffer: None,
            }
        });
        let face = self.face.insert(face);
        // Its size follows what it shows, as a choice refused adds a line.
        face.layer.set_size(width, height);
        if !face.ready {
            // The compositor answers this by saying it may be drawn on.
            face.surface.commit();
            return behind;
        }
        let canvas = self.card.paint(&mut self.text, &self.theme, face.scale);
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
            let backdrop = shell
                .backdrop
                .iter_mut()
                .filter(|_| *which == Which::Backdrop);
            backdrop.for_each(|backdrop| backdrop.size = Some((width, height)));
            let face = shell.face.iter_mut().filter(|_| *which == Which::Box);
            face.for_each(|face| face.ready = true);
            shell.stale = true;
            return;
        }
        // The only other thing either says is that the compositor closed
        // it, as it does a component's when it is started again; the card
        // is shown again when it is next asked for.
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
        // The scale comes in parts of 120 to one.
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
            } => shell.press(key),
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
            // A click beside the card does nothing: it is closed by its
            // button or by Escape.
            wl_pointer::Event::Button {
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } if shell.on_box => {
                let outcome = shell.card.click(x, y, &shell.theme);
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
