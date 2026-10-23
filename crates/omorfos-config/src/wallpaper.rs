//! What is shown behind the windows: `wallpaper` in `config.yaml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Deserializer, de};

use crate::{Bounded, Color};

/// The wallpaper that comes with omorfos: a picture or a colour on each
/// screen, behind everything else.
///
/// Without a picture or a colour the theme's `desktop` part is all there
/// is behind the windows.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Wallpaper {
    /// Whether it is started with the session. Turn it off to use another
    /// program for it.
    pub enabled: bool,
    /// A picture, PNG or JPEG, or a directory of them to be shown in turn:
    /// a path from the root, or from the home directory as `~/`.
    pub picture: Option<Picture>,
    /// How the picture is laid on the screen.
    pub fit: Fit,
    /// The colour of the screen where the picture does not cover it, or of
    /// the whole screen without one. Without this, the background of the
    /// theme's `desktop` part.
    #[serde(deserialize_with = "present")]
    #[cfg_attr(feature = "schema", schemars(with = "Color"))]
    pub color: Option<Color>,
    /// How many seconds each picture of a directory is shown for.
    pub every: Bounded<5, 604_800>,
    /// The order the pictures of a directory are shown in.
    pub order: Order,
    /// Particular screens, by name, that show something else.
    pub screens: Screens,
}

impl Default for Wallpaper {
    fn default() -> Self {
        Self {
            enabled: true,
            picture: None,
            fit: Fit::default(),
            color: None,
            every: Bounded::clamped(600),
            order: Order::default(),
            screens: Screens::default(),
        }
    }
}

impl Wallpaper {
    /// What the screen called `name` shows: its own picture or colour, if
    /// it is listed with either, and otherwise those of every screen; and
    /// its own fit, if it is listed with one.
    #[must_use]
    pub fn on(&self, name: &str) -> Shown<'_> {
        let listed = self.screens.0.iter().find(|screen| screen.name == name);
        let own = listed.filter(|screen| screen.picture.is_some() || screen.color.is_some());
        let fit = listed.and_then(|screen| screen.fit).unwrap_or(self.fit);
        own.map_or(
            Shown {
                picture: self.picture.as_ref(),
                fit,
                color: self.color,
            },
            |screen| Shown {
                picture: screen.picture.as_ref(),
                fit,
                color: screen.color,
            },
        )
    }
}

/// What one screen shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown<'a> {
    /// The picture, or the directory of them.
    pub picture: Option<&'a Picture>,
    /// How it is laid on the screen.
    pub fit: Fit,
    /// The colour where it does not cover the screen.
    pub color: Option<Color>,
}

/// Where a picture is, or a directory of them: from the root, or from the
/// home directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(try_from = "String")]
pub struct Picture(PathBuf);

impl Picture {
    /// Where it is, given the home directory: nothing if it is in the home
    /// directory and there is none.
    #[must_use]
    pub fn path(&self, home: Option<&Path>) -> Option<PathBuf> {
        match self.0.strip_prefix("~") {
            Ok(within) => home.map(|home| home.join(within)),
            Err(_) => Some(self.0.clone()),
        }
    }
}

impl TryFrom<String> for Picture {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let path = PathBuf::from(&text);
        // A path from wherever the program happens to have been started is
        // no path at all.
        if path.is_absolute() || text.starts_with("~/") {
            Ok(Self(path))
        } else {
            Err(format!(
                "`{text}` is not a path from the root or from the home directory: write it as \
                 `/…` or `~/…`"
            ))
        }
    }
}

/// How a picture is laid on a screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// As large as it must be to cover the screen, its shape kept, and
    /// what does not fit cut off on two sides.
    #[default]
    Fill,
    /// As large as fits on the screen, its shape kept, with the colour
    /// round it.
    Fit,
    /// Made the screen's size and shape.
    Stretch,
    /// As it is, a pixel of it to a pixel of the screen, in the middle.
    Center,
    /// As it is, again and again from the top left corner.
    Tile,
}

/// The order the pictures of a directory are shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Order {
    /// By their names.
    #[default]
    Name,
    /// Any order, a different one each time through.
    Shuffled,
}

/// The screens that show something other than every other screen. No name
/// is listed twice.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "Vec<ScreenWallpaper>")]
pub struct Screens(Vec<ScreenWallpaper>);

impl Screens {
    /// The screens listed.
    #[must_use]
    pub fn listed(&self) -> &[ScreenWallpaper] {
        &self.0
    }
}

impl TryFrom<Vec<ScreenWallpaper>> for Screens {
    type Error = String;

    fn try_from(screens: Vec<ScreenWallpaper>) -> Result<Self, Self::Error> {
        let repeated = screens.iter().enumerate().find(|(index, screen)| {
            let earlier = &screens[..*index];
            earlier.iter().any(|other| other.name == screen.name)
        });
        if screens.iter().any(|screen| screen.name.is_empty()) {
            Err("a screen's name must not be empty".to_owned())
        } else if let Some((_, screen)) = repeated {
            Err(format!("screen `{}` is listed more than once", screen.name))
        } else {
            Ok(Self(screens))
        }
    }
}

/// What one screen shows, by the name of the socket it is plugged into.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ScreenWallpaper {
    /// The screen's name, such as `eDP-1` or `HDMI-A-1`, as `omorfosctl
    /// state` shows it.
    pub name: String,
    /// Its own picture, or directory of them. With this or `color`, it
    /// shows nothing of what every screen does.
    #[serde(default)]
    pub picture: Option<Picture>,
    /// Its own way of laying the picture on it.
    #[serde(default)]
    pub fit: Option<Fit>,
    /// Its own colour.
    #[serde(default, deserialize_with = "present")]
    #[cfg_attr(feature = "schema", schemars(with = "Color"))]
    pub color: Option<Color>,
}

/// A colour that is there. One given as nothing is all but certainly one
/// written as `#203040` without quotes, which YAML takes for a comment,
/// and is said to be so rather than left to do nothing.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Color>, D::Error> {
    let text = Option::<String>::deserialize(deserializer)?;
    let text = text.ok_or_else(|| {
        de::Error::custom(
            "a colour is needed here: write it in quotes, as '#203040', since YAML takes a bare \
             `#` and what follows for a comment",
        )
    })?;
    Color::try_from(text).map(Some).map_err(de::Error::custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Wallpaper, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn without_configuration_it_runs_and_shows_the_themes_desktop() {
        let wallpaper = Wallpaper::default();
        assert!(wallpaper.enabled);
        let shown = wallpaper.on("eDP-1");
        assert_eq!(shown.picture, None);
        assert_eq!(shown.color, None);
        assert_eq!(shown.fit, Fit::Fill);
        assert_eq!(wallpaper.every.get(), 600);
        assert_eq!(wallpaper.order, Order::Name);
        assert_eq!(parse("{}"), Ok(Wallpaper::default()));
    }

    #[test]
    fn a_picture_is_from_the_root_or_from_home() {
        let home = Some(Path::new("/home/someone"));
        let wallpaper = parse("picture: ~/Pictures/hills.jpg").unwrap();
        let picture = wallpaper.picture.unwrap();
        assert_eq!(
            picture.path(home),
            Some(PathBuf::from("/home/someone/Pictures/hills.jpg"))
        );
        assert_eq!(picture.path(None), None, "no home to be in");
        let wallpaper = parse("picture: /usr/share/backgrounds").unwrap();
        assert_eq!(
            wallpaper.picture.unwrap().path(None),
            Some(PathBuf::from("/usr/share/backgrounds"))
        );
        for relative in ["Pictures/hills.jpg", "~someone/hills.jpg", "''"] {
            let error = parse(&format!("picture: {relative}")).unwrap_err();
            assert!(error.contains("`/…` or `~/…`"), "{relative}: {error}");
        }
    }

    #[test]
    fn every_way_of_laying_a_picture_down_is_named() {
        let named = [
            ("fill", Fit::Fill),
            ("fit", Fit::Fit),
            ("stretch", Fit::Stretch),
            ("center", Fit::Center),
            ("tile", Fit::Tile),
        ];
        for (name, fit) in named {
            assert_eq!(parse(&format!("fit: {name}")).unwrap().fit, fit);
        }
        assert!(parse("fit: zoom").is_err());
        let shuffled = parse("order: shuffled").unwrap();
        assert_eq!(shuffled.order, Order::Shuffled);
        assert!(parse("order: random").is_err());
    }

    #[test]
    fn pictures_change_no_more_often_than_every_five_seconds() {
        assert_eq!(parse("every: 5").unwrap().every.get(), 5);
        assert!(parse("every: 4").is_err());
        assert!(parse("every: 604801").is_err());
    }

    #[test]
    fn a_screen_listed_with_a_picture_or_a_colour_shows_only_its_own() {
        let yaml = "
picture: ~/hills.jpg
fit: fit
color: '#102030'
screens:
  - { name: HDMI-A-1, picture: /srv/sea.png }
  - { name: DP-1, color: '#ff0000', fit: stretch }
  - { name: DP-2, fit: tile }
";
        let wallpaper = parse(yaml).unwrap();
        assert_eq!(wallpaper.screens.listed().len(), 3);
        let every = wallpaper.on("eDP-1");
        assert_eq!(every.picture, wallpaper.picture.as_ref());
        assert_eq!(
            (every.fit, every.color),
            (Fit::Fit, Some(Color::rgb(16, 32, 48)))
        );
        // Its own picture, and no colour, since it named none.
        let own = wallpaper.on("HDMI-A-1");
        assert_eq!(own.picture, wallpaper.screens.listed()[0].picture.as_ref());
        assert_eq!((own.fit, own.color), (Fit::Fit, None));
        // Its own colour, and no picture.
        let red = wallpaper.on("DP-1");
        assert_eq!(red.picture, None);
        assert_eq!(
            (red.fit, red.color),
            (Fit::Stretch, Some(Color::rgb(255, 0, 0)))
        );
        // Only its own fit, and the rest as every screen has it.
        let tiled = wallpaper.on("DP-2");
        assert_eq!(tiled.picture, wallpaper.picture.as_ref());
        assert_eq!((tiled.fit, tiled.color), (Fit::Tile, every.color));
    }

    #[test]
    fn a_screen_is_named_once_and_by_a_name() {
        let twice = "screens: [{ name: DP-1, fit: fit }, { name: DP-1, fit: tile }]";
        assert!(
            parse(twice)
                .unwrap_err()
                .contains("`DP-1` is listed more than once")
        );
        let nameless = "screens: [{ name: '', fit: fit }]";
        assert!(parse(nameless).unwrap_err().contains("must not be empty"));
        assert!(parse("screens: [{ name: DP-1, size: 3 }]").is_err());
        assert!(parse("color: red").is_err());
    }

    #[test]
    fn a_colour_written_without_quotes_is_said_to_need_them() {
        for yaml in [
            "color: #ff0000",
            "color:",
            "screens: [{ name: DP-1, color: }]",
        ] {
            let error = parse(yaml).unwrap_err();
            assert!(error.contains("write it in quotes"), "{yaml}: {error}");
        }
        assert!(parse("color: [1]").is_err(), "not a colour at all");
    }
}
