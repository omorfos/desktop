//! The display key: the screens stood another way for now, as the
//! configuration has them, all showing what the first shows, or each
//! alone; each way kept only if the user says so in time, since it is the
//! key pressed when a screen has gone dark.
//!
//! What is tried is never written to the configuration: it is for the
//! screens plugged in when it was tried, and the configuration's way comes
//! back as soon as another is plugged in or one taken out. What is kept is
//! kept by the session for the next login too, with those screens
//! (ADR 104).

use omorfos_i18n::{Value, Words};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omorfos_ui::Answering;
use smithay::input::keyboard::Keysym;

use omorfos_config::{KeptScreens, KeptWay, NamedOutput, Outputs, Rotation, Stands, Theme};
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
    /// What it is, said to the user in `words`, with `first` the screen
    /// that comes first.
    fn described(&self, words: &Words, first: &str) -> String {
        match self {
            Self::Configured => words.say("screens-configured", &[]),
            Self::Mirrored => words.say("screens-mirrored", &[("first", Value::Words(first))]),
            Self::Alone(alone) => words.say("screens-alone", &[("alone", Value::Words(alone))]),
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

/// The question's answers: putting the way before back, which refuses, and
/// keeping the way tried.
const PUT_BACK: usize = 0;
const KEEP: usize = 1;

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
    /// How the question whether to keep the way tried is being answered
    /// (ADR 111): Enter keeps it a moment after it is asked, and only as
    /// it is let go, having gone down while it was asked.
    answering: Option<Answering<Duration, 2>>,
    /// What the session keeps of the screens between logins, and the file
    /// it keeps it in, once it is told where (ADR 104).
    pub(crate) remembered: KeptScreens,
    at: Option<PathBuf>,
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
    pub(crate) fn stood_for<'a>(
        &self,
        names: &(impl Iterator<Item = &'a str> + Clone),
    ) -> Option<&Outputs> {
        // A screen's name is plugged in once, so the same number of names,
        // each among those kept, are the same names.
        let same = |stood: &&Stood| {
            let count = names.clone().count() == stood.names.len();
            count
                && names
                    .clone()
                    .all(|name| stood.names.iter().any(|kept| kept == name))
        };
        let tried = self.interim.trying.as_ref().map(|(stood, _)| stood);
        let stood = tried.into_iter().chain(&self.interim.kept);
        let mut stood = stood.filter(same);
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
        let plugged: Vec<String> = self
            .every_head()
            .map(|head| head.known().to_owned())
            .collect();
        if plugged.len() < 2 {
            self.interim.said.push("only-one-screen");
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
        let configured = self.configured_for(&plugged.iter().map(String::as_str));
        let outputs = way_of(configured, &order, &way);
        let stood = Stood {
            way,
            order,
            names,
            outputs,
        };
        self.interim.trying = Some((stood, None));
        let mut answering = Answering::new(PUT_BACK);
        answering.shown(self.clock());
        self.interim.answering = Some(answering);
        self.apply_scale();
    }

    /// Says where the session keeps what it keeps of the screens between
    /// logins, `directory`, and takes up what is kept there: the way the
    /// display key last kept them in, and the scales guessed for them. What
    /// cannot be read is guessed again (ADR 104).
    pub fn set_state_directory(&mut self, directory: &Path) {
        let at = directory.join("screens.yaml");
        let text = std::fs::read_to_string(&at).unwrap_or_default();
        let remembered = KeptScreens::parse(&text).unwrap_or_default();
        self.interim.kept = remembered.way.as_ref().map(|kept| {
            let way = match &kept.stands {
                Stands::Mirrored => Way::Mirrored,
                Stands::Alone(alone) => Way::Alone(alone.clone()),
            };
            let configured = self.configured_for(&kept.names.iter().map(String::as_str));
            let outputs = way_of(configured, &kept.order, &way);
            Stood {
                way,
                order: kept.order.clone(),
                names: sorted(kept.names.clone()),
                outputs,
            }
        });
        self.interim.remembered = remembered;
        self.interim.at = Some(at);
        self.apply_scale();
    }

    /// Writes what the session keeps of the screens where it is kept, if it
    /// has been told where: whole or not at all, as it is written aside and
    /// put in place. One that cannot be written is guessed again next time.
    pub(crate) fn remember(&self) {
        if let Some(at) = &self.interim.at {
            let aside = at.with_extension("yaml.new");
            let _ = at.parent().map(std::fs::create_dir_all);
            let written = std::fs::write(&aside, self.interim.remembered.to_text());
            let _ = written.and_then(|()| std::fs::rename(&aside, at));
        }
    }

    /// The screens called `plugged`, standing as the configuration has
    /// them, in the order the display key goes through them: the one that
    /// comes first, which the others mirror, and then the rest as they
    /// were plugged in.
    fn screen_order(&self, plugged: &[String]) -> Vec<String> {
        // There is always one that comes first, the screens never all off.
        let first = self.primary_head().map(|head| head.known().to_owned());
        let first = first.unwrap_or_default();
        let rest = plugged.iter().filter(|name| **name != first).cloned();
        std::iter::once(first.clone()).chain(rest).collect()
    }

    /// Takes a key going down or up, while a way of the
    /// screens is tried: says whether it was the question's, Enter keeping
    /// the way and Escape putting the one before back, each as it is let
    /// go, Enter not until a moment after it was asked.
    pub(crate) fn screens_key(&mut self, keysym: Keysym, pressed: bool) -> bool {
        let button = match keysym {
            Keysym::Return | Keysym::KP_Enter => KEEP,
            Keysym::Escape => PUT_BACK,
            _ => return false,
        };
        let now = self.clock();
        let trying = self.interim.trying();
        let answering = self.interim.answering.as_mut().filter(|_| trying);
        let Some(answering) = answering else {
            return false;
        };
        // There are no buttons to be on here: each key presses its own.
        if let Some(chosen) = answering.press(Some(button), pressed, now) {
            self.answer_screens(chosen == KEEP);
        }
        true
    }

    /// Keeps the way the screens are being tried in, if `keep`, and for the
    /// next login too, or else has them stand as they did before it.
    pub(crate) fn answer_screens(&mut self, keep: bool) {
        let tried = self.interim.trying.take().map(|(stood, _)| stood);
        if keep {
            // The configuration's way is kept by keeping no other, so that
            // a change to the configuration is followed again.
            self.interim.kept = tried.filter(|stood| stood.way != Way::Configured);
            self.interim.remembered.way = self.interim.kept.as_ref().map(|stood| KeptWay {
                stands: match &stood.way {
                    Way::Alone(alone) => Stands::Alone(alone.clone()),
                    Way::Mirrored | Way::Configured => Stands::Mirrored,
                },
                order: stood.order.clone(),
                names: stood.names.clone(),
            });
            self.remember();
            self.interim.said.push("screens-kept");
        } else {
            self.interim.said.push("screens-back");
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
            let said = self.words.say(said, &[]);
            self.happened(&said, now);
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
        let what = tried.way.described(&self.words, first);
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
                let canvas =
                    paint::keep_screens((&mut self.text, &self.words), &theme, asked, head.scale);
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
        let english = Words::in_languages(&omorfos_i18n::COMPOSITOR, &[]);
        let polish = Words::in_languages(&omorfos_i18n::COMPOSITOR, &["pl".to_owned()]);
        assert_eq!(
            Way::Mirrored.described(&polish, "eDP-1"),
            "Lustrzane: każdy ekran pokazuje to, co eDP-1"
        );
        assert_eq!(
            Way::Configured.described(&english, "eDP-1"),
            "As config.yaml has them"
        );
        assert_eq!(
            Way::Mirrored.described(&english, "eDP-1"),
            "Mirrored: every screen shows what eDP-1 shows"
        );
        let alone = Way::Alone("HDMI-A-1".to_owned());
        assert_eq!(
            alone.described(&english, "eDP-1"),
            "Only HDMI-A-1: the others are off"
        );
    }
}
