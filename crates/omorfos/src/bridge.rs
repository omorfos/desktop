//! Translating configuration into the domain model's terms.
//!
//! `omorfos-config` and `omorfos-core` do not know about each other. This
//! module is where a configuration value becomes domain settings, rules and
//! key chains.

use omorfos_config::{Action, FocusModel, General, Keys, Modifiers, Rules};
use omorfos_core::{Effects, KeyChains, Pattern, Placement, Rule, Settings};
use smithay::input::keyboard::{ModifiersState, xkb};

/// One step of a key binding: modifiers held while a key is pressed.
///
/// The key is identified by the keysym it produces without modifiers, so
/// `Super+Shift+r` is the `r` key with Shift held, not the symbol `R`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The modifiers held.
    pub modifiers: Modifiers,
    /// The raw XKB keysym of the key.
    pub keysym: u32,
}

/// The key bindings in the form the compositor matches key presses against.
pub type Bindings = KeyChains<Chord, Action>;

/// Converts the general configuration into domain settings.
#[must_use]
pub fn settings(general: &General) -> Settings {
    Settings {
        focus_follows_pointer: general.focus.model == FocusModel::Sloppy,
        raise_on_focus: general.focus.raise_on_focus,
        placement: match general.placement {
            omorfos_config::Placement::Cascade => Placement::Cascade,
            omorfos_config::Placement::Center => Placement::Center,
            omorfos_config::Placement::Pointer => Placement::Pointer,
        },
        ..Settings::default()
    }
}

/// Converts configured rules into domain rules.
///
/// Workspace numbers count from one in configuration and from zero in the
/// domain.
#[must_use]
pub fn rules(rules: &Rules) -> Vec<Rule> {
    let convert = |rule: &omorfos_config::Rule| Rule {
        app_id: rule.matches.app_id().map(Pattern::new),
        title: rule.matches.title().map(Pattern::new),
        effects: Effects {
            workspace: rule.workspace.map(workspace_index),
            maximized: rule.maximized,
            fullscreen: rule.fullscreen,
            decorations: rule.decorations,
        },
    };
    rules.rules.iter().map(convert).collect()
}

/// Converts a one-based workspace number into a zero-based index.
#[must_use]
pub fn workspace_index(number: omorfos_config::WorkspaceNumber) -> usize {
    usize::try_from(number.get())
        .unwrap_or(usize::MAX)
        .saturating_sub(1)
}

/// Converts configured key bindings into key chains.
///
/// A binding that names a key XKB does not know is left out, and described
/// in the returned list of problems so the user can be told.
#[must_use]
pub fn bindings(keys: &Keys) -> (Bindings, Vec<String>) {
    let mut problems = Vec::new();
    let mut chains = Vec::new();
    for binding in keys.bindings() {
        let chords: Option<Vec<Chord>> = binding
            .keys
            .chords()
            .iter()
            .map(|chord| {
                keysym(&chord.key).map(|keysym| Chord {
                    modifiers: chord.modifiers,
                    keysym,
                })
            })
            .collect();
        match chords {
            Some(chords) => chains.push((chords, binding.action.clone())),
            None => problems.push(format!(
                "keys.yaml: `{}` names a key that does not exist; the binding is ignored",
                binding.keys
            )),
        }
    }
    (KeyChains::new(chains), problems)
}

/// Looks up the keysym a key name refers to.
///
/// Names are XKB keysym names. The exact spelling is tried first, then any
/// capitalisation. A single letter always means the unshifted key, because
/// bindings are matched on the key rather than the symbol it produces.
fn keysym(name: &str) -> Option<u32> {
    let name = if name.len() == 1 {
        name.to_ascii_lowercase()
    } else {
        name.to_owned()
    };
    [xkb::KEYSYM_NO_FLAGS, xkb::KEYSYM_CASE_INSENSITIVE]
        .into_iter()
        .map(|flags| xkb::keysym_from_name(&name, flags).raw())
        .find(|raw| *raw != xkb::keysyms::KEY_NoSymbol)
}

/// Converts the modifier state Smithay reports into configuration terms.
#[must_use]
pub fn modifiers(state: &ModifiersState) -> Modifiers {
    [
        (state.ctrl, Modifiers::CTRL),
        (state.alt, Modifiers::ALT),
        (state.shift, Modifiers::SHIFT),
        (state.logo, Modifiers::SUPER),
    ]
    .into_iter()
    .filter(|(held, _)| *held)
    .fold(Modifiers::NONE, |all, (_, modifier)| all.with(modifier))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use omorfos_config::{Command, Config};
    use omorfos_core::KeyOutcome;
    use smithay::input::keyboard::Keysym;

    use super::*;

    /// Loads a configuration made of one file.
    fn config(file: &str, yaml: &str) -> Config {
        let directory = std::env::temp_dir().join(format!("omorfos-bridge-{}", fastrand()));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(file), yaml).unwrap();
        let (config, diagnostics) = Config::load(Path::new(&directory), &Config::default());
        fs::remove_dir_all(&directory).unwrap();
        assert_eq!(diagnostics, []);
        config
    }

    /// A process-unique number, so tests running in parallel do not collide.
    fn fastrand() -> String {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let next = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!("{}-{next}", std::process::id())
    }

    fn chord(modifiers: Modifiers, keysym: Keysym) -> Chord {
        Chord {
            modifiers,
            keysym: keysym.raw(),
        }
    }

    #[test]
    fn default_settings_are_click_to_focus_with_raise_and_cascade() {
        assert_eq!(settings(&General::default()), Settings::default());
    }

    #[test]
    fn focus_and_placement_settings_carry_over() {
        let yaml = "focus: { model: sloppy, raise_on_focus: false }\nplacement: center\n";
        assert_eq!(
            settings(&config("config.yaml", yaml).general),
            Settings {
                focus_follows_pointer: true,
                raise_on_focus: false,
                placement: Placement::Center,
                ..Settings::default()
            }
        );
        let general = config("config.yaml", "placement: pointer\n").general;
        assert_eq!(settings(&general).placement, Placement::Pointer);
    }

    #[test]
    fn rules_carry_over_with_zero_based_workspaces() {
        let yaml = "
rules:
  - match: { app_id: 'org.gnome.*', title: '*Settings' }
    workspace: 3
    maximized: true
  - match: { app_id: mpv }
    fullscreen: true
    decorations: false
";
        let rules = rules(&config("rules.yaml", yaml).rules);
        assert_eq!(
            rules,
            [
                Rule {
                    app_id: Some(Pattern::new("org.gnome.*")),
                    title: Some(Pattern::new("*Settings")),
                    effects: Effects {
                        workspace: Some(2),
                        maximized: Some(true),
                        ..Effects::default()
                    },
                },
                Rule {
                    app_id: Some(Pattern::new("mpv")),
                    title: None,
                    effects: Effects {
                        fullscreen: Some(true),
                        decorations: Some(false),
                        ..Effects::default()
                    },
                },
            ]
        );
    }

    #[test]
    fn the_built_in_bindings_all_resolve() {
        let (_, problems) = bindings(&Keys::default());
        assert_eq!(problems, [""; 0]);
    }

    #[test]
    fn bindings_trigger_on_the_unshifted_key_with_the_named_modifiers() {
        let (mut bindings, _) = bindings(&Keys::default());
        let terminal = chord(Modifiers::SUPER, Keysym::Return);
        let spawn_terminal = Action::Spawn(Command::terminal());
        assert_eq!(
            bindings.press(terminal),
            KeyOutcome::Triggered(&spawn_terminal)
        );
        let reload = chord(Modifiers::SUPER.with(Modifiers::SHIFT), Keysym::r);
        assert_eq!(
            bindings.press(reload),
            KeyOutcome::Triggered(&Action::ReloadConfig)
        );
        assert_eq!(
            bindings.press(chord(Modifiers::NONE, Keysym::r)),
            KeyOutcome::Unbound
        );
    }

    #[test]
    fn key_names_accept_any_capitalisation_and_letters_mean_the_key() {
        let yaml = "
bindings:
  - { keys: Super+return, action: close }
  - { keys: Super+Q, action: quit }
  - { keys: XF86AudioMute x, action: maximize }
";
        let (mut bindings, problems) = bindings(&config("keys.yaml", yaml).keys);
        assert_eq!(problems, [""; 0]);
        let close = chord(Modifiers::SUPER, Keysym::Return);
        assert_eq!(bindings.press(close), KeyOutcome::Triggered(&Action::Close));
        let quit = chord(Modifiers::SUPER, Keysym::q);
        assert_eq!(bindings.press(quit), KeyOutcome::Triggered(&Action::Quit));
        let mute = chord(Modifiers::NONE, Keysym::XF86_AudioMute);
        assert_eq!(bindings.press(mute), KeyOutcome::Pending);
        let x = chord(Modifiers::NONE, Keysym::x);
        assert_eq!(bindings.press(x), KeyOutcome::Triggered(&Action::Maximize));
    }

    #[test]
    fn a_binding_naming_an_unknown_key_is_dropped_and_reported() {
        let yaml = "
bindings:
  - { keys: Super+Retrun, action: close }
  - { keys: Super+q, action: quit }
";
        let (mut bindings, problems) = bindings(&config("keys.yaml", yaml).keys);
        assert_eq!(
            problems,
            ["keys.yaml: `Super+Retrun` names a key that does not exist; the binding is ignored"]
        );
        let quit = chord(Modifiers::SUPER, Keysym::q);
        assert_eq!(bindings.press(quit), KeyOutcome::Triggered(&Action::Quit));
    }

    #[test]
    fn modifier_state_carries_over() {
        assert_eq!(modifiers(&ModifiersState::default()), Modifiers::NONE);
        let held = ModifiersState {
            ctrl: true,
            alt: true,
            shift: true,
            logo: true,
            ..Default::default()
        };
        let all = Modifiers::CTRL
            .with(Modifiers::ALT)
            .with(Modifiers::SHIFT)
            .with(Modifiers::SUPER);
        assert_eq!(modifiers(&held), all);
        let logo = ModifiersState {
            logo: true,
            caps_lock: true,
            ..Default::default()
        };
        assert_eq!(modifiers(&logo), Modifiers::SUPER);
    }

    #[test]
    fn workspace_numbers_become_indices() {
        assert_eq!(
            workspace_index(omorfos_config::WorkspaceNumber::clamped(1)),
            0
        );
        assert_eq!(
            workspace_index(omorfos_config::WorkspaceNumber::clamped(32)),
            31
        );
    }
}
