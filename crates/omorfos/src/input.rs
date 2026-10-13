//! Input from the user.

use omorfos_config::Modifiers;
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Button, Edges, MenuKey, MenuOutcome, Part, WindowId, resize};
use smithay::backend::input::{Axis, ButtonState, KeyState};
use smithay::desktop::{WindowSurfaceType, layer_map_for_output};
use smithay::input::keyboard::{FilterResult, Keycode, Keysym};
use smithay::input::pointer::{AxisFrame, ButtonEvent, CursorIcon, CursorImageStatus, MotionEvent};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::shell::wlr_layer::Layer;
use smithay::wayland::shell::xdg::PopupSurface;

use crate::bridge::{self, Chord};
use crate::compositor::{Compositor, Look};
use crate::reveal::Placing;
use omorfos_core::KeyOutcome;

/// An input event, reduced to what the compositor acts on.
///
/// Backends translate whatever their input source produces into this, which
/// keeps input handling independent of where input comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    /// A key was pressed or released.
    Key {
        /// The XKB keycode.
        code: Keycode,
        /// `true` for a press, `false` for a release.
        pressed: bool,
    },
    /// The pointer moved to a position in the space the outputs show, in
    /// units of layout (see [`Compositor::area_of`]).
    Motion {
        /// Distance from the left edge.
        x: f64,
        /// Distance from the top edge.
        y: f64,
    },
    /// The pointer moved by a distance, in units of layout, as a mouse or
    /// touchpad reports it. The pointer stays on an output: it crosses from
    /// one to the next where they meet and stops at their other edges.
    MotionBy {
        /// Distance moved to the right.
        dx: f64,
        /// Distance moved down.
        dy: f64,
    },
    /// A pointer button was pressed or released.
    Button {
        /// The Linux input event code, such as `0x110` for the left button.
        button: u32,
        /// `true` for a press, `false` for a release.
        pressed: bool,
    },
    /// The wheel or touchpad scrolled by these amounts.
    Scroll {
        /// Rightwards scroll distance.
        horizontal: f64,
        /// Downwards scroll distance.
        vertical: f64,
    },
}

/// What a key means to an open menu, if anything. A key pressed with a
/// modifier held is a binding's, not the menu's.
fn menu_key(keysym: Keysym, held: Modifiers) -> Option<MenuKey> {
    let key = match keysym {
        Keysym::Up => MenuKey::Up,
        Keysym::Down => MenuKey::Down,
        Keysym::Left => MenuKey::Left,
        Keysym::Right => MenuKey::Right,
        Keysym::Return | Keysym::KP_Enter => MenuKey::Enter,
        _ => return None,
    };
    (held == Modifiers::NONE).then_some(key)
}

/// The virtual terminal a key asks for, if it is one of the keys that
/// switch between them: Ctrl+Alt+F1 to F12 on the usual layouts.
pub(crate) fn virtual_terminal(keysym: Keysym) -> Option<i32> {
    let first = Keysym::XF86_Switch_VT_1.raw();
    (first..first + 12)
        .position(|switch| switch == keysym.raw())
        .and_then(|index| i32::try_from(index + 1).ok())
}

/// The pointer shape that says a drag here resizes these edges.
fn resize_icon(edges: Edges) -> CursorIcon {
    let has = |edge| edges.contains(edge);
    let (left, right) = (has(Edges::LEFT), has(Edges::RIGHT));
    let (top, bottom) = (has(Edges::TOP), has(Edges::BOTTOM));
    match (left || right, top || bottom) {
        (true, false) => CursorIcon::EwResize,
        (false, _) => CursorIcon::NsResize,
        (true, true) if left == top => CursorIcon::NwseResize,
        (true, true) => CursorIcon::NeswResize,
    }
}

/// An input event once the pointer's position is known: a distance moved
/// has become the place arrived at, so nothing later has to tell the two
/// apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Placed {
    Key { code: Keycode, pressed: bool },
    Motion { x: f64, y: f64 },
    Button { button: u32, pressed: bool },
    Scroll { horizontal: f64, vertical: f64 },
}

/// Linux input event codes for the pointer buttons.
/// How long there may be between two presses on a titlebar for them to be
/// a double click, in thousandths of a second.
const DOUBLE_CLICK: u32 = 400;

const BUTTON_LEFT: u32 = 0x110;
const BUTTON_RIGHT: u32 = 0x111;
const BUTTON_MIDDLE: u32 = 0x112;

/// The smallest window a frame can be resized down to.
const MINIMUM_WINDOW: DomainSize = DomainSize::new(64, 32);

/// A drag in progress, which has the pointer to itself until a button is
/// released.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Drag {
    /// Moving a frame. `grab` is where on the frame the pointer holds it.
    Move {
        window: WindowId,
        grab: (i32, i32),
        size: DomainSize,
    },
    /// Resizing a frame by some of its edges, from `initial`.
    Resize {
        window: WindowId,
        edges: Edges,
        initial: Rect,
        start: DomainPoint,
    },
    /// Carrying a tab to drop on another frame, or out of its own.
    Tab { window: WindowId },
}

/// What a press asks for, before checking that the frame allows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wanted {
    Move,
    Resize(Edges),
    Tab,
    /// The menu of what can be done with the window.
    Menu,
    Nothing,
}

/// What is under the pointer.
enum Under {
    /// A window's frame.
    Frame(WindowId, Hit),
    /// A layer-shell surface, and where it is on the output. `wallpaper`
    /// marks the background layer, which stands in for the bare desktop.
    /// `takes_keyboard` is the layer's own surface if it accepts the
    /// keyboard when clicked.
    Layer {
        surface: WlSurface,
        origin: Point<f64, Logical>,
        wallpaper: bool,
        takes_keyboard: Option<WlSurface>,
    },
}

impl Under {
    /// The client surface this is, if it is one rather than a decoration.
    /// The surface that has the pointer when this is under it, and where
    /// that surface is. The compositor's own parts of a frame are no
    /// surface's.
    fn focus(&self) -> Option<(WlSurface, Point<f64, Logical>)> {
        match self {
            Self::Frame(_, Hit::Surface(surface, origin))
            | Self::Layer {
                surface, origin, ..
            } => Some((surface.clone(), *origin)),
            Self::Frame(_, Hit::Part(_)) => None,
        }
    }

    const fn surface(&self) -> Option<&WlSurface> {
        match self {
            Self::Frame(_, Hit::Surface(surface, _)) | Self::Layer { surface, .. } => Some(surface),
            Self::Frame(_, Hit::Part(_)) => None,
        }
    }
}

/// What the pointer is on, within a frame.
enum Hit {
    /// Part of a frame's decoration.
    Part(Part),
    /// A client surface, and where that surface is on the output.
    Surface(WlSurface, Point<f64, Logical>),
}

#[derive(Default)]
pub(crate) struct FrameFacts {
    outer: Rect,
    movable: bool,
    tabs: Vec<WindowId>,
}

impl FrameFacts {
    /// The corner of the frame nearest `point`, as the two edges meeting
    /// there.
    pub(crate) fn nearest_corner(&self, point: DomainPoint) -> Edges {
        let middle = |start: i32, length: u32| i64::from(start) + i64::from(length / 2);
        let left = i64::from(point.x) < middle(self.outer.origin.x, self.outer.size.width);
        let top = i64::from(point.y) < middle(self.outer.origin.y, self.outer.size.height);
        Edges::NONE
            .with_if(left, Edges::LEFT)
            .with_if(!left, Edges::RIGHT)
            .with_if(top, Edges::TOP)
            .with_if(!top, Edges::BOTTOM)
    }
}

impl Compositor {
    /// Handles an input event that happened at `time`, in milliseconds on
    /// any monotonic clock.
    pub fn input(&mut self, event: Input, time: u32) {
        // Whatever it is, the user is here.
        let seat = self.seat.clone();
        self.idle.notify_activity(&seat);
        self.presence.stirred = true;
        // A distance moved becomes the position arrived at.
        let event = match event {
            Input::Key { code, pressed } => Placed::Key { code, pressed },
            Input::Motion { x, y } => Placed::Motion { x, y },
            Input::MotionBy { dx, dy } => {
                let to = self.moved_by(dx, dy);
                Placed::Motion { x: to.x, y: to.y }
            }
            Input::Button { button, pressed } => Placed::Button { button, pressed },
            Input::Scroll {
                horizontal,
                vertical,
            } => Placed::Scroll {
                horizontal,
                vertical,
            },
        };
        if self.is_locked() {
            // A locked session sees none of what follows.
            self.locked_input(event, time);
            return;
        }
        match event {
            Placed::Key { code, pressed } => self.key(code, pressed, time),
            Placed::Motion { x, y } => self.motion(x, y, time),
            Placed::Button { button, pressed } => self.button(button, pressed, time),
            Placed::Scroll {
                horizontal,
                vertical,
            } => {
                let frame = AxisFrame::new(time)
                    .value(Axis::Horizontal, horizontal)
                    .value(Axis::Vertical, vertical);
                let pointer = self.pointer.clone();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
        }
    }

    fn key(&mut self, code: Keycode, pressed: bool, time: u32) {
        let keyboard = self.keyboard.clone();
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        let serial = SERIAL_COUNTER.next_serial();
        let action = keyboard.input(self, code, state, serial, time, |compositor, held, key| {
            if !pressed {
                // Swallow the release of a key whose press was swallowed.
                let before = compositor.suppressed_keys.len();
                compositor
                    .suppressed_keys
                    .retain(|suppressed| *suppressed != code);
                return if compositor.suppressed_keys.len() < before {
                    FilterResult::Intercept(None)
                } else {
                    FilterResult::Forward
                };
            }
            if let Some(terminal) = virtual_terminal(key.modified_sym()) {
                // Not ours to act on: the backend that holds the seat is
                // asked, and no client or binding sees the key.
                compositor.terminal_wanted = Some(terminal);
                compositor.suppressed_keys.push(code);
                return FilterResult::Intercept(None);
            }
            let keysym = key
                .raw_latin_sym_or_raw_current_sym()
                .unwrap_or(key.modified_sym());
            // A key sends away what was brought back over a fullscreen
            // window, and is the window's all the same.
            compositor.hover.reveal.dismiss();
            if compositor.menu.is_some() && keysym == Keysym::Escape {
                compositor.menu = None;
                compositor.suppressed_keys.push(code);
                return FilterResult::Intercept(None);
            }
            // An open menu is steered by the arrows and Enter, which no
            // client or binding then sees.
            let steer = menu_key(keysym, bridge::modifiers(held));
            if let Some((menu, steer)) = compositor.menu.as_mut().zip(steer) {
                let chosen = match menu.key(steer) {
                    MenuOutcome::Activate(action) => Some(action),
                    _ => None,
                };
                if chosen.is_some() {
                    compositor.menu = None;
                }
                compositor.suppressed_keys.push(code);
                return FilterResult::Intercept(chosen);
            }
            if keysym.is_modifier_key() {
                // Holding a modifier on the way to the next chord is not a
                // step in a chain.
                return FilterResult::Forward;
            }
            let chord = Chord {
                modifiers: bridge::modifiers(held),
                keysym: keysym.raw(),
            };
            let action = match compositor.bindings.press(chord) {
                KeyOutcome::Unbound => return FilterResult::Forward,
                KeyOutcome::Pending | KeyOutcome::Cancelled => None,
                KeyOutcome::Triggered(action) => Some(action.clone()),
            };
            compositor.suppressed_keys.push(code);
            FilterResult::Intercept(action)
        });
        if let Some(action) = action.flatten() {
            self.perform(&action);
        }
    }

    fn motion(&mut self, x: f64, y: f64, time: u32) {
        self.pointer_location = (x, y).into();
        // Whatever else has the pointer, no button of a titlebar has.
        self.hover.button = None;
        let position = self.pointer_position();
        if let Some(drag) = self.drag {
            match drag {
                Drag::Move { window, grab, size } => {
                    let origin = position.offset(-grab.0, -grab.1);
                    self.desktop.set_geometry(window, Rect::new(origin, size));
                }
                Drag::Resize {
                    window,
                    edges,
                    initial,
                    start,
                } => {
                    let (dx, dy) = (
                        position.x.saturating_sub(start.x),
                        position.y.saturating_sub(start.y),
                    );
                    let resized = resize(initial, edges, dx, dy, self.minimum_frame_size());
                    self.desktop.set_geometry(window, resized);
                }
                Drag::Tab { .. } => {}
            }
            // The drag has the pointer; no window sees it meanwhile.
            self.move_pointer(None, time);
            return;
        }

        if self.over_notice() {
            // The notice is ours; nothing underneath sees the pointer.
            self.move_pointer(None, time);
            return;
        }
        if let Some(menu) = &mut self.menu {
            // An open menu has the pointer, like a drag.
            menu.hover(position);
            self.move_pointer(None, time);
            return;
        }

        let under = self.under();
        self.hover.button = Self::button_under(under.as_ref());
        let hovered = match &under {
            Some(Under::Frame(id, _)) => Some(*id),
            _ => None,
        };
        if hovered != self.hover.window {
            self.hover.window = hovered;
            if let Some(id) = hovered {
                self.desktop.pointer_entered(id);
            }
        }
        self.move_pointer(under.as_ref().and_then(Under::focus), time);
    }

    /// Takes a press of a mouse button on the frame of `id`, on its
    /// decoration or on its window: gives the frame focus, and begins
    /// whatever the press begins there.
    fn press_frame(&mut self, id: WindowId, hit: &Hit, button: u32, time: u32) {
        let frame = self.frame_facts(id);
        let tab = |index: &usize| frame.tabs.get(*index).copied().unwrap_or(id);
        let alt = self.keyboard.modifier_state().alt;
        let (clicked, wanted) = match (hit, button, alt) {
            (Hit::Part(Part::Button(which)), BUTTON_LEFT, _) => {
                // It does what it does when it is let go on.
                self.hover.pressed = Some((id, *which));
                (id, Wanted::Nothing)
            }
            (Hit::Part(Part::Tab(_)), BUTTON_LEFT, _) if self.double_click(id, time) => {
                self.desktop.toggle_maximized(id);
                (id, Wanted::Nothing)
            }
            (Hit::Part(Part::Tab(index)), BUTTON_LEFT, _) => (tab(index), Wanted::Move),
            (Hit::Part(Part::Tab(index)), BUTTON_MIDDLE, _) => (tab(index), Wanted::Tab),
            (Hit::Part(Part::Tab(index)), BUTTON_RIGHT, _) => (tab(index), Wanted::Menu),
            (Hit::Part(Part::Border(edges)), BUTTON_LEFT, _) => (id, Wanted::Resize(*edges)),
            (Hit::Surface(..), BUTTON_LEFT, true) => (id, Wanted::Move),
            (Hit::Surface(..), BUTTON_RIGHT, true) => (
                id,
                Wanted::Resize(frame.nearest_corner(self.pointer_position())),
            ),
            _ => (id, Wanted::Nothing),
        };
        self.desktop.clicked(clicked);
        self.begin(clicked, wanted);
        if wanted == Wanted::Menu {
            self.open_window_menu();
        }
    }

    /// The button of a titlebar that is under the pointer, and the window
    /// whose frame has it.
    fn button_under(under: Option<&Under>) -> Option<(WindowId, Button)> {
        match under {
            Some(Under::Frame(id, Hit::Part(Part::Button(which)))) => Some((*id, *which)),
            _ => None,
        }
    }

    /// Does what a button of a window's titlebar does.
    fn press_button(&mut self, window: WindowId, which: Button) {
        match which {
            Button::Close => {
                // A window that has just gone has nothing to close.
                let _ = self.mapped(window).map(|mapped| mapped.role.close());
            }
            // The button is for fullscreen, as on macOS: the window goes
            // to a workspace of its own. Maximizing is a double click on
            // the titlebar.
            Button::Maximize => self.desktop.toggle_fullscreen(window),
            Button::Minimize => self.desktop.minimize(window),
        }
    }

    /// Whether a press on a window's titlebar at `time`, in thousandths
    /// of a second, is the second of two close together in time and
    /// place. Either way it is remembered, to be the first of the next.
    fn double_click(&mut self, window: WindowId, time: u32) -> bool {
        let position = self.pointer_position();
        let before = self.hover.clicked.replace((window, time, position));
        before.is_some_and(|(last, at, place)| {
            let near = place.x.abs_diff(position.x) <= 4 && place.y.abs_diff(position.y) <= 4;
            last == window && near && time.wrapping_sub(at) <= DOUBLE_CLICK
        })
    }

    pub(crate) fn move_pointer(
        &mut self,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        time: u32,
    ) {
        let pointer = self.pointer.clone();
        let event = MotionEvent {
            location: self.pointer_location,
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        pointer.motion(self, focus, &event);
        pointer.frame(self);
    }

    /// Closes the menus that asked for it, if a press lands anywhere but on
    /// a surface of the application they belong to. Innermost first, as the
    /// protocol requires.
    fn dismiss_popups(&mut self, pressed_on: Option<&Under>) {
        self.grabbed_popups.retain(PopupSurface::alive);
        let client = |surface: &WlSurface| surface.client().map(|client| client.id());
        let owner = self
            .grabbed_popups
            .first()
            .and_then(|popup| client(popup.wl_surface()));
        let target = pressed_on.and_then(Under::surface).and_then(client);
        if owner.is_some() && owner != target {
            for popup in self.grabbed_popups.drain(..).rev() {
                popup.send_popup_done();
            }
        }
    }

    /// Whether the pointer is on the problem notice.
    /// What the pointer should look like where it is.
    ///
    /// Over an application's window it is what the application asked for.
    /// Everywhere else it is the compositor's to choose: a shape that says
    /// what a drag is doing or would do, or the ordinary pointer.
    pub(crate) fn cursor_wanted(&self) -> Look {
        if self.is_locked() {
            // The locker's choice, as for any application, and nothing of
            // the session's own.
            return self.cursor_asked_for();
        }
        match self.drag {
            Some(Drag::Move { .. } | Drag::Tab { .. }) => {
                return Look::Icon(CursorIcon::Grabbing);
            }
            Some(Drag::Resize { edges, .. }) => return Look::Icon(resize_icon(edges)),
            None => {}
        }
        if self.over_notice() || self.menu.is_some() {
            return Look::Icon(CursorIcon::Default);
        }
        match self.under() {
            Some(Under::Frame(_, Hit::Part(Part::Border(edges)))) => Look::Icon(resize_icon(edges)),
            Some(Under::Frame(_, Hit::Surface(..)) | Under::Layer { .. }) => {
                self.cursor_asked_for()
            }
            _ => Look::Icon(CursorIcon::Default),
        }
    }

    /// What the application under the pointer last asked it to look like.
    fn cursor_asked_for(&self) -> Look {
        match &self.cursor {
            CursorImageStatus::Hidden => Look::Hidden,
            CursorImageStatus::Named(icon) => Look::Icon(*icon),
            CursorImageStatus::Surface(surface) if surface.is_alive() => {
                Look::Surface(surface.clone())
            }
            CursorImageStatus::Surface(_) => Look::Icon(CursorIcon::Default),
        }
    }

    fn over_notice(&self) -> bool {
        let position = self.pointer_position();
        self.notice
            .as_ref()
            .is_some_and(|notice| notice.area.contains(position))
    }

    /// What is under the pointer, in the order things are drawn: overlays,
    /// fullscreen windows, panels and docks, ordinary windows, then the
    /// layers behind them.
    fn under(&self) -> Option<Under> {
        // Panels belong to an output and are placed within it.
        let point = self.pointer_location;
        let index = self.heads.iter().position(|head| head.contains(point));
        let on_panel = |layer: Layer, standing: Placing| {
            let index = index?;
            let head = &self.heads[index];
            let layers = layer_map_for_output(&head.output);
            let corner = head.area().loc;
            let surface = layers.layer_under(layer, self.pointer_location - corner.to_f64())?;
            // A panel that is not shown is not there to be pointed at.
            let area = layers.layer_geometry(surface).unwrap_or_default();
            let shown = self.standing(index, layer, area) == standing;
            let surface = shown.then_some(surface)?;
            let origin = layers.layer_geometry(surface).unwrap_or_default().loc + corner;
            let within = self.pointer_location - origin.to_f64();
            let takes_keyboard = surface
                .can_receive_keyboard_focus()
                .then(|| surface.wl_surface().clone());
            let (surface, offset) = surface.surface_under(within, WindowSurfaceType::ALL)?;
            let wallpaper = layer == Layer::Background;
            Some(Under::Layer {
                surface,
                origin: (offset + origin).to_f64(),
                wallpaper,
                takes_keyboard,
            })
        };
        let on_frame = |fullscreen| {
            self.frame_under(fullscreen)
                .map(|(id, hit)| Under::Frame(id, hit))
        };
        // A menu or a tooltip of a program written for X11, which is a
        // window that manages itself, is over every other window.
        let on_unmanaged = || {
            let point = self.pointer_location;
            let mut top_first = self.x11.unmanaged().rev();
            top_first.find_map(|(window, surface)| {
                let area = window.geometry().to_f64();
                area.contains(point).then(|| Under::Layer {
                    surface: surface.clone(),
                    origin: area.loc,
                    wallpaper: false,
                    takes_keyboard: None,
                })
            })
        };
        let on_layer = |layer: Layer| on_panel(layer, Placing::Usual);
        on_layer(Layer::Overlay)
            .or_else(on_unmanaged)
            .or_else(|| on_panel(Layer::Top, Placing::Raised))
            .or_else(|| on_frame(true))
            .or_else(|| on_layer(Layer::Top))
            .or_else(|| on_frame(false))
            .or_else(|| on_layer(Layer::Bottom))
            .or_else(|| on_layer(Layer::Background))
    }

    /// Whether the pointer is on the titlebar of a window that fills a
    /// workspace made for it, or its part of one: a titlebar that was
    /// brought back.
    pub(crate) fn on_titlebar(&self) -> bool {
        matches!(self.frame_under(true), Some((_, Hit::Part(_))))
    }

    /// The frame under the pointer, whatever else may be in front of it.
    fn target(&self) -> Option<(WindowId, Hit)> {
        self.frame_under(true).or_else(|| self.frame_under(false))
    }

    /// The topmost frame, among those that are or are not fullscreen, whose
    /// decoration or surface is under the pointer, and which of the two.
    fn frame_under(&self, fullscreen: bool) -> Option<(WindowId, Hit)> {
        let point = self.pointer_position();
        let frames = self
            .scene
            .iter()
            .rev()
            .filter(|shown| shown.over == fullscreen);
        frames.into_iter().find_map(|shown| {
            let part = shown
                .decoration()
                .and_then(|decoration| decoration.hit(point))
                .filter(|part| *part != Part::Client);
            // Nothing of a window that is rolled up into its titlebar is
            // there to be pointed at.
            let window = (!shown.shaded).then_some(&shown.window);
            let hit = part.map(Hit::Part).or_else(|| {
                let window = window?;
                // A window may draw outside what it calls its geometry, for
                // a shadow say, so its surface can begin above and left of
                // where the window is placed.
                let surface_origin = shown.origin - shown.window.geometry().loc;
                let within = self.pointer_location - surface_origin.to_f64();
                window
                    .surface_under(within, WindowSurfaceType::ALL)
                    .map(|(surface, offset)| {
                        Hit::Surface(surface, (offset + surface_origin).to_f64())
                    })
            });
            hit.map(|hit| (shown.id, hit))
        })
    }

    fn button(&mut self, button: u32, pressed: bool, time: u32) {
        if let Some(drag) = self.drag {
            // Everything is swallowed during a drag; any release ends it.
            if !pressed {
                self.drag = None;
                self.drop_tab(drag);
            }
            return;
        }
        let position = self.pointer_position();
        if self.over_notice() {
            // A click on the notice dismisses it, until what it says changes.
            if pressed {
                self.dismissed = self.said().into_owned();
            }
            return;
        }
        if let Some(menu) = &mut self.menu {
            let outcome = if pressed {
                menu.press(position)
            } else {
                MenuOutcome::Nothing
            };
            match outcome {
                MenuOutcome::Activate(action) => {
                    self.menu = None;
                    self.perform(&action);
                }
                MenuOutcome::Dismiss => self.menu = None,
                MenuOutcome::Nothing => {}
            }
            return;
        }
        let under = self.under();
        // A press in a window is for the window: what was brought back
        // over it has served.
        if pressed && matches!(under, Some(Under::Frame(_, Hit::Surface(..)))) {
            self.hover.reveal.dismiss();
        }
        // A button of a titlebar does what it does if the pointer is let
        // go on the button it was pressed on.
        let on = Self::button_under(under.as_ref());
        let chosen = self.hover.pressed.take_if(|_| !pressed);
        if let Some((window, which)) = chosen.filter(|chosen| on == Some(*chosen)) {
            self.press_button(window, which);
        }
        if pressed {
            // Something may have come or gone under a pointer that has not
            // moved since, as a launcher does when it opens. The press is
            // for what is there now, which is told the pointer is on it
            // first.
            let focus = under.as_ref().and_then(Under::focus);
            let on = focus.as_ref().map(|(surface, _)| surface);
            if on != self.pointer.current_focus().as_ref() {
                self.move_pointer(focus, time);
            }
            self.dismiss_popups(under.as_ref());
            // A panel that accepts the keyboard, such as one with a search
            // box, has it from a click on it until a click elsewhere.
            self.layer_focus = match &under {
                Some(Under::Layer { takes_keyboard, .. }) => takes_keyboard.clone(),
                _ => None,
            };
        }
        let desktop = matches!(
            under,
            None | Some(Under::Layer {
                wallpaper: true,
                ..
            })
        );
        if pressed && button == BUTTON_RIGHT && desktop {
            // A right click on the bare desktop, or on a wallpaper covering
            // it, opens the root menu.
            self.open_menu();
            self.move_pointer(None, time);
            return;
        }
        if let (true, Some(Under::Frame(id, hit))) = (pressed, under) {
            self.press_frame(id, &hit, button, time);
            if self.drag.is_some() {
                self.move_pointer(None, time);
                return;
            }
        }
        let state = if pressed {
            ButtonState::Pressed
        } else {
            ButtonState::Released
        };
        let pointer = self.pointer.clone();
        let event = ButtonEvent {
            serial: SERIAL_COUNTER.next_serial(),
            time,
            button,
            state,
        };
        pointer.button(self, &event);
        pointer.frame(self);
    }

    /// Starts a drag of `window`'s frame from where the pointer is now. A
    /// maximized or fullscreen frame cannot be moved or resized.
    pub(crate) fn begin(&mut self, window: WindowId, wanted: Wanted) {
        let frame = self.frame_facts(window);
        let position = self.pointer_position();
        self.drag = match wanted {
            Wanted::Move if frame.movable => {
                let grab = (
                    position.x.saturating_sub(frame.outer.origin.x),
                    position.y.saturating_sub(frame.outer.origin.y),
                );
                Some(Drag::Move {
                    window,
                    grab,
                    size: frame.outer.size,
                })
            }
            Wanted::Resize(edges) if frame.movable => Some(Drag::Resize {
                window,
                edges,
                initial: frame.outer,
                start: position,
            }),
            Wanted::Tab => Some(Drag::Tab { window }),
            _ => None,
        };
    }

    /// Finishes a tab drag: dropped on another frame's titlebar the tab
    /// joins that frame, and dropped anywhere else it gets a frame of its
    /// own. Other drags need no finishing.
    fn drop_tab(&mut self, drag: Drag) {
        if let Drag::Tab { window } = drag {
            let onto = self
                .target()
                .and_then(|(id, hit)| matches!(hit, Hit::Part(Part::Tab(_))).then_some(id));
            match onto {
                Some(target) => self.desktop.attach(window, target),
                None => self.desktop.detach(window),
            }
        }
    }

    /// The facts about the frame holding `window` that dragging needs.
    pub(crate) fn frame_facts(&self, window: WindowId) -> FrameFacts {
        self.desktop
            .frames()
            .find(|frame| frame.tabs.iter().any(|tab| tab.id == window))
            .map(|frame| FrameFacts {
                outer: frame.geometry,
                movable: !frame.maximized && !frame.fullscreen,
                tabs: frame.tabs.iter().map(|tab| tab.id).collect(),
            })
            .unwrap_or_default()
    }

    /// The smallest a frame may be dragged: room for a small window.
    fn minimum_frame_size(&self) -> DomainSize {
        bridge::settings(self.config())
            .decoration
            .outer_size(MINIMUM_WINDOW)
    }

    /// The pointer position as the desktop model sees it.
    pub(crate) fn pointer_position(&self) -> DomainPoint {
        // Positions are within the output, far inside the range of `i32`.
        #[expect(
            clippy::cast_possible_truncation,
            reason = "pointer positions fit in an i32"
        )]
        let round = |value: f64| value.round() as i32;
        DomainPoint::new(
            round(self.pointer_location.x),
            round(self.pointer_location.y),
        )
    }
}
