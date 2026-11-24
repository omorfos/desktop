//! How each screen's frames are drawn: how many have been, how long the
//! last one took and how many blurs it made, and the middle and slowest of
//! the last few, for the socket to tell.

use std::time::Duration;

use omorfos_ipc::Drawing;

/// How many of each screen's last frames are kept: four seconds' worth at
/// 60 a second, enough to say how a drag or an animation went.
pub(crate) const KEPT: usize = 256;

/// The frames drawn on each screen that has drawn any.
#[derive(Debug, Default)]
pub(crate) struct Frames {
    screens: Vec<Screen>,
}

/// A screen's frames: as told, and how long each of the last took.
#[derive(Debug)]
struct Screen {
    drawing: Drawing,
    /// The last [`KEPT`] frames' times, in microseconds, the next written
    /// over the oldest: held once, so that a frame is noted without
    /// allocating.
    took: Box<[u32; KEPT]>,
}

impl Frames {
    /// Notes a frame drawn on the screen called `output`, which took
    /// `took` and made `blurs` blurs.
    pub(crate) fn drew(&mut self, output: &str, took: Duration, blurs: u64) {
        let known = self
            .screens
            .iter()
            .position(|seen| seen.drawing.output == output);
        let index = known.unwrap_or_else(|| {
            self.screens.push(Screen {
                drawing: Self::none(output),
                took: Box::new([0; KEPT]),
            });
            self.screens.len() - 1
        });
        let screen = &mut self.screens[index];
        let micros = u64::try_from(took.as_micros()).unwrap_or(u64::MAX);
        let at = usize::try_from(screen.drawing.frames % KEPT as u64).unwrap_or_default();
        screen.took[at] = u32::try_from(micros).unwrap_or(u32::MAX);
        let drawing = &mut screen.drawing;
        drawing.frames += 1;
        drawing.took = micros;
        drawing.blurs = blurs;
    }

    /// How the frames on the screen called `output` are drawn: none, if it
    /// has drawn none.
    pub(crate) fn of(&self, output: &str) -> Drawing {
        let seen = self
            .screens
            .iter()
            .find(|seen| seen.drawing.output == output);
        seen.map_or_else(|| Self::none(output), Screen::told)
    }

    /// A screen that has drawn nothing.
    fn none(output: &str) -> Drawing {
        Drawing {
            output: output.to_owned(),
            frames: 0,
            took: 0,
            blurs: 0,
            kept: 0,
            middle: 0,
            slowest: 0,
        }
    }
}

impl Screen {
    /// As told: with the middle and the slowest of the frames kept, worked
    /// out only when asked.
    fn told(&self) -> Drawing {
        let kept = usize::try_from(self.drawing.frames).map_or(KEPT, |frames| frames.min(KEPT));
        let mut took = *self.took;
        let took = &mut took[..kept];
        // Of an even number, the faster of the two in the middle.
        let (_, middle, _) = took.select_nth_unstable(kept.saturating_sub(1) / 2);
        let middle = u64::from(*middle);
        let slowest = took.iter().max().copied().map_or(0, u64::from);
        Drawing {
            kept: kept as u64,
            middle,
            slowest,
            ..self.drawing.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Draws a frame on `output` taking each of `millis`, in turn.
    fn drawing(frames: &mut Frames, output: &str, millis: impl IntoIterator<Item = u64>) {
        for each in millis {
            frames.drew(output, Duration::from_millis(each), 0);
        }
    }

    #[test]
    fn the_middle_and_the_slowest_of_the_frames_kept_are_told() {
        let mut frames = Frames::default();
        drawing(&mut frames, "eDP-1", [3, 1, 9, 2, 5]);
        let drawn = frames.of("eDP-1");
        assert_eq!((drawn.frames, drawn.took), (5, 5_000));
        assert_eq!((drawn.kept, drawn.middle, drawn.slowest), (5, 3_000, 9_000));
        // Of an even number, the faster of the two in the middle.
        drawing(&mut frames, "eDP-1", [4]);
        assert_eq!(frames.of("eDP-1").middle, 3_000);
    }

    #[test]
    fn only_the_last_frames_are_kept_so_a_slow_one_long_ago_goes() {
        let mut frames = Frames::default();
        drawing(&mut frames, "eDP-1", [900]);
        drawing(&mut frames, "eDP-1", (0..KEPT).map(|_| 2));
        let drawn = frames.of("eDP-1");
        assert_eq!(drawn.frames, 1 + KEPT as u64);
        assert_eq!((drawn.kept, drawn.slowest), (KEPT as u64, 2_000));
    }

    #[test]
    fn each_screen_keeps_its_own_and_one_that_drew_none_has_none() {
        let mut frames = Frames::default();
        drawing(&mut frames, "eDP-1", [1, 1]);
        drawing(&mut frames, "HDMI-A-1", [7]);
        assert_eq!(frames.of("eDP-1").slowest, 1_000);
        assert_eq!(frames.of("HDMI-A-1").slowest, 7_000);
        let none = frames.of("DP-2");
        assert_eq!(
            (none.frames, none.kept, none.middle, none.slowest),
            (0, 0, 0, 0)
        );
    }
}
