//! `config.yaml`: general behaviour.

use serde::Deserialize;

use crate::{Command, MAX_WORKSPACES};

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
    /// How every screen is driven, unless it is named in `outputs`.
    pub output: Output,
    /// Particular screens, by name: where each stands and how it is
    /// driven.
    pub outputs: Outputs,
    /// Programs to start with the session, each as a program and its
    /// arguments. They are started once, in order, and not again when the
    /// configuration is reloaded.
    pub autostart: Vec<Command>,
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

/// How the screen is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Output {
    /// How large everything is drawn: `auto` to suit the screen, or a
    /// number from 1 to 4 in steps of 0.25.
    pub scale: OutputScale,
}

/// Particular screens, by name.
///
/// The screens listed stand side by side in this order, from the left,
/// ahead of any that are connected but not listed. No name is listed twice.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "Vec<NamedOutput>")]
pub struct Outputs(Vec<NamedOutput>);

impl Outputs {
    /// The screens listed, from the left.
    #[must_use]
    pub fn listed(&self) -> &[NamedOutput] {
        &self.0
    }

    /// What is said about the screen called `name`, if it is listed.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&NamedOutput> {
        self.0.iter().find(|output| output.name == name)
    }
}

impl TryFrom<Vec<NamedOutput>> for Outputs {
    type Error = String;

    fn try_from(outputs: Vec<NamedOutput>) -> Result<Self, Self::Error> {
        let repeated = outputs.iter().enumerate().find(|(index, output)| {
            let earlier = &outputs[..*index];
            earlier.iter().any(|other| other.name == output.name)
        });
        if outputs.iter().any(|output| output.name.is_empty()) {
            Err("an output's name must not be empty".to_owned())
        } else if let Some((_, output)) = repeated {
            Err(format!("output `{}` is listed more than once", output.name))
        } else {
            Ok(Self(outputs))
        }
    }
}

/// One screen, by the name of the socket it is plugged into.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NamedOutput {
    /// The screen's name, such as `eDP-1` or `HDMI-A-1`, as `omorfosctl
    /// state` shows it.
    pub name: String,
    /// How large everything is drawn on it. Without this, the scale given
    /// for every screen applies.
    #[serde(default)]
    pub scale: Option<OutputScale>,
}

/// How large everything is drawn on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "String")]
pub enum OutputScale {
    /// Whatever suits the screen, going by how dense it is.
    #[default]
    Auto,
    /// A fixed scale, counted in quarters: 4 draws one pixel to a unit of
    /// layout, 6 one and a half, 8 two. Between 4 and 16.
    Quarters(u8),
}

impl TryFrom<String> for OutputScale {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text == "auto" {
            return Ok(Self::Auto);
        }
        let quarters = text.parse::<f64>().ok().map(|scale| scale * 4.0);
        let whole = quarters.filter(|quarters| (4.0..=16.0).contains(quarters));
        whole
            .filter(|quarters| quarters.fract() == 0.0)
            .and_then(|quarters| format!("{quarters}").parse().ok())
            .map(Self::Quarters)
            .ok_or_else(|| {
                format!(
                    "invalid scale `{text}`: expected `auto` or a number from 1 to 4 in \
                     steps of 0.25"
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn programs_to_start_with_the_session_are_listed_with_their_arguments() {
        assert_eq!(General::default().autostart, []);
        let yaml = "autostart:\n  - [swayidle, -w]\n  - [waybar]\n";
        let started = parse(yaml).unwrap().autostart;
        assert_eq!(started.len(), 2);
        assert_eq!(started[0].program(), "swayidle");
        assert_eq!(started[0].arguments(), ["-w"]);
        assert_eq!(started[1].program(), "waybar");
        assert!(
            parse("autostart: [[]]").is_err(),
            "a program has to be named"
        );
    }

    #[test]
    fn screens_are_listed_by_name_in_the_order_they_stand() {
        assert_eq!(General::default().outputs.listed(), []);
        let yaml = "outputs:\n  - { name: DP-3, scale: 1.25 }\n  - { name: eDP-1 }\n";
        let outputs = parse(yaml).unwrap().outputs;
        let names: Vec<&str> = outputs.listed().iter().map(|o| o.name.as_str()).collect();
        assert_eq!(names, ["DP-3", "eDP-1"]);
        assert_eq!(
            outputs.named("DP-3").unwrap().scale,
            Some(OutputScale::Quarters(5))
        );
        assert_eq!(outputs.named("eDP-1").unwrap().scale, None);
        assert_eq!(outputs.named("HDMI-A-1"), None);
    }

    #[test]
    fn a_screen_cannot_be_listed_twice_or_without_a_name() {
        let twice = "outputs:\n  - { name: DP-3 }\n  - { name: eDP-1 }\n  - { name: DP-3 }\n";
        let error = parse(twice).unwrap_err();
        assert!(
            error.contains("output `DP-3` is listed more than once"),
            "{error}"
        );
        let error = parse("outputs: [{ name: '' }]").unwrap_err();
        assert!(
            error.contains("an output's name must not be empty"),
            "{error}"
        );
        assert!(
            parse("outputs: [{ scale: 2 }]").is_err(),
            "a name is needed"
        );
        assert!(parse("outputs: [{ name: DP-3, size: 2 }]").is_err());
    }

    #[test]
    fn the_scale_is_automatic_unless_a_number_is_given() {
        assert_eq!(General::default().output.scale, OutputScale::Auto);
        let scale = |yaml: &str| parse(yaml).map(|general| general.output.scale);
        assert_eq!(scale("output: { scale: auto }"), Ok(OutputScale::Auto));
        assert_eq!(scale("output: { scale: 1 }"), Ok(OutputScale::Quarters(4)));
        assert_eq!(
            scale("output: { scale: 1.5 }"),
            Ok(OutputScale::Quarters(6))
        );
        assert_eq!(
            scale("output: { scale: 2.75 }"),
            Ok(OutputScale::Quarters(11))
        );
        assert_eq!(scale("output: { scale: 4 }"), Ok(OutputScale::Quarters(16)));
    }

    #[test]
    fn a_scale_that_cannot_be_used_is_rejected_with_what_can() {
        for wrong in ["big", "0.5", "4.25", "1.1", "-2", "nan", "inf", "Auto"] {
            let error = parse(&format!("output: {{ scale: {wrong} }}")).unwrap_err();
            assert!(
                error.contains(&format!(
                    "invalid scale `{wrong}`: expected `auto` or a number from 1 to 4 in steps \
                     of 0.25"
                )),
                "{wrong}: {error}"
            );
        }
    }

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
