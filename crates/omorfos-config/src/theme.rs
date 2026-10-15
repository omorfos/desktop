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
use crate::value::{color, fraction, size, time, words};
use crate::{Bounded, Color, Easing, Edges, Part, Position, States, Style, Transition};

/// The stylesheet every theme starts from.
const BUILT_IN: &str = include_str!("default.css");

/// A font: a family and a size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Font {
    /// A font family name, or a generic one such as `sans-serif`.
    pub family: String,
    /// Size in points.
    pub size: Bounded<6, 72>,
    /// How heavy it is, from 100 to 900. A regular font is 400 and a bold
    /// one 700.
    pub weight: u16,
    /// Whether it slants.
    pub italic: bool,
}

impl Default for Font {
    fn default() -> Self {
        Self {
            family: "sans-serif".to_owned(),
            size: Bounded::clamped(11),
            weight: 400,
            italic: false,
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

/// A side of a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

/// The sides, by the word a property names them with.
const SIDES: [(&str, Side); 4] = [
    ("top", Side::Top),
    ("right", Side::Right),
    ("bottom", Side::Bottom),
    ("left", Side::Left),
];

/// One property of a part, understood. Where there is a side, `None` is
/// every side.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Setting {
    Background(Color),
    Color(Color),
    Opacity(u8),
    BorderColor(Option<Side>, Color),
    BorderWidth(Option<Side>, u32),
    Padding(Option<Side>, u32),
    Margin(Option<Side>, u32),
    Radius(u32),
    Height(u32),
    MinWidth(u32),
    MaxWidth(u32),
    FontFamily(String),
    FontSize(u32),
    FontWeight(u16),
    FontItalic(bool),
    TransitionDuration(u32),
    TransitionEasing(Easing),
}

/// The properties a stylesheet can give, as it is told when it gives
/// another.
const PROPERTIES: &str = "background, color, opacity, border, border-color, border-width, \
                          border-radius, padding, margin, height, min-width, max-width, \
                          font-family, font-size, font-weight, font-style, transition, \
                          transition-duration and transition-timing-function; border, padding \
                          and margin may name a side, as in border-bottom-color";

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
            let mut all = Vec::new();
            for declaration in &rule.declarations {
                all.extend(settings(declaration, by_user, &variables)?);
            }
            understood.push((by_user, &rule.selectors, all));
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
            let mut opacity = u8::MAX;
            let applies = |rule: &&Ranked| {
                let selector = rule.selector;
                selector.part.is_none_or(|named| named == part) && selector.states.within(states)
            };
            for rule in ranked.iter().filter(applies) {
                for setting in rule.settings {
                    look.set(setting, &mut opacity);
                }
            }
            look.fade(opacity);
            looks.push(look);
        }
    }
    Ok(Table {
        looks,
        spare: Font::default(),
    })
}

/// Applies `change` to the side named, or to every side.
fn on<T>(edges: &mut Edges<T>, side: Option<Side>, mut change: impl FnMut(&mut T)) {
    let Edges {
        top,
        right,
        bottom,
        left,
    } = edges;
    let each = [
        (Side::Top, top),
        (Side::Right, right),
        (Side::Bottom, bottom),
        (Side::Left, left),
    ];
    for (which, edge) in each {
        if side.is_none_or(|named| named == which) {
            change(edge);
        }
    }
}

impl Look {
    fn set(&mut self, setting: &Setting, opacity: &mut u8) {
        let style = &mut self.style;
        match setting {
            Setting::Background(color) => style.background = *color,
            Setting::Color(color) => style.color = *color,
            Setting::Opacity(share) => *opacity = *share,
            Setting::BorderColor(side, color) => {
                on(&mut style.border, *side, |line| line.color = *color);
            }
            Setting::BorderWidth(side, width) => {
                on(&mut style.border, *side, |line| line.width = *width);
            }
            Setting::Padding(side, room) => on(&mut style.padding, *side, |edge| *edge = *room),
            Setting::Margin(side, room) => on(&mut style.margin, *side, |edge| *edge = *room),
            Setting::Radius(radius) => style.radius = *radius,
            Setting::Height(height) => style.height = *height,
            Setting::MinWidth(width) => style.min_width = *width,
            Setting::MaxWidth(width) => style.max_width = *width,
            Setting::FontFamily(family) => self.font.family.clone_from(family),
            Setting::FontSize(size) => self.font.size = Bounded::clamped(*size),
            Setting::FontWeight(weight) => self.font.weight = *weight,
            Setting::FontItalic(italic) => self.font.italic = *italic,
            Setting::TransitionDuration(millis) => style.transition.duration = *millis,
            Setting::TransitionEasing(easing) => style.transition.easing = *easing,
        }
    }

    /// Makes everything of the part as see-through as its opacity says.
    fn fade(&mut self, opacity: u8) {
        let faded = |color: &mut Color| {
            let alpha = u16::from(color.alpha) * u16::from(opacity) / u16::from(u8::MAX);
            color.alpha = u8::try_from(alpha).unwrap_or(u8::MAX);
        };
        let style = &mut self.style;
        faded(&mut style.background);
        faded(&mut style.color);
        on(&mut style.border, None, |line| faded(&mut line.color));
    }
}

/// Understands one property and its value, which may come to several
/// settings.
fn settings(
    declaration: &Declaration,
    by_user: bool,
    variables: &Variables,
) -> Result<Vec<Setting>, Problem> {
    let written = by_user.then_some(declaration.offset);
    let (value, origin) = resolve(&declaration.value, written, variables)?;
    let value = value.as_str();
    let (property, side) = sided(&declaration.name);
    let one = |setting: Result<Setting, String>| setting.map(|setting| vec![setting]);
    let understood = match (property.as_str(), side) {
        ("background" | "background-color", None) => one(color(value).map(Setting::Background)),
        ("color", None) => one(color(value).map(Setting::Color)),
        ("opacity", None) => one(opacity(value)),
        ("border", side) => border(value, side),
        ("border-color", side) => one(color(value).map(|color| Setting::BorderColor(side, color))),
        ("border-width", side) => {
            one(size(value, "px", 0, 16).map(|width| Setting::BorderWidth(side, width)))
        }
        ("border-radius", None) => one(size(value, "px", 0, 32).map(Setting::Radius)),
        ("padding", side) => around(value, side, Setting::Padding),
        ("margin", side) => around(value, side, Setting::Margin),
        ("height", None) => one(size(value, "px", 16, 64).map(Setting::Height)),
        ("min-width", None) => one(size(value, "px", 0, 4096).map(Setting::MinWidth)),
        ("max-width", None) => one(size(value, "px", 0, 4096).map(Setting::MaxWidth)),
        ("font-family", None) => one(family(value).map(Setting::FontFamily)),
        ("font-size", None) => one(size(value, "pt", 6, 72).map(Setting::FontSize)),
        ("font-weight", None) => one(weight(value)),
        ("font-style", None) => one(slant(value)),
        ("transition", None) => transition(value),
        ("transition-duration", None) => one(lasting(value).map(Setting::TransitionDuration)),
        ("transition-timing-function", None) => one(easing(value).map(Setting::TransitionEasing)),
        _ => {
            let name = &declaration.name;
            let message = format!("`{name}` is not a property; the properties are {PROPERTIES}");
            return Err((by_user.then_some(declaration.at), message));
        }
    };
    understood.map_err(|message| (origin, message))
}

/// Takes the side out of a property's name: `border-bottom-color` is
/// `border-color` of the bottom.
fn sided(name: &str) -> (String, Option<Side>) {
    let found = SIDES.iter().find_map(|(word, side)| {
        let marked = format!("-{word}");
        let named = name.contains(&marked);
        named.then(|| (name.replacen(&marked, "", 1), Some(*side)))
    });
    found.unwrap_or_else(|| (name.to_owned(), None))
}

/// How many variables a value may be made of, one inside another or side
/// by side, before the theme is taken to be going round in a circle.
const SUBSTITUTIONS: usize = 32;

/// The longest a value may grow as its variables are filled in.
const LONGEST: usize = 1024;

/// Fills in the variables of a value, to what is meant and where that was
/// written.
fn resolve(
    value: &str,
    written: Option<usize>,
    variables: &Variables,
) -> Result<(String, Option<usize>), Problem> {
    let mut meant = value.to_owned();
    let mut origin = written;
    for _ in 0..SUBSTITUTIONS {
        let Some(start) = meant.find("var(") else {
            return Ok((meant, origin));
        };
        let inside = meant.get(start + 4..).unwrap_or_default();
        let Some(length) = inside.find(')') else {
            return Err((origin, "this `var(` is never closed with `)`".to_owned()));
        };
        let name = inside.get(..length).unwrap_or_default().trim();
        let Some(found) = variables.get(name) else {
            return Err((origin, format!("there is no variable `{name}`")));
        };
        if meant.len() + found.value.len() > LONGEST {
            let message = "this value grows too long as its variables are filled in";
            return Err((origin, message.to_owned()));
        }
        // What went wrong from here on went wrong where the variable was
        // given its value, if that was in the user's stylesheet.
        origin = found.origin.or(origin);
        meant.replace_range(start..=start + 4 + length, found.value);
    }
    let message = "this variable stands for itself, by way of others or directly";
    Err((origin, message.to_owned()))
}

/// How see-through a part is: from `0` to `1`, or a percentage.
fn opacity(value: &str) -> Result<Setting, String> {
    let share = fraction(value).map(Setting::Opacity);
    share.ok_or_else(|| format!("`{value}` is not an opacity from 0 to 1, or from 0% to 100%"))
}

/// A border all at once: a width, `solid` or `none`, and a colour, any of
/// them and in any order.
fn border(value: &str, side: Option<Side>) -> Result<Vec<Setting>, String> {
    let understood = words(value).into_iter().map(|word| match word {
        // The only kind of line there is.
        "solid" => Ok(None),
        "none" | "hidden" => Ok(Some(Setting::BorderWidth(side, 0))),
        sized if sized.starts_with(|first: char| first.is_ascii_digit()) => {
            let width = size(sized, "px", 0, 16);
            width.map(|width| Some(Setting::BorderWidth(side, width)))
        }
        other => color(other).map(|color| Some(Setting::BorderColor(side, color))),
    });
    let understood: Result<Vec<Option<Setting>>, String> = understood.collect();
    understood.map(|settings| settings.into_iter().flatten().collect())
}

/// Padding or a margin: for one side, or for all at once as one to four
/// sizes.
fn around(
    value: &str,
    side: Option<Side>,
    setting: fn(Option<Side>, u32) -> Setting,
) -> Result<Vec<Setting>, String> {
    let sizes = words(value).into_iter().map(|word| size(word, "px", 0, 64));
    let sizes: Vec<u32> = sizes.collect::<Result<_, _>>()?;
    let [top, right, bottom, left] = match (sizes.as_slice(), side) {
        ([all], Some(_)) => return Ok(vec![setting(side, *all)]),
        ([all], None) => [*all; 4],
        ([upright, level], None) => [*upright, *level, *upright, *level],
        ([top, level, bottom], None) => [*top, *level, *bottom, *level],
        ([top, right, bottom, left], None) => [*top, *right, *bottom, *left],
        _ => {
            let message = "give one size for a side, or for all of them one to four: all; top \
                           and bottom, then the sides; top, the sides, bottom; or top, right, \
                           bottom, left";
            return Err(message.to_owned());
        }
    };
    let each = [
        (Side::Top, top),
        (Side::Right, right),
        (Side::Bottom, bottom),
        (Side::Left, left),
    ];
    Ok(each.map(|(side, size)| setting(Some(side), size)).into())
}

/// How heavy a font is: `normal`, `bold`, or a number from 100 to 900.
fn weight(value: &str) -> Result<Setting, String> {
    let number = match value {
        "normal" => Some(400),
        "bold" => Some(700),
        other => other.parse::<u16>().ok(),
    };
    let number = number.filter(|number| (100..=900).contains(number));
    let weight = number.map(Setting::FontWeight);
    weight.ok_or_else(|| {
        format!("`{value}` is not a font weight; write `normal`, `bold` or 100 to 900")
    })
}

/// Whether a font slants: `normal` or `italic`.
fn slant(value: &str) -> Result<Setting, String> {
    match value {
        "normal" => Ok(Setting::FontItalic(false)),
        "italic" | "oblique" => Ok(Setting::FontItalic(true)),
        _ => Err(format!(
            "`{value}` is not a font style; write `normal` or `italic`"
        )),
    }
}

/// How long a change of colours takes.
fn lasting(value: &str) -> Result<u32, String> {
    time(value, Transition::LONGEST)
}

/// How a change of colours gathers and loses speed, by its name.
fn easing(value: &str) -> Result<Easing, String> {
    let named = Easing::ALL
        .into_iter()
        .find(|easing| easing.name() == value);
    named.ok_or_else(|| {
        format!(
            "`{value}` is not a way for colours to change; write linear, ease, ease-in, \
             ease-out or ease-in-out"
        )
    })
}

/// A change of colours all at once: how long it takes and perhaps how it
/// goes, or `none`. What is left out is as it would be had nothing been
/// said, as in CSS.
fn transition(value: &str) -> Result<Vec<Setting>, String> {
    let mut change = Transition::NONE;
    for word in words(value) {
        match word {
            // Every colour of a part changes together.
            "none" | "all" => {}
            timed if timed.starts_with(|first: char| first.is_ascii_digit() || first == '.') => {
                change.duration = lasting(timed)?;
            }
            other => {
                change.easing = easing(other).map_err(|_| {
                    format!(
                        "`{other}` is not part of a transition; write how long the colours of \
                         the part take to change, as in `150ms` or `0.2s`, and perhaps how: \
                         linear, ease, ease-in, ease-out or ease-in-out"
                    )
                })?;
            }
        }
    }
    Ok(vec![
        Setting::TransitionDuration(change.duration),
        Setting::TransitionEasing(change.easing),
    ])
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
    use crate::style::CLEAR;
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

    /// How a part looks under a theme of one rule, in no state.
    fn styled(part: Part, stylesheet: &str) -> Style {
        Theme::parse(stylesheet).unwrap().style(part, States::NONE)
    }

    #[test]
    fn colours_change_over_the_time_and_in_the_way_a_theme_says() {
        let change = |stylesheet: &str| styled(Part::Tab, stylesheet).transition;
        let gentle = Transition {
            duration: 150,
            easing: Easing::EaseOut,
        };
        assert_eq!(change("tab { transition: 150ms ease-out; }"), gentle);
        assert_eq!(change("tab { transition: all ease-out 0.15s; }"), gentle);
        assert_eq!(change("* { transition: .15s ease-out; }"), gentle);
        assert_eq!(
            change("tab { transition: 150ms; }").easing,
            Easing::Ease,
            "as CSS does when it is not told"
        );
        assert_eq!(
            change("tab { transition-duration: 150ms; transition-timing-function: ease-out; }"),
            gentle
        );
        assert_eq!(
            change("* { transition: 1s linear; } tab { transition: none; }"),
            Transition::NONE,
            "said all at once, what is left out is as if never said"
        );
        assert_eq!(
            change("* { transition: 1s linear; } tab { transition-duration: 0s; }").easing,
            Easing::Linear
        );
    }

    #[test]
    fn a_state_can_have_a_change_of_its_own() {
        let theme = Theme::parse("tab { transition: 300ms; } tab:active { transition: 50ms; }");
        let theme = theme.unwrap();
        assert_eq!(
            theme.style(Part::Tab, States::NONE).transition.duration,
            300
        );
        assert_eq!(theme.style(Part::Tab, ACTIVE).transition.duration, 50);
    }

    #[test]
    fn colours_take_a_moment_to_change_unless_a_theme_says_otherwise() {
        let gentle = Transition {
            duration: 120,
            easing: Easing::EaseOut,
        };
        let theme = Theme::default();
        for part in Part::ALL {
            assert_eq!(theme.style(part, States::NONE).transition, gentle);
            assert_eq!(theme.style(part, HOVERED).transition, gentle);
        }
        let still = Theme::parse("* { transition: none; }").unwrap();
        for part in Part::ALL {
            assert_eq!(still.style(part, FOCUSED).transition, Transition::NONE);
        }
    }

    #[test]
    fn a_change_that_is_not_understood_is_an_error() {
        let message = problem("tab { transition: background 150ms; }");
        assert!(
            message.contains("`background` is not part of a transition"),
            "{message}"
        );
        let message = problem("tab { transition: 11s; }");
        assert!(message.contains("`11s` is not a time"), "{message}");
        let message = problem("tab { transition-duration: soon; }");
        assert!(message.contains("`soon` is not a time"), "{message}");
        let message = problem("tab { transition-timing-function: bouncy; }");
        assert!(
            message.contains("`bouncy` is not a way for colours to change"),
            "{message}"
        );
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
        assert_eq!(focused.border, Edges::all(line(ACCENT, 1)));
        assert_eq!(focused.radius, 8);
        assert_eq!(focused.background, CLEAR, "the window fills it itself");
        let other = theme.style(Part::Window, States::NONE);
        assert_eq!(other.border, Edges::all(line(BORDER, 1)));
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
        assert_eq!(focused.border.widths(), Edges::NONE, "the frame has it");
    }

    #[test]
    fn the_tab_being_shown_is_underlined_and_the_others_are_dimmed() {
        let theme = Theme::default();
        let shown = theme.style(Part::Tab, FOCUSED.with(State::Active));
        assert_eq!((shown.background, shown.color), (SURFACE, TEXT));
        assert_eq!(shown.border.bottom, line(ACCENT, 2));
        assert_eq!(shown.border.left, line(ACCENT, 1));
        assert_eq!((shown.border.top.width, shown.border.right.width), (0, 0));
        assert_eq!(shown.padding.across(), 20);

        // In a frame without focus the line is there but does not stand out.
        let shown = theme.style(Part::Tab, ACTIVE);
        assert_eq!((shown.background, shown.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(shown.border.bottom, line(BORDER, 2));

        let behind = theme.style(Part::Tab, FOCUSED);
        assert_eq!((behind.background, behind.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(behind.border.bottom.width, 0);
        assert_eq!(behind.border.left, line(ACCENT, 1));
    }

    #[test]
    fn the_menu_row_under_the_pointer_is_filled_with_the_accent() {
        let theme = Theme::default();
        let menu = theme.style(Part::Menu, States::NONE);
        assert_eq!((menu.background, menu.color), (SURFACE, TEXT));
        assert_eq!(menu.border, Edges::all(line(BORDER, 1)));
        let row = theme.style(Part::MenuItem, States::NONE);
        assert_eq!((row.background, row.color), (CLEAR, TEXT));
        assert_eq!((row.padding.left, row.padding.right), (10, 10));
        let under = theme.style(Part::MenuItem, HOVERED);
        assert_eq!((under.background, under.color), (ACCENT, BACKGROUND));
        assert_eq!((row.height, under.height), (28, 28));
        let separator = theme.style(Part::MenuSeparator, States::NONE);
        assert_eq!(separator.color, BORDER);
    }

    #[test]
    fn the_line_between_windows_is_drawn_as_their_borders_are() {
        let theme = Theme::default();
        let line = theme.style(Part::TileLine, States::NONE);
        assert_eq!(line.color, BORDER);
    }

    #[test]
    fn the_notice_is_outlined_and_headed_in_the_accent() {
        let theme = Theme::default();
        let notice = theme.style(Part::Notice, States::NONE);
        assert_eq!((notice.background, notice.color), (SURFACE, TEXT));
        assert_eq!(notice.border, Edges::all(line(ACCENT, 1)));
        assert_eq!(notice.padding, Edges::all(10));
        let heading = theme.style(Part::NoticeHeading, States::NONE);
        assert_eq!(heading.color, ACCENT);
    }

    #[test]
    fn the_mark_of_capture_is_a_red_dot_on_the_background() {
        let theme = Theme::default();
        let mark = theme.style(Part::Capture, States::NONE);
        assert_eq!(mark.background, BACKGROUND);
        assert_eq!(mark.color, Color::rgb(0xf0, 0x52, 0x4f));
    }

    #[test]
    fn what_is_active_on_the_panel_is_lit_and_underlined() {
        let theme = Theme::default();
        let panel = theme.style(Part::Panel, States::NONE);
        assert_eq!((panel.background, panel.height), (SURFACE_DIM, 28));
        assert_eq!(panel.margin, Edges::NONE);
        for part in [Part::Workspace, Part::Task] {
            let button = theme.style(part, States::NONE);
            assert_eq!((button.background, button.color), (CLEAR, TEXT_DIM));
            assert_eq!(button.border.bottom.width, 0);
            assert_eq!(button.padding.across(), 20);
            let lit = theme.style(part, ACTIVE);
            assert_eq!((lit.background, lit.color), (SURFACE, TEXT));
            assert_eq!(lit.border.bottom, line(ACCENT, 2));
        }
        let workspace = theme.style(Part::Workspace, States::NONE);
        assert_eq!((workspace.min_width, workspace.max_width), (28, 0));
        let task = theme.style(Part::Task, States::NONE);
        assert_eq!((task.min_width, task.max_width), (0, 240));
        assert_eq!(theme.style(Part::Clock, States::NONE).color, TEXT);
        let desktop = theme.style(Part::Desktop, FOCUSED);
        assert_eq!(desktop.background, BACKGROUND);
    }

    #[test]
    fn every_part_is_written_in_the_same_font_unless_told_otherwise() {
        let theme = Theme::default();
        for part in Part::ALL {
            let font = theme.font(part, States::NONE);
            assert_eq!(font.family, Font::default().family, "{part:?}");
            assert_eq!(font.size, Font::default().size, "{part:?}");
        }
        // Only a notification's title is heavier than the rest.
        let heavy = Part::ALL.into_iter();
        let heavy: Vec<Part> = heavy
            .filter(|part| theme.font(*part, States::NONE).weight != 400)
            .collect();
        assert_eq!(heavy, [Part::NotificationTitle]);
        let theme = Theme::parse(
            "* { font-family: \"DejaVu Sans\"; }\n\
             panel { font-size: 9pt; }\n\
             clock { font-family: 'monospace'; font-weight: bold; }\n\
             notice-heading { font-weight: 600; font-style: italic; }\n\
             task:active { font-style: oblique; }\n\
             task { font-style: normal; font-weight: normal; }\n\
             tab:active { font-size: 14 }",
        );
        let theme = theme.unwrap();
        let font = |part, states| {
            let font = theme.font(part, states);
            (
                font.family.as_str(),
                font.size.get(),
                font.weight,
                font.italic,
            )
        };
        let plain = States::NONE;
        assert_eq!(font(Part::Menu, plain), ("DejaVu Sans", 11, 400, false));
        assert_eq!(font(Part::Panel, plain), ("DejaVu Sans", 9, 400, false));
        assert_eq!(font(Part::Clock, plain), ("monospace", 11, 700, false));
        assert_eq!(
            font(Part::NoticeHeading, plain),
            ("DejaVu Sans", 11, 600, true)
        );
        assert_eq!(font(Part::Task, plain), ("DejaVu Sans", 11, 400, false));
        assert_eq!(font(Part::Task, ACTIVE), ("DejaVu Sans", 11, 400, true));
        assert_eq!(font(Part::Tab, plain), ("DejaVu Sans", 11, 400, false));
        assert_eq!(font(Part::Tab, ACTIVE).1, 14);
    }

    #[test]
    fn a_rule_of_ones_own_changes_only_what_it_names() {
        let theme = Theme::parse("window { border-radius: 0; border-width: 3px }").unwrap();
        let frame = theme.style(Part::Window, States::NONE);
        assert_eq!(
            frame.border,
            Edges::all(line(BORDER, 3)),
            "its colour is left"
        );
        assert_eq!(frame.radius, 0);
        assert_eq!(
            theme.style(Part::Titlebar, FOCUSED),
            Theme::default().style(Part::Titlebar, FOCUSED)
        );
        assert_ne!(theme, Theme::default());
    }

    #[test]
    fn giving_a_variable_another_value_changes_everything_that_uses_it() {
        let theme = Theme::parse(":root { --accent: #f00; }").unwrap();
        assert_eq!(theme.style(Part::Window, FOCUSED).border.top.color, RED);
        assert_eq!(theme.style(Part::MenuItem, HOVERED).background, RED);
        assert_eq!(theme.style(Part::NoticeHeading, States::NONE).color, RED);
        assert_eq!(theme.style(Part::Task, ACTIVE).border.bottom.color, RED);
        let plain = theme.style(Part::Window, States::NONE);
        assert_eq!(plain.border.top.color, BORDER);
    }

    #[test]
    fn a_variable_may_stand_for_others_and_be_part_of_a_value() {
        let theme = Theme::parse(
            ":root { --loud: var( --accent ); --mine: var(--loud); --thick: 3px; }\n\
             clock { color: var(--mine); background-color: transparent; }\n\
             task { border: var(--thick) solid var(--mine); padding: 0 var(--thick); }",
        );
        let theme = theme.unwrap();
        let clock = theme.style(Part::Clock, States::NONE);
        assert_eq!((clock.color, clock.background), (ACCENT, CLEAR));
        let task = theme.style(Part::Task, States::NONE);
        assert_eq!(task.border, Edges::all(line(ACCENT, 3)));
        assert_eq!((task.padding.left, task.padding.top), (3, 0));
        // The last word on a variable is the one that counts.
        let clock = styled(
            Part::Clock,
            ":root { --mine: #f00; --mine: #00f; } clock { color: var(--mine) }",
        );
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
        let blue = Color::rgb(0, 0, 255);
        assert_eq!(color(Part::Tab, FOCUSED.with(State::Active)), blue);
        assert_eq!(color(Part::Tab, ACTIVE), Color::rgb(0, 255, 0));
        assert_eq!(color(Part::Tab, FOCUSED), RED);
        assert_eq!(color(Part::Menu, States::NONE), Color::rgb(255, 255, 255));
        assert_eq!(color(Part::Clock, States::NONE), Color::rgb(0, 255, 0));
        // A rule for every part still loses to one for the part.
        assert_eq!(theme.style(Part::Tab, States::NONE).color, RED);
        let menu = theme.style(Part::Menu, States::NONE);
        assert_eq!(menu.background, Color::rgb(0, 0, 0));
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
    fn a_colour_may_be_written_any_way_css_writes_one() {
        for red in [
            "#f00",
            "#ff0000",
            "red",
            "rgb(255, 0, 0)",
            "hsl(0, 100%, 50%)",
        ] {
            let panel = styled(Part::Panel, &format!("panel {{ background: {red}; }}"));
            assert_eq!(panel.background, RED, "{red}");
        }
        let panel = styled(Part::Panel, "panel { background: rgba(20, 22, 27, 0.5); }");
        assert_eq!(panel.background, Color::rgba(20, 22, 27, 128));
        let message = problem("panel { background: banana; }");
        assert!(message.contains("`banana` is not a color"), "{message}");
    }

    #[test]
    fn every_property_can_be_set() {
        let theme = Theme::parse(
            "task {\n\
               background: #010101;\n\
               color: #020202;\n\
               border-color: #030303;\n\
               border-width: 4px;\n\
               border-radius: 5px;\n\
               border-bottom-color: #060606;\n\
               border-bottom-width: 7px;\n\
               height: 18px;\n\
               padding: 1px 2px 3px 4px;\n\
               margin: 5px 6px;\n\
               margin-left: 9px;\n\
               min-width: 40px;\n\
               max-width: 400px;\n\
               transition: 80ms linear;\n\
             }",
        );
        let style = theme.unwrap().style(Part::Task, States::NONE);
        let side = line(Color::rgb(3, 3, 3), 4);
        let expected = Style {
            background: Color::rgb(1, 1, 1),
            color: Color::rgb(2, 2, 2),
            border: Edges {
                bottom: line(Color::rgb(6, 6, 6), 7),
                ..Edges::all(side)
            },
            padding: Edges {
                top: 1,
                right: 2,
                bottom: 3,
                left: 4,
            },
            margin: Edges {
                top: 5,
                right: 6,
                bottom: 5,
                left: 9,
            },
            radius: 5,
            height: 18,
            min_width: 40,
            max_width: 400,
            transition: Transition {
                duration: 80,
                easing: Easing::Linear,
            },
        };
        assert_eq!(style, expected);
    }

    #[test]
    fn padding_and_margins_take_one_to_four_sizes_or_one_for_a_side() {
        let padding = |value: &str| {
            let edges = styled(Part::Clock, &format!("clock {{ padding: {value}; }}")).padding;
            (edges.top, edges.right, edges.bottom, edges.left)
        };
        assert_eq!(padding("4px"), (4, 4, 4, 4));
        assert_eq!(padding("1px 2px"), (1, 2, 1, 2));
        assert_eq!(padding("1px 2px 3px"), (1, 2, 3, 2));
        assert_eq!(padding("1px 2px 3px 4px"), (1, 2, 3, 4));
        let clock = styled(Part::Clock, "clock { padding: 0; padding-top: 7px; }");
        assert_eq!((clock.padding.top, clock.padding.left), (7, 0));
        let clock = styled(Part::Clock, "clock { margin: 2px; margin-right: 8px }");
        assert_eq!((clock.margin.top, clock.margin.right), (2, 8));

        let message = problem("clock { padding: 1px 2px 3px 4px 5px; }");
        assert!(message.contains("one to four"), "{message}");
        let message = problem("clock { margin-top: 1px 2px; }");
        assert!(message.contains("one size for a side"), "{message}");
        let message = problem("clock { padding: 1px wide; }");
        assert!(message.contains("`wide` is not a size"), "{message}");
        let message = problem("clock { margin: 65px; }");
        assert!(message.contains("from 0px to 64px"), "{message}");
    }

    #[test]
    fn a_border_may_be_given_all_at_once_for_every_side_or_for_one() {
        let task = styled(Part::Task, "task { border: 2px solid red; }");
        assert_eq!(task.border, Edges::all(line(RED, 2)));
        let task = styled(Part::Task, "task { border: rgb(255, 0, 0) 3px; }");
        assert_eq!(task.border, Edges::all(line(RED, 3)));
        let task = styled(
            Part::Task,
            "task { border: 2px solid red; border-top: none; }",
        );
        assert_eq!((task.border.top.width, task.border.left.width), (0, 2));
        let task = styled(Part::Task, "task { border-left: 5px solid #f00; }");
        assert_eq!(task.border.left, line(RED, 5));
        assert_eq!(task.border.right.width, 0);
        let task = styled(
            Part::Task,
            "task { border: 1px solid red; border-right-width: 6px; border-top: hidden; }",
        );
        assert_eq!(task.border.right, line(RED, 6));
        assert_eq!(task.border.top, line(RED, 0));
        // Naming only the kind of line changes nothing.
        assert_eq!(
            styled(Part::Task, "task { border: solid; }"),
            Theme::default().style(Part::Task, States::NONE)
        );

        let message = problem("task { border: 99px solid red; }");
        assert!(message.contains("from 0px to 16px"), "{message}");
        let message = problem("task { border: 1px dotted red; }");
        assert!(message.contains("`dotted` is not a color"), "{message}");
    }

    #[test]
    fn opacity_makes_everything_of_a_part_see_through() {
        let theme = Theme::parse("panel { opacity: 0.5; border: 1px solid #fff; }").unwrap();
        let panel = theme.style(Part::Panel, States::NONE);
        assert_eq!(panel.background.alpha, 128);
        assert_eq!(panel.color.alpha, 128);
        assert_eq!(panel.border.top.color, Color::rgba(255, 255, 255, 128));
        // It is the part's alone.
        assert_eq!(theme.style(Part::Clock, States::NONE).color.alpha, 255);

        let panel = styled(
            Part::Panel,
            "panel { opacity: 50%; background: #ffffff80; }",
        );
        assert_eq!(panel.background.alpha, 64, "on top of the colour's own");
        let panel = styled(Part::Panel, "panel { opacity: 0; }");
        assert_eq!((panel.background.alpha, panel.color.alpha), (0, 0));
        let message = problem("panel { opacity: 2; }");
        assert!(message.contains("`2` is not an opacity"), "{message}");
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
            ("task { min-width: 5000px }", "from 0px to 4096px"),
            ("task { max-width: wide }", "`wide` is not a size"),
            ("* { font-size: 200pt }", "from 6pt to 72pt"),
            ("* { font-size: 12px }", "in whole pt"),
            ("* { font-weight: 50 }", "`50` is not a font weight"),
            ("* { font-weight: heavy }", "`heavy` is not a font weight"),
            ("* { font-style: wobbly }", "`wobbly` is not a font style"),
            ("* { font-family: \"\" }", "needs a name"),
        ];
        for (stylesheet, expected) in cases {
            let message = problem(stylesheet);
            assert!(message.contains(expected), "{stylesheet}: {message}");
        }
    }

    #[test]
    fn a_property_that_takes_no_side_is_not_one_with_a_side() {
        for stylesheet in [
            "tab { color-left: #fff; }",
            "tab { height-top: 20px; }",
            "tab { border-top-left-radius: 4px; }",
            "tab { font-size-bottom: 12pt; }",
        ] {
            let message = problem(stylesheet);
            assert!(
                message.contains("is not a property"),
                "{stylesheet}: {message}"
            );
        }
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
        assert_eq!(
            problem("clock { color: var(--accent; }"),
            "1:16: this `var(` is never closed with `)`"
        );
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
        // A built-in variable put to a use it does not fit is the fault of
        // the rule that did so.
        let message = problem("\nclock { height: var(--accent); }");
        assert!(
            message.starts_with("2:17: `#5b9cf5` is not a size"),
            "{message}"
        );
    }

    #[test]
    fn variables_that_go_round_in_a_circle_are_refused() {
        let message = problem(":root { --a: var(--b); --b: var(--a); }\nclock { color: var(--a) }");
        assert!(message.contains("stands for itself"), "{message}");
        assert!(message.starts_with("1:"), "{message}");
        // A long way round that does end is no circle.
        let chain = ":root { --a: #f00; --b: var(--a); --c: var(--b); --d: var(--c); }\n\
                     clock { color: var(--d) }";
        assert_eq!(styled(Part::Clock, chain).color, RED);
    }

    #[test]
    fn a_value_that_grows_without_end_is_refused() {
        let long = "x".repeat(600);
        let stylesheet = format!(
            ":root {{ --long: {long}; --twice: var(--long) var(--long); }}\n\
             clock {{ font-family: var(--twice); }}"
        );
        let message = problem(&stylesheet);
        assert!(message.contains("grows too long"), "{message}");
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
        let menu = theme.style(Part::Menu, States::NONE);
        assert_eq!(menu.color, Color::rgb(0, 0, 0));
    }

    #[test]
    fn a_theme_equals_its_copies_and_others_that_work_out_the_same() {
        let theme = Theme::parse("clock { color: #f00 }").unwrap();
        let copy = theme.clone();
        assert_eq!(theme, copy);
        assert_eq!(theme, Theme::parse("clock{color:red;}").unwrap());
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
