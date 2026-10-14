//! Asking the user, over everything on the screen, whether to give an
//! application what it asked for.
//!
//! The question is a box in the middle of the screen, above everything
//! and with the keyboard, saying which application asks and what for,
//! with a button to give it and one not to. Enter gives it and Escape
//! does not, as do the buttons. It takes the look of the launcher.

use std::fs::File;
use std::io::{self, Write};
use std::mem;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;

use omorfos_config::{Part, State, States, Store, Theme};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Area, Canvas, Scale, Text};
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

use crate::Failure;

/// How large the box is, in units of layout.
const SIZE: (u32, u32) = (440, 150);

/// The room between the edge of the box and what is in it.
const MARGIN: i32 = 20;

/// Where the buttons are: to give, and not to.
const GIVE: Area = (300, 98, 120, 32);
const KEEP: Area = (168, 98, 120, 32);

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// The largest keyboard layout taken in, in bytes.
const LARGEST_KEYMAP: u32 = 1024 * 1024;

/// What the user said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Give the application what it asked for.
    Give,
    /// Do not.
    Keep,
}

/// What an application asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// A picture of the screen, once.
    Picture,
    /// To see the screen for as long as it is shared.
    Share,
}

impl Question {
    /// What it is said to want, what that would let it see, and what the
    /// button that gives it says.
    #[must_use]
    pub const fn words(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::Picture => (
                "wants a picture of your screen",
                "It would see everything on every screen.",
                "Take picture",
            ),
            Self::Share => (
                "wants to share your screen",
                "It would see everything on it until you stop.",
                "Share",
            ),
        }
    }
}

/// What the box says an application wants: it, by name, or a program if
/// it has none, as one not in a sandbox has not.
#[must_use]
pub fn asking(app: &str, question: Question) -> String {
    let who = if app.is_empty() { "A program" } else { app };
    format!("{who} {}", question.words().0)
}

/// The button at a place on the box, if there is one.
#[must_use]
pub fn button_at((x, y): (i32, i32)) -> Option<Answer> {
    let inside = |(left, top, width, height): Area| {
        let (right, bottom) = (left + width.cast_signed(), top + height.cast_signed());
        (left..right).contains(&x) && (top..bottom).contains(&y)
    };
    [(GIVE, Answer::Give), (KEEP, Answer::Keep)]
        .into_iter()
        .find_map(|(area, answer)| inside(area).then_some(answer))
}

/// Paints the box: what is asked, and the two buttons, the one that gives
/// in the colour of what is picked out.
#[must_use]
pub fn paint(
    theme: &Theme,
    text: &mut Text,
    scale: Scale,
    (app, question): (&str, Question),
) -> Canvas {
    let (width, height) = SIZE;
    let mut canvas = Canvas::scaled(width, height, scale);
    let whole_box = theme.style(Part::Launcher, States::NONE);
    canvas.draw_box((0, 0, width, height), &whole_box);
    let room = width - MARGIN.unsigned_abs() * 2;
    let font = theme.font(Part::Launcher, States::NONE);
    let line = (MARGIN, MARGIN, room, 24);
    text.draw(
        &mut canvas,
        &asking(app, question),
        line,
        whole_box.color,
        font,
    );
    let said = theme.style(Part::LauncherDescription, States::NONE);
    let font = theme.font(Part::LauncherDescription, States::NONE);
    let line = (MARGIN, MARGIN + 28, room, 20);
    let (_, warning, give) = question.words();
    text.draw(&mut canvas, warning, line, said.color, font);
    let picked = States::NONE.with(State::Active);
    let buttons = [
        (GIVE, give, Part::LauncherItem, picked),
        (KEEP, "Cancel", Part::LauncherInput, States::NONE),
    ];
    for ((left, top, across, down), label, part, states) in buttons {
        let style = theme.style(part, states);
        canvas.draw_box((left, top, across, down), &style);
        let font = theme.font(part, states);
        let within = (left + 12, top + 6, across - 24, down - 12);
        text.draw(&mut canvas, label, within, style.color, font);
    }
    canvas
}

/// Says in the session's log how far putting the question up has got,
/// so that what went wrong can be found out.
fn note(said: &str) {
    let _ = writeln!(io::stderr(), "xdg-desktop-portal-omorfos: {said}");
}

/// The box on the screen.
struct Face {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    fraction: wp_fractional_scale_v1::WpFractionalScaleV1,
    /// Whether the compositor has said how large it is.
    ready: bool,
    scale: Scale,
    buffer: Option<wl_buffer::WlBuffer>,
}

impl Face {
    /// Takes the box off the screen.
    fn close(self) {
        self.fraction.destroy();
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
        self.buffer.iter().for_each(wl_buffer::WlBuffer::destroy);
    }
}

/// What the box is put on the screen with.
#[derive(Clone)]
struct Globals {
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    layers: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    viewporter: wp_viewporter::WpViewporter,
    scales: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
}

/// What the compositor has said, and the question being asked.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    face: Option<Face>,
    /// The application being asked about while the question is up, and
    /// what it is put on the screen with.
    asked: Option<(String, Question, Globals)>,
    answer: Option<Answer>,
    over: (i32, i32),
    store: Store,
    text: Text,
    layouts: xkb::Context,
    typing: Option<xkb::State>,
    /// Whether what is on the screen may not be what should be.
    stale: bool,
}

impl Shell {
    /// What the box is put on the screen with, if the compositor offers
    /// all of it.
    fn globals(&self) -> Option<Globals> {
        let globals = self.compositor.clone().zip(self.shm.clone());
        let globals = globals.zip(self.layers.clone());
        let globals = globals.zip(self.viewporter.clone().zip(self.scales.clone()));
        globals.map(
            |(((compositor, shm), layers), (viewporter, scales))| Globals {
                compositor,
                shm,
                layers,
                viewporter,
                scales,
            },
        )
    }

    /// Notes what the user said, if the question is up to be answered.
    fn answered(&mut self, answer: Option<Answer>) {
        let open = self.asked.is_some() && self.answer.is_none();
        self.answer = answer.filter(|_| open).or(self.answer);
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

    /// Takes in a key going down: Enter gives, Escape does not.
    fn pressed(&mut self, code: u32) {
        // The numbers the layout knows keys by are eight higher.
        let code = xkb::Keycode::new(code.saturating_add(8));
        let typing = self.typing.as_ref();
        let answer = typing.and_then(|typing| match typing.key_get_one_sym(code) {
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => Some(Answer::Give),
            xkb::Keysym::Escape => Some(Answer::Keep),
            _ => None,
        });
        self.answered(answer);
    }

    /// Puts the box up while there is a question, takes it down when
    /// there is not, and draws it if it no longer shows what it should.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        if !mem::take(&mut self.stale) {
            return Ok(());
        }
        let Some((app, question, globals)) = self.asked.clone() else {
            self.face.take().into_iter().for_each(Face::close);
            return Ok(());
        };
        let Globals {
            compositor,
            shm,
            layers,
            viewporter,
            scales,
        } = &globals;
        let face = self.face.get_or_insert_with(|| {
            let surface = compositor.create_surface(handle, ());
            // In the middle of the screen, above everything, with the
            // keyboard. Its name is no part of the theme's, so that it
            // goes at once and is not in the picture it leads to.
            let layer = layers.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Overlay,
                "omorfos-prompt".to_owned(),
                handle,
                (),
            );
            layer.set_size(SIZE.0, SIZE.1);
            let exclusive = zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive;
            layer.set_keyboard_interactivity(exclusive);
            let viewport = viewporter.get_viewport(&surface, handle, ());
            let fraction = scales.get_fractional_scale(&surface, handle, ());
            surface.commit();
            note("putting the question up");
            Face {
                surface,
                layer,
                viewport,
                fraction,
                ready: false,
                scale: Scale::ONE,
                buffer: None,
            }
        });
        if !face.ready {
            return Ok(());
        }
        let theme = &self.store.current().theme;
        let canvas = paint(theme, &mut self.text, face.scale, (&app, question));
        picture(shm, &canvas, handle).map(|buffer| {
            face.surface.attach(Some(&buffer), 0, 0);
            // However many pixels the picture has, it covers this much.
            let (width, height) = (SIZE.0.cast_signed(), SIZE.1.cast_signed());
            face.viewport.set_destination(width, height);
            face.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            face.surface.commit();
            if face.buffer.is_none() {
                note(&format!(
                    "the question is up, at {} to one",
                    face.scale.factor()
                ));
            }
            face.buffer = Some(buffer);
        })
    }
}

/// The connection to the compositor, and the question asked over it.
pub struct Desk {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
}

impl Desk {
    /// Speaks to the compositor over `stream`, in the look the
    /// configuration in `store` gives, once it has said what it offers,
    /// so that whether a question can be asked is known before any is.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(stream: UnixStream, store: Store) -> Result<Self, Failure> {
        let connection = Connection::from_socket(stream).map_err(Failure::from);
        let desk = connection.map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            let shell = Shell {
                compositor: None,
                shm: None,
                layers: None,
                viewporter: None,
                scales: None,
                face: None,
                asked: None,
                answer: None,
                over: (-1, -1),
                store,
                text: Text::new(),
                layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
                typing: None,
                stale: false,
            };
            Self {
                connection,
                queue,
                shell,
            }
        });
        desk.and_then(|mut desk| {
            let heard = desk.queue.roundtrip(&mut desk.shell);
            heard.map(|_| desk).map_err(Failure::from)
        })
    }

    /// The connection, over which pictures are taken as well.
    #[must_use]
    pub const fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Asks the user whether `app` is to have what it asks for. Returns
    /// whether it can be asked: it cannot if the compositor does not offer
    /// what a question is put on the screen with.
    pub fn ask(&mut self, app: &str, question: Question) -> bool {
        let globals = self.shell.globals();
        let asked = globals.is_some();
        self.shell.asked = globals.map(|globals| (app.to_owned(), question, globals));
        self.shell.answer = None;
        self.shell.stale = true;
        asked
    }

    /// Takes the question down, answered or not.
    pub fn dismiss(&mut self) {
        self.shell.asked = None;
        self.shell.answer = None;
        self.shell.stale = true;
    }

    /// Reads the configuration again, for a look that has changed.
    pub fn reread(&mut self) {
        self.shell.stale |= self.shell.store.reload();
    }

    /// Does whatever there is to do, without waiting: takes in what the
    /// compositor said, and shows what should be shown. Returns what the
    /// user said, once they have.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone.
    pub fn turn(&mut self) -> Result<Option<Answer>, Failure> {
        // Whether reading with nothing to read counts as a failure depends
        // on how the library was built, so the outcome is not looked at
        // here. A real failure is kept by the connection and comes back
        // from everything after it, down to the flush that ends the turn.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        let handled = self.queue.dispatch_pending(&mut self.shell);
        handled
            .map_err(Failure::from)
            .and_then(|_| {
                let shown = self.shell.show(&self.queue.handle());
                shown.map_err(Failure::from)
            })
            .and_then(|()| self.connection.flush().map_err(Failure::from))
            .map(|()| {
                let answer = self.shell.answer.take();
                // Once answered, the question goes.
                if answer.is_some() {
                    self.dismiss();
                }
                answer
            })
    }
}

impl AsFd for Desk {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.connection.as_fd()
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
        // A registry also says when something goes, which a question in
        // the middle of the screen does not mind.
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
            note("the compositor has made room for the question");
            shell.face.iter_mut().for_each(|face| face.ready = true);
            return;
        }
        // The only other thing it says is that the compositor closed it,
        // as when its screen goes, which is taken for a no.
        note("the compositor took the question down");
        shell.face.take().into_iter().for_each(Face::close);
        shell.answered(Some(Answer::Keep));
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
        // has, there is nothing to draw again.
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
            let pointing = capabilities.contains(wl_seat::Capability::Pointer);
            let _ = pointing.then(|| seat.get_pointer(handle, ()));
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
            wl_keyboard::Event::Keymap { fd, size, .. } => shell.learn_layout(fd, size),
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => shell.pressed(key),
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
                surface_x,
                surface_y,
                ..
            }
            | wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => shell.over = (whole(surface_x), whole(surface_y)),
            wl_pointer::Event::Button {
                button: LEFT_BUTTON,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => shell.answered(button_at(shell.over)),
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
    fn the_box_says_which_application_asks_or_that_a_program_does() {
        assert_eq!(
            asking("org.example.App", Question::Picture),
            "org.example.App wants a picture of your screen"
        );
        assert_eq!(
            asking("", Question::Picture),
            "A program wants a picture of your screen"
        );
        assert_eq!(
            asking("", Question::Share),
            "A program wants to share your screen"
        );
        assert_eq!(Question::Share.words().2, "Share");
    }

    #[test]
    fn each_button_is_where_it_is_drawn_and_nowhere_else() {
        assert_eq!(button_at((300, 98)), Some(Answer::Give));
        assert_eq!(button_at((419, 129)), Some(Answer::Give));
        assert_eq!(button_at((420, 129)), None);
        assert_eq!(button_at((168, 110)), Some(Answer::Keep));
        assert_eq!(button_at((295, 110)), None, "between them");
        assert_eq!(button_at((200, 20)), None, "on what is said");
        assert_eq!(button_at((-1, -1)), None);
    }

    #[test]
    fn the_button_that_gives_is_in_the_colour_of_what_is_picked_out() {
        let theme = Theme::default();
        let canvas = paint(
            &theme,
            &mut Text::new(),
            Scale::ONE,
            ("org.example.App", Question::Picture),
        );
        let picked = theme.style(Part::LauncherItem, States::NONE.with(State::Active));
        let unpicked = theme.style(Part::LauncherInput, States::NONE);
        let argb = |color: omorfos_config::Color| {
            u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
        };
        // Near the top left corner of each, clear of the label.
        assert_eq!(canvas.at(304, 100), argb(picked.background));
        assert_eq!(canvas.at(172, 100), argb(unpicked.background));
        assert_eq!(canvas.units(), SIZE);
    }
}
