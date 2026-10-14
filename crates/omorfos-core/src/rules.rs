//! Per-application rules.

/// A pattern matched against a whole string, where `*` stands for any run of
/// characters, including none. Matching is case-sensitive.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Pattern(String);

impl Pattern {
    /// Creates a pattern.
    #[must_use]
    pub fn new(pattern: impl Into<String>) -> Self {
        Self(pattern.into())
    }

    /// Returns `true` if the pattern matches all of `text`.
    #[must_use]
    pub fn matches(&self, text: &str) -> bool {
        let mut literals = self.0.split('*');
        let first = literals.next().unwrap_or_default();
        let Some(mut rest) = text.strip_prefix(first) else {
            return false;
        };
        let Some(last) = literals.next_back() else {
            // No `*` at all: the pattern must be the whole text.
            return rest.is_empty();
        };
        // Taking the leftmost occurrence of each literal leaves the most
        // text for the ones that follow, so no backtracking is needed.
        for literal in literals {
            match rest.find(literal) {
                Some(start) => rest = &rest[start + literal.len()..],
                None => return false,
            }
        }
        rest.ends_with(last)
    }
}

/// What a rule changes about a new window. `None` leaves a property alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Effects {
    /// The zero-based workspace to open the window on.
    pub workspace: Option<usize>,
    /// Whether the window opens maximized.
    pub maximized: Option<bool>,
    /// Whether the window opens fullscreen.
    pub fullscreen: Option<bool>,
    /// Whether the window gets a titlebar and border.
    pub decorations: Option<bool>,
    /// Whether the window is left out of pictures taken of the screen.
    /// This is asked of a window as it is now, not only as it opened.
    pub private: Option<bool>,
    /// Whether the window opens rolled up into its titlebar. One without a
    /// titlebar cannot be.
    pub shaded: Option<bool>,
    /// Whether the window opens kept above those that are not.
    pub above: Option<bool>,
    /// Whether the window opens on every workspace of its screen.
    pub sticky: Option<bool>,
}

impl Effects {
    /// Combines two sets of effects, with `later` winning where both set a
    /// property.
    #[must_use]
    pub fn then(self, later: Self) -> Self {
        Self {
            workspace: later.workspace.or(self.workspace),
            maximized: later.maximized.or(self.maximized),
            fullscreen: later.fullscreen.or(self.fullscreen),
            decorations: later.decorations.or(self.decorations),
            private: later.private.or(self.private),
            shaded: later.shaded.or(self.shaded),
            above: later.above.or(self.above),
            sticky: later.sticky.or(self.sticky),
        }
    }
}

/// Applies [`Effects`] to the windows it matches.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Rule {
    /// The pattern the application identifier must match, if any.
    pub app_id: Option<Pattern>,
    /// The pattern the title must match, if any.
    pub title: Option<Pattern>,
    /// What to change about a matching window.
    pub effects: Effects,
}

impl Rule {
    /// Returns `true` if every pattern the rule has matches the window.
    #[must_use]
    pub fn matches(&self, app_id: &str, title: &str) -> bool {
        self.app_id
            .as_ref()
            .is_none_or(|pattern| pattern.matches(app_id))
            && self
                .title
                .as_ref()
                .is_none_or(|pattern| pattern.matches(title))
    }
}

/// Returns the combined effects of every rule that matches a window.
///
/// Rules apply in order, so a later rule overrides an earlier one.
#[must_use]
pub fn resolve(rules: &[Rule], app_id: &str, title: &str) -> Effects {
    rules
        .iter()
        .filter(|rule| rule.matches(app_id, title))
        .fold(Effects::default(), |effects, rule| {
            effects.then(rule.effects)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, text: &str) -> bool {
        Pattern::new(pattern).matches(text)
    }

    #[test]
    fn a_pattern_without_a_wildcard_must_equal_the_text() {
        assert!(matches("firefox", "firefox"));
        assert!(!matches("firefox", "firefox-esr"));
        assert!(!matches("firefox", "org.firefox"));
        assert!(!matches("firefox", "Firefox"));
        assert!(matches("", ""));
        assert!(!matches("", "x"));
    }

    #[test]
    fn a_lone_wildcard_matches_anything() {
        assert!(matches("*", ""));
        assert!(matches("*", "anything at all"));
        assert!(matches("**", "anything"));
    }

    #[test]
    fn a_wildcard_matches_a_prefix_suffix_or_middle() {
        assert!(matches("org.gnome.*", "org.gnome.Nautilus"));
        assert!(matches("org.gnome.*", "org.gnome."));
        assert!(!matches("org.gnome.*", "org.kde.dolphin"));
        assert!(matches("*Preferences", "Firefox Preferences"));
        assert!(!matches("*Preferences", "Preferences of Firefox"));
        assert!(matches("a*z", "abcz"));
        assert!(matches("a*z", "az"));
        assert!(!matches("a*z", "a"));
    }

    #[test]
    fn several_wildcards_match_their_literals_in_order() {
        assert!(matches("*foo*bar*", "a foo then bar."));
        assert!(!matches("*foo*bar*", "bar then foo"));
        assert!(matches("a*b*c", "a-b-b-c"));
        assert!(!matches("a*b*c", "a-c-b"));
        assert!(!matches("a*bc*c", "abc"), "literals may not overlap");
    }

    #[test]
    fn wildcards_span_multi_byte_characters() {
        assert!(matches("*é*", "café au lait"));
        assert!(matches("日*語", "日本語"));
    }

    #[test]
    fn a_rule_matches_on_whichever_patterns_it_has() {
        let by_app = Rule {
            app_id: Some(Pattern::new("mpv")),
            ..Rule::default()
        };
        assert!(by_app.matches("mpv", "any title"));
        assert!(!by_app.matches("vlc", "any title"));

        let by_title = Rule {
            title: Some(Pattern::new("*Settings")),
            ..Rule::default()
        };
        assert!(by_title.matches("any", "App Settings"));
        assert!(!by_title.matches("any", "App"));

        let by_both = Rule {
            title: Some(Pattern::new("*Settings")),
            ..by_app.clone()
        };
        assert!(by_both.matches("mpv", "mpv Settings"));
        assert!(!by_both.matches("mpv", "Video"));
        assert!(!by_both.matches("vlc", "vlc Settings"));

        assert!(Rule::default().matches("anything", "anything"));
    }

    #[test]
    fn matching_rules_combine_and_later_rules_win() {
        let rule = |app_id: &str, effects| Rule {
            app_id: Some(Pattern::new(app_id)),
            title: None,
            effects,
        };
        let rules = [
            rule(
                "*",
                Effects {
                    workspace: Some(1),
                    decorations: Some(true),
                    ..Effects::default()
                },
            ),
            rule(
                "mpv",
                Effects {
                    fullscreen: Some(true),
                    decorations: Some(false),
                    ..Effects::default()
                },
            ),
            rule(
                "vlc",
                Effects {
                    maximized: Some(true),
                    workspace: Some(3),
                    ..Effects::default()
                },
            ),
        ];
        assert_eq!(
            resolve(&rules, "mpv", ""),
            Effects {
                workspace: Some(1),
                maximized: None,
                fullscreen: Some(true),
                decorations: Some(false),
                ..Effects::default()
            }
        );
        assert_eq!(resolve(&rules, "vlc", "").workspace, Some(3));
        assert_eq!(resolve(&rules, "vlc", "").maximized, Some(true));
        assert_eq!(resolve(&[], "mpv", ""), Effects::default());
    }
}
