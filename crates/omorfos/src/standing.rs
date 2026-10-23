//! The display key: the screens stood another way for now, as the
//! configuration has them, all showing what the first shows, or each
//! alone; each way kept only if the user says so in time, since it is the
//! key pressed when a screen has gone dark.
//!
//! What is tried is never written to the configuration: it is for the
//! screens plugged in when it was tried, and the configuration's way comes
//! back as soon as another is plugged in or one taken out.

use std::time::{Duration, Instant};

use omorfos_config::{NamedOutput, Outputs, Rotation, Theme};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use crate::compositor::{Compositor, Painted};
use crate::{Scale, paint};

/// How long a way the screens are tried in waits to be kept before they
/// go back to the way they were.
pub(crate) const KEPT_WITHIN: Duration = Duration::from_secs(15);

/// A way for the screens to stand.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Way {
    /// As the configuration says.
    Configured,
    /// All showing what the one that comes first shows.
    Mirrored,
    /// The screen of this name alone, and the others off.
    Alone(String),
}

impl Way {
    /// What it is, said to the user, with `first` the screen that comes
    /// first.
    fn described(&self, first: &str) -> String {
        match self {
            Self::Configured => "As config.yaml has them".to_owned(),
            Self::Mirrored => format!("Mirrored: every screen shows what {first} shows"),
            Self::Alone(alone) => format!("Only {alone}: the others are off"),
        }
    }
}

/// A way the screens stand in place of the configuration's, for the
/// screens of these names.
#[derive(Debug)]
struct Stood {
    way: Way,
    /// The screens in the order the display key goes through them alone,
    /// the one the others mirror first: fixed when it was first pressed,
    /// since screens that are off or mirror another stand in another order.
    order: Vec<String>,
    /// Their names, in order, so that they are told apart from another set.
    names: Vec<String>,
    outputs: Outputs,
}

/// How the screens are made to stand for now, if otherwise than the
/// configuration says.
#[derive(Debug, Default)]
pub(crate) struct Interim {
    /// The way last kept.
    kept: Option<Stood>,
    /// The way being tried, and when it goes back unless kept: none until
    /// the compositor next knows what time it is.
    trying: Option<(Stood, Option<Instant>)>,
    /// The question asked on each screen that is on while a way is tried.
    pub(crate) prompts: Vec<Prompt>,
    /// What is to be said of it, once the compositor knows what time it is.
    said: Vec<&'static str>,
}

/// What asks whether to keep the screens as they are, on one screen.
#[derive(Debug)]
pub(crate) struct Prompt {
    pub(crate) output: Output,
    /// Where it is, in the middle of that screen.
    pub(crate) at: Point<i32, Logical>,
    scale: Scale,
    theme: Theme,
    /// The screen's place, which it is in the middle of.
    area: Rectangle<i32, Logical>,
    /// The way it says the screens are in, and how many seconds it says
    /// are left.
    what: String,
    left: u64,
    pub(crate) pixels: Painted,
}

impl Interim {
    /// Whether a way is being tried, and is to be kept or not.
    pub(crate) const fn trying(&self) -> bool {
        self.trying.is_some()
    }

    /// When something is next due: a second less to wait, or the screens
    /// going back as they were.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        let until = self.trying.as_ref().and_then(|(_, until)| *until)?;
        let left = self.prompts.first().map_or(0, |prompt| prompt.left);
        // The prompt says how many whole seconds are left, and changes as
        // each goes.
        let next = Duration::from_secs(left.saturating_sub(1));
        Some(until.checked_sub(next).unwrap_or(until))
    }
}

/// `names` in order.
fn sorted(mut names: Vec<String>) -> Vec<String> {
    names.sort();
    names
}

impl Compositor {
    /// The way the screens stand for now in place of the configuration's,
    /// if there is one for those called `names`: what is being tried, or
    /// else what was kept, unless that is the configuration's way.
    pub(crate) fn stood_for(&self, names: &[String]) -> Option<&Outputs> {
        let names = sorted(names.to_vec());
        let tried = self.interim.trying.as_ref().map(|(stood, _)| stood);
        let stood = tried.into_iter().chain(&self.interim.kept);
        let mut stood = stood.filter(|stood| stood.names == names);
        let stood = stood.next()?;
        (stood.way != Way::Configured).then_some(&stood.outputs)
    }

    /// Forgets the way being tried if the screens plugged in are no longer
    /// those it was for: these called `names` stand as the configuration
    /// says for them.
    pub(crate) fn forget_tried(&mut self, names: &[String]) {
        let names = sorted(names.to_vec());
        let other = |(stood, _): &(Stood, Option<Instant>)| stood.names != names;
        if self.interim.trying.as_ref().is_some_and(other) {
            self.interim.trying = None;
        }
    }

    /// Has the screens stand the next way, to be kept only if the user
    /// says so in time. With one screen there is no other way.
    pub(crate) fn next_screens(&mut self) {
        let plugged: Vec<String> = self.every_head().map(|head| head.output.name()).collect();
        if plugged.len() < 2 {
            self.interim
                .said
                .push("There is only one screen, so it stands only one way");
            return;
        }
        let names = sorted(plugged.clone());
        let tried = self.interim.trying.as_ref().map(|(stood, _)| stood);
        let mut now = tried.into_iter().chain(&self.interim.kept);
        let now = now.find(|stood| stood.names == names);
        let (now, order) = match now {
            Some(stood) => (stood.way.clone(), stood.order.clone()),
            None => (Way::Configured, self.screen_order(&plugged)),
        };
        let mut ways = vec![Way::Configured, Way::Mirrored];
        ways.extend(order.iter().cloned().map(Way::Alone));
        let at = ways.iter().position(|way| *way == now).unwrap_or(0);
        let way = ways
            .into_iter()
            .cycle()
            .nth(at + 1)
            .unwrap_or(Way::Configured);
        let outputs = way_of(self.configured_for(&plugged), &order, &way);
        let stood = Stood {
            way,
            order,
            names,
            outputs,
        };
        self.interim.trying = Some((stood, None));
        self.apply_scale();
    }

    /// The screens called `plugged`, standing as the configuration has
    /// them, in the order the display key goes through them: the one that
    /// comes first, which the others mirror, and then the rest as they
    /// were plugged in.
    fn screen_order(&self, plugged: &[String]) -> Vec<String> {
        // There is always one that comes first, the screens never all off.
        let first = self.primary_head().map(|head| head.output.name());
        let first = first.unwrap_or_default();
        let rest = plugged.iter().filter(|name| **name != first).cloned();
        std::iter::once(first.clone()).chain(rest).collect()
    }

    /// Keeps the way the screens are being tried in, if `keep`, or else
    /// has them stand as they did before it.
    pub(crate) fn answer_screens(&mut self, keep: bool) {
        let tried = self.interim.trying.take().map(|(stood, _)| stood);
        if keep {
            // The configuration's way is kept by keeping no other, so that
            // a change to the configuration is followed again.
            self.interim.kept = tried.filter(|stood| stood.way != Way::Configured);
            self.interim.said.push("The screens are kept as they are");
        } else {
            self.interim.said.push("The screens are back as they were");
        }
        self.apply_scale();
    }

    /// Starts the time a way being tried has to be kept in, and puts the
    /// screens back once it is up.
    pub(crate) fn time_screens(&mut self, now: Instant) {
        let trying = self.interim.trying.as_mut();
        let until = trying.map(|(_, until)| *until.get_or_insert(now + KEPT_WITHIN));
        if until.is_some_and(|until| until <= now) {
            self.answer_screens(false);
        }
        for said in std::mem::take(&mut self.interim.said) {
            self.happened(said, now);
        }
        // Each whole second or part of one left is said as one.
        let left = until.map_or(0, |until| {
            let left = until.saturating_duration_since(now);
            left.as_secs() + u64::from(left.subsec_nanos() > 0)
        });
        self.ask_to_keep(left);
    }

    /// Puts the question whether to keep the screens as they are in the
    /// middle of each that is on while a way is tried, saying how many
    /// seconds are `left`; or takes it away.
    fn ask_to_keep(&mut self, left: u64) {
        let mut had = std::mem::take(&mut self.interim.prompts);
        let Some((tried, _)) = self.interim.trying.as_ref() else {
            return;
        };
        let first = tried.order.first().map_or("", String::as_str);
        let what = tried.way.described(first);
        let theme = self.config().theme.clone();
        let mut prompts = Vec::new();
        for head in &self.heads {
            let known = had.iter().position(|prompt| {
                prompt.output == head.output
                    && prompt.scale == head.scale
                    && prompt.area == head.area()
                    && prompt.what == what
                    && prompt.left == left
                    && prompt.theme == theme
            });
            let prompt = known.map(|at| had.swap_remove(at));
            let prompt = prompt.unwrap_or_else(|| {
                let asked = (what.as_str(), left);
                let canvas = paint::keep_screens(&mut self.text, &theme, asked, head.scale);
                let (width, height) = canvas.units();
                let area = head.area();
                let middle = |room: i32, length: u32| (room - length.cast_signed()) / 2;
                let within = (middle(area.size.w, width), middle(area.size.h, height));
                Prompt {
                    output: head.output.clone(),
                    at: area.loc + Point::from(within),
                    scale: head.scale,
                    theme: theme.clone(),
                    area,
                    what: what.clone(),
                    left,
                    pixels: Painted::new(&canvas),
                }
            });
            prompts.push(prompt);
        }
        self.interim.prompts = prompts;
    }
}

/// The screens called `order` standing `way`, each as `configured` has it
/// as far as that goes: driven in the mode it gives, and turned as it
/// says. The first is the one the others show.
fn way_of(configured: &Outputs, order: &[String], way: &Way) -> Outputs {
    let first = order.first().map_or("", String::as_str);
    let mut stood = Vec::new();
    for name in order {
        let listed = configured.named(name);
        let mut screen = NamedOutput {
            name: name.clone(),
            scale: listed.and_then(|listed| listed.scale),
            mode: listed.and_then(|listed| listed.mode),
            place: None,
            align: None,
            offset: None,
            rotate: listed.map_or(Rotation::Normal, |listed| listed.rotate),
            primary: false,
            mirror: None,
            enabled: true,
        };
        let standing = match way {
            // Never stood in place of the configuration's, only kept as a
            // way to go back to.
            Way::Configured => true,
            Way::Mirrored => *name == first,
            Way::Alone(alone) => name == alone,
        };
        if !standing {
            screen.scale = None;
            if *way == Way::Mirrored {
                screen.mirror = Some(first.to_owned());
            } else {
                screen.enabled = false;
            }
        }
        stood.push(screen);
    }
    Outputs::try_from(stood).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_way_is_said_as_it_is() {
        assert_eq!(
            Way::Configured.described("eDP-1"),
            "As config.yaml has them"
        );
        assert_eq!(
            Way::Mirrored.described("eDP-1"),
            "Mirrored: every screen shows what eDP-1 shows"
        );
        let alone = Way::Alone("HDMI-A-1".to_owned());
        assert_eq!(
            alone.described("eDP-1"),
            "Only HDMI-A-1: the others are off"
        );
    }
}
