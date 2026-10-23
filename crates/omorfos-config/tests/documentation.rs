//! Keeps the examples in `docs/CONFIGURATION.md` valid.

use std::path::Path;
use std::{fs, mem};

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
    assert_eq!(config.rules.rules.len(), 5);
    assert_eq!(config.menu.items.len(), 3);
}
