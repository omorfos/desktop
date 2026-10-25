//! Gestures on a touchpad: three fingers swiped across it go from one
//! workspace to the next, and every other gesture is the program's under
//! the pointer (`pointer-gestures`), for a pinch to zoom a page or a map.
//!
//! A swipe of three fingers is taken from its start, before anything says
//! which way it goes, as no program can be given part of one: it is
//! ours, whichever way it goes, and comes to something only once the
//! fingers lift, if it went mostly across and far enough.

use smithay::delegate_pointer_gestures;
use smithay::input::pointer::{
    GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
    GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::pointer_gestures::PointerGesturesState;

use omorfos_config::Action;

use crate::compositor::Compositor;
use crate::input::Gesture;

/// How many fingers swipe from one workspace to the next.
const FINGERS: u32 = 3;

/// How far across, in units of layout, the fingers have to go for a swipe
/// to come to something.
const ACROSS: f64 = 100.0;

/// What the compositor keeps of gestures.
#[derive(Debug)]
pub(crate) struct Gestures {
    /// How far the fingers of a swipe that is ours have gone, while it
    /// lasts.
    swiping: Option<Point<f64, Logical>>,
    _protocol: PointerGesturesState,
}

impl Gestures {
    /// Offers the protocol on `display`.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        Self {
            swiping: None,
            _protocol: PointerGesturesState::new::<Compositor>(display),
        }
    }
}

impl Compositor {
    /// Takes a gesture that happened at `time`, in milliseconds: a swipe
    /// of three fingers, or passes it on.
    pub(crate) fn gesture(&mut self, gesture: Gesture, time: u32) {
        let pointer = self.pointer.clone();
        let serial = SERIAL_COUNTER.next_serial();
        match gesture {
            Gesture::SwipeBegin { fingers: FINGERS } => {
                self.protocols.gestures.swiping = Some(Point::default());
            }
            Gesture::SwipeBegin { fingers } => {
                let event = GestureSwipeBeginEvent {
                    serial,
                    time,
                    fingers,
                };
                pointer.gesture_swipe_begin(self, &event);
            }
            Gesture::SwipeUpdate { dx, dy } => {
                if let Some(moved) = &mut self.protocols.gestures.swiping {
                    *moved += Point::from((dx, dy));
                } else {
                    let delta = (dx, dy).into();
                    pointer.gesture_swipe_update(self, &GestureSwipeUpdateEvent { time, delta });
                }
            }
            Gesture::SwipeEnd { cancelled } => {
                if let Some(moved) = self.protocols.gestures.swiping.take() {
                    self.swiped(moved, cancelled);
                } else {
                    let event = GestureSwipeEndEvent {
                        serial,
                        time,
                        cancelled,
                    };
                    pointer.gesture_swipe_end(self, &event);
                }
            }
            Gesture::PinchBegin { fingers } => {
                let event = GesturePinchBeginEvent {
                    serial,
                    time,
                    fingers,
                };
                pointer.gesture_pinch_begin(self, &event);
            }
            Gesture::PinchUpdate {
                dx,
                dy,
                scale,
                rotation,
            } => {
                let event = GesturePinchUpdateEvent {
                    time,
                    delta: (dx, dy).into(),
                    scale,
                    rotation,
                };
                pointer.gesture_pinch_update(self, &event);
            }
            Gesture::PinchEnd { cancelled } => {
                let event = GesturePinchEndEvent {
                    serial,
                    time,
                    cancelled,
                };
                pointer.gesture_pinch_end(self, &event);
            }
            Gesture::HoldBegin { fingers } => {
                let event = GestureHoldBeginEvent {
                    serial,
                    time,
                    fingers,
                };
                pointer.gesture_hold_begin(self, &event);
            }
            Gesture::HoldEnd { cancelled } => {
                let event = GestureHoldEndEvent {
                    serial,
                    time,
                    cancelled,
                };
                pointer.gesture_hold_end(self, &event);
            }
        }
    }

    /// What a swipe of three fingers comes to once they lift, having gone
    /// `moved`: fingers that went left bring the next workspace in from
    /// the right, as a page turned, and those that went right the one
    /// before. One that went more up or down than across, not far enough,
    /// or that was cancelled, comes to nothing.
    fn swiped(&mut self, moved: Point<f64, Logical>, cancelled: bool) {
        let across = moved.x.abs() >= ACROSS && moved.x.abs() > moved.y.abs();
        if cancelled || !across {
            return;
        }
        let action = if moved.x < 0.0 {
            Action::NextWorkspace
        } else {
            Action::PreviousWorkspace
        };
        self.perform(&action);
    }
}

delegate_pointer_gestures!(Compositor);
