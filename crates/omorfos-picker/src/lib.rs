//! Lets the user pick what a picture is to be of on the omorfos desktop:
//! every screen, one screen, a window, or a region of one screen.
//!
//! Each screen is shown still, as the picture of it it was given, over all
//! of it and with the keyboard, and dimmed. A drag draws a region, which
//! is shown bright with a border round it, and letting go picks it. The
//! window under the pointer is shown so until a drag begins, and a click
//! picks it; a click where there is none picks that screen. Enter picks
//! every screen, and Escape or another button nothing. The pointer is a
//! crosshair meanwhile.
//!
//! The dim and the border are each a surface of their own, a pixel of
//! colour made as large as it is to be, so that a drag moves them and
//! paints nothing again; the pictures are painted once.
//!
//! A program that may put this over the screen is one the compositor
//! started with `may: [panel, overlay]`.

mod area;

use std::error::Error;
use std::fs::File;
use std::mem;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;

pub use area::{Area, Picked};
use omorfos_config::{Color, Part, States, Theme};
use omorfos_shell::{picture, whole};
use omorfos_ui::Canvas;
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_region, wl_registry, wl_seat,
    wl_shm, wl_shm_pool, wl_subcompositor, wl_subsurface, wl_surface,
};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1, wp_cursor_shape_manager_v1,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use xkbcommon::xkb;

use crate::area::{Rect, bordered, dimmed};

/// Why nothing could be picked, in words for the user.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The left and the right button of a mouse, as Linux numbers them.
const LEFT: u32 = 0x110;

/// The largest keyboard layout taken in, in bytes.
const LARGEST_KEYMAP: u32 = 1024 * 1024;

/// A picture of a screen, to pick from: what the screen is called, how
/// many pixels across and down it is, its pixels from the top left, three
/// bytes to each, red, green and blue, and where the windows on it are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Still {
    /// What the screen is called, which the picture is shown on.
    pub screen: String,
    /// Its size in pixels.
    pub size: (u32, u32),
    /// Its pixels.
    pub pixels: Vec<u8>,
    /// Where each window on it is in the picture, the one on top first.
    pub windows: Vec<Area>,
}

impl Still {
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

/// Lets the user pick, over `stills`, pictures of the screens of the
/// compositor `connection` is to, what a picture is to be of, in the look
/// `theme` gives the `picker` part. Returns what was picked, or nothing
/// if the user picked nothing; it waits until they have.
///
/// # Errors
///
/// Returns why nothing could be asked: the compositor does not let this
/// program put anything over the screen, none of the screens is there any
/// more, or the session ended.
pub fn pick(
    connection: &Connection,
    stills: Vec<Still>,
    theme: &Theme,
) -> Result<Option<Picked>, Failure> {
    let mut queue = connection.new_event_queue();
    let handle = queue.handle();
    connection.display().get_registry(&handle, ());
    let mut state = State::new(stills, theme);
    // Twice: once to be told what there is, and again for what the screens
    // are called, which they say once they are bound.
    let known = queue.roundtrip(&mut state);
    let known = known.and_then(|_| queue.roundtrip(&mut state));
    let up = known.map_err(Failure::from).map(|_| state.put_up(&handle));
    let picked = up.and_then(|()| {
        while state.outcome.is_none() {
            let turned = queue.blocking_dispatch(&mut state);
            let shown = turned.map_err(Failure::from).map(|_| state.show(&handle));
            shown.map_err(|_| "the session ended before anything was picked")?;
        }
        state.outcome.take().unwrap_or(Outcome::Nothing).into()
    });
    state.close();
    let _ = connection.flush();
    picked
}

/// How picking ended.
#[derive(Debug, Clone, Copy)]
enum Outcome {
    /// With this picked.
    Picked(Picked),
    /// With nothing picked.
    Nothing,
    /// Before anything could be: the compositor would not let it show.
    Refused,
}

impl From<Outcome> for Result<Option<Picked>, Failure> {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Picked(picked) => Ok(Some(picked)),
            Outcome::Nothing => Ok(None),
            Outcome::Refused => Err("omorfos did not let this program put anything over the \
                                     screen. It lets a program it starts with `may: [panel, \
                                     overlay]`"
                .into()),
        }
    }
}

/// What the dim and the border look like.
#[derive(Debug, Clone, Copy)]
struct Look {
    dim: Color,
    border: Color,
    thick: u32,
}

/// A pixel of colour made as large as it is to be: a band of the dim, or a
/// side of the border.
struct Band {
    surface: wl_surface::WlSurface,
    over: wl_subsurface::WlSubsurface,
    viewport: wp_viewport::WpViewport,
    colour: wl_buffer::WlBuffer,
}

/// One screen's picture, with its dim and its border.
struct Screen {
    /// Which of the pictures it shows.
    still: usize,
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    /// How large the compositor has made it, once it has said.
    shown: Option<(u32, u32)>,
    /// The picture, once it is painted.
    painted: Option<wl_buffer::WlBuffer>,
    /// The four bands of the dim, then the four sides of the border.
    bands: Vec<Band>,
}

/// A drag: on which screen, where it began, and where it has got to.
type Drag = (usize, (i32, i32), (i32, i32));

/// What it is put on the screen with.
struct Globals {
    compositor: wl_compositor::WlCompositor,
    subsurfaces: wl_subcompositor::WlSubcompositor,
    shm: wl_shm::WlShm,
    layers: zwlr_layer_shell_v1::ZwlrLayerShellV1,
    viewporter: wp_viewporter::WpViewporter,
}

/// What the compositor has said, and how far picking has got.
struct State {
    compositor: Option<wl_compositor::WlCompositor>,
    subsurfaces: Option<wl_subcompositor::WlSubcompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    shapes: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    /// The screens, by what they are called.
    outputs: Vec<(wl_output::WlOutput, String)>,
    stills: Vec<Still>,
    screens: Vec<Screen>,
    look: Look,
    /// The screen the pointer is over, and where on it.
    over: Option<(usize, (i32, i32))>,
    /// Where a drag began, on which screen, and where it has got to.
    drag: Option<Drag>,
    shape: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    layouts: xkb::Context,
    typing: Option<xkb::State>,
    /// Whether the dim and the border may not be where they should be.
    moved: bool,
    outcome: Option<Outcome>,
}

impl State {
    fn new(stills: Vec<Still>, theme: &Theme) -> Self {
        let style = theme.style(Part::Picker, States::NONE);
        let look = Look {
            dim: style.background,
            border: style.border.top.color,
            thick: style.border.top.width,
        };
        Self {
            compositor: None,
            subsurfaces: None,
            shm: None,
            layers: None,
            viewporter: None,
            shapes: None,
            outputs: Vec::new(),
            stills,
            screens: Vec::new(),
            look,
            over: None,
            drag: None,
            shape: None,
            layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            typing: None,
            moved: false,
            outcome: None,
        }
    }

    /// What it is put on the screen with, if the compositor offers all of
    /// it. The crosshair is not among it: without, the pointer is as it is.
    fn globals(&self) -> Option<Globals> {
        let globals = self.compositor.clone().zip(self.subsurfaces.clone());
        let globals = globals.zip(self.shm.clone().zip(self.layers.clone()));
        let globals = globals.zip(self.viewporter.clone());
        globals.map(
            |(((compositor, subsurfaces), (shm, layers)), viewporter)| Globals {
                compositor,
                subsurfaces,
                shm,
                layers,
                viewporter,
            },
        )
    }

    /// Puts a picture over each screen there is one of.
    fn put_up(&mut self, handle: &QueueHandle<Self>) {
        // Without what it is put on the screen with, it is not let show.
        let globals = self.globals();
        self.ended(globals.is_none().then_some(Outcome::Refused));
        let screens = globals.iter().flat_map(|globals| {
            let empty = globals.compositor.create_region(handle, ());
            let look = self.look;
            let shown = self
                .stills
                .iter()
                .enumerate()
                .filter_map(|(still, picture)| {
                    let on = self
                        .outputs
                        .iter()
                        .find(|(_, name)| *name == picture.screen);
                    on.map(|(output, _)| (still, output.clone()))
                });
            let screens: Vec<Screen> = shown
                .map(|(still, output)| {
                    Screen::new(globals, (still, &output), (look, &empty), handle)
                })
                .collect();
            empty.destroy();
            screens
        });
        self.screens = screens.collect();
        // None of the screens it was given pictures of may be there now,
        // and then nothing can be picked, as when a screen goes meanwhile.
        self.ended(self.screens.is_empty().then_some(Outcome::Nothing));
    }

    /// Paints each picture once the compositor has said how large it is,
    /// and puts the dim and the border where they are to be.
    fn show(&mut self, handle: &QueueHandle<Self>) {
        let shm = self.shm.clone();
        for screen in &mut self.screens {
            let fresh = screen
                .shown
                .zip(shm.as_ref())
                .filter(|_| screen.painted.is_none());
            let painted = fresh.and_then(|(shown, shm)| {
                let canvas = self.stills[screen.still].canvas();
                let buffer = picture(shm, &canvas, handle).ok();
                buffer.inspect(|buffer| {
                    screen.surface.attach(Some(buffer), 0, 0);
                    let (width, height) = (shown.0.cast_signed(), shown.1.cast_signed());
                    screen.viewport.set_destination(width, height);
                    screen.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
                })
            });
            self.moved |= painted.is_some();
            screen.painted = screen.painted.take().or(painted);
        }
        if !mem::take(&mut self.moved) {
            return;
        }
        for (index, screen) in self.screens.iter().enumerate() {
            let shown = screen.shown.unwrap_or_default();
            // What a drag that has gone far enough draws, or else the window
            // under the pointer, which a click would pick.
            let drag = self
                .drag
                .filter(|(on, from, to)| *on == index && !area::click(*from, *to));
            let drawn = drag.map(|(_, from, to)| area::between(from, to, shown));
            let still = &self.stills[screen.still];
            let over = self.over.filter(|(on, _)| *on == index);
            let under = over.and_then(|(_, at)| area::under(at, &still.windows, shown, still.size));
            let drawn = drawn.or(under.map(|(_, rect)| rect));
            let rects = dimmed(drawn, shown).into_iter();
            let rects = rects.chain(bordered(drawn, self.look.thick));
            for (band, rect) in screen.bands.iter().zip(rects) {
                band.place(rect);
            }
            screen.surface.commit();
        }
    }

    /// Takes in where the pointer is, on which surface.
    fn pointed(&mut self, surface: &wl_surface::WlSurface, (x, y): (f64, f64)) {
        let at = (whole(x), whole(y));
        let on = self
            .screens
            .iter()
            .position(|screen| screen.surface == *surface);
        self.over = on.map(|on| (on, at));
        self.dragged(at);
    }

    /// Takes in the pointer moving on the screen it is over.
    fn moved_to(&mut self, (x, y): (f64, f64)) {
        let at = (whole(x), whole(y));
        self.over = self.over.map(|(on, _)| (on, at));
        self.dragged(at);
    }

    /// Has a drag reach `at`, if the pointer is on the screen it began on.
    fn dragged(&mut self, at: (i32, i32)) {
        let on = self.over.map(|(on, _)| on);
        let dragging = self.drag.filter(|(from, ..)| Some(*from) == on);
        self.drag = dragging.map(|(on, from, _)| (on, from, at)).or(self.drag);
        self.moved = true;
    }

    /// Takes in a button going down or up: the left begins a drag and,
    /// let go, picks what was drawn; any other picks nothing.
    fn clicked(&mut self, button: u32, pressed: bool) {
        match (button, pressed) {
            (LEFT, true) => self.drag = self.over.map(|(on, at)| (on, at, at)),
            (LEFT, false) => {
                let drag = self.drag.take();
                // What is picked is told by the picture it was, not by where
                // that is among those that are shown.
                let picked = drag.map(|(on, from, to)| {
                    let screen = &self.screens[on];
                    let shown = screen.shown.unwrap_or_default();
                    let still = &self.stills[screen.still];
                    let windows = &still.windows;
                    area::picked(screen.still, (from, to), windows, shown, still.size)
                });
                self.ended(picked.map(Outcome::Picked));
            }
            (_, true) => self.ended(Some(Outcome::Nothing)),
            (_, false) => {}
        }
        self.moved = true;
    }

    /// Notes how picking ended, if it has not already.
    fn ended(&mut self, outcome: Option<Outcome>) {
        self.outcome = self.outcome.or(outcome);
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

    /// Takes in a key going down: Enter picks every screen, and Escape
    /// nothing.
    fn pressed(&mut self, code: u32) {
        // The numbers the layout knows keys by are eight higher.
        let code = xkb::Keycode::new(code.saturating_add(8));
        let typing = self.typing.as_ref();
        let outcome = typing.and_then(|typing| match typing.key_get_one_sym(code) {
            xkb::Keysym::Return | xkb::Keysym::KP_Enter => {
                Some(Outcome::Picked(Picked::Everything))
            }
            xkb::Keysym::Escape => Some(Outcome::Nothing),
            _ => None,
        });
        self.ended(outcome);
    }

    /// Takes everything off the screen.
    fn close(&mut self) {
        self.screens.drain(..).for_each(Screen::close);
        self.shape
            .take()
            .into_iter()
            .for_each(|shape| shape.destroy());
    }
}

impl Screen {
    /// Puts picture `still` over `output`, above everything and with the
    /// keyboard, the dim and the border over it in `look`, which take
    /// nothing from the pointer: `empty` is where they do.
    fn new(
        globals: &Globals,
        (still, output): (usize, &wl_output::WlOutput),
        (look, empty): (Look, &wl_region::WlRegion),
        handle: &QueueHandle<State>,
    ) -> Self {
        let surface = globals.compositor.create_surface(handle, ());
        // Its name is no part of the theme's, so that it goes at once and
        // is not in the picture it leads to.
        let layer = globals.layers.get_layer_surface(
            &surface,
            Some(output),
            zwlr_layer_shell_v1::Layer::Overlay,
            "omorfos-picker".to_owned(),
            handle,
            (),
        );
        layer.set_anchor(zwlr_layer_surface_v1::Anchor::all());
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
        let viewport = globals.viewporter.get_viewport(&surface, handle, ());
        let colours = [look.dim; 4].into_iter().chain([look.border; 4]);
        let bands = colours
            .filter_map(|colour| Band::new(globals, &surface, (colour, empty), handle))
            .collect();
        surface.commit();
        Self {
            still,
            surface,
            layer,
            viewport,
            shown: None,
            painted: None,
            bands,
        }
    }

    fn close(self) {
        self.bands.into_iter().for_each(Band::close);
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
        self.painted.iter().for_each(wl_buffer::WlBuffer::destroy);
    }
}

impl Band {
    /// A pixel of `colour` over `parent`, which takes nothing from the
    /// pointer: `empty` is where it does. Nothing, if the pixel cannot be
    /// made.
    fn new(
        globals: &Globals,
        parent: &wl_surface::WlSurface,
        (colour, empty): (Color, &wl_region::WlRegion),
        handle: &QueueHandle<State>,
    ) -> Option<Self> {
        let mut canvas = Canvas::new(1, 1);
        canvas.blend(0, 0, colour, u8::MAX);
        let colour = picture(&globals.shm, &canvas, handle).ok();
        colour.map(|colour| {
            let surface = globals.compositor.create_surface(handle, ());
            surface.set_input_region(Some(empty));
            let over = globals
                .subsurfaces
                .get_subsurface(&surface, parent, handle, ());
            let viewport = globals.viewporter.get_viewport(&surface, handle, ());
            Self {
                surface,
                over,
                viewport,
                colour,
            }
        })
    }

    /// Puts it over `rect` of the picture, or takes it off where that has
    /// nothing in it.
    fn place(&self, (x, y, width, height): Rect) {
        let shown = width > 0 && height > 0;
        self.over.set_position(x, y);
        let size = (width.max(1).cast_signed(), height.max(1).cast_signed());
        self.viewport.set_destination(size.0, size.1);
        self.surface.attach(shown.then_some(&self.colour), 0, 0);
        self.surface.damage_buffer(0, 0, 1, 1);
        self.surface.commit();
    }

    fn close(self) {
        self.viewport.destroy();
        self.over.destroy();
        self.surface.destroy();
        self.colour.destroy();
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        // What goes while it is up is seen to by what it was on going.
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_compositor" => {
                    state.compositor = Some(registry.bind(name, version.min(4), handle, ()));
                }
                "wl_subcompositor" => state.subsurfaces = Some(registry.bind(name, 1, handle, ())),
                "wl_shm" => state.shm = Some(registry.bind(name, 1, handle, ())),
                "zwlr_layer_shell_v1" => state.layers = Some(registry.bind(name, 1, handle, ())),
                "wp_viewporter" => state.viewporter = Some(registry.bind(name, 1, handle, ())),
                "wp_cursor_shape_manager_v1" => {
                    state.shapes = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_seat" => {
                    let _: wl_seat::WlSeat = registry.bind(name, version.min(5), handle, ());
                }
                // Each screen says what it is called from the fourth.
                "wl_output" if version >= 4 => {
                    let output = registry.bind(name, 4, handle, ());
                    state.outputs.push((output, String::new()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            let known = state
                .outputs
                .iter_mut()
                .filter(|(known, _)| known == output);
            known.for_each(|(_, called)| called.clone_from(&name));
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
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

impl Dispatch<wl_pointer::WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        pointer: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                state.pointed(&surface, (surface_x, surface_y));
                // Over the pictures the pointer is a crosshair.
                let shapes = state.shapes.as_ref();
                let shape = shapes.map(|shapes| {
                    let made = || shapes.get_pointer(pointer, handle, ());
                    state.shape.get_or_insert_with(made)
                });
                let crosshair = wp_cursor_shape_device_v1::Shape::Crosshair;
                let _ = shape.inspect(|shape| shape.set_shape(serial, crosshair));
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => state.moved_to((surface_x, surface_y)),
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(pressed),
                ..
            } => state.clicked(button, pressed == wl_pointer::ButtonState::Pressed),
            _ => {}
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { fd, size, .. } => state.learn_layout(fd, size),
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(wl_keyboard::KeyState::Pressed),
                ..
            } => state.pressed(key),
            _ => {}
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, ()> for State {
    fn event(
        state: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let ours = state
            .screens
            .iter_mut()
            .filter(|screen| screen.layer == *layer);
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            layer.ack_configure(serial);
            for screen in ours {
                screen.shown = Some((width, height));
            }
            return;
        }
        // The only other thing it says is that the compositor closed it: it
        // would not let it show, or its screen went.
        let shown = ours.into_iter().any(|screen| screen.shown.is_some());
        let outcome = if shown {
            Outcome::Nothing
        } else {
            Outcome::Refused
        };
        state.ended(Some(outcome));
    }
}

delegate_noop!(State: wl_compositor::WlCompositor);
delegate_noop!(State: wl_subcompositor::WlSubcompositor);
delegate_noop!(State: wl_subsurface::WlSubsurface);
delegate_noop!(State: wl_region::WlRegion);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ignore wl_surface::WlSurface);
delegate_noop!(State: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(State: wp_viewporter::WpViewporter);
delegate_noop!(State: wp_viewport::WpViewport);
delegate_noop!(State: wp_cursor_shape_manager_v1::WpCursorShapeManagerV1);
delegate_noop!(State: wp_cursor_shape_device_v1::WpCursorShapeDeviceV1);
