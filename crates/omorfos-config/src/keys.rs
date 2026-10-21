//! `keys.yaml`: key bindings.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

use crate::{Action, Command, Half, Position, Started, WorkspaceNumber};

/// The contents of `keys.yaml`.
///
/// No two bindings conflict: none repeats another's keys, and none starts
/// with the complete keys of another (which would make it unreachable).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawKeys")]
pub struct Keys {
    bindings: Vec<Binding>,
}

impl Keys {
    /// The bindings, in the order they were written.
    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }
}

/// `keys.yaml` as written, before conflicts between bindings are checked.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "Keys"))]
#[serde(deny_unknown_fields)]
pub(crate) struct RawKeys {
    /// The key bindings. Giving this list replaces all built-in bindings.
    bindings: Vec<RawBinding>,
}

impl TryFrom<RawKeys> for Keys {
    type Error = String;

    fn try_from(raw: RawKeys) -> Result<Self, Self::Error> {
        Self::checked(raw.bindings.into_iter().map(Binding::from).collect())
    }
}

impl Keys {
    /// Accepts bindings only if none conflicts with another.
    fn checked(bindings: Vec<Binding>) -> Result<Self, String> {
        for (index, later) in bindings.iter().enumerate() {
            for earlier in &bindings[..index] {
                let (short, long) = if earlier.keys.0.len() <= later.keys.0.len() {
                    (&earlier.keys, &later.keys)
                } else {
                    (&later.keys, &earlier.keys)
                };
                if short == long {
                    return Err(format!("`{short}` is bound more than once"));
                }
                if long.0.starts_with(&short.0) {
                    return Err(format!(
                        "`{long}` can never be reached because `{short}` is bound on its own"
                    ));
                }
            }
        }
        Ok(Self { bindings })
    }
}

impl Default for Keys {
    fn default() -> Self {
        let mut bindings = vec![
            bind("Super+Return", Action::Spawn(Command::terminal().into())),
            bind("Super+space", Action::ShowLauncher),
            bind("Super+Escape", Action::ShowMenu),
            bind("Super+q", Action::Close),
            bind("Super+m", Action::Maximize),
            bind("Super+f", Action::Fullscreen),
            bind("Super+h", Action::Minimize),
            bind("Alt+Tab", Action::FocusNext),
            bind("Alt+Shift+Tab", Action::FocusPrevious),
            bind("Super+Tab", Action::NextTab),
            bind("Super+Shift+Tab", Action::PreviousTab),
            bind("Super+Right", Action::NextWorkspace),
            bind("Super+Left", Action::PreviousWorkspace),
            bind("Super+n", Action::NewWorkspace),
            bind("Super+Shift+n", Action::MoveToNewWorkspace),
            bind("Print", Action::Spawn(Started::screenshot())),
            bind("Shift+Print", Action::Spawn(Started::screenshot_copied())),
            bind("Alt+Print", Action::Spawn(Started::screenshot_window())),
            bind("Super+Shift+r", Action::ReloadConfig),
            bind("Super+Shift+e", Action::Quit),
        ];
        // The keys a keyboard has for the volume, the brightness and what
        // is playing, which mean nothing else.
        let session = [
            ("XF86AudioRaiseVolume", "volume up"),
            ("XF86AudioLowerVolume", "volume down"),
            ("XF86AudioMute", "volume mute"),
            ("XF86AudioMicMute", "microphone mute"),
            ("XF86MonBrightnessUp", "brightness up"),
            ("XF86MonBrightnessDown", "brightness down"),
            ("XF86AudioPlay", "media play-pause"),
            ("XF86AudioPause", "media play-pause"),
            ("XF86AudioNext", "media next"),
            ("XF86AudioPrev", "media previous"),
            ("XF86AudioStop", "media stop"),
        ];
        for (key, asked) in session {
            bindings.push(bind(key, Action::Spawn(Command::session(asked).into())));
        }
        // Among windows that share a workspace: to the one beside, and
        // changing places with it.
        let beside = [
            ("Left", Half::Left),
            ("Right", Half::Right),
            ("Up", Half::Top),
            ("Down", Half::Bottom),
        ];
        for (key, toward) in beside {
            bindings.push(bind(&format!("Super+Ctrl+{key}"), Action::Focus(toward)));
            bindings.push(bind(
                &format!("Super+Ctrl+Shift+{key}"),
                Action::Swap(toward),
            ));
        }
        for number in 1..=9 {
            let workspace = WorkspaceNumber::clamped(number);
            bindings.push(bind(
                &format!("Super+{number}"),
                Action::Workspace(workspace),
            ));
            bindings.push(bind(
                &format!("Super+Shift+{number}"),
                Action::MoveToWorkspace(workspace),
            ));
        }
        Self { bindings }
    }
}

/// Builds a built-in binding from a single chord with a non-empty key.
fn bind(chord: &str, action: Action) -> Binding {
    // A key by itself has nothing held with it.
    let (modifiers, key) = chord.rsplit_once('+').unwrap_or(("", chord));
    let modifiers = modifiers
        .split('+')
        .filter_map(Modifiers::named)
        .fold(Modifiers::NONE, Modifiers::with);
    Binding {
        keys: KeyChain(vec![KeyChord {
            modifiers,
            key: key.to_owned(),
        }]),
        action,
        position: None,
    }
}

/// One key binding: the keys to press and what they do.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "RawBinding")]
pub struct Binding {
    /// The keys to press, such as `Super+Return`. Separate several chords
    /// with spaces to make a chain pressed one after another, such as
    /// `Super+x t`.
    pub keys: KeyChain,
    /// What the binding does.
    pub action: Action,
    /// Where the keys are written in `keys.yaml`, so that a problem found
    /// with them later can point there. Built-in bindings are written
    /// nowhere.
    pub position: Option<Position>,
}

/// One key binding: the keys to press and what they do.
// This is the binding as written, with the place its keys were found. Its
// documentation is the user's, because it becomes the JSON Schema.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "Binding"))]
#[serde(deny_unknown_fields)]
pub(crate) struct RawBinding {
    /// The keys to press, such as `Super+Return`. Separate several chords
    /// with spaces to make a chain pressed one after another, such as
    /// `Super+x t`.
    #[cfg_attr(feature = "schema", schemars(with = "KeyChain"))]
    keys: serde_saphyr::Spanned<KeyChain>,
    /// What the binding does.
    action: Action,
}

impl From<RawBinding> for Binding {
    fn from(raw: RawBinding) -> Self {
        let written = raw.keys.referenced;
        Self {
            keys: raw.keys.value,
            action: raw.action,
            position: Some(Position {
                line: written.line(),
                column: written.column(),
            }),
        }
    }
}

/// One or more chords pressed in sequence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct KeyChain(Vec<KeyChord>);

impl KeyChain {
    /// The chords, in the order they are pressed. Never empty.
    #[must_use]
    pub fn chords(&self) -> &[KeyChord] {
        &self.0
    }
}

impl FromStr for KeyChain {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let chords = text
            .split_whitespace()
            .map(str::parse)
            .collect::<Result<Vec<_>, _>>()?;
        if chords.is_empty() {
            Err("expected at least one key, such as `Super+Return`".to_owned())
        } else {
            Ok(Self(chords))
        }
    }
}

impl TryFrom<String> for KeyChain {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl fmt::Display for KeyChain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let chords: Vec<String> = self.0.iter().map(KeyChord::to_string).collect();
        f.write_str(&chords.join(" "))
    }
}

/// A key pressed together with a set of modifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The modifiers that must be held.
    pub modifiers: Modifiers,
    /// The XKB keysym name of the key, such as `Return`, `a` or `F1`.
    pub key: String,
}

impl FromStr for KeyChord {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (modifiers, key) = match text.rsplit_once('+') {
            Some((names, key)) => {
                let modifiers = names.split('+').try_fold(Modifiers::NONE, |held, name| {
                    Modifiers::named(name)
                        .map(|modifier| held.with(modifier))
                        .ok_or_else(|| {
                            format!(
                                "unknown modifier `{name}` in `{text}`: \
                             expected Ctrl, Alt, Shift or Super"
                            )
                        })
                })?;
                (modifiers, key)
            }
            None => (Modifiers::NONE, text),
        };
        if key.is_empty() {
            Err(format!("`{text}` is missing a key after the last `+`"))
        } else {
            Ok(Self {
                modifiers,
                key: key.to_owned(),
            })
        }
    }
}

impl fmt::Display for KeyChord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.modifiers, self.key)
    }
}

/// A set of keyboard modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self(0);
    /// The Control key.
    pub const CTRL: Self = Self(1);
    /// The Alt key.
    pub const ALT: Self = Self(1 << 1);
    /// The Shift key.
    pub const SHIFT: Self = Self(1 << 2);
    /// The Super key, often labelled with a logo.
    pub const SUPER: Self = Self(1 << 3);

    /// In the order modifiers are conventionally written.
    const NAMES: [(Self, &'static str); 4] = [
        (Self::CTRL, "Ctrl"),
        (Self::ALT, "Alt"),
        (Self::SHIFT, "Shift"),
        (Self::SUPER, "Super"),
    ];

    /// Returns the union of `self` and `other`.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns `true` if every modifier in `other` is also in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Looks a modifier up by name, ignoring case and accepting the X11
    /// aliases `Control`, `Mod1` and `Mod4`.
    fn named(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(Self::CTRL),
            "alt" | "mod1" => Some(Self::ALT),
            "shift" => Some(Self::SHIFT),
            "super" | "mod4" => Some(Self::SUPER),
            _ => None,
        }
    }
}

impl fmt::Display for Modifiers {
    /// Writes each held modifier followed by `+`, ready to prefix a key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Self::NAMES
            .iter()
            .filter(|(modifier, _)| self.contains(*modifier))
            .try_for_each(|(_, name)| write!(f, "{name}+"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Keys, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    fn chain(text: &str) -> KeyChain {
        text.parse().unwrap()
    }

    #[test]
    fn a_chord_is_modifiers_then_a_key() {
        let chord: KeyChord = "Ctrl+Alt+Delete".parse().unwrap();
        assert_eq!(chord.modifiers, Modifiers::CTRL.with(Modifiers::ALT));
        assert_eq!(chord.key, "Delete");
    }

    #[test]
    fn a_chord_may_be_a_bare_key() {
        let chord: KeyChord = "XF86AudioMute".parse().unwrap();
        assert_eq!(chord.modifiers, Modifiers::NONE);
        assert_eq!(chord.key, "XF86AudioMute");
    }

    #[test]
    fn modifier_names_ignore_case_and_accept_x11_aliases() {
        let all = Modifiers::CTRL
            .with(Modifiers::ALT)
            .with(Modifiers::SHIFT)
            .with(Modifiers::SUPER);
        assert_eq!(
            chain("ctrl+ALT+shift+super+x"),
            chain("Control+Mod1+Shift+Mod4+x")
        );
        assert_eq!(
            chain("Control+Mod1+Shift+Mod4+x").chords()[0].modifiers,
            all
        );
    }

    #[test]
    fn key_names_keep_their_case() {
        assert_ne!(chain("Super+a"), chain("Super+A"));
    }

    #[test]
    fn modifiers_report_what_they_contain() {
        let held = Modifiers::CTRL.with(Modifiers::SHIFT);
        assert!(held.contains(Modifiers::CTRL));
        assert!(held.contains(Modifiers::NONE));
        assert!(!held.contains(Modifiers::ALT));
        assert!(!held.contains(Modifiers::CTRL.with(Modifiers::ALT)));
    }

    #[test]
    fn a_chain_is_chords_separated_by_whitespace() {
        let keys = chain("  Super+x \t t ");
        assert_eq!(keys.chords().len(), 2);
        assert_eq!(
            keys.chords()[1],
            KeyChord {
                modifiers: Modifiers::NONE,
                key: "t".to_owned()
            }
        );
    }

    #[test]
    fn chains_display_in_a_canonical_form() {
        assert_eq!(
            chain("super+shift+alt+control+x   t").to_string(),
            "Ctrl+Alt+Shift+Super+x t"
        );
    }

    #[test]
    fn an_empty_chain_is_rejected() {
        let expected = Err("expected at least one key, such as `Super+Return`".to_owned());
        assert_eq!("  ".parse::<KeyChain>(), expected);
    }

    #[test]
    fn an_unknown_modifier_is_rejected_with_the_valid_names() {
        let expected = "unknown modifier `Hyper` in `Hyper+x`: expected Ctrl, Alt, Shift or Super";
        assert_eq!(
            "Super+a Hyper+x".parse::<KeyChain>(),
            Err(expected.to_owned())
        );
    }

    #[test]
    fn a_chord_without_a_key_is_rejected() {
        let expected = Err("`Super+` is missing a key after the last `+`".to_owned());
        assert_eq!("Super+".parse::<KeyChain>(), expected);
    }

    #[test]
    fn bindings_pair_keys_with_actions() {
        let yaml = "
bindings:
  - keys: Super+Return
    action: { spawn: [foot] }
  - keys: Super+x t
    action: close
";
        let keys = parse(yaml).unwrap();
        assert_eq!(keys.bindings().len(), 2);
        assert_eq!(keys.bindings()[0].keys, chain("Super+Return"));
        assert_eq!(keys.bindings()[1].action, Action::Close);
    }

    #[test]
    fn a_binding_remembers_where_its_keys_are_written() {
        let yaml = "bindings:\n  - keys: Super+Return\n    action: close\n";
        let keys = parse(yaml).unwrap();
        let position = Position {
            line: 2,
            column: 11,
        };
        assert_eq!(keys.bindings()[0].position, Some(position));
        assert_eq!(Keys::default().bindings()[0].position, None, "built in");
    }

    #[test]
    fn an_empty_list_removes_every_binding() {
        assert_eq!(parse("bindings: []").unwrap().bindings(), []);
    }

    #[test]
    fn binding_the_same_keys_twice_is_rejected() {
        let yaml = "
bindings:
  - { keys: Super+q, action: close }
  - { keys: Mod4+q, action: quit }
";
        let error = parse(yaml).unwrap_err();
        assert!(
            error.contains("`Super+q` is bound more than once"),
            "{error}"
        );
    }

    #[test]
    fn a_chain_shadowed_by_a_shorter_binding_is_rejected_in_either_order() {
        let message = "`Super+x t` can never be reached because `Super+x` is bound on its own";
        let short_first = "
bindings:
  - { keys: Super+x, action: close }
  - { keys: Super+x t, action: quit }
";
        let long_first = "
bindings:
  - { keys: Super+x t, action: quit }
  - { keys: Super+x, action: close }
";
        for yaml in [short_first, long_first] {
            let error = parse(yaml).unwrap_err();
            assert!(error.contains(message), "{error}");
        }
    }

    #[test]
    fn chains_sharing_only_a_prefix_do_not_conflict() {
        let yaml = "
bindings:
  - { keys: Super+x t, action: close }
  - { keys: Super+x q, action: quit }
  - { keys: Super+y, action: maximize }
";
        assert_eq!(parse(yaml).unwrap().bindings().len(), 3);
    }

    #[test]
    fn the_built_in_bindings_cover_the_essentials_and_do_not_conflict() {
        let keys = Keys::default();
        let action_for = |text: &str| {
            let wanted = chain(text);
            keys.bindings()
                .iter()
                .find(|binding| binding.keys == wanted)
                .map(|b| b.action.clone())
        };
        assert_eq!(
            action_for("Super+Return"),
            Some(Action::Spawn(Command::terminal().into()))
        );
        assert_eq!(action_for("Super+q"), Some(Action::Close));
        // A picture of the screen is taken by a program that may.
        let screenshot = action_for("Print");
        assert_eq!(screenshot, Some(Action::Spawn(Started::screenshot())));
        let started = Started::screenshot();
        assert_eq!(started.run.program(), "omorfos-screenshot");
        assert_eq!(started.may, [crate::Role::Capture, crate::Role::Clipboard]);
        // With Shift the user picks what it is of, over everything, and it
        // goes on the clipboard and nowhere else.
        let copied = Started::screenshot_copied();
        assert_eq!(
            action_for("Shift+Print"),
            Some(Action::Spawn(copied.clone()))
        );
        assert_eq!(copied.run.arguments(), ["--pick", "--to", "clipboard"]);
        let may = [
            crate::Role::Capture,
            crate::Role::Clipboard,
            crate::Role::Panel,
            crate::Role::Overlay,
        ];
        assert_eq!(copied.may, may);
        // With Alt it is of the window with focus, which it has to be
        // told of to ask for.
        let window = Started::screenshot_window();
        assert_eq!(action_for("Alt+Print"), Some(Action::Spawn(window.clone())));
        assert_eq!(window.run.arguments(), ["--window"]);
        assert!(window.may.contains(&crate::Role::Panel));
        assert_eq!(action_for("Alt+Shift+Tab"), Some(Action::FocusPrevious));
        assert_eq!(
            action_for("Super+3"),
            Some(Action::Workspace(WorkspaceNumber::clamped(3)))
        );
        assert_eq!(
            action_for("Super+Shift+4"),
            Some(Action::MoveToWorkspace(WorkspaceNumber::clamped(4)))
        );
        // Workspaces are made as they are wanted, so every digit goes to
        // one, and there is a key to make one.
        assert_eq!(
            action_for("Super+9"),
            Some(Action::Workspace(WorkspaceNumber::clamped(9)))
        );
        assert_eq!(action_for("Super+n"), Some(Action::NewWorkspace));
        assert_eq!(action_for("Super+Ctrl+Up"), Some(Action::Focus(Half::Top)));
        assert_eq!(
            action_for("Super+Ctrl+Shift+Left"),
            Some(Action::Swap(Half::Left))
        );
        assert_eq!(
            action_for("Super+Shift+n"),
            Some(Action::MoveToNewWorkspace)
        );
        // The keys for the volume, the brightness and what is playing ask
        // the session's own program, as an ordinary one.
        let session = |asked: &str| Some(Action::Spawn(Command::session(asked).into()));
        assert_eq!(action_for("XF86AudioRaiseVolume"), session("volume up"));
        assert_eq!(action_for("XF86AudioMicMute"), session("microphone mute"));
        assert_eq!(
            action_for("XF86MonBrightnessDown"),
            session("brightness down")
        );
        assert_eq!(action_for("XF86AudioPause"), session("media play-pause"));
        assert_eq!(action_for("XF86AudioPrev"), session("media previous"));
        let volume = Command::session("volume up");
        assert_eq!(
            (volume.program(), volume.arguments()),
            (
                "omorfos-session",
                &["volume".to_owned(), "up".to_owned()][..]
            )
        );
        assert_eq!(Keys::checked(keys.bindings().to_vec()), Ok(keys));
    }
}
