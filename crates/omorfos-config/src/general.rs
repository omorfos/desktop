//! `config.yaml`: general behaviour.

use serde::Deserialize;

use crate::MAX_WORKSPACES;

/// The contents of `config.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct General {
    /// The workspaces of the session, as a list of names.
    pub workspaces: Workspaces,
    /// How windows gain focus.
    pub focus: Focus,
    /// Where new windows appear.
    pub placement: Placement,
    /// Keyboard layout and key repeat.
    pub keyboard: Keyboard,
    /// Touchpad behaviour.
    pub touchpad: Touchpad,
}

/// The names of the workspaces, in order. There is always at least one.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "Vec<String>")]
pub struct Workspaces(Vec<String>);

impl Workspaces {
    /// The workspace names, in order. Never empty.
    #[must_use]
    pub fn names(&self) -> &[String] {
        &self.0
    }
}

impl Default for Workspaces {
    fn default() -> Self {
        Self(["1", "2", "3", "4"].map(String::from).into())
    }
}

impl TryFrom<Vec<String>> for Workspaces {
    type Error = String;

    fn try_from(names: Vec<String>) -> Result<Self, Self::Error> {
        let max = MAX_WORKSPACES as usize;
        if names.is_empty() || names.len() > max {
            Err(format!(
                "expected between 1 and {max} workspace names, got {}",
                names.len()
            ))
        } else if names.iter().any(String::is_empty) {
            Err("workspace names must not be empty".to_owned())
        } else {
            Ok(Self(names))
        }
    }
}

/// How windows gain focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Focus {
    /// What gives a window focus.
    pub model: FocusModel,
    /// Whether focusing a window also raises it above the others.
    pub raise_on_focus: bool,
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            model: FocusModel::default(),
            raise_on_focus: true,
        }
    }
}

/// What gives a window focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum FocusModel {
    /// A window is focused when it is clicked.
    #[default]
    Click,
    /// A window is focused when the pointer enters it, and keeps focus while
    /// the pointer is over the desktop.
    Sloppy,
}

/// Where a new window appears when no rule positions it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Each window is offset from the previous one, so titlebars stay visible.
    #[default]
    Cascade,
    /// In the middle of the screen.
    Center,
    /// Under the pointer.
    Pointer,
}

/// Keyboard layout and key repeat.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Keyboard {
    /// XKB layout, such as `us` or `gb,pl` for several.
    pub layout: String,
    /// XKB variant, such as `dvorak`. Empty for the default.
    pub variant: String,
    /// XKB options, such as `caps:escape,grp:alt_shift_toggle`.
    pub options: String,
    /// Milliseconds a key is held before it starts repeating.
    pub repeat_delay: u16,
    /// Repeats per second once repeating starts. `0` disables key repeat.
    pub repeat_rate: u16,
}

impl Default for Keyboard {
    fn default() -> Self {
        Self {
            layout: "us".to_owned(),
            variant: String::new(),
            options: String::new(),
            repeat_delay: 400,
            repeat_rate: 30,
        }
    }
}

/// Touchpad behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Touchpad {
    /// Whether tapping the touchpad clicks.
    pub tap_to_click: bool,
    /// Whether content follows the fingers when scrolling.
    pub natural_scroll: bool,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self {
            tap_to_click: true,
            natural_scroll: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<General, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn defaults_are_usable_without_any_configuration() {
        let general = General::default();
        assert_eq!(general.workspaces.names(), ["1", "2", "3", "4"]);
        assert_eq!(
            general.focus,
            Focus {
                model: FocusModel::Click,
                raise_on_focus: true
            }
        );
        assert_eq!(general.placement, Placement::Cascade);
        assert_eq!(general.keyboard.layout, "us");
        assert_eq!(
            general.touchpad,
            Touchpad {
                tap_to_click: true,
                natural_scroll: false
            }
        );
    }

    #[test]
    fn unspecified_settings_keep_their_defaults() {
        let general = parse("focus:\n  model: sloppy\n").unwrap();
        assert_eq!(
            general.focus,
            Focus {
                model: FocusModel::Sloppy,
                raise_on_focus: true
            }
        );
        assert_eq!(general.keyboard, Keyboard::default());
    }

    #[test]
    fn every_setting_can_be_overridden() {
        let yaml = "
workspaces: [web, code]
focus: { model: sloppy, raise_on_focus: false }
placement: pointer
keyboard: { layout: 'gb,pl', variant: dvorak, options: 'caps:escape', repeat_delay: 250, repeat_rate: 0 }
touchpad: { tap_to_click: false, natural_scroll: true }
";
        let general = parse(yaml).unwrap();
        assert_eq!(general.workspaces.names(), ["web", "code"]);
        assert_eq!(
            general.focus,
            Focus {
                model: FocusModel::Sloppy,
                raise_on_focus: false
            }
        );
        assert_eq!(general.placement, Placement::Pointer);
        assert_eq!(
            general.keyboard,
            Keyboard {
                layout: "gb,pl".to_owned(),
                variant: "dvorak".to_owned(),
                options: "caps:escape".to_owned(),
                repeat_delay: 250,
                repeat_rate: 0,
            }
        );
        assert_eq!(
            general.touchpad,
            Touchpad {
                tap_to_click: false,
                natural_scroll: true
            }
        );
    }

    #[test]
    fn there_must_be_at_least_one_workspace() {
        let error = parse("workspaces: []").unwrap_err();
        assert!(
            error.contains("expected between 1 and 32 workspace names, got 0"),
            "{error}"
        );
    }

    #[test]
    fn there_cannot_be_more_workspaces_than_the_maximum() {
        let names = vec!["w"; 33].join(", ");
        let error = parse(&format!("workspaces: [{names}]")).unwrap_err();
        assert!(
            error.contains("expected between 1 and 32 workspace names, got 33"),
            "{error}"
        );
    }

    #[test]
    fn workspace_names_must_not_be_empty() {
        let error = parse("workspaces: [web, '']").unwrap_err();
        assert!(
            error.contains("workspace names must not be empty"),
            "{error}"
        );
    }

    #[test]
    fn unknown_settings_are_rejected_rather_than_ignored() {
        let error = parse("wallpaper: {}").unwrap_err();
        assert!(error.contains("unknown field `wallpaper`"), "{error}");
    }
}
