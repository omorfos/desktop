//! The windows set aside to show the desktop by four fingers on a touchpad,
//! spread apart, and brought back by them pinched together, as a Mac has
//! it: the windows go with the fingers, toward the sides of the screen as
//! they spread and back as they close, with no easing at all, so that the
//! fingers can take them a little way and back again before deciding. Once
//! they lift, the windows go on or back as `fingers` decides for every
//! gesture, the rest of the way taking the time a window takes to go where
//! it was sent, as when Escape brings them back.
//!
//! Pinched together with nothing set aside, nothing moves, and the
//! launcher opens onto every program as they lift, if they went far enough
//! or fast enough by the same rule, as a Mac opens its Apps (ADR 101).
//! Spread apart while the launcher, or anything else that demands the
//! keyboard, is open, they close it by the same rule, as a press beside it
//! would, and set nothing aside: one gesture means one thing at a time.
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
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::backend::ClientId;

use crate::compositor::Compositor;
use crate::fingers::Fingers;

/// How much farther apart the fingers end than they began, for the whole
/// way aside; as much nearer together for the whole way back.
const WHOLE: f64 = 1.5;

/// How long the windows set aside on a workspace being left take to come
/// back, as long as a window takes to go where it was sent.
const BACK: Duration = Duration::from_millis(240);

/// Fingers carrying the windows aside or back.
#[derive(Debug, Clone)]
pub(crate) struct Spread {
    /// Whether the windows were aside as they began.
    aside: bool,
    /// How far aside the fingers have taken them, out of one.
    reached: f64,
    fingers: Fingers,
    /// With nothing aside, how far toward every program the fingers have
    /// pinched, out of one, and how they went there.
    pinched: (f64, Fingers),
    /// With nothing aside, the program that demanded the keyboard as they
    /// began, if one did, for a spread to close rather than set windows
    /// aside.
    closing: Option<ClientId>,
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

    /// How far toward every program fingers pinched to `scale` times as
    /// far apart as they began go: as far as they would bring the windows
    /// back from aside.
    fn pinch(scale: f64) -> f64 {
        1.0 - Self::reach(true, scale)
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
        let demanding = self.demanding().and_then(|surface| surface.client());
        let closing = demanding.map(|client| client.id()).filter(|_| !aside);
        Spread {
            aside,
            reached: if aside { 1.0 } else { 0.0 },
            fingers: Fingers::down(time),
            pinched: (0.0, Fingers::down(time)),
            closing,
        }
    }

    /// Carries the windows with the fingers of `spread`, now `scale` times
    /// as far apart as they began, at `time`.
    pub(crate) fn spread_moved(&mut self, spread: &mut Spread, scale: f64, time: u32) {
        let reached = Spread::reach(spread.aside, scale);
        spread.fingers.went(reached - spread.reached, time);
        spread.reached = reached;
        if !spread.aside {
            let (was, fingers) = &mut spread.pinched;
            let pinched = Spread::pinch(scale);
            fingers.went(pinched - *was, time);
            *was = pinched;
        }
        if spread.closing.is_none() && !self.config().general.appearance.reduce_motion {
            self.desktop.carry_aside(share(reached));
        }
    }

    /// Takes the fingers of `spread` lifting, or the pinch being
    /// `cancelled`: the windows go on aside or back from where they were
    /// let go of; or, pinched with nothing aside far or fast enough, the
    /// launcher opens onto every program.
    pub(crate) fn spread_ended(&mut self, spread: Spread, cancelled: bool) {
        let Spread {
            aside,
            reached,
            fingers,
            pinched: (pinched, pinching),
            closing,
        } = spread;
        if let Some(demanding) = closing {
            if !cancelled && reached > 0.0 && fingers.go_on(reached, true) {
                self.close_demanding(&demanding);
            }
            return;
        }
        if !aside && !cancelled && pinched > 0.0 && pinching.go_on(pinched, true) {
            self.perform(&omorfos_config::Action::ShowPrograms);
            return;
        }
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
