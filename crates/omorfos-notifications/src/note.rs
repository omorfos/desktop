//! The notifications on the screen: which there are, how long each stays,
//! how they are drawn and what a click does.
//!
//! Nothing here knows of D-Bus or of Wayland. Time is whatever whoever
//! calls says it is, counted from when the program started.

use std::time::Duration;

use omorfos_config::{Edges, Part, State, States, Style, Theme};
use omorfos_icons::Icons;
use omorfos_ui::{Area, Canvas, Image, Motion, Scale, Text, inset};

/// The most notifications kept at once. When there are more, the oldest
/// goes.
const MOST: usize = 8;

/// The most lines of a notification's body that are shown.
const LINES: usize = 3;

/// The most lines that are shown of something that is wrong, which is
/// worth more room: it is often a file, a place in it and a reason.
const PROBLEM_LINES: usize = 6;

/// What a notification about something that is wrong is headed.
const OURS: &str = "omorfos";

/// What sets the notifications about what is wrong apart from the numbers
/// programs give theirs, which are no larger than thirty-two bits.
const APART: u64 = 1 << 32;

/// The most of a title or of a line of body that is kept, in characters.
/// Only as much as fits is drawn; this keeps what is not drawn from being
/// carried about.
const LONGEST: usize = 400;

/// How large a notification's picture is, in units of layout.
const PICTURE: u32 = 40;

/// The room between a notification's picture and its words.
const GAP: u32 = 12;

/// How thick the bar is that shows how full something is.
const LEVEL: u32 = 6;

/// The narrowest a notification is made, whatever the theme says.
const NARROWEST: u32 = 120;

/// The picture that goes with a notification.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Picture {
    /// It has none.
    #[default]
    None,
    /// An icon, by name or by the path of its file.
    Named(String),
    /// The picture itself.
    Pixels(Image),
}

/// A notification, as a program sent it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Note {
    /// The number it goes by. One with the number of a notification that
    /// is showing takes its place.
    pub id: u32,
    /// What it is about, in a few words.
    pub title: String,
    /// The rest of what it says. It may have markup in it, which is taken
    /// out.
    pub body: String,
    /// The picture that goes with it.
    pub picture: Picture,
    /// Whether the program marked it as critical.
    pub urgent: bool,
    /// How long it stays, in milliseconds: `-1` for as long as
    /// notifications usually do, `0` for until it is dismissed.
    pub stays: i32,
    /// Whether its sender wants to hear of it being clicked, so as to do
    /// something then, such as showing the window it is about.
    pub acts: bool,
    /// How full something is, out of 100, such as the volume, shown as a
    /// bar under what it says.
    pub level: Option<u8>,
    /// What it is one of, such as the volume: one with the tag of a
    /// notification that is showing takes its place, whatever its number,
    /// so that a program that does not remember the number of the one it
    /// sent before still shows one at a time.
    pub tag: Option<String>,
}

/// Why a notification is no longer showing, as the numbers programs are
/// told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Its time ran out.
    Expired = 1,
    /// The user dismissed it.
    Dismissed = 2,
    /// The program that sent it took it back.
    Closed = 3,
}

/// A notification that is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shown {
    /// What tells it from every other that is showing.
    key: u64,
    /// The number its program gave it, if it is a program's.
    id: u32,
    /// Whether it is from omorfos itself and not from a program.
    ours: bool,
    /// What is wrong, as the compositor put it, if it is about something
    /// that stays wrong until it is put right.
    problem: Option<String>,
    title: String,
    /// What it says, as its sender broke it into lines.
    said: Vec<String>,
    /// What it says, as it is drawn: broken again to fit, if it has been
    /// fitted to a theme.
    lines: Vec<String>,
    picture: Picture,
    urgent: bool,
    acts: bool,
    level: Option<u8>,
    tag: Option<String>,
    /// When it goes, if it does of its own accord.
    until: Option<Duration>,
}

/// The notifications that are showing, the newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    shown: Vec<Shown>,
    /// How long one stays when the program that sent it does not say.
    /// Nothing is for until it is dismissed.
    usual: Option<Duration>,
    /// The one the pointer is over.
    under: Option<u64>,
    /// What is wrong that the user has dismissed, and is still wrong.
    dismissed: Vec<String>,
    /// What has happened that has been shown, and that the compositor is
    /// still saying has.
    seen: Vec<String>,
    /// How many times omorfos itself has had something shown.
    told: u32,
}

impl Stack {
    /// No notifications yet. One whose sender does not say how long it
    /// stays, stays `seconds`, or until it is dismissed if that is 0.
    #[must_use]
    pub fn new(seconds: u32) -> Self {
        let usual = Duration::from_secs(u64::from(seconds));
        Self {
            shown: Vec::new(),
            usual: (seconds > 0).then_some(usual),
            under: None,
            dismissed: Vec::new(),
            seen: Vec::new(),
            told: 0,
        }
    }

    /// Changes how long a notification stays when the program that sent
    /// it does not say, as [`Stack::new`] has it. Those showing keep the
    /// time they were given.
    pub fn set_usual(&mut self, seconds: u32) {
        self.usual = Self::new(seconds).usual;
    }

    /// Whether there is nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    /// How many notifications are showing.
    #[must_use]
    pub fn len(&self) -> usize {
        self.shown.len()
    }

    /// Shows a notification, at the time `now`. If one with its number is
    /// showing, it takes that one's place. Returns the number of the one
    /// that had to go to make room, if any did.
    pub fn show(&mut self, note: Note, now: Duration) -> Option<u32> {
        let stays = u64::try_from(note.stays).ok().map(Duration::from_millis);
        // A critical one stays until it is seen to, unless its sender set
        // a time itself.
        let usual = self.usual.filter(|_| !note.urgent);
        let lasts = stays.map_or(usual, |stays| Some(stays).filter(|stays| !stays.is_zero()));
        let said = plain(&note.body);
        let said = said.lines().map(str::trim).filter(|line| !line.is_empty());
        let said: Vec<String> = said.take(LINES).map(clipped).collect();
        let shown = Shown {
            key: u64::from(note.id),
            id: note.id,
            ours: false,
            problem: None,
            title: clipped(plain(&note.title).trim()),
            lines: said.clone(),
            said,
            picture: note.picture,
            urgent: note.urgent,
            acts: note.acts,
            level: note.level.map(|level| level.min(100)),
            tag: note.tag,
            until: lasts.map(|lasts| now.saturating_add(lasts)),
        };
        let tagged = |other: &Shown| shown.tag.is_some() && other.tag == shown.tag;
        let place = self
            .shown
            .iter()
            .position(|other| other.key == shown.key || tagged(other));
        match place.and_then(|place| self.shown.get_mut(place)) {
            Some(other) => *other = shown,
            None => self.shown.insert(0, shown),
        }
        // Room is made by a program's notification going, never by one
        // about something that is wrong.
        let theirs = |shown: &Shown| !shown.ours;
        let crowded = self.shown.iter().filter(|shown| theirs(shown)).count() > MOST;
        let oldest = self.shown.iter().rposition(theirs);
        let gone = oldest.filter(|_| crowded);
        gone.map(|oldest| self.shown.remove(oldest).id)
    }

    /// Takes in, at the time `now`, what the compositor has to say of the
    /// session: what is wrong with it, and what has just happened in it.
    ///
    /// Each thing that is wrong is shown as a notification that stays
    /// until it is dismissed or put right, and is not shown again once
    /// dismissed unless it is put right and goes wrong again. Each thing
    /// that has happened is shown once, for as long as notifications
    /// usually stay. Returns whether that changed what is showing.
    pub fn tell(&mut self, problems: &[String], events: &[String], now: Duration) -> bool {
        self.dismissed.retain(|known| problems.contains(known));
        self.seen.retain(|known| events.contains(known));
        let before = self.shown.len();
        let still = |shown: &Shown| {
            shown
                .problem
                .as_ref()
                .is_none_or(|it| problems.contains(it))
        };
        self.shown.retain(still);
        let mut changed = self.shown.len() != before;
        // As many of each as there is room for of a program's
        // notifications.
        for problem in problems.iter().take(MOST) {
            let mut showing = self.shown.iter();
            let showing = showing.any(|shown| shown.problem.as_ref() == Some(problem));
            if !showing && !self.dismissed.contains(problem) {
                self.say(problem, Some(problem.clone()), None);
                changed = true;
            }
        }
        let lasts = self.usual.map(|usual| now.saturating_add(usual));
        for event in events.iter().take(MOST) {
            if !self.seen.contains(event) {
                self.seen.push(event.clone());
                self.say(event, None, lasts);
                changed = true;
            }
        }
        changed
    }

    /// Shows something omorfos itself has to say, above everything else:
    /// about a `problem`, which is urgent, or not, and staying `until` a
    /// time if it goes by itself.
    fn say(&mut self, words: &str, problem: Option<String>, until: Option<Duration>) {
        self.told = self.told.wrapping_add(1);
        let said = vec![clipped(words.trim())];
        let shown = Shown {
            key: APART | u64::from(self.told),
            id: 0,
            ours: true,
            urgent: problem.is_some(),
            problem,
            title: OURS.to_owned(),
            lines: said.clone(),
            said,
            picture: Picture::None,
            acts: false,
            level: None,
            tag: None,
            until,
        };
        self.shown.insert(0, shown);
    }

    /// Breaks what each notification says into lines that fit it under
    /// `theme`, at the spaces between words, keeping the breaks its sender
    /// made. To be called before the notifications are measured or drawn,
    /// when what is showing or the theme has changed.
    pub fn fit(&mut self, theme: &Theme, text: &mut Text) {
        let fitted = self.shown.iter().map(|shown| {
            let states = self.states(shown);
            let style = theme.style(Part::Notification, states);
            let font = theme.font(Part::NotificationBody, states);
            let pictured = shown.picture != Picture::None;
            let taken = style.inset().across() + if pictured { PICTURE + GAP } else { 0 };
            let room = width(&style).saturating_sub(taken);
            let mut lines: Vec<String> = Vec::new();
            for words in &shown.said {
                // Once the lines there may be are full, the rest is left
                // out.
                let most = if shown.ours { PROBLEM_LINES } else { LINES };
                let left = most.saturating_sub(lines.len());
                if left > 0 {
                    lines.extend(text.wrap(words, room, font, left));
                }
            }
            lines
        });
        let fitted: Vec<Vec<String>> = fitted.collect();
        for (shown, lines) in self.shown.iter_mut().zip(fitted) {
            shown.lines = lines;
        }
    }

    /// Takes a notification away. Returns whether it was showing.
    pub fn close(&mut self, id: u32) -> bool {
        let before = self.shown.len();
        // The numbers programs know are those of their own.
        self.shown.retain(|shown| shown.key != u64::from(id));
        self.shown.len() != before
    }

    /// Takes away the notifications whose time has run out by `now`, and
    /// returns the numbers of those that were programs'.
    pub fn expire(&mut self, now: Duration) -> Vec<u32> {
        let due = |shown: &Shown| shown.until.is_some_and(|until| until <= now);
        let gone = self.shown.extract_if(.., |shown| due(shown));
        // Programs are told of their own, and only of their own.
        let theirs = gone.filter(|shown| !shown.ours);
        theirs.map(|shown| shown.id).collect()
    }

    /// When the next notification's time runs out, if any will.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        self.shown.iter().filter_map(|shown| shown.until).min()
    }

    /// How large the notifications are together under `theme`, in units of
    /// layout, with the room kept around each.
    #[must_use]
    pub fn size(&self, theme: &Theme) -> (u32, u32) {
        let places = self.places(theme);
        let style = theme.style(Part::Notification, States::NONE);
        let width = width(&style).saturating_add(style.margin.across());
        let bottom = places.last().map_or(0, |(_, area)| {
            let below = area.1.saturating_add_unsigned(area.3);
            below.saturating_add_unsigned(style.margin.bottom)
        });
        (width, bottom.unsigned_abs())
    }

    /// Notes where the pointer is, so that the notification under it can
    /// be drawn as such. Returns whether that changed anything.
    pub fn point(&mut self, x: i32, y: i32, theme: &Theme) -> bool {
        let under = self.at(x, y, theme);
        let changed = under != self.under;
        self.under = under;
        changed
    }

    /// Notes that the pointer has left. Returns whether that changed
    /// anything.
    pub fn leave(&mut self) -> bool {
        self.under.take().is_some()
    }

    /// Takes a click: the notification at that point is dismissed.
    /// Returns its number, if there was a program's there, and whether its
    /// sender wants to hear of the click. One about something that is
    /// wrong is not shown again while that stays wrong.
    pub fn click(&mut self, x: i32, y: i32, theme: &Theme) -> Option<(u32, bool)> {
        let under = self.at(x, y, theme);
        let place = under.and_then(|key| self.shown.iter().position(|shown| shown.key == key));
        let clicked = place.map(|place| self.shown.remove(place));
        let problem = clicked.as_ref().and_then(|shown| shown.problem.clone());
        self.dismissed.extend(problem);
        let theirs = clicked.filter(|shown| !shown.ours);
        theirs.map(|shown| (shown.id, shown.acts))
    }

    /// Draws the notifications, with `scale` pixels to a unit of layout.
    /// Pictures that are named are looked up in `icons`. A notification
    /// the pointer has just come onto or left is as far on its way to its
    /// new colours as `motion` has it.
    #[must_use]
    pub fn paint(
        &self,
        text: &mut Text,
        theme: &Theme,
        scale: Scale,
        icons: &mut Icons,
        motion: &mut Motion,
    ) -> Canvas {
        let (width, height) = self.size(theme);
        let mut canvas = Canvas::scaled(width, height, scale);
        for (shown, area) in self.places(theme) {
            let states = self.states(shown);
            let key = shown.key;
            let style = motion.style(theme, Part::Notification, key, states);
            canvas.draw_box(area, &style);
            let mut room = inset(area, style.inset());
            let pixels = scale.pixels(PICTURE.cast_signed()).unsigned_abs();
            let picture = match &shown.picture {
                Picture::None => None,
                Picture::Named(name) => icons.get(name, pixels, style.color),
                Picture::Pixels(image) => Some(image),
            };
            if let Some(picture) = picture {
                canvas.draw_image((room.0, room.1, PICTURE, PICTURE), picture);
                room = inset(room, on_the_left(PICTURE + GAP));
            }
            let title = motion.style(theme, Part::NotificationTitle, key, states);
            let font = theme.font(Part::NotificationTitle, states);
            let tall = line(font.size.get());
            text.draw(
                &mut canvas,
                &shown.title,
                (room.0, room.1, room.2, tall),
                title.color,
                font,
            );
            let body = motion.style(theme, Part::NotificationBody, key, states);
            let font = theme.font(Part::NotificationBody, states);
            let mut top = room.1.saturating_add_unsigned(tall);
            for words in &shown.lines {
                let tall = line(font.size.get());
                text.draw(
                    &mut canvas,
                    words,
                    (room.0, top, room.2, tall),
                    body.color,
                    font,
                );
                top = top.saturating_add_unsigned(tall);
            }
            if let Some(level) = shown.level {
                let style = motion.style(theme, Part::NotificationLevel, key, states);
                let track = (
                    room.0,
                    top.saturating_add_unsigned(style.margin.top),
                    room.2,
                    LEVEL,
                );
                canvas.draw_box(track, &style);
                // How full it is, in the part's colour over its background.
                let full = room.2.saturating_mul(u32::from(level)) / 100;
                let filled = Style {
                    background: style.color,
                    ..style
                };
                canvas.draw_box((track.0, track.1, full, track.3), &filled);
            }
        }
        canvas
    }

    /// The states a notification is in.
    fn states(&self, shown: &Shown) -> States {
        let urgent = States::NONE.when(State::Urgent, shown.urgent);
        urgent.when(State::Hovered, self.under == Some(shown.key))
    }

    /// Where each notification's box is, from the top.
    fn places(&self, theme: &Theme) -> Vec<(&Shown, Area)> {
        let mut top = 0_i32;
        let placed = self.shown.iter().map(|shown| {
            let states = self.states(shown);
            let style = theme.style(Part::Notification, states);
            let title = line(theme.font(Part::NotificationTitle, states).size.get());
            let body = line(theme.font(Part::NotificationBody, states).size.get());
            let lines = u32::try_from(shown.lines.len()).unwrap_or(0);
            let words = title.saturating_add(body.saturating_mul(lines));
            let level = theme.style(Part::NotificationLevel, states);
            let level = LEVEL.saturating_add(level.margin.down());
            let words = words.saturating_add(if shown.level.is_some() { level } else { 0 });
            let pictured = shown.picture != Picture::None;
            let content = words.max(if pictured { PICTURE } else { 0 });
            let height = content.saturating_add(style.inset().down());
            let area = (
                style.margin.left.cast_signed(),
                top.saturating_add_unsigned(style.margin.top),
                width(&style),
                height,
            );
            top = area.1.saturating_add_unsigned(height);
            top = top.saturating_add_unsigned(style.margin.bottom);
            (shown, area)
        });
        placed.collect()
    }

    /// The notification whose box a point is in.
    fn at(&self, x: i32, y: i32, theme: &Theme) -> Option<u64> {
        let places = self.places(theme);
        let inside = |(left, top, width, height): Area| {
            let across = u32::try_from(x.saturating_sub(left)).ok();
            let down = u32::try_from(y.saturating_sub(top)).ok();
            across
                .zip(down)
                .is_some_and(|(across, down)| across < width && down < height)
        };
        let found = places.into_iter().find(|(_, area)| inside(*area));
        found.map(|(shown, _)| shown.key)
    }
}

/// How wide a notification's box is.
fn width(style: &Style) -> u32 {
    style.min_width.max(NARROWEST)
}

/// How tall a line of text of a size in points is, in units of layout.
fn line(points: u32) -> u32 {
    points.saturating_mul(5) / 3
}

/// Room taken from the left of a box and from nowhere else.
const fn on_the_left(room: u32) -> Edges<u32> {
    Edges {
        left: room,
        ..Edges::NONE
    }
}

/// As much of a line as is kept.
fn clipped(words: &str) -> String {
    words.chars().take(LONGEST).collect()
}

/// What a program wrote, without the markup programs put in notifications:
/// tags are taken out, `<br>` is a new line, and the few characters that
/// have to be written another way inside markup are written plainly.
fn plain(marked: &str) -> String {
    let mut text = String::new();
    let mut rest = marked;
    while let Some(start) = rest.find('<') {
        let (before, tag) = rest.split_at(start);
        text.push_str(before);
        let Some(end) = tag.find('>') else {
            // Not a tag after all.
            rest = tag;
            break;
        };
        let name = tag.get(1..end).unwrap_or_default();
        let name = name.trim_matches(['/', ' ']).to_ascii_lowercase();
        if name == "br" {
            text.push('\n');
        }
        rest = tag.get(end + 1..).unwrap_or_default();
    }
    text.push_str(rest);
    let written = [
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&amp;", "&"),
    ];
    written
        .iter()
        .fold(text, |text, (coded, meant)| text.replace(coded, meant))
}

#[cfg(test)]
mod tests {
    use omorfos_config::Color;

    use super::*;

    const START: Duration = Duration::ZERO;

    fn seconds(count: u64) -> Duration {
        Duration::from_secs(count)
    }

    fn note(id: u32, title: &str, body: &str) -> Note {
        Note {
            id,
            title: title.to_owned(),
            body: body.to_owned(),
            stays: -1,
            ..Note::default()
        }
    }

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    fn painted(stack: &Stack, theme: &Theme) -> Canvas {
        let (mut text, mut icons) = (Text::new(), Icons::default());
        stack.paint(&mut text, theme, Scale::ONE, &mut icons, &mut Motion::new())
    }

    /// The numbers of the notifications showing, from the top.
    fn showing(stack: &Stack) -> Vec<u32> {
        stack.shown.iter().map(|shown| shown.id).collect()
    }

    #[test]
    fn the_newest_notification_is_shown_first() {
        let mut stack = Stack::new(5);
        assert!(stack.is_empty());
        assert_eq!(stack.show(note(1, "First", ""), START), None);
        assert_eq!(stack.show(note(2, "Second", ""), START), None);
        assert!(!stack.is_empty());
        assert_eq!(showing(&stack), [2, 1]);
    }

    #[test]
    fn a_notification_with_the_number_of_one_showing_takes_its_place() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "Downloading", "10%"), START);
        stack.show(note(2, "Mail", ""), START);
        stack.show(note(1, "Downloading", "90%"), seconds(3));
        assert_eq!(showing(&stack), [2, 1], "where the old one was");
        assert_eq!(stack.shown[1].lines, ["90%"]);
        // And its time starts again.
        assert_eq!(stack.expire(seconds(6)), [2]);
        assert_eq!(stack.expire(seconds(8)), [1]);
    }

    #[test]
    fn a_notification_goes_when_its_time_runs_out() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "Usual", ""), START);
        let brief = Note {
            stays: 1500,
            ..note(2, "Brief", "")
        };
        stack.show(brief, START);
        let lasting = Note {
            stays: 0,
            ..note(3, "Lasting", "")
        };
        stack.show(lasting, START);
        assert_eq!(stack.deadline(), Some(Duration::from_millis(1500)));
        assert_eq!(stack.expire(seconds(1)), [0; 0]);
        assert_eq!(stack.expire(seconds(2)), [2]);
        assert_eq!(stack.deadline(), Some(seconds(5)));
        assert_eq!(stack.expire(seconds(5)), [1]);
        // The one that said to stay does.
        assert_eq!(stack.deadline(), None);
        assert_eq!(stack.expire(seconds(5000)), [0; 0]);
        assert_eq!(showing(&stack), [3]);
    }

    #[test]
    fn a_critical_notification_stays_until_it_is_seen_to() {
        let mut stack = Stack::new(5);
        let urgent = Note {
            urgent: true,
            ..note(1, "Battery low", "")
        };
        stack.show(urgent.clone(), START);
        assert_eq!(stack.deadline(), None);
        // Unless whoever sent it gave it a time.
        let timed = Note {
            stays: 2000,
            ..urgent
        };
        stack.show(timed, START);
        assert_eq!(stack.deadline(), Some(seconds(2)));
    }

    #[test]
    fn with_no_usual_time_notifications_stay_until_dismissed() {
        let mut stack = Stack::new(0);
        stack.show(note(1, "Stays", ""), START);
        assert_eq!(stack.deadline(), None);
        let brief = Note {
            stays: 1000,
            ..note(2, "Brief", "")
        };
        stack.show(brief, START);
        assert_eq!(stack.deadline(), Some(seconds(1)));
    }

    #[test]
    fn the_usual_time_can_be_changed_for_those_to_come() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "Before", ""), START);
        stack.set_usual(60);
        stack.show(note(2, "After", ""), START);
        assert_eq!(
            stack.expire(seconds(5)),
            [1],
            "it keeps the time it was given"
        );
        assert_eq!(stack.deadline(), Some(seconds(60)));
        stack.set_usual(0);
        stack.show(note(3, "Later", ""), START);
        assert_eq!(stack.expire(seconds(60)), [2]);
        assert_eq!(stack.deadline(), None);
    }

    #[test]
    fn a_program_can_take_its_notification_back() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "One", ""), START);
        stack.show(note(2, "Two", ""), START);
        assert!(stack.close(1));
        assert!(!stack.close(1), "it is no longer showing");
        assert!(!stack.close(99));
        assert_eq!(showing(&stack), [2]);
    }

    #[test]
    fn only_so_many_are_kept_and_the_oldest_makes_room() {
        let mut stack = Stack::new(5);
        for id in 1..=8 {
            assert_eq!(stack.show(note(id, "One of many", ""), START), None);
        }
        assert_eq!(stack.show(note(9, "One too many", ""), START), Some(1));
        assert_eq!(showing(&stack), [9, 8, 7, 6, 5, 4, 3, 2]);
        // One that takes another's place needs no room made.
        assert_eq!(stack.show(note(5, "Again", ""), START), None);
    }

    #[test]
    fn markup_is_taken_out_of_what_a_program_wrote() {
        assert_eq!(plain("plain"), "plain");
        assert_eq!(plain("<b>Bold</b> and <i>slanted</i>"), "Bold and slanted");
        assert_eq!(
            plain("one<br>two<BR/>three<br />four"),
            "one\ntwo\nthree\nfour"
        );
        assert_eq!(
            plain("<a href=\"https://example.org\">a link</a>"),
            "a link"
        );
        assert_eq!(plain("1 &lt; 2 &amp;&amp; 3 &gt; 2"), "1 < 2 && 3 > 2");
        assert_eq!(plain("&quot;said&quot; &apos;so&apos;"), "\"said\" 'so'");
        assert_eq!(plain("&amp;lt; stays as written"), "&lt; stays as written");
        assert_eq!(plain("a < b"), "a < b", "not every < begins a tag");
        assert_eq!(plain("a <b>c</b> < d"), "a c < d");
        assert_eq!(plain(""), "");
    }

    #[test]
    fn a_body_is_shown_as_a_few_lines_and_no_more_is_kept_than_can_be_seen() {
        let mut stack = Stack::new(5);
        let body = "  first  \n\nsecond<br>third\nfourth\nfifth";
        stack.show(note(1, " <b>Title</b> ", body), START);
        assert_eq!(stack.shown[0].title, "Title");
        assert_eq!(stack.shown[0].lines, ["first", "second", "third"]);
        let long = "x".repeat(5000);
        stack.show(note(2, &long, &long), START);
        assert_eq!(stack.shown[0].title.chars().count(), LONGEST);
        assert_eq!(stack.shown[0].lines[0].chars().count(), LONGEST);
    }

    #[test]
    fn a_long_body_is_broken_to_fit_once_it_is_fitted_to_a_theme() {
        let theme = Theme::default();
        let mut text = Text::new();
        let mut stack = Stack::new(5);
        let long = "The quick brown fox jumps over the lazy dog and keeps on running \
                    through the fields until it is quite out of sight";
        stack.show(note(1, "Long", long), START);
        assert_eq!(stack.shown[0].lines.len(), 1, "as its sender wrote it");
        let before = stack.size(&theme).1;

        stack.fit(&theme, &mut text);

        let lines = stack.shown[0].lines.clone();
        assert!(lines.len() > 1 && lines.len() <= LINES, "{lines:?}");
        assert_eq!(
            lines.join(" "),
            long.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        let more = u32::try_from(lines.len() - 1).unwrap();
        assert_eq!(stack.size(&theme).1, before + 18 * more);
        // Each line but the last fits the room there is for words.
        let style = theme.style(Part::Notification, States::NONE);
        let room = 340 - style.inset().across();
        let font = theme.font(Part::NotificationBody, States::NONE);
        for line in &lines[..lines.len() - 1] {
            assert!(text.measure(line, font) <= room, "{line:?}");
        }

        // The breaks its sender made are kept, and there are still only so
        // many lines; a picture leaves less room for words.
        stack.show(note(2, "Broken", "one\ntwo\nthree\nfour"), START);
        stack.fit(&theme, &mut text);
        assert_eq!(stack.shown[0].lines, ["one", "two", "three"]);
        let picture = Image::new(1, 1, vec![0, 0, 0, 255]).unwrap();
        let pictured = Note {
            picture: Picture::Pixels(picture),
            ..note(1, "Long", long)
        };
        stack.show(pictured, START);
        stack.fit(&theme, &mut text);
        let narrower = &stack.shown[1].lines;
        assert!(text.measure(&narrower[0], font) <= room - PICTURE - GAP);
        // What its sender put on a later line is left out when the lines
        // before it have filled the room.
        stack.show(
            note(3, "Both", &format!("{long} {long}\nand a last line")),
            START,
        );
        stack.fit(&theme, &mut text);
        assert_eq!(stack.shown[0].lines.len(), LINES);
        assert!(!stack.shown[0].lines.concat().contains("last line"));
        // Fitted again with nothing changed, it comes out the same.
        let once = stack.clone();
        stack.fit(&theme, &mut text);
        assert_eq!(stack, once);
    }

    fn level(id: u32, tag: Option<&str>, level: u8) -> Note {
        Note {
            level: Some(level),
            tag: tag.map(str::to_owned),
            ..note(id, "Volume", "")
        }
    }

    #[test]
    fn a_notification_with_the_tag_of_one_showing_takes_its_place_whatever_its_number() {
        let mut stack = Stack::new(5);
        stack.show(level(1, Some("volume"), 40), START);
        stack.show(note(2, "Mail", ""), START);
        stack.show(level(3, Some("volume"), 45), seconds(3));
        assert_eq!(showing(&stack), [2, 3], "where the old one was");
        assert_eq!(stack.shown[1].level, Some(45));
        // Another tag, or none, is one more.
        stack.show(level(4, Some("brightness"), 10), START);
        stack.show(level(5, None, 10), START);
        stack.show(level(6, None, 20), START);
        assert_eq!(showing(&stack), [6, 5, 4, 2, 3]);
    }

    #[test]
    fn a_level_is_drawn_as_a_bar_under_the_words_full_as_far_as_it_goes() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(level(1, Some("volume"), 50), START);
        // A line of title, and the bar under it with its room above.
        let height = 8 + 1 + 10 + 18 + 8 + 6 + 10 + 1 + 8;
        assert_eq!(stack.size(&theme).1, height);
        let style = theme.style(Part::NotificationLevel, States::NONE);
        let canvas = painted(&stack, &theme);
        let (left, width, middle) = (8 + 1 + 12, 340 - 2 - 24, 8 + 1 + 10 + 18 + 8 + 3);
        assert_eq!(
            canvas.at(left + width / 4, middle),
            argb(style.color),
            "full"
        );
        assert_eq!(
            canvas.at(left + width * 3 / 4, middle),
            argb(style.background)
        );

        // More than full is full.
        stack.show(level(1, Some("volume"), 150), START);
        let canvas = painted(&stack, &theme);
        assert_eq!(canvas.at(left + width * 3 / 4, middle), argb(style.color));
        // And nothing is nothing.
        stack.show(level(1, Some("volume"), 0), START);
        let canvas = painted(&stack, &theme);
        assert_eq!(canvas.at(left + width / 4, middle), argb(style.background));
    }

    #[test]
    fn notifications_are_stacked_with_room_around_each() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        assert_eq!(stack.size(&theme), (340 + 16, 0), "nothing to show");
        stack.show(note(1, "Title alone", ""), START);
        // A margin above and below, a border and padding inside, and one
        // line of title.
        let one = 8 + 1 + 10 + 18 + 10 + 1 + 8;
        assert_eq!(stack.size(&theme), (356, one));
        stack.show(note(2, "With a body", "one\ntwo"), START);
        assert_eq!(stack.size(&theme), (356, one + one + 2 * 18));

        let canvas = painted(&stack, &theme);
        assert_eq!((canvas.width(), canvas.height()), stack.size(&theme));
        let surface = argb(theme.style(Part::Notification, States::NONE).background);
        assert_eq!(canvas.at(4, 30), 0, "the margin is clear");
        assert_eq!(canvas.at(352, 30), 0);
        assert_eq!(canvas.at(340, 15), surface);
        let between = one + 2 * 18 - 4;
        assert_eq!(canvas.at(178, between), 0, "and so is the gap between two");
        assert_eq!(canvas.at(340, between + 16), surface);
    }

    #[test]
    fn a_narrow_theme_still_leaves_room_to_read() {
        let theme = Theme::parse("notification { min-width: 10px; margin: 0; }").unwrap();
        let mut stack = Stack::new(5);
        stack.show(note(1, "Title", ""), START);
        assert_eq!(stack.size(&theme).0, NARROWEST);
    }

    #[test]
    fn a_click_dismisses_the_notification_it_is_on() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(note(1, "Lower", ""), START);
        stack.show(note(2, "Upper", ""), START);
        assert_eq!(stack.click(4, 30, &theme), None, "in the margin");
        assert_eq!(stack.click(-5, 30, &theme), None);
        assert_eq!(stack.click(100, 5000, &theme), None);
        assert_eq!(stack.click(100, 80, &theme), Some((1, false)));
        assert_eq!(stack.click(100, 80, &theme), None, "nothing is there now");
        assert_eq!(stack.click(100, 20, &theme), Some((2, false)));
        assert!(stack.is_empty());
        // One whose sender wants to hear of the click says so.
        let acting = Note {
            acts: true,
            ..note(3, "Click to open", "")
        };
        stack.show(acting, START);
        assert_eq!(stack.click(100, 20, &theme), Some((3, true)));
    }

    #[test]
    fn the_notification_under_the_pointer_and_a_critical_one_are_marked() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        let urgent = Note {
            urgent: true,
            ..note(1, "Critical", "")
        };
        stack.show(urgent, START);
        stack.show(note(2, "Plain", ""), START);
        let edge = |stack: &Stack, y: u32| painted(stack, &theme).at(8, y);
        let border = |states| argb(theme.style(Part::Notification, states).border.left.color);
        let plain = border(States::NONE);
        assert_eq!(edge(&stack, 30), plain);
        assert_eq!(edge(&stack, 90), border(States::NONE.with(State::Urgent)));

        assert!(stack.point(100, 30, &theme));
        assert!(!stack.point(101, 31, &theme), "still the same one");
        assert_eq!(edge(&stack, 30), border(States::NONE.with(State::Hovered)));
        assert!(stack.point(100, 4, &theme), "off it, into the margin");
        assert_eq!(edge(&stack, 30), plain);
        assert!(stack.point(100, 30, &theme));
        assert!(stack.leave());
        assert!(!stack.leave());
        assert_eq!(edge(&stack, 30), plain);
    }

    #[test]
    fn a_picture_is_drawn_beside_the_words_and_makes_room_for_itself() {
        let theme = Theme::default();
        let green = [0, 255, 0, 255].repeat(16);
        let picture = Image::new(4, 4, green).unwrap();
        let mut stack = Stack::new(5);
        let pictured = Note {
            picture: Picture::Pixels(picture),
            ..note(1, "With a picture", "")
        };
        stack.show(pictured, START);
        // Taller than its one line of words: as tall as the picture.
        assert_eq!(stack.size(&theme).1, 8 + 1 + 10 + 40 + 10 + 1 + 8);
        let canvas = painted(&stack, &theme);
        let left = 8 + 1 + 12;
        assert_eq!(canvas.at(left, 19), 0xff00_ff00);
        assert_eq!(canvas.at(left + 39, 19 + 39), 0xff00_ff00);
        let surface = argb(theme.style(Part::Notification, States::NONE).background);
        let gap = left + 40..left + 40 + GAP;
        assert!(gap.into_iter().all(|x| canvas.at(x, 40) == surface));

        // One named but not to be found is left out, and takes no room.
        let named = Note {
            picture: Picture::Named("no-such-icon".to_owned()),
            ..note(1, "With a name", "")
        };
        stack.show(named, START);
        let canvas = painted(&stack, &theme);
        let green = (0..canvas.width()).filter(|x| canvas.at(*x, 30) == 0xff00_ff00);
        assert_eq!(green.count(), 0);
    }

    /// What is wrong that is showing, from the top.
    fn wrong(stack: &Stack) -> Vec<&str> {
        let wrong = stack
            .shown
            .iter()
            .filter_map(|shown| shown.problem.as_deref());
        wrong.collect()
    }

    fn all(them: &[&str]) -> Vec<String> {
        them.iter().map(|one| (*one).to_owned()).collect()
    }

    /// Tells a stack what is wrong, with nothing having happened.
    fn tell(stack: &mut Stack, problems: &[&str]) -> bool {
        stack.tell(&all(problems), &[], START)
    }

    #[test]
    fn what_is_wrong_with_the_session_is_shown_until_it_is_put_right() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "From a program", ""), START);
        let two = ["keys.yaml: no such key", "theme.css:2:3: nope"];

        assert!(tell(&mut stack, &two));
        assert_eq!(wrong(&stack), [two[1], two[0]]);
        assert_eq!(stack.len(), 3, "above what programs have said");
        assert!(stack.shown[0].urgent);
        assert_eq!(stack.shown[0].title, "omorfos");
        assert!(!tell(&mut stack, &two));

        // However long it is left.
        assert_eq!(stack.expire(seconds(3600)), [1]);
        assert_eq!(stack.deadline(), None);
        assert_eq!(wrong(&stack).len(), 2);

        assert!(tell(&mut stack, &two[..1]));
        assert_eq!(wrong(&stack), [two[0]]);
        assert!(tell(&mut stack, &[]));
        assert!(stack.is_empty());
        assert!(!tell(&mut stack, &[]));
    }

    #[test]
    fn what_is_wrong_and_has_been_dismissed_stays_away_until_it_goes_wrong_again() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        let broken = ["keys.yaml: no such key"];
        tell(&mut stack, &broken);

        assert_eq!(stack.click(100, 20, &theme), None, "no program to tell");
        assert!(stack.is_empty());
        let again = tell(&mut stack, &broken);
        assert!(!again, "still wrong, and still dismissed");
        assert!(stack.is_empty());

        // Put right, and then wrong again.
        tell(&mut stack, &[]);
        assert!(tell(&mut stack, &broken));
        assert_eq!(wrong(&stack), broken);
    }

    #[test]
    fn programs_cannot_take_away_or_crowd_out_what_is_wrong() {
        let mut stack = Stack::new(5);
        tell(&mut stack, &["something is wrong"]);
        // The numbers programs use are not the ones these go by.
        for id in [0, 1, u32::MAX] {
            assert!(!stack.close(id));
        }
        stack.show(note(1, "Replaces nothing", ""), START);
        assert_eq!(wrong(&stack).len(), 1);

        // When there are too many, it is a program's that goes.
        let mut gone = Vec::new();
        for id in 2..=12 {
            gone.extend(stack.show(note(id, "Another", ""), START));
        }
        assert_eq!(gone, [1, 2, 3, 4]);
        assert_eq!(wrong(&stack), ["something is wrong"]);
        assert_eq!(stack.len(), 9);
    }

    #[test]
    fn something_wrong_is_given_more_lines_than_a_program_gets() {
        let theme = Theme::default();
        let long = "the quick brown fox jumps over the lazy dog ".repeat(12);
        let mut text = Text::new();

        let mut theirs = Stack::new(5);
        theirs.show(note(1, "Long", &long), START);
        theirs.fit(&theme, &mut text);
        let mut ours = Stack::new(5);
        tell(&mut ours, &[&long]);
        ours.fit(&theme, &mut text);

        assert_eq!(theirs.shown[0].lines.len(), 3);
        assert_eq!(ours.shown[0].lines.len(), 6);
        assert!(ours.size(&theme).1 > theirs.size(&theme).1);
    }

    #[test]
    fn only_so_much_that_is_wrong_is_shown_at_once() {
        let many: Vec<String> = (0..20).map(|n| format!("problem {n}")).collect();
        let mut stack = Stack::new(5);
        assert!(stack.tell(&many, &[], START));
        assert_eq!(wrong(&stack).len(), 8);
        assert_eq!(wrong(&stack)[7], "problem 0", "the first of them");
        assert!(stack.tell(&[], &many, START));
        assert_eq!(stack.len(), 8, "and of what has happened");
    }

    #[test]
    fn what_has_happened_is_shown_once_and_goes_by_itself() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        let happened = all(&["the X server has been started again"]);

        assert!(stack.tell(&[], &happened, seconds(100)));
        assert_eq!(stack.len(), 1);
        assert!(!stack.shown[0].urgent, "nothing is wrong");
        assert_eq!(stack.shown[0].title, "omorfos");
        assert_eq!(wrong(&stack), [] as [&str; 0]);
        assert_eq!(stack.deadline(), Some(seconds(105)));
        // The compositor goes on saying so for a while. It is not news.
        assert!(!stack.tell(&[], &happened, seconds(102)));

        // No program is told when it goes.
        assert_eq!(stack.expire(seconds(105)), [] as [u32; 0]);
        assert!(stack.is_empty());
        assert!(!stack.tell(&[], &happened, seconds(106)));
        assert!(stack.is_empty(), "it is not shown again");

        // Once the compositor has stopped saying so, the same thing
        // happening again is news.
        assert!(!stack.tell(&[], &[], seconds(110)));
        assert!(stack.tell(&[], &happened, seconds(200)));
        // It can be clicked away, and no program is told of that either.
        assert_eq!(stack.click(100, 20, &theme), None);
        assert!(stack.is_empty());
        assert!(!stack.tell(&[], &happened, seconds(201)));
    }

    #[test]
    fn what_has_happened_stays_if_notifications_stay_until_dismissed() {
        let mut stack = Stack::new(0);
        assert!(stack.tell(&[], &all(&["something happened"]), START));
        assert_eq!(stack.deadline(), None);
        assert_eq!(stack.expire(seconds(3600)), [] as [u32; 0]);
        assert_eq!(stack.len(), 1);
    }
}
