//! Workspaces slid across by three fingers on a touchpad, as a page is
//! turned: the one shown goes with the fingers, and the next comes in from
//! the right, or the one before from the left.
//!
//! What the fingers carry follows them with no easing at all. Once they
//! lift, the workspace that came in is shown if they went more than half
//! the screen, or were going fast enough that they would have, and the
//! one that was there is kept otherwise, which is how a swipe is taken
//! back halfway. Either way it is shown at once, so that nothing waits on
//! the movement, and the workspaces settle where they belong over 240 ms,
//! as `docs/DESIGN.md` says of a workspace sliding. With nothing beside
//! the one shown, it goes a little way with the fingers, and back.
//!
//! The keys that go between workspaces slide them the same way, without
//! the fingers: the one a key brings is shown at once and slides in from
//! the side it lies on, with the one it left beside it, however far apart
//! the two are. A key pressed again before they settle takes them on from
//! where they are.

use std::time::Duration;

use omorfos_config::Easing;

use crate::compositor::Compositor;

/// How long the workspaces take to settle once the fingers lift.
const SETTLE: Duration = Duration::from_millis(240);

/// How far a swipe let go of is taken to carry on, as the time it would go
/// on for at the speed it was going, in milliseconds.
const THROW: f64 = 120.0;

/// How much of the fingers' way a workspace with nothing beside it goes.
const RESISTED: f64 = 0.25;

/// A slide of the workspaces of the current output, while the fingers are
/// on the touchpad and while it settles after.
#[derive(Debug, Default)]
pub(crate) struct Slide {
    following: Option<Following>,
    settling: Option<Settling>,
}

impl Slide {
    /// Whether it is settling, and to be drawn again shortly.
    pub(crate) const fn settling(&self) -> bool {
        self.settling.is_some()
    }
}

/// Fingers carrying the workspaces.
#[derive(Debug)]
struct Following {
    /// Where the workspace shown was when they began, in units of layout.
    from: f64,
    /// How far across they have gone since.
    moved: f64,
    /// How fast they were going across when last they moved, in units of
    /// layout a millisecond.
    speed: f64,
    /// When they last moved, in milliseconds.
    last: u32,
}

/// The workspaces on their way to where they belong.
#[derive(Debug)]
struct Settling {
    /// Where the workspace shown was when the fingers lifted.
    from: f64,
    /// When that was, by the compositor's clock.
    began: Duration,
}

/// An offset as the desktop model has it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "offsets are within a screen's width, far inside an i32"
)]
fn layout(offset: f64) -> i32 {
    offset.round() as i32
}

impl Compositor {
    /// How far a slide goes across: the width of the current output.
    fn slide_width(&self) -> f64 {
        let output = self.desktop.current_output();
        let width = self.heads.get(output).map_or(0, |head| head.area().size.w);
        f64::from(width)
    }

    /// Whether the current output has a workspace to slide in beside the
    /// one it shows.
    fn slides_to_another(&self) -> bool {
        let output = self.desktop.current_output();
        let workspaces = self.desktop.workspaces();
        workspaces.filter(|place| place.output == output).count() > 1
    }

    /// Takes three fingers that began to swipe at `time`, on the output the
    /// pointer is on. A slide still settling is taken up from where it is.
    pub(crate) fn swipe_began(&mut self, time: u32) {
        self.desktop.turn_to(self.pointer_position());
        let slide = &mut self.protocols.gestures.slide;
        slide.settling = None;
        slide.following = Some(Following {
            from: f64::from(self.desktop.slid()),
            moved: 0.0,
            speed: 0.0,
            last: time,
        });
    }

    /// Carries the workspaces `dx` further across with the fingers, at
    /// `time`. Says whether there was a swipe of ours to carry them.
    pub(crate) fn swipe_moved(&mut self, dx: f64, time: u32) -> bool {
        let width = self.slide_width();
        let resisted = if self.slides_to_another() {
            1.0
        } else {
            RESISTED
        };
        let Some(following) = self.protocols.gestures.slide.following.as_mut() else {
            return false;
        };
        let gone = f64::from(time.wrapping_sub(following.last));
        if gone > 0.0 {
            following.speed = dx / gone;
            following.last = time;
        }
        following.moved += dx;
        let offset = following.moved.mul_add(resisted, following.from);
        self.desktop.slide(layout(offset.clamp(-width, width)));
        true
    }

    /// Takes the fingers lifting, or the swipe being `cancelled`: shows
    /// the workspace that came in, if they took it far enough, and lets the
    /// workspaces settle. Says whether there was a swipe of ours.
    pub(crate) fn swipe_ended(&mut self, cancelled: bool) -> bool {
        let Some(following) = self.protocols.gestures.slide.following.take() else {
            return false;
        };
        let offset = f64::from(self.desktop.slid());
        let width = self.slide_width();
        let next = offset < 0.0;
        let thrown = following.speed.mul_add(THROW, offset);
        let far = thrown.abs() > width / 2.0 && (thrown < 0.0) == next;
        let turned = !cancelled && far && self.slides_to_another();
        // The workspace that came in is shown from where it has got to.
        let from = match (turned, next) {
            (false, _) => offset,
            (true, true) => {
                self.desktop.next_workspace();
                offset + width
            }
            (true, false) => {
                self.desktop.previous_workspace();
                offset - width
            }
        };
        self.desktop.slide(layout(from));
        let began = self.clock();
        self.protocols.gestures.slide.settling = Some(Settling { from, began });
        true
    }

    /// Slides in the workspace a key has brought onto the current output,
    /// from the side it lies on, `forwards` being the right, with `was`,
    /// the one it was showing, beside it. Nothing slides if the key brought
    /// nothing else.
    pub(crate) fn slide_in(&mut self, was: usize, forwards: bool) {
        if self.desktop.active_workspace() == was {
            return;
        }
        let width = self.slide_width();
        let side = if forwards { width } else { -width };
        let from = f64::from(self.desktop.slid()) + side;
        self.desktop.slide_from(layout(from), was);
        let began = self.clock();
        self.protocols.gestures.slide.settling = Some(Settling { from, began });
    }

    /// Brings a slide that is settling to where it has got to by now.
    pub(crate) fn settle_slide(&mut self) {
        let now = self.clock();
        let Some(settling) = &self.protocols.gestures.slide.settling else {
            return;
        };
        let gone = now.saturating_sub(settling.began);
        let share = gone.as_secs_f32() / SETTLE.as_secs_f32();
        if share >= 1.0 {
            self.protocols.gestures.slide.settling = None;
            self.desktop.slide(0);
            return;
        }
        let left = f64::from(1.0 - Easing::EaseInOut.at(share));
        self.desktop.slide(layout(settling.from * left));
    }
}
