//! `config.yaml`: general behaviour.

use serde::Deserialize;

use crate::{Bounded, Command, MAX_WORKSPACES, Started};

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
    /// Mouse behaviour.
    pub mouse: Mouse,
    /// How every screen is driven, unless it is named in `outputs`.
    pub output: Output,
    /// Particular screens, by name: where each stands and how it is
    /// driven.
    pub outputs: Outputs,
    /// Screens as `outputs` lists them, for when just those are plugged
    /// in: the first that names exactly the screens there are is used in
    /// place of `outputs`.
    pub arrangements: Vec<Outputs>,
    /// What happens when the user is away.
    pub idle: Idle,
    /// Programs to start with the session, each as a program and its
    /// arguments, or as that and what it may do. They are started once, in
    /// order, and not again when the configuration is reloaded.
    pub autostart: Vec<Started>,
    /// The panel that comes with omorfos.
    pub panel: Panel,
    /// The pointer.
    pub cursor: Cursor,
    /// The launcher that comes with omorfos.
    pub launcher: Launcher,
    /// The notifications that come with omorfos.
    pub notifications: Notifications,
    /// Pictures of the screen.
    pub screenshots: Screenshots,
    /// The clipboard.
    pub clipboard: Clipboard,
    /// What applications ask the desktop through the portals.
    pub portals: Portals,
    /// Programs written for X11.
    pub xwayland: Xwayland,
    /// The program that locks the session, and its arguments, such as
    /// `[swaylock, -f]`. The `lock` action starts it, and it alone is let
    /// lock. Without one there is nothing to lock with.
    pub locker: Option<Command>,
}

/// Programs written for X11, which run through `Xwayland`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Xwayland {
    /// Whether `Xwayland` is started with the session. Turn it off to do
    /// without programs written for X11: they can read each other's
    /// windows and keys, which programs written for Wayland cannot.
    pub enabled: bool,
    /// How they are drawn on a screen with a scale.
    pub scaling: X11Scaling,
}

impl Default for Xwayland {
    fn default() -> Self {
        Self {
            enabled: true,
            scaling: X11Scaling::default(),
        }
    }
}

/// How programs written for X11 are drawn on a screen with a scale. X11
/// has no notion of one, so something has to give.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum X11Scaling {
    /// They draw at one pixel to a unit of layout and are stretched to
    /// size. All of them are the right size, and blurred.
    #[default]
    Stretched,
    /// They draw with as many pixels as the screen has, and are told how
    /// dense it is. Those that take that in are sharp and the right size;
    /// those that do not are sharp and small.
    Native,
}

/// The clipboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Clipboard {
    /// Whether what is copied is kept by `omorfos-clipboard` once the
    /// program it was copied from has closed. Turn it off to have another
    /// program keep it, or none. What a password manager copies is never
    /// kept.
    pub enabled: bool,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// What applications ask the desktop through the portals
/// (`xdg-desktop-portal`), such as whether it is light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Portals {
    /// Whether omorfos answers them. Turn it off to have another
    /// program answer, or none.
    pub enabled: bool,
}

impl Default for Portals {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Pictures of the screen, as `omorfos-screenshot` takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Screenshots {
    /// Where a picture goes when the program is not told otherwise.
    pub destination: Kept,
}

/// Where a picture of the screen goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Kept {
    /// Saved as a file among the user's pictures.
    #[default]
    File,
    /// Copied, to be pasted, and not saved.
    Clipboard,
    /// Saved and copied.
    Both,
}

impl Kept {
    /// Whether a picture is saved as a file.
    #[must_use]
    pub const fn saves(self) -> bool {
        matches!(self, Self::File | Self::Both)
    }

    /// Whether a picture is copied to the clipboard.
    #[must_use]
    pub const fn copies(self) -> bool {
        matches!(self, Self::Clipboard | Self::Both)
    }

    /// The place of this name, as it is written in the configuration.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        let all = [
            ("file", Self::File),
            ("clipboard", Self::Clipboard),
            ("both", Self::Both),
        ];
        all.into_iter()
            .find(|(known, _)| *known == name)
            .map(|(_, kept)| kept)
    }
}

/// The notifications that come with omorfos: what programs have to tell
/// the user, shown in a corner of the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    /// Whether they are shown by omorfos. Turn it off to go without, or
    /// to use another program for them.
    pub enabled: bool,
    /// How long one stays, in seconds, when the program that sent it does
    /// not say. `0` leaves it until it is clicked.
    pub timeout: Bounded<0, 3600>,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout: Bounded::clamped(5),
        }
    }
}

/// The launcher that comes with omorfos: what it shows of each program
/// besides its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Launcher {
    /// Whether each program's icon is shown beside its name.
    pub icons: bool,
    /// Whether what each program says of itself is shown after its name,
    /// and looked through for what is typed.
    pub descriptions: bool,
}

/// The pictures used for the pointer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Cursor {
    /// The name of an installed cursor theme, such as `Adwaita`. `default`
    /// is whichever the system has set up as its default.
    pub theme: String,
    /// The size of the pointer, before scaling.
    pub size: Bounded<8, 128>,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            theme: "default".to_owned(),
            size: Bounded::clamped(24),
        }
    }
}

/// The panel that comes with omorfos: a bar along the bottom of each
/// screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Panel {
    /// Whether it is started with the session. Turn it off to go without
    /// a panel or to use another one.
    pub enabled: bool,
}

impl Default for Panel {
    fn default() -> Self {
        Self { enabled: true }
    }
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
        // More are made as they are wanted, and go when they are not.
        Self(vec!["1".to_owned()])
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

/// What happens when the user is away.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Idle {
    /// Seconds without input after which the screens are turned off. `0`
    /// never turns them off.
    pub screen_off: u32,
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
    /// How fast the pointer moves under a finger.
    pub speed: Speed,
    /// Whether a quick movement takes the pointer further than a slow one
    /// of the same length.
    pub acceleration: Acceleration,
}

impl Default for Touchpad {
    fn default() -> Self {
        Self {
            tap_to_click: true,
            // Fingers on a touchpad move what is under them, as they do on
            // a telephone.
            natural_scroll: true,
            speed: Speed::default(),
            acceleration: Acceleration::default(),
        }
    }
}

/// How fast a device moves the pointer: from -1, the slowest, to 1, the
/// fastest, with 0 what the device is made for. Kept in hundredths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "f64")]
pub struct Speed(i8);

impl Speed {
    /// The speed as a number from -1 to 1.
    #[must_use]
    pub fn value(self) -> f64 {
        f64::from(self.0) / 100.0
    }
}

impl TryFrom<f64> for Speed {
    type Error = String;

    fn try_from(speed: f64) -> Result<Self, Self::Error> {
        Some(speed * 100.0)
            .filter(|hundredths| (-100.0..=100.0).contains(hundredths))
            .and_then(|hundredths| format!("{hundredths:.0}").parse().ok())
            .map(Self)
            .ok_or_else(|| format!("invalid speed `{speed}`: expected a number from -1 to 1"))
    }
}

/// How the speed of a movement changes how far the pointer goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum Acceleration {
    /// A quick movement takes the pointer further than a slow one of the
    /// same length: slow for precision, quick to cross the screen.
    #[default]
    Adaptive,
    /// The pointer goes as far as the device moved, however quickly.
    Flat,
}

/// Mouse behaviour. A mouse and a touchpad are set apart because people
/// expect opposite things of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Mouse {
    /// Whether content follows the wheel when scrolling. A wheel turned
    /// towards the user moves down the page unless this is on.
    pub natural_scroll: bool,
    /// How fast the pointer moves with the mouse.
    pub speed: Speed,
    /// Whether a quick movement takes the pointer further than a slow one
    /// of the same length.
    pub acceleration: Acceleration,
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

    /// Whether these are the screens called `names`, and no others, in
    /// whatever order.
    pub fn are<'a>(&self, names: impl Iterator<Item = &'a str> + Clone) -> bool {
        let mut listed = self.0.iter();
        listed.all(|listed| names.clone().any(|name| name == listed.name))
            && self.0.len() == names.count()
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
        let primaries = outputs.iter().filter(|output| output.primary).count();
        if outputs.iter().any(|output| output.name.is_empty()) {
            Err("an output's name must not be empty".to_owned())
        } else if primaries > 1 {
            Err("only one output may be `primary`".to_owned())
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
    /// The size it is driven at, and how often it is drawn. Without this,
    /// what the screen says suits it best.
    #[serde(default)]
    pub mode: Option<ScreenMode>,
    /// Where it stands next to the screen listed before it: to its right,
    /// unless this says otherwise.
    #[serde(default)]
    pub place: Option<Place>,
    /// Where along the edge it shares with the one before it it is lined
    /// up: at the start, the top or the left; the middle; or the end.
    /// Without this, the start beside it, and the middle above or below.
    #[serde(default)]
    pub align: Option<Align>,
    /// How far along that edge it is moved from there: right or down, or
    /// left or up if less than nothing.
    #[serde(default)]
    pub offset: Option<Offset>,
    /// Which way the screen is turned, as `xrandr --rotate` says it: what
    /// is drawn is turned to match, so that it stands the right way up.
    #[serde(default)]
    pub rotate: Rotation,
    /// Whether it is the one that comes first: the one the notice of what
    /// is wrong is shown on, and that windows go to from a screen that is
    /// unplugged. Without one, the first screen plugged in. One screen at
    /// most.
    #[serde(default)]
    pub primary: bool,
}

/// Which way a screen is turned, with the top of what it shows: where it
/// is as made, on the left, on the right, or at the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Rotation {
    /// As it was made.
    #[default]
    Normal,
    /// A quarter turn to the left: the top of the picture on the left.
    Left,
    /// A quarter turn to the right: the top of the picture on the right.
    Right,
    /// Upside down.
    Inverted,
}

/// Where along the edge two screens share one is lined up with the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Align {
    /// Their tops, or their left sides, in line.
    Start,
    /// Their middles in line.
    Middle,
    /// Their bottoms, or their right sides, in line.
    End,
}

/// How far a screen is moved along the edge it shares with another: in
/// units of layout, such as `-300`, or as a share of its own length along
/// that edge, such as `-30%`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "OffsetWritten")]
pub enum Offset {
    /// So many units of layout.
    Units(i32),
    /// So many hundredths of the screen's own length.
    Share(i32),
}

/// An offset as written: a number, or text such as `30%`.
#[derive(Deserialize)]
#[serde(untagged)]
enum OffsetWritten {
    Number(i32),
    Text(String),
}

impl TryFrom<OffsetWritten> for Offset {
    type Error = String;

    fn try_from(written: OffsetWritten) -> Result<Self, Self::Error> {
        match written {
            OffsetWritten::Number(units) => Ok(Self::Units(units)),
            OffsetWritten::Text(text) => {
                let share = text.strip_suffix('%').and_then(|share| share.parse().ok());
                share.filter(|share: &i32| share.abs() <= 100).map(Self::Share).ok_or_else(|| {
                    format!(
                        "invalid offset `{text}`: expected units of layout, such as `-300`, or a \
                         share of the screen from -100% to 100%, such as `-30%`"
                    )
                })
            }
        }
    }
}

impl Offset {
    /// How many units it moves a screen `length` units long along the edge.
    #[must_use]
    pub fn along(self, length: i32) -> i32 {
        match self {
            Self::Units(units) => units,
            Self::Share(share) => length * share / 100,
        }
    }
}

/// Where a screen stands next to the one listed before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "lowercase")]
pub enum Place {
    /// To its right, the tops in line.
    #[default]
    Right,
    /// To its left, the tops in line.
    Left,
    /// Above it, the middles in line.
    Above,
    /// Below it, the middles in line.
    Below,
}

/// The size a screen is driven at, in pixels, and how many times a second
/// it is drawn if that is said: `2560x1440`, or `2560x1440@144`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct ScreenMode {
    /// How many pixels across.
    pub width: u16,
    /// How many pixels down.
    pub height: u16,
    /// How many times a second, if that is said.
    pub refresh: Option<u32>,
}

impl TryFrom<String> for ScreenMode {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let (size, refresh) = text
            .split_once('@')
            .map_or((text.as_str(), None), |(size, refresh)| {
                (size, Some(refresh))
            });
        let size = size.split_once('x');
        let size = size.and_then(|(width, height)| width.parse().ok().zip(height.parse().ok()));
        let size = size.filter(|&(width, height): &(u16, u16)| width > 0 && height > 0);
        let refresh = refresh.map(|refresh| refresh.parse().ok().filter(|refresh| *refresh > 0));
        let mode = size
            .zip(refresh.unwrap_or(Some(0)))
            .map(|((width, height), refresh)| Self {
                width,
                height,
                refresh: (refresh > 0).then_some(refresh),
            });
        mode.ok_or_else(|| {
            format!(
                "invalid mode `{text}`: expected the size in pixels, such as `2560x1440`, and \
                 how many times a second it is drawn if that matters, such as `2560x1440@144`"
            )
        })
    }
}

impl std::fmt::Display for ScreenMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (width, height) = (self.width, self.height);
        match self.refresh {
            Some(refresh) => write!(f, "{width}x{height}@{refresh}"),
            None => write!(f, "{width}x{height}"),
        }
    }
}

/// One way a screen can be driven, as it says: its size in pixels, how
/// many times a second it is drawn, and whether it says it suits it best.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferedMode {
    /// How many pixels across.
    pub width: u16,
    /// How many pixels down.
    pub height: u16,
    /// How many times a second.
    pub refresh: u32,
    /// Whether the screen says this suits it best.
    pub preferred: bool,
}

impl std::fmt::Display for OfferedMode {
    /// As the configuration writes a mode: `2560x1440@144`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}x{}@{}", self.width, self.height, self.refresh)
    }
}

impl ScreenMode {
    /// Which of the ways a screen offers it is driven in, if it offers any,
    /// by its place among them, for the mode `wanted` if one is: that size
    /// at that rate, or at the fastest if no rate is said. Otherwise the
    /// one the screen says suits it best, or the first; and then also what
    /// the user is to be told of why it is not what they asked.
    #[must_use]
    pub fn choose(
        wanted: Option<&Self>,
        offered: &[OfferedMode],
    ) -> Option<(usize, Option<String>)> {
        let best = offered.iter().position(|mode| mode.preferred);
        let usual = best.or_else(|| (!offered.is_empty()).then_some(0));
        let Some(wanted) = wanted else {
            return usual.map(|at| (at, None));
        };
        let sized =
            |mode: &&OfferedMode| (mode.width, mode.height) == (wanted.width, wanted.height);
        let fastest = offered
            .iter()
            .enumerate()
            .filter(|(_, mode)| sized(mode))
            .max_by_key(|(_, mode)| mode.refresh);
        let exact = wanted.refresh.and_then(|refresh| {
            let mut modes = offered.iter().enumerate();
            modes.find(|(_, mode)| sized(mode) && mode.refresh == refresh)
        });
        let said = OfferedMode::to_string;
        match (exact, fastest) {
            (Some((at, _)), _) => Some((at, None)),
            // At that size, the fastest; which is what was asked unless a
            // rate was said that it is not offered at.
            (None, Some((at, mode))) => {
                let told = wanted.refresh.map(|refresh| {
                    let size = (wanted.width, wanted.height);
                    format!(
                        "cannot be drawn {refresh} times a second at {}x{}, and is at {}",
                        size.0,
                        size.1,
                        said(mode)
                    )
                });
                Some((at, told))
            }
            (None, None) => usual.map(|at| {
                let instead = said(&offered[at]);
                (
                    at,
                    Some(format!("cannot be driven at {wanted}, and is at {instead}")),
                )
            }),
        }
    }
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
    fn the_pointer_is_the_systems_default_unless_a_theme_and_size_are_named() {
        assert_eq!(General::default().cursor.theme, "default");
        assert_eq!(General::default().cursor.size.get(), 24);
        let cursor = parse("cursor: { theme: Adwaita, size: 32 }")
            .unwrap()
            .cursor;
        assert_eq!((cursor.theme.as_str(), cursor.size.get()), ("Adwaita", 32));
        assert!(parse("cursor: { size: 4 }").is_err());
    }

    #[test]
    fn the_launcher_shows_names_alone_unless_more_is_asked_for() {
        let launcher = General::default().launcher;
        assert!(!launcher.icons && !launcher.descriptions);
        let launcher = parse("launcher: { icons: true }").unwrap().launcher;
        assert!(launcher.icons && !launcher.descriptions);
        let launcher = parse("launcher: { descriptions: true }").unwrap().launcher;
        assert!(!launcher.icons && launcher.descriptions);
        assert!(parse("launcher: { fuzzy: true }").is_err());
    }

    #[test]
    fn what_is_copied_is_kept_unless_turned_off() {
        assert!(General::default().clipboard.enabled);
        let general = parse("clipboard: { enabled: false }").unwrap();
        assert!(!general.clipboard.enabled);
        assert!(parse("clipboard: { history: 10 }").is_err());
    }

    #[test]
    fn the_portals_are_answered_unless_turned_off() {
        assert!(General::default().portals.enabled);
        assert!(
            !parse("portals: { enabled: false }")
                .unwrap()
                .portals
                .enabled
        );
        assert!(parse("portals: { on: false }").is_err());
    }

    #[test]
    fn a_picture_of_the_screen_is_saved_unless_told_otherwise() {
        assert_eq!(General::default().screenshots.destination, Kept::File);
        let to = |yaml: &str| parse(yaml).map(|general| general.screenshots.destination);
        assert_eq!(
            to("screenshots: { destination: clipboard }").unwrap(),
            Kept::Clipboard
        );
        assert_eq!(
            to("screenshots: { destination: both }").unwrap(),
            Kept::Both
        );
        assert!(to("screenshots: { destination: printer }").is_err());
        assert!(parse("screenshots: { into: file }").is_err());

        assert!(Kept::File.saves() && !Kept::File.copies());
        assert!(!Kept::Clipboard.saves() && Kept::Clipboard.copies());
        assert!(Kept::Both.saves() && Kept::Both.copies());
        assert_eq!(Kept::named("file"), Some(Kept::File));
        assert_eq!(Kept::named("clipboard"), Some(Kept::Clipboard));
        assert_eq!(Kept::named("both"), Some(Kept::Both));
        assert_eq!(Kept::named("File"), None);
    }

    #[test]
    fn notifications_are_shown_for_five_seconds_unless_told_otherwise() {
        let notifications = General::default().notifications;
        assert!(notifications.enabled);
        assert_eq!(notifications.timeout.get(), 5);
        let yaml = "notifications: { enabled: false, timeout: 0 }";
        let notifications = parse(yaml).unwrap().notifications;
        assert!(!notifications.enabled);
        assert_eq!(notifications.timeout.get(), 0);
        assert!(parse("notifications: { timeout: 99999 }").is_err());
    }

    #[test]
    fn programs_written_for_x11_can_run_unless_that_is_turned_off() {
        assert!(General::default().xwayland.enabled);
        assert!(parse("xwayland: {}").unwrap().xwayland.enabled);
        assert!(
            !parse("xwayland: { enabled: false }")
                .unwrap()
                .xwayland
                .enabled
        );
        assert!(parse("xwayland: { display: 3 }").is_err());
        assert_eq!(
            General::default().xwayland.scaling,
            X11Scaling::Stretched,
            "the right size unless told otherwise"
        );
        let native = parse("xwayland: { scaling: native }").unwrap().xwayland;
        assert_eq!(native.scaling, X11Scaling::Native);
        assert!(native.enabled);
        assert!(parse("xwayland: { scaling: blurry }").is_err());
    }

    #[test]
    fn a_program_started_with_the_session_can_be_told_what_it_may_do() {
        use crate::Role;

        let yaml = "
autostart:
  - [swayidle]
  - { run: [waybar, -c, mine], may: [panel] }
  - { run: [swaylock] , may: [lock, overlay] }
  - { run: [foot] }
";
        let started = parse(yaml).unwrap().autostart;
        assert_eq!(started[0].may, []);
        assert_eq!(started[1].run.arguments(), ["-c", "mine"]);
        assert_eq!(started[1].may, [Role::Panel]);
        assert_eq!(started[2].may, [Role::Lock, Role::Overlay]);
        assert_eq!(started[3].may, [], "nothing unless it is said");
        for wrong in [
            "autostart: [{ run: [waybar], may: [everything] }]",
            "autostart: [{ run: [waybar], might: [panel] }]",
            "autostart: [{ may: [panel] }]",
            "autostart: [{ run: [], may: [panel] }]",
        ] {
            assert!(parse(wrong).is_err(), "{wrong}");
        }
        let all = "autostart: [{ run: [x], may: [ask, capture, clipboard, input, screens] }]";
        assert_eq!(parse(all).unwrap().autostart[0].may.len(), 5);
    }

    #[test]
    fn a_program_can_be_named_to_lock_the_session_with() {
        assert_eq!(General::default().locker, None);
        let locker = parse("locker: [swaylock, -f]").unwrap().locker.unwrap();
        assert_eq!(
            (locker.program(), locker.arguments()),
            ("swaylock", ["-f".to_owned()].as_slice())
        );
        assert!(parse("locker: []").is_err());
    }

    #[test]
    fn the_panel_is_started_unless_it_is_turned_off() {
        assert!(General::default().panel.enabled);
        assert!(parse("panel: {}").unwrap().panel.enabled);
        assert!(!parse("panel: { enabled: false }").unwrap().panel.enabled);
        assert!(parse("panel: { position: top }").is_err());
    }

    #[test]
    fn programs_to_start_with_the_session_are_listed_with_their_arguments() {
        assert_eq!(General::default().autostart, []);
        let yaml = "autostart:\n  - [swayidle, -w]\n  - [waybar]\n";
        let started = parse(yaml).unwrap().autostart;
        assert_eq!(started.len(), 2);
        assert_eq!(started[0].run.program(), "swayidle");
        assert_eq!(started[0].run.arguments(), ["-w"]);
        assert_eq!(started[1].run.program(), "waybar");
        assert_eq!(started[1].may, [], "an ordinary program");
        assert!(
            parse("autostart: [[]]").is_err(),
            "a program has to be named"
        );
    }

    #[test]
    fn the_screens_stay_on_unless_a_time_is_given() {
        assert_eq!(General::default().idle.screen_off, 0);
        let idle = parse("idle: { screen_off: 600 }").unwrap().idle;
        assert_eq!(idle.screen_off, 600);
        assert!(parse("idle: { screen_off: -5 }").is_err());
        assert!(parse("idle: { screen_of: 5 }").is_err());
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

    fn mode(text: &str) -> Result<ScreenMode, String> {
        ScreenMode::try_from(text.to_owned())
    }

    #[test]
    fn a_mode_is_a_size_and_perhaps_how_often_it_is_drawn() {
        let sized = mode("2560x1440").unwrap();
        assert_eq!(
            (sized.width, sized.height, sized.refresh),
            (2560, 1440, None)
        );
        let fast = mode("2560x1440@144").unwrap();
        assert_eq!(fast.refresh, Some(144));
        assert_eq!(
            (sized.to_string(), fast.to_string()),
            ("2560x1440".to_owned(), "2560x1440@144".to_owned())
        );
        for wrong in [
            "",
            "2560",
            "2560x",
            "x1440",
            "0x1440",
            "2560x0",
            "2560x1440@",
            "2560x1440@0",
            "2560x1440@fast",
            "99999x1",
        ] {
            let said = mode(wrong).unwrap_err();
            assert!(said.contains("invalid mode"), "{wrong}: {said}");
        }
        let listed = parse("{}").unwrap();
        assert_eq!(listed.outputs, Outputs::default());
    }

    fn offered(width: u16, height: u16, refresh: u32, preferred: bool) -> OfferedMode {
        OfferedMode {
            width,
            height,
            refresh,
            preferred,
        }
    }

    #[test]
    fn a_screen_is_driven_as_asked_if_it_can_be_and_as_suits_it_best_otherwise() {
        let modes = [
            offered(1920, 1080, 60, false),
            offered(2560, 1440, 60, true),
            offered(2560, 1440, 144, false),
            offered(2560, 1440, 120, false),
        ];
        // Asked nothing, as it says suits it best, or the first.
        assert_eq!(ScreenMode::choose(None, &modes), Some((1, None)));
        let plain = [offered(800, 600, 60, false), offered(640, 480, 60, false)];
        assert_eq!(ScreenMode::choose(None, &plain), Some((0, None)));
        assert_eq!(ScreenMode::choose(None, &[]), None);
        // A size, at its fastest; a size and a rate, at that.
        let sized = mode("2560x1440").unwrap();
        assert_eq!(ScreenMode::choose(Some(&sized), &modes), Some((2, None)));
        let rated = mode("2560x1440@120").unwrap();
        assert_eq!(ScreenMode::choose(Some(&rated), &modes), Some((3, None)));
        // A rate it is not offered at: the fastest at that size, and why.
        let unrated = mode("2560x1440@75").unwrap();
        let (at, told) = ScreenMode::choose(Some(&unrated), &modes).unwrap();
        assert_eq!(at, 2);
        let told = told.unwrap();
        assert!(
            told.contains("75 times a second") && told.contains("2560x1440@144"),
            "{told}"
        );
        // A size it is not offered at: as it suits it best, and why.
        let elsewhere = mode("3840x2160").unwrap();
        let (at, told) = ScreenMode::choose(Some(&elsewhere), &modes).unwrap();
        assert_eq!(at, 1);
        let told = told.unwrap();
        assert!(
            told.contains("3840x2160") && told.contains("2560x1440@60"),
            "{told}"
        );
        assert_eq!(ScreenMode::choose(Some(&elsewhere), &[]), None);
    }

    #[test]
    fn a_listed_screen_may_be_lined_up_and_moved_along_the_edge_it_shares() {
        let general = parse(
            "outputs:\n  - { name: DP-1 }\n  - { name: eDP-1, place: below, align: start, offset: -30% }\n  - { name: HDMI-A-1, align: end, offset: 120 }\n",
        )
        .unwrap();
        let listed = general.outputs.listed();
        assert_eq!(
            (listed[0].place, listed[0].align, listed[0].offset),
            (None, None, None)
        );
        assert_eq!(listed[1].place, Some(Place::Below));
        assert_eq!(listed[1].align, Some(Align::Start));
        assert_eq!(listed[1].offset, Some(Offset::Share(-30)));
        assert_eq!(listed[2].offset, Some(Offset::Units(120)));
        // A share is of the screen's own length; units are units.
        assert_eq!(Offset::Share(-30).along(1000), -300);
        assert_eq!(Offset::Units(120).along(1000), 120);
        for wrong in ["30", "101%", "-101%", "far%", "left"] {
            let yaml = format!("outputs: [{{ name: DP-1, offset: \"{wrong}\" }}]\n");
            let said = parse(&yaml).unwrap_err();
            assert!(said.contains("invalid offset"), "{wrong}: {said}");
        }
        assert!(parse("outputs: [{ name: DP-1, align: centre }]\n").is_err());
    }

    #[test]
    fn an_arrangement_is_for_just_the_screens_it_names() {
        let general = parse(
            "arrangements:\n  - [{ name: eDP-1 }]\n  - [{ name: HDMI-A-1 }, { name: eDP-1, place: below }]\n",
        )
        .unwrap();
        assert_eq!(general.arrangements.len(), 2);
        let (alone, docked) = (&general.arrangements[0], &general.arrangements[1]);
        let names = |names: &'static [&'static str]| names.iter().copied();
        assert!(alone.are(names(&["eDP-1"])));
        assert!(
            !alone.are(names(&["eDP-1", "HDMI-A-1"])),
            "not with another"
        );
        assert!(
            docked.are(names(&["eDP-1", "HDMI-A-1"])),
            "in whatever order"
        );
        assert!(!docked.are(names(&["eDP-1", "DP-1"])));
        assert!(
            !docked.are(names(&["HDMI-A-1"])),
            "nor with one of them gone"
        );
        assert_eq!(parse("{}").unwrap().arrangements, []);
    }

    #[test]
    fn one_listed_screen_may_come_first() {
        let one = parse("outputs:\n  - { name: DP-1 }\n  - { name: eDP-1, primary: true }\n");
        let listed = one.unwrap().outputs;
        let primary = listed.listed().iter().map(|listed| listed.primary);
        assert_eq!(primary.collect::<Vec<_>>(), [false, true]);
        let two = parse(
            "outputs:\n  - { name: DP-1, primary: true }\n  - { name: eDP-1, primary: true }\n",
        );
        let said = two.unwrap_err();
        assert!(said.contains("only one output may be `primary`"), "{said}");
    }

    #[test]
    fn a_listed_screen_may_say_what_it_is_driven_at() {
        let general =
            parse("outputs:\n  - { name: DP-1, mode: 2560x1440@144 }\n  - { name: eDP-1 }\n")
                .unwrap();
        let listed = general.outputs.listed();
        assert_eq!(listed[0].mode, Some(mode("2560x1440@144").unwrap()));
        assert_eq!(listed[1].mode, None);
        let wrong = parse("outputs:\n  - { name: DP-1, mode: big }\n").unwrap_err();
        assert!(wrong.contains("invalid mode `big`"), "{wrong}");
    }

    fn parse(yaml: &str) -> Result<General, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn defaults_are_usable_without_any_configuration() {
        let general = General::default();
        assert_eq!(general.workspaces.names(), ["1"]);
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
                natural_scroll: true,
                speed: Speed(0),
                acceleration: Acceleration::Adaptive,
            }
        );
    }

    #[test]
    fn a_mouse_and_a_touchpad_scroll_each_in_the_way_people_expect_of_it() {
        let general = General::default();
        assert!(general.touchpad.natural_scroll, "as fingers do");
        assert!(!general.mouse.natural_scroll, "as a wheel does");
        let swapped = "touchpad: { natural_scroll: false }\nmouse: { natural_scroll: true }";
        let general = parse(swapped).unwrap();
        assert!(!general.touchpad.natural_scroll);
        assert!(general.mouse.natural_scroll);
        assert!(
            general.touchpad.tap_to_click,
            "what is not said is as before"
        );
        assert!(parse("mouse: { tap_to_click: true }").is_err());
    }

    #[test]
    fn a_mouse_and_a_touchpad_each_have_a_speed_and_an_acceleration() {
        let general = General::default();
        for (speed, acceleration) in [
            (general.touchpad.speed, general.touchpad.acceleration),
            (general.mouse.speed, general.mouse.acceleration),
        ] {
            assert!(
                speed.value().abs() < f64::EPSILON,
                "what the device is made for"
            );
            assert_eq!(acceleration, Acceleration::Adaptive);
        }
        let yaml = "touchpad: { speed: 0.3 }\nmouse: { speed: -1, acceleration: flat }";
        let general = parse(yaml).unwrap();
        assert!((general.touchpad.speed.value() - 0.3).abs() < f64::EPSILON);
        assert_eq!(general.touchpad.acceleration, Acceleration::Adaptive);
        assert!((general.mouse.speed.value() + 1.0).abs() < f64::EPSILON);
        assert_eq!(general.mouse.acceleration, Acceleration::Flat);
        assert_eq!(
            parse("mouse: { speed: 1 }").unwrap().mouse.speed,
            Speed(100)
        );
    }

    #[test]
    fn a_speed_outside_what_a_device_takes_is_refused() {
        for speed in ["1.01", "-2", ".nan"] {
            let error = parse(&format!("mouse: {{ speed: {speed} }}")).unwrap_err();
            assert!(error.contains("speed"), "{error}");
        }
        let error = parse("touchpad: { speed: 3 }").unwrap_err();
        assert!(
            error.contains("invalid speed `3`: expected a number from -1 to 1"),
            "{error}"
        );
        assert!(parse("mouse: { speed: fast }").is_err());
        assert!(parse("mouse: { acceleration: quick }").is_err());
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
touchpad: { tap_to_click: false, natural_scroll: true, speed: -0.5, acceleration: flat }
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
                natural_scroll: true,
                speed: Speed(-50),
                acceleration: Acceleration::Flat,
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
