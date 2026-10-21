//! The prompt: a box in the middle of the screen that asks for a secret,
//! on the overlay layer, with the keyboard. Started with the `ask` role,
//! it is sealed by the compositor: the session is dimmed behind it, and
//! nothing else sees a key while it is up.
//!
//! It says what is asked for, whose secret it wants, and has a field the
//! secret is typed into, shown as dots unless asked to be shown, and
//! buttons to show it, to give up and to go on. Enter goes on and Escape
//! gives up. It is drawn as the `question` parts of the theme.

use std::fs::File;
use std::io;
use std::mem;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;

use omorfos_config::{Part, State, States, Store, Theme};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Area, Canvas, Scale, Text};
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

use crate::Failure;

/// How large the box is, in units of layout.
pub const SIZE: (u32, u32) = (440, 216);

/// The room between the edge of the box and what is in it.
const MARGIN: i32 = 20;

/// Where the field is that the secret is typed into.
pub const FIELD: Area = (MARGIN, 100, 400, 32);

/// Where the button is that shows what is typed, or hides it again.
pub const SHOW: Area = (MARGIN, 164, 96, 32);
/// Where the button is that gives up.
pub const CANCEL: Area = (168, 164, 120, 32);
/// Where the button is that goes on.
pub const GO: Area = (300, 164, 120, 32);

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

/// The largest keyboard layout taken in, in bytes.
const LARGEST_KEYMAP: u32 = 1024 * 1024;

/// The longest secret taken in, in characters.
const LONGEST: usize = 1024;

/// What the prompt asks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    /// What is asked for, as polkit says it.
    pub message: String,
    /// Whose secret it is.
    pub user: String,
    /// What is asked, as PAM says it: `Password: ` most often.
    pub question: String,
    /// Whether the answer may be shown as it is typed.
    pub open: bool,
    /// What went wrong, or something to know, said under the field.
    pub said: String,
    /// Whether what was typed is being checked, and nothing more can be
    /// typed meanwhile.
    pub checking: bool,
}

/// What the user did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Went on, with what they typed.
    Typed(String),
    /// Gave up.
    Cancel,
}

/// A button, by what it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    /// Shows what is typed, or hides it again.
    Show,
    /// Gives up.
    Cancel,
    /// Goes on.
    Go,
}

/// The button at a place on the box, if there is one.
#[must_use]
pub fn button_at((x, y): (i32, i32)) -> Option<Button> {
    let inside = |(left, top, width, height): Area| {
        let (right, bottom) = (left + width.cast_signed(), top + height.cast_signed());
        (left..right).contains(&x) && (top..bottom).contains(&y)
    };
    [
        (SHOW, Button::Show),
        (CANCEL, Button::Cancel),
        (GO, Button::Go),
    ]
    .into_iter()
    .find_map(|(area, button)| inside(area).then_some(button))
}

/// What the field is called: whose password it is, where PAM asks for
/// one, and what PAM asks otherwise.
#[must_use]
pub fn label(question: &str, user: &str) -> String {
    let question = question.trim().trim_end_matches(':').trim_end();
    if question.eq_ignore_ascii_case("password") {
        format!("Password for {user}")
    } else {
        question.to_owned()
    }
}

/// What the field shows of `typed`: itself if it may be shown, and a dot
/// for each character otherwise.
#[must_use]
pub fn shown(typed: &str, open: bool) -> String {
    if open {
        typed.to_owned()
    } else {
        "\u{2022}".repeat(typed.chars().count())
    }
}

/// Paints the box: what is asked for, the field with what is typed shown
/// or hidden, what was said, and the buttons, the one that goes on picked
/// out.
#[must_use]
pub fn paint(
    theme: &Theme,
    text: &mut Text,
    scale: Scale,
    (asking, typed, open): (&Asking, &str, bool),
) -> Canvas {
    let (width, height) = SIZE;
    let mut canvas = Canvas::scaled(width, height, scale);
    let whole_box = theme.style(Part::Question, States::NONE);
    canvas.draw_box((0, 0, width, height), &whole_box);
    let room = width - MARGIN.unsigned_abs() * 2;
    let font = theme.font(Part::Question, States::NONE);
    let title = "Authentication is required";
    text.draw(
        &mut canvas,
        title,
        (MARGIN, MARGIN, room, 24),
        whole_box.color,
        font,
    );
    let detail = theme.style(Part::QuestionDetail, States::NONE);
    let font = theme.font(Part::QuestionDetail, States::NONE);
    text.draw(
        &mut canvas,
        &asking.message,
        (MARGIN, MARGIN + 28, room, 20),
        detail.color,
        font,
    );
    let called = label(&asking.question, &asking.user);
    text.draw(
        &mut canvas,
        &called,
        (MARGIN, 76, room, 20),
        detail.color,
        font,
    );
    let typing = if asking.checking {
        States::NONE
    } else {
        States::NONE.with(State::Focused)
    };
    let field = theme.style(Part::QuestionInput, typing);
    canvas.draw_box(FIELD, &field);
    let font = theme.font(Part::QuestionInput, typing);
    let (left, top, across, down) = FIELD;
    let within = (left + 8, top + 6, across - 16, down - 12);
    text.draw(
        &mut canvas,
        &shown(typed, open || asking.open),
        within,
        field.color,
        font,
    );
    text.draw(
        &mut canvas,
        &asking.said,
        (MARGIN, 140, room, 20),
        detail.color,
        font,
    );
    let showing = if open { "Hide" } else { "Show" };
    let picked = States::NONE.with(State::Active);
    let buttons = [
        (SHOW, showing, States::NONE),
        (CANCEL, "Cancel", States::NONE),
        (GO, "Authenticate", picked),
    ];
    for ((left, top, across, down), label, states) in buttons {
        let style = theme.style(Part::QuestionButton, states);
        canvas.draw_box((left, top, across, down), &style);
        let font = theme.font(Part::QuestionButton, states);
        let within = (left + 10, top + 6, across - 20, down - 12);
        text.draw(&mut canvas, label, within, style.color, font);
    }
    canvas
}

/// The box on the screen.
struct Face {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    fraction: wp_fractional_scale_v1::WpFractionalScaleV1,
    /// Whether the compositor has made room for it.
    ready: bool,
    scale: Scale,
    buffer: Option<wl_buffer::WlBuffer>,
}

impl Face {
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

/// What the compositor has said, and what is being asked.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    face: Option<Face>,
    /// What is asked while it is up, and what it is put up with.
    asked: Option<(Asking, Globals)>,
    /// What is typed, and whether it is shown.
    typed: String,
    open: bool,
    answer: Option<Answer>,
    /// Where the pointer is on the box, while it is over it.
    over: Option<(i32, i32)>,
    store: Store,
    text: Text,
    layouts: xkb::Context,
    typing: Option<xkb::State>,
    /// Whether what is on the screen may not be what should be.
    stale: bool,
}

/// Wipes what was typed, so that it is not left in memory.
fn wipe(typed: &mut String) {
    let mut bytes = mem::take(typed).into_bytes();
    bytes.fill(0);
}

impl Shell {
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

    /// Notes what the user did, if something is asked and is not being
    /// checked.
    fn answered(&mut self, answer: Answer) {
        let open = self
            .asked
            .as_ref()
            .is_some_and(|(asking, _)| !asking.checking);
        if open && self.answer.is_none() {
            self.answer = Some(answer);
        }
    }

    /// Goes on with what is typed, which is no longer kept here.
    fn go(&mut self) {
        let typed = mem::take(&mut self.typed);
        self.answered(Answer::Typed(typed));
        self.stale = true;
    }

    /// Takes in the keyboard layout the compositor sent.
    fn learn_layout(&mut self, layout: OwnedFd, size: u32) {
        let length = usize::try_from(size.min(LARGEST_KEYMAP)).unwrap_or(0);
        let mut bytes = vec![0; length];
        let read = File::from(layout).read_exact_at(&mut bytes, 0);
        let text = read.ok().and_then(|()| String::from_utf8(bytes).ok());
        let keymap = text.and_then(|text| {
            let text = text.trim_end_matches('\0').to_owned();
            let (format, flags) = (xkb::KEYMAP_FORMAT_TEXT_V1, xkb::KEYMAP_COMPILE_NO_FLAGS);
            xkb::Keymap::new_from_string(&self.layouts, text, format, flags)
        });
        self.typing = keymap.as_ref().map(xkb::State::new);
    }

    /// Takes in a key going down: what it types goes in the field, `⌫`
    /// takes the last character out, Enter goes on and Escape gives up.
    fn pressed(&mut self, code: u32) {
        let checking = self
            .asked
            .as_ref()
            .is_none_or(|(asking, _)| asking.checking);
        let Some(typing) = self.typing.as_ref().filter(|_| !checking) else {
            return;
        };
        // The numbers the layout knows keys by are eight higher.
        let code = xkb::Keycode::new(code.saturating_add(8));
        let held = |name| typing.mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE);
        let chord = held(xkb::MOD_NAME_CTRL) || held(xkb::MOD_NAME_ALT) || held(xkb::MOD_NAME_LOGO);
        match typing.key_get_one_sym(code) {
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => self.go(),
            xkb::Keysym::Escape => self.answered(Answer::Cancel),
            xkb::Keysym::BackSpace => {
                self.typed.pop();
                self.stale = true;
            }
            _ => {
                let typed = typing.key_get_utf8(code);
                let printable = typed.chars().all(|character| !character.is_control());
                let room = self.typed.chars().count() < LONGEST;
                if !chord && printable && room {
                    self.typed.push_str(&typed);
                    self.stale = true;
                }
            }
        }
    }

    /// Takes in a button going down on the box.
    fn clicked(&mut self, button: u32) {
        let on = self
            .over
            .and_then(button_at)
            .filter(|_| button == LEFT_BUTTON);
        match on {
            Some(Button::Show) => {
                self.open = !self.open;
                self.stale = true;
            }
            Some(Button::Cancel) => self.answered(Answer::Cancel),
            Some(Button::Go) => self.go(),
            None => {}
        }
    }

    /// Puts the box up while something is asked, takes it down when it is
    /// not, and draws it if it no longer shows what it should.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        if !mem::take(&mut self.stale) {
            return Ok(());
        }
        let Some((asking, globals)) = &self.asked else {
            self.face.take().into_iter().for_each(Face::close);
            self.over = None;
            return Ok(());
        };
        let face = self.face.get_or_insert_with(|| Face::new(globals, handle));
        if !face.ready {
            return Ok(());
        }
        let theme = &self.store.current().theme;
        let canvas = paint(
            theme,
            &mut self.text,
            face.scale,
            (asking, &self.typed, self.open),
        );
        picture(&globals.shm, &canvas, handle).map(|buffer| {
            face.surface.attach(Some(&buffer), 0, 0);
            let (width, height) = (SIZE.0.cast_signed(), SIZE.1.cast_signed());
            face.viewport.set_destination(width, height);
            face.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            face.surface.commit();
            face.buffer = Some(buffer);
        })
    }
}

impl Face {
    fn new(globals: &Globals, handle: &QueueHandle<Shell>) -> Self {
        let surface = globals.compositor.create_surface(handle, ());
        let layer = globals.layers.get_layer_surface(
            &surface,
            None,
            zwlr_layer_shell_v1::Layer::Overlay,
            "omorfos-ask".to_owned(),
            handle,
            (),
        );
        layer.set_size(SIZE.0, SIZE.1);
        let exclusive = zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive;
        layer.set_keyboard_interactivity(exclusive);
        let viewport = globals.viewporter.get_viewport(&surface, handle, ());
        let fraction = globals.scales.get_fractional_scale(&surface, handle, ());
        surface.commit();
        Self {
            surface,
            layer,
            viewport,
            fraction,
            ready: false,
            scale: Scale::ONE,
            buffer: None,
        }
    }
}

/// The connection to the compositor, and what is asked over it.
pub struct Desk {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
}

impl Desk {
    /// Speaks to the compositor over `stream`, in the look the
    /// configuration in `store` gives, once it has said what it offers.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(stream: UnixStream, store: Store) -> Result<Self, Failure> {
        let connection = Connection::from_socket(stream).map_err(Failure::from);
        connection.and_then(|connection| {
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
                typed: String::new(),
                open: false,
                answer: None,
                over: None,
                store,
                text: Text::new(),
                layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
                typing: None,
                stale: false,
            };
            let mut desk = Self {
                connection,
                queue,
                shell,
            };
            let heard = desk.queue.roundtrip(&mut desk.shell);
            heard.map(|_| desk).map_err(Failure::from)
        })
    }

    /// Asks, or says anew what is asked, as when what was typed is being
    /// checked or was not right. What is typed is kept unless `afresh`.
    /// Returns whether it can be asked: it cannot if the compositor does
    /// not offer what the box is put on the screen with.
    pub fn ask(&mut self, asking: Asking, afresh: bool) -> bool {
        if afresh {
            wipe(&mut self.shell.typed);
            self.shell.answer = None;
        }
        let globals = self.shell.globals();
        let asked = globals.is_some();
        self.shell.asked = globals.map(|globals| (asking, globals));
        self.shell.stale = true;
        asked
    }

    /// Takes the box down, and forgets what was typed.
    pub fn dismiss(&mut self) {
        self.shell.asked = None;
        self.shell.answer = None;
        self.shell.open = false;
        wipe(&mut self.shell.typed);
        self.shell.stale = true;
    }

    /// Does whatever there is to do, without waiting: takes in what the
    /// compositor said, and shows what should be shown. Returns what the
    /// user did, once they have.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone.
    pub fn turn(&mut self) -> Result<Option<Answer>, Failure> {
        let guard = self.queue.prepare_read();
        let mut socket = [PollFd::new(&self.connection, PollFlags::IN)];
        let there = poll(&mut socket, Some(&Timespec::default())).is_ok_and(|ready| ready > 0);
        let _ = guard.filter(|_| there).map(ReadEventsGuard::read);
        // A failure is kept by the connection and comes back from the
        // flush that ends the turn.
        let _ = self.queue.dispatch_pending(&mut self.shell);
        let _ = self.shell.show(&self.queue.handle());
        let flushed = self.connection.flush().map_err(Failure::from);
        flushed.map(|()| self.shell.answer.take())
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
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
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
        // The only other thing it says is that the compositor closed it, as
        // when its screen goes, which is taken for giving up.
        shell.face.take().into_iter().for_each(Face::close);
        shell.over = None;
        shell.answered(Answer::Cancel);
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
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                // The layout is always said first.
                let typing = shell.typing.as_mut();
                let _ = typing.map(|typing| {
                    typing.update_mask(mods_depressed, mods_latched, mods_locked, 0, 0, group)
                });
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
            } => shell.over = Some((whole(surface_x), whole(surface_y))),
            wl_pointer::Event::Leave { .. } => shell.over = None,
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => shell.clicked(button),
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
        // All a buffer says is that the compositor has finished with it.
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
    use omorfos_config::Color;

    use super::*;

    #[test]
    fn each_button_is_where_it_is_drawn_and_nowhere_else() {
        assert_eq!(button_at((MARGIN, 164)), Some(Button::Show));
        assert_eq!(button_at((168, 170)), Some(Button::Cancel));
        assert_eq!(button_at((419, 195)), Some(Button::Go));
        assert_eq!(button_at((420, 195)), None);
        assert_eq!(button_at((130, 170)), None, "between them");
        assert_eq!(button_at((200, 110)), None, "on the field");
    }

    #[test]
    fn the_field_says_whose_password_it_wants_or_what_pam_asks() {
        assert_eq!(label("Password: ", "rafal"), "Password for rafal");
        assert_eq!(label("password:", "rafal"), "Password for rafal");
        assert_eq!(label("Verification code: ", "rafal"), "Verification code");
    }

    #[test]
    fn what_is_typed_is_shown_as_dots_unless_it_may_be_seen() {
        assert_eq!(shown("zażółć", false), "\u{2022}".repeat(6));
        assert_eq!(shown("zażółć", true), "zażółć");
        assert_eq!(shown("", false), "");
    }

    #[test]
    fn the_button_that_goes_on_is_in_the_colour_of_what_is_picked_out() {
        let theme = Theme::default();
        let asking = Asking {
            message: "Authentication is needed to install software".to_owned(),
            user: "rafal".to_owned(),
            question: "Password: ".to_owned(),
            open: false,
            said: String::new(),
            checking: false,
        };
        let mut text = Text::new();
        let canvas = paint(&theme, &mut text, Scale::ONE, (&asking, "secret", false));
        let argb =
            |color: Color| u32::from_be_bytes([color.alpha, color.red, color.green, color.blue]);
        let picked = theme.style(Part::QuestionButton, States::NONE.with(State::Active));
        let unpicked = theme.style(Part::QuestionButton, States::NONE);
        assert_eq!(
            canvas.at(GO.0.unsigned_abs() + 4, GO.1.unsigned_abs() + 2),
            argb(picked.background)
        );
        assert_eq!(
            canvas.at(CANCEL.0.unsigned_abs() + 4, CANCEL.1.unsigned_abs() + 2),
            argb(unpicked.background)
        );
        assert_eq!(canvas.units(), SIZE);
        // Being checked, the field is not being typed into.
        let checking = Asking {
            checking: true,
            ..asking
        };
        let other = paint(&theme, &mut text, Scale::ONE, (&checking, "secret", true));
        let field = |states| theme.style(Part::QuestionInput, states).border.top.color;
        assert_ne!(
            field(States::NONE),
            field(States::NONE.with(State::Focused))
        );
        assert_ne!(
            canvas.at(FIELD.0.unsigned_abs(), FIELD.1.unsigned_abs() + 16),
            other.at(FIELD.0.unsigned_abs(), FIELD.1.unsigned_abs() + 16)
        );
    }

    #[test]
    fn what_was_typed_is_wiped() {
        let mut typed = "secret".to_owned();
        wipe(&mut typed);
        assert_eq!(typed, "");
    }
}
