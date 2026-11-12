//! Gestures on a touchpad: three fingers swiped across it go from one
//! workspace to the next, swiped up open the overview and down close it,
//! and every other gesture is the program's under the pointer
//! (`pointer-gestures`), for a pinch to zoom a page or a map.
//!
//! A swipe of three fingers is taken from its start, before anything says
//! which way it goes, as no program can be given part of one: it is ours,
//! whichever way it goes. Which way is told once the fingers have gone
//! [`DECIDED`] units: mostly across, they slide the workspaces with them
//! (see `slide.rs`); mostly up or down, they carry the overview open or
//! closed (see `overview.rs`); anything else they do is nothing.

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
/// How far three fingers go before which way they swipe is told, in the
/// units the touchpad tells.
const DECIDED: f64 = 8.0;

/// A swipe of three fingers, while they are on the touchpad.
#[derive(Debug, Clone, Copy)]
enum Swipe {
    /// Which way they go is not told yet: how far they have gone, across
    /// and down.
    Telling(f64, f64),
    /// It is told, and what they swipe is carried.
    Told,
}

/// What the compositor keeps of gestures.
#[derive(Debug)]
pub(crate) struct Gestures {
    /// The workspaces slid by a swipe that is ours.
    pub(crate) slide: Slide,
    /// A swipe of three fingers, while they are on the touchpad.
    swipe: Option<Swipe>,
    _protocol: PointerGesturesState,
}

impl Gestures {
    /// Offers the protocol on `display`.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        Self {
            slide: Slide::default(),
            swipe: None,
            _protocol: PointerGesturesState::new::<Compositor>(display),
        }
    }
}

impl Compositor {
    /// Takes a gesture that happened at `time`, in milliseconds: a swipe
    /// of three fingers, or passes it on.
    pub(crate) fn gesture(&mut self, gesture: Gesture, time: u32) {
        // The overview has the pointer, and the touchpad with it: but for
        // a swipe of three fingers, none is a program's.
        let swiping = self.protocols.gestures.swipe.is_some();
        let three = matches!(gesture, Gesture::SwipeBegin { fingers: FINGERS });
        if self.overview_open() && !swiping && !three {
            return;
        }
        let pointer = self.pointer.clone();
        let serial = SERIAL_COUNTER.next_serial();
        match gesture {
            Gesture::SwipeBegin { fingers: FINGERS } => {
                self.protocols.gestures.swipe = Some(Swipe::Telling(0.0, 0.0));
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
                if swiping {
                    self.swiped((dx, dy), time);
                } else {
                    let delta = (dx, dy).into();
                    pointer.gesture_swipe_update(self, &GestureSwipeUpdateEvent { time, delta });
                }
            }
            Gesture::SwipeEnd { cancelled } => {
                if self.protocols.gestures.swipe.take().is_some() {
                    if !self.overview_swipe_ended(cancelled) {
                        self.swipe_ended(cancelled);
                    }
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

    /// Takes three fingers going `dx` across and `dy` down at `time`:
    /// until which way they go is told, how far they have gone is kept;
    /// once it is, they carry what they swipe.
    fn swiped(&mut self, (dx, dy): (f64, f64), time: u32) {
        let Some(Swipe::Telling(across, down)) = self.protocols.gestures.swipe else {
            if !self.overview_swipe_moved(dy, time) {
                self.swipe_moved(dx, time);
            }
            return;
        };
        let (across, down) = (across + dx, down + dy);
        if across.hypot(down) < DECIDED {
            self.protocols.gestures.swipe = Some(Swipe::Telling(across, down));
            return;
        }
        self.protocols.gestures.swipe = Some(Swipe::Told);
        if down.abs() > across.abs() {
            // Up opens the overview, and down closes it.
            if self.overview_swipe_began(down < 0.0, time) {
                self.overview_swipe_moved(down, time);
            }
        } else if !self.overview_open() {
            self.swipe_began(time);
            self.swipe_moved(across, time);
        }
    }
}

delegate_pointer_gestures!(Compositor);
