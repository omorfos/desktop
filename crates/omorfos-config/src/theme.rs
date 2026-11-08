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
use crate::scheme;
use crate::style::CLEAR;
use crate::value::{
    backdrop, color, cubic_bezier, fraction, gradient, image, shadows, size, time, words,
};
use crate::{
    Accent, Bounded, Color, Easing, Edges, Gradient, Part, Position, Scheme, Shadow, States, Style,
    Transition,
};

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
        let table = build(&built_in(Scheme::Dark, Accent::Cyan), &Sheet::default());
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
        Self::parse_in(stylesheet, Scheme::Dark, Accent::Cyan)
    }

    /// The theme that `stylesheet` makes when laid over the built-in one in
    /// `scheme`, picking things out with `accent`. What the stylesheet
    /// says wins over both.
    ///
    /// # Errors
    ///
    /// As [`Theme::parse`].
    pub fn parse_in(stylesheet: &str, scheme: Scheme, accent: Accent) -> Result<Self, ThemeError> {
        let own = Sheet::parse(stylesheet);
        let own = own.map_err(|fault| (Some(fault.offset), fault.message));
        let table = own.and_then(|own| build(&built_in(scheme, accent), &own));
        let theme = table.map(|table| Self {
            table: Arc::new(table),
        });
        theme.map_err(|(offset, message)| ThemeError {
            position: offset.map(|offset| css::position(stylesheet, offset)),
            message,
        })
    }

    /// Whether the theme is dark: whether it writes its menus in a colour
    /// brighter than the one it writes them on. What follows the desktop's
    /// scheme goes by this: the wallpaper, and what applications are told.
    #[must_use]
    pub fn dark(&self) -> bool {
        // How bright a colour looks, weighing green most and blue least as
        // the eye does. Only for telling which of two is the brighter.
        let brightness = |color: Color| {
            2126 * u32::from(color.red)
                + 7152 * u32::from(color.green)
                + 722 * u32::from(color.blue)
        };
        let menu = self.style(Part::Menu, States::NONE);
        brightness(menu.color) > brightness(menu.background)
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

/// The built-in stylesheet in `scheme` with `accent`, taken apart.
fn built_in(scheme: Scheme, accent: Accent) -> Sheet {
    let stylesheet = format!("{BUILT_IN}{}", scheme::sheet(scheme, accent));
    Sheet::parse(&stylesheet).unwrap_or_default()
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
    OutlineColor(Color),
    OutlineWidth(u32),
    OutlineOffset(u32),
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
    Gradient(Option<Gradient>),
    TransitionDuration(u32),
    TransitionDelay(u32),
    TransitionEasing(Easing),
    Shadow(Shadow),
    Inset(Shadow),
    Blur(u32),
}

/// The properties a stylesheet can give, as it is told when it gives
/// another.
const PROPERTIES: &str = "background, background-color, background-image, color, opacity, border, border-color, border-width, \
                          border-radius, outline, outline-color, outline-width, \
                          outline-offset, padding, margin, height, min-width, max-width, \
                          font-family, font-size, font-weight, font-style, transition, \
                          transition-duration, transition-delay, transition-timing-function, \
                          box-shadow and \
                          backdrop-filter; \
                          border, padding and margin may name a side, as in \
                          border-bottom-color";

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
            Setting::Gradient(gradient) => style.gradient = *gradient,
            Setting::Color(color) => style.color = *color,
            Setting::Opacity(share) => *opacity = *share,
            Setting::BorderColor(side, color) => {
                on(&mut style.border, *side, |line| line.color = *color);
            }
            Setting::BorderWidth(side, width) => {
                on(&mut style.border, *side, |line| line.width = *width);
            }
            Setting::OutlineColor(color) => style.outline.color = *color,
            Setting::OutlineWidth(width) => style.outline.width = *width,
            Setting::OutlineOffset(offset) => style.outline_offset = *offset,
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
            Setting::TransitionDelay(millis) => style.transition.delay = *millis,
            Setting::TransitionEasing(easing) => style.transition.easing = *easing,
            Setting::Shadow(shadow) => style.shadow = *shadow,
            Setting::Inset(shadow) => style.inset = *shadow,
            Setting::Blur(blur) => style.blur = *blur,
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
        style.gradient = style.gradient.map(|gradient| {
            gradient.recolored(|mut color| {
                faded(&mut color);
                color
            })
        });
        faded(&mut style.color);
        faded(&mut style.shadow.color);
        faded(&mut style.inset.color);
        on(&mut style.border, None, |line| faded(&mut line.color));
        faded(&mut style.outline.color);
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
        ("background", None) => background(value),
        ("background-color", None) => one(color(value).map(Setting::Background)),
        ("background-image", None) => one(image(value).map(Setting::Gradient)),
        ("color", None) => one(color(value).map(Setting::Color)),
        ("opacity", None) => one(opacity(value)),
        ("border", side) => line(
            value,
            |width| Setting::BorderWidth(side, width),
            |color| Setting::BorderColor(side, color),
        ),
        ("border-color", side) => one(color(value).map(|color| Setting::BorderColor(side, color))),
        ("border-width", side) => {
            one(size(value, "px", 0, 16).map(|width| Setting::BorderWidth(side, width)))
        }
        ("outline", None) => line(value, Setting::OutlineWidth, Setting::OutlineColor),
        ("outline-color", None) => one(color(value).map(Setting::OutlineColor)),
        ("outline-width", None) => one(size(value, "px", 0, 16).map(Setting::OutlineWidth)),
        ("outline-offset", None) => one(size(value, "px", 0, 16).map(Setting::OutlineOffset)),
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
        ("transition-delay", None) => one(lasting(value).map(Setting::TransitionDelay)),
        ("transition-timing-function", None) => one(easing(value).map(Setting::TransitionEasing)),
        ("box-shadow", None) => {
            shadows(value).map(|(cast, inset)| vec![Setting::Shadow(cast), Setting::Inset(inset)])
        }
        ("backdrop-filter", None) => one(backdrop(value).map(Setting::Blur)),
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

/// A border or an outline all at once: a width, `solid` or `none`, and a
/// colour, any of them and in any order, made the settings `width` and
/// `colored` make of each.
fn line(
    value: &str,
    width: impl Fn(u32) -> Setting,
    colored: impl Fn(Color) -> Setting,
) -> Result<Vec<Setting>, String> {
    let understood = words(value).into_iter().map(|word| match word {
        // The only kind of line there is.
        "solid" => Ok(None),
        "none" | "hidden" => Ok(Some(width(0))),
        sized if sized.starts_with(|first: char| first.is_ascii_digit()) => {
            size(sized, "px", 0, 16).map(|sized| Some(width(sized)))
        }
        other => color(other).map(|color| Some(colored(color))),
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

/// What a part is filled with all at once: a colour, a gradient over it,
/// or both. What is left out is none, as in CSS, so a gradient alone has
/// no colour under it.
fn background(value: &str) -> Result<Vec<Setting>, String> {
    let (mut filled, mut over) = (None, None);
    for word in words(value) {
        if word.contains("gradient(") {
            if over.replace(gradient(word)?).is_some() {
                return Err(format!(
                    "`{value}` has a gradient too many; a background has one gradient, over \
                     perhaps one colour"
                ));
            }
        } else if filled.replace(color(word)?).is_some() {
            return Err(format!(
                "`{value}` has a colour too many; a background has one colour, under perhaps \
                 one gradient"
            ));
        }
    }
    Ok(vec![
        Setting::Background(filled.unwrap_or(CLEAR)),
        Setting::Gradient(over),
    ])
}

/// How long a change of colours takes, or waits.
fn lasting(value: &str) -> Result<u32, String> {
    time(value, Transition::LONGEST)
}

/// How a change of colours gathers and loses speed, by its name or by a
/// curve of the theme's own.
fn easing(value: &str) -> Result<Easing, String> {
    let named = Easing::ALL
        .into_iter()
        .find(|easing| easing.name() == value);
    let easing = named.or_else(|| cubic_bezier(value));
    easing.ok_or_else(|| {
        format!(
            "`{value}` is not a way for colours to change; write {EASINGS}, each x from 0 to \
             1 and each y from -10 to 10"
        )
    })
}

/// The ways colours may change, as a theme writes them.
const EASINGS: &str = "linear, ease, ease-in, ease-out, ease-in-out or \
                       cubic-bezier(x1, y1, x2, y2)";

/// A change of colours all at once: how long it takes, and perhaps how
/// long it waits first and how it goes, or `none`. The first time is how
/// long it takes and the second how long it waits, as in CSS, and what is
/// left out is as it would be had nothing been said.
fn transition(value: &str) -> Result<Vec<Setting>, String> {
    let mut change = Transition::NONE;
    let mut given = 0;
    for word in words(value) {
        match word {
            // Every colour of a part changes together.
            "none" | "all" => {}
            timed if timed.starts_with(|first: char| first.is_ascii_digit() || first == '.') => {
                let time = lasting(timed)?;
                match given {
                    0 => change.duration = time,
                    1 => change.delay = time,
                    _ => {
                        return Err(format!(
                            "`{timed}` is one time too many in a transition; write how long the \
                             colours take to change, then perhaps how long they wait first"
                        ));
                    }
                }
                given += 1;
            }
            other => {
                change.easing = easing(other).map_err(|_| {
                    format!(
                        "`{other}` is not part of a transition; write how long the colours of \
                         the part take to change, as in `150ms` or `0.2s`, perhaps how long \
                         they wait first, and perhaps how: {EASINGS}"
                    )
                })?;
            }
        }
    }
    Ok(vec![
        Setting::TransitionDuration(change.duration),
        Setting::TransitionDelay(change.delay),
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
    use crate::scheme::ACCENTS;
    use crate::{Direction, Gradient, Line, Shadow, State, Stop};

    const FOCUSED: States = States::NONE.with(State::Focused);
    const ACTIVE: States = States::NONE.with(State::Active);
    const HOVERED: States = States::NONE.with(State::Hovered);

    const BACKGROUND: Color = Color::rgb(0x0c, 0x12, 0x26);
    const ACCENT: Color = Color::rgb(0x22, 0xd3, 0xee);
    const SURFACE: Color = Color::rgb(0x14, 0x1c, 0x38);
    const SURFACE_DIM: Color = Color::rgb(0x10, 0x17, 0x2f);
    const TEXT: Color = Color::rgb(0xe6, 0xea, 0xf5);
    const TEXT_DIM: Color = Color::rgb(0x9a, 0xa4, 0xc7);
    const BORDER: Color = Color::rgb(0x2b, 0x3a, 0x6b);
    const BORDER_STRONG: Color = Color::rgb(0x74, 0x82, 0xb3);
    const SURFACE_RAISED: Color = Color::rgb(0x24, 0x33, 0x64);
    const ON_ACCENT: Color = BACKGROUND;
    const DANGER: Color = Color::rgb(0xf8, 0x61, 0x6d);
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
    fn a_part_is_filled_with_a_gradient_a_theme_gives_it_over_its_colour() {
        let looks = |stylesheet: &str| {
            let style = styled(Part::Panel, stylesheet);
            (style.background, style.gradient)
        };
        let red = Color::rgb(255, 0, 0);
        let blue = Color::rgb(0, 0, 255);
        let stop = |color, at| Stop { color, at };
        let across = |direction, stops: &[Stop]| Gradient::new(direction, stops);
        let even = [stop(red, 0), stop(blue, 1000)];
        assert_eq!(
            looks("panel { background: linear-gradient(red, blue); }"),
            (CLEAR, across(Direction::Angle(180), &even)),
            "down, as CSS has it unless told, and with no colour under it"
        );
        assert_eq!(
            looks("panel { background: #0f0 linear-gradient(to right, red, blue); }"),
            (Color::rgb(0, 255, 0), across(Direction::Angle(90), &even))
        );
        assert_eq!(
            looks("panel { background: linear-gradient(-45deg, red 20%, rgba(0, 0, 255, .5)); }").1,
            across(
                Direction::Angle(315),
                &[stop(red, 200), stop(Color::rgba(0, 0, 255, 128), 1000)]
            )
        );
        let corner = Direction::Corner {
            right: true,
            down: false,
        };
        assert_eq!(
            looks("panel { background-image: linear-gradient(to top right, red, blue); }"),
            (styled(Part::Panel, "").background, across(corner, &even)),
            "the colour as it was"
        );
        assert_eq!(
            looks("panel { background: linear-gradient(0.25turn, red, #0f0, blue 90%); }").1,
            across(
                Direction::Angle(90),
                &[
                    stop(red, 0),
                    stop(Color::rgb(0, 255, 0), 450),
                    stop(blue, 900)
                ]
            ),
            "a stop not placed halfway between those either side"
        );
        for (side, degrees) in [("top", 0), ("right", 90), ("bottom", 180), ("left", 270)] {
            let sheet = format!("panel {{ background: linear-gradient(to {side}, red, blue); }}");
            assert_eq!(looks(&sheet).1, across(Direction::Angle(degrees), &even));
        }
        let corner = |right, down| across(Direction::Corner { right, down }, &even);
        let sheet = "panel { background: linear-gradient(to left bottom, red, blue); }";
        assert_eq!(looks(sheet).1, corner(false, true), "named in either order");
        let gone = "panel { background: linear-gradient(red, blue); } panel { background: red; }";
        assert_eq!(looks(gone), (red, None), "said all at once, there is none");
        let kept = "panel { background: linear-gradient(red, blue); background-color: red; }";
        assert_eq!(looks(kept), (red, across(Direction::Angle(180), &even)));
        let none = "panel { background: linear-gradient(red, blue); background-image: none; }";
        assert_eq!(looks(none).1, None);
        // As see-through as the part is.
        let faded = looks("panel { background: linear-gradient(red, blue); opacity: 0.5; }");
        let alphas: Vec<u8> = faded
            .1
            .unwrap()
            .stops()
            .iter()
            .map(|stop| stop.color.alpha)
            .collect();
        assert_eq!(alphas, [128, 128]);
    }

    #[test]
    fn a_gradient_that_is_not_understood_is_an_error() {
        for gradient in [
            "linear-gradient(red)",
            "linear-gradient(90deg)",
            "linear-gradient(to middle, red, blue)",
            "linear-gradient(to top bottom, red, blue)",
            "linear-gradient(sideways, red, blue)",
            "linear-gradient(red 50, blue)",
            "linear-gradient(red 120%, blue)",
            "linear-gradient(red 10% 20%, blue)",
            "linear-gradient(red, blue",
            "radial-gradient(red, blue)",
            "linear-gradient(red, red, red, red, red, red, red, red, red)",
            "linear-gradient(xdeg, red, blue)",
            "linear-gradient(xturn, red, blue)",
            "linear-gradient(redd 10%, blue)",
        ] {
            let message = problem(&format!("panel {{ background-image: {gradient}; }}"));
            assert!(
                message.contains("is not a gradient"),
                "{gradient}: {message}"
            );
        }
        let message = problem("panel { background: #000 linear-gradient(red); }");
        assert!(message.contains("is not a gradient"), "{message}");
        let message = problem("panel { background: red blue; }");
        assert!(message.contains("one colour"), "{message}");
        let message =
            problem("panel { background: linear-gradient(red, blue) linear-gradient(red, blue); }");
        assert!(message.contains("one gradient"), "{message}");
        let message = problem("panel { background: sparkly; }");
        assert!(message.contains("`sparkly` is not"), "{message}");
    }

    #[test]
    fn a_part_casts_the_shadow_a_theme_gives_it_or_none() {
        let shadow = |stylesheet: &str| styled(Part::Window, stylesheet).shadow;
        let black = Color::rgb(0, 0, 0);
        let soft = Shadow {
            x: 0,
            y: 8,
            blur: 24,
            spread: 0,
            color: Color::rgba(0, 0, 0, 0x47),
        };
        assert_eq!(shadow("window { box-shadow: 0 8px 24px #00000047; }"), soft);
        assert_eq!(
            shadow("window { box-shadow: 0px 8px 24px 0px rgba(0, 0, 0, 0.28); }"),
            soft
        );
        let sharp = Shadow {
            x: -2,
            y: 3,
            color: black,
            ..Shadow::NONE
        };
        assert_eq!(shadow("window { box-shadow: -2px 3px black; }"), sharp);
        assert_eq!(
            shadow("window { box-shadow: 0 0 4px -2px #000; }").spread,
            -2
        );
        assert_eq!(
            shadow("window { box-shadow: 0 8px 24px black; box-shadow: none; }"),
            Shadow::NONE
        );
        let titlebar = styled(Part::Titlebar, "").shadow;
        assert_eq!(titlebar, Shadow::NONE, "none unless the theme gives one");
        assert!(Shadow::NONE.is_none() && !soft.is_none());
        assert_eq!(
            shadow("window { box-shadow: 0 1px black; opacity: 0.5; }")
                .color
                .alpha,
            0x80,
            "as see-through as the part"
        );
    }

    #[test]
    fn a_part_has_a_line_inside_it_where_a_shadow_inside_falls_from() {
        let both = styled(
            Part::Menu,
            "menu { box-shadow: 0 12px 32px black, inset 0 1px 0 rgba(255, 255, 255, 0.06); }",
        );
        assert_eq!((both.shadow.y, both.shadow.blur), (12, 32));
        let rim = Shadow {
            y: 1,
            color: Color::rgba(255, 255, 255, 15),
            ..Shadow::NONE
        };
        assert_eq!(both.inset, rim);
        // `inset` may be last, and on its own leaves nothing cast.
        let alone = styled(Part::Menu, "menu { box-shadow: 0 1px #ffffff0f inset; }");
        assert_eq!((alone.inset, alone.shadow), (rim, Shadow::NONE));
        // Another `box-shadow` takes the place of both, as in CSS.
        let after = styled(
            Part::Menu,
            "menu { box-shadow: inset 0 1px white; box-shadow: 0 2px black; }",
        );
        assert_eq!(after.inset, Shadow::NONE);
        let faded = styled(
            Part::Menu,
            "menu { box-shadow: inset 0 1px white; opacity: 0.5; }",
        );
        assert_eq!(faded.inset.color.alpha, 0x80);
        assert_eq!(styled(Part::Menu, "").inset, Shadow::NONE, "none built in");
    }

    #[test]
    fn a_shadow_the_theme_cannot_cast_is_said_not_to_be_one() {
        for value in [
            "8px",
            "0 8px 24px",
            "0 100px black",
            "0 8px -1px black",
            "0 8px 24px 1px 2px black",
            "0 8px 24px 40px black",
            "0 8px 24px black, 0 1px red",
            "0 8px 24px nothing",
            "inset 0 1px 2px white",
            "inset 0 1px 0 1px white",
            "inset 0 1px white, inset 0 2px white",
            "inset",
        ] {
            let said = problem(&format!("window {{ box-shadow: {value}; }}"));
            assert!(said.contains("shadow"), "{value}: {said}");
        }
    }

    #[test]
    fn a_part_has_what_is_behind_it_blurred_as_far_as_the_theme_says_or_not_at_all() {
        let blur = |stylesheet: &str| styled(Part::Menu, stylesheet).blur;
        assert_eq!(blur("menu { backdrop-filter: blur(24px); }"), 24);
        assert_eq!(blur("menu { backdrop-filter: blur( 64px ); }"), 64);
        assert_eq!(blur("menu { backdrop-filter: blur(0); }"), 0);
        let none = "menu { backdrop-filter: blur(24px); backdrop-filter: none; }";
        assert_eq!(blur(none), 0);
        assert_eq!(blur(""), 0, "not unless the theme says");
        // Opacity makes the part fainter, not its blur less.
        assert_eq!(
            blur("menu { backdrop-filter: blur(8px); opacity: 0.5; }"),
            8
        );
        for value in [
            "blur(65px)",
            "blur(-1px)",
            "blur(8px",
            "8px",
            "brightness(2)",
        ] {
            let said = problem(&format!("menu {{ backdrop-filter: {value}; }}"));
            assert!(said.contains("backdrop filter"), "{value}: {said}");
        }
    }

    #[test]
    fn colours_change_over_the_time_and_in_the_way_a_theme_says() {
        let change = |stylesheet: &str| styled(Part::Tab, stylesheet).transition;
        let gentle = Transition {
            duration: 150,
            delay: 0,
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
    fn colours_wait_before_they_change_as_long_as_a_theme_says() {
        let change = |stylesheet: &str| styled(Part::Tab, stylesheet).transition;
        let waiting = Transition {
            duration: 150,
            delay: 50,
            easing: Easing::EaseOut,
        };
        assert_eq!(change("tab { transition: 150ms ease-out 50ms; }"), waiting);
        assert_eq!(
            change("tab { transition: all 0.15s .05s ease-out; }"),
            waiting
        );
        assert_eq!(
            change("tab { transition: 150ms ease-out; transition-delay: 50ms; }"),
            waiting
        );
        assert_eq!(
            change("* { transition: 1s 1s; } tab { transition: 150ms ease-out; }"),
            Transition {
                delay: 0,
                ..waiting
            },
            "said all at once, a wait left out is none"
        );
    }

    #[test]
    fn colours_change_along_a_curve_a_theme_gives() {
        let easing = |stylesheet: &str| styled(Part::Tab, stylesheet).transition.easing;
        assert_eq!(
            easing("tab { transition: 200ms cubic-bezier(0.2, 0, 0, 1); }"),
            Easing::CubicBezier {
                x1: 200,
                y1: 0,
                x2: 0,
                y2: 1000
            }
        );
        assert_eq!(
            easing("tab { transition-timing-function: cubic-bezier(.4,-0.5,.6,1.5); }"),
            Easing::CubicBezier {
                x1: 400,
                y1: -500,
                x2: 600,
                y2: 1500
            },
            "beyond its start and end, but not before or after its time"
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
    fn what_comes_onto_the_screen_takes_longer_than_a_colour_and_leaves_quicker() {
        let at = |duration, easing| Transition {
            duration,
            delay: 0,
            easing,
        };
        let (gentle, coming, going) = (
            at(120, Easing::EaseOut),
            at(180, Easing::EaseOut),
            at(120, Easing::EaseIn),
        );
        let comes_and_goes = [
            Part::Window,
            Part::Menu,
            Part::Notice,
            Part::Tooltip,
            Part::Panel,
            Part::Notification,
            Part::Launcher,
            Part::Switcher,
            Part::Question,
        ];
        let closing = States::NONE.with(State::Closing);
        let theme = Theme::default();
        for part in Part::ALL {
            let style = |states| theme.style(part, states).transition;
            if comes_and_goes.contains(&part) {
                assert_eq!(style(States::NONE), coming, "{part:?}");
                assert_eq!(style(closing), going, "{part:?}");
            } else {
                assert_eq!(style(States::NONE), gentle, "{part:?}");
                assert_eq!(style(HOVERED), gentle, "{part:?}");
                assert_eq!(style(closing), gentle, "{part:?}");
            }
        }
        let still = Theme::parse("* { transition: none; }").unwrap();
        for part in Part::ALL {
            assert_eq!(still.style(part, FOCUSED).transition, Transition::NONE);
            assert_eq!(still.style(part, closing).transition, Transition::NONE);
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
        let message = problem("tab { transition: 1s 2s 3s; }");
        assert!(message.contains("`3s` is one time too many"), "{message}");
        let message = problem("tab { transition-delay: 11s; }");
        assert!(message.contains("`11s` is not a time"), "{message}");
        for curve in [
            "cubic-bezier(1.5, 0, 0, 1)",
            "cubic-bezier(0, 0, -0.5, 1)",
            "cubic-bezier(0, 0, 1)",
            "cubic-bezier(0, a, 1, 1)",
            "cubic-bezier(0, 0, 1, 11)",
            "cubic-bezier(0, 0, 1, 1",
        ] {
            let message = problem(&format!("tab {{ transition-timing-function: {curve}; }}"));
            assert!(
                message.contains("not a way for colours to change")
                    && message.contains("cubic-bezier("),
                "{curve}: {message}"
            );
        }
    }

    #[test]
    fn the_built_in_stylesheet_is_sound() {
        let sheet = Sheet::parse(BUILT_IN).unwrap();
        assert!(build(&sheet, &Sheet::default()).is_ok());
        // In either scheme, with any accent.
        for scheme in [Scheme::Dark, Scheme::Light] {
            for (_, accent) in ACCENTS {
                let laid = format!("{BUILT_IN}{}", scheme::sheet(scheme, accent));
                let sheet = Sheet::parse(&laid).unwrap();
                assert!(build(&sheet, &Sheet::default()).is_ok());
            }
        }
        assert_eq!(Theme::parse("").unwrap(), Theme::default());
        assert_eq!(
            Theme::parse("/* nothing of my own */").unwrap(),
            Theme::default()
        );
    }

    #[test]
    fn the_built_in_theme_meets_the_contrast_the_design_language_asks() {
        for scheme in [Scheme::Dark, Scheme::Light] {
            for (name, accent) in ACCENTS {
                let theme = Theme::parse_in("", scheme, accent).unwrap();
                meets_contrast(&theme, &format!("{scheme:?} with {name}"));
            }
        }
    }

    /// Asserts that each text of `theme` is at least 4.5 to 1 against what
    /// it is written on, and each edge that says a state 3 to 1.
    fn meets_contrast(theme: &Theme, which: &str) {
        let style = |part, states| theme.style(part, states);
        let hovered = States::NONE.with(State::Hovered);
        // Text, and what it is written on: the part's own background, or
        // that of the part it is in where its own is clear.
        let written = [
            (style(Part::Titlebar, FOCUSED), None),
            (style(Part::Titlebar, States::NONE), None),
            (style(Part::Tab, FOCUSED.with(State::Active)), None),
            (style(Part::Tab, States::NONE), None),
            (style(Part::Menu, States::NONE), None),
            (style(Part::MenuItem, States::NONE), Some(Part::Menu)),
            (style(Part::MenuItem, hovered), None),
            (style(Part::Notice, States::NONE), None),
            (style(Part::NoticeHeading, States::NONE), Some(Part::Notice)),
            (style(Part::Tooltip, States::NONE), None),
            (style(Part::Panel, States::NONE), None),
            (style(Part::Workspace, States::NONE), Some(Part::Panel)),
            (style(Part::Task, ACTIVE), None),
            (style(Part::Clock, States::NONE), Some(Part::Panel)),
            (style(Part::Launcher, States::NONE), None),
            (style(Part::LauncherInput, States::NONE), None),
            (
                style(Part::LauncherItem, States::NONE),
                Some(Part::Launcher),
            ),
            (style(Part::LauncherItem, ACTIVE), None),
            (
                style(Part::LauncherDescription, States::NONE),
                Some(Part::Launcher),
            ),
            (
                style(Part::LauncherDescription, ACTIVE),
                Some(Part::LauncherItem),
            ),
            (style(Part::Notification, States::NONE), None),
            (
                style(Part::NotificationBody, States::NONE),
                Some(Part::Notification),
            ),
            (style(Part::Question, States::NONE), None),
            (
                style(Part::QuestionDetail, States::NONE),
                Some(Part::Question),
            ),
            (style(Part::QuestionButton, States::NONE), None),
            (style(Part::QuestionButton, ACTIVE), None),
            (style(Part::QuestionInput, States::NONE), None),
            (style(Part::QuestionInput, ACTIVE), None),
            (
                style(Part::SwitcherTitle, States::NONE),
                Some(Part::Switcher),
            ),
        ];
        for (index, (text, within)) in written.into_iter().enumerate() {
            let under = within.map_or(text.background, |part| {
                let states = if part == Part::LauncherItem {
                    ACTIVE
                } else {
                    States::NONE
                };
                style(part, states).background
            });
            let ratio = text.color.contrast(under);
            assert!(ratio >= 4.5, "{which}: text {index}: {ratio:.2} to 1");
        }
        let surface = style(Part::Menu, States::NONE).background;
        let raised = style(Part::MenuItem, hovered).background;
        let background = style(Part::Desktop, States::NONE).background;
        // The edge of what takes input, and what says a state without
        // words, against what it is drawn on.
        let edges = [
            (
                style(Part::QuestionInput, States::NONE).border.top.color,
                surface,
            ),
            (
                style(Part::Tab, FOCUSED.with(State::Active))
                    .border
                    .bottom
                    .color,
                surface,
            ),
            (style(Part::Task, ACTIVE).border.bottom.color, raised),
            (
                style(Part::Picker, States::NONE).border.top.color,
                background,
            ),
            (
                style(Part::QuestionButton, FOCUSED).outline.color,
                style(Part::Question, States::NONE).background,
            ),
        ];
        for (index, (edge, under)) in edges.into_iter().enumerate() {
            let ratio = edge.contrast(under);
            assert!(ratio >= 3.0, "{which}: edge {index}: {ratio:.2} to 1");
        }
    }

    #[test]
    fn a_frame_has_a_hairline_border_with_focus_or_without() {
        let theme = Theme::default();
        let focused = theme.style(Part::Window, FOCUSED);
        assert_eq!(focused.border, Edges::all(line(BORDER, 1)));
        assert_eq!(focused.radius, 12);
        assert_eq!(focused.background, CLEAR, "the window fills it itself");
        let other = theme.style(Part::Window, States::NONE);
        assert_eq!(other.border, Edges::all(line(BORDER, 1)));
        assert_eq!(other.radius, 12);
    }

    #[test]
    fn what_floats_casts_a_shadow_as_deep_as_it_is_high() {
        let theme = Theme::default();
        let shadow = |part, states| theme.style(part, states).shadow;
        let cast = |y, blur, alpha| Shadow {
            y,
            blur,
            color: Color::rgba(0, 0, 0, alpha),
            ..Shadow::NONE
        };
        // DESIGN.md's levels: a window is on the first, half as deep
        // without focus; what floats over windows is on the second.
        assert_eq!(shadow(Part::Window, FOCUSED), cast(8, 24, 0x47));
        assert_eq!(shadow(Part::Window, States::NONE), cast(4, 12, 0x24));
        assert_eq!(shadow(Part::Menu, States::NONE), cast(12, 32, 0x5c));
        for floating in [
            Part::Launcher,
            Part::Notice,
            Part::Notification,
            Part::Switcher,
            Part::Tooltip,
        ] {
            assert_eq!(
                shadow(floating, States::NONE),
                cast(12, 32, 0x5c),
                "{floating:?}"
            );
        }
        for level in [Part::Desktop, Part::Panel, Part::Titlebar, Part::MenuItem] {
            assert_eq!(shadow(level, States::NONE), Shadow::NONE, "{level:?}");
        }
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
        assert_eq!(shown.border.left, line(BORDER, 1));
        assert_eq!((shown.border.top.width, shown.border.right.width), (0, 0));
        assert_eq!(shown.padding.across(), 24);

        // In a frame without focus the line is there but does not stand out.
        let shown = theme.style(Part::Tab, ACTIVE);
        assert_eq!((shown.background, shown.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(shown.border.bottom, line(BORDER, 2));

        let behind = theme.style(Part::Tab, FOCUSED);
        assert_eq!((behind.background, behind.color), (SURFACE_DIM, TEXT_DIM));
        assert_eq!(behind.border.bottom.width, 0);
        assert_eq!(behind.border.left, line(BORDER, 1));
    }

    #[test]
    fn the_menu_row_under_the_pointer_is_raised() {
        let theme = Theme::default();
        let menu = theme.style(Part::Menu, States::NONE);
        assert_eq!((menu.background, menu.color), (SURFACE, TEXT));
        assert_eq!(menu.border, Edges::all(line(BORDER, 1)));
        assert_eq!(menu.radius, 10);
        let row = theme.style(Part::MenuItem, States::NONE);
        assert_eq!((row.background, row.color), (CLEAR, TEXT));
        assert_eq!((row.padding.left, row.padding.right), (12, 12));
        let under = theme.style(Part::MenuItem, HOVERED);
        assert_eq!((under.background, under.color), (SURFACE_RAISED, TEXT));
        assert_eq!((row.height, under.height), (28, 28));
        let separator = theme.style(Part::MenuSeparator, States::NONE);
        assert_eq!(separator.color, BORDER);
    }

    #[test]
    fn the_picker_dims_what_is_not_picked_and_borders_what_is_in_the_accent() {
        let theme = Theme::default();
        let picker = theme.style(Part::Picker, States::NONE);
        assert_eq!(picker.background, Color::rgba(0, 0, 0, 0x66));
        assert_eq!(picker.border, Edges::all(line(ACCENT, 2)));
    }

    #[test]
    fn the_question_of_the_portals_is_a_box_whose_button_that_gives_is_the_accent() {
        let theme = Theme::default();
        let question = theme.style(Part::Question, States::NONE);
        assert_eq!((question.background, question.color), (SURFACE, TEXT));
        assert_eq!(question.border, Edges::all(line(BORDER_STRONG, 1)));
        assert_eq!(question.radius, 12);
        assert_eq!(
            theme.style(Part::QuestionDetail, States::NONE).color,
            TEXT_DIM
        );
        let button = theme.style(Part::QuestionButton, States::NONE);
        let gives = theme.style(Part::QuestionButton, States::NONE.with(State::Active));
        assert_eq!((button.background, button.color), (SURFACE_DIM, TEXT));
        assert_eq!((gives.background, gives.color), (ACCENT, ON_ACCENT));
    }

    #[test]
    fn a_window_offered_is_boxed_with_its_title_brighter_under_the_pointer() {
        let theme = Theme::default();
        let offer = theme.style(Part::Offer, States::NONE);
        let hovered = theme.style(Part::Offer, States::NONE.with(State::Hovered));
        assert_eq!((offer.color, hovered.color), (TEXT_DIM, TEXT));
        assert_eq!(offer.border, Edges::all(line(BORDER, 1)));
    }

    #[test]
    fn the_line_between_windows_is_drawn_as_their_borders_are() {
        let theme = Theme::default();
        let line = theme.style(Part::TileLine, States::NONE);
        assert_eq!(line.color, BORDER);
    }

    #[test]
    fn a_tooltip_is_a_rounded_box_of_text_on_the_surface() {
        let theme = Theme::default();
        let tooltip = theme.style(Part::Tooltip, States::NONE);
        assert_eq!((tooltip.background, tooltip.color), (SURFACE, TEXT));
        assert_eq!(tooltip.border, Edges::all(line(BORDER, 1)));
        assert_eq!(tooltip.radius, 10);
    }

    #[test]
    fn the_notice_of_what_is_wrong_is_outlined_and_headed_in_the_colour_of_danger() {
        let theme = Theme::default();
        let notice = theme.style(Part::Notice, States::NONE);
        assert_eq!((notice.background, notice.color), (SURFACE, TEXT));
        assert_eq!(notice.border, Edges::all(line(DANGER, 1)));
        assert_eq!(notice.padding, Edges::all(12));
        let heading = theme.style(Part::NoticeHeading, States::NONE);
        assert_eq!(heading.color, DANGER);
    }

    #[test]
    fn the_mark_of_capture_is_a_red_dot_on_the_background() {
        let theme = Theme::default();
        let mark = theme.style(Part::Capture, States::NONE);
        assert_eq!(mark.background, BACKGROUND);
        assert_eq!(mark.color, DANGER);
    }

    #[test]
    fn what_is_active_on_the_panel_is_lit_and_underlined() {
        let theme = Theme::default();
        let panel = theme.style(Part::Panel, States::NONE);
        assert_eq!((panel.background, panel.height), (SURFACE, 28));
        assert_eq!(panel.margin, Edges::NONE);
        for part in [Part::Workspace, Part::Task] {
            let button = theme.style(part, States::NONE);
            // Written in the full colour of text, as on thin glass.
            assert_eq!((button.background, button.color), (CLEAR, TEXT));
            assert_eq!(button.border.bottom.width, 0);
            assert_eq!(button.padding.across(), 24);
            let lit = theme.style(part, ACTIVE);
            assert_eq!((lit.background, lit.color), (SURFACE_RAISED, TEXT));
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
    fn a_theme_is_dark_if_its_menus_are_written_light_on_dark() {
        assert!(Theme::default().dark(), "the built-in one");
        let light = Theme::parse("menu { color: #000; background: #fff; }").unwrap();
        assert!(!light.dark());
        // Green counts for more than blue, as it looks brighter.
        let green_on_blue = Theme::parse("menu { color: #00c800; background: #0000ff; }");
        assert!(green_on_blue.unwrap().dark());
        let nearly = Theme::parse("menu { color: #feffff; background: #ffffff; }");
        assert!(!nearly.unwrap().dark(), "only what is brighter is dark");
    }

    #[test]
    fn giving_a_variable_another_value_changes_everything_that_uses_it() {
        let theme = Theme::parse(":root { --accent: #f00; }").unwrap();
        assert_eq!(
            theme
                .style(Part::Tab, FOCUSED.with(State::Active))
                .border
                .bottom
                .color,
            RED
        );
        assert_eq!(theme.style(Part::LauncherItem, ACTIVE).background, RED);
        assert_eq!(
            theme.style(Part::Picker, States::NONE).border.top.color,
            RED
        );
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
               box-shadow: 1px 2px 3px 4px #050505, inset 0 1px #090909;\n\
               backdrop-filter: blur(24px);\n\
               outline: 3px solid #080808;\n\
               outline-offset: 2px;\n\
             }",
        );
        let style = theme.unwrap().style(Part::Task, States::NONE);
        let side = line(Color::rgb(3, 3, 3), 4);
        let expected = Style {
            background: Color::rgb(1, 1, 1),
            gradient: None,
            color: Color::rgb(2, 2, 2),
            border: Edges {
                bottom: line(Color::rgb(6, 6, 6), 7),
                ..Edges::all(side)
            },
            outline: line(Color::rgb(8, 8, 8), 3),
            outline_offset: 2,
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
                delay: 0,
                easing: Easing::Linear,
            },
            shadow: Shadow {
                x: 1,
                y: 2,
                blur: 3,
                spread: 4,
                color: Color::rgb(5, 5, 5),
            },
            inset: Shadow {
                y: 1,
                color: Color::rgb(9, 9, 9),
                ..Shadow::NONE
            },
            blur: 24,
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
    fn an_outline_is_given_all_at_once_or_a_property_at_a_time() {
        let task = styled(Part::Task, "task { outline: 2px solid red; }");
        assert_eq!((task.outline, task.outline_offset), (line(RED, 2), 0));
        let task = styled(
            Part::Task,
            "task { outline: red 3px; outline-offset: 4px; outline-width: 1px; }",
        );
        assert_eq!((task.outline, task.outline_offset), (line(RED, 1), 4));
        let task = styled(
            Part::Task,
            "task { outline-color: red; outline-width: 2px; outline: none; }",
        );
        assert_eq!(task.outline, line(RED, 0));
        assert_eq!(task.border, Edges::all(Line::NONE), "it is no border");

        let message = problem("task { outline-offset: 17px; }");
        assert!(message.contains("from 0px to 16px"), "{message}");
        let message = problem("task { outline-top: 1px solid red; }");
        assert!(message.contains("is not a property"), "{message}");
        let message = problem("task { outline: 1px dashed red; }");
        assert!(message.contains("`dashed` is not a color"), "{message}");
    }

    #[test]
    fn what_the_keyboard_is_on_has_a_ring_of_the_accent_a_little_way_out() {
        let theme = Theme::default();
        let on = States::NONE.with(State::Focused);
        for states in [on, on.with(State::Active)] {
            let button = theme.style(Part::QuestionButton, states);
            assert_eq!(button.outline, line(ACCENT, 2));
            assert_eq!(button.outline_offset, 2);
            assert_eq!(button.border, Edges::all(Line::NONE), "within it, nothing");
        }
        let off = theme.style(Part::QuestionButton, States::NONE);
        assert_eq!(off.outline.width, 0);
    }

    #[test]
    fn opacity_makes_everything_of_a_part_see_through() {
        let theme = Theme::parse(
            "panel { opacity: 0.5; border: 1px solid #fff; outline: 1px solid #fff; }",
        )
        .unwrap();
        let panel = theme.style(Part::Panel, States::NONE);
        assert_eq!(panel.background.alpha, 128);
        assert_eq!(panel.color.alpha, 128);
        assert_eq!(panel.border.top.color, Color::rgba(255, 255, 255, 128));
        assert_eq!(panel.outline.color, Color::rgba(255, 255, 255, 128));
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
            message.starts_with("2:17: `#22d3ee` is not a size"),
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
