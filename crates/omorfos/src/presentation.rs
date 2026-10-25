//! Telling programs when what they drew reached the screen
//! (`presentation-time`), so that a video keeps time with its sound and a
//! game knows how late its frames are.
//!
//! A program asks, with what it commits, to be told of the frame that
//! commit is first shown in. What a frame drew is known as it is drawn:
//! [`Compositor::render`] takes, from the surfaces in it, what their
//! programs asked, and hands it back with the frame as a [`Feedback`]. The
//! backend keeps that until the graphics card says the frame is on the
//! screen, and then [`Feedback::shown`] tells them when. A surface that
//! was not in the frame keeps what it asked for the next one it is in; a
//! frame that is never shown tells its programs so, as it is dropped.

use std::time::Duration;

use smithay::backend::drm::{DrmEventMetadata, DrmEventTime};
use smithay::backend::renderer::element::RenderElementStates;
use smithay::delegate_presentation;
use smithay::desktop::layer_map_for_output;
use smithay::desktop::utils::{
    OutputPresentationFeedback, surface_presentation_feedback_flags_from_states,
    take_presentation_feedback_surface_tree,
};
use smithay::output::Output;
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback::Kind;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Clock, Monotonic, Time};
use smithay::wayland::compositor::SurfaceData;
use smithay::wayland::presentation::Refresh;

use crate::compositor::Compositor;

/// The programs to be told when a frame that was drawn reaches the screen.
#[derive(Debug)]
pub struct Feedback(OutputPresentationFeedback);

impl Feedback {
    /// Tells the programs that the frame reached the screen.
    ///
    /// `card` is what the graphics card said of it, if it said anything:
    /// when, and which of the screen's refreshes it was. The programs were
    /// promised times by the monotonic clock, so a time by the wall clock
    /// is no use to them: with that, as with nothing, the frame is said to
    /// have been shown now, and nothing is said of where the time came
    /// from. `refresh` is how long one of the screen's refreshes takes, if
    /// that is known.
    pub fn shown(mut self, card: Option<&DrmEventMetadata>, refresh: Option<Duration>) {
        let card = card.and_then(|card| match card.time {
            DrmEventTime::Monotonic(at) => Some((at, card.sequence)),
            DrmEventTime::Realtime(_) => None,
        });
        let (at, sequence, kind) = match card {
            Some((at, sequence)) => (
                Time::from(at),
                u64::from(sequence),
                Kind::Vsync | Kind::HwClock | Kind::HwCompletion,
            ),
            None => (Clock::<Monotonic>::new().now(), 0, Kind::empty()),
        };
        let refresh = refresh.map_or(Refresh::Unknown, Refresh::fixed);
        self.0.presented(at, refresh, sequence, kind);
    }
}

impl Compositor {
    /// Takes, from the surfaces drawn in a frame on `output`, what their
    /// programs asked to be told of it. `shows` is the output whose
    /// surfaces those are, which is another for a mirror; `states` is
    /// what the frame drew.
    pub(crate) fn feedback(
        &self,
        output: &Output,
        shows: &Output,
        states: &RenderElementStates,
    ) -> Feedback {
        let mut feedback = OutputPresentationFeedback::new(output);
        let drawn = |surface: &WlSurface, _: &SurfaceData| {
            states.element_render_state(surface).map(|_| output.clone())
        };
        let kind = |surface: &WlSurface, _: &SurfaceData| {
            surface_presentation_feedback_flags_from_states(surface, states)
        };
        for mapped in &self.windows {
            mapped
                .window
                .take_presentation_feedback(&mut feedback, drawn, kind);
        }
        for layer in layer_map_for_output(shows).layers() {
            layer.take_presentation_feedback(&mut feedback, drawn, kind);
        }
        if let Some(surface) = self.lock_surface_on(shows) {
            take_presentation_feedback_surface_tree(surface, &mut feedback, drawn, kind);
        }
        Feedback(feedback)
    }
}

delegate_presentation!(Compositor);
