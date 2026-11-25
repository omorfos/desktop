//! The windows set aside to show the desktop by four fingers on a touchpad,
//! spread apart, and brought back by them pinched together, as a Mac has
//! it: the windows go with the fingers, toward the sides of the screen as
//! they spread and back as they close, with no easing at all, so that the
//! fingers can take them a little way and back again before deciding. Once
//! they lift, the windows go on or back as `fingers` decides for every
//! gesture, the rest of the way taking the time a window takes to go where
//! it was sent, as when Escape brings them back.
//!
//! Pinched together with nothing set aside, nothing moves: that is to
//! show the launcher's every program, once there is that.
//!
//! A workspace left with its windows aside, by fingers beginning to swipe
//! to another or by a key, has them back over the same time as it begins
//! to slide away, so that the two go together; nothing of it is drawn over
//! the workspace that comes in beside it.
//!
//! With `appearance.reduce_motion` the fingers carry nothing, and the
//! windows are set aside or brought back as they lift, at once.

use std::time::Duration;

use omorfos_config::Easing;

use crate::compositor::Compositor;
use crate::fingers::Fingers;

/// How much farther apart the fingers end than they began, for the whole
/// way aside; as much nearer together for the whole way back.
const WHOLE: f64 = 1.5;

/// How long the windows set aside on a workspace being left take to come
/// back, as long as a window takes to go where it was sent.
const BACK: Duration = Duration::from_millis(240);

/// Fingers carrying the windows aside or back.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Spread {
    /// Whether the windows were aside as they began.
    aside: bool,
    /// How far aside the fingers have taken them, out of one.
    reached: f64,
    fingers: Fingers,
}

impl Spread {
    /// How far aside fingers that began with the windows `aside` take them,
    /// as far apart as they are `scale` times what they began.
    fn reach(aside: bool, scale: f64) -> f64 {
        let from = if aside { 1.0 } else { 0.0 };
        // Spread twice as far as half way is the same way as pinched to
        // half: what is told is a share of how far apart they began.
        let way = scale.max(f64::MIN_POSITIVE).ln() / WHOLE.ln();
        (from + way).clamp(0.0, 1.0)
    }
}

/// A share out of one as the desktop model has it, out of 65536.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "a share from 0 to 1 by the clamp, of 65536"
)]
fn share(of: f64) -> u32 {
    (of.clamp(0.0, 1.0) * 65536.0).round() as u32
}

impl Compositor {
    /// Takes four fingers that began to pinch at `time`.
    pub(crate) fn spread_began(&mut self, time: u32) -> Spread {
        let aside = self.desktop.showing_desktop();
        Spread {
            aside,
            reached: if aside { 1.0 } else { 0.0 },
            fingers: Fingers::down(time),
        }
    }

    /// Carries the windows with the fingers of `spread`, now `scale` times
    /// as far apart as they began, at `time`.
    pub(crate) fn spread_moved(&mut self, spread: &mut Spread, scale: f64, time: u32) {
        let reached = Spread::reach(spread.aside, scale);
        spread.fingers.went(reached - spread.reached, time);
        spread.reached = reached;
        if !self.config().general.appearance.reduce_motion {
            self.desktop.carry_aside(share(reached));
        }
    }

    /// Takes the fingers of `spread` lifting, or the pinch being
    /// `cancelled`: the windows go on aside or back from where they were
    /// let go of.
    pub(crate) fn spread_ended(&mut self, spread: Spread, cancelled: bool) {
        let Spread {
            aside,
            reached,
            fingers,
        } = spread;
        let from = if aside { 1.0 } else { 0.0 };
        // Not moved at all, as a pinch with nothing aside, there is nothing
        // to let go of.
        if (reached - from).abs() < f64::EPSILON {
            return;
        }
        let ends_aside = if cancelled {
            aside
        } else if aside {
            !fingers.go_on(1.0 - reached, false)
        } else {
            fingers.go_on(reached, true)
        };
        self.desktop.let_go_aside(ends_aside);
    }

    /// Brings back, over the time a window takes to go where it was sent, the windows
    /// set aside on a workspace that is being left: by a key or the
    /// overview, which shows another, or by fingers that begin to swipe
    /// to one. They come back as it slides, never reaching past it.
    pub(crate) fn unspread(&mut self) {
        let clock = self.clock();
        if let Some(id) = self.desktop.unspreading() {
            self.protocols.gestures.unspreading = Some((id, clock));
        }
        let Some((id, began)) = self.protocols.gestures.unspreading else {
            return;
        };
        let gone = clock.saturating_sub(began);
        if gone >= BACK {
            self.protocols.gestures.unspreading = None;
            self.desktop.bring_back_aside(id);
        } else {
            let along = Easing::EaseInOut.at(gone.as_secs_f32() / BACK.as_secs_f32());
            self.desktop
                .carry_aside_of(id, share(1.0 - f64::from(along)));
        }
    }
}
