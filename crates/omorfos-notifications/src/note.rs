//! The notifications on the screen: which there are, how long each stays,
//! how they are drawn and what a click does.
//!
//! Nothing here knows of D-Bus or of Wayland. Time is whatever whoever
//! calls says it is, counted from when the program started.

use std::time::Duration;

use omorfos_config::{Edges, Part, State, States, Style, Theme};
use omorfos_icons::Icons;
use omorfos_ui::{Area, Canvas, Image, Motion, Scale, Text, button, button_look, inset};

/// The most notifications kept at once. When there are more, the oldest
/// of the others goes to make room for the one that has just come.
const MOST: usize = 8;

/// The most lines of a notification's body that are shown.
const LINES: usize = 3;

/// The most lines that are shown of something that is wrong, which is
/// worth more room: it is often a file, a place in it and a reason.
const PROBLEM_LINES: usize = 6;

/// What a notification about something that is wrong is headed.
const OURS: &str = "omorfos";

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

/// The most of a program's actions shown as buttons, besides the click on
/// the notification itself. More would not fit across it; the rest are
/// not shown, as the specification lets a server choose.
pub const ACTIONS: usize = 3;

/// What a click on a notification itself is called, as the specification
/// names it, which its sender is told when it asked to be.
pub const DEFAULT_ACTION: &str = "default";

/// The room between a notification's buttons, and above them.
const BUTTON_GAP: u32 = 8;

/// What a program offers to do about its notification, beside what a click
/// on it does: a button, as Reply or Mark read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// What the program calls it, which it is told when it is chosen.
    pub key: String,
    /// What the button says.
    pub label: String,
}

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
    /// What else it offers to do, as buttons: at most [`ACTIONS`].
    pub actions: Vec<Act>,
    /// Whether it stays once one of them is chosen, as the program asks
    /// for one it goes on changing.
    pub resident: bool,
    /// How full something is, out of 100, such as the volume, shown as a
    /// bar under what it says.
    pub level: Option<u8>,
    /// What it is one of, such as the volume: one with the tag of a
    /// notification that is showing takes its place, whatever its number,
    /// so that a program that does not remember the number of the one it
    /// sent before still shows one at a time.
    pub tag: Option<String>,
}

/// What tells a notification that is showing from every other. One that
/// takes the place of another, as a program's next word of the volume
/// does, keeps it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key(u64);

/// Where a notification is put: its box, as far from the top and from the
/// right of the screen as the room kept round it and above it puts it, in
/// units of layout. Each is a surface of its own, so that each casts a
/// shadow round itself and comes and goes by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Place {
    /// Which notification it is.
    pub key: Key,
    /// How far its box is from the top of the screen.
    pub top: u32,
    /// How far its box is from the right of the screen.
    pub right: u32,
    /// How large its box is.
    pub size: (u32, u32),
}

/// What a click did: which notification it was on, what its sender is to
/// be told was chosen, if anything, and whether it stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clicked {
    /// The number its program gave it.
    pub id: u32,
    /// The action chosen: one of its buttons, or [`DEFAULT_ACTION`] for the
    /// notification itself where its sender asked to hear of that.
    pub chose: Option<String>,
    /// Whether it stays: one a button was chosen of that asked to.
    pub stays: bool,
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
    key: Key,
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
    offers: Offers,
    level: Option<u8>,
    tag: Option<String>,
    /// How long it stays, if it goes of its own accord, counted again from
    /// when the pointer leaves it.
    lasts: Option<Duration>,
    /// When it goes, if it does of its own accord.
    until: Option<Duration>,
}

/// What a notification that is showing offers to do: what its sender
/// sent as [`Note::acts`], [`Note::actions`] and [`Note::resident`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Offers {
    /// Whether its sender is told of a click on it.
    clicked: bool,
    /// Its buttons.
    buttons: Vec<Act>,
    /// Whether it stays once a button is chosen.
    stays: bool,
}

/// The notifications that are showing, the newest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stack {
    shown: Vec<Shown>,
    /// How long one stays when the program that sent it does not say.
    /// Nothing is for until it is dismissed.
    usual: Option<Duration>,
    /// The one the pointer is over.
    under: Option<Key>,
    /// Which of its buttons the pointer is over, if one.
    aimed: Option<usize>,
    /// The one with the keyboard, and which of its buttons has it: none,
    /// for the notification itself.
    focused: Option<(Key, Option<usize>)>,
    /// What is wrong that the user has dismissed, and is still wrong.
    dismissed: Vec<String>,
    /// What has happened that has been shown, and that the compositor is
    /// still saying has.
    seen: Vec<String>,
    /// How many notifications have been shown, each of which took the
    /// next key.
    made: u64,
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
            aimed: None,
            focused: None,
            dismissed: Vec::new(),
            seen: Vec::new(),
            made: 0,
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
        let lasts = match (stays, self.usual) {
            // Until it is dismissed, if its sender says so.
            (Some(stays), _) if stays.is_zero() => None,
            // A level keeps its sender's time: it says what a key just did,
            // and is not there to be read (ADR 112).
            (Some(stays), _) if note.level.is_some() => Some(stays),
            // Until it is dismissed, if the user says so, over any sender.
            (_, None) => None,
            // The user's time is the shortest one is shown.
            (Some(stays), Some(usual)) => Some(stays.max(usual)),
            // A critical one stays until it is seen to, unless its sender
            // set a time itself.
            (None, Some(usual)) => (!note.urgent).then_some(usual),
        };
        let said = plain(&note.body);
        let said = said.lines().map(str::trim).filter(|line| !line.is_empty());
        let said: Vec<String> = said.take(LINES).map(clipped).collect();
        let mut shown = Shown {
            key: self.next(),
            id: note.id,
            ours: false,
            problem: None,
            title: clipped(plain(&note.title).trim()),
            lines: said.clone(),
            said,
            picture: note.picture,
            urgent: note.urgent,
            offers: Offers {
                clicked: note.acts,
                buttons: note.actions,
                stays: note.resident,
            },
            level: note.level.map(|level| level.min(100)),
            tag: note.tag,
            lasts,
            until: lasts.map(|lasts| now.saturating_add(lasts)),
        };
        let tagged = |other: &Shown| shown.tag.is_some() && other.tag == shown.tag;
        let same = |other: &Shown| !other.ours && other.id == shown.id;
        let place = self
            .shown
            .iter()
            .position(|other| same(other) || tagged(other));
        let come = shown.key;
        match place.and_then(|place| self.shown.get_mut(place)) {
            Some(other) => {
                shown.key = other.key;
                *other = shown;
            }
            None => self.shown.insert(0, shown),
        }
        // Room is made by a program's notification going, never by one
        // about something that is wrong, nor by the one that has just
        // come, which would then never be seen; and by one its program did
        // not mark as critical, while there is one, as a critical one stays
        // until it is clicked.
        let theirs = |shown: &Shown| !shown.ours;
        let crowded = self.shown.iter().filter(|shown| theirs(shown)).count() > MOST;
        let other = |shown: &Shown| theirs(shown) && shown.key != come;
        let oldest = self
            .shown
            .iter()
            .rposition(|shown| other(shown) && !shown.urgent)
            .or_else(|| self.shown.iter().rposition(other));
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
                self.say(problem, Some(problem.clone()), (None, now));
                changed = true;
            }
        }
        for event in events.iter().take(MOST) {
            if !self.seen.contains(event) {
                self.seen.push(event.clone());
                self.say(event, None, (self.usual, now));
                changed = true;
            }
        }
        changed
    }

    /// Shows something omorfos itself has to say, above everything else:
    /// about a `problem`, which is urgent, or not, and staying as long as
    /// it `lasts` from `now` if it goes by itself.
    fn say(
        &mut self,
        words: &str,
        problem: Option<String>,
        (lasts, now): (Option<Duration>, Duration),
    ) {
        let said = vec![clipped(words.trim())];
        let shown = Shown {
            key: self.next(),
            id: 0,
            ours: true,
            urgent: problem.is_some(),
            problem,
            title: OURS.to_owned(),
            lines: said.clone(),
            said,
            picture: Picture::None,
            offers: Offers::default(),
            level: None,
            tag: None,
            lasts,
            until: lasts.map(|lasts| now.saturating_add(lasts)),
        };
        self.shown.insert(0, shown);
    }

    /// A key no notification has had.
    fn next(&mut self) -> Key {
        self.made = self.made.wrapping_add(1);
        Key(self.made)
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
        self.shown.retain(|shown| shown.ours || shown.id != id);
        self.shown.len() != before
    }

    /// Takes away the notifications whose time has run out by `now`, and
    /// returns the numbers of those that were programs'.
    pub fn expire(&mut self, now: Duration) -> Vec<u32> {
        // The one the pointer or the keyboard is on stays while it is
        // (ADR 112).
        let held = (self.under, self.focused.map(|(key, _)| key));
        let due = |shown: &Shown| {
            let held = held.0 == Some(shown.key) || held.1 == Some(shown.key);
            !held && shown.until.is_some_and(|until| until <= now)
        };
        let gone = self.shown.extract_if(.., |shown| due(shown));
        // Programs are told of their own, and only of their own.
        let theirs = gone.filter(|shown| !shown.ours);
        theirs.map(|shown| shown.id).collect()
    }

    /// When the next notification's time runs out, if any will.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let free = self.shown.iter().filter(|shown| !self.held(shown.key));
        free.filter_map(|shown| shown.until).min()
    }

    /// Whether the notification `key` is of is held, by the pointer or the
    /// keyboard being on it.
    fn held(&self, key: Key) -> bool {
        self.under == Some(key) || self.focused.is_some_and(|(focused, _)| focused == key)
    }

    /// Has the notification `key` is of, if any, go its time from `now`,
    /// unless it is still held.
    fn let_go(&mut self, key: Option<Key>, now: Duration) {
        let Some(key) = key.filter(|key| !self.held(*key)) else {
            return;
        };
        let left = self.shown.iter_mut().find(|shown| shown.key == key);
        if let Some(left) = left {
            left.until = left.lasts.map(|lasts| now.saturating_add(lasts));
        }
    }

    /// Where each notification is put under `theme`, from the top.
    #[must_use]
    pub fn layout(&self, theme: &Theme) -> Vec<Place> {
        let places = self.places(theme).into_iter().map(|(shown, area)| {
            let style = theme.style(Part::Notification, self.states(shown));
            Place {
                key: shown.key,
                top: area.1.unsigned_abs(),
                right: style.margin.right,
                size: (area.2, area.3),
            }
        });
        places.collect()
    }

    /// Notes which notification the pointer is on, if any, at `now`, so
    /// that it can be drawn as such and stays while it is; the one it
    /// leaves goes its time from then. Returns whether that changed
    /// anything.
    pub fn point(&mut self, under: Option<Key>, now: Duration) -> bool {
        let changed = under != self.under;
        let before = self.under;
        self.under = under;
        if changed {
            self.aimed = None;
            self.let_go(before, now);
        }
        changed
    }

    /// Notes where on the notification it is on the pointer is, `at` from
    /// its top left corner under `theme`, so that a button it is over is
    /// drawn as such and is what a click chooses. Returns whether that
    /// changed anything.
    pub fn aim(&mut self, theme: &Theme, (x, y): (i32, i32)) -> bool {
        let buttons = self.under.map(|key| self.buttons(theme, key));
        let over = |area: &Area| {
            (area.0..area.0.saturating_add_unsigned(area.2)).contains(&x)
                && (area.1..area.1.saturating_add_unsigned(area.3)).contains(&y)
        };
        let aimed = buttons.and_then(|buttons| buttons.iter().position(over));
        let changed = aimed != self.aimed;
        self.aimed = aimed;
        changed
    }

    /// Where on the notification `key` is of each of its buttons is under
    /// `theme`, from its top left corner, in the order its program gave
    /// them: none, if it has none or is not showing.
    #[must_use]
    pub fn buttons(&self, theme: &Theme, key: Key) -> Vec<Area> {
        let mut places = self.places(theme).into_iter();
        let found = places.find(|(shown, _)| shown.key == key);
        found.map_or_else(Vec::new, |(shown, (_, _, width, height))| {
            let style = theme.style(Part::Notification, self.states(shown));
            let room = inset((0, 0, width, height), style.inset());
            row(theme, room, shown.offers.buttons.len())
        })
    }

    /// Takes a click, `choosing` with the button that chooses and not the
    /// one that only dismisses, on the notification the pointer is on: on
    /// one of its buttons, that button's action is chosen, and the
    /// notification goes unless it asked to stay; anywhere else on it, it
    /// is dismissed, its sender told of the click where it asked to be.
    /// One about something that is wrong is not shown again while that
    /// stays wrong. Returns what it did, if it was on a program's.
    pub fn click(&mut self, choosing: bool) -> Option<Clicked> {
        let aimed = self.under.map(|key| (key, self.aimed));
        self.act(aimed, choosing)
    }

    /// Gives the keyboard to the notification `key` is of, the notification
    /// itself before any of its buttons, or takes it away, at `now`: the
    /// one it was on goes its time from then, unless the pointer is on it
    /// (ADR 112). Returns whether that changed anything.
    pub fn focus(&mut self, key: Option<Key>, now: Duration) -> bool {
        let focused = key.map(|key| (key, None));
        let changed = focused != self.focused;
        let before = self.focused.map(|(key, _)| key);
        self.focused = focused;
        if changed {
            self.let_go(before, now);
        }
        changed
    }

    /// Moves the keyboard on, `forwards` or back, through the notification
    /// that has it and its buttons, round from the last to the first.
    /// Returns whether it moved.
    pub fn step(&mut self, forwards: bool) -> bool {
        let focused = self.focused.and_then(|(key, button)| {
            let shown = self.shown.iter().find(|shown| shown.key == key)?;
            // The notification itself, then each button, in a ring.
            let stops = shown.offers.buttons.len() + 1;
            let at = button.map_or(0, |button| button + 1);
            let next = if forwards { at + 1 } else { at + stops - 1 } % stops;
            Some((key, next.checked_sub(1)))
        });
        let moved = focused.is_some();
        self.focused = focused.or(self.focused);
        moved
    }

    /// Takes a key that chooses, or one that only dismisses where not
    /// `choosing`, as a click is taken, on whatever has the keyboard.
    pub fn press(&mut self, choosing: bool) -> Option<Clicked> {
        self.act(self.focused, choosing)
    }

    /// Does what a click does, `choosing` or not, on the notification
    /// `target` names, and the button of it, if one: that button's action
    /// is chosen, and the notification goes unless it asked to stay; or,
    /// on the notification itself, it is dismissed, its sender told where
    /// it asked to be. One about something that is wrong is not shown
    /// again while that stays wrong. Returns what it did, if it was on a
    /// program's.
    fn act(&mut self, target: Option<(Key, Option<usize>)>, choosing: bool) -> Option<Clicked> {
        let (key, button) = target?;
        let place = self.shown.iter().position(|shown| shown.key == key)?;
        let shown = &self.shown[place];
        let button = button.filter(|_| choosing);
        let action = button.and_then(|button| shown.offers.buttons.get(button));
        let chose = action.map(|action| action.key.clone());
        let stays = chose.is_some() && shown.offers.stays;
        let default = (choosing && shown.offers.clicked).then(|| DEFAULT_ACTION.to_owned());
        let clicked = Clicked {
            id: shown.id,
            chose: chose.or(default),
            stays,
        };
        let ours = shown.ours;
        if !stays {
            self.under.take_if(|under| *under == key);
            self.aimed.take_if(|_| self.under.is_none());
            self.focused.take_if(|(focused, _)| *focused == key);
            let gone = self.shown.remove(place);
            self.dismissed.extend(gone.problem);
        }
        (!ours).then_some(clicked)
    }

    /// Draws the notification `key` is of, as large as its box, with
    /// `scale` pixels to a unit of layout, if it is showing. Pictures that
    /// are named are looked up in `icons`. One the pointer has just come
    /// onto or left is as far on its way to its new colours as `motion` has
    /// it.
    #[must_use]
    pub fn paint(
        &self,
        key: Key,
        text: &mut Text,
        theme: &Theme,
        scale: Scale,
        icons: &mut Icons,
        motion: &mut Motion,
    ) -> Option<Canvas> {
        let mut places = self.places(theme).into_iter();
        let found = places.find(|(shown, _)| shown.key == key);
        found.map(|(shown, (_, _, width, height))| {
            let mut canvas = Canvas::scaled(width, height, scale);
            let area = (0, 0, width, height);
            let states = self.states(shown);
            let key = key.0;
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
            let row = row(theme, room, shown.offers.buttons.len());
            for (index, (act, area)) in shown.offers.buttons.iter().zip(row).enumerate() {
                let lit = self.under == Some(shown.key) && self.aimed == Some(index);
                let keyed = self.focused == Some((shown.key, Some(index)));
                let states = States::NONE.when(State::Hovered, lit);
                let states = states.when(State::Focused, keyed);
                let (style, font) = button_look(theme, states);
                button(&mut canvas, text, (&style, font), (&act.label, area));
            }
            canvas
        })
    }

    /// The states a notification is in.
    fn states(&self, shown: &Shown) -> States {
        let urgent = States::NONE.when(State::Urgent, shown.urgent);
        let keyed = self.focused == Some((shown.key, None));
        let hovered = urgent.when(State::Hovered, self.under == Some(shown.key));
        hovered.when(State::Focused, keyed)
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
            let buttons = if shown.offers.buttons.is_empty() {
                0
            } else {
                BUTTON_GAP.saturating_add(button_look(theme, States::NONE).0.height)
            };
            let content = content.saturating_add(buttons);
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
}

/// Where `count` buttons go in `room`, the inside of a notification: across
/// the bottom of it, as wide as each other, with room between them.
fn row(theme: &Theme, room: Area, count: usize) -> Vec<Area> {
    let tall = button_look(theme, States::NONE).0.height;
    let count = u32::try_from(count).unwrap_or(0);
    let gaps = BUTTON_GAP.saturating_mul(count.saturating_sub(1));
    let wide = room.2.saturating_sub(gaps).checked_div(count).unwrap_or(0);
    let top = room
        .1
        .saturating_add_unsigned(room.3)
        .saturating_sub_unsigned(tall);
    let across = (0..count).map(|index| {
        let step = wide.saturating_add(BUTTON_GAP).saturating_mul(index);
        (room.0.saturating_add_unsigned(step), top, wide, tall)
    });
    across.collect()
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

    /// A colour as a canvas holds it: each of red, green and blue as much
    /// of itself as the colour is there at all.
    fn argb(color: Color) -> u32 {
        let share = |channel: u8| {
            let shared = (u16::from(channel) * u16::from(color.alpha) + 127) / 255;
            u8::try_from(shared).unwrap()
        };
        let (red, green, blue) = (share(color.red), share(color.green), share(color.blue));
        u32::from_be_bytes([color.alpha, red, green, blue])
    }

    /// The notification `key` is of, drawn, if it is showing.
    fn drawn(stack: &Stack, theme: &Theme, key: Key) -> Option<Canvas> {
        let (mut text, mut icons) = (Text::new(), Icons::default());
        let motion = &mut Motion::new();
        stack.paint(key, &mut text, theme, Scale::ONE, &mut icons, motion)
    }

    /// The notification `index` from the top, drawn.
    fn painted(stack: &Stack, theme: &Theme, index: usize) -> Canvas {
        drawn(stack, theme, stack.layout(theme)[index].key).unwrap()
    }

    /// How tall the notification `index` from the top is.
    fn tall(stack: &Stack, theme: &Theme, index: usize) -> u32 {
        stack.layout(theme)[index].size.1
    }

    /// Points at the notification `index` from the top, clear of its
    /// buttons, and clicks: its number, and whether its sender is told.
    fn click(stack: &mut Stack, theme: &Theme, index: usize) -> Option<(u32, bool)> {
        stack.point(Some(stack.layout(theme)[index].key), START);
        clicked(stack)
    }

    /// Clicks where the pointer is: the number of what was clicked, and
    /// whether its sender is told.
    fn clicked(stack: &mut Stack) -> Option<(u32, bool)> {
        let clicked = stack.click(true);
        clicked.map(|clicked| (clicked.id, clicked.chose.is_some()))
    }

    /// A notification of a program's that offers to `reply` and to `read`,
    /// and to be clicked.
    fn mail(id: u32) -> Note {
        let act = |key: &str, label: &str| Act {
            key: key.to_owned(),
            label: label.to_owned(),
        };
        Note {
            acts: true,
            actions: vec![act("reply", "Reply"), act("read", "Mark read")],
            ..note(id, "Mail", "From someone")
        }
    }

    #[test]
    fn from_the_keyboard_the_notification_and_each_button_is_reached_and_chosen_in_turn() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(mail(1), START);
        let key = stack.layout(&theme)[0].key;
        let plain = painted(&stack, &theme, 0);
        assert!(stack.focus(Some(key), START), "the notification first");
        assert!(!stack.focus(Some(key), START), "as it was");
        let ringed = painted(&stack, &theme, 0);
        assert_ne!(ringed, plain, "drawn as having the keyboard");
        // Forwards through its buttons and round again, and back.
        assert!(stack.step(true));
        let on_reply = painted(&stack, &theme, 0);
        assert_ne!(on_reply, ringed);
        stack.step(true);
        stack.step(true);
        assert_eq!(painted(&stack, &theme, 0), ringed, "round to itself");
        stack.step(false);
        let chose = stack.press(true).unwrap();
        assert_eq!((chose.id, chose.chose.as_deref()), (1, Some("read")));
        assert!(stack.is_empty());
        assert!(!stack.step(true), "nothing to move through");
        assert_eq!(stack.press(true), None, "nor to choose");

        // On itself, the click; Escape, dismissed with nothing chosen.
        stack.show(mail(2), START);
        stack.show(mail(3), START);
        let [upper, lower] = [0, 1].map(|index| stack.layout(&theme)[index].key);
        stack.focus(Some(lower), START);
        let chose = stack.press(true).unwrap();
        assert_eq!(
            (chose.id, chose.chose.as_deref()),
            (2, Some(DEFAULT_ACTION))
        );
        stack.focus(Some(upper), START);
        stack.step(true);
        let chose = stack.press(false).unwrap();
        assert_eq!((chose.id, chose.chose), (3, None));
        assert!(stack.is_empty());
        assert!(!stack.focus(None, START), "had none already, as it went");

        // Taken back by its program while it has the keyboard, there is
        // nothing to move through.
        stack.show(mail(4), START);
        stack.focus(Some(stack.layout(&theme)[0].key), START);
        assert!(stack.close(4));
        assert!(!stack.step(true));
        assert_eq!(stack.press(true), None);
    }

    #[test]
    fn a_programs_actions_are_drawn_as_buttons_under_its_words() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(note(1, "Mail", "From someone"), START);
        let plain = tall(&stack, &theme, 0);
        stack.show(mail(2), START);
        let key = stack.layout(&theme)[0].key;
        assert!(tall(&stack, &theme, 0) > plain, "room is made for them");
        let buttons = stack.buttons(&theme, key);
        assert_eq!(buttons.len(), 2);
        let ([first, second], card) = ([buttons[0], buttons[1]], painted(&stack, &theme, 0));
        // Side by side, as wide as each other, under the words.
        assert_eq!((first.1, first.2, first.3), (second.1, second.2, second.3));
        assert!(second.0 > first.0.saturating_add_unsigned(first.2));
        assert!(first.1.unsigned_abs() > plain / 2);
        // Each its box, unlike the notification round it.
        let middle = |area: Area| {
            let x = area.0.unsigned_abs() + area.2 / 2;
            let y = area.1.unsigned_abs() + area.3 / 2;
            (x, y)
        };
        let beside = card.at(2, first.1.unsigned_abs() + first.3 / 2);
        let inside = (0..first.2 / 4)
            .map(|step| card.at(first.0.unsigned_abs() + 4 + step, middle(first).1));
        assert!(inside.clone().any(|pixel| pixel != beside), "a box drawn");
        assert_eq!(stack.buttons(&theme, Key(u64::MAX)), [], "of none");
        // The one the pointer is over is lit, and only it.
        stack.point(Some(key), START);
        let pointed = painted(&stack, &theme, 0);
        stack.aim(&theme, (second.0 + 2, second.1 + 2));
        let lit = painted(&stack, &theme, 0);
        let across = |canvas: &Canvas, area: Area| {
            let (x, y) = middle(area);
            (area.0.unsigned_abs() + 2..x)
                .map(move |x| canvas.at(x, y))
                .collect::<Vec<_>>()
        };
        assert_eq!(across(&lit, first), across(&pointed, first));
        assert_ne!(across(&lit, second), across(&pointed, second), "lit");
    }

    #[test]
    fn a_button_clicked_chooses_its_action_and_the_notification_goes_unless_it_stays() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(mail(1), START);
        let key = stack.layout(&theme)[0].key;
        let reply = stack.buttons(&theme, key)[0];
        stack.point(Some(key), START);
        assert!(stack.aim(&theme, (reply.0 + 2, reply.1 + 2)), "lit");
        assert!(!stack.aim(&theme, (reply.0 + 3, reply.1 + 2)), "as it was");
        let chose = stack.click(true).unwrap();
        assert_eq!(
            (chose.id, chose.chose.as_deref(), chose.stays),
            (1, Some("reply"), false)
        );
        assert!(stack.is_empty());

        // One that asks to stay stays, and is not taken for dismissed.
        stack.show(
            Note {
                resident: true,
                ..mail(2)
            },
            START,
        );
        let key = stack.layout(&theme)[0].key;
        let read = stack.buttons(&theme, key)[1];
        stack.point(Some(key), START);
        stack.aim(&theme, (read.0 + 2, read.1 + 2));
        let chose = stack.click(true).unwrap();
        assert_eq!((chose.chose.as_deref(), chose.stays), (Some("read"), true));
        assert_eq!(showing(&stack), [2]);
        // A click beside the buttons is the click on the notification.
        stack.aim(&theme, (2, 2));
        let chose = stack.click(true).unwrap();
        assert_eq!(
            (chose.chose.as_deref(), chose.stays),
            (Some(DEFAULT_ACTION), false)
        );

        // Dismissed, by the other button, a button's action is not chosen.
        stack.show(mail(3), START);
        let key = stack.layout(&theme)[0].key;
        let reply = stack.buttons(&theme, key)[0];
        stack.point(Some(key), START);
        stack.aim(&theme, (reply.0 + 2, reply.1 + 2));
        let chose = stack.click(false).unwrap();
        assert_eq!((chose.id, chose.chose, chose.stays), (3, None, false));
        assert!(stack.is_empty());
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
    fn a_notification_goes_when_its_time_runs_out_and_the_users_time_is_the_shortest() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "Usual", ""), START);
        // A sender's time shorter than the user's lasts the user's.
        let brief = Note {
            stays: 1500,
            ..note(2, "Brief", "")
        };
        stack.show(brief, START);
        // And one longer, its own.
        let long = Note {
            stays: 60_000,
            ..note(3, "Long", "")
        };
        stack.show(long, START);
        let lasting = Note {
            stays: 0,
            ..note(4, "Lasting", "")
        };
        stack.show(lasting, START);
        assert_eq!(stack.deadline(), Some(seconds(5)));
        assert_eq!(stack.expire(seconds(4)), [0; 0]);
        assert_eq!(stack.expire(seconds(5)), [2, 1]);
        assert_eq!(stack.deadline(), Some(seconds(60)));
        assert_eq!(stack.expire(seconds(60)), [3]);
        // The one that said to stay does.
        assert_eq!(stack.deadline(), None);
        assert_eq!(stack.expire(seconds(5000)), [0; 0]);
        assert_eq!(showing(&stack), [4]);
    }

    #[test]
    fn a_level_shown_keeps_its_senders_time() {
        let level = |id, stays| Note {
            stays,
            level: Some(40),
            ..note(id, "Volume", "")
        };
        let mut stack = Stack::new(30);
        stack.show(level(1, 1500), START);
        assert_eq!(stack.deadline(), Some(Duration::from_millis(1500)));
        // Even where the user has notifications stay until dismissed.
        let mut stack = Stack::new(0);
        stack.show(level(1, 1500), START);
        assert_eq!(stack.deadline(), Some(Duration::from_millis(1500)));
        // And with no time of its own, the user's.
        let mut stack = Stack::new(5);
        stack.show(level(1, -1), START);
        assert_eq!(stack.deadline(), Some(seconds(5)));
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
        // Unless whoever sent it gave it a time, and then for at least the
        // user's.
        let timed = Note {
            stays: 2000,
            ..urgent.clone()
        };
        stack.show(timed, START);
        assert_eq!(stack.deadline(), Some(seconds(5)));
        let longer = Note {
            stays: 9000,
            ..urgent
        };
        stack.show(longer, START);
        assert_eq!(stack.deadline(), Some(seconds(9)));
    }

    #[test]
    fn with_no_usual_time_notifications_stay_until_dismissed_whatever_their_sender_says() {
        let mut stack = Stack::new(0);
        stack.show(note(1, "Stays", ""), START);
        assert_eq!(stack.deadline(), None);
        let brief = Note {
            stays: 1000,
            ..note(2, "Brief", "")
        };
        stack.show(brief, START);
        assert_eq!(stack.deadline(), None);
        assert_eq!(stack.expire(seconds(5000)), [0; 0]);
        assert_eq!(showing(&stack).len(), 2);
    }

    #[test]
    fn a_notification_pointed_at_stays_and_goes_its_time_after_the_pointer_leaves() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "Held", ""), START);
        stack.show(note(2, "Not", ""), START);
        let key = stack.layout(&Theme::default())[1].key;
        stack.point(Some(key), seconds(1));
        // Its time is not waited for while it is held, and is never past.
        assert_eq!(stack.deadline(), Some(seconds(5)));
        assert_eq!(stack.expire(seconds(5)), [2]);
        assert_eq!(stack.deadline(), None, "held, nothing is due");
        assert_eq!(stack.expire(seconds(60)), [0; 0]);
        // Let go of at a minute, it goes five seconds after.
        stack.point(None, seconds(60));
        assert_eq!(stack.deadline(), Some(seconds(65)));
        assert_eq!(stack.expire(seconds(64)), [0; 0]);
        assert_eq!(stack.expire(seconds(65)), [1]);
    }

    #[test]
    fn a_notification_the_keyboard_is_on_stays_and_goes_its_time_after_it_leaves() {
        let mut stack = Stack::new(5);
        stack.show(note(1, "Held", ""), START);
        let key = stack.layout(&Theme::default())[0].key;
        stack.focus(Some(key), seconds(1));
        assert_eq!(stack.deadline(), None, "held, nothing is due");
        assert_eq!(stack.expire(seconds(60)), [0; 0]);
        // Pointed at as the keyboard leaves, it is held still.
        stack.point(Some(key), seconds(60));
        stack.focus(None, seconds(61));
        assert_eq!(stack.expire(seconds(100)), [0; 0]);
        stack.point(None, seconds(100));
        assert_eq!(stack.deadline(), Some(seconds(105)));
        // And the other way about.
        stack.focus(Some(key), seconds(101));
        stack.point(Some(key), seconds(102));
        stack.point(None, seconds(103));
        assert_eq!(stack.deadline(), None, "the keyboard holds it still");
        stack.focus(None, seconds(110));
        assert_eq!(stack.deadline(), Some(seconds(115)));
    }

    #[test]
    fn a_notification_that_stays_until_dismissed_still_does_once_pointed_at() {
        let mut stack = Stack::new(0);
        stack.show(note(1, "Stays", ""), START);
        let key = stack.layout(&Theme::default())[0].key;
        stack.point(Some(key), seconds(1));
        stack.point(None, seconds(2));
        assert_eq!(stack.deadline(), None);
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
    fn a_critical_notification_is_not_the_one_to_make_room_while_another_can() {
        let mut stack = Stack::new(5);
        stack.show(
            Note {
                urgent: true,
                ..note(1, "Critical", "")
            },
            START,
        );
        for id in 2..=8 {
            stack.show(note(id, "Ordinary", ""), START);
        }

        // The oldest that is not critical goes, though the critical is older.
        assert_eq!(stack.show(note(9, "Another ordinary", ""), START), Some(2));
        assert_eq!(showing(&stack), [9, 8, 7, 6, 5, 4, 3, 1]);
    }

    #[test]
    fn when_every_notification_is_critical_the_oldest_makes_room_and_is_said_to_have_gone() {
        let mut stack = Stack::new(5);
        for id in 1..=8 {
            stack.show(
                Note {
                    urgent: true,
                    ..note(id, "Critical", "")
                },
                START,
            );
        }

        assert_eq!(
            stack.show(
                Note {
                    urgent: true,
                    ..note(9, "Another critical", "")
                },
                START
            ),
            Some(1)
        );
        assert_eq!(showing(&stack), [9, 8, 7, 6, 5, 4, 3, 2]);
    }

    #[test]
    fn what_has_just_come_is_shown_though_every_other_is_critical() {
        let mut stack = Stack::new(5);
        for id in 1..=8 {
            stack.show(
                Note {
                    urgent: true,
                    ..note(id, "Critical", "")
                },
                START,
            );
        }

        // The oldest critical goes, not the ordinary one that has just come.
        assert_eq!(stack.show(note(9, "Ordinary", ""), START), Some(1));
        assert_eq!(showing(&stack), [9, 8, 7, 6, 5, 4, 3, 2]);
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
        let before = tall(&stack, &theme, 0);

        stack.fit(&theme, &mut text);

        let lines = stack.shown[0].lines.clone();
        assert!(lines.len() > 1 && lines.len() <= LINES, "{lines:?}");
        assert_eq!(
            lines.join(" "),
            long.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        let more = u32::try_from(lines.len() - 1).unwrap();
        assert_eq!(tall(&stack, &theme, 0), before + 18 * more);
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
        let height = 1 + 12 + 18 + 8 + 6 + 12 + 1;
        assert_eq!(tall(&stack, &theme, 0), height);
        let style = theme.style(Part::NotificationLevel, States::NONE);
        let canvas = painted(&stack, &theme, 0);
        let (left, width, middle) = (1 + 12, 340 - 2 - 24, 1 + 12 + 18 + 8 + 3);
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
        let canvas = painted(&stack, &theme, 0);
        assert_eq!(canvas.at(left + width * 3 / 4, middle), argb(style.color));
        // And nothing is nothing.
        stack.show(level(1, Some("volume"), 0), START);
        let canvas = painted(&stack, &theme, 0);
        assert_eq!(canvas.at(left + width / 4, middle), argb(style.background));
    }

    #[test]
    fn notifications_are_stacked_with_room_around_each() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        assert_eq!(stack.layout(&theme), [], "nothing to show");
        stack.show(note(1, "Title alone", ""), START);
        // A border and padding inside, and one line of title, with the
        // margin round it.
        let one = 1 + 12 + 18 + 12 + 1;
        let first = stack.layout(&theme)[0].key;
        let alone = Place {
            key: first,
            top: 8,
            right: 8,
            size: (340, one),
        };
        assert_eq!(stack.layout(&theme), [alone]);
        stack.show(note(2, "With a body", "one\ntwo"), START);
        let second = stack.layout(&theme)[0].key;
        assert_ne!(second, first);
        let upper = Place {
            key: second,
            size: (340, one + 2 * 18),
            ..alone
        };
        // The newest on top, with the margins of both between them.
        let lower = Place {
            top: 8 + one + 2 * 18 + 8 + 8,
            ..alone
        };
        assert_eq!(stack.layout(&theme), [upper, lower]);

        // Each is drawn by itself, as large as its box, with its corners
        // rounded off.
        let canvas = painted(&stack, &theme, 1);
        assert_eq!((canvas.width(), canvas.height()), (340, one));
        let surface = argb(theme.style(Part::Notification, States::NONE).background);
        assert_eq!(canvas.at(170, 5), surface);
        assert_eq!(canvas.at(0, 0), 0);
        // The other with its body written under its title.
        let canvas = painted(&stack, &theme, 0);
        let mut body =
            (1 + 12 + 18..1 + 12 + 18 + 2 * 18).flat_map(|y| (13..327).map(move |x| (x, y)));
        assert!(body.any(|(x, y)| canvas.at(x, y) != surface), "written");
    }

    #[test]
    fn one_that_takes_the_place_of_another_is_put_where_it_was() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(level(1, Some("volume"), 10), START);
        stack.show(note(2, "Other", ""), START);
        let before = stack.layout(&theme)[1].key;
        stack.show(level(3, Some("volume"), 20), START);
        assert_eq!(stack.layout(&theme)[1].key, before, "of the same tag");
        stack.show(note(3, "Louder", ""), START);
        assert_eq!(stack.layout(&theme)[1].key, before, "and the same number");
        assert_ne!(stack.layout(&theme)[0].key, before);
        assert_eq!(showing(&stack), [2, 3]);
    }

    #[test]
    fn a_narrow_theme_still_leaves_room_to_read() {
        let theme = Theme::parse("notification { min-width: 10px; margin: 0; }").unwrap();
        let mut stack = Stack::new(5);
        stack.show(note(1, "Title", ""), START);
        assert_eq!(stack.layout(&theme)[0].size.0, NARROWEST);
    }

    #[test]
    fn a_click_dismisses_the_notification_the_pointer_is_on() {
        let theme = Theme::default();
        let mut stack = Stack::new(5);
        stack.show(note(1, "Lower", ""), START);
        stack.show(note(2, "Upper", ""), START);
        assert_eq!(clicked(&mut stack), None, "on none");
        let lower = stack.layout(&theme)[1].key;
        assert_eq!(click(&mut stack, &theme, 1), Some((1, false)));
        assert_eq!(clicked(&mut stack), None, "nothing is there now");
        assert_eq!(drawn(&stack, &theme, lower), None, "nor drawn");
        assert_eq!(click(&mut stack, &theme, 0), Some((2, false)));
        assert!(stack.is_empty());
        // One whose sender wants to hear of the click says so.
        let acting = Note {
            acts: true,
            ..note(3, "Click to open", "")
        };
        stack.show(acting, START);
        assert_eq!(click(&mut stack, &theme, 0), Some((3, true)));
        // One the pointer was on that has gone by other means is not
        // there to click.
        stack.show(note(4, "Gone", ""), START);
        stack.point(Some(stack.layout(&theme)[0].key), START);
        assert!(stack.close(4));
        assert_eq!(clicked(&mut stack), None);
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
        let edge = |stack: &Stack, index: usize| painted(stack, &theme, index).at(0, 20);
        let border = |states| argb(theme.style(Part::Notification, states).border.left.color);
        let plain = border(States::NONE);
        assert_eq!(edge(&stack, 0), plain);
        assert_eq!(edge(&stack, 1), border(States::NONE.with(State::Urgent)));

        let keys: Vec<Key> = stack.layout(&theme).iter().map(|place| place.key).collect();
        assert!(stack.point(Some(keys[0]), START));
        assert!(!stack.point(Some(keys[0]), START), "still the same one");
        assert_eq!(edge(&stack, 0), border(States::NONE.with(State::Hovered)));
        assert!(stack.point(Some(keys[1]), START), "onto the next");
        assert_eq!(edge(&stack, 0), plain);
        assert!(stack.point(None, START), "off them");
        assert!(!stack.point(None, START));
        assert_eq!(edge(&stack, 1), border(States::NONE.with(State::Urgent)));
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
        assert_eq!(tall(&stack, &theme, 0), 1 + 12 + 40 + 12 + 1);
        let canvas = painted(&stack, &theme, 0);
        let left = 1 + 12;
        assert_eq!(canvas.at(left, 13), 0xff00_ff00);
        assert_eq!(canvas.at(left + 39, 11 + 39), 0xff00_ff00);
        let surface = argb(theme.style(Part::Notification, States::NONE).background);
        let gap = left + 40..left + 40 + GAP;
        assert!(gap.into_iter().all(|x| canvas.at(x, 32) == surface));

        // One named but not to be found is left out, and takes no room.
        let named = Note {
            picture: Picture::Named("no-such-icon".to_owned()),
            ..note(1, "With a name", "")
        };
        stack.show(named, START);
        let canvas = painted(&stack, &theme, 0);
        let green = (0..canvas.width()).filter(|x| canvas.at(*x, 22) == 0xff00_ff00);
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

        assert_eq!(click(&mut stack, &theme, 0), None, "no program to tell");
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
        assert!(tall(&ours, &theme, 0) > tall(&theirs, &theme, 0));
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
        assert_eq!(click(&mut stack, &theme, 0), None);
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
