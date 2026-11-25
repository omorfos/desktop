//! `keys.yaml`: key bindings.

use std::fmt;
use std::str::FromStr;

use serde::Deserialize;

use crate::{Action, Application, Command, Half, Position, Started, WorkspaceNumber};

/// The contents of `keys.yaml`: the bindings written there, and the
/// built-in ones on keys they leave alone.
///
/// No two bindings conflict: none repeats another's keys, and none starts
/// with the complete keys of another (which would make it unreachable).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawKeys")]
pub struct Keys {
    bindings: Vec<Binding>,
    defaults: Defaults,
}

impl Keys {
    /// The bindings, in the order they were written, and then the built-in
    /// ones that are kept.
    #[must_use]
    pub fn bindings(&self) -> &[Binding] {
        &self.bindings
    }

    /// Whether the built-in bindings are the ordinary way's, as Windows,
    /// GNOME and KDE have them, in which a chord with Super that nothing
    /// binds reaches the program as it is, and not as Ctrl (ADR 101).
    #[must_use]
    pub const fn ordinary(&self) -> bool {
        matches!(self.defaults, Defaults::Ordinary)
    }
}

/// Which built-in bindings `keys.yaml` keeps, as `defaults` says: a Mac's,
/// with Super for Command, as when it says nothing or `true`; the ordinary
/// way's, with Ctrl and Alt as Windows, GNOME and KDE have them, as
/// `ordinary`; or none, as `false` (ADR 101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "DefaultsWritten")]
pub(crate) enum Defaults {
    /// A Mac's keys, with Super for Command.
    #[default]
    Super,
    /// The ordinary way's, with Ctrl and Alt.
    Ordinary,
    /// None.
    None,
}

/// `defaults` as written: yes or no, or which.
#[derive(Deserialize)]
#[serde(untagged)]
enum DefaultsWritten {
    Said(bool),
    Which(String),
}

impl TryFrom<DefaultsWritten> for Defaults {
    type Error = String;

    fn try_from(written: DefaultsWritten) -> Result<Self, Self::Error> {
        match written {
            DefaultsWritten::Said(true) => Ok(Self::Super),
            DefaultsWritten::Said(false) => Ok(Self::None),
            DefaultsWritten::Which(which) if which == "ordinary" => Ok(Self::Ordinary),
            DefaultsWritten::Which(which) => Err(format!(
                "invalid `defaults` `{which}`: expected true, false or `ordinary`"
            )),
        }
    }
}

/// `keys.yaml` as written, before conflicts between bindings are checked.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "Keys"))]
#[serde(deny_unknown_fields)]
pub(crate) struct RawKeys {
    /// Which built-in bindings are kept, on the keys these bindings leave
    /// alone: a Mac's, with Super for Command, unless this is `ordinary`,
    /// for the ordinary way's with Ctrl and Alt, or `false`, for none.
    #[serde(default)]
    defaults: Defaults,
    /// Built-in bindings to do without, by their keys, such as `Super+h`.
    #[serde(default)]
    unbind: Vec<KeyChain>,
    /// The key bindings, beside the built-in ones. One on keys a built-in
    /// binding uses, or that begin with them, takes its place.
    #[serde(default)]
    bindings: Vec<RawBinding>,
}

impl TryFrom<RawKeys> for Keys {
    type Error = String;

    fn try_from(raw: RawKeys) -> Result<Self, Self::Error> {
        let own = Self::checked(raw.bindings.into_iter().map(Binding::from).collect())?;
        let built_in = match raw.defaults {
            Defaults::Super => super_way(),
            Defaults::Ordinary => ordinary_way(),
            Defaults::None => Vec::new(),
        };
        let unknown = raw.unbind.iter().find(|keys| {
            let mut built_in = built_in.iter();
            !built_in.any(|binding| binding.keys == **keys)
        });
        if let Some(unknown) = unknown {
            let why = if raw.defaults == Defaults::None {
                "with `defaults: false` there are no built-in bindings"
            } else {
                "there is no built-in binding on it"
            };
            return Err(format!("`{unknown}` cannot be unbound: {why}"));
        }
        // A built-in binding gives way to one written on its keys, or on a
        // chain that begins with them, which it would otherwise make
        // unreachable.
        let clashes = |keys: &KeyChain| {
            let mut own = own.iter();
            own.any(|binding| binding.keys.0.starts_with(&keys.0))
        };
        let kept: Vec<Binding> = built_in
            .into_iter()
            .filter(|binding| !raw.unbind.contains(&binding.keys) && !clashes(&binding.keys))
            .collect();
        let mut bindings = own;
        bindings.extend(kept);
        Ok(Self {
            bindings,
            defaults: raw.defaults,
        })
    }
}

impl Keys {
    /// Accepts bindings only if none conflicts with another.
    fn checked(bindings: Vec<Binding>) -> Result<Vec<Binding>, String> {
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
        Ok(bindings)
    }
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            bindings: super_way(),
            defaults: Defaults::Super,
        }
    }
}

/// The built-in bindings of a Mac's way, with Super for Command, and
/// Super+Ctrl where a Mac has Ctrl alone for the desktop (ADR 101, as
/// accepted).
fn super_way() -> Vec<Binding> {
    // A chord with
    // Super that nothing here binds reaches the program as Ctrl (ADR 78):
    // Super+W closes its tab, Super+F finds, Super+N opens a window.
    let mut bindings = vec![
        bind("Super+Return", Action::Open(Application::Terminal)),
        bind("Super+space", Action::ShowLauncher),
        bind("Super+q", Action::CloseProgram),
        bind("Super+m", Action::Minimize),
        bind("Super+h", Action::HideProgram),
        bind("Super+Alt+h", Action::HideOthers),
        bind("Super+Ctrl+f", Action::Fullscreen),
        bind("Super+Alt+Up", Action::Maximize),
        bind("Super+Alt+Left", Action::Tile(Half::Left)),
        bind("Super+Alt+Right", Action::Tile(Half::Right)),
        bind("Super+Tab", Action::SwitchPrograms),
        bind("Super+Shift+Tab", Action::SwitchProgramsBack),
        bind("Super+grave", Action::NextWindow),
        bind("Super+Shift+grave", Action::PreviousWindow),
        // A Mac goes through its windows' tabs with Ctrl+Tab, which here
        // is the program's.
        bind("Super+Ctrl+Tab", Action::NextTab),
        bind("Super+Ctrl+Shift+Tab", Action::PreviousTab),
        bind("Super+Up", Action::Overview),
        bind("Super+Right", Action::NextWorkspace),
        bind("Super+Left", Action::PreviousWorkspace),
        bind("Super+Ctrl+n", Action::NewWorkspace),
        bind("Super+Ctrl+Shift+n", Action::MoveToNewWorkspace),
        bind("Super+Ctrl+q", Action::Lock),
        bind("XF86ScreenSaver", Action::Lock),
        bind("Super+Shift+q", Action::LogOut),
        bind("Print", Action::Spawn(Started::screenshot())),
        bind("Shift+Print", Action::Spawn(Started::screenshot_copied())),
        bind("Alt+Print", Action::Spawn(Started::screenshot_window())),
        bind("Super+Shift+3", Action::Spawn(Started::screenshot())),
        bind("Super+Shift+4", Action::Spawn(Started::screenshot_picked())),
        bind("Super+Shift+5", Action::Spawn(Started::screenshot_picked())),
        bind(
            "Super+Ctrl+Shift+3",
            Action::Spawn(Started::screenshot_screens_copied()),
        ),
        bind(
            "Super+Ctrl+Shift+4",
            Action::Spawn(Started::screenshot_copied()),
        ),
        // The keys for the screens, the layout and Mission Control, which
        // mean nothing else; and Super+F1, as Cmd+F1 mirrors on a Mac.
        bind("XF86Display", Action::NextScreens),
        bind("Super+F1", Action::NextScreens),
        bind("XF86Keyboard", Action::NextLayout),
        bind("XF86LaunchA", Action::Overview),
    ];
    media(&mut bindings);
    beside(&mut bindings);
    for number in 1..=9 {
        let workspace = WorkspaceNumber::clamped(number);
        bindings.push(bind(
            &format!("Super+{number}"),
            Action::Workspace(workspace),
        ));
        // Super+Shift with 3, 4 and 5 is for pictures of the screen.
        bindings.push(bind(
            &format!("Super+Ctrl+{number}"),
            Action::MoveToWorkspace(workspace),
        ));
    }
    bindings
}

/// The built-in bindings of the ordinary way, with Ctrl and Alt as Windows,
/// GNOME and KDE have them (ADR 101, as accepted): Super is left to the
/// launcher and the overview, and to what ADR 101 names beside them, locking,
/// half the screen and the workspaces; GNOME's keys with Alt and Ctrl where
/// it names none. A chord with Super that nothing here binds reaches the
/// program as it is.
fn ordinary_way() -> Vec<Binding> {
    let mut bindings = vec![
        bind("Super+space", Action::ShowLauncher),
        bind("Super+Up", Action::Overview),
        bind("XF86LaunchA", Action::Overview),
        bind("Super+l", Action::Lock),
        bind("Super+Left", Action::Tile(Half::Left)),
        bind("Super+Right", Action::Tile(Half::Right)),
        bind("Super+Page_Up", Action::PreviousWorkspace),
        bind("Super+Page_Down", Action::NextWorkspace),
        bind("Alt+Tab", Action::SwitchWindows),
        bind("Alt+Shift+Tab", Action::SwitchWindowsBack),
        bind("Alt+F4", Action::Close),
        bind("Alt+F10", Action::Maximize),
        bind("Ctrl+Alt+Delete", Action::LogOut),
        bind("XF86ScreenSaver", Action::Lock),
        bind("Print", Action::Spawn(Started::screenshot())),
        bind("Shift+Print", Action::Spawn(Started::screenshot_copied())),
        bind("Alt+Print", Action::Spawn(Started::screenshot_window())),
        bind("XF86Display", Action::NextScreens),
        bind("XF86Keyboard", Action::NextLayout),
    ];
    media(&mut bindings);
    bindings
}

/// The keys a keyboard has for the volume, the brightness and what is
/// playing, which mean nothing else, as both ways have them.
fn media(bindings: &mut Vec<Binding>) {
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
}

/// Among windows that share a workspace: to the one beside, and changing
/// places with it.
fn beside(bindings: &mut Vec<Binding>) {
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
defaults: false
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

    /// What `keys` do, among `bindings`.
    fn action_for(keys: &Keys, text: &str) -> Option<Action> {
        let wanted = chain(text);
        let mut bindings = keys.bindings().iter();
        let found = bindings.find(|binding| binding.keys == wanted);
        found.map(|binding| binding.action.clone())
    }

    #[test]
    fn a_binding_is_made_beside_the_built_in_ones() {
        let keys = parse("bindings:\n  - { keys: Super+l, action: lock }\n").unwrap();
        assert_eq!(action_for(&keys, "Super+l"), Some(Action::Lock));
        assert_eq!(action_for(&keys, "Super+q"), Some(Action::CloseProgram));
        let built_in = Keys::default().bindings().len();
        assert_eq!(keys.bindings().len(), built_in + 1);
        // Written first, and the built-in ones after, as they come.
        assert_eq!(keys.bindings()[0].keys, chain("Super+l"));
        assert_eq!(keys.bindings()[1..], *Keys::default().bindings());
        // A file with nothing in it changes nothing.
        assert_eq!(parse("{}").unwrap(), Keys::default());
    }

    #[test]
    fn a_binding_on_the_keys_of_a_built_in_one_takes_its_place() {
        let yaml = "
bindings:
  - { keys: Super+q, action: quit }
  - { keys: Mod4+Return t, action: close }
";
        let keys = parse(yaml).unwrap();
        assert_eq!(action_for(&keys, "Super+q"), Some(Action::Quit));
        // A chain that begins with built-in keys takes them, or it could
        // never be reached.
        assert_eq!(action_for(&keys, "Super+Return"), None);
        assert_eq!(action_for(&keys, "Super+Return t"), Some(Action::Close));
        assert_eq!(keys.bindings().len(), Keys::default().bindings().len());
        assert_eq!(
            Keys::checked(keys.bindings().to_vec()),
            Ok(keys.bindings().to_vec())
        );
    }

    #[test]
    fn a_built_in_binding_can_be_done_without_and_so_can_all_of_them() {
        let keys = parse("unbind: [Super+h, Mod4+m]").unwrap();
        assert_eq!(action_for(&keys, "Super+h"), None);
        assert_eq!(action_for(&keys, "Super+m"), None);
        assert_eq!(action_for(&keys, "Super+q"), Some(Action::CloseProgram));
        assert_eq!(keys.bindings().len(), Keys::default().bindings().len() - 2);
        let error = parse("unbind: [Super+F12]").unwrap_err();
        assert!(
            error.contains("`Super+F12` cannot be unbound: there is no"),
            "{error}"
        );

        let none = parse("defaults: false\nbindings: [{ keys: Super+l, action: lock }]").unwrap();
        assert_eq!(none.bindings().len(), 1);
        assert_eq!(parse("defaults: false").unwrap().bindings(), []);
        let error = parse("defaults: false\nunbind: [Super+h]").unwrap_err();
        assert!(
            error.contains("with `defaults: false` there are no"),
            "{error}"
        );
        assert!(parse("unbind: [Super+]").is_err(), "not keys at all");
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
    fn an_empty_list_of_bindings_leaves_the_built_in_ones() {
        assert_eq!(parse("bindings: []").unwrap(), Keys::default());
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
defaults: false
bindings:
  - { keys: Super+x t, action: close }
  - { keys: Super+x q, action: quit }
  - { keys: Super+y, action: maximize }
";
        assert_eq!(parse(yaml).unwrap().bindings().len(), 3);
    }

    #[test]
    fn the_ordinary_way_has_ctrl_and_alt_as_windows_gnome_and_kde_have_them() {
        let keys = parse("defaults: ordinary").unwrap();
        assert!(keys.ordinary());
        assert!(!Keys::default().ordinary());
        assert!(!parse("defaults: true").unwrap().ordinary());
        let action_for = |text: &str| action_for(&keys, text);
        // As ADR 101 names them.
        assert_eq!(action_for("Alt+Tab"), Some(Action::SwitchWindows));
        assert_eq!(action_for("Alt+Shift+Tab"), Some(Action::SwitchWindowsBack));
        assert_eq!(action_for("Alt+F4"), Some(Action::Close));
        assert_eq!(action_for("Super+l"), Some(Action::Lock));
        assert_eq!(action_for("Super+Left"), Some(Action::Tile(Half::Left)));
        assert_eq!(action_for("Super+Right"), Some(Action::Tile(Half::Right)));
        assert_eq!(action_for("Super+Page_Up"), Some(Action::PreviousWorkspace));
        assert_eq!(action_for("Super+Page_Down"), Some(Action::NextWorkspace));
        assert_eq!(action_for("Super+space"), Some(Action::ShowLauncher));
        assert_eq!(action_for("Super+Up"), Some(Action::Overview));
        // And with Alt and Ctrl, as GNOME has them, where it names none.
        assert_eq!(action_for("Alt+F10"), Some(Action::Maximize));
        assert_eq!(action_for("Ctrl+Alt+Delete"), Some(Action::LogOut));
        assert_eq!(
            action_for("XF86AudioMute"),
            Some(Action::Spawn(Command::session("volume mute").into()))
        );
        // Every other chord with Super is the program's, as it is typed
        // (#153): none of the Mac way's, nor GNOME's beyond ADR 101's.
        for chord in [
            "Super+Tab",
            "Super+grave",
            "Super+h",
            "Super+Return",
            "Super+3",
            "Super+Shift+3",
            "Super+Ctrl+Left",
            "Super+Ctrl+Tab",
            "Super+F1",
        ] {
            assert_eq!(action_for(chord), None, "{chord}");
        }
        let super_held = keys
            .bindings()
            .iter()
            .filter(|binding| {
                binding
                    .keys
                    .chords()
                    .iter()
                    .any(|chord| chord.modifiers.contains(Modifiers::SUPER))
            })
            .count();
        assert_eq!(super_held, 7, "only the seven ADR 101 names hold Super");
        // Super with a letter is the program's, as it is typed.
        assert_eq!(action_for("Super+q"), None);
        assert_eq!(action_for("Super+m"), None);
        assert_eq!(
            Keys::checked(keys.bindings().to_vec()),
            Ok(keys.bindings().to_vec())
        );
        // The Mac way's keys are not there to unbind.
        let error = parse("defaults: ordinary\nunbind: [Super+Ctrl+q]").unwrap_err();
        assert!(
            error.contains("there is no built-in binding on it"),
            "{error}"
        );
    }

    #[test]
    fn defaults_is_true_false_or_ordinary() {
        let error = parse("defaults: windows").unwrap_err();
        assert!(
            error.contains("invalid `defaults` `windows`: expected true, false or `ordinary`"),
            "{error}"
        );
    }

    #[test]
    fn the_built_in_bindings_are_a_macs_with_super_for_command() {
        let keys = Keys::default();
        let action_for = |text: &str| action_for(&keys, text);
        // ADR 101, as accepted.
        assert_eq!(action_for("Super+q"), Some(Action::CloseProgram));
        assert_eq!(action_for("Super+w"), None, "the program's, as Ctrl+W");
        assert_eq!(action_for("Super+m"), Some(Action::Minimize));
        assert_eq!(action_for("Super+h"), Some(Action::HideProgram));
        assert_eq!(action_for("Super+Alt+h"), Some(Action::HideOthers));
        assert_eq!(action_for("Super+Alt+Up"), Some(Action::Maximize));
        assert_eq!(action_for("Super+Alt+Left"), Some(Action::Tile(Half::Left)));
        assert_eq!(
            action_for("Super+Alt+Right"),
            Some(Action::Tile(Half::Right))
        );
        assert_eq!(action_for("Super+Up"), Some(Action::Overview));
        assert_eq!(action_for("Super+Ctrl+q"), Some(Action::Lock));
        assert_eq!(action_for("XF86ScreenSaver"), Some(Action::Lock));
        assert_eq!(action_for("Super+Shift+q"), Some(Action::LogOut));
        assert_eq!(action_for("Super+F1"), Some(Action::NextScreens));
        assert_eq!(action_for("XF86Keyboard"), Some(Action::NextLayout));
        // What the review left without a key: the next layout but on its
        // own key, the root menu, reloading, and ending at once.
        for none in [
            "Super+Alt+space",
            "Super+Escape",
            "Super+Shift+r",
            "Super+Shift+e",
            "Super+l",
            "Alt+Tab",
        ] {
            assert_eq!(action_for(none), None, "{none}");
        }
        // Pictures of the screen as a Mac's: every screen, or picked, kept
        // where the configuration says, or with Ctrl on the clipboard.
        let spawned = |started: Started| Some(Action::Spawn(started));
        assert_eq!(action_for("Super+Shift+3"), spawned(Started::screenshot()));
        assert_eq!(
            action_for("Super+Shift+4"),
            spawned(Started::screenshot_picked())
        );
        assert_eq!(
            action_for("Super+Shift+5"),
            spawned(Started::screenshot_picked())
        );
        assert_eq!(
            action_for("Super+Ctrl+Shift+3"),
            spawned(Started::screenshot_screens_copied())
        );
        assert_eq!(
            action_for("Super+Ctrl+Shift+4"),
            spawned(Started::screenshot_copied())
        );
        assert_eq!(Started::screenshot_picked().run.arguments(), ["--pick"]);
        assert_eq!(
            Started::screenshot_screens_copied().run.arguments(),
            ["--to", "clipboard"]
        );
    }

    #[test]
    fn the_built_in_bindings_cover_the_essentials_and_do_not_conflict() {
        let keys = Keys::default();
        let action_for = |text: &str| action_for(&keys, text);
        assert_eq!(
            action_for("Super+Return"),
            Some(Action::Open(Application::Terminal))
        );
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
        // Super is the desktop's (ADR 78): programs on Super+Tab, the
        // program's windows on the key above Tab, the tabs of a group with
        // Ctrl, and what macOS leaves to programs on Cmd is theirs, as Ctrl.
        assert_eq!(action_for("Super+Tab"), Some(Action::SwitchPrograms));
        assert_eq!(
            action_for("Super+Shift+Tab"),
            Some(Action::SwitchProgramsBack)
        );
        assert_eq!(action_for("Super+grave"), Some(Action::NextWindow));
        assert_eq!(
            action_for("Super+Shift+grave"),
            Some(Action::PreviousWindow)
        );
        assert_eq!(action_for("Super+Ctrl+Tab"), Some(Action::NextTab));
        assert_eq!(
            action_for("Super+Ctrl+Shift+Tab"),
            Some(Action::PreviousTab)
        );
        assert_eq!(action_for("Super+Ctrl+f"), Some(Action::Fullscreen));
        assert_eq!(action_for("Super+f"), None);
        assert_eq!(action_for("Super+n"), None);
        assert_eq!(action_for("Super+Shift+n"), None);
        assert_eq!(action_for("XF86LaunchA"), Some(Action::Overview));
        assert_eq!(
            action_for("Super+3"),
            Some(Action::Workspace(WorkspaceNumber::clamped(3)))
        );
        assert_eq!(
            action_for("Super+Ctrl+4"),
            Some(Action::MoveToWorkspace(WorkspaceNumber::clamped(4)))
        );
        // Workspaces are made as they are wanted, so every digit goes to
        // one, and there is a key to make one.
        assert_eq!(
            action_for("Super+9"),
            Some(Action::Workspace(WorkspaceNumber::clamped(9)))
        );
        assert_eq!(action_for("Super+Ctrl+n"), Some(Action::NewWorkspace));
        assert_eq!(action_for("XF86Display"), Some(Action::NextScreens));
        assert_eq!(action_for("Super+Ctrl+Up"), Some(Action::Focus(Half::Top)));
        assert_eq!(
            action_for("Super+Ctrl+Shift+Left"),
            Some(Action::Swap(Half::Left))
        );
        assert_eq!(
            action_for("Super+Ctrl+Shift+n"),
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
        assert_eq!(
            Keys::checked(keys.bindings().to_vec()),
            Ok(keys.bindings().to_vec())
        );
    }
}
