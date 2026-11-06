//! How each screen's frames are drawn: how many have been, and how long the
//! last one took and how many blurs it made, for the socket to tell.

use std::time::Duration;

use omorfos_ipc::Drawing;

/// The frames drawn on each screen that has drawn any.
#[derive(Debug, Default)]
pub(crate) struct Frames {
    screens: Vec<Drawing>,
}

impl Frames {
    /// Notes a frame drawn on the screen called `output`, which took
    /// `took` and made `blurs` blurs.
    pub(crate) fn drew(&mut self, output: &str, took: Duration, blurs: u64) {
        let known = self.screens.iter().position(|seen| seen.output == output);
        let index = known.unwrap_or_else(|| {
            self.screens.push(Self::none(output));
            self.screens.len() - 1
        });
        let drawing = &mut self.screens[index];
        drawing.frames += 1;
        drawing.took = u64::try_from(took.as_micros()).unwrap_or(u64::MAX);
        drawing.blurs = blurs;
    }

    /// How the frames on the screen called `output` are drawn: none, if it
    /// has drawn none.
    pub(crate) fn of(&self, output: &str) -> Drawing {
        let seen = self.screens.iter().find(|seen| seen.output == output);
        seen.cloned().unwrap_or_else(|| Self::none(output))
    }

    /// A screen that has drawn nothing.
    fn none(output: &str) -> Drawing {
        Drawing {
            output: output.to_owned(),
            frames: 0,
            took: 0,
            blurs: 0,
        }
    }
}
