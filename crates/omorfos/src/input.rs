//! Input from the user.

use omorfos_config::{Action, Modifiers};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Button, Edges, Home, MenuKey, MenuOutcome, Part, Pattern, WindowId, resize};
use omorfos_i18n::Value;
use smithay::backend::input::{Axis, ButtonState, KeyState};
use smithay::desktop::{WindowSurfaceType, layer_map_for_output};
use smithay::input::keyboard::{FilterResult, KeyboardTarget, Keycode, Keysym, ModifiersState};
use smithay::input::pointer::{AxisFrame, ButtonEvent, CursorIcon, CursorImageStatus, MotionEvent};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER, Serial};
use smithay::wayland::shell::wlr_layer::Layer;
use smithay::wayland::shell::xdg::PopupSurface;

use crate::bridge::{self, Chord};
use crate::compositor::{About, Compositor, Look};
use crate::holding::Held;
use crate::paint;
use crate::reveal::Placing;
use crate::roles::Roles;
use crate::surfaces;
use crate::x11::Focus;
use omorfos_core::KeyOutcome;
use smithay::delegate_keyboard_shortcuts_inhibit;
use smithay::wayland::keyboard_shortcuts_inhibit::{
    KeyboardShortcutsInhibitHandler, KeyboardShortcutsInhibitState, KeyboardShortcutsInhibitor,
    KeyboardShortcutsInhibitorSeat,
};

/// The surface of a program the pointer is over, and where its corner is,
/// as the program is told: nothing while the compositor's own things have
/// the pointer.
pub(crate) type Focused = Option<(WlSurface, Point<f64, Logical>)>;

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
        /// Distance moved to the right as the device measured it, before
        /// a fast move was made to go further.
        raw_dx: f64,
        /// Distance moved down, as the device measured it.
        raw_dy: f64,
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
    /// Fingers moved together on a touchpad.
    Gesture(Gesture),
}

/// A gesture on a touchpad, in the steps the touchpad reports it: each
/// begins, may go on, and ends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    /// Fingers began to move across the touchpad together.
    SwipeBegin {
        /// How many.
        fingers: u32,
    },
    /// They moved, by a distance in units of layout.
    SwipeUpdate {
        /// Distance moved to the right.
        dx: f64,
        /// Distance moved down.
        dy: f64,
    },
    /// They lifted, or the swipe was `cancelled`.
    SwipeEnd {
        /// Whether it was cancelled rather than finished.
        cancelled: bool,
    },
    /// Fingers began to move towards or away from each other.
    PinchBegin {
        /// How many.
        fingers: u32,
    },
    /// They moved: their middle by a distance, how far apart they are by
    /// `scale` of how far they began, and turned by `rotation`.
    PinchUpdate {
        /// Distance their middle moved to the right.
        dx: f64,
        /// Distance their middle moved down.
        dy: f64,
        /// How far apart they are, as a part of how far they began.
        scale: f64,
        /// How far they turned since the last update, in degrees
        /// clockwise.
        rotation: f64,
    },
    /// They lifted, or the pinch was `cancelled`.
    PinchEnd {
        /// Whether it was cancelled rather than finished.
        cancelled: bool,
    },
    /// Fingers came to rest on the touchpad.
    HoldBegin {
        /// How many.
        fingers: u32,
    },
    /// They lifted, or moved, which `cancelled` says.
    HoldEnd {
        /// Whether they moved rather than lifted.
        cancelled: bool,
    },
}

/// What became of keys pressed, for when they are let go of.
#[derive(Debug, Default)]
pub(crate) struct Taken {
    /// Keys whose press was consumed, so their release is consumed too.
    pub(crate) suppressed: Vec<Keycode>,
    /// Keys pressed with Super that went to the program as something else,
    /// as what, whose release goes the same way (ADR 78).
    pub(crate) as_ctrl: Vec<(Keycode, Handed)>,
    /// A key the keyboard has taken that is to go to the program as
    /// something else, and whether it was pressed or let go of.
    pub(crate) handing: Option<(Handed, bool)>,
}

/// What a key pressed with Super, which nothing binds, goes to the program
/// as (ADR 78): itself with Ctrl in Super's place, or, for an arrow, Home
/// or End, as a Mac's Cmd with the arrows goes to the ends of the line and
/// of the text (#155).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Handed {
    /// The key sent.
    code: Keycode,
    /// Whether Ctrl is said to be held with it.
    ctrl: bool,
    /// Whether it is an arrow made Home or End, which a terminal is sent as
    /// it is, Shift being for choosing.
    arrow: bool,
}

impl Handed {
    /// The numbers of Home and End, as the keyboard numbers them, which are
    /// the same whatever the layout.
    const HOME: Keycode = Keycode::new(110);
    const END: Keycode = Keycode::new(115);

    /// What `code`, which is `keysym`, goes to the program as.
    fn of(code: Keycode, keysym: Keysym) -> Self {
        let arrow = |code, ctrl| Self {
            code,
            ctrl,
            arrow: true,
        };
        match keysym {
            Keysym::Left => arrow(Self::HOME, false),
            Keysym::Right => arrow(Self::END, false),
            Keysym::Up => arrow(Self::HOME, true),
            Keysym::Down => arrow(Self::END, true),
            _ => Self {
                code,
                ctrl: true,
                arrow: false,
            },
        }
    }
}

/// How far a key scrolls "About", in units of layout, a page being `page`
/// of them: a line for an arrow, a page for the keys of a page and for
/// space, and all the way for Home and End. None for any other key.
fn about_scroll(keysym: Keysym, page: f64) -> Option<f64> {
    let line = f64::from(paint::ABOUT_ROW);
    match keysym {
        Keysym::Down | Keysym::KP_Down => Some(line),
        Keysym::Up | Keysym::KP_Up => Some(-line),
        Keysym::Page_Down | Keysym::KP_Page_Down | Keysym::space => Some(page),
        Keysym::Page_Up | Keysym::KP_Page_Up => Some(-page),
        Keysym::End | Keysym::KP_End => Some(f64::INFINITY),
        Keysym::Home | Keysym::KP_Home => Some(f64::NEG_INFINITY),
        _ => None,
    }
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
/// The pointer that says a line between two windows can be dragged: up
/// and down for one that lies across the screen, side to side otherwise.
const fn line_icon(across: bool) -> CursorIcon {
    if across {
        CursorIcon::RowResize
    } else {
        CursorIcon::ColResize
    }
}

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
    Gesture(Gesture),
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
    /// Moving a frame. `grab` is where on the frame the pointer holds it,
    /// and `from` where the frame was before it was moved.
    Move {
        window: WindowId,
        grab: (i32, i32),
        size: DomainSize,
        from: DomainPoint,
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
    /// Carrying a window that fills a part of a workspace to another part,
    /// to change places with what is there.
    Tile { window: WindowId },
    /// Moving a line between windows that share a workspace: which
    /// output's and which of its lines, which lies `across` or runs down,
    /// and where it is `wanted` while it waits for the windows to catch up.
    Line {
        line: (usize, usize),
        across: bool,
        wanted: Option<DomainPoint>,
    },
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
    /// Whether it fills a part of a workspace it shares with others.
    shares: bool,
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
        // And the question before leaving counts its wait again from now
        // (ADR 112): only a machine walked away from goes on by itself.
        self.cards.leave.stirred();
        if self.withholding.holding() && !self.is_locked() && self.sealed().is_none() {
            // Held back after a prompt that went unanswered: a key reaches
            // nothing, and a press lets the keyboard go where it would.
            match event {
                Input::Key { code, pressed } => {
                    self.withholding.touched(self.clock());
                    self.withheld_key(code, pressed, time);
                    return;
                }
                Input::Button { pressed: true, .. } => self.withholding.release(),
                _ => {}
            }
        }
        if self.is_locked() || self.sealed().is_some() {
            // A locked session sees none of what follows, nor one a prompt
            // asks a secret over.
            let event = self.place(event, time);
            self.guarded_input(event, time);
            return;
        }
        // The program under the pointer is told how far it moved, and one
        // that holds it keeps it where it lets it be.
        if let Input::MotionBy {
            dx,
            dy,
            raw_dx,
            raw_dy,
        } = event
        {
            self.moved_relatively((dx, dy), (raw_dx, raw_dy), time);
            match self.held() {
                Some(Held::Locked) => return,
                Some(Held::Confined(bounds)) => {
                    let to = bounds.slide(self.pointer_location, (dx, dy));
                    self.motion(to.x, to.y, time);
                    return;
                }
                None => {}
            }
        }
        match self.place(event, time) {
            Placed::Key { code, pressed } => self.key(code, pressed, time),
            Placed::Motion { x, y } => self.motion(x, y, time),
            Placed::Button { button, pressed } => self.button(button, pressed, time),
            // "About" has the pointer while it is shown, and is scrolled
            // by it.
            Placed::Scroll { vertical, .. } if self.cards.about.asked.is_some() => {
                let _ = self
                    .cards
                    .about
                    .shown
                    .as_mut()
                    .map(|it| it.scroll(vertical));
            }
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
            Placed::Gesture(gesture) => self.gesture(gesture, time),
        }
    }

    /// What an event comes to on the screens: a distance moved becomes the
    /// position arrived at.
    fn place(&mut self, event: Input, time: u32) -> Placed {
        match event {
            Input::Key { code, pressed } => Placed::Key { code, pressed },
            Input::Motion { x, y } => Placed::Motion { x, y },
            Input::MotionBy { dx, dy, .. } => {
                // What of the move did not happen, the pointer being held at
                // an edge it was against already, is a push against it. A
                // move that only reaches the edge, however fast, is not:
                // that is a pointer on its way to what is there.
                let held = self.held_at();
                let to = self.moved_by(dx, dy);
                let past = self.pointer_location.y + dy - to.y;
                self.hover.reveal.push(past, held, time);
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
            Input::Gesture(gesture) => Placed::Gesture(gesture),
        }
    }

    /// Whether a chord with Super that nothing binds reaches the program
    /// with the keyboard as Ctrl (ADR 78): unless the configuration says
    /// not, or the bindings are the ordinary way's, where it is the
    /// program's as it is, or the program is one of the shell's with fields
    /// of its own, which read Super as a Mac's read Command (ADR 101).
    fn super_as_ctrl(&self) -> bool {
        let config = self.config();
        let way = config.general.keyboard.super_as_ctrl && !config.keys.ordinary();
        way && !self.shell_focused()
    }

    /// Whether `code`, pressed with `held`, is one of the launcher's modes,
    /// Super with a digit, while it has the keyboard (ADR 109), rather than
    /// a workspace's: by the key's place on the number row, as the
    /// launcher reads it. Super alone, so that a chord with Shift or Ctrl
    /// beside it, as the pictures of the screen are, is still its
    /// binding's; and in the Mac's way, as in the ordinary way the modes
    /// are on Ctrl, which no binding takes.
    fn launchers_mode_key(&self, held: &ModifiersState, code: Keycode) -> bool {
        let row = (10..=19).contains(&code.raw());
        let alone = bridge::modifiers(held) == Modifiers::SUPER;
        let ordinary = self.config().keys.ordinary();
        alone && row && !ordinary && self.overlay_focused()
    }

    /// The surface that has the keyboard, if one does that is not an X11
    /// window's.
    fn focused_surface(&self) -> Option<WlSurface> {
        match self.keyboard.current_focus() {
            Some(Focus::Surface(surface)) => Some(surface),
            _ => None,
        }
    }

    /// Whether what has the keyboard is one of the shell's surfaces over
    /// everything, as the launcher's: not a window, and of a program the
    /// compositor started with the role for it.
    fn overlay_focused(&self) -> bool {
        let surface = self.focused_surface();
        let window = |surface: &WlSurface| self.scene.iter().any(|shown| shown.surface == *surface);
        let surface = surface.filter(|surface| !window(surface));
        let roles = Self::roles(surface.and_then(|surface| surface.client()).as_ref());
        roles.has(Roles::OVERLAY)
    }

    /// Whether what has the keyboard is one of the shell's programs with
    /// fields: the launcher, the boxes that ask for a secret and the lock
    /// screen, by the roles the compositor started them with.
    fn shell_focused(&self) -> bool {
        let surface = self.focused_surface();
        let roles = Self::roles(surface.and_then(|surface| surface.client()).as_ref());
        [Roles::OVERLAY, Roles::ASK, Roles::LOCK]
            .into_iter()
            .any(|role| roles.has(role))
    }

    /// Takes Escape, if it is the desktop's: closing an open menu, or with
    /// the windows set aside, none of which has the keyboard, bringing them
    /// back, but for a panel that has it.
    fn escape(&mut self) -> bool {
        if self.menu.take().is_some() {
            return true;
        }
        let aside = self.desktop.showing_desktop() && self.layer_focus.is_none();
        if aside {
            self.desktop.show_desktop();
        }
        aside
    }

    /// Takes a key while the keyboard is held back: the keyboard is told
    /// it went down or up, so that no key is taken to be held once a window
    /// has it again, and nothing sees it, no binding and no program.
    fn withheld_key(&mut self, code: Keycode, pressed: bool, time: u32) {
        let keyboard = self.keyboard.clone();
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        let serial = SERIAL_COUNTER.next_serial();
        keyboard.input(self, code, state, serial, time, |_, _, _| {
            FilterResult::Intercept(())
        });
    }

    fn key(&mut self, code: Keycode, pressed: bool, time: u32) {
        let keyboard = self.keyboard.clone();
        let state = if pressed {
            KeyState::Pressed
        } else {
            KeyState::Released
        };
        let serial = self.key_serial(pressed);
        let action = keyboard.input(self, code, state, serial, time, |compositor, held, key| {
            if !pressed {
                // A question is answered as a key is let go (ADR 111).
                let keysym = key
                    .raw_latin_sym_or_raw_current_sym()
                    .unwrap_or(key.modified_sym());
                compositor.leaving_key(code.raw(), keysym, false);
                compositor.screens_key(keysym, false);
                return compositor.let_go(code, held);
            }
            if let Some(terminal) = virtual_terminal(key.modified_sym()) {
                // Not ours to act on: the backend that holds the seat is
                // asked, and no client or binding sees the key.
                compositor.terminal_wanted = Some(terminal);
                compositor.taken.suppressed.push(code);
                return FilterResult::Intercept(None);
            }
            let keysym = key
                .raw_latin_sym_or_raw_current_sym()
                .unwrap_or(key.modified_sym());
            // A key sends away what was brought back over a fullscreen
            // window, and is the window's all the same.
            compositor.hover.reveal.dismiss();
            compositor.hover.palette.dismiss();
            // Whether to leave is answered by Enter, space, Escape, Tab and
            // the arrows; the screens tried another way are kept by Enter
            // and put back by Escape. Nothing else then sees them.
            if compositor.leaving_key(code.raw(), keysym, true)
                || compositor.screens_key(keysym, true)
            {
                compositor.taken.suppressed.push(code);
                return FilterResult::Intercept(None);
            }
            // An open switcher is steered by Escape, Enter and the arrows,
            // and so is the overview, by them alone: with a modifier held
            // they are a binding's, as Super+Ctrl+Up closes it (#155).
            let plain = bridge::modifiers(held) == Modifiers::NONE;
            if compositor.switcher_key(keysym) || (plain && compositor.overview_key(keysym)) {
                compositor.taken.suppressed.push(code);
                return FilterResult::Intercept(None);
            }
            if keysym == Keysym::Escape && compositor.escape() {
                compositor.taken.suppressed.push(code);
                return FilterResult::Intercept(None);
            }
            // "About" has the keyboard while it is shown: Escape puts it
            // away, Enter copies what it says, and the arrows, the keys of
            // a page, space, Home and End scroll it. Nothing else of it
            // reaches a window; a binding still does what it is bound to,
            // so a key pressed with a modifier held is the binding's.
            let about = compositor.cards.about.asked.is_some();
            if about && plain && compositor.about_key(keysym) {
                compositor.taken.suppressed.push(code);
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
                compositor.taken.suppressed.push(code);
                return FilterResult::Intercept(chosen);
            }
            // A program given every key has them while it has the keyboard,
            // but for the one that takes them back.
            if let Some(taken) = compositor.every_key(held, keysym, code) {
                return taken;
            }
            if keysym.is_modifier_key() {
                // Holding a modifier on the way to the next chord is not a
                // step in a chain.
                if !about {
                    return FilterResult::Forward;
                }
                compositor.taken.suppressed.push(code);
                return FilterResult::Intercept(None);
            }
            let chord = Chord {
                modifiers: bridge::modifiers(held),
                keysym: keysym.raw(),
            };
            if compositor.launchers_mode_key(held, code) {
                return FilterResult::Forward;
            }
            // Super is the desktop's: a chord with it that nothing binds
            // is the program's as Ctrl (ADR 78), unless it is not to be.
            let as_ctrl = held.logo && compositor.super_as_ctrl();
            let action = match compositor.bindings.press(chord) {
                KeyOutcome::Unbound if !about && as_ctrl => {
                    return compositor.take_to_hand_on(code, keysym);
                }
                KeyOutcome::Unbound if !about => return FilterResult::Forward,
                KeyOutcome::Unbound | KeyOutcome::Pending | KeyOutcome::Cancelled => None,
                KeyOutcome::Triggered(action) => Some(action.clone()),
            };
            compositor.taken.suppressed.push(code);
            // The switcher a binding opens stays open while they are held.
            compositor.switch.held = crate::switcher::held_with(held);
            FilterResult::Intercept(action)
        });
        if let Some((handed, pressed)) = self.taken.handing.take() {
            self.hand_on_as_ctrl(handed, pressed, serial, time);
        }
        if let Some(action) = action.flatten() {
            self.perform(&action);
        }
        self.switch.held = Modifiers::NONE;
    }

    /// What becomes of `code`, pressed with `held` as `keysym`, if what
    /// has the keyboard was given every key (ADR 78): `Super+Escape` takes
    /// them back from it, until it asks again, and anything else is its
    /// own. Nothing, if it was given none.
    fn every_key(
        &mut self,
        held: &ModifiersState,
        keysym: Keysym,
        code: Keycode,
    ) -> Option<FilterResult<Option<Action>>> {
        let surface = match self.keyboard.current_focus()? {
            Focus::Surface(surface) => Some(surface),
            Focus::X11(window) => window.wl_surface(),
        };
        let given = surface
            .and_then(|surface| self.seat.keyboard_shortcuts_inhibitor_for_surface(&surface));
        let given = given.filter(KeyboardShortcutsInhibitor::is_active)?;
        if held.logo && keysym == Keysym::Escape {
            given.inactivate();
            self.taken.suppressed.push(code);
            return Some(FilterResult::Intercept(None));
        }
        Some(FilterResult::Forward)
    }

    /// Takes `code`, pressed with Super as `keysym`, to go to the program as
    /// something else once the keyboard has done with it, and its release
    /// after it.
    fn take_to_hand_on(&mut self, code: Keycode, keysym: Keysym) -> FilterResult<Option<Action>> {
        let handed = Handed::of(code, keysym);
        self.taken.as_ctrl.push((code, handed));
        self.taken.handing = Some((handed, true));
        FilterResult::Intercept(None)
    }

    /// What becomes of `code` let go of with `held`: the switcher it was
    /// held open by chooses, a key that went to the program as Ctrl is let
    /// go of so, and the release of a key whose press was swallowed is
    /// swallowed too.
    fn let_go(&mut self, code: Keycode, held: &ModifiersState) -> FilterResult<Option<Action>> {
        self.switcher_let_go(held);
        let handed = self
            .taken
            .as_ctrl
            .iter()
            .position(|(held, _)| *held == code);
        if let Some(index) = handed {
            let (_, handed) = self.taken.as_ctrl.remove(index);
            self.taken.handing = Some((handed, false));
            return FilterResult::Intercept(None);
        }
        let before = self.taken.suppressed.len();
        self.taken
            .suppressed
            .retain(|suppressed| *suppressed != code);
        if self.taken.suppressed.len() < before {
            FilterResult::Intercept(None)
        } else {
            FilterResult::Forward
        }
    }

    /// Hands the program with the keyboard a key pressed or let go of with
    /// Super held, as `handed` says: as though Ctrl were held in Super's
    /// place, and Shift with it in a terminal, where Ctrl alone is the
    /// shell's (ADR 78); or an arrow as Home or End. The program is told
    /// the modifiers so for that key alone, and told them as they are once
    /// it is let go of.
    fn hand_on_as_ctrl(&mut self, handed: Handed, pressed: bool, serial: Serial, time: u32) {
        let code = handed.code;
        let keyboard = self.keyboard.clone();
        let focus = keyboard.current_focus();
        let held = keyboard.modifier_state();
        if pressed {
            let mut told = held;
            told.logo = false;
            told.ctrl |= handed.ctrl;
            told.shift |= !handed.arrow && self.terminal_focused();
            // What is sent of them is made from the keymap: they are set
            // so, read back, and set as they were before anything else.
            keyboard.set_modifier_state(told);
            let told = keyboard.modifier_state();
            keyboard.set_modifier_state(held);
            self.tell_modifiers(focus.as_ref(), told, serial);
            keyboard.input_forward(self, code, KeyState::Pressed, serial, time, false);
        } else {
            keyboard.input_forward(self, code, KeyState::Released, serial, time, false);
            self.tell_modifiers(focus.as_ref(), held, serial);
        }
    }

    /// Tells `focus`, if anything has the keyboard, that `modifiers` are
    /// held.
    fn tell_modifiers(&mut self, focus: Option<&Focus>, modifiers: ModifiersState, serial: Serial) {
        if let Some(focus) = focus {
            let seat = self.seat.clone();
            focus.modifiers(&seat, self, modifiers, serial);
        }
    }

    /// Whether the window with focus is a terminal, as `keyboard.terminals`
    /// says.
    fn terminal_focused(&self) -> bool {
        let window = self
            .desktop
            .focused()
            .and_then(|id| self.desktop.window(id));
        let app_id = window.map_or("", |window| window.app_id.as_str());
        let terminals = &self.config().general.keyboard.terminals;
        terminals
            .iter()
            .any(|terminal| Pattern::new(terminal.as_str()).matches(app_id))
    }

    fn motion(&mut self, x: f64, y: f64, time: u32) {
        self.pointer_location = (x, y).into();
        // Whatever else has the pointer, no button of a titlebar has.
        self.hover.button = None;
        let position = self.pointer_position();
        if let Some(drag) = self.drag {
            match drag {
                Drag::Move {
                    window, grab, size, ..
                } => {
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
                Drag::Tab { .. } | Drag::Tile { .. } => {}
                Drag::Line { line, across, .. } => {
                    let wanted = Some(position);
                    self.drag = Some(Drag::Line {
                        line,
                        across,
                        wanted,
                    });
                    self.catch_line_up();
                }
            }
            // The drag has the pointer; no window sees it meanwhile.
            self.move_pointer(None, time);
            return;
        }

        if self.over_notice()
            || self.cards.about.asked.is_some()
            || self.cards.leave.asked.is_some()
        {
            // The notice is ours, and "About" has the pointer while it is
            // shown, like the menu, as the question whether to leave has;
            // nothing underneath sees it.
            self.move_pointer(None, time);
            return;
        }
        if self.overview_motion(position) {
            // So has the overview, which picks the window under it.
            self.move_pointer(None, time);
            return;
        }
        if let Some(menu) = &mut self.menu {
            // An open menu has the pointer, like a drag.
            menu.hover(position);
            self.move_pointer(None, time);
            return;
        }
        if self.over_line().is_some() || self.over_palette() || self.over_offer().is_some() {
            // The line between two windows is ours, and neither window's;
            // and so are the pictures of arrangements and of the windows
            // offered in a part with nothing in it.
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

    pub(crate) fn move_pointer(&mut self, focus: Focused, time: u32) {
        // Over another surface, the pointer looks as it ordinarily does
        // until that surface's program says otherwise: what the last one
        // asked for is not this one's.
        let surface = |pointed: &Focused| pointed.as_ref().map(|(surface, _)| surface.clone());
        if surface(&focus) != surface(&self.hover.pointed.0) {
            self.cursor = CursorImageStatus::default_named();
        }
        self.hover.pointed = (focus.clone(), time);
        let pointer = self.pointer.clone();
        let event = MotionEvent {
            location: self.pointer_location,
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        pointer.motion(self, focus, &event);
        pointer.frame(self);
        // Over a program that asked to hold it, the pointer may now be held.
        self.hold();
    }

    /// Tells the program under a pointer that has not moved that it is,
    /// when something has come or gone or moved under it: as moving the
    /// pointer would, and only then.
    /// Nothing changes while the session is locked: the locker has the
    /// pointer, and what is behind it is not there to be pointed at.
    pub(crate) fn repoint(&mut self) {
        if self.is_locked() {
            return;
        }
        // While a prompt asks for a secret, it is all there is to point at.
        if self.sealed().is_some() {
            let time = self.hover.pointed.1;
            self.point_at_sealed(time);
            return;
        }
        let ours = self.drag.is_some()
            || self.overview_open()
            || self.menu.is_some()
            || self.over_notice()
            || self.cards.about.asked.is_some()
            || self.cards.leave.asked.is_some()
            || self.over_line().is_some()
            || self.over_palette()
            || self.over_offer().is_some();
        let under = self.under();
        let focus = under.as_ref().and_then(Under::focus).filter(|_| !ours);
        let (pointed, time) = &self.hover.pointed;
        if focus != *pointed {
            self.move_pointer(focus, *time);
        }
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
            Some(Drag::Move { .. } | Drag::Tab { .. } | Drag::Tile { .. }) => {
                return Look::Icon(CursorIcon::Grabbing);
            }
            Some(Drag::Resize { edges, .. }) => return Look::Icon(resize_icon(edges)),
            Some(Drag::Line { across, .. }) => return Look::Icon(line_icon(across)),
            None => {}
        }
        let about = self.cards.about.asked.is_some();
        // The buttons of "About" are pointed at with a hand.
        let shown = self.cards.about.shown.as_ref().filter(|_| about);
        let position = self.pointer_position();
        if shown.is_some_and(|about| about.copies_at(position) || about.closes_at(position)) {
            return Look::Icon(CursorIcon::Pointer);
        }
        // So are those of the question whether to leave.
        let asking = self
            .cards
            .leave
            .shown
            .as_ref()
            .filter(|_| self.cards.leave.asked.is_some());
        if asking
            .and_then(|asking| asking.choice_at(position))
            .is_some()
        {
            return Look::Icon(CursorIcon::Pointer);
        }
        if self.over_notice() || about || self.menu.is_some() || self.over_palette() {
            return Look::Icon(CursorIcon::Default);
        }
        if let Some((.., across)) = self.over_line() {
            return Look::Icon(line_icon(across));
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

    /// What a key does to "About" while it is shown, and whether it did
    /// anything: Escape puts it away, Enter presses its one button, which
    /// copies what the computer is, and the keys `about_scroll` knows
    /// scroll it.
    fn about_key(&mut self, keysym: Keysym) -> bool {
        match keysym {
            Keysym::Escape => {
                self.cards.about.asked = None;
                return true;
            }
            Keysym::Return | Keysym::KP_Enter => {
                self.copy_about();
                return true;
            }
            _ => {}
        }
        let shown = self.cards.about.shown.as_mut();
        let page = shown.as_deref().map_or(0.0, About::page);
        let by = about_scroll(keysym, page);
        let _ = shown.zip(by).map(|(about, by)| about.scroll(by));
        by.is_some()
    }

    /// A press while "About" is shown: beside it, or on its button that
    /// closes it, it puts it away; on its button that copies, it copies
    /// what the computer is; anywhere else on it does nothing. None is any
    /// window's.
    fn press_about(&mut self, position: DomainPoint) {
        let about = self.cards.about.shown.as_ref();
        if about.is_some_and(|about| about.closes_at(position)) {
            self.cards.about.asked = None;
        } else if about.is_some_and(|about| about.copies_at(position)) {
            self.copy_about();
        } else if !self.over_about() {
            self.cards.about.asked = None;
        }
    }

    /// Whether the pointer is on "About".
    fn over_about(&self) -> bool {
        let position = self.pointer_position();
        let about = self.cards.about.shown.as_ref();
        about.is_some_and(|about| about.area.contains(position))
    }

    fn over_notice(&self) -> bool {
        let position = self.pointer_position();
        self.cards
            .notice
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
            let origin = crate::compositor::layer_origin(&layers, surface) + corner;
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

    /// Takes a press beside what demands the keyboard, where its program
    /// has laid a surface that takes no pointer over the rest, as the
    /// launcher has its backdrop: a way of saying never mind, which closes
    /// what that program shows; and takes the release after it. Says
    /// whether it took either: nothing behind sees them. What is behind is
    /// pointed at and scrolled all the same, which a program that took
    /// the pointer there could not pass on.
    fn press_beside(&mut self, pressed: bool) -> bool {
        if !pressed {
            return std::mem::take(&mut self.hover.beside);
        }
        let demanding = self.demanding().and_then(|surface| surface.client());
        let Some(demanding) = demanding.map(|client| client.id()) else {
            return false;
        };
        let theirs = |surface: &WlSurface| {
            let client = surface.client().map(|client| client.id());
            client.as_ref() == Some(&demanding)
        };
        let under = self.under().and_then(|under| under.focus());
        if under.is_some_and(|(surface, _)| theirs(&surface)) {
            return false;
        }
        let point = self.pointer_location;
        let mut shown = Vec::new();
        let mut over = false;
        for head in &self.heads {
            let layers = layer_map_for_output(&head.output);
            let corner = head.area().loc.to_f64();
            for layer in layers.layers().filter(|layer| theirs(layer.wl_surface())) {
                let area = layers.layer_geometry(layer).unwrap_or_default();
                over |= area.to_f64().contains(point - corner);
                shown.push(layer.clone());
            }
        }
        if over {
            for layer in shown {
                layer.layer_surface().send_close();
            }
        }
        self.hover.beside = over;
        over
    }

    /// Takes a press or a release that is for something of ours laid over
    /// the windows, the pictures of arrangements or a line between two
    /// windows, and says whether it was: nothing under them sees it.
    fn press_ours(&mut self, button: u32, pressed: bool, time: u32) -> bool {
        if self.over_palette() {
            // A press on a part of a picture chooses it; anywhere else on
            // them it only takes them away.
            if pressed {
                self.choose_arrangement();
                self.move_pointer(None, time);
            }
            return true;
        }
        if let Some(window) = self.over_offer() {
            // A press on a window offered in a part with nothing in it puts
            // it there.
            if pressed && button == BUTTON_LEFT {
                self.choose_offered(window);
                self.move_pointer(None, time);
            }
            return true;
        }
        let Some((output, which, across)) = self.over_line() else {
            return false;
        };
        // The line between two windows is taken hold of and dragged.
        // Pressed twice in quick succession, it goes back to the middle.
        if pressed && button == BUTTON_LEFT {
            let before = self.hover.lines.pressed.replace(time);
            let twice = before.is_some_and(|at| time.wrapping_sub(at) <= DOUBLE_CLICK);
            if twice {
                self.hover.lines.pressed = None;
                self.move_line((output, which), None);
            } else {
                let line = (output, which);
                self.drag = Some(Drag::Line {
                    line,
                    across,
                    wanted: None,
                });
            }
            self.move_pointer(None, time);
        }
        true
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
                let surface_origin = shown.origin - surfaces::proper(&shown.window).loc;
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
                // A frame let go against an edge of the screen is put in
                // the part of a workspace that was shown for it.
                self.drop_at_edge();
                self.let_go_line(drag);
                self.drag = None;
                self.drop_tab(drag);
            }
            return;
        }
        let position = self.pointer_position();
        if self.switcher_open() || self.overview_open() {
            // The switcher has the pointer while it is open, as the
            // overview has; each takes a press only while it is.
            if pressed {
                self.switcher_press(position);
                self.overview_press(position);
            }
            return;
        }
        if self.leaving_press(pressed, position) {
            return;
        }
        if self.cards.about.asked.is_some() {
            // "About" has the pointer while it is shown.
            if pressed {
                self.press_about(position);
            }
            return;
        }
        if self.over_notice() {
            // A click on the notice dismisses it, until what it says changes.
            if pressed {
                self.cards.dismissed = self.said().into_owned();
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
        if self.press_ours(button, pressed, time) || self.press_beside(pressed) {
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
            // What came or went under a pointer that has not moved since,
            // as a launcher does when it opens, has been told already.
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
            // A press that begins a drag or opens the window's menu is
            // ours, as the root menu's is: had the window been told of it,
            // its release, which the menu takes, would never be, and the
            // pointer would stay held by where it was pressed.
            if self.drag.is_some() || self.menu.is_some() {
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
            serial: self.button_serial(pressed),
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
                    from: frame.outer.origin,
                })
            }
            Wanted::Resize(edges) if frame.movable => Some(Drag::Resize {
                window,
                edges,
                initial: frame.outer,
                start: position,
            }),
            // One that fills a part of a workspace is carried to another
            // part, by the titlebar that was brought back over it.
            Wanted::Move if frame.shares => Some(Drag::Tile { window }),
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
                shares: frame.fullscreen && frame.home == Home::Shared,
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

/// A program may have every key, those the desktop keeps among them, while
/// its window has the keyboard, as a virtual machine or a remote desktop
/// asks to (ADR 78). It is given them at once, and the user told so, and
/// how to have them back.
impl KeyboardShortcutsInhibitHandler for Compositor {
    fn keyboard_shortcuts_inhibit_state(&mut self) -> &mut KeyboardShortcutsInhibitState {
        &mut self.protocols.shortcuts
    }

    fn new_inhibitor(&mut self, inhibitor: KeyboardShortcutsInhibitor) {
        inhibitor.activate();
        let window = self.window_id(inhibitor.wl_surface());
        let window = window.and_then(|id| self.desktop.window(id));
        let unnamed = self.words.say("a-program", &[]);
        let program = window.map_or(unnamed, |window| window.app_id.clone());
        let said = self
            .words
            .say("every-key", &[("program", Value::Words(&program))]);
        self.refused.say(&said);
    }
}

delegate_keyboard_shortcuts_inhibit!(Compositor);
