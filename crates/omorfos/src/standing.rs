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

use omorfos_config::{Part, State, States};
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use omorfos_ui::question::Asked;
use omorfos_ui::{Answering, Area, Canvas};
use smithay::input::keyboard::Keysym;

use omorfos_config::{KeptScreens, KeptWay, NamedOutput, Outputs, Rotation, Stands, Theme};
use smithay::output::Output;
use smithay::utils::{Logical, Point, Rectangle};

use crate::Scale;
use crate::compositor::{Compositor, Painted};
use crate::leaving::held;
use crate::paint::Text;

/// How long a way the screens are tried in waits to be kept before they
/// go back to the way they were.
pub(crate) const KEPT_WITHIN: Duration = Duration::from_secs(15);

/// How long after it was asked the question goes back by itself at the
/// latest, however long being there puts it off: it is how the screens
/// come back after a way of showing them that shows nothing (ADR 112).
const KEPT_AT_MOST: Duration = Duration::from_secs(120);

/// How wide the question is; the rest of its layout is a question's.
const WIDTH: u32 = 380;

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
    /// the compositor next knows what time it is, and again each time the
    /// user is there.
    trying: Option<(Stood, Option<Instant>)>,
    /// When the way being tried was first counted from, past two minutes
    /// after which being there no longer puts it off.
    first: Option<Instant>,
    /// The question asked on each screen that is on while a way is tried.
    pub(crate) prompts: Vec<Prompt>,
    /// What is to be said of it, once the compositor knows what time it is.
    said: Vec<&'static str>,
    /// How the question whether to keep the way tried is being answered
    /// (ADR 111): the keyboard starts on Put back, and Keep keeps it only
    /// a moment after it is asked, as it is let go.
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
    /// The button the pointer is on, if any, and the one the keyboard is.
    hovered: Option<usize>,
    focus: usize,
    /// Where it is all, and its buttons, Put back and Keep, are.
    whole: Rect,
    buttons: [Rect; 2],
    pub(crate) pixels: Painted,
}

impl Prompt {
    /// The button at `point`, if one is.
    fn button_at(&self, point: DomainPoint) -> Option<usize> {
        self.buttons
            .iter()
            .position(|button| button.contains(point))
    }
}

impl Interim {
    /// Whether a way is being tried, and is to be kept or not.
    pub(crate) const fn trying(&self) -> bool {
        self.trying.is_some()
    }

    /// The user was there just now: the way tried waits its time again,
    /// counted from the next time it is looked at, to two minutes after it
    /// was asked at most.
    pub(crate) fn stirred(&mut self) {
        if let Some((_, until)) = self.trying.as_mut() {
            *until = None;
        }
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
        self.interim.first = None;
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

    /// Takes a key, by its `code`, going down or up, while a way of the
    /// screens is tried: says whether it was the question's. The keyboard
    /// starts on Put back; Tab and Right move it to Keep, Enter presses
    /// the one it is on, as it is let go, Keep not until a moment after;
    /// and Escape puts the way before back at once, from wherever the
    /// keyboard is (ADR 111, 112). Space is left to programs, as it goes
    /// on with a slide on a screen just mirrored to a projector.
    pub(crate) fn screens_key(&mut self, code: u32, keysym: Keysym, pressed: bool) -> bool {
        let now = self.clock();
        let trying = self.interim.trying();
        let answering = self.interim.answering.as_mut().filter(|_| trying);
        let held = held(keysym).filter(|_| keysym != Keysym::space);
        let Some((answering, held)) = answering.zip(held) else {
            return false;
        };
        if let Some(chosen) = answering.key(code, held, pressed, now) {
            self.answer_screens(chosen == KEEP);
        }
        true
    }

    /// Takes the pointer's button going down or up at `point` while a way
    /// of the screens is tried: says whether it was on the question, where
    /// a button is answered as it is let go on what it went down on.
    pub(crate) fn screens_press(&mut self, pressed: bool, point: DomainPoint) -> bool {
        let now = self.clock();
        let trying = self.interim.trying();
        let Some(answering) = self.interim.answering.as_mut().filter(|_| trying) else {
            return false;
        };
        let prompts = &self.interim.prompts;
        let Some(prompt) = prompts.iter().find(|prompt| prompt.whole.contains(point)) else {
            // Let go off the question, a press on it is let go of too.
            let _ = answering.press(None, pressed, now);
            return false;
        };
        if let Some(chosen) = answering.press(prompt.button_at(point), pressed, now) {
            self.answer_screens(chosen == KEEP);
        }
        true
    }

    /// Where the buttons of the question are on the first screen, while it
    /// is asked, each by its middle: Put back, then Keep. For tests, which
    /// press them.
    #[must_use]
    pub fn keep_screens_buttons(&self) -> Option<((i32, i32), (i32, i32))> {
        let middle = |rect: &Rect| {
            let across = i32::try_from(rect.size.width / 2).unwrap_or(0);
            let down = i32::try_from(rect.size.height / 2).unwrap_or(0);
            (rect.origin.x + across, rect.origin.y + down)
        };
        let prompt = self.interim.prompts.first();
        prompt.map(|prompt| (middle(&prompt.buttons[0]), middle(&prompt.buttons[1])))
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
        let first = self
            .interim
            .trying
            .as_ref()
            .map(|_| *self.interim.first.get_or_insert(now));
        let trying = self.interim.trying.as_mut();
        let until = trying.zip(first).map(|((_, until), first)| {
            // Its 15 seconds, from now or when the user was last there,
            // but no later than two minutes after it was asked.
            *until.get_or_insert((now + KEPT_WITHIN).min(first + KEPT_AT_MOST))
        });
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
        let pointer = self.pointer_position();
        let focus = self
            .interim
            .answering
            .as_ref()
            .map_or(PUT_BACK, Answering::focus);
        let first = tried.order.first().map_or("", String::as_str);
        let what = tried.way.described(&self.words, first);
        let theme = self.config().theme.clone();
        let mut prompts = Vec::new();
        for head in &self.heads {
            let before = had.iter().position(|prompt| prompt.output == head.output);
            let hovered = before.and_then(|at| had[at].button_at(pointer));
            let known = had.iter().position(|prompt| {
                prompt.output == head.output
                    && prompt.scale == head.scale
                    && prompt.area == head.area()
                    && prompt.what == what
                    && prompt.left == left
                    && prompt.theme == theme
                    && prompt.hovered == hovered
                    && prompt.focus == focus
            });
            let prompt = known.map(|at| had.swap_remove(at));
            let prompt = prompt.unwrap_or_else(|| {
                let asked = (what.as_str(), left, (hovered, focus));
                let tools = (&mut self.text, &self.words);
                let (canvas, buttons) = paint_keep(tools, &theme, asked, head.scale);
                let (width, height) = canvas.units();
                let area = head.area();
                let middle = |room: i32, length: u32| (room - length.cast_signed()) / 2;
                let within = (middle(area.size.w, width), middle(area.size.h, height));
                let at = area.loc + Point::from(within);
                let on = |(x, y, wide, high): Area| {
                    let origin = DomainPoint::new(at.x + x, at.y + y);
                    Rect::new(origin, DomainSize::new(wide, high))
                };
                Prompt {
                    output: head.output.clone(),
                    at,
                    scale: head.scale,
                    theme: theme.clone(),
                    area,
                    what: what.clone(),
                    left,
                    hovered,
                    focus,
                    whole: on((0, 0, width, height)),
                    buttons: [on(buttons.0), on(buttons.1)],
                    pixels: Painted::new(&canvas),
                }
            });
            prompts.push(prompt);
        }
        self.interim.prompts = prompts;
    }
}

/// Paints the question whether to keep the screens as they stand now,
/// saying what that is and how many seconds are `left` before they go back
/// as they were, the button `hovered` lit and the one the keyboard is on,
/// `focus`, as such; and says where its buttons are on it: Put back, then
/// Keep, which is filled with the accent.
fn paint_keep(
    (text, words): (&mut Text, &Words),
    theme: &Theme,
    (what, left, (hovered, focus)): (&str, u64, (Option<usize>, usize)),
    scale: Scale,
) -> (Canvas, (Area, Area)) {
    let question = words.say("keep-screens", &[]);
    let left = i64::try_from(left).unwrap_or(i64::MAX);
    let detail_font = theme.font(Part::QuestionDetail, States::NONE);
    let inset = theme.style(Part::Question, States::NONE).inset();
    let mut lines = text.wrap(what, WIDTH.saturating_sub(inset.across()), detail_font, 2);
    lines.push(words.say("screens-back-in", &[("left", Value::Number(left))]));
    let (put_back, keep) = (words.say("put-back", &[]), words.say("keep", &[]));
    let asked = Asked {
        question: &question,
        lines: &lines,
        room: 0,
        buttons: [put_back.as_str(), keep.as_str()],
    };
    let laid = omorfos_ui::question::lay_out(text, theme, WIDTH, &asked);
    let states = [PUT_BACK, KEEP].map(|button| {
        States::NONE
            .when(State::Hovered, hovered == Some(button))
            .when(State::Active, button == KEEP)
            .when(State::Focused, button == focus)
    });
    let canvas = omorfos_ui::question::paint_question(text, theme, (&asked, &laid), states, scale);
    let [put_back, keep] = laid.buttons;
    (canvas, (put_back, keep))
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
