//! The prompt: a box in the middle of the screen that asks for a secret,
//! on the overlay layer, with the keyboard. Started with the `ask` role,
//! it is sealed by the compositor: the session is dimmed behind it, and
//! nothing else sees a key while it is up.
//!
//! It says what is asked for, whose secret it wants, and has a field the
//! secret is typed into, shown as dots unless asked to be shown, and
//! buttons to show it, to give up and to go on. Enter goes on and Escape
//! gives up. It is drawn as the `question` parts of the theme.

use std::io;
use std::mem;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::net::UnixStream;

use omorfos_config::{Color, Part, State, States, Store, Theme};
use omorfos_shell::shake::{FRAME, SHAKE, SHAKING, shaken};
use omorfos_shell::typing::{Key, Typing};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Area, Canvas, Scale, Text};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_keyboard, wl_pointer, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::Failure;

/// How large the box is, in units of layout.
pub const SIZE: (u32, u32) = (440, 216);

/// The room between the edge of the box and what is in it.
const MARGIN: i32 = 20;

/// Where the field is that the secret is typed into.
pub const FIELD: Area = (MARGIN, 100, 400, 32);

/// Where the eye is, at the end of the field, that shows what is typed or
/// hides it again.
pub const EYE: Area = (MARGIN + 400 - 30, 104, 24, 24);

/// How large the surface is, in units of layout: the box, and the room it
/// shakes in.
pub const SURFACE: (u32, u32) = (SIZE.0 + 2 * SHAKE.unsigned_abs(), SIZE.1);
/// Where the button is that gives up.
pub const CANCEL: Area = (168, 164, 120, 32);
/// Where the button is that goes on.
pub const GO: Area = (300, 164, 120, 32);

/// The left button of a mouse, as Linux numbers it.
const LEFT_BUTTON: u32 = 0x110;

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
        (EYE, Button::Show),
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

/// What the tooltip over the eye says: what pressing it does.
#[must_use]
pub const fn eye_says(open: bool) -> &'static str {
    if open {
        "Hide the password"
    } else {
        "Show the password"
    }
}

/// Paints an eye in `area`, in `ink`: open, as what is typed is shown; or
/// struck through, as it is hidden. Each pixel is looked at in four places,
/// so that the curves are smooth.
fn paint_eye(canvas: &mut Canvas, (area, scale): (Area, Scale), ink: Color, open: bool) {
    let (left, top, across, down) = area;
    let (left, top) = (scale.pixels(left), scale.pixels(top));
    let (across, down) = (
        scale.pixels(across.cast_signed()),
        scale.pixels(down.cast_signed()),
    );
    let (wide, high) = (f64::from(across), f64::from(down));
    let thick = f64::from(scale.pixels(2).max(1));
    let inside = |x: f64, y: f64| {
        // From the middle, the eye half as wide as the area and its lids
        // meeting at its corners.
        let (u, v) = ((x - wide / 2.0) / (wide * 0.45), y - high / 2.0);
        let lid = wide * 0.45 * 0.55 * (1.0 - u * u);
        let outline = u.abs() <= 1.0 && (v.abs() - lid).abs() <= thick / 2.0;
        let pupil = (x - wide / 2.0).hypot(v) <= wide * 0.16;
        // Hidden, a stroke from one corner to the other.
        let stroke = (x * high - y * wide).abs() / wide.hypot(high) <= thick / 2.0;
        outline || pupil || (!open && stroke)
    };
    for row in 0..down {
        for column in 0..across {
            let (x, y) = (f64::from(column), f64::from(row));
            let quarters = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];
            let hit = quarters
                .iter()
                .filter(|(dx, dy)| inside(x + dx, y + dy))
                .count();
            let coverage = u8::try_from(hit * 255 / 4).unwrap_or(u8::MAX);
            if hit > 0 {
                canvas.blend(left + column, top + row, ink, coverage);
            }
        }
    }
}

/// Moves `area` along by `by` units.
const fn along((left, top, across, down): Area, by: i32) -> Area {
    (left + by, top, across, down)
}

/// Paints the box, `shift` units from where it rests: what is asked for,
/// the field with what is typed shown or hidden and the eye that shows or
/// hides it, what was said, the buttons with the one that goes on picked
/// out; and, while the pointer is on the eye, what the eye is for.
#[must_use]
pub fn paint(
    theme: &Theme,
    text: &mut Text,
    scale: Scale,
    (asking, typed, open): (&Asking, &str, bool),
    (shift, on_eye): (i32, bool),
) -> Canvas {
    let (width, height) = SURFACE;
    let mut canvas = Canvas::scaled(width, height, scale);
    let by = SHAKE + shift;
    let whole_box = theme.style(Part::Question, States::NONE);
    canvas.draw_box((by, 0, SIZE.0, SIZE.1), &whole_box);
    let room = SIZE.0 - MARGIN.unsigned_abs() * 2;
    let font = theme.font(Part::Question, States::NONE);
    let title = "Authentication is required";
    let line = along((MARGIN, MARGIN, room, 24), by);
    text.draw(&mut canvas, title, line, whole_box.color, font);
    let detail = theme.style(Part::QuestionDetail, States::NONE);
    let font = theme.font(Part::QuestionDetail, States::NONE);
    let line = along((MARGIN, MARGIN + 28, room, 20), by);
    text.draw(&mut canvas, &asking.message, line, detail.color, font);
    let called = label(&asking.question, &asking.user);
    text.draw(
        &mut canvas,
        &called,
        along((MARGIN, 76, room, 20), by),
        detail.color,
        font,
    );
    let typing = if asking.checking {
        States::NONE
    } else {
        States::NONE.with(State::Focused)
    };
    let field = theme.style(Part::QuestionInput, typing);
    canvas.draw_box(along(FIELD, by), &field);
    let font = theme.font(Part::QuestionInput, typing);
    let (left, top, across, down) = FIELD;
    // The text stops short of the eye.
    let within = along((left + 8, top + 6, across - 48, down - 12), by);
    let showing = shown(typed, open || asking.open);
    text.draw(&mut canvas, &showing, within, field.color, font);
    // A line where the next character will go, while it may be typed, which
    // stays in the field however much has been typed.
    if !asking.checking {
        let (left, top, room, tall) = within;
        let at = text.measure(&showing, font).min(room.saturating_sub(1));
        let margin = tall / 5;
        let caret = (
            left.saturating_add_unsigned(at),
            top.saturating_add_unsigned(margin),
            1,
            tall.saturating_sub(margin * 2),
        );
        canvas.fill(caret, field.color);
    }
    // A question whose answer may be seen has nothing to hide.
    if !asking.open {
        paint_eye(&mut canvas, (along(EYE, by), scale), field.color, open);
    }
    let line = along((MARGIN, 140, room, 20), by);
    text.draw(&mut canvas, &asking.said, line, detail.color, font);
    let picked = States::NONE.with(State::Active);
    let buttons = [
        (CANCEL, "Cancel", States::NONE),
        (GO, "Authenticate", picked),
    ];
    for (area, label, states) in buttons {
        let style = theme.style(Part::QuestionButton, states);
        canvas.draw_box(along(area, by), &style);
        let font = theme.font(Part::QuestionButton, states);
        let (left, top, across, down) = along(area, by);
        let within = (left + 10, top + 6, across - 20, down - 12);
        text.draw(&mut canvas, label, within, style.color, font);
    }
    if on_eye && !asking.open {
        paint_tooltip(&mut canvas, theme, text, (eye_says(open), by));
    }
    canvas
}

/// Paints what the eye is for, under it, ending where the field does.
fn paint_tooltip(canvas: &mut Canvas, theme: &Theme, text: &mut Text, (says, by): (&str, i32)) {
    let style = theme.style(Part::Tooltip, States::NONE);
    let font = theme.font(Part::Tooltip, States::NONE);
    let across = text.measure(says, font) + 16;
    let (field_left, field_top, field_across, field_down) = FIELD;
    let right = field_left + field_across.cast_signed();
    let area = (
        right - across.cast_signed(),
        field_top + field_down.cast_signed() + 4,
        across,
        24,
    );
    let area = along(area, by);
    canvas.draw_box(area, &style);
    let (left, top, across, down) = area;
    text.draw(
        canvas,
        says,
        (left + 8, top + 4, across - 16, down - 8),
        style.color,
        font,
    );
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
    /// While the box shakes: when the first frame of it was shown, once one
    /// has been, and how long it has shaken for since, in milliseconds, as
    /// the compositor tells the time of each frame.
    shaking: Option<(Option<u32>, u32)>,
    store: Store,
    text: Text,
    typing: Typing,
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

    /// Takes in a key going down: what it types goes in the field, `⌫`
    /// takes the last character out, Enter goes on and Escape gives up.
    fn pressed(&mut self, code: u32) {
        let checking = self
            .asked
            .as_ref()
            .is_none_or(|(asking, _)| asking.checking);
        if checking {
            return;
        }
        match self.typing.pressed(code) {
            Key::Enter => self.go(),
            Key::Escape => self.answered(Answer::Cancel),
            Key::Back => {
                self.typed.pop();
                self.stale = true;
            }
            Key::Typed(typed) => {
                // No more than a secret may be is kept.
                let room = LONGEST.saturating_sub(self.typed.chars().count());
                self.typed.extend(typed.chars().take(room));
                self.stale = true;
            }
            Key::Nothing => {}
        }
    }

    /// The button the pointer is on, if any: where it is on the surface,
    /// less the room the box shakes in.
    fn pointed(&self) -> Option<Button> {
        self.over.and_then(|(x, y)| button_at((x - SHAKE, y)))
    }

    /// Takes the pointer in at `at` on the surface, or gone from it, and
    /// draws the box again if it has come onto the eye or left it.
    fn point(&mut self, at: Option<(i32, i32)>) {
        let was = self.pointed() == Some(Button::Show);
        self.over = at;
        self.stale |= was != (self.pointed() == Some(Button::Show));
    }

    /// Takes in a button going down on the box.
    fn clicked(&mut self, button: u32) {
        let on = self.pointed().filter(|_| button == LEFT_BUTTON);
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
        let on_eye = self.pointed() == Some(Button::Show);
        let face = self.face.get_or_insert_with(|| Face::new(globals, handle));
        if !face.ready {
            return Ok(());
        }
        let theme = &self.store.current().theme;
        // Shaking, it is drawn again as each frame is shown, until it has
        // shaken for as long as it does.
        let elapsed = self.shaking.map(|(_, elapsed)| elapsed);
        self.shaking = self.shaking.filter(|(_, elapsed)| *elapsed < SHAKING);
        // Told when each drawing is shown: shaking, that is when the next
        // is due; otherwise it says nothing to do.
        face.surface.frame(handle, ());
        let shift = elapsed.map_or(0, shaken);
        let canvas = paint(
            theme,
            &mut self.text,
            face.scale,
            (asking, &self.typed, self.open),
            (shift, on_eye),
        );
        picture(&globals.shm, &canvas, handle).map(|buffer| {
            face.surface.attach(Some(&buffer), 0, 0);
            let (width, height) = (SURFACE.0.cast_signed(), SURFACE.1.cast_signed());
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
        layer.set_size(SURFACE.0, SURFACE.1);
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
                shaking: None,
                store,
                text: Text::new(),
                typing: Typing::default(),
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

    /// Shakes the box, as when what was typed was not right; unless the
    /// theme has the question change at once, which is how a theme turns
    /// movement off.
    pub fn shake(&mut self) {
        let moves = self
            .shell
            .store
            .current()
            .theme
            .style(Part::Question, States::NONE);
        if moves.transition.duration > 0 {
            self.shell.shaking = Some((None, 0));
            self.shell.stale = true;
        }
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
            } => shell.point(Some((whole(surface_x), whole(surface_y)))),
            wl_pointer::Event::Leave { .. } => shell.point(None),
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(wl_pointer::ButtonState::Pressed),
                ..
            } => shell.clicked(button),
            _ => {}
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
            // The box was last shown at rest a frame before the first of
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
delegate_noop!(Shell: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Shell: wp_viewporter::WpViewporter);
delegate_noop!(Shell: wp_viewport::WpViewport);
delegate_noop!(Shell: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_box_drawn_moved_is_where_it_was_moved_to() {
        let theme = Theme::default();
        let mut text = Text::new();
        let background = argb(theme.style(Part::Question, States::NONE).background);
        let left = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&asking(), "", false),
            (-SHAKE, false),
        );
        let right = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&asking(), "", false),
            (SHAKE, false),
        );
        assert_eq!(left.at(12, 12), background);
        assert_ne!(right.at(12, 12), background, "not yet there");
    }

    fn asking() -> Asking {
        Asking {
            message: "Authentication is needed to install software".to_owned(),
            user: "rafal".to_owned(),
            question: "Password: ".to_owned(),
            open: false,
            said: String::new(),
            checking: false,
        }
    }

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    /// A place on the box, on the surface it is drawn on at rest.
    fn on((x, y): (i32, i32)) -> (u32, u32) {
        ((x + SHAKE).unsigned_abs(), y.unsigned_abs())
    }

    /// The pixels of `area` on `canvas`, at rest.
    fn pixels(canvas: &Canvas, (left, top, across, down): Area) -> Vec<u32> {
        let (left, top) = on((left, top));
        let rows = top..top + down;
        rows.flat_map(|y| (left..left + across).map(move |x| canvas.at(x, y)))
            .collect()
    }

    #[test]
    fn each_button_is_where_it_is_drawn_and_nowhere_else() {
        assert_eq!(button_at((EYE.0 + 2, EYE.1 + 2)), Some(Button::Show));
        assert_eq!(button_at((168, 170)), Some(Button::Cancel));
        assert_eq!(button_at((419, 195)), Some(Button::Go));
        assert_eq!(button_at((420, 195)), None);
        assert_eq!(button_at((MARGIN, 170)), None, "where Show was");
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
        let mut text = Text::new();
        let canvas = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&asking(), "secret", false),
            (0, false),
        );
        let picked = theme.style(Part::QuestionButton, States::NONE.with(State::Active));
        let unpicked = theme.style(Part::QuestionButton, States::NONE);
        let (x, y) = on((GO.0 + 4, GO.1 + 2));
        assert_eq!(canvas.at(x, y), argb(picked.background));
        let (x, y) = on((CANCEL.0 + 4, CANCEL.1 + 2));
        assert_eq!(canvas.at(x, y), argb(unpicked.background));
        assert_eq!(canvas.units(), SURFACE);
        // Being checked, the field is not being typed into.
        let checking = Asking {
            checking: true,
            ..asking()
        };
        let other = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&checking, "secret", true),
            (0, false),
        );
        let field = |states| theme.style(Part::QuestionInput, states).border.top.color;
        assert_ne!(
            field(States::NONE),
            field(States::NONE.with(State::Focused))
        );
        let (x, y) = on((FIELD.0, FIELD.1 + 16));
        assert_ne!(canvas.at(x, y), other.at(x, y));
    }

    #[test]
    fn a_line_shows_where_the_next_character_goes_while_one_may_be_typed() {
        let theme = Theme::default();
        let mut text = Text::new();
        let field = |states| theme.style(Part::QuestionInput, states);
        let ink = argb(field(States::NONE.with(State::Focused)).color);
        // The first column of the field's text, half way down.
        let (left, top, _, down) = FIELD;
        let start = (left + 8, top + 6, 1, down - 12);
        let middle = (left + 8, top + down.cast_signed() / 2, 1, 1);
        let mut drawn = |asked: &Asking, typed: &str| {
            paint(
                &theme,
                &mut text,
                Scale::ONE,
                (asked, typed, false),
                (0, false),
            )
        };
        let empty = drawn(&asking(), "");
        assert_eq!(pixels(&empty, middle), [ink], "at the start, nothing typed");
        assert_ne!(pixels(&empty, start)[0], ink, "short of the field's top");
        // Once something is typed, it is after it.
        let typed = drawn(&asking(), "secret");
        let after = (left + 8 + 40, top + 6, 120, down - 12);
        let between = |canvas: &Canvas| pixels(canvas, after).iter().filter(|&&p| p == ink).count();
        assert!(between(&typed) > 0);
        // However much is typed, it stays in the field, short of the eye.
        let long = drawn(&asking(), &"x".repeat(200));
        let last = (left + 8 + 352 - 1, top + down.cast_signed() / 2, 1, 1);
        assert_eq!(pixels(&long, last), [ink]);
        // While what was typed is being checked, there is none.
        let checking = Asking {
            checking: true,
            ..asking()
        };
        let checked = drawn(&checking, "");
        assert_eq!(
            pixels(&checked, middle),
            [argb(field(States::NONE).background)]
        );
    }

    #[test]
    fn the_eye_is_open_or_struck_through_and_not_there_when_nothing_is_hidden() {
        let theme = Theme::default();
        let mut text = Text::new();
        let ink = argb(
            theme
                .style(Part::QuestionInput, States::NONE.with(State::Focused))
                .color,
        );
        let at = |open: bool, asked: &Asking| {
            let canvas = paint(
                &theme,
                &mut Text::new(),
                Scale::ONE,
                (asked, "", open),
                (0, false),
            );
            pixels(&canvas, EYE)
        };
        let hidden = at(false, &asking());
        let showing = at(true, &asking());
        assert!(
            hidden.contains(&ink) && showing.contains(&ink),
            "an eye is drawn"
        );
        assert_ne!(hidden, showing, "struck through, or not");
        // Its corner is where the stroke is, and only that.
        let corner = |eye: &[u32]| eye[EYE.2 as usize + 1];
        assert_ne!(corner(&hidden), corner(&showing));
        let open = Asking {
            open: true,
            ..asking()
        };
        let none = at(false, &open);
        assert!(!none.contains(&ink), "nothing to hide, no eye");
        // At a scale of two it is as sharp.
        let large = paint(
            &theme,
            &mut text,
            Scale::new(2.0),
            (&asking(), "", false),
            (0, false),
        );
        assert_eq!(large.units(), SURFACE);
    }

    #[test]
    fn the_eye_says_what_it_does_while_the_pointer_is_on_it() {
        assert_eq!(eye_says(false), "Show the password");
        assert_eq!(eye_says(true), "Hide the password");
        let theme = Theme::default();
        let mut text = Text::new();
        // Under the end of the field, where what was said is not.
        let below = (
            FIELD.0 + FIELD.2.cast_signed() - 60,
            FIELD.1 + FIELD.3.cast_signed() + 4,
            60,
            24,
        );
        let away = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&asking(), "", false),
            (0, false),
        );
        let over = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&asking(), "", false),
            (0, true),
        );
        assert_ne!(pixels(&away, below), pixels(&over, below));
        // Nothing to hide, nothing to say.
        let open = Asking {
            open: true,
            ..asking()
        };
        let still = paint(
            &theme,
            &mut text,
            Scale::ONE,
            (&open, "", false),
            (0, false),
        );
        let quiet = paint(&theme, &mut text, Scale::ONE, (&open, "", false), (0, true));
        assert_eq!(pixels(&still, below), pixels(&quiet, below));
    }

    #[test]
    fn what_was_typed_is_wiped() {
        let mut typed = "secret".to_owned();
        wipe(&mut typed);
        assert_eq!(typed, "");
    }
}
