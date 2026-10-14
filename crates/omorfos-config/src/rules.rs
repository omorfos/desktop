//! `rules.yaml`: per-application rules.

use serde::Deserialize;

use crate::WorkspaceNumber;

/// The contents of `rules.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Rules {
    /// Rules applied to new windows. Every matching rule applies, in order,
    /// so a later rule overrides an earlier one.
    pub rules: Vec<Rule>,
}

/// What to do with a new window that matches.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Rule {
    /// Which windows the rule applies to.
    #[serde(rename = "match")]
    pub matches: Match,
    /// Open the window on the workspace with this number.
    #[serde(default)]
    pub workspace: Option<WorkspaceNumber>,
    /// Open the window maximized, or not.
    #[serde(default)]
    pub maximized: Option<bool>,
    /// Open the window fullscreen, or not.
    #[serde(default)]
    pub fullscreen: Option<bool>,
    /// Draw a titlebar and border around the window, or not.
    #[serde(default)]
    pub decorations: Option<bool>,
    /// Leave the window blank in any picture taken of the screen, or not.
    /// Unlike the others this holds for as long as the window matches,
    /// not only when it opens: a title that comes to match hides it.
    #[serde(default)]
    pub private: Option<bool>,
    /// Open the window rolled up into its titlebar, or not. A window
    /// without a titlebar is not rolled up.
    #[serde(default)]
    pub shaded: Option<bool>,
    /// Open the window kept above those that are not, or not.
    #[serde(default)]
    pub above: Option<bool>,
    /// Open the window on every workspace of its screen, or not.
    #[serde(default)]
    pub sticky: Option<bool>,
}

/// Selects windows by application and title.
///
/// At least one of the two is present. A pattern must match the whole value;
/// `*` stands for any run of characters.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawMatch")]
pub struct Match {
    app_id: Option<String>,
    title: Option<String>,
}

impl Match {
    /// The pattern the window's application identifier must match, if any.
    #[must_use]
    pub fn app_id(&self) -> Option<&str> {
        self.app_id.as_deref()
    }

    /// The pattern the window's title must match, if any.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }
}

/// A match as written, before checking that it selects on something.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "Match"))]
#[serde(deny_unknown_fields)]
pub(crate) struct RawMatch {
    /// Pattern for the application identifier, such as `firefox` or
    /// `org.gnome.*`. `*` stands for any run of characters.
    #[serde(default)]
    app_id: Option<String>,
    /// Pattern for the window title. `*` stands for any run of characters.
    #[serde(default)]
    title: Option<String>,
}

impl TryFrom<RawMatch> for Match {
    type Error = &'static str;

    fn try_from(raw: RawMatch) -> Result<Self, Self::Error> {
        if raw.app_id.is_none() && raw.title.is_none() {
            Err("a rule must match on `app_id`, `title` or both")
        } else {
            Ok(Self {
                app_id: raw.app_id,
                title: raw.title,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Rules, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn there_are_no_rules_by_default() {
        assert_eq!(Rules::default().rules, []);
    }

    #[test]
    fn a_rule_sets_only_the_properties_it_names() {
        let rules = parse("rules:\n  - match: { app_id: firefox }\n    workspace: 2\n").unwrap();
        let rule = &rules.rules[0];
        assert_eq!(rule.matches.app_id(), Some("firefox"));
        assert_eq!(rule.matches.title(), None);
        assert_eq!(rule.workspace, Some(WorkspaceNumber::clamped(2)));
        assert_eq!(
            (rule.maximized, rule.fullscreen, rule.decorations),
            (None, None, None)
        );
    }

    #[test]
    fn a_rule_can_match_on_title_and_set_every_property() {
        let yaml = "
rules:
  - match: { app_id: 'org.gnome.*', title: '*Preferences' }
    maximized: false
    fullscreen: true
    decorations: false
";
        let rules = parse(yaml).unwrap();
        let rule = &rules.rules[0];
        assert_eq!(rule.matches.app_id(), Some("org.gnome.*"));
        assert_eq!(rule.matches.title(), Some("*Preferences"));
        assert_eq!(rule.workspace, None);
        assert_eq!(
            (rule.maximized, rule.fullscreen, rule.decorations),
            (Some(false), Some(true), Some(false))
        );
    }

    #[test]
    fn a_rule_that_matches_on_nothing_is_rejected() {
        let error = parse("rules:\n  - match: {}\n    maximized: true\n").unwrap_err();
        assert!(
            error.contains("a rule must match on `app_id`, `title` or both"),
            "{error}"
        );
    }

    #[test]
    fn a_rule_without_a_match_is_rejected() {
        let error = parse("rules:\n  - maximized: true\n").unwrap_err();
        assert!(error.contains("missing field `match`"), "{error}");
    }
}
