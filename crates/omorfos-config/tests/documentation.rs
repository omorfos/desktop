//! Keeps the examples in `docs/CONFIGURATION.md` valid, and the manual
//! pages of the files, which end with them, made.

use std::path::Path;
use std::{env, fs, mem};

use omorfos_config::{Config, FILES};
use tempfile::TempDir;

/// Returns the first example under each heading that names a file, in the
/// language that file is written in.
fn examples(guide: &str) -> Vec<(String, String)> {
    let mut examples: Vec<(String, String)> = Vec::new();
    let mut file = None;
    let mut block = None;
    for line in guide.lines() {
        if let Some(heading) = line.strip_prefix("## `") {
            file = heading.strip_suffix('`').map(str::to_owned);
        } else if line == "```yaml" || line == "```css" {
            let language = line.trim_start_matches('`');
            let wanted = file.as_ref().is_some_and(|name| name.ends_with(language));
            block = wanted.then(String::new);
        } else if line == "```" {
            if let (Some(name), Some(text)) = (file.take(), block.take()) {
                examples.push((name, text));
            }
        } else if let Some(text) = block.as_mut() {
            text.push_str(line);
            text.push('\n');
        }
    }
    examples
}

#[test]
fn every_stylesheet_in_the_guide_is_a_theme() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONFIGURATION.md");
    let guide = fs::read_to_string(path).unwrap();
    let blocks = guide.split("```css\n").skip(1);
    let stylesheets: Vec<&str> = blocks
        .filter_map(|block| block.split("```").next())
        .collect();
    assert!(
        stylesheets.len() >= 3,
        "the built-in theme and two examples"
    );
    for stylesheet in stylesheets {
        let theme = omorfos_config::Theme::parse(stylesheet);
        assert!(theme.is_ok(), "{theme:?}\n{stylesheet}");
    }
    // The light scheme is shown as it is.
    let light = format!("```css\n{}```", include_str!("../src/light.css"));
    assert!(
        guide.contains(&light),
        "the guide's light scheme is not the one built in"
    );
}

#[test]
fn every_example_in_the_guide_is_valid_and_the_stated_defaults_are_true() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONFIGURATION.md");
    let examples = examples(&fs::read_to_string(path).unwrap());

    let documented: Vec<&str> = examples.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        documented, FILES,
        "the guide documents every file, in order"
    );

    let directory = TempDir::new().unwrap();
    for (name, text) in &examples {
        fs::write(directory.path().join(name), text).unwrap();
    }
    let defaults = Config::default();
    let (mut config, diagnostics) = Config::load(directory.path(), &defaults);
    assert_eq!(diagnostics, [], "an example in the guide is invalid");

    // The guide says the theme example is the built-in theme.
    assert_eq!(config.theme, defaults.theme);

    // ... and that `config.yaml` does too, apart from the workspace names.
    let workspaces = mem::take(&mut config.general.workspaces);
    assert_eq!(workspaces.names(), ["web", "code", "chat", "media"]);
    assert_eq!(config.general, defaults.general);

    // Three made, one of them in place of a built-in binding, and one
    // built-in binding done without.
    let built_in = defaults.keys.bindings().len();
    assert_eq!(config.keys.bindings().len(), built_in + 1);
    let bound = |keys: &str| {
        let mut bindings = config.keys.bindings().iter();
        let found = bindings.find(|binding| binding.keys.to_string() == keys);
        found.map(|binding| binding.action.clone())
    };
    assert_eq!(bound("Super+l"), Some(omorfos_config::Action::Lock));
    assert_eq!(bound("Super+h"), None);
    assert_eq!(config.rules.rules.len(), 6);
    assert_eq!(config.menu.items.len(), 3);
}

/// The pages of the files in `man/`, made from the schemas and the guide's
/// examples. Make them again after changing either with:
///
/// ```sh
/// OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page
/// ```
#[cfg(feature = "schema")]
#[test]
fn every_manual_page_of_a_file_is_made_from_its_schema_and_the_guide() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let guide = fs::read_to_string(top.join("docs/CONFIGURATION.md")).unwrap();
    let examples = examples(&guide);
    let example = |file: &str| {
        let found = examples.iter().find(|(name, _)| name == file);
        found.map(|(_, text)| text.as_str()).unwrap_or_default()
    };
    let examples = omorfos_config::manual::Examples {
        config: example("config.yaml"),
        keys: example("keys.yaml"),
        rules: example("rules.yaml"),
        menu: example("menu.yaml"),
        theme: example("theme.css"),
    };
    let bless = env::var_os("OMORFOS_BLESS").is_some();
    for (name, made) in omorfos_config::manual::pages(&examples) {
        let path = top.join("man").join(name);
        let kept = omorfos_ipc::manual::kept(&path, &made, bless).unwrap();
        assert!(
            kept,
            "{} is out of date; make it again with \
             `OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page`",
            path.display(),
        );
    }
}

/// Where the guide's table of the built-in bindings begins, which a test
/// makes, and where it ends.
#[cfg(feature = "schema")]
const BINDINGS: [&str; 2] = [
    "<!-- The built-in bindings, made by a test: \
     OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page -->\n",
    "<!-- The end of the built-in bindings. -->\n",
];

/// The guide's table of the built-in bindings, made from them as their
/// manual page is, so that neither falls behind them; made again by the
/// same command as the pages.
#[cfg(feature = "schema")]
#[test]
fn the_guides_built_in_bindings_are_made_as_the_manual_pages_are() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/CONFIGURATION.md");
    let guide = fs::read_to_string(&path).unwrap();
    let (before, rest) = guide
        .split_once(BINDINGS[0])
        .expect("the guide marks the table");
    let (_, after) = rest
        .split_once(BINDINGS[1])
        .expect("the guide ends the table");
    let rows = omorfos_config::manual::bindings().into_iter();
    let rows: Vec<String> = rows
        .map(|(keys, action, does)| format!("| `{keys}` | `{action}` | {does} |\n"))
        .collect();
    let table = format!(
        "| Keys | Action | What it does |\n|---|---|---|\n{}",
        rows.concat()
    );
    assert!(table.contains("| `Super+q` | `close` | Ask the focused window to close |\n"));
    let made = format!("{before}{}{table}{}{after}", BINDINGS[0], BINDINGS[1]);
    let bless = env::var_os("OMORFOS_BLESS").is_some();
    let kept = omorfos_ipc::manual::kept(&path, &made, bless).unwrap();
    assert!(
        kept,
        "the guide's built-in bindings are out of date; make them again with \
         `OMORFOS_BLESS=1 cargo test --workspace --all-features manual_page`",
    );
}
