//! Keeps the examples in `docs/CONFIGURATION.md` valid.

use std::path::Path;
use std::{fs, mem};

use omorfos_config::{Config, FILES};
use tempfile::TempDir;

/// Returns the first YAML example under each heading that names a file.
fn examples(guide: &str) -> Vec<(String, String)> {
    let mut examples: Vec<(String, String)> = Vec::new();
    let mut file = None;
    let mut block = None;
    for line in guide.lines() {
        if let Some(heading) = line.strip_prefix("## `") {
            file = heading.strip_suffix('`').map(str::to_owned);
        } else if line == "```yaml" {
            block = Some(String::new());
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

    // The guide says the theme example shows the defaults.
    assert_eq!(config.theme, defaults.theme);

    // ... and that `config.yaml` does too, apart from the workspace names.
    let workspaces = mem::take(&mut config.general.workspaces);
    assert_eq!(workspaces.names(), ["web", "code", "chat", "media"]);
    assert_eq!(config.general, defaults.general);

    assert_eq!(config.keys.bindings().len(), 3);
    assert_eq!(config.rules.rules.len(), 3);
    assert_eq!(config.menu.items.len(), 3);
}
