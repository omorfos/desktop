//! Input from the user.

use omorfos_core::geometry::Point as DomainPoint;
use smithay::backend::input::{Axis, ButtonState, KeyState};
use smithay::desktop::WindowSurfaceType;
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;

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
        let under = self
            .space
            .element_under(self.pointer_location)
            .map(|(window, origin)| {
                let surface = window
                    .surface_under(
                        self.pointer_location - origin.to_f64(),
                        WindowSurfaceType::ALL,
                    )
                    .map(|(surface, offset)| (surface, (offset + origin).to_f64()));
                (window.clone(), surface)
            });
        let hovered = under
            .as_ref()
            .and_then(|(window, _)| self.windows.iter().find(|mapped| mapped.window == *window))
            .map(|mapped| mapped.id);
        if hovered != self.hovered {
            self.hovered = hovered;
            if let Some(id) = hovered {
                self.desktop.pointer_entered(id);
            }
        }
        let focus = under.and_then(|(_, surface)| surface);
        let pointer = self.pointer.clone();
        let event = MotionEvent {
            location: self.pointer_location,
            serial: SERIAL_COUNTER.next_serial(),
            time,
        };
        pointer.motion(self, focus, &event);
        pointer.frame(self);
    }

    fn button(&mut self, button: u32, pressed: bool, time: u32) {
        if let (true, Some(id)) = (pressed, self.hovered) {
            self.desktop.clicked(id);
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
