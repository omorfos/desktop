//! The notifications on the screen: which there are, how long each stays,
//! how they are drawn and what a click does.
//!
//! Nothing here knows of D-Bus or of Wayland. Time is whatever whoever
//! calls says it is, counted from when the program started.

use std::mem;
use std::time::Duration;

use omorfos_config::{Edges, Part, State, States, Style, Theme};
use omorfos_icons::Icons;
use omorfos_ui::{Area, Canvas, Image, Motion, Scale, Text, inset};

/// The most notifications kept at once. When there are more, the oldest
/// goes.
const MOST: usize = 8;

/// The most lines of a notification's body that are shown.
const LINES: usize = 3;

/// The most of a title or of a line of body that is kept, in characters.
/// Only as much as fits is drawn; this keeps what is not drawn from being
/// carried about.
const LONGEST: usize = 400;

/// How large a notification's picture is, in units of layout.
const PICTURE: u32 = 40;

/// The room between a notification's picture and its words.
const GAP: u32 = 12;

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
    id: u32,
    title: String,
    /// What it says, as its sender broke it into lines.
    said: Vec<String>,
    /// What it says, as it is drawn: broken again to fit, if it has been
    /// fitted to a theme.
    lines: Vec<String>,
    picture: Picture,
    urgent: bool,
    acts: bool,
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
    /// The one the pointer is over, by its number.
    under: Option<u32>,
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
        }
    }

    /// Changes how long a notification stays when the program that sent
    /// it does not say, as [`Stack::new`] has it. Those showing keep the
    /// time they were given.
    pub fn set_usual(&mut self, seconds: u32) {
        *self = Self {
            usual: Self::new(seconds).usual,
            shown: mem::take(&mut self.shown),
            under: self.under,
        };
    }

    /// Whether there is nothing to show.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
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
            id: note.id,
            title: clipped(plain(&note.title).trim()),
            lines: said.clone(),
            said,
            picture: note.picture,
            urgent: note.urgent,
            acts: note.acts,
            until: lasts.map(|lasts| now.saturating_add(lasts)),
        };
        let place = self.shown.iter().position(|other| other.id == shown.id);
        match place.and_then(|place| self.shown.get_mut(place)) {
            Some(other) => *other = shown,
            None => self.shown.insert(0, shown),
        }
        let crowded = self.shown.len() > MOST;
        crowded
            .then(|| self.shown.pop())
            .flatten()
            .map(|gone| gone.id)
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
                let left = LINES.saturating_sub(lines.len());
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
        self.shown.retain(|shown| shown.id != id);
        self.shown.len() != before
    }

    /// Takes away the notifications whose time has run out by `now`, and
    /// returns their numbers.
    pub fn expire(&mut self, now: Duration) -> Vec<u32> {
        let due = |shown: &Shown| shown.until.is_some_and(|until| until <= now);
        let gone = self.shown.extract_if(.., |shown| due(shown));
        gone.map(|shown| shown.id).collect()
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
    /// Returns its number, if there was one there, and whether its sender
    /// wants to hear of the click.
    pub fn click(&mut self, x: i32, y: i32, theme: &Theme) -> Option<(u32, bool)> {
        let under = self.at(x, y, theme);
        let shown = under.and_then(|id| self.shown.iter().find(|shown| shown.id == id));
        let clicked = shown.map(|shown| (shown.id, shown.acts));
        clicked.filter(|(id, _)| self.close(*id))
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
            let key = u64::from(shown.id);
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
        }
        canvas
    }

    /// The states a notification is in.
    fn states(&self, shown: &Shown) -> States {
        let urgent = States::NONE.when(State::Urgent, shown.urgent);
        urgent.when(State::Hovered, self.under == Some(shown.id))
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

    /// The number of the notification whose box a point is in.
    fn at(&self, x: i32, y: i32, theme: &Theme) -> Option<u32> {
        let places = self.places(theme);
        let inside = |(left, top, width, height): Area| {
            let across = u32::try_from(x.saturating_sub(left)).ok();
            let down = u32::try_from(y.saturating_sub(top)).ok();
            across
                .zip(down)
                .is_some_and(|(across, down)| across < width && down < height)
        };
        let found = places.into_iter().find(|(_, area)| inside(*area));
        found.map(|(shown, _)| shown.id)
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
}
