//! The configuration in effect.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::{Config, Diagnostic, SchemeChoice, Theme};

/// How long day takes to turn into night, or night into day: how long a
/// change of scheme that `scheme: auto` makes is drawn over.
pub const TURNING: Duration = Duration::from_mins(1);

/// How often the colours move on as one scheme turns into the other: a
/// step a quarter of a second, so that each is too small to be seen, and
/// what is drawn is not drawn again every frame for a minute.
pub const STEP: Duration = Duration::from_millis(250);

/// Holds the configuration in effect and reloads it safely.
///
/// A reload never makes things worse: a file that fails to load keeps the
/// value it had, the other files still apply, and the problems are kept for
/// the caller to show to the user.
#[derive(Debug)]
pub struct Store {
    directory: PathBuf,
    /// What is in effect: what was loaded, but for its theme while one
    /// scheme turns into the other.
    current: Arc<Config>,
    /// What was loaded.
    loaded: Arc<Config>,
    diagnostics: Vec<Diagnostic>,
    turning: Option<Turning>,
}

/// One scheme turning into the other.
#[derive(Debug)]
struct Turning {
    /// The theme it is turning from.
    from: Theme,
    /// When it began, on the clock of whoever draws it, once one has; and
    /// the step it was last drawn at.
    since: Option<Duration>,
    step: Option<u128>,
}

impl Store {
    /// Loads the configuration in `directory`.
    ///
    /// Files that fail to load start out with built-in defaults; check
    /// [`Store::diagnostics`] to find out whether any did.
    #[must_use]
    pub fn open(directory: impl Into<PathBuf>) -> Self {
        let directory = directory.into();
        let (config, diagnostics) = Config::load(&directory, &Config::default());
        let config = Arc::new(config);
        Self {
            directory,
            current: Arc::clone(&config),
            loaded: config,
            diagnostics,
            turning: None,
        }
    }

    /// The directory the configuration is read from.
    #[must_use]
    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// The configuration in effect.
    ///
    /// Cloning the returned [`Arc`] is cheap and gives a consistent snapshot
    /// that later reloads do not change.
    #[must_use]
    pub const fn current(&self) -> &Arc<Config> {
        &self.current
    }

    /// When the configuration is to be loaded again, whether or not a file
    /// changes: when the scheme may next change, if it goes by the time of
    /// day.
    #[must_use]
    pub fn due(&self) -> Option<SystemTime> {
        self.current.changes
    }

    /// The problems found by the most recent load. Empty when every file
    /// was applied.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Reads the directory again.
    ///
    /// Returns `true` if the configuration in effect changed. Independently
    /// of that, [`Store::diagnostics`] now describes this load.
    ///
    /// What is loaded is in effect at once. A change of scheme that
    /// `scheme: auto` makes, rather than a change of the setting, is also
    /// kept as day turning into night, or night into day, for whoever
    /// draws to show over a minute: see [`Store::at`].
    pub fn reload(&mut self) -> bool {
        let (config, diagnostics) = Config::load(&self.directory, &self.loaded);
        self.diagnostics = diagnostics;
        let changed = config != *self.loaded;
        if changed {
            let auto = |config: &Config| config.general.appearance.scheme == SchemeChoice::Auto;
            let turned = auto(&self.loaded) && auto(&config) && self.loaded.scheme != config.scheme;
            // From what was being drawn, which may be on its way already.
            self.turning = turned.then(|| Turning {
                from: self.current.theme.clone(),
                since: None,
                step: None,
            });
            self.loaded = Arc::new(config);
            self.current = Arc::clone(&self.loaded);
        }
        changed
    }

    /// Brings the configuration in effect to how it is drawn at `now`, on
    /// the clock of whoever asks, while day turns into night or night into
    /// day: its theme's colours on their way from the scheme it was in to
    /// the one it is in, a [`STEP`] at a time, from when this is first
    /// asked until [`TURNING`] has gone by. Everything else is what was
    /// loaded from the start, the scheme among it.
    ///
    /// Returns whether the configuration in effect changed. A program that
    /// never asks has the new scheme's colours at once, as one that tells
    /// other programs the scheme is to.
    pub fn at(&mut self, now: Duration) -> bool {
        let Some(turning) = self.turning.as_mut() else {
            return false;
        };
        let since = *turning.since.get_or_insert(now);
        let gone = now.saturating_sub(since);
        if gone >= TURNING {
            self.turning = None;
            self.current = Arc::clone(&self.loaded);
            return true;
        }
        let step = gone.as_millis() / STEP.as_millis();
        if turning.step == Some(step) {
            return false;
        }
        turning.step = Some(step);
        let share = step_share(step);
        let theme = self.loaded.theme.blended(&turning.from, share);
        self.current = Arc::new(Config {
            theme,
            ..Config::clone(&self.loaded)
        });
        true
    }

    /// When the configuration in effect next changes of itself, on the
    /// clock [`Store::at`] was asked by: its next step while day turns into
    /// night or night into day.
    #[must_use]
    pub fn turns(&self) -> Option<Duration> {
        let turning = self.turning.as_ref()?;
        let next = turning.step.map_or(0, |step| step + 1);
        let steps = u32::try_from(next).unwrap_or(u32::MAX);
        let since = turning.since.unwrap_or_default();
        Some(since + STEP.saturating_mul(steps).min(TURNING))
    }
}

/// How far, from 0 to 1, a scheme has turned into the other by `step`.
#[expect(
    clippy::cast_precision_loss,
    reason = "a step of a minute's turning is a small number"
)]
fn step_share(step: u128) -> f32 {
    let steps = TURNING.as_millis() / STEP.as_millis();
    (step as f32 / steps as f32).min(1.0)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::{FocusModel, Placement};

    fn write(directory: &TempDir, name: &str, contents: &str) {
        fs::write(directory.path().join(name), contents).unwrap();
    }

    #[test]
    fn opens_with_what_is_on_disk() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", "placement: center\n");
        let store = Store::open(directory.path());
        assert_eq!(store.directory(), directory.path());
        assert_eq!(store.current().general.placement, Placement::Center);
        assert_eq!(store.diagnostics(), []);
    }

    #[test]
    fn opens_with_defaults_and_a_diagnostic_when_a_file_is_invalid() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", "placement: sideways\n");
        let store = Store::open(directory.path());
        assert_eq!(**store.current(), Config::default());
        assert_eq!(store.diagnostics().len(), 1);
    }

    #[test]
    fn reload_applies_a_valid_change() {
        let directory = TempDir::new().unwrap();
        let mut store = Store::open(directory.path());
        let before = Arc::clone(store.current());

        write(&directory, "config.yaml", "placement: pointer\n");

        assert!(store.reload());
        assert_eq!(store.current().general.placement, Placement::Pointer);
        assert_eq!(
            before.general.placement,
            Placement::Cascade,
            "snapshots are not mutated"
        );
    }

    #[test]
    fn it_is_due_again_when_the_scheme_may_change_and_never_if_it_cannot() {
        let directory = TempDir::new().unwrap();
        let store = Store::open(directory.path());
        assert_eq!(store.due(), None);
        write(&directory, "config.yaml", "appearance: { scheme: auto }\n");
        let mut store = Store::open(directory.path());
        let due = store.due().unwrap();
        // By midnight, at the latest.
        let day = Duration::from_hours(24);
        assert!(due <= SystemTime::now() + day, "{due:?}");
        assert!(due > SystemTime::now());
        write(&directory, "config.yaml", "appearance: { scheme: dark }\n");
        store.reload();
        assert_eq!(store.due(), None);
    }

    #[test]
    fn reload_reports_no_change_when_nothing_changed() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", "placement: pointer\n");
        let mut store = Store::open(directory.path());
        let before = Arc::clone(store.current());

        assert!(!store.reload());
        assert!(Arc::ptr_eq(&before, store.current()));
    }

    #[test]
    fn reload_keeps_the_previous_value_of_a_file_that_became_invalid() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", "placement: pointer\n");
        let mut store = Store::open(directory.path());

        write(&directory, "config.yaml", "placement: [\n");

        assert!(!store.reload());
        assert_eq!(store.current().general.placement, Placement::Pointer);
        assert_eq!(store.diagnostics().len(), 1);
        assert_eq!(
            store.diagnostics()[0].file,
            directory.path().join("config.yaml")
        );
    }

    #[test]
    fn reload_applies_valid_files_even_while_another_is_invalid() {
        let directory = TempDir::new().unwrap();
        let mut store = Store::open(directory.path());

        write(&directory, "theme.css", "* { font-size: 0; }\n");
        write(&directory, "config.yaml", "focus: { model: sloppy }\n");

        assert!(store.reload());
        assert_eq!(store.current().general.focus.model, FocusModel::Sloppy);
        assert_eq!(store.current().theme, Config::default().theme);
        assert_eq!(store.diagnostics().len(), 1);
    }

    #[test]
    fn fixing_the_file_clears_the_diagnostics() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", "placement: sideways\n");
        let mut store = Store::open(directory.path());
        assert_eq!(store.diagnostics().len(), 1);

        write(&directory, "config.yaml", "placement: center\n");

        assert!(store.reload());
        assert_eq!(store.diagnostics(), []);
    }

    #[test]
    fn deleting_a_file_returns_its_section_to_the_defaults() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", "placement: center\n");
        let mut store = Store::open(directory.path());

        fs::remove_file(directory.path().join("config.yaml")).unwrap();

        assert!(store.reload());
        assert_eq!(**store.current(), Config::default());
    }

    /// `config.yaml` going by the sun, on the equator, as far round as it
    /// is noon now, or midnight.
    fn by_the_sun(noon: bool) -> String {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        #[expect(clippy::cast_precision_loss, reason = "the seconds of a day")]
        let hours = (now.as_secs() % 86_400) as f64 / 3600.0;
        let noon_here = (12.0 - hours) * 15.0;
        // Half way round from noon, within the same 360 degrees.
        let midnight = (noon_here + 360.0) % 360.0 - 180.0;
        let longitude = [midnight, noon_here][usize::from(noon)];
        format!(
            "appearance: {{ scheme: auto, location: {{ latitude: 0, longitude: {longitude:.3} }} }}\n"
        )
    }

    /// The background of a focused titlebar in the theme in effect.
    fn titlebar(store: &Store) -> crate::Color {
        let focused = crate::States::NONE.with(crate::State::Focused);
        store
            .current()
            .theme
            .style(crate::Part::Titlebar, focused)
            .background
    }

    #[test]
    fn day_turns_into_night_a_step_at_a_time_over_a_minute_of_whoever_draws_it() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", &by_the_sun(true));
        let mut store = Store::open(directory.path());
        assert_eq!(store.current().scheme, crate::Scheme::Light);
        let day = titlebar(&store);
        assert_eq!(store.turns(), None, "nothing turning");
        assert!(!store.at(Duration::ZERO));

        write(&directory, "config.yaml", &by_the_sun(false));
        assert!(store.reload());
        // In effect at once, for whoever does not draw it.
        assert_eq!(store.current().scheme, crate::Scheme::Dark);
        let night = titlebar(&store);
        assert_ne!(night, day);
        assert_eq!(
            store.turns(),
            Some(Duration::ZERO),
            "begun by the first to draw it"
        );

        let start = Duration::from_secs(100);
        assert!(store.at(start));
        assert_eq!(titlebar(&store), day, "drawn from the day");
        assert_eq!(
            store.current().scheme,
            crate::Scheme::Dark,
            "and the scheme it is in"
        );
        assert_eq!(store.turns(), Some(start + STEP));
        assert!(!store.at(start + STEP / 2), "nothing new within a step");
        assert!(store.at(start + TURNING / 2));
        let half = titlebar(&store);
        assert!(half != day && half != night, "{half:?}");
        assert!(store.at(start + TURNING));
        assert_eq!(titlebar(&store), night, "and then the night");
        assert_eq!(store.turns(), None);
        assert!(!store.at(start + TURNING * 2));

        // As the setting is changed by hand, it changes at once.
        write(&directory, "config.yaml", "appearance: { scheme: light }\n");
        assert!(store.reload());
        assert_eq!(store.turns(), None);
        assert!(!store.at(start * 2));
        assert_eq!(titlebar(&store), day);
    }

    #[test]
    fn a_turning_taken_over_by_another_goes_on_from_where_it_was() {
        let directory = TempDir::new().unwrap();
        write(&directory, "config.yaml", &by_the_sun(true));
        let mut store = Store::open(directory.path());
        let day = titlebar(&store);
        write(&directory, "config.yaml", &by_the_sun(false));
        store.reload();
        store.at(Duration::ZERO);
        store.at(TURNING / 2);
        let half = titlebar(&store);
        // Back, before it has turned.
        write(&directory, "config.yaml", &by_the_sun(true));
        assert!(store.reload());
        assert!(store.at(TURNING));
        assert_eq!(titlebar(&store), half, "from where it had got to");
        assert!(store.at(TURNING * 2));
        assert_eq!(titlebar(&store), day);
        // And a file changed meanwhile, the scheme as it was, is in effect at
        // once, the turning over.
        write(&directory, "config.yaml", &by_the_sun(false));
        store.reload();
        store.at(Duration::ZERO);
        write(
            &directory,
            "config.yaml",
            &format!("{}placement: center\n", by_the_sun(false)),
        );
        assert!(store.reload());
        assert_eq!(store.turns(), None);
    }
}
