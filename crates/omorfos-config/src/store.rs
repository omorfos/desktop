//! The configuration in effect.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{Config, Diagnostic};

/// Holds the configuration in effect and reloads it safely.
///
/// A reload never makes things worse: a file that fails to load keeps the
/// value it had, the other files still apply, and the problems are kept for
/// the caller to show to the user.
#[derive(Debug)]
pub struct Store {
    directory: PathBuf,
    current: Arc<Config>,
    diagnostics: Vec<Diagnostic>,
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
        Self {
            directory,
            current: Arc::new(config),
            diagnostics,
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
    pub fn reload(&mut self) -> bool {
        let (config, diagnostics) = Config::load(&self.directory, &self.current);
        self.diagnostics = diagnostics;
        let changed = config != *self.current;
        if changed {
            self.current = Arc::new(config);
        }
        changed
    }
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
}
