//! The picture each screen showed as the compositor was started, drawn over
//! the session's first frames and faded out (ADR 62, the third step).
//!
//! Started anew, the compositor takes the screens as the one before left
//! them, showing its last frame, and goes on showing it until the session
//! is whole again (ADR 62, the first two steps). Its own first frame would
//! then replace that picture at once; laid over it, and faded, the picture
//! gives way to the session instead.

use std::time::Duration;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::output::Output;
use smithay::utils::Size;

use crate::compositor::{Compositor, Element, Painted, View};

/// How long the picture before takes to give way to the session.
const FADING: Duration = Duration::from_millis(250);

/// What a screen showed as the compositor was started.
#[derive(Debug)]
pub(crate) struct Fade {
    output: Output,
    picture: Painted,
    /// When it began to fade, on the animation clock: once the session is
    /// shown.
    since: Option<Duration>,
}

impl Compositor {
    /// Lays `bytes`, a picture of `pixels` across and down in `format`,
    /// what `output` showed as the compositor was started, over the
    /// session's first frames there, fading it out once the session is
    /// shown. One that is not as large as it says is left out.
    pub fn fade_from(&mut self, output: &Output, bytes: &[u8], pixels: (i32, i32), format: Fourcc) {
        let (width, height) = pixels;
        let whole = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .map(|(width, height)| width * height * 4);
        let fits = whole.is_some_and(|whole| whole > 0 && bytes.len() == whole);
        // Made as large as the screen as it is drawn: the backend says how
        // large the screen's pixels are only after.
        let picture = fits.then(|| Painted::from_bytes(bytes, format, pixels, pixels.into()));
        self.animation.fading.extend(picture.map(|picture| Fade {
            output: output.clone(),
            picture,
            since: None,
        }));
    }

    /// Begins to fade what is to fade once the session is shown, and lets
    /// go of what has.
    pub(crate) fn fade(&mut self) {
        let shown = self.showing();
        let clock = self.clock();
        for fade in &mut self.animation.fading {
            fade.since = fade.since.or(shown.then_some(clock));
        }
        let faded = |fade: &Fade| fade.since.is_some_and(|since| clock >= since + FADING);
        self.animation.fading.retain(|fade| !faded(fade));
    }

    /// Whether anything is fading, to be drawn again a frame later.
    pub(crate) fn is_fading(&self) -> bool {
        self.animation
            .fading
            .iter()
            .any(|fade| fade.since.is_some())
    }

    /// What is fading on `output`, over everything else there.
    pub(crate) fn fading_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let clock = self.clock();
        // A screen with no area of its own shows none of it.
        let size = self
            .head(output)
            .map_or(Size::default(), |head| head.area().size);
        let here = self
            .animation
            .fading
            .iter()
            .filter(|fade| fade.output == *output);
        for fade in here {
            let gone = fade
                .since
                .map_or(Duration::ZERO, |since| clock.saturating_sub(since));
            let opacity = 1.0 - gone.as_secs_f32() / FADING.as_secs_f32();
            let placed = (view.origin.to_f64(), size);
            elements.extend(fade.picture.stretched(renderer, placed, view, opacity));
        }
    }
}
