//! Which picture is shown: the one file the configuration names, or one of
//! a directory of them, a new one at an interval. Two pictures of a
//! directory named alike but for `-dark` and `-light` at the end are a
//! pair, shown as one: the first while the desktop is dark, the other
//! while it is light.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use omorfos_config::Order;

/// What the configuration names as the picture, and which of it is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slides {
    source: PathBuf,
    /// Whether it was a directory when it was first looked at.
    directory: bool,
    order: Order,
    every: Duration,
    /// The picture shown, and since when.
    shown: Option<Slide>,
    since: Duration,
    /// Those not yet shown this time through a shuffled directory, the
    /// next last.
    left: Vec<Slide>,
    /// What the next shuffle is made from.
    seed: u64,
    /// The picture to be shown next, once it has been chosen ahead of
    /// its time.
    upcoming: Upcoming,
}

/// What is to be shown next.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Upcoming {
    /// Not chosen yet.
    Open,
    /// The picture chosen, or none if the directory has none left.
    Chosen(Option<Slide>),
}

/// A picture of those shown in turn: one, or a pair of them, one for a
/// dark desktop and one for a light one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Slide {
    dark: PathBuf,
    light: PathBuf,
}

impl Slide {
    /// The one picture, whether the desktop is dark or light.
    fn one(path: PathBuf) -> Self {
        Self {
            dark: path.clone(),
            light: path,
        }
    }

    /// The picture shown while the desktop is `dark`, or light.
    #[must_use]
    pub fn path(&self, dark: bool) -> &Path {
        if dark { &self.dark } else { &self.light }
    }

    /// Whether `path` is this picture, or one of its pair.
    fn has(&self, path: &Path) -> bool {
        self.dark == path || self.light == path
    }
}

/// How long before it is due the next picture is chosen, at most, so that
/// it is ready to be shown the moment it is.
const LEAD: Duration = Duration::from_secs(10);

impl Slides {
    /// The pictures `source` names, from `now`: in `order`, each for
    /// `every`, if it is a directory. The pictures are shuffled from `seed`.
    ///
    /// A directory starts from `from` if that is one of its pictures, as
    /// it does when it was the one shown when the session last ended.
    #[must_use]
    pub fn new(
        source: PathBuf,
        order: Order,
        every: Duration,
        now: Duration,
        seed: u64,
        from: Option<&Path>,
    ) -> Self {
        let directory = source.is_dir();
        let mut slides = Self {
            source,
            directory,
            order,
            every,
            shown: None,
            since: now,
            left: Vec::new(),
            seed,
            upcoming: Upcoming::Open,
        };
        let listed = from.filter(|_| directory).map(|from| {
            let mut pictures = pictures(&slides.source).into_iter();
            pictures.find(|picture| picture.has(from))
        });
        slides.shown = listed.flatten().or_else(|| slides.next());
        slides
    }

    /// Whether these are the pictures `source` names, in `order`, each
    /// for `every`.
    #[must_use]
    pub fn are(&self, source: &Path, order: Order, every: Duration) -> bool {
        self.shows(source) && self.order == order && self.every == every
    }

    /// Whether these are the pictures `source` names.
    #[must_use]
    pub fn shows(&self, source: &Path) -> bool {
        self.source == source
    }

    /// The picture shown now, if there is one.
    #[must_use]
    pub fn shown(&self) -> Option<&Slide> {
        self.shown.as_ref()
    }

    /// When another picture may be shown: for a directory, once each has
    /// been for as long as it is to be, since one may have been put there
    /// meanwhile; for a file, never.
    #[must_use]
    pub fn due(&self) -> Option<Duration> {
        self.directory.then(|| self.since + self.every)
    }

    /// When the next picture is to be chosen, ahead of its time: half the
    /// time it is shown for, or ten seconds, before it is due. Nothing once
    /// it has been, or for a file.
    #[must_use]
    pub fn choosing(&self) -> Option<Duration> {
        let lead = (self.every / 2).min(LEAD);
        let due = self.due().filter(|_| self.upcoming == Upcoming::Open);
        due.map(|due| due.saturating_sub(lead))
    }

    /// Chooses the next picture if it is time to by `now`. Returns whether
    /// it was chosen now.
    pub fn choose(&mut self, now: Duration) -> bool {
        if self.choosing().is_none_or(|choosing| choosing > now) {
            return false;
        }
        self.upcoming = Upcoming::Chosen(self.next());
        true
    }

    /// The picture chosen to be shown next, if one has been.
    #[must_use]
    pub fn coming(&self) -> Option<&Slide> {
        match &self.upcoming {
            Upcoming::Chosen(coming) => coming.as_ref(),
            Upcoming::Open => None,
        }
    }

    /// Moves on to the next picture if it is time to by `now`: the one
    /// chosen, if one was. Returns whether that is another picture.
    pub fn turn(&mut self, now: Duration) -> bool {
        if self.due().is_none_or(|due| due > now) {
            return false;
        }
        self.since = now;
        let next = match std::mem::replace(&mut self.upcoming, Upcoming::Open) {
            Upcoming::Chosen(chosen) => chosen,
            Upcoming::Open => self.next(),
        };
        let changed = next != self.shown;
        self.shown = next;
        changed
    }

    /// The picture after the one shown.
    fn next(&mut self) -> Option<Slide> {
        if !self.directory {
            return Some(Slide::one(self.source.clone()));
        }
        let pictures = pictures(&self.source);
        match self.order {
            Order::Name => {
                let mut later = pictures
                    .iter()
                    .filter(|picture| Some(*picture) > self.shown.as_ref());
                later.next().or_else(|| pictures.first()).cloned()
            }
            Order::Shuffled => {
                if self.left.is_empty() {
                    self.left = self.shuffled(pictures);
                    // The first of a new time through is not the last of
                    // the one before.
                    let last = self.left.len().saturating_sub(1);
                    if last > 0 && self.left.last() == self.shown.as_ref() {
                        self.left.swap(0, last);
                    }
                }
                self.left.pop()
            }
        }
    }

    /// The pictures in an order of chance, a different one each time.
    fn shuffled(&mut self, mut pictures: Vec<Slide>) -> Vec<Slide> {
        // Fisher and Yates's shuffle, by Vigna's SplitMix64: chance enough
        // to choose a picture by, from any seed, with nothing to depend on.
        for last in (1..pictures.len()).rev() {
            self.seed = self.seed.wrapping_add(0x9e37_79b9_7f4a_7c15);
            let mut mixed = self.seed;
            mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
            mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
            mixed ^= mixed >> 31;
            let bound = u64::try_from(last + 1).unwrap_or(u64::MAX);
            let other = usize::try_from(mixed % bound).unwrap_or(0);
            pictures.swap(last, other);
        }
        pictures
    }
}

/// The pictures in a directory, by name: the files that are PNG or JPEG,
/// as their names say, each pair of them as one. Those in directories
/// within it are not.
fn pictures(directory: &Path) -> Vec<Slide> {
    let entries = fs::read_dir(directory).into_iter().flatten().flatten();
    let paths = entries.map(|entry| entry.path());
    let mut pictures: Vec<PathBuf> = paths
        .filter(|path| {
            let extension = path.extension().and_then(|extension| extension.to_str());
            let extension = extension.map(str::to_ascii_lowercase);
            let picture = matches!(extension.as_deref(), Some("png" | "jpg" | "jpeg"));
            picture && path.is_file()
        })
        .collect();
    pictures.sort();
    // What a picture's name is without `end`, if it ends so.
    let without = |picture: &Path, end: &str| {
        let name = picture.file_stem().and_then(OsStr::to_str);
        name.and_then(|name| name.strip_suffix(end))
            .map(str::to_owned)
    };
    let light = |dark: &Path| {
        let named = without(dark, "-dark");
        let mut lights = pictures.iter();
        named.and_then(|named| lights.find(|light| without(light, "-light") == Some(named.clone())))
    };
    let paired: Vec<&PathBuf> = pictures.iter().filter_map(|dark| light(dark)).collect();
    let alone = pictures.iter().filter(|picture| !paired.contains(picture));
    alone
        .map(|picture| match light(picture) {
            Some(light) => Slide {
                dark: picture.clone(),
                light: light.clone(),
            },
            None => Slide::one(picture.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    /// A directory with three pictures in it and things that are not.
    fn directory() -> tempfile::TempDir {
        let directory = tempfile::TempDir::new().unwrap();
        for name in ["b.jpg", "a.png", "c.JPEG", "notes.txt", "README"] {
            fs::write(directory.path().join(name), "").unwrap();
        }
        fs::create_dir(directory.path().join("inner.png")).unwrap();
        directory
    }

    fn name(slides: &Slides) -> Option<String> {
        let name = slides
            .shown()
            .and_then(|shown| shown.path(true).file_name());
        name.map(|name| name.to_string_lossy().into_owned())
    }

    #[test]
    fn a_file_is_shown_and_nothing_else_ever_is() {
        let file = PathBuf::from("/no/such/hills.jpg");
        let mut slides = Slides::new(
            file.clone(),
            Order::Shuffled,
            MINUTE,
            Duration::ZERO,
            0,
            None,
        );
        assert_eq!(slides.shown(), Some(&Slide::one(file.clone())));
        assert_eq!(slides.due(), None);
        assert!(!slides.turn(MINUTE * 1000));
        assert!(slides.are(&file, Order::Shuffled, MINUTE));
        assert!(!slides.are(&file, Order::Name, MINUTE));
        assert!(!slides.are(&file, Order::Shuffled, MINUTE * 2));
        assert!(!slides.are(Path::new("/other.jpg"), Order::Shuffled, MINUTE));
    }

    #[test]
    fn the_pictures_of_a_directory_are_shown_by_name_in_turn() {
        let directory = directory();
        let start = Duration::from_secs(5);
        let mut slides = Slides::new(directory.path().into(), Order::Name, MINUTE, start, 0, None);
        assert_eq!(name(&slides).as_deref(), Some("a.png"));
        assert_eq!(slides.due(), Some(start + MINUTE));
        assert!(!slides.turn(start + Duration::from_secs(59)), "not yet");
        let mut now = start;
        let mut seen = Vec::new();
        for _ in 0..4 {
            now += MINUTE;
            assert!(slides.turn(now));
            seen.extend(name(&slides));
        }
        assert_eq!(seen, ["b.jpg", "c.JPEG", "a.png", "b.jpg"]);
        assert_eq!(slides.due(), Some(now + MINUTE));
    }

    #[test]
    fn a_picture_put_in_the_directory_is_shown_in_its_place_and_one_taken_out_is_not() {
        let directory = directory();
        let mut slides = Slides::new(
            directory.path().into(),
            Order::Name,
            MINUTE,
            Duration::ZERO,
            0,
            None,
        );
        fs::write(directory.path().join("aa.png"), "").unwrap();
        fs::remove_file(directory.path().join("b.jpg")).unwrap();
        slides.turn(MINUTE);
        assert_eq!(name(&slides).as_deref(), Some("aa.png"));
        slides.turn(MINUTE * 2);
        assert_eq!(name(&slides).as_deref(), Some("c.JPEG"));
        // With none left there is nothing to show, until one is put there.
        for picture in ["a.png", "aa.png", "c.JPEG"] {
            fs::remove_file(directory.path().join(picture)).unwrap();
        }
        assert!(slides.turn(MINUTE * 3));
        assert_eq!(slides.shown(), None);
        assert!(!slides.turn(MINUTE * 4), "still nothing");
        fs::write(directory.path().join("new.jpg"), "").unwrap();
        assert!(slides.turn(MINUTE * 5));
        assert_eq!(name(&slides).as_deref(), Some("new.jpg"));
    }

    #[test]
    fn pictures_named_alike_but_for_dark_and_light_are_shown_as_one() {
        let directory = tempfile::TempDir::new().unwrap();
        let names = [
            "hills-dark.png",
            "hills-light.jpg",
            "lone-dark.png",
            "sea.png",
            "town-light.png",
        ];
        for name in names {
            fs::write(directory.path().join(name), "").unwrap();
        }
        let source = directory.path().to_owned();
        let mut slides = Slides::new(source, Order::Name, MINUTE, Duration::ZERO, 0, None);
        let both = |slides: &Slides| {
            let shown = slides.shown().unwrap();
            let name = |dark| shown.path(dark).file_name().unwrap().to_owned();
            (name(true), name(false))
        };
        let mut seen = vec![both(&slides)];
        for minute in 1..4 {
            slides.turn(MINUTE * minute);
            seen.push(both(&slides));
        }
        // Without its other half, a picture is itself, dark or light.
        let alone = |name: &str| (name.into(), name.into());
        assert_eq!(
            seen,
            [
                ("hills-dark.png".into(), "hills-light.jpg".into()),
                alone("lone-dark.png"),
                alone("sea.png"),
                alone("town-light.png"),
            ]
        );
        // One left showing the light half starts the pair again.
        let source = directory.path().to_owned();
        let from = directory.path().join("hills-light.jpg");
        let again = Slides::new(source, Order::Name, MINUTE, Duration::ZERO, 0, Some(&from));
        assert_eq!(both(&again), seen[0]);
    }

    #[test]
    fn a_shuffled_directory_shows_each_picture_once_each_time_through() {
        let directory = directory();
        for seed in 0..20 {
            let source = directory.path().to_owned();
            let mut slides =
                Slides::new(source, Order::Shuffled, MINUTE, Duration::ZERO, seed, None);
            let mut seen = vec![name(&slides).unwrap()];
            for minute in 1..9 {
                slides.turn(MINUTE * minute);
                seen.extend(name(&slides));
            }
            for pass in seen.chunks(3) {
                let mut pass = pass.to_vec();
                pass.sort();
                assert_eq!(pass, ["a.png", "b.jpg", "c.JPEG"], "{seed}: {seen:?}");
            }
            // Nor is one shown twice in a row, from one time through to
            // the next.
            let twice = seen.windows(2).any(|pair| pair[0] == pair[1]);
            assert!(!twice, "{seed}: {seen:?}");
        }
    }

    #[test]
    fn shuffles_differ_from_seed_to_seed() {
        let directory = directory();
        let order = |seed| {
            let source = directory.path().to_owned();
            let slides = Slides::new(source, Order::Shuffled, MINUTE, Duration::ZERO, seed, None);
            name(&slides)
        };
        let firsts: std::collections::HashSet<_> = (0..20).map(order).collect();
        assert_eq!(firsts.len(), 3, "{firsts:?}");
    }

    #[test]
    fn a_directory_of_one_picture_or_none_shows_that_or_nothing() {
        let empty = tempfile::TempDir::new().unwrap();
        let source = empty.path().to_owned();
        let mut slides = Slides::new(source, Order::Shuffled, MINUTE, Duration::ZERO, 7, None);
        assert_eq!(slides.shown(), None);
        fs::write(empty.path().join("only.png"), "").unwrap();
        assert!(slides.turn(MINUTE));
        assert!(!slides.turn(MINUTE * 2), "the same one again");
        assert_eq!(name(&slides).as_deref(), Some("only.png"));
    }

    #[test]
    fn the_next_picture_is_chosen_before_it_is_due_and_shown_when_it_is() {
        let directory = directory();
        let source = directory.path().to_owned();
        let mut slides = Slides::new(source, Order::Name, MINUTE, Duration::ZERO, 0, None);
        // Ten seconds before, or half the time each is shown for if that
        // is less.
        assert_eq!(slides.choosing(), Some(Duration::from_secs(50)));
        assert!(!slides.choose(Duration::from_secs(49)));
        assert_eq!(slides.coming(), None);
        assert!(slides.choose(Duration::from_secs(50)));
        assert_eq!(
            slides.coming(),
            Some(&Slide::one(directory.path().join("b.jpg")))
        );
        assert_eq!(slides.choosing(), None, "chosen once");
        assert!(!slides.choose(Duration::from_secs(55)));
        // Taken out of the directory since, it is shown even so: it is
        // ready.
        fs::remove_file(directory.path().join("b.jpg")).unwrap();
        assert!(slides.turn(MINUTE));
        assert_eq!(name(&slides).as_deref(), Some("b.jpg"));
        assert_eq!(slides.coming(), None);
        assert_eq!(slides.choosing(), Some(MINUTE + Duration::from_secs(50)));
        // Shown for ten seconds, it is chosen five before.
        let short = Duration::from_secs(10);
        let source = directory.path().to_owned();
        let quick = Slides::new(source, Order::Name, short, Duration::ZERO, 0, None);
        assert_eq!(quick.choosing(), Some(Duration::from_secs(5)));
        // A file is never chosen again.
        let file = PathBuf::from("/no/such/hills.jpg");
        let mut one = Slides::new(file, Order::Name, short, Duration::ZERO, 0, None);
        assert_eq!(one.choosing(), None);
        assert!(!one.choose(MINUTE));
    }

    #[test]
    fn a_directory_starts_from_the_picture_it_was_left_at_if_it_is_still_there() {
        let directory = directory();
        let start = |from: &Path| {
            let source = directory.path().to_owned();
            let slides = Slides::new(source, Order::Name, MINUTE, Duration::ZERO, 0, Some(from));
            name(&slides)
        };
        assert_eq!(
            start(&directory.path().join("c.JPEG")).as_deref(),
            Some("c.JPEG")
        );
        // One that is gone, one elsewhere, and what is not a picture are
        // not started from.
        for other in ["gone.png", "../c.JPEG", "notes.txt"] {
            assert_eq!(
                start(&directory.path().join(other)).as_deref(),
                Some("a.png"),
                "{other}"
            );
        }
        // Nor is a file other than itself.
        let file = PathBuf::from("/no/such/hills.jpg");
        let from = Path::new("/no/such/other.jpg");
        let slides = Slides::new(
            file.clone(),
            Order::Name,
            MINUTE,
            Duration::ZERO,
            0,
            Some(from),
        );
        assert_eq!(slides.shown(), Some(&Slide::one(file.clone())));
    }
}
