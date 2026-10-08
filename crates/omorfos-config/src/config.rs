//! Loading the configuration directory.

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use serde::de::DeserializeOwned;

use crate::{Diagnostic, General, Keys, Menu, Rules, Theme};

/// The files that make up the configuration, relative to its directory.
///
/// Every file is optional; an absent or empty file means built-in defaults.
pub const FILES: [&str; 5] = [GENERAL, KEYS, RULES, THEME, MENU];

const GENERAL: &str = "config.yaml";
const KEYS: &str = "keys.yaml";
const RULES: &str = "rules.yaml";
const THEME: &str = "theme.yaml";
const MENU: &str = "menu.yaml";

/// The largest configuration file that will be read, in bytes.
///
/// Real configuration is a few kilobytes. The limit stops a stray or hostile
/// file from being read into memory whole.
pub const MAX_FILE_SIZE: u64 = 1024 * 1024;

/// The complete, validated configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Config {
    /// General behaviour, from `config.yaml`.
    pub general: General,
    /// Key bindings, from `keys.yaml`.
    pub keys: Keys,
    /// Per-application rules, from `rules.yaml`.
    pub rules: Rules,
    /// Appearance, from `theme.yaml`.
    pub theme: Theme,
    /// The root menu, from `menu.yaml`.
    pub menu: Menu,
}

impl Config {
    /// Loads the configuration in `directory`.
    ///
    /// Each file is handled independently:
    ///
    /// - absent or empty: built-in defaults;
    /// - valid: its contents;
    /// - unreadable or invalid: the matching part of `fallback`, plus a
    ///   [`Diagnostic`] explaining why.
    ///
    /// Loading therefore always produces a usable configuration. Pass
    /// [`Config::default`] as the fallback at startup and the configuration
    /// in effect when reloading.
    #[must_use]
    pub fn load(directory: &Path, fallback: &Self) -> (Self, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let config = Self {
            general: section(directory, GENERAL, &fallback.general, &mut diagnostics),
            keys: section(directory, KEYS, &fallback.keys, &mut diagnostics),
            rules: section(directory, RULES, &fallback.rules, &mut diagnostics),
            theme: section(directory, THEME, &fallback.theme, &mut diagnostics),
            menu: section(directory, MENU, &fallback.menu, &mut diagnostics),
        };
        (config, diagnostics)
    }
}

fn section<T>(directory: &Path, name: &str, fallback: &T, diagnostics: &mut Vec<Diagnostic>) -> T
where
    T: DeserializeOwned + Default + Clone,
{
    let path = directory.join(name);
    read(&path)
        .and_then(|text| parse(&path, &text))
        .unwrap_or_else(|diagnostic| {
            diagnostics.push(diagnostic);
            fallback.clone()
        })
}

/// Reads a file, treating one that does not exist as empty.
fn read(path: &Path) -> Result<String, Diagnostic> {
    let mut text = String::new();
    match File::open(path) {
        Ok(file) => {
            // Read one byte past the limit so an oversized file is detected
            // without reading all of it.
            file.take(MAX_FILE_SIZE + 1)
                .read_to_string(&mut text)
                .map_err(|error| Diagnostic::io(path, &error))?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(Diagnostic::io(path, &error)),
    }
    if text.len() as u64 > MAX_FILE_SIZE {
        return Err(Diagnostic::new(
            path,
            format!("file is larger than the {} KiB limit", MAX_FILE_SIZE / 1024),
        ));
    }
    Ok(text)
}

/// Parses a file. A document with no content yields the defaults.
fn parse<T: DeserializeOwned + Default>(path: &Path, text: &str) -> Result<T, Diagnostic> {
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true;
    serde_saphyr::from_str_with_options::<Option<T>>(text, options)
        .map(Option::unwrap_or_default)
        .map_err(|error| Diagnostic::yaml(path, &error))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::{Color, FocusModel, Position};

    fn directory(files: &[(&str, &str)]) -> TempDir {
        let directory = TempDir::new().unwrap();
        for (name, contents) in files {
            fs::write(directory.path().join(name), contents).unwrap();
        }
        directory
    }

    fn load(files: &[(&str, &str)]) -> (Config, Vec<String>) {
        let directory = directory(files);
        let (config, diagnostics) = Config::load(directory.path(), &Config::default());
        let prefix = format!("{}/", directory.path().display());
        let messages = diagnostics
            .iter()
            .map(|d| d.to_string().replace(&prefix, ""))
            .collect();
        (config, messages)
    }

    #[test]
    fn a_missing_directory_means_built_in_defaults() {
        let directory = TempDir::new().unwrap();
        let missing = directory.path().join("does-not-exist");
        assert_eq!(
            Config::load(&missing, &Config::default()),
            (Config::default(), vec![])
        );
    }

    #[test]
    fn empty_and_comment_only_files_mean_built_in_defaults() {
        let (config, diagnostics) = load(&[("config.yaml", ""), ("keys.yaml", "# nothing yet\n")]);
        assert_eq!(config, Config::default());
        assert_eq!(diagnostics, [""; 0]);
    }

    #[test]
    fn each_file_configures_its_own_section() {
        let (config, diagnostics) = load(&[
            ("config.yaml", "focus: { model: sloppy }\n"),
            ("keys.yaml", "bindings: []\n"),
            (
                "rules.yaml",
                "rules:\n  - match: { app_id: mpv }\n    fullscreen: true\n",
            ),
            ("theme.yaml", "colors: { accent: '#ff0000' }\n"),
            ("menu.yaml", "items: [separator]\n"),
        ]);
        assert_eq!(diagnostics, [""; 0]);
        assert_eq!(config.general.focus.model, FocusModel::Sloppy);
        assert_eq!(config.keys.bindings(), []);
        assert_eq!(config.rules.rules.len(), 1);
        assert_eq!(config.theme.colors.accent, Color::rgb(255, 0, 0));
        assert_eq!(config.menu.items.len(), 1);
    }

    #[test]
    fn an_invalid_file_falls_back_while_the_valid_ones_still_apply() {
        let directory = directory(&[
            ("config.yaml", "focus: { model: sloppy }\n"),
            ("theme.yaml", "font:\n  size: huge\n"),
        ]);
        let mut fallback = Config::default();
        fallback.theme.font.family = "Previous".to_owned();

        let (config, diagnostics) = Config::load(directory.path(), &fallback);

        assert_eq!(config.general.focus.model, FocusModel::Sloppy);
        assert_eq!(config.theme, fallback.theme);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].file, directory.path().join("theme.yaml"));
        assert_eq!(
            diagnostics[0].position,
            Some(Position { line: 2, column: 9 })
        );
    }

    #[test]
    fn every_invalid_file_is_reported() {
        let (config, diagnostics) = load(&[
            ("config.yaml", "placement: everywhere\n"),
            (
                "keys.yaml",
                "bindings:\n  - { keys: Hyper+x, action: close }\n",
            ),
            ("menu.yaml", "items: [\n"),
        ]);
        assert_eq!(config, Config::default());
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
        assert!(diagnostics[0].starts_with("config.yaml:1:12: unknown variant `everywhere`"));
        assert!(diagnostics[1].starts_with("keys.yaml:2:13: unknown modifier `Hyper`"));
        assert!(diagnostics[2].starts_with("menu.yaml:"));
    }

    #[test]
    fn only_true_and_false_are_booleans() {
        let (_, diagnostics) = load(&[("config.yaml", "touchpad: { natural_scroll: yes }\n")]);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].contains("invalid boolean"),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn a_repeated_setting_is_rejected() {
        let (_, diagnostics) = load(&[("config.yaml", "placement: center\nplacement: pointer\n")]);
        assert!(diagnostics[0].starts_with("config.yaml:2:1: duplicate mapping key"));
    }

    #[test]
    fn an_alias_bomb_is_rejected_instead_of_exhausting_memory() {
        let levels: Vec<String> = (1..=24)
            .map(|level| {
                let alias = format!("{{label: x, items: *l{}}}", level - 1);
                let aliases = vec![alias; 8].join(", ");
                format!("  - label: bomb\n    items: &l{level} [{aliases}]\n")
            })
            .collect();
        let yaml = format!(
            "items:\n  - label: bomb\n    items: &l0 [separator]\n{}",
            levels.concat()
        );
        let (config, diagnostics) = load(&[("menu.yaml", &yaml)]);
        assert_eq!(config.menu, Menu::default());
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].contains("budget"), "{diagnostics:?}");
    }

    #[test]
    fn a_file_over_the_size_limit_is_rejected_without_being_parsed() {
        let size = usize::try_from(MAX_FILE_SIZE).unwrap();
        let (config, diagnostics) = load(&[("rules.yaml", &"#".repeat(size + 1))]);
        assert_eq!(config, Config::default());
        assert_eq!(
            diagnostics,
            ["rules.yaml: file is larger than the 1024 KiB limit"]
        );
    }

    #[test]
    fn a_file_exactly_at_the_size_limit_is_accepted() {
        let size = usize::try_from(MAX_FILE_SIZE).unwrap();
        let (_, diagnostics) = load(&[("rules.yaml", &"#".repeat(size))]);
        assert_eq!(diagnostics, [""; 0]);
    }

    #[test]
    fn a_file_that_is_not_text_is_reported() {
        let directory = TempDir::new().unwrap();
        fs::write(directory.path().join("theme.yaml"), [0xff, 0xfe, 0x00]).unwrap();
        let (config, diagnostics) = Config::load(directory.path(), &Config::default());
        assert_eq!(config, Config::default());
        assert!(
            diagnostics[0].message.starts_with("cannot read file: "),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn a_path_that_cannot_be_opened_is_reported_for_every_file() {
        let directory = directory(&[("not-a-directory", "")]);
        let path = directory.path().join("not-a-directory");
        let (config, diagnostics) = Config::load(&path, &Config::default());
        assert_eq!(config, Config::default());
        assert_eq!(diagnostics.len(), FILES.len());
        assert!(
            diagnostics[0].message.starts_with("cannot read file: "),
            "{diagnostics:?}"
        );
    }
}
