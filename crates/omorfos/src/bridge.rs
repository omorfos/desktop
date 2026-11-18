//! Translating configuration into the domain model's terms.
//!
//! `omorfos-config` and `omorfos-core` do not know about each other. This
//! module is where a configuration value becomes domain settings, rules and
//! key chains.

use std::path::Path;

use omorfos_config::{
    Action, Application, Config, Diagnostic, FocusModel, Keys, Modifiers, OutputScale, Part, Rules,
    States,
};
use omorfos_core::{
    Button, Effects, Item, KeyChains, Keycap, Mark, Metrics, Pattern, Placement, Private, Rule,
    Settings, Sign, Standing,
};
use omorfos_i18n::Words;
use smithay::input::keyboard::{ModifiersState, xkb};

use crate::Scale;

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

/// Converts configuration into domain settings: behaviour from the general
/// configuration, decoration sizes from the theme.
#[must_use]
pub fn settings(config: &Config) -> Settings {
    let general = &config.general;
    let theme = &config.theme;
    Settings {
        focus_follows_pointer: general.focus.model == FocusModel::Sloppy,
        raise_on_focus: general.focus.raise_on_focus,
        placement: match general.placement {
            omorfos_config::Placement::Cascade => Placement::Cascade,
            omorfos_config::Placement::Center => Placement::Center,
            omorfos_config::Placement::Pointer => Placement::Pointer,
        },
        decoration: Metrics {
            border: theme.style(Part::Window, States::NONE).border.top.width,
            titlebar: theme.style(Part::Titlebar, States::NONE).height,
            buttons: BUTTONS.map(|(_, part)| {
                // A button is as wide as it says, with its margin around
                // it. One with no width is not there, margin and all.
                let style = theme.style(part, States::NONE);
                let wide = Some(style.min_width).filter(|wide| *wide > 0);
                wide.map_or(0, |wide| wide.saturating_add(style.margin.across()))
            }),
        },
    }
}

/// The buttons of a titlebar, from the left, and the part of the theme
/// that says how each looks.
pub(crate) const BUTTONS: [(Button, Part); 3] = [
    (Button::Close, Part::ButtonClose),
    (Button::Minimize, Part::ButtonMinimize),
    (Button::Maximize, Part::ButtonMaximize),
];

/// How a window a rule calls private is hidden, as the domain has it.
const fn private(private: omorfos_config::Private) -> Private {
    match private {
        omorfos_config::Private::Not => Private::Not,
        omorfos_config::Private::Blank => Private::Blank,
        omorfos_config::Private::Blur => Private::Blur,
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
            private: rule.private.map(private),
            shaded: rule.shaded,
            above: rule.above,
            sticky: rule.sticky,
        },
    };
    rules.rules.iter().map(convert).collect()
}

/// The messages the entries of the built-in root menu are said by, in its
/// order.
const BUILT_IN: [&str; 13] = [
    "run",
    "browser",
    "files",
    "editor",
    "terminal",
    "settings",
    "stay-awake",
    "about-omorfos",
    "lock-screen",
    "sleep",
    "log-out-session",
    "restart-machine",
    "shut-down-machine",
];

/// Converts the configured root menu into the domain's menu items. An
/// entry just as the built-in menu has it, its label and its action, is
/// said in the user's language, as the configuration's defaults are
/// English; one named otherwise keeps what it is named.
#[must_use]
pub fn menu_items(
    items: &[omorfos_config::MenuItem],
    (on, keys): (Toggled, &Keys),
    words: &Words,
) -> Vec<Item<Action>> {
    let built_in = omorfos_config::Menu::default();
    let entries = built_in.items.into_iter().filter_map(|item| match item {
        omorfos_config::MenuItem::Entry { label, action, .. } => Some((label, action)),
        _ => None,
    });
    let said = entries.zip(BUILT_IN);
    let said = said.map(|((label, action), name)| (label, action, words.say(name, &[])));
    converted(items, (on, keys), &said.collect::<Vec<_>>())
}

/// Converts `items`, saying an entry `said` has, by its label and action,
/// as it says it.
fn converted(
    items: &[omorfos_config::MenuItem],
    (on, keys): (Toggled, &Keys),
    said: &[(String, Action, String)],
) -> Vec<Item<Action>> {
    let convert = |item: &omorfos_config::MenuItem| match item {
        omorfos_config::MenuItem::Entry {
            label,
            action,
            icon,
        } => Item::Entry {
            label: said
                .iter()
                .find(|(english, built, _)| english == label && built == action)
                .map_or(label, |(_, _, said)| said)
                .clone(),
            action: action.clone(),
            mark: icon.clone().map_or_else(|| mark(action, on), Mark::Icon),
            keys: keycaps(keys, action),
        },
        omorfos_config::MenuItem::Submenu { label, items, icon } => Item::Submenu {
            label: label.clone(),
            items: converted(items, (on, keys), said),
            mark: icon.clone().map_or(Mark::Blank, Mark::Icon),
        },
        omorfos_config::MenuItem::Separator => Item::Separator,
    };
    items.iter().map(convert).collect()
}

/// The keys of the first binding that does `action`, as a menu shows them
/// at the end of its row: Super, Ctrl, Alt and Shift, in that order, then
/// the key, each by its sign where it has one, and what is written on it
/// otherwise, as `Q`; the chords of a chain apart. None if no binding does
/// it.
#[must_use]
pub fn keycaps(keys: &Keys, action: &Action) -> Vec<Keycap> {
    let held = [
        (Modifiers::SUPER, Sign::Super),
        (Modifiers::CTRL, Sign::Ctrl),
        (Modifiers::ALT, Sign::Alt),
        (Modifiers::SHIFT, Sign::Shift),
    ];
    let mut bindings = keys.bindings().iter();
    let binding = bindings.find(|binding| binding.action == *action);
    let chords = binding
        .into_iter()
        .flat_map(|binding| binding.keys.chords());
    let mut caps = Vec::new();
    for (index, chord) in chords.enumerate() {
        // A chord after another, as in a chain, stands apart from it.
        caps.extend((index > 0).then(|| Keycap::Text(" ".to_owned())));
        let modifiers = held
            .iter()
            .filter(|(modifier, _)| chord.modifiers.contains(*modifier));
        caps.extend(modifiers.map(|(_, sign)| Keycap::Sign(*sign)));
        caps.push(keycap(&chord.key));
    }
    caps
}

/// How a key, by its XKB name, is shown among the keys at a row's end.
fn keycap(key: &str) -> Keycap {
    let sign = match key {
        "Return" | "KP_Enter" => Some(Sign::Enter),
        "space" => Some(Sign::Space),
        "Tab" => Some(Sign::Tab),
        "Up" => Some(Sign::Up),
        "Down" => Some(Sign::Down),
        "Left" => Some(Sign::Left),
        "Right" => Some(Sign::Right),
        "BackSpace" => Some(Sign::Backspace),
        _ => None,
    };
    let written = match key {
        "Delete" => "Del".to_owned(),
        "Escape" => "Esc".to_owned(),
        // Begun with a capital, as on the key.
        other => {
            let first = other.chars().take(1).flat_map(char::to_uppercase);
            first.chain(other.chars().skip(1)).collect()
        }
    };
    sign.map_or(Keycap::Text(written), Keycap::Sign)
}

/// What is on, of what the rows of a menu turn on and off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Toggled {
    /// How the window with focus stands.
    pub standing: Standing,
    /// Whether the window with focus is maximized.
    pub maximized: bool,
    /// Whether the session is kept awake.
    pub awake: bool,
}

/// What stands before the label of a row of a menu that does `action`: a
/// sign for what it does, pressed in if it turns on what `on` has on, or
/// the icon of the program it starts. A program is known by the name of
/// its file, as its desktop entry is.
#[must_use]
pub fn mark(action: &Action, on: Toggled) -> Mark {
    use omorfos_config::Half;
    match action {
        Action::Spawn(started) => {
            let program = started.run.program();
            let name = program.rsplit('/').next().unwrap_or(program);
            Mark::Program(name.to_owned())
        }
        Action::Maximize => Mark::Toggle(Sign::Expand, on.maximized),
        Action::Shade => Mark::Toggle(Sign::Shade, on.standing.shaded),
        Action::KeepAbove => Mark::Toggle(Sign::Pin, on.standing.above),
        Action::Stick => Mark::Toggle(Sign::Everywhere, on.standing.sticky),
        Action::StayAwake | Action::StayAwakeFor(_) => Mark::Toggle(Sign::Awake, on.awake),
        Action::Close => Mark::Sign(Sign::Close),
        Action::Minimize => Mark::Sign(Sign::Minimize),
        Action::Fullscreen => Mark::Sign(Sign::Fullscreen),
        Action::Tile(Half::Left) => Mark::Sign(Sign::LeftHalf),
        Action::Tile(Half::Right) => Mark::Sign(Sign::RightHalf),
        Action::Tile(Half::Top) => Mark::Sign(Sign::TopHalf),
        Action::Tile(Half::Bottom) => Mark::Sign(Sign::BottomHalf),
        Action::NewWorkspace | Action::MoveToNewWorkspace => Mark::Sign(Sign::Add),
        Action::ShowLauncher => Mark::Sign(Sign::Search),
        Action::ReloadConfig => Mark::Sign(Sign::Reload),
        Action::About | Action::ShowWelcome => Mark::Sign(Sign::About),
        Action::Lock => Mark::Sign(Sign::Lock),
        Action::Quit | Action::LogOut => Mark::Sign(Sign::Quit),
        Action::Sleep => Mark::Sign(Sign::Sleep),
        Action::Open(Application::Browser) => Mark::Sign(Sign::Browser),
        Action::Open(Application::Files) => Mark::Sign(Sign::Files),
        Action::Open(Application::Editor) => Mark::Sign(Sign::Editor),
        Action::Open(Application::Settings) => Mark::Sign(Sign::Settings),
        Action::Restart => Mark::Sign(Sign::Restart),
        Action::ShutDown => Mark::Sign(Sign::ShutDown),
        _ => Mark::Blank,
    }
}

/// Says which half of the screen the configuration means, as the desktop
/// model has it.
#[must_use]
pub const fn half(half: omorfos_config::Half) -> omorfos_core::Half {
    match half {
        omorfos_config::Half::Left => omorfos_core::Half::Left,
        omorfos_config::Half::Right => omorfos_core::Half::Right,
        omorfos_config::Half::Top => omorfos_core::Half::Top,
        omorfos_config::Half::Bottom => omorfos_core::Half::Bottom,
    }
}

/// Converts a one-based workspace number into a zero-based index.
#[must_use]
pub fn workspace_index(number: omorfos_config::WorkspaceNumber) -> usize {
    usize::try_from(number.get())
        .unwrap_or(usize::MAX)
        .saturating_sub(1)
}

/// Settles the scale to draw at: the configured one, or failing that what
/// the backend `suggested` for the screen.
#[must_use]
pub fn scale(configured: OutputScale, suggested: Scale) -> Scale {
    match configured {
        OutputScale::Auto => suggested,
        OutputScale::Quarters(quarters) => Scale::new(f64::from(quarters) / 4.0),
    }
}

/// Converts configured key bindings into key chains.
///
/// A binding that names a key XKB does not know is left out, and described
/// in the returned list of problems so the user can be told. `file` is where
/// the bindings were read from, for those problems to point at.
#[must_use]
pub fn bindings(keys: &Keys, file: &Path) -> (Bindings, Vec<Diagnostic>) {
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
            None => problems.push(Diagnostic {
                file: file.to_owned(),
                position: binding.position,
                message: format!(
                    "`{}` names a key that does not exist; the binding is ignored",
                    binding.keys
                ),
            }),
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

    use omorfos_config::Command;
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
    fn default_settings_are_click_to_focus_with_raise_cascade_and_the_default_frame() {
        let expected = Settings {
            focus_follows_pointer: false,
            raise_on_focus: true,
            placement: Placement::Cascade,
            decoration: Metrics {
                border: 1,
                titlebar: 48,
                buttons: [32, 24, 24],
            },
        };
        assert_eq!(settings(&Config::default()), expected);
    }

    #[test]
    fn decoration_sizes_come_from_the_theme() {
        let theme = config(
            "theme.css",
            "window { border-width: 3px; }\ntitlebar { height: 40px; }\n\
             button-close { min-width: 16px; margin: 0 2px; }\n\
             button-maximize { min-width: 0; }\n",
        );
        assert_eq!(
            settings(&theme).decoration,
            Metrics {
                border: 3,
                titlebar: 40,
                // One as wide as it says with its margin, one as the
                // built-in theme has it, and one that is not there.
                buttons: [20, 24, 0],
            }
        );
    }

    #[test]
    fn focus_and_placement_settings_carry_over() {
        let yaml = "focus: { model: sloppy, raise_on_focus: false }\nplacement: center\n";
        assert_eq!(
            settings(&config("config.yaml", yaml)),
            Settings {
                focus_follows_pointer: true,
                raise_on_focus: false,
                placement: Placement::Center,
                decoration: Metrics {
                    border: 1,
                    titlebar: 48,
                    buttons: [32, 24, 24],
                },
            }
        );
        let pointer = config("config.yaml", "placement: pointer\n");
        assert_eq!(settings(&pointer).placement, Placement::Pointer);
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
  - match: { title: '*Bank*' }
    private: true
  - match: { app_id: photos }
    private: blur
  - match: { app_id: photos, title: Wallpapers }
    private: false
  - match: { app_id: notes }
    shaded: true
    above: true
    sticky: false
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
                Rule {
                    app_id: None,
                    title: Some(Pattern::new("*Bank*")),
                    effects: Effects {
                        private: Some(Private::Blank),
                        ..Effects::default()
                    },
                },
                Rule {
                    app_id: Some(Pattern::new("photos")),
                    title: None,
                    effects: Effects {
                        private: Some(Private::Blur),
                        ..Effects::default()
                    },
                },
                Rule {
                    app_id: Some(Pattern::new("photos")),
                    title: Some(Pattern::new("Wallpapers")),
                    effects: Effects {
                        private: Some(Private::Not),
                        ..Effects::default()
                    },
                },
                Rule {
                    app_id: Some(Pattern::new("notes")),
                    title: None,
                    effects: Effects {
                        shaded: Some(true),
                        above: Some(true),
                        sticky: Some(false),
                        ..Effects::default()
                    },
                },
            ]
        );
    }

    #[test]
    fn the_menu_carries_over_with_its_submenus_and_what_stands_before_each_label() {
        let yaml = "
items:
  - { label: Terminal, action: { spawn: [foot] } }
  - separator
  - label: Session
    items:
      - { label: Quit, action: quit }
      - { label: Away, action: quit, icon: system-suspend }
  - { label: Places, items: [], icon: folder }
";
        // Each with the keys the built-in bindings give what it does.
        let quit = |label: &str, mark| Item::Entry {
            label: label.to_owned(),
            action: Action::Quit,
            mark,
            keys: vec![
                Keycap::Sign(Sign::Super),
                Keycap::Sign(Sign::Shift),
                Keycap::Text("E".to_owned()),
            ],
        };
        let spawn = Action::Spawn(Command::terminal().into());
        let on = Toggled {
            standing: Standing::default(),
            maximized: false,
            awake: false,
        };
        assert_eq!(
            menu_items(
                &config("menu.yaml", yaml).menu.items,
                (on, &Keys::default()),
                &english()
            ),
            [
                Item::Entry {
                    label: "Terminal".to_owned(),
                    action: spawn,
                    mark: Mark::Program("foot".to_owned()),
                    keys: vec![Keycap::Sign(Sign::Super), Keycap::Sign(Sign::Enter)],
                },
                Item::Separator,
                Item::Submenu {
                    label: "Session".to_owned(),
                    items: vec![
                        quit("Quit", Mark::Sign(Sign::Quit)),
                        quit("Away", Mark::Icon("system-suspend".to_owned())),
                    ],
                    mark: Mark::Blank,
                },
                Item::Submenu {
                    label: "Places".to_owned(),
                    items: Vec::new(),
                    mark: Mark::Icon("folder".to_owned()),
                },
            ]
        );
    }

    fn english() -> Words {
        Words::in_languages(&omorfos_i18n::COMPOSITOR, &[])
    }

    fn labels(items: &[Item<Action>]) -> Vec<&str> {
        items.iter().filter_map(label).collect()
    }

    fn label(item: &Item<Action>) -> Option<&str> {
        match item {
            Item::Entry { label, .. } | Item::Submenu { label, .. } => Some(label.as_str()),
            Item::Separator => None,
        }
    }

    fn off() -> Toggled {
        Toggled {
            standing: Standing::default(),
            maximized: false,
            awake: false,
        }
    }

    #[test]
    fn the_keys_of_an_action_are_shown_by_their_signs_or_what_is_written_on_them() {
        let yaml = "
defaults: false
bindings:
  - { keys: Super+space, action: show_launcher }
  - { keys: Ctrl+Alt+Shift+Return, action: lock }
  - { keys: Super+x t, action: close }
  - { keys: Super+Shift+q, action: close }
  - { keys: Super+Tab, action: switch_windows }
  - { keys: Super+Up, action: maximize }
  - { keys: Super+Down, action: minimize }
  - { keys: Super+Left, action: previous_workspace }
  - { keys: Super+Right, action: next_workspace }
  - { keys: Super+BackSpace, action: quit }
  - { keys: Super+Delete, action: about }
  - { keys: Escape, action: show_menu }
  - { keys: KP_Enter, action: show_welcome }
";
        let keys = config("keys.yaml", yaml).keys;
        let caps = |action: &Action| keycaps(&keys, action);
        let sign = Keycap::Sign;
        let text = |written: &str| Keycap::Text(written.to_owned());
        assert_eq!(
            caps(&Action::ShowLauncher),
            [sign(Sign::Super), sign(Sign::Space)]
        );
        assert_eq!(
            caps(&Action::Lock),
            [
                sign(Sign::Ctrl),
                sign(Sign::Alt),
                sign(Sign::Shift),
                sign(Sign::Enter)
            ],
            "Super, Ctrl, Alt, Shift: in that order"
        );
        assert_eq!(
            caps(&Action::Close),
            [sign(Sign::Super), text("X"), text(" "), text("T")],
            "the first binding, a chain, its chords apart"
        );
        let signed = [
            (Action::SwitchWindows, Sign::Tab),
            (Action::Maximize, Sign::Up),
            (Action::Minimize, Sign::Down),
            (Action::PreviousWorkspace, Sign::Left),
            (Action::NextWorkspace, Sign::Right),
            (Action::Quit, Sign::Backspace),
        ];
        for (action, key) in signed {
            assert_eq!(caps(&action), [sign(Sign::Super), sign(key)]);
        }
        assert_eq!(caps(&Action::About), [sign(Sign::Super), text("Del")]);
        assert_eq!(caps(&Action::ShowMenu), [text("Esc")]);
        assert_eq!(caps(&Action::ShowWelcome), [sign(Sign::Enter)]);
        assert_eq!(caps(&Action::ReloadConfig), [], "none bound");
    }

    #[test]
    fn the_built_in_menu_is_said_in_the_users_language() {
        let polish = Words::in_languages(&omorfos_i18n::COMPOSITOR, &["pl".to_owned()]);
        let built_in = omorfos_config::Menu::default();
        assert_eq!(
            labels(&menu_items(
                &built_in.items,
                (off(), &Keys::default()),
                &polish
            )),
            [
                "Uruchom\u{2026}",
                "Przeglądarka",
                "Pliki",
                "Edytor",
                "Terminal",
                "Ustawienia",
                "Nie usypiaj",
                "O omorfos",
                "Zablokuj ekran",
                "Uśpij",
                "Wyloguj\u{2026}",
                "Uruchom ponownie\u{2026}",
                "Wyłącz\u{2026}",
            ]
        );
    }

    #[test]
    fn every_entry_of_the_built_in_menu_is_said_by_a_message_whose_english_is_its_label() {
        let built_in = omorfos_config::Menu::default();
        let entries = built_in.items.iter();
        let configured = entries.filter_map(|item| match item {
            omorfos_config::MenuItem::Entry { label, .. } => Some(label.as_str()),
            _ => None,
        });
        let said = menu_items(&built_in.items, (off(), &Keys::default()), &english());
        let configured: Vec<&str> = configured.collect();
        assert_eq!(configured.len(), BUILT_IN.len());
        assert_eq!(labels(&said), configured);
    }

    #[test]
    fn an_entry_the_configuration_names_or_points_elsewhere_keeps_its_label_in_every_language() {
        let yaml = "
items:
  - { label: 'Log out…', action: about }
  - { label: Leave, action: log_out }
  - label: Session
    items:
      - { label: 'Log out…', action: log_out }
";
        let polish = Words::in_languages(&omorfos_i18n::COMPOSITOR, &["pl".to_owned()]);
        let items = menu_items(
            &config("menu.yaml", yaml).menu.items,
            (off(), &Keys::default()),
            &polish,
        );
        assert_eq!(labels(&items), ["Log out\u{2026}", "Leave", "Session"]);
        let quit = Item::Entry {
            label: "Wyloguj\u{2026}".to_owned(),
            action: Action::LogOut,
            mark: Mark::Sign(Sign::Quit),
            keys: Vec::new(),
        };
        let session = Item::Submenu {
            label: "Session".to_owned(),
            items: vec![quit],
            mark: Mark::Blank,
        };
        assert_eq!(items.get(2), Some(&session));
    }

    #[test]
    fn the_configured_scale_wins_over_what_suits_the_screen() {
        let dense = Scale::new(2.0);
        assert_eq!(scale(OutputScale::Auto, dense), dense);
        assert_eq!(scale(OutputScale::Quarters(6), dense), Scale::new(1.5));
        assert_eq!(scale(OutputScale::Quarters(4), dense), Scale::ONE);
    }

    #[test]
    fn the_built_in_bindings_all_resolve() {
        let (_, problems) = bindings(&Keys::default(), Path::new("keys.yaml"));
        assert_eq!(problems, []);
    }

    #[test]
    fn bindings_trigger_on_the_unshifted_key_with_the_named_modifiers() {
        let (mut bindings, _) = bindings(&Keys::default(), Path::new("keys.yaml"));
        let terminal = chord(Modifiers::SUPER, Keysym::Return);
        let spawn_terminal = Action::Spawn(Command::terminal().into());
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
        let keys = config("keys.yaml", yaml).keys;
        let (mut bindings, problems) = bindings(&keys, Path::new("keys.yaml"));
        assert_eq!(problems, []);
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
        let keys = config("keys.yaml", yaml).keys;
        let (mut bindings, problems) = bindings(&keys, Path::new("/etc/keys.yaml"));
        assert_eq!(
            problems.iter().map(ToString::to_string).collect::<Vec<_>>(),
            [
                "/etc/keys.yaml:3:13: `Super+Retrun` names a key that does not exist; \
              the binding is ignored"
            ]
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
    fn a_menu_row_is_signed_for_what_it_does_and_pressed_in_for_what_is_on() {
        use omorfos_config::{Bounded, Half, Started};

        let off = Toggled {
            standing: Standing::default(),
            maximized: false,
            awake: false,
        };
        let on = Toggled {
            standing: Standing {
                shaded: true,
                above: true,
                sticky: true,
            },
            maximized: true,
            awake: true,
        };
        let toggles = [
            (Action::Maximize, Sign::Expand),
            (Action::Shade, Sign::Shade),
            (Action::KeepAbove, Sign::Pin),
            (Action::Stick, Sign::Everywhere),
            (Action::StayAwake, Sign::Awake),
            (Action::StayAwakeFor(Bounded::clamped(60)), Sign::Awake),
        ];
        for (action, sign) in toggles {
            assert_eq!(mark(&action, off), Mark::Toggle(sign, false), "{action:?}");
            assert_eq!(mark(&action, on), Mark::Toggle(sign, true), "{action:?}");
        }
        let signed = [
            (Action::Close, Sign::Close),
            (Action::Minimize, Sign::Minimize),
            (Action::Fullscreen, Sign::Fullscreen),
            (Action::Tile(Half::Left), Sign::LeftHalf),
            (Action::Tile(Half::Right), Sign::RightHalf),
            (Action::Tile(Half::Top), Sign::TopHalf),
            (Action::Tile(Half::Bottom), Sign::BottomHalf),
            (Action::NewWorkspace, Sign::Add),
            (Action::MoveToNewWorkspace, Sign::Add),
            (Action::ShowLauncher, Sign::Search),
            (Action::ReloadConfig, Sign::Reload),
            (Action::About, Sign::About),
            (Action::Lock, Sign::Lock),
            (Action::Quit, Sign::Quit),
        ];
        for (action, sign) in signed {
            assert_eq!(mark(&action, on), Mark::Sign(sign), "{action:?}");
        }
        assert_eq!(mark(&Action::FocusNext, on), Mark::Blank, "nothing to show");
        let started = |program: &str| {
            let command = Command::new(program, Vec::<String>::new()).unwrap();
            Action::Spawn(Started::from(command))
        };
        for program in ["foot", "/usr/bin/foot"] {
            let program = mark(&started(program), off);
            assert_eq!(program, Mark::Program("foot".to_owned()), "by its name");
        }
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

    #[test]
    fn a_half_of_the_screen_is_the_same_half_to_the_desktop() {
        use omorfos_config::Half as Asked;
        use omorfos_core::Half as Meant;
        let halves = [
            (Asked::Left, Meant::Left),
            (Asked::Right, Meant::Right),
            (Asked::Top, Meant::Top),
            (Asked::Bottom, Meant::Bottom),
        ];
        for (asked, meant) in halves {
            assert_eq!(half(asked), meant);
        }
    }
}
