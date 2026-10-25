//! Gestures on a touchpad: three fingers swiped across it go from one
//! workspace to the next, and every other gesture is the program's under
//! the pointer (`pointer-gestures`), for a pinch to zoom a page or a map.
//!
//! A swipe of three fingers is taken from its start, before anything says
//! which way it goes, as no program can be given part of one: it is
//! ours, whichever way it goes, and slides the workspaces across with the
//! fingers (see `slide.rs`).

use smithay::delegate_pointer_gestures;
use smithay::input::pointer::{
    GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
    GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::pointer_gestures::PointerGesturesState;

use crate::compositor::Compositor;
use crate::input::Gesture;
use crate::slide::Slide;

/// How many fingers swipe from one workspace to the next.
const FINGERS: u32 = 3;

/// What the compositor keeps of gestures.
#[derive(Debug)]
pub(crate) struct Gestures {
    /// The workspaces slid by a swipe that is ours.
    pub(crate) slide: Slide,
    _protocol: PointerGesturesState,
}

impl Gestures {
    /// Offers the protocol on `display`.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        Self {
            slide: Slide::default(),
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
            Gesture::SwipeBegin { fingers: FINGERS } => self.swipe_began(time),
            Gesture::SwipeBegin { fingers } => {
                let event = GestureSwipeBeginEvent {
                    serial,
                    time,
                    fingers,
                };
                pointer.gesture_swipe_begin(self, &event);
            }
            Gesture::SwipeUpdate { dx, dy } => {
                if !self.swipe_moved(dx, time) {
                    let delta = (dx, dy).into();
                    pointer.gesture_swipe_update(self, &GestureSwipeUpdateEvent { time, delta });
                }
            }
            Gesture::SwipeEnd { cancelled } => {
                if !self.swipe_ended(cancelled) {
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
}

delegate_pointer_gestures!(Compositor);
