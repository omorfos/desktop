//! The welcome card of the omorfos desktop: shown at the first login, and
//! whenever it is asked for from the menu, so that the first thing seen is
//! not an empty screen.
//!
//! It goes a step at a time: a welcome; what the few keys that matter do,
//! as `keys.yaml` has them; light, dark or with the day, and the
//! wallpapers that come with omorfos, each saved to `config.yaml` as it is
//! chosen, so that the desktop behind the card changes at once (ADR 81);
//! and the applications that open each kind of thing, as the system says.
//! Closed, skipped or finished, it is not shown at a login again.
//!
//! [`Card`] is all of the card that decides anything; the rest finds what
//! it shows and carries keys in and pictures out.

mod card;
mod client;

use omorfos_entries::defaults;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use omorfos_brand::{DESIGNS, Design, Scheme};
use omorfos_config::edit::{self, Value};
use omorfos_config::{
    Accent, Action, Command, Keys, Part, Picture, Scheme as Colours, SchemeChoice, States, Theme,
};
use omorfos_i18n::{Value as Given, Words};
use omorfos_ipc::help::About;
use omorfos_ui::Image;

pub use card::{Card, Choice, Focus, Key, Opener, Outcome, SCHEMES, Said, Shown, Step};
pub use client::{Failure, Save, Welcome};

/// What it says of itself, when asked and in its manual page.
pub const ABOUT: About = About {
    name: "omorfos-welcome",
    what: "The welcome card of the omorfos desktop: the keys that matter, a wallpaper, and light or dark.",
    usage: " [--first]",
    commands: &[],
    options: &[(
        "--first",
        "Show the card only if it has not been closed before, as at a login",
    )],
    examples: &[],
    notes: "The omorfos compositor starts it with --first each time it starts, and without when \
            Welcome is chosen from the menu. Each choice is saved to config.yaml as it is made, \
            the rest of the file kept as it is written.",
    version: env!("CARGO_PKG_VERSION"),
};

/// The file that says the card has been closed, in the session's state
/// directory.
pub const WELCOMED: &str = "welcomed";

/// How large each wallpaper is drawn to be chosen by, in pixels: about as
/// large as it is shown on a dense screen.
const MINIATURE: (u32, u32) = (192, 108);

/// Shows the card in the session the environment describes, until it is
/// closed; or, `first`, only if it has never been closed.
///
/// `variable` looks a variable of the environment up, so that this can be
/// used and tested without touching the environment: the Wayland socket is
/// `WAYLAND_DISPLAY` in `XDG_RUNTIME_DIR`, the configuration is where
/// `XDG_CONFIG_HOME` or `HOME` says, a file of it that does not load as
/// the compositor keeps it in `XDG_RUNTIME_DIR`, the mark of its having been closed
/// where `XDG_STATE_HOME` or `HOME` does, the wallpapers where
/// `XDG_DATA_HOME` and `XDG_DATA_DIRS` do, and it speaks the language
/// `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` or `LANG` asks for.
///
/// # Errors
///
/// Returns why the card could not be shown, or could not go on.
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>, first: bool) -> Result<(), Failure> {
    let get = |name: &str| variable(name);
    let (runtime, display) = (get("XDG_RUNTIME_DIR"), get("WAYLAND_DISPLAY"));
    let home = get("HOME");
    let wayland = runtime.as_deref().zip(display.as_deref());
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let config = omorfos_config::config_dir(get("XDG_CONFIG_HOME").as_deref(), home.as_deref());
    let state =
        omorfos_ipc::session_log::directory(get("XDG_STATE_HOME").as_deref(), home.as_deref());
    let Some(((wayland, config), state)) = wayland.zip(config).zip(state) else {
        return Err(
            "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is not set"
                .into(),
        );
    };
    let marked = state.join(WELCOMED);
    if first && marked.exists() {
        return Ok(());
    }
    let data = (get("XDG_DATA_HOME"), get("XDG_DATA_DIRS"));
    let places =
        omorfos_icons::data_directories(data.0.as_deref(), home.as_deref(), data.1.as_deref());
    let languages = omorfos_i18n::languages(&|name| get(name)?.into_string().ok());
    let words = Words::in_languages(&omorfos_i18n::WELCOME, &languages);
    let offered = offered(&places);
    // What it shows is what is in effect: the keys, scheme and wallpaper
    // the compositor goes by, which, where a file has gone wrong since it
    // last loaded, are of the copy it kept. A choice is still saved to the
    // file on disk, and where that does not load it is refused, the card
    // saying that the mistake is to be mended first.
    let store = omorfos_config::Store::open_in_session(&config, runtime.as_deref());
    let loaded = store.current();
    let home = home.map(PathBuf::from);
    let in_use = (
        in_use(
            loaded.general.wallpaper.picture.as_ref(),
            &offered,
            home.as_deref(),
        ),
        loaded.general.appearance.scheme,
    );
    let pictures = offered
        .iter()
        .filter_map(|(design, ..)| miniature(*design))
        .collect();
    let config_dirs = get("XDG_CONFIG_DIRS");
    let config_dirs = config_dirs.as_deref().and_then(std::ffi::OsStr::to_str);
    let configured = defaults::configuration(
        get("XDG_CONFIG_HOME").as_deref().map(Path::new),
        home.as_deref(),
        config_dirs,
    );
    let locales = omorfos_entries::locales(&languages);
    let mut icons = omorfos_icons::Icons::new(omorfos_icons::icon_directories(&places));
    let shown = Shown {
        mark: the_mark(),
        rows: rows(&loaded.keys, loaded.general.keyboard.super_as_ctrl, &words),
        schemes: schemes(loaded.general.appearance.accent),
        wallpapers: pictures,
        openers: openers((&configured, &places), &locales, &words, &mut icons),
    };
    let card = Card::new(said(&words), shown, in_use);
    let save = saving(config, offered, words);
    client::run(&wayland, card, (&loaded.theme, save), &marked)
}

/// Marks the card as closed, at `marked`, so that it is not shown at a
/// login again. Where it cannot be, it is shown again: nothing worse.
pub fn mark(marked: &Path) {
    let _ = marked.parent().map(fs::create_dir_all);
    let _ = fs::write(marked, "");
}

/// What the card says, from `words`.
fn said(words: &Words) -> Said {
    let say = |name: &str| words.say(name, &[]);
    Said {
        title: say("title"),
        greeting: say("greeting"),
        keys: say("keys"),
        scheme: say("scheme"),
        wallpaper: say("wallpaper"),
        applications: say("applications"),
        chosen_elsewhere: say("chosen-elsewhere"),
        none: say("none-set"),
        schemes: [say("dark"), say("light"), say("auto")],
        skip: say("skip"),
        back: say("back"),
        next: say("next"),
        finish: say("finish"),
    }
}

/// How large the mark is drawn, in pixels: twice as large as it is shown,
/// for a dense screen.
const MARK: u32 = 192;

/// The mark of omorfos, for the first step.
fn the_mark() -> Option<Image> {
    let png = omorfos_brand::draw(&omorfos_brand::mark(Scheme::Dark), (MARK, MARK));
    png.and_then(|png| omorfos_icons::decode_png(&png))
}

/// How large each picture of the desktop is drawn, in pixels.
const DESKTOP: (u32, u32) = (192, 108);

/// A small picture of the desktop in each of [`SCHEMES`], in its colours
/// with `accent`: with the day as dark and light, split from corner to
/// corner, light above.
#[must_use]
pub fn schemes(accent: Accent) -> Vec<Image> {
    let (width, height) = DESKTOP;
    let pixels = scheme_pixels(accent).into_iter();
    pixels
        .filter_map(|pixels| Image::new(width, height, pixels))
        .collect()
}

/// The pixels of [`schemes`]' pictures, as red, green, blue and opacity,
/// row by row.
fn scheme_pixels(accent: Accent) -> [Vec<u8>; 3] {
    let dark = desktop(&Theme::built_in(Colours::Dark, accent));
    let light = desktop(&Theme::built_in(Colours::Light, accent));
    let (width, height) = DESKTOP;
    let split = (0..height).flat_map(|y| (0..width).map(move |x| (x, y)));
    let both = split.zip(dark.chunks(4).zip(light.chunks(4)));
    let auto: Vec<u8> = both
        .flat_map(|((x, y), (dark, light))| {
            // Above the line from the bottom left corner to the top right.
            let above = u64::from(x) * u64::from(height) + u64::from(y) * u64::from(width)
                < u64::from(width) * u64::from(height);
            if above { light } else { dark }.to_vec()
        })
        .collect();
    [dark, light, auto]
}

/// The desktop in `theme`'s colours, drawn small: its background, a window
/// with its titlebar and lines of text, and a bar along the bottom with a
/// dot of the accent on it; as red, green, blue and opacity, row by row.
fn desktop(theme: &Theme) -> Vec<u8> {
    let (width, height) = DESKTOP;
    let colour = |part: Part| theme.style(part, States::NONE).ground();
    let text = theme.style(Part::LauncherDescription, States::NONE).color;
    let accent = theme
        .style(
            Part::LauncherItem,
            States::NONE.with(omorfos_config::State::Active),
        )
        .background;
    // Each part, by where it is in hundredths across and down, the last
    // over the rest.
    let parts = [
        ((0, 0, 100, 100), colour(Part::Desktop)),
        ((14, 12, 72, 66), colour(Part::Menu)),
        ((14, 12, 72, 12), colour(Part::Titlebar)),
        ((20, 34, 50, 4), text),
        ((20, 44, 40, 4), text),
        ((20, 54, 45, 4), text),
        ((0, 88, 100, 12), colour(Part::Panel)),
        ((3, 91, 4, 6), accent),
    ];
    let mut pixels = Vec::with_capacity(usize::try_from(width * height * 4).unwrap_or(0));
    for y in 0..height {
        for x in 0..width {
            let (across, down) = (x * 100 / width, y * 100 / height);
            let on = parts.iter().rev().find(|((left, top, wide, tall), _)| {
                (*left..left + wide).contains(&across) && (*top..top + tall).contains(&down)
            });
            let colour = on.map_or(colour(Part::Desktop), |(_, colour)| *colour);
            pixels.extend([colour.red, colour.green, colour.blue, 255]);
        }
    }
    pixels
}

/// What opens each kind of thing [`defaults::KINDS`] names, by default, as
/// the `mimeapps.list` files in the places of configuration and of data
/// say: the kind as `words` call it, and the application's name in
/// `locales` and its icon from `icons`, or none set.
#[must_use]
pub fn openers(
    (config, data): (&[PathBuf], &[PathBuf]),
    locales: &[String],
    words: &Words,
    icons: &mut omorfos_icons::Icons,
) -> Vec<Opener> {
    let lists = defaults::lists(config, data);
    let texts: Vec<String> = lists
        .iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect();
    let ink = omorfos_config::Color::rgba(0, 0, 0, 255);
    defaults::KINDS
        .iter()
        .map(|(kind, types)| {
            let entry = defaults::default_for(types, &texts);
            let path = entry.and_then(|entry| defaults::entry(&entry, data));
            let text = path.and_then(|path| fs::read_to_string(path).ok());
            let named = text.and_then(|text| defaults::named(&text, locales));
            let icon = named
                .as_ref()
                .and_then(|(_, icon)| icon.as_deref())
                .and_then(|icon| icons.get(icon, 64, ink).cloned());
            Opener {
                kind: words.say(kind, &[]),
                name: named.map(|(name, _)| name),
                icon,
            }
        })
        .collect()
}

/// The keys that matter as `keys` has them, and what each does in
/// `words`: one that is not bound to anything is left out, and of two
/// ways to the same the one with Super is said. Copying, cutting, pasting
/// and undoing with Super is the desktop's own (ADR 78), not a binding,
/// and said only where it is so, `super_as_ctrl`.
#[must_use]
pub fn rows(keys: &Keys, super_as_ctrl: bool, words: &Words) -> Vec<(String, String)> {
    let wanted = [
        (Action::Spawn(Command::terminal().into()), "terminal"),
        (Action::ShowLauncher, "launcher"),
        (Action::ShowMenu, "menu"),
        (Action::SwitchWindows, "switcher"),
    ];
    let bound = wanted.into_iter().filter_map(|(action, name)| {
        let bindings = keys
            .bindings()
            .iter()
            .filter(|binding| binding.action == action);
        // Of two ways to the same, the one with Super, which is the
        // desktop's (ADR 78), as Super+Tab over Alt+Tab.
        let written = bindings.map(|binding| binding.keys.to_string());
        let keys = written.min_by_key(|keys| !keys.starts_with("Super"))?;
        Some((keys, words.say(name, &[])))
    });
    let copying =
        super_as_ctrl.then(|| ("Super+c, x, v, z".to_owned(), words.say("copy-paste", &[])));
    bound.chain(copying).collect()
}

/// The wallpapers that come with omorfos, by design, with where each is
/// dark and light: those of the first of `places` that has any, in the
/// order omorfos offers them.
#[must_use]
pub fn offered(places: &[PathBuf]) -> Vec<(Design, PathBuf, PathBuf)> {
    let each = places.iter().map(|place| {
        let there = place.join("backgrounds/omorfos");
        let found = DESIGNS.into_iter().filter_map(|design| {
            let (dark, light) = (file(&there, design, "dark"), file(&there, design, "light"));
            (dark.is_file() && light.is_file()).then_some((design, dark, light))
        });
        found.collect::<Vec<_>>()
    });
    let mut each = each.filter(|found| !found.is_empty());
    each.next().unwrap_or_default()
}

/// Where the picture of `design` in the scheme `named` is in `there`.
fn file(there: &Path, design: Design, named: &str) -> PathBuf {
    there.join(format!("{}-{named}.png", design.name()))
}

/// Which of `offered` is in use, when `config.yaml` names `picture`: the
/// first, Prism, when it names none, as the wallpaper then shows it; the
/// one whose pictures it names; or none.
#[must_use]
pub fn in_use(
    picture: Option<&Picture>,
    offered: &[(Design, PathBuf, PathBuf)],
    home: Option<&Path>,
) -> Option<usize> {
    let Some(picture) = picture else {
        return offered
            .iter()
            .position(|(design, ..)| *design == Design::Prism);
    };
    let named = |dark: bool| picture.path(home, dark);
    let (dark, light) = (named(true), named(false));
    offered.iter().position(|(_, offered_dark, offered_light)| {
        dark.as_ref() == Some(offered_dark) && light.as_ref() == Some(offered_light)
    })
}

/// `design` drawn small, dark, to be chosen by.
fn miniature(design: Design) -> Option<Image> {
    let (width, height) = MINIATURE;
    let pixels = omorfos_brand::miniature(design, Scheme::Dark, MINIATURE);
    pixels.and_then(|pixels| Image::new(width, height, pixels))
}

/// Saves `choice` among `offered` to the `config.yaml` of `config`.
///
/// # Errors
///
/// Returns why the file was left as it was.
pub fn save(
    config: &Path,
    choice: Choice,
    offered: &[(Design, PathBuf, PathBuf)],
) -> Result<(), edit::Refused> {
    match choice {
        Choice::Scheme(scheme) => {
            let key: &[&str] = &["appearance", "scheme"];
            edit::change(config, &[(key, Value::Scalar(word(scheme)))])
        }
        Choice::Wallpaper(place) => {
            offered
                .iter()
                .skip(place)
                .take(1)
                .try_for_each(|(_, dark, light)| {
                    let (dark, light) = (dark.to_string_lossy(), light.to_string_lossy());
                    let pairs = [("dark", &*dark), ("light", &*light)];
                    let key: &[&str] = &["wallpaper", "picture"];
                    edit::change(config, &[(key, Value::Pairs(&pairs))])
                })
        }
    }
}

/// What saves a choice among `offered` to the `config.yaml` of `config`,
/// and says in `words` why it could not.
pub fn saving(config: PathBuf, offered: Vec<(Design, PathBuf, PathBuf)>, words: Words) -> Save {
    Box::new(move |choice| {
        let saved = save(&config, choice, &offered);
        saved.map_err(|why| words.say("refused", &[("why", Given::Words(&why.0))]))
    })
}

/// How `config.yaml` writes `scheme`.
const fn word(scheme: SchemeChoice) -> &'static str {
    match scheme {
        SchemeChoice::Dark => "dark",
        SchemeChoice::Light => "light",
        SchemeChoice::Auto => "auto",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use omorfos_config::Config;

    use super::*;

    fn english() -> Words {
        Words::in_languages(&omorfos_i18n::WELCOME, &[])
    }

    fn folder() -> tempfile::TempDir {
        tempfile::TempDir::new().unwrap()
    }

    #[test]
    fn the_keys_that_matter_are_said_as_they_are_bound_and_one_not_bound_is_left_out() {
        let rows = rows(&Keys::default(), true, &english());
        let keys: Vec<&str> = rows.iter().map(|(keys, _)| keys.as_str()).collect();
        assert_eq!(
            keys,
            [
                "Super+Return",
                "Super+space",
                "Super+Escape",
                "Super+Tab",
                "Super+c, x, v, z"
            ]
        );
        assert_eq!(rows[0].1, "Open a terminal");

        let there = folder();
        let bound = |unbound: &str| {
            let written = format!(
                "bindings:\n  - {{ keys: Super+t, action: {{ spawn: [foot] }} }}\nunbind: [Super+Return, {unbound}]\n"
            );
            fs::write(there.path().join("keys.yaml"), written).unwrap();
            let (config, problems) = Config::load(there.path(), &Config::default());
            assert_eq!(problems, []);
            let rows = super::rows(&config.keys, true, &english());
            rows.into_iter().map(|(keys, _)| keys).collect::<Vec<_>>()
        };
        assert_eq!(
            bound("Super+Tab"),
            [
                "Super+t",
                "Super+space",
                "Super+Escape",
                "Alt+Tab",
                "Super+c, x, v, z"
            ]
        );
        assert_eq!(
            bound("Super+Tab, Alt+Tab"),
            ["Super+t", "Super+space", "Super+Escape", "Super+c, x, v, z"]
        );
        // Where Super is not Ctrl, Super+C does not copy, and is not said to.
        let without = super::rows(&Keys::default(), false, &english());
        let keys: Vec<&str> = without.iter().map(|(keys, _)| keys.as_str()).collect();
        assert_eq!(
            keys,
            ["Super+Return", "Super+space", "Super+Escape", "Super+Tab"]
        );
    }

    /// A place of data with `designs`' pictures, dark and light, in it.
    fn installed(designs: &[&str]) -> tempfile::TempDir {
        let place = folder();
        let there = place.path().join("backgrounds/omorfos");
        fs::create_dir_all(&there).unwrap();
        for design in designs {
            for scheme in ["dark", "light"] {
                fs::write(there.join(format!("{design}-{scheme}.png")), "").unwrap();
            }
        }
        place
    }

    #[test]
    fn the_wallpapers_offered_are_those_of_the_first_place_that_has_any_in_order() {
        let empty = folder();
        let half = installed(&[]);
        fs::write(half.path().join("backgrounds/omorfos/spill-dark.png"), "").unwrap();
        let some = installed(&["ground", "prism", "horizon"]);
        let all = installed(&["prism", "spill", "horizon", "facets", "ground"]);
        let places = [empty.path(), half.path(), some.path(), all.path()].map(Path::to_owned);
        let offered = offered(&places);
        let named: Vec<&str> = offered.iter().map(|(design, ..)| design.name()).collect();
        assert_eq!(named, ["prism", "horizon", "ground"]);
        let there = some.path().join("backgrounds/omorfos");
        assert_eq!(offered[1].1, there.join("horizon-dark.png"));
        assert_eq!(offered[1].2, there.join("horizon-light.png"));
        assert_eq!(super::offered(&[empty.path().to_owned()]), []);
    }

    #[test]
    fn the_wallpaper_in_use_is_prism_until_one_is_named_and_then_the_one_named() {
        let all = installed(&["prism", "spill", "horizon"]);
        let offered = offered(&[all.path().to_owned()]);
        assert_eq!(in_use(None, &offered, None), Some(0));
        let (_, dark, light) = &offered[2];
        let named = Picture::paired(dark.clone(), light.clone());
        assert_eq!(in_use(Some(&named), &offered, None), Some(2));
        let own = Picture::paired(dark.clone(), PathBuf::from("/elsewhere.png"));
        assert_eq!(in_use(Some(&own), &offered, None), None);
        let without_prism = &offered[1..];
        assert_eq!(in_use(None, without_prism, None), None);
    }

    fn config_yaml(there: &Path) -> String {
        fs::read_to_string(there.join("config.yaml")).unwrap()
    }

    #[test]
    fn a_choice_is_saved_to_config_yaml_as_a_person_would_write_it() {
        let all = installed(&["prism", "spill"]);
        let offered = offered(&[all.path().to_owned()]);
        let config = folder();
        fs::write(
            config.path().join("config.yaml"),
            "# mine\nappearance:\n  accent: teal\n",
        )
        .unwrap();
        for (scheme, word) in SCHEMES.iter().zip(["dark", "light", "auto"]) {
            save(config.path(), Choice::Scheme(*scheme), &offered).unwrap();
            assert_eq!(
                config_yaml(config.path()),
                format!("# mine\nappearance:\n  accent: teal\n  scheme: {word}\n")
            );
        }
        save(config.path(), Choice::Wallpaper(1), &offered).unwrap();
        let there = all.path().join("backgrounds/omorfos");
        assert!(config_yaml(config.path()).ends_with(&format!(
            "wallpaper:\n  picture: {{ dark: {}, light: {} }}\n",
            there.join("spill-dark.png").display(),
            there.join("spill-light.png").display()
        )));
        let (loaded, problems) = Config::load(config.path(), &Config::default());
        assert_eq!(problems, []);
        assert_eq!(
            in_use(loaded.general.wallpaper.picture.as_ref(), &offered, None),
            Some(1)
        );
        // One not offered is nothing to save.
        let before = config_yaml(config.path());
        save(config.path(), Choice::Wallpaper(7), &offered).unwrap();
        assert_eq!(config_yaml(config.path()), before);

        fs::write(
            config.path().join("config.yaml"),
            "appearance: { accent: teal }\n",
        )
        .unwrap();
        assert!(save(config.path(), Choice::Scheme(SchemeChoice::Dark), &offered).is_err());
    }

    #[test]
    fn why_a_choice_was_not_saved_is_said_in_the_users_language() {
        let config = folder();
        fs::write(
            config.path().join("config.yaml"),
            "appearance: { accent: teal }\n",
        )
        .unwrap();
        let polish = Words::in_languages(&omorfos_i18n::WELCOME, &["pl".to_owned()]);
        let mut save = saving(config.path().to_owned(), Vec::new(), polish);
        let said = save(Choice::Scheme(SchemeChoice::Light)).unwrap_err();
        assert!(
            said.starts_with("config.yaml pozostał bez zmian: `appearance`"),
            "{said}"
        );
        let mut save = saving(config.path().to_owned(), Vec::new(), english());
        fs::write(config.path().join("config.yaml"), "").unwrap();
        assert_eq!(save(Choice::Scheme(SchemeChoice::Light)), Ok(()));
    }

    #[test]
    fn closing_the_card_marks_it_so_and_marked_it_is_not_shown_at_a_login() {
        let state = folder();
        let marked = state.path().join("omorfos").join(WELCOMED);
        mark(&marked);
        assert!(marked.exists());
        let runtime = folder();
        let (runtime, home) = (
            runtime.path().as_os_str().to_owned(),
            state.path().as_os_str().to_owned(),
        );
        let variable = move |name: &str| match name {
            "XDG_RUNTIME_DIR" => Some(runtime.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
            "HOME" | "XDG_STATE_HOME" => Some(home.clone()),
            _ => None,
        };
        assert!(
            start(&variable, true).is_ok(),
            "shown at a login once closed"
        );
        let said = start(&variable, false).unwrap_err().to_string();
        assert!(
            !said.is_empty(),
            "asked for, it is shown, here where there is no session"
        );
    }

    #[test]
    fn the_schemes_are_shown_as_the_desktop_in_their_colours_with_the_day_split_from_corner_to_corner()
     {
        assert_eq!(schemes(Accent::default()).len(), 3);
        let pictures = scheme_pixels(Accent::default());
        let (dark, light) = (
            Theme::built_in(Colours::Dark, Accent::default()),
            Theme::built_in(Colours::Light, Accent::default()),
        );
        let ground = |theme: &Theme| {
            let colour = theme.style(Part::Desktop, States::NONE).ground();
            [colour.red, colour.green, colour.blue, 255]
        };
        let pixel = |pixels: &[u8], (x, y): (u32, u32)| {
            let at = usize::try_from((y * DESKTOP.0 + x) * 4).unwrap();
            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
        };
        // A corner of the background, clear of the window and the bar.
        let (top_left, bottom_right) = ((1, 1), (DESKTOP.0 - 2, DESKTOP.1 * 80 / 100));
        assert_eq!(pixel(&pictures[0], top_left), ground(&dark));
        assert_eq!(pixel(&pictures[1], top_left), ground(&light));
        assert_eq!(pixel(&pictures[2], top_left), ground(&light), "light above");
        assert_eq!(
            pixel(&pictures[2], bottom_right),
            ground(&dark),
            "dark below"
        );
        assert_ne!(ground(&dark), ground(&light));
    }

    #[test]
    fn the_applications_are_those_the_system_sets_named_in_the_users_language() {
        let config = folder();
        let data = folder();
        let applications = data.path().join("applications");
        fs::create_dir_all(&applications).unwrap();
        fs::write(
            config.path().join("mimeapps.list"),
            "[Default Applications]
x-scheme-handler/https=browser.desktop
inode/directory=missing.desktop
",
        )
        .unwrap();
        fs::write(
            applications.join("browser.desktop"),
            "[Desktop Entry]
Name=Browser
Name[pl]=Przeglądarka
Icon=browser
",
        )
        .unwrap();
        let polish = Words::in_languages(&omorfos_i18n::WELCOME, &["pl".to_owned()]);
        let places = ([config.path().to_owned()], [data.path().to_owned()]);
        let mut icons = omorfos_icons::Icons::new(Vec::new());
        let openers = openers(
            (&places.0, &places.1),
            &["pl".to_owned()],
            &polish,
            &mut icons,
        );
        assert_eq!(openers.len(), defaults::KINDS.len());
        assert_eq!(openers[0].kind, "Internet");
        assert_eq!(openers[0].name.as_deref(), Some("Przeglądarka"));
        assert_eq!(openers[0].icon, None, "no icon of that name installed");
        assert_eq!(openers[1].name, None, "an entry that is not there");
        assert_eq!(openers[2].name, None, "none set");
        assert!(the_mark().is_some());
    }

    #[test]
    fn outside_a_session_the_card_says_so() {
        let said = start(&|_| None, false).unwrap_err().to_string();
        assert!(said.contains("not an omorfos session"), "{said}");
    }

    #[test]
    fn what_the_card_says_is_said_in_the_users_language() {
        let polish = Words::in_languages(&omorfos_i18n::WELCOME, &["pl".to_owned()]);
        let said = said(&polish);
        assert_eq!(said.finish, "Gotowe");
        assert_eq!(said.schemes[2], "Wraz z porą dnia");
        assert_eq!(super::said(&english()).title, "Welcome to omorfos");
    }
}
