//! Asking the user, over everything on the screen, whether to give an
//! application what it asked for, or which colour on the screen to give it.
//!
//! The question is a box in the middle of the screen, above everything
//! and with the keyboard, saying which application asks and what for,
//! with a button to give it and one not to. Enter gives it and Escape
//! does not, as do the buttons. It is drawn as the `question` parts of the
//! theme.
//!
//! A colour is picked from a picture of the screen, shown still over all
//! of it, under a crosshair and beside a round loupe that shows the pixels
//! round the pointer larger: a click of the left button, or Enter, picks
//! the colour under the pointer, and Escape or another button picks none.
//! Picking it is the user's say, so nothing else is asked.

use std::fs::File;
use std::io::{self, Write};
use std::mem;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;

use omorfos_config::{Color, Part, State, States, Store, Theme};
use omorfos_shell::{picture, whole};
use omorfos_ui::{Area, Canvas, Scale, Text};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
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
    /// Do not, or pick no colour.
    Keep,
    /// The colour picked, red, green and blue.
    Picked([u8; 3]),
}

/// A picture of a screen to pick a colour from: what the screen is called,
/// how many pixels across and down it is, and its pixels from the top
/// left, three bytes to each, red, green and blue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Still {
    /// What the screen is called, which the picture is shown on.
    pub screen: String,
    /// Its size in pixels.
    pub size: (u32, u32),
    /// Its pixels.
    pub pixels: Vec<u8>,
}

impl Still {
    /// The colour at a place on the screen, when the picture is shown
    /// `shown` units large: as many pixels across and down as there are
    /// units in that, whatever the screen's scale.
    fn at(&self, at: (i32, i32), shown: (u32, u32)) -> Option<[u8; 3]> {
        self.place(at, shown).and_then(|place| self.pixel(place))
    }

    /// The pixel at a place on the screen, when the picture is shown
    /// `shown` units large, by its column and row.
    fn place(&self, (x, y): (i32, i32), shown: (u32, u32)) -> Option<(i64, i64)> {
        let along = |at: i32, shown: u32, pixels: u32| {
            let at = i64::from(at.max(-1));
            let pixel = at * i64::from(pixels) / i64::from(shown.max(1));
            (0..i64::from(pixels)).contains(&pixel).then_some(pixel)
        };
        let (width, height) = self.size;
        along(x, shown.0, width).zip(along(y, shown.1, height))
    }

    /// The colour of the pixel in `column` and `row`, if there is one.
    fn pixel(&self, (column, row): (i64, i64)) -> Option<[u8; 3]> {
        let (width, height) = self.size;
        let inside =
            (0..i64::from(width)).contains(&column) && (0..i64::from(height)).contains(&row);
        // An index too large to be one has no pixel there.
        let index = (row * i64::from(width) + column) * 3;
        let index = usize::try_from(index)
            .ok()
            .filter(|_| inside)
            .unwrap_or(usize::MAX);
        let pixel = self.pixels.get(index..index.saturating_add(3))?;
        Some([pixel[0], pixel[1], pixel[2]])
    }

    /// The picture, to be shown pixel for pixel.
    fn canvas(&self) -> Canvas {
        let (width, height) = self.size;
        let mut canvas = Canvas::new(width, height);
        let pixels = self.pixels.as_chunks::<3>().0.iter().zip(0..);
        for (&[red, green, blue], at) in pixels {
            let (x, y) = (at % width.max(1), at / width.max(1));
            let colour = Color::rgb(red, green, blue);
            canvas.blend(x.cast_signed(), y.cast_signed(), colour, u8::MAX);
        }
        canvas
    }
}

/// How large the loupe beside the pointer is across, in units of layout;
/// how many pixels across it shows; how wide its ring is; and how far it
/// is from the pointer.
const LOUPE: u32 = 120;
const SEEN: u32 = 15;
const RING: u32 = 2;
const LOUPE_AWAY: i32 = 24;

/// Paints the loupe for the pixel in `place` of `still`: the pixels round
/// it, each a square, in a circle with a ring round it in `ring`, and the
/// one in the middle outlined in black or white, whichever it is not.
fn paint_loupe(still: &Still, (column, row): (i64, i64), ring: Color, scale: Scale) -> Canvas {
    let mut canvas = Canvas::scaled(LOUPE, LOUPE, scale);
    let size = canvas.width().max(1);
    let thick = RING * size / LOUPE;
    let cell = |at: u32| i64::from(at * SEEN / size) - i64::from(SEEN / 2);
    let middle = still.pixel((column, row)).unwrap_or_default();
    let light = middle
        .iter()
        .map(|&channel| u32::from(channel))
        .sum::<u32>()
        > 3 * 128;
    let outline = if light {
        Color::rgb(0, 0, 0)
    } else {
        Color::rgb(255, 255, 255)
    };
    // Twice as far from the middle as it is, to keep to whole numbers.
    let far = |at: u32| i64::from(2 * at + 1) - i64::from(size);
    let (outer, inner) = (i64::from(size).pow(2), i64::from(size - 2 * thick).pow(2));
    for y in 0..size {
        for x in 0..size {
            let distance = far(x).pow(2) + far(y).pow(2);
            let (across, down) = (cell(x), cell(y));
            let edge = |at: u32| cell(at) != cell(at.saturating_sub(1)) || cell(at) != cell(at + 1);
            let centre = across == 0 && down == 0 && (edge(x) || edge(y));
            let shown = still
                .pixel((column + across, row + down))
                .unwrap_or_default();
            let colour = match (distance <= inner, centre) {
                (true, true) => outline,
                (true, false) => Color::rgb(shown[0], shown[1], shown[2]),
                (false, _) => ring,
            };
            if distance <= outer {
                canvas.blend(x.cast_signed(), y.cast_signed(), colour, u8::MAX);
            }
        }
    }
    canvas
}

/// Where the loupe goes for a pointer at `at` on a picture `shown` large:
/// below and to the right of it, or above or to the left where there is no
/// room there.
fn loupe_at((x, y): (i32, i32), (width, height): (u32, u32)) -> (i32, i32) {
    let across = LOUPE.cast_signed();
    let side = |at: i32, room: u32| {
        let after = at + LOUPE_AWAY;
        if after + across <= room.cast_signed() {
            after
        } else {
            at - LOUPE_AWAY - across
        }
    };
    (side(x, width), side(y, height))
}

/// The loupe's surface, over the picture to pick from.
struct Loupe {
    surface: wl_surface::WlSurface,
    over: wl_subsurface::WlSubsurface,
    viewport: wp_viewport::WpViewport,
}

impl Loupe {
    fn close(self) {
        self.viewport.destroy();
        self.over.destroy();
        self.surface.destroy();
    }
}

/// What is being asked of the user.
#[derive(Debug, Clone)]
enum Asking {
    /// Whether to give `app` what it asked for.
    Question(String, Question),
    /// Which colour of the screen in the picture to give.
    Pick(Still),
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
    let whole_box = theme.style(Part::Question, States::NONE);
    canvas.draw_box((0, 0, width, height), &whole_box);
    let room = width - MARGIN.unsigned_abs() * 2;
    let font = theme.font(Part::Question, States::NONE);
    let line = (MARGIN, MARGIN, room, 24);
    text.draw(
        &mut canvas,
        &asking(app, question),
        line,
        whole_box.color,
        font,
    );
    let said = theme.style(Part::QuestionDetail, States::NONE);
    let font = theme.font(Part::QuestionDetail, States::NONE);
    let line = (MARGIN, MARGIN + 28, room, 20);
    let (_, warning, give) = question.words();
    text.draw(&mut canvas, warning, line, said.color, font);
    let picked = States::NONE.with(State::Active);
    let buttons = [
        (GIVE, give, Part::QuestionButton, picked),
        (KEEP, "Cancel", Part::QuestionButton, States::NONE),
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
    /// How large the compositor has made it, once it has said.
    ready: Option<(u32, u32)>,
    scale: Scale,
    buffer: Option<wl_buffer::WlBuffer>,
    /// The loupe over a picture to pick from, once it has been shown.
    loupe: Option<Loupe>,
}

impl Face {
    /// Takes the box off the screen.
    fn close(self) {
        self.loupe.into_iter().for_each(Loupe::close);
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
    subsurfaces: wl_subcompositor::WlSubcompositor,
    shapes: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
}

/// What the compositor has said, and the question being asked.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    subsurfaces: Option<wl_subcompositor::WlSubcompositor>,
    shapes: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    /// What the pointer's shape is set by, once it has been.
    shape: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    face: Option<Face>,
    /// The screens, by what they are called.
    outputs: Vec<(wl_output::WlOutput, String)>,
    /// What is being asked while it is up, and what it is put on the
    /// screen with.
    asked: Option<(Asking, Globals)>,
    answer: Option<Answer>,
    over: (i32, i32),
    store: Store,
    text: Text,
    layouts: xkb::Context,
    typing: Option<xkb::State>,
    /// Whether what is on the screen may not be what should be.
    stale: bool,
    /// Whether the pointer has moved since the loupe was last shown.
    moved: bool,
}

impl Shell {
    /// What the box is put on the screen with, if the compositor offers
    /// all of it.
    fn globals(&self) -> Option<Globals> {
        let globals = self.compositor.clone().zip(self.shm.clone());
        let globals = globals.zip(self.layers.clone());
        let globals = globals.zip(self.viewporter.clone().zip(self.scales.clone()));
        let globals = globals.zip(self.subsurfaces.clone().zip(self.shapes.clone()));
        globals.map(
            |((((compositor, shm), layers), (viewporter, scales)), (subsurfaces, shapes))| {
                Globals {
                    compositor,
                    shm,
                    layers,
                    viewporter,
                    scales,
                    subsurfaces,
                    shapes,
                }
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

    /// Takes in a key going down: Enter gives, or picks the colour under
    /// the pointer, and Escape does neither.
    fn pressed(&mut self, code: u32) {
        // The numbers the layout knows keys by are eight higher.
        let code = xkb::Keycode::new(code.saturating_add(8));
        let typing = self.typing.as_ref();
        let answer = typing.and_then(|typing| match typing.key_get_one_sym(code) {
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => self.giving(),
            xkb::Keysym::Escape => Some(Answer::Keep),
            _ => None,
        });
        self.answered(answer);
    }

    /// What giving is, as Enter or the left button gives: the question's
    /// yes; or the colour under the pointer, once where it is on the
    /// picture is known, and nothing until then.
    fn giving(&self) -> Option<Answer> {
        let shown = self.face.as_ref().and_then(|face| face.ready);
        match self.asked.as_ref().map(|(asking, _)| asking) {
            Some(Asking::Pick(still)) => {
                let colour = shown.and_then(|shown| still.at(self.over, shown));
                colour.map(Answer::Picked)
            }
            _ => Some(Answer::Give),
        }
    }

    /// Takes in a button going down: on the box, the button under the
    /// pointer; on a picture, the left picks the colour and any other none.
    fn clicked(&mut self, button: u32) {
        let picking = matches!(self.asked, Some((Asking::Pick(_), _)));
        let answer = match (picking, button) {
            (true, LEFT_BUTTON) => self.giving(),
            (true, _) => Some(Answer::Keep),
            (false, LEFT_BUTTON) => button_at(self.over),
            (false, _) => None,
        };
        self.answered(answer);
    }

    /// Puts the box up while there is a question, takes it down when
    /// there is not, and draws it if it no longer shows what it should.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        let shown = if mem::take(&mut self.stale) {
            self.show_face(handle)
        } else {
            Ok(())
        };
        shown.and_then(|()| {
            if mem::take(&mut self.moved) {
                self.show_loupe(handle)
            } else {
                Ok(())
            }
        })
    }

    /// Shows the loupe where the pointer is over a picture to pick from,
    /// and nothing while where it is is not known.
    fn show_loupe(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        let (Some((Asking::Pick(still), globals)), Some(face)) = (&self.asked, self.face.as_mut())
        else {
            return Ok(());
        };
        let shown = face.ready.unwrap_or(SIZE);
        let parent = &face.surface;
        let loupe = face.loupe.get_or_insert_with(|| {
            let surface = globals.compositor.create_surface(handle, ());
            let over = globals
                .subsurfaces
                .get_subsurface(&surface, parent, handle, ());
            over.set_desync();
            let viewport = globals.viewporter.get_viewport(&surface, handle, ());
            Loupe {
                surface,
                over,
                viewport,
            }
        });
        let ring = self
            .store
            .current()
            .theme
            .style(Part::Question, States::NONE);
        let ring = ring.border.top.color;
        let place = still.place(self.over, shown);
        let canvas = place.map(|place| paint_loupe(still, place, ring, face.scale));
        let buffer = canvas
            .map(|canvas| picture(&globals.shm, &canvas, handle))
            .transpose();
        let at = loupe_at(self.over, shown);
        buffer.map(|buffer| {
            loupe.over.set_position(at.0, at.1);
            loupe.surface.attach(buffer.as_ref(), 0, 0);
            let size = LOUPE.cast_signed();
            loupe.viewport.set_destination(size, size);
            loupe.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            loupe.surface.commit();
            // Where it is is taken with what it is over.
            parent.commit();
        })
    }

    /// Puts the box or the picture up while there is something to ask,
    /// takes it down when there is not, and draws it if it no longer shows
    /// what it should.
    fn show_face(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        let Some((asking, globals)) = &self.asked else {
            self.face.take().into_iter().for_each(Face::close);
            // Where the pointer was on it is nothing to what comes next.
            self.over = (-1, -1);
            return Ok(());
        };
        let Globals {
            compositor,
            shm,
            layers,
            viewporter,
            scales,
            ..
        } = globals;
        let outputs = &self.outputs;
        let face = self.face.get_or_insert_with(|| {
            let surface = compositor.create_surface(handle, ());
            // In the middle of the screen, above everything, with the
            // keyboard; or a picture of a screen, all over it. Its name is
            // no part of the theme's, so that it goes at once and is not
            // in the picture it leads to.
            let on = match asking {
                Asking::Pick(still) => outputs.iter().find(|(_, name)| *name == still.screen),
                Asking::Question(..) => None,
            };
            let layer = layers.get_layer_surface(
                &surface,
                on.map(|(output, _)| output),
                zwlr_layer_shell_v1::Layer::Overlay,
                "omorfos-prompt".to_owned(),
                handle,
                (),
            );
            if let Asking::Pick(_) = asking {
                let all = zwlr_layer_surface_v1::Anchor::all();
                layer.set_anchor(all);
                layer.set_exclusive_zone(-1);
            } else {
                layer.set_size(SIZE.0, SIZE.1);
            }
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
                ready: None,
                scale: Scale::ONE,
                buffer: None,
                loupe: None,
            }
        });
        let Some(shown) = face.ready else {
            return Ok(());
        };
        let theme = &self.store.current().theme;
        let canvas = match asking {
            Asking::Question(app, question) => {
                paint(theme, &mut self.text, face.scale, (app, *question))
            }
            Asking::Pick(still) => still.canvas(),
        };
        picture(shm, &canvas, handle).map(|buffer| {
            face.surface.attach(Some(&buffer), 0, 0);
            // However many pixels the picture has, it covers this much.
            let (width, height) = (shown.0.cast_signed(), shown.1.cast_signed());
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
                subsurfaces: None,
                shapes: None,
                shape: None,
                face: None,
                outputs: Vec::new(),
                asked: None,
                answer: None,
                over: (-1, -1),
                store,
                text: Text::new(),
                layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
                typing: None,
                stale: false,
                moved: false,
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
        self.put(Asking::Question(app.to_owned(), question))
    }

    /// Asks the user to pick a colour from `still`, a picture of one of the
    /// screens, shown over it. Returns whether it can be asked.
    pub fn pick(&mut self, still: Still) -> bool {
        self.put(Asking::Pick(still))
    }

    /// Puts up what is asked, if it can be: the compositor offers what it
    /// is put on the screen with.
    fn put(&mut self, asking: Asking) -> bool {
        // Each is put up anew, whatever was up before and is not yet taken
        // down: a box and a picture are not the same shape, and where the
        // pointer was on one is nothing to the next.
        self.shell.face.take().into_iter().for_each(Face::close);
        self.shell.over = (-1, -1);
        let globals = self.shell.globals();
        let asked = globals.is_some();
        self.shell.asked = globals.map(|globals| (asking, globals));
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
                "wl_subcompositor" => shell.subsurfaces = Some(registry.bind(name, 1, handle, ())),
                "wp_cursor_shape_manager_v1" => {
                    shell.shapes = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_seat" => {
                    let _: wl_seat::WlSeat = registry.bind(name, version.min(5), handle, ());
                }
                // Each screen says what it is called from the fourth.
                "wl_output" if version >= 4 => {
                    let output = registry.bind(name, 4, handle, ());
                    shell.outputs.push((output, String::new()));
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
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            layer.ack_configure(serial);
            note("the compositor has made room for the question");
            // A box is as large as it asked to be; a picture, as the
            // screen it is on.
            let size = (width, height);
            shell
                .face
                .iter_mut()
                .for_each(|face| face.ready = Some(size));
            return;
        }
        // The only other thing it says is that the compositor closed it,
        // as when its screen goes, which is taken for a no.
        note("the compositor took the question down");
        shell.face.take().into_iter().for_each(Face::close);
        shell.over = (-1, -1);
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
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                shell.over = (whole(surface_x), whole(surface_y));
                shell.moved = true;
                // Over a picture to pick from, the pointer is a crosshair.
                if let Some((Asking::Pick(_), globals)) = &shell.asked {
                    let shapes = &globals.shapes;
                    let shape = shell
                        .shape
                        .get_or_insert_with(|| shapes.get_pointer(pointer, handle, ()));
                    shape.set_shape(serial, wp_cursor_shape_device_v1::Shape::Crosshair);
                }
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => {
                shell.over = (whole(surface_x), whole(surface_y));
                shell.moved = true;
            }
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
        // All a buffer ever says is that the compositor has finished with
        // it, which it does once a newer picture has taken its place.
        buffer.destroy();
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Shell {
    fn event(
        shell: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            let known = shell
                .outputs
                .iter_mut()
                .filter(|(known, _)| known == output);
            known.for_each(|(_, called)| called.clone_from(&name));
        }
    }
}

delegate_noop!(Shell: wl_compositor::WlCompositor);
delegate_noop!(Shell: wl_subcompositor::WlSubcompositor);
delegate_noop!(Shell: wl_subsurface::WlSubsurface);
delegate_noop!(Shell: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
delegate_noop!(Shell: wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);
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

    /// A picture two pixels across and one down: red, then blue.
    fn still() -> Still {
        Still {
            screen: "DP-1".to_owned(),
            size: (2, 1),
            pixels: vec![255, 0, 0, 0, 0, 255],
        }
    }

    #[test]
    fn a_colour_is_picked_from_where_the_pointer_is_on_the_picture() {
        let still = still();
        assert_eq!(still.at((0, 0), (2, 1)), Some([255, 0, 0]));
        assert_eq!(still.at((1, 0), (2, 1)), Some([0, 0, 255]));
        // Shown larger than it has pixels, as at a scale of two.
        assert_eq!(still.at((3, 1), (4, 2)), Some([0, 0, 255]));
        // Nowhere on it, nothing.
        for at in [(-1, 0), (0, -1), (2, 0), (0, 1)] {
            assert_eq!(still.at(at, (2, 1)), None, "at {at:?}");
        }
        // Nor where it has no pixels for.
        let short = Still {
            pixels: vec![255, 0, 0],
            ..still
        };
        assert_eq!(short.at((1, 0), (2, 1)), None);
    }

    #[test]
    fn the_loupe_is_below_and_right_of_the_pointer_unless_there_is_no_room() {
        let size = LOUPE.cast_signed();
        assert_eq!(loupe_at((100, 50), (640, 480)), (124, 74));
        let near = (640 - 30, 480 - 30);
        assert_eq!(
            loupe_at(near, (640, 480)),
            (near.0 - LOUPE_AWAY - size, near.1 - LOUPE_AWAY - size)
        );
    }

    /// A picture all of one colour, as large as the loupe shows.
    fn plain(colour: [u8; 3]) -> Still {
        let count = usize::try_from(SEEN * SEEN).unwrap();
        Still {
            screen: "DP-1".to_owned(),
            size: (SEEN, SEEN),
            pixels: colour.repeat(count),
        }
    }

    #[test]
    fn the_loupe_shows_the_pixels_round_the_pointer_larger_in_a_ring() {
        let ring = Color::rgb(1, 2, 3);
        let argb = |[red, green, blue]: [u8; 3]| u32::from_be_bytes([255, red, green, blue]);
        let middle = i64::from(SEEN / 2);
        let dark = paint_loupe(&plain([10, 20, 30]), (middle, middle), ring, Scale::ONE);
        let centre = LOUPE / 2;
        assert_eq!(dark.at(centre, centre), argb([10, 20, 30]), "the picture");
        assert_eq!(dark.at(centre, 1), argb([1, 2, 3]), "the ring");
        assert_eq!(dark.at(0, 0), 0, "and nothing outside it");
        // The pixel picked is outlined in what stands out from it.
        let edge = LOUPE * (SEEN / 2) / SEEN;
        assert_eq!(dark.at(edge, centre), argb([255, 255, 255]));
        let light = paint_loupe(&plain([250, 240, 230]), (middle, middle), ring, Scale::ONE);
        assert_eq!(light.at(edge, centre), argb([0, 0, 0]));
        // Past the edge of the picture there is nothing to show.
        let corner = paint_loupe(&plain([250, 240, 230]), (0, 0), ring, Scale::ONE);
        assert_eq!(corner.at(centre / 2, centre / 2), argb([0, 0, 0]));
    }

    #[test]
    fn a_picture_to_pick_from_is_shown_pixel_for_pixel() {
        let canvas = still().canvas();
        assert_eq!((canvas.width(), canvas.height()), (2, 1));
        assert_eq!(
            (canvas.at(0, 0), canvas.at(1, 0)),
            (0xffff_0000, 0xff00_00ff)
        );
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
        let picked = theme.style(Part::QuestionButton, States::NONE.with(State::Active));
        let unpicked = theme.style(Part::QuestionButton, States::NONE);
        let argb =
            |color: Color| u32::from_be_bytes([color.alpha, color.red, color.green, color.blue]);
        // Near the top left corner of each, clear of the label.
        assert_eq!(canvas.at(304, 100), argb(picked.background));
        assert_eq!(canvas.at(172, 100), argb(unpicked.background));
        assert_eq!(canvas.units(), SIZE);
    }
}
