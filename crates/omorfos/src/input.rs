//! Input from the user.

use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_core::{Edges, Part, WindowId, resize};
use smithay::backend::input::{Axis, ButtonState, KeyState};
use smithay::desktop::WindowSurfaceType;
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use crate::bridge::{self, Chord};
use crate::compositor::Compositor;
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
    /// The pointer moved to a position on the output, in pixels.
    Motion {
        /// Distance from the left edge.
        x: f64,
        /// Distance from the top edge.
        y: f64,
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

/// Linux input event codes for the pointer buttons.
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
    Nothing,
}

/// What the pointer is on.
enum Hit {
    /// Part of a frame's decoration.
    Part(Part),
    /// A client surface, and where that surface is on the output.
    Surface(WlSurface, Point<f64, Logical>),
}

#[derive(Default)]
struct FrameFacts {
    outer: Rect,
    movable: bool,
    tabs: Vec<WindowId>,
}

impl FrameFacts {
    /// The corner of the frame nearest `point`, as the two edges meeting
    /// there.
    fn nearest_corner(&self, point: DomainPoint) -> Edges {
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
        match event {
            Input::Key { code, pressed } => self.key(code, pressed, time),
            Input::Motion { x, y } => self.motion(x, y, time),
            Input::Button { button, pressed } => self.button(button, pressed, time),
            Input::Scroll {
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
            let keysym = key
                .raw_latin_sym_or_raw_current_sym()
                .unwrap_or(key.modified_sym());
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

        let target = self.target();
        let hovered = target.as_ref().map(|(id, _)| *id);
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Some(id) = hovered {
                self.desktop.pointer_entered(id);
            }
        }
        let focus = target.and_then(|(_, hit)| match hit {
            Hit::Surface(surface, origin) => Some((surface, origin)),
            Hit::Part(_) => None,
        });
        self.move_pointer(focus, time);
    }

    fn move_pointer(&mut self, focus: Option<(WlSurface, Point<f64, Logical>)>, time: u32) {
        let pointer = self.pointer.clone();
        let event = MotionEvent {
            location: self.pointer_location,
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        pointer.motion(self, focus, &event);
        pointer.frame(self);
    }

    /// What is under the pointer: the topmost window whose decoration or
    /// surface is there, and which of the two.
    fn target(&self) -> Option<(WindowId, Hit)> {
        let point = self.pointer_position();
        self.scene.iter().rev().find_map(|shown| {
            let part = shown
                .decoration()
                .and_then(|decoration| decoration.hit(point))
                .filter(|part| *part != Part::Client);
            let hit = part.map(Hit::Part).or_else(|| {
                let within = self.pointer_location - shown.origin.to_f64();
                shown
                    .window
                    .surface_under(within, WindowSurfaceType::ALL)
                    .map(|(surface, offset)| {
                        Hit::Surface(surface, (offset + shown.origin).to_f64())
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
        if let (true, Some((id, hit))) = (pressed, self.target()) {
            let frame = self.frame_facts(id);
            let tab = |index: &usize| frame.tabs.get(*index).copied().unwrap_or(id);
            let alt = self.keyboard.modifier_state().alt;
            let (clicked, wanted) = match (&hit, button, alt) {
                (Hit::Part(Part::Tab(index)), BUTTON_LEFT, _) => (tab(index), Wanted::Move),
                (Hit::Part(Part::Tab(index)), BUTTON_MIDDLE, _) => (tab(index), Wanted::Tab),
                (Hit::Part(Part::Tab(index)), ..) => (tab(index), Wanted::Nothing),
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
    fn frame_facts(&self, window: WindowId) -> FrameFacts {
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
