//! `theme.css`: how the desktop looks.
//!
//! A theme is a stylesheet: rules that give properties to parts of the
//! desktop, perhaps in some state. The user's is laid over a built-in one,
//! so it only has to say what is to be different.
//!
//! Everything is worked out when the theme is loaded. Asking how a part
//! looks afterwards is looking it up.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::css::{self, Declaration, Selector, Sheet};
use crate::style::CLEAR;
use crate::{Bounded, Color, Part, Position, States, Style};

/// The stylesheet every theme starts from.
const BUILT_IN: &str = include_str!("default.css");

/// How many times a variable may stand for another before the theme is
/// taken to be going round in a circle.
const DEPTH: usize = 8;

/// A font: a family and a size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    /// A font family name, or a generic one such as `sans-serif`.
    pub family: String,
    /// Size in points.
    pub size: Bounded<6, 72>,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_owned(),
            size: Bounded::clamped(11),
        }
    }
}

/// How the desktop looks: for every part, in every state, its [`Style`]
/// and its [`Font`].
///
/// Copying a theme is cheap, and so is comparing one with a copy of itself.
#[derive(Debug, Clone)]
pub struct Theme {
    table: Arc<Table>,
}

/// What a theme works out to.
#[derive(Debug, Default, PartialEq, Eq)]
struct Table {
    /// A look for each part in each combination of states, part by part.
    looks: Vec<Look>,
    /// The font of a part that is not in the table, which there is none of.
    spare: Font,
}

/// How one part looks in one combination of states.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Look {
    style: Style,
    font: Font,
}

/// Something wrong with a theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError {
    /// Where in the stylesheet it is, when that can be said.
    pub position: Option<Position>,
    /// What is wrong, written for the user.
    pub message: String,
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let place = self.position;
        let place = place.map(|at| format!("{}:{}: ", at.line, at.column));
        write!(f, "{}{}", place.unwrap_or_default(), self.message)
    }
}

impl std::error::Error for ThemeError {}

impl Default for Theme {
    /// The built-in theme.
    fn default() -> Self {
        let table = build(&built_in(), &Sheet::default());
        Self {
            table: Arc::new(table.unwrap_or_default()),
        }
    }
}

impl PartialEq for Theme {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.table, &other.table) || self.table == other.table
    }
}

impl Eq for Theme {}

impl Theme {
    /// The theme that `stylesheet` makes when laid over the built-in one.
    /// An empty stylesheet makes the built-in theme.
    ///
    /// # Errors
    ///
    /// Returns what is wrong with the stylesheet, and where. Nothing in a
    /// stylesheet is passed over: a part, state, property or value that is
    /// not understood is an error.
    pub fn parse(stylesheet: &str) -> Result<Self, ThemeError> {
        let own = Sheet::parse(stylesheet);
        let own = own.map_err(|fault| (Some(fault.offset), fault.message));
        let table = own.and_then(|own| build(&built_in(), &own));
        let theme = table.map(|table| Self {
            table: Arc::new(table),
        });
        theme.map_err(|(offset, message)| ThemeError {
            position: offset.map(|offset| css::position(stylesheet, offset)),
            message,
        })
    }

    /// How `part` looks while it is in `states`. States a part cannot be
    /// in make no difference to it.
    #[must_use]
    pub fn style(&self, part: Part, states: States) -> Style {
        let look = self.table.looks.get(index(part, states));
        look.map_or(Style::NONE, |look| look.style)
    }

    /// The font `part` is written in while it is in `states`.
    #[must_use]
    pub fn font(&self, part: Part, states: States) -> &Font {
        let look = self.table.looks.get(index(part, states));
        look.map_or(&self.table.spare, |look| &look.font)
    }
}

/// Where in the table a part in some states is.
const fn index(part: Part, states: States) -> usize {
    part as usize * States::COMBINATIONS + states.index()
}

/// The built-in stylesheet, taken apart.
fn built_in() -> Sheet {
    Sheet::parse(BUILT_IN).unwrap_or_default()
}

/// What is wrong, and how far into the user's stylesheet, if it is there
/// that it is.
type Problem = (Option<usize>, String);

/// A variable: what stands for what, and where the user said so, if it was
/// the user.
struct Variable<'a> {
    name: &'a str,
    value: &'a str,
    origin: Option<usize>,
}

/// The variables in effect, by name.
type Variables<'a> = HashMap<&'a str, Variable<'a>>;

/// One property of a part, understood.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Setting {
    Background(Color),
    Color(Color),
    BorderColor(Color),
    BorderWidth(u32),
    BorderRadius(u32),
    UnderlineColor(Color),
    UnderlineWidth(u32),
    Height(u32),
    FontFamily(String),
    FontSize(u32),
}

/// The properties a stylesheet can give, as it is told when it gives
/// another.
const PROPERTIES: &str = "background, color, border-color, border-width, border-radius, \
                          border-bottom-color, border-bottom-width, height, font-family and \
                          font-size";

/// The variables a stylesheet declares.
fn declared(sheet: &Sheet, by_user: bool) -> impl Iterator<Item = Variable<'_>> {
    let variables = sheet.variables.iter();
    variables.map(move |one| Variable {
        name: &one.name,
        value: &one.value,
        origin: by_user.then_some(one.offset),
    })
}

/// A selector, the settings its rule makes, and how it ranks among the
/// others that apply to the same part.
struct Ranked<'a> {
    selector: Selector,
    /// The user's rules outrank the built-in ones; then a rule for a part
    /// outranks one for every part; then the one naming more states wins.
    rank: (bool, bool, u32),
    settings: &'a [Setting],
}

/// Works a theme out from the built-in stylesheet and the user's.
///
/// The work is in proportion to the size of the stylesheets: each variable
/// is found by name, and each rule's settings are understood once however
/// many parts the rule names.
fn build(built_in: &Sheet, own: &Sheet) -> Result<Table, Problem> {
    // The user's variables replace the built-in ones of the same name, and
    // a later one an earlier.
    let declared = declared(built_in, false).chain(declared(own, true));
    let variables: Variables = declared.map(|one| (one.name, one)).collect();

    let mut understood: Vec<(bool, &[Selector], Vec<Setting>)> = Vec::new();
    for (by_user, sheet) in [(false, built_in), (true, own)] {
        for rule in &sheet.rules {
            let settings = rule.declarations.iter();
            let settings: Result<Vec<Setting>, Problem> = settings
                .map(|declaration| setting(declaration, by_user, &variables))
                .collect();
            understood.push((by_user, &rule.selectors, settings?));
        }
    }
    let ranked = understood
        .iter()
        .flat_map(|(by_user, selectors, settings)| {
            selectors.iter().map(move |selector| Ranked {
                selector: *selector,
                rank: (*by_user, selector.part.is_some(), selector.states.count()),
                settings,
            })
        });
    let mut ranked: Vec<Ranked> = ranked.collect();
    // The sort is stable, so among equals the later rule is applied later.
    ranked.sort_by_key(|rule| rule.rank);

    let mut looks = Vec::with_capacity(Part::ALL.len() * States::COMBINATIONS);
    for part in Part::ALL {
        for states in States::every() {
            let mut look = Look {
                style: Style::NONE,
                font: Font::default(),
            };
            let applies = |rule: &&Ranked| {
                let selector = rule.selector;
                selector.part.is_none_or(|named| named == part) && selector.states.within(states)
            };
            for rule in ranked.iter().filter(applies) {
                rule.settings.iter().for_each(|setting| look.set(setting));
            }
            looks.push(look);
        }
    }
    Ok(Table {
        looks,
        spare: Font::default(),
    })
}

impl Look {
    fn set(&mut self, setting: &Setting) {
        let style = &mut self.style;
        match setting {
            Setting::Background(color) => style.background = *color,
            Setting::Color(color) => style.color = *color,
            Setting::BorderColor(color) => style.border.color = *color,
            Setting::BorderWidth(width) => style.border.width = *width,
            Setting::BorderRadius(radius) => style.radius = *radius,
            Setting::UnderlineColor(color) => style.underline.color = *color,
            Setting::UnderlineWidth(width) => style.underline.width = *width,
            Setting::Height(height) => style.height = *height,
            Setting::FontFamily(family) => self.font.family.clone_from(family),
            Setting::FontSize(size) => self.font.size = Bounded::clamped(*size),
        }
    }
}

/// Understands one property and its value.
fn setting(
    declaration: &Declaration,
    by_user: bool,
    variables: &Variables,
) -> Result<Setting, Problem> {
    let written = by_user.then_some(declaration.offset);
    let (value, origin) = resolve(&declaration.value, written, variables)?;
    let understood = match declaration.name.as_str() {
        "background" | "background-color" => color(value).map(Setting::Background),
        "color" => color(value).map(Setting::Color),
        "border-color" => color(value).map(Setting::BorderColor),
        "border-width" => size(value, "px", 0, 16).map(Setting::BorderWidth),
        "border-radius" => size(value, "px", 0, 32).map(Setting::BorderRadius),
        "border-bottom-color" => color(value).map(Setting::UnderlineColor),
        "border-bottom-width" => size(value, "px", 0, 16).map(Setting::UnderlineWidth),
        "height" => size(value, "px", 16, 64).map(Setting::Height),
        "font-family" => family(value).map(Setting::FontFamily),
        "font-size" => size(value, "pt", 6, 72).map(Setting::FontSize),
        other => {
            let message = format!("`{other}` is not a property; the properties are {PROPERTIES}");
            return Err((by_user.then_some(declaration.at), message));
        }
    };
    understood.map_err(|message| (origin, message))
}

/// Follows a value through the variables it stands for, to what is meant
/// and where that was written.
fn resolve<'a>(
    value: &'a str,
    written: Option<usize>,
    variables: &Variables<'a>,
) -> Result<(&'a str, Option<usize>), Problem> {
    let mut meant = (value, written);
    for _ in 0..DEPTH {
        let Some(name) = variable(meant.0) else {
            return Ok(meant);
        };
        let Some(found) = variables.get(name) else {
            return Err((meant.1, format!("there is no variable `{name}`")));
        };
        meant = (found.value, found.origin);
    }
    let message = "this variable stands for itself, by way of others or directly";
    Err((meant.1, message.to_owned()))
}

/// The name in `var(--name)`, if that is what a value is.
fn variable(value: &str) -> Option<&str> {
    let inside = value.strip_prefix("var(");
    let inside = inside.and_then(|rest| rest.strip_suffix(')'));
    inside.map(str::trim)
}

/// A colour: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` or `transparent`.
fn color(value: &str) -> Result<Color, String> {
    if value == "transparent" {
        return Ok(CLEAR);
    }
    let digits = value.strip_prefix('#').unwrap_or_default();
    // The short forms give each digit once.
    let doubled: String = digits.chars().flat_map(|digit| [digit, digit]).collect();
    let long = if matches!(digits.len(), 3 | 4) {
        format!("#{doubled}")
    } else {
        value.to_owned()
    };
    let unknown = |_| {
        format!(
            "`{value}` is not a color; write `#rgb`, `#rrggbb`, either with opacity as a \
             further digit or pair, or `transparent`"
        )
    };
    long.parse().map_err(unknown)
}

/// A whole number of `unit` from `least` to `most`. The unit may be left
/// off.
fn size(value: &str, unit: &str, least: u32, most: u32) -> Result<u32, String> {
    let digits = value.strip_suffix(unit).unwrap_or(value);
    let number = digits.parse::<u32>().ok();
    let number = number.filter(|number| (least..=most).contains(number));
    number.ok_or_else(|| {
        format!("`{value}` is not a size from {least}{unit} to {most}{unit}, in whole {unit}")
    })
}

/// The name of a font family, with or without quotation marks.
fn family(value: &str) -> Result<String, String> {
    let name = value.trim_matches(['"', '\'']).trim();
    let named = (!name.is_empty()).then(|| name.to_owned());
    named.ok_or_else(|| "a font family needs a name".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Line, State};

    const FOCUSED: States = States::NONE.with(State::Focused);
    const ACTIVE: States = States::NONE.with(State::Active);
    const HOVERED: States = States::NONE.with(State::Hovered);

    const BACKGROUND: Color = Color::rgb(0x14, 0x16, 0x1b);
    const ACCENT: Color = Color::rgb(0x5b, 0x9c, 0xf5);
    const SURFACE: Color = Color::rgb(0x23, 0x26, 0x2e);
    const SURFACE_DIM: Color = Color::rgb(0x1b, 0x1d, 0x23);
    const TEXT: Color = Color::rgb(0xe6, 0xe8, 0xee);
    const TEXT_DIM: Color = Color::rgb(0x8b, 0x90, 0xa0);
    const BORDER: Color = Color::rgb(0x2c, 0x30, 0x3a);
    const RED: Color = Color::rgb(255, 0, 0);

    fn line(color: Color, width: u32) -> Line {
        Line { color, width }
    }

    fn problem(stylesheet: &str) -> String {
        Theme::parse(stylesheet).unwrap_err().to_string()
    }

    #[test]
    fn the_built_in_stylesheet_is_sound() {
        let sheet = Sheet::parse(BUILT_IN).unwrap();
        assert!(build(&sheet, &Sheet::default()).is_ok());
        assert_eq!(Theme::parse("").unwrap(), Theme::default());
        assert_eq!(
            Theme::parse("/* nothing of my own */").unwrap(),
            Theme::default()
        );
    }

    #[test]
    fn a_frame_has_a_border_in_the_colour_of_its_focus() {
        let theme = Theme::default();
        let focused = theme.style(Part::Window, FOCUSED);
        assert_eq!(focused.border, line(ACCENT, 1));
        assert_eq!(focused.radius, 8);
        assert_eq!(focused.background, CLEAR, "the window fills it itself");
        let other = theme.style(Part::Window, States::NONE);
        assert_eq!(other.border, line(BORDER, 1));
        assert_eq!(other.radius, 8);
    }

    #[test]
    fn a_titlebar_takes_its_colours_from_its_focus() {
        let theme = Theme::default();
        let focused = theme.style(Part::Titlebar, FOCUSED);
        assert_eq!((focused.background, focused.color), (SURFACE, TEXT));
        assert_eq!(focused.height, 28);
        let other = theme.style(Part::Titlebar, States::NONE);
        assert_eq!((other.background, other.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(focused.border, Line::NONE, "the frame has the border");
    }

    #[test]
    fn the_tab_being_shown_is_underlined_and_the_others_are_dimmed() {
        let theme = Theme::default();
        let shown = theme.style(Part::Tab, FOCUSED.with(State::Active));
        assert_eq!((shown.background, shown.color), (SURFACE, TEXT));
        assert_eq!(shown.underline, line(ACCENT, 2));
        assert_eq!(shown.border, line(ACCENT, 1));

        // In a frame without focus the line is there but does not stand out.
        let shown = theme.style(Part::Tab, ACTIVE);
        assert_eq!((shown.background, shown.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(shown.underline, line(BORDER, 2));

        let behind = theme.style(Part::Tab, FOCUSED);
        assert_eq!((behind.background, behind.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(behind.underline.width, 0);
        assert_eq!(behind.border, line(ACCENT, 1));
    }

    #[test]
    fn the_menu_row_under_the_pointer_is_filled_with_the_accent() {
        let theme = Theme::default();
        let menu = theme.style(Part::Menu, States::NONE);
        assert_eq!((menu.background, menu.color), (SURFACE, TEXT));
        assert_eq!(menu.border, line(BORDER, 1));
        let row = theme.style(Part::MenuItem, States::NONE);
        assert_eq!((row.background, row.color), (CLEAR, TEXT));
        let under = theme.style(Part::MenuItem, HOVERED);
        assert_eq!((under.background, under.color), (ACCENT, BACKGROUND));
        assert_eq!((row.height, under.height), (28, 28));
        let separator = theme.style(Part::MenuSeparator, States::NONE);
        assert_eq!(separator.color, BORDER);
    }

    #[test]
    fn the_notice_is_outlined_and_headed_in_the_accent() {
        let theme = Theme::default();
        let notice = theme.style(Part::Notice, States::NONE);
        assert_eq!((notice.background, notice.color), (SURFACE, TEXT));
        assert_eq!(notice.border, line(ACCENT, 1));
        let heading = theme.style(Part::NoticeHeading, States::NONE);
        assert_eq!(heading.color, ACCENT);
    }

    #[test]
    fn what_is_active_on_the_panel_is_lit_and_underlined() {
        let theme = Theme::default();
        let panel = theme.style(Part::Panel, States::NONE);
        assert_eq!((panel.background, panel.height), (SURFACE_DIM, 28));
        let button = theme.style(Part::PanelButton, States::NONE);
        assert_eq!((button.background, button.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(button.underline.width, 0);
        let lit = theme.style(Part::PanelButton, ACTIVE);
        assert_eq!((lit.background, lit.color), (SURFACE, TEXT));
        assert_eq!(lit.underline, line(ACCENT, 2));
        assert_eq!(theme.style(Part::Clock, States::NONE).color, TEXT);
        let desktop = theme.style(Part::Desktop, FOCUSED);
        assert_eq!(desktop.background, BACKGROUND);
    }

    #[test]
    fn every_part_is_written_in_the_same_font_unless_told_otherwise() {
        let theme = Theme::default();
        for part in Part::ALL {
            assert_eq!(theme.font(part, States::NONE), &Font::default());
        }
        let theme = Theme::parse(
            "* { font-family: \"DejaVu Sans\"; }\n\
             panel { font-size: 9pt; }\n\
             clock { font-family: 'monospace' }\n\
             tab:active { font-size: 14 }",
        );
        let theme = theme.unwrap();
        let font = |part, states| {
            let font = theme.font(part, states);
            (font.family.as_str(), font.size.get())
        };
        assert_eq!(font(Part::Menu, States::NONE), ("DejaVu Sans", 11));
        assert_eq!(font(Part::Panel, States::NONE), ("DejaVu Sans", 9));
        assert_eq!(font(Part::Clock, States::NONE), ("monospace", 11));
        assert_eq!(font(Part::Tab, States::NONE), ("DejaVu Sans", 11));
        assert_eq!(
            font(Part::Tab, ACTIVE.with(State::Focused)),
            ("DejaVu Sans", 14)
        );
    }

    #[test]
    fn a_rule_of_ones_own_changes_only_what_it_names() {
        let theme = Theme::parse("window { border-radius: 0; border-width: 3px }").unwrap();
        let frame = theme.style(Part::Window, States::NONE);
        assert_eq!((frame.border.width, frame.radius), (3, 0));
        assert_eq!(frame.border.color, BORDER, "left as it was");
        assert_eq!(
            theme.style(Part::Titlebar, FOCUSED),
            Theme::default().style(Part::Titlebar, FOCUSED)
        );
        assert_ne!(theme, Theme::default());
    }

    #[test]
    fn giving_a_variable_another_value_changes_everything_that_uses_it() {
        let theme = Theme::parse(":root { --accent: #f00; }").unwrap();
        assert_eq!(theme.style(Part::Window, FOCUSED).border.color, RED);
        assert_eq!(theme.style(Part::MenuItem, HOVERED).background, RED);
        assert_eq!(theme.style(Part::NoticeHeading, States::NONE).color, RED);
        assert_eq!(theme.style(Part::PanelButton, ACTIVE).underline.color, RED);
        assert_eq!(theme.style(Part::Window, States::NONE).border.color, BORDER);
    }

    #[test]
    fn a_theme_may_have_variables_of_its_own_standing_for_others() {
        let theme = Theme::parse(
            ":root { --loud: var( --accent ); --mine: var(--loud); }\n\
             clock { color: var(--mine); background-color: transparent; }",
        );
        let clock = theme.unwrap().style(Part::Clock, States::NONE);
        assert_eq!((clock.color, clock.background), (ACCENT, CLEAR));
        // The last word on a variable is the one that counts.
        let theme =
            Theme::parse(":root { --mine: #f00; --mine: #00f; } clock { color: var(--mine) }");
        let clock = theme.unwrap().style(Part::Clock, States::NONE);
        assert_eq!(clock.color, Color::rgb(0, 0, 255));
    }

    #[test]
    fn ones_own_rule_wins_over_a_built_in_one_however_particular() {
        // The built-in theme has a rule for a tab that is active and
        // focused. A plain rule of the user's still has the last word.
        let theme = Theme::parse("tab { color: #f00; }").unwrap();
        assert_eq!(theme.style(Part::Tab, States::NONE).color, RED);
        assert_eq!(
            theme.style(Part::Tab, FOCUSED.with(State::Active)).color,
            RED
        );
    }

    #[test]
    fn among_ones_own_rules_the_more_particular_wins_and_then_the_later() {
        let theme = Theme::parse(
            "tab:active:focus { color: #00f; }\n\
             tab:active { color: #0f0; }\n\
             tab { color: #f00; }\n\
             * { color: #fff; background: #000; }\n\
             clock { color: #f00; }\n\
             clock { color: #0f0; }",
        );
        let theme = theme.unwrap();
        let color = |part, states| theme.style(part, states).color;
        assert_eq!(
            color(Part::Tab, FOCUSED.with(State::Active)),
            Color::rgb(0, 0, 255)
        );
        assert_eq!(color(Part::Tab, ACTIVE), Color::rgb(0, 255, 0));
        assert_eq!(color(Part::Tab, FOCUSED), RED);
        assert_eq!(color(Part::Menu, States::NONE), Color::rgb(255, 255, 255));
        assert_eq!(color(Part::Clock, States::NONE), Color::rgb(0, 255, 0));
        // A rule for every part still loses to one for the part.
        assert_eq!(theme.style(Part::Tab, States::NONE).color, RED);
        assert_eq!(
            theme.style(Part::Menu, States::NONE).background,
            Color::rgb(0, 0, 0)
        );
    }

    #[test]
    fn a_rule_for_several_parts_applies_to_each() {
        let theme = Theme::parse("clock, notice-heading, tab:active { color: #f00 }").unwrap();
        assert_eq!(theme.style(Part::Clock, States::NONE).color, RED);
        assert_eq!(theme.style(Part::NoticeHeading, States::NONE).color, RED);
        assert_eq!(theme.style(Part::Tab, ACTIVE).color, RED);
        assert_eq!(theme.style(Part::Tab, States::NONE).color, TEXT_DIM);
    }

    #[test]
    fn colours_may_be_written_short_or_long_with_or_without_opacity() {
        let background = |value: &str| {
            let theme = Theme::parse(&format!("panel {{ background: {value}; }}"));
            theme.map(|theme| theme.style(Part::Panel, States::NONE).background)
        };
        assert_eq!(background("#f00"), Ok(RED));
        assert_eq!(background("#F00"), Ok(RED));
        assert_eq!(background("#ff0000"), Ok(RED));
        assert_eq!(background("#f008"), Ok(Color::rgba(255, 0, 0, 0x88)));
        assert_eq!(background("#ff000080"), Ok(Color::rgba(255, 0, 0, 0x80)));
        assert_eq!(background("transparent"), Ok(CLEAR));
        for wrong in ["red", "#ff", "#ff000", "#gggggg", "ff0000", "#"] {
            let message = background(wrong).unwrap_err().to_string();
            assert!(message.contains("is not a color"), "{wrong}: {message}");
        }
    }

    #[test]
    fn every_property_can_be_set() {
        let theme = Theme::parse(
            "panel-button {\n\
               background: #010101;\n\
               color: #020202;\n\
               border-color: #030303;\n\
               border-width: 4px;\n\
               border-radius: 5px;\n\
               border-bottom-color: #060606;\n\
               border-bottom-width: 7px;\n\
               height: 18px;\n\
             }",
        );
        let style = theme.unwrap().style(Part::PanelButton, States::NONE);
        let expected = Style {
            background: Color::rgb(1, 1, 1),
            color: Color::rgb(2, 2, 2),
            border: line(Color::rgb(3, 3, 3), 4),
            underline: line(Color::rgb(6, 6, 6), 7),
            radius: 5,
            height: 18,
        };
        assert_eq!(style, expected);
    }

    #[test]
    fn sizes_outside_what_can_be_drawn_are_refused() {
        let cases = [
            ("window { border-width: 17px }", "from 0px to 16px"),
            ("window { border-radius: 33px }", "from 0px to 32px"),
            ("tab { border-bottom-width: 99 }", "from 0px to 16px"),
            ("titlebar { height: 8px }", "from 16px to 64px"),
            ("titlebar { height: 1.5px }", "in whole px"),
            ("titlebar { height: 2em }", "`2em` is not a size"),
            ("titlebar { height: -20px }", "`-20px` is not a size"),
            ("* { font-size: 200pt }", "from 6pt to 72pt"),
            ("* { font-size: 12px }", "in whole pt"),
        ];
        for (stylesheet, expected) in cases {
            let message = problem(stylesheet);
            assert!(message.contains(expected), "{stylesheet}: {message}");
        }
        assert!(problem("* { font-family: \"\" }").contains("needs a name"));
    }

    #[test]
    fn a_mistake_is_reported_where_it_was_written() {
        assert_eq!(
            problem("tab {\n  colour: #fff;\n}"),
            format!("2:3: `colour` is not a property; the properties are {PROPERTIES}")
        );
        assert!(problem("tab {\n  color: banana;\n}").starts_with("2:10: `banana` is not a color"));
        assert_eq!(
            problem("clock { color: var(--nope); }"),
            "1:16: there is no variable `--nope`"
        );
        assert!(problem("clock { color: var(--accent; }").starts_with("1:16: `var(--accent`"));
        assert!(problem("gadget { }").starts_with("1:1: `gadget` is not a part"));
    }

    #[test]
    fn a_bad_value_given_to_a_variable_is_reported_at_the_variable() {
        // The rule that uses it is a built-in one, which is not where the
        // mistake is.
        let message = problem("\n:root { --accent: banana; }");
        assert!(
            message.starts_with("2:19: `banana` is not a color"),
            "{message}"
        );
        let message = problem(":root { --mine: var(--gone); }\nclock { color: var(--mine); }");
        assert_eq!(message, "1:17: there is no variable `--gone`");
    }

    #[test]
    fn variables_that_go_round_in_a_circle_are_refused() {
        let message = problem(":root { --a: var(--b); --b: var(--a); }\nclock { color: var(--a) }");
        assert!(message.contains("stands for itself"), "{message}");
        assert!(message.starts_with("1:"), "{message}");
        // A long way round that does end is no circle.
        let chain = ":root { --a: #f00; --b: var(--a); --c: var(--b); --d: var(--c); }\n\
                     clock { color: var(--d) }";
        let clock = Theme::parse(chain)
            .unwrap()
            .style(Part::Clock, States::NONE);
        assert_eq!(clock.color, RED);
    }

    #[test]
    fn a_very_long_stylesheet_is_worked_out_all_the_same() {
        use std::fmt::Write;

        let mut stylesheet = String::from(":root {");
        for index in 0..5000 {
            write!(stylesheet, " --v{index}: #000;").unwrap();
        }
        stylesheet.push_str(" }\n");
        let everything = Part::ALL.map(Part::name).join(", ");
        for index in 0..5000 {
            writeln!(stylesheet, "{everything} {{ color: var(--v{index}); }}").unwrap();
        }
        stylesheet.push_str("clock { color: #f00; }");
        let theme = Theme::parse(&stylesheet).unwrap();
        assert_eq!(theme.style(Part::Clock, States::NONE).color, RED);
        assert_eq!(
            theme.style(Part::Menu, States::NONE).color,
            Color::rgb(0, 0, 0)
        );
    }

    #[test]
    fn a_theme_equals_its_copies_and_others_that_work_out_the_same() {
        let theme = Theme::parse("clock { color: #f00 }").unwrap();
        let copy = theme.clone();
        assert_eq!(theme, copy);
        assert_eq!(theme, Theme::parse("clock{color:#ff0000;}").unwrap());
        assert_ne!(theme, Theme::parse("clock { color: #f01 }").unwrap());
    }

    #[test]
    fn a_problem_without_a_place_is_just_what_is_wrong() {
        let error = ThemeError {
            position: None,
            message: "something".to_owned(),
        };
        assert_eq!(error.to_string(), "something");
        let source: &dyn std::error::Error = &error;
        assert!(source.source().is_none());
    }

    #[test]
    fn a_theme_that_could_not_be_worked_out_draws_nothing_rather_than_fail() {
        let empty = Theme {
            table: Arc::new(Table::default()),
        };
        assert_eq!(empty.style(Part::Clock, States::NONE), Style::NONE);
        assert_eq!(empty.font(Part::Clock, States::NONE), &Font::default());
    }
}
