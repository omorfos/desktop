//! Reading a stylesheet: the small part of CSS that themes are written in.
//!
//! A stylesheet is a list of rules. A rule names parts of the desktop,
//! perhaps in some state, and gives them properties:
//!
//! ```css
//! :root { --accent: #5b9cf5; }
//! tab:active:focus { border-bottom-color: var(--accent); }
//! ```
//!
//! This only takes the text apart. What the values mean, and which rule
//! wins, is for the theme to work out.
//!
//! A stylesheet is untrusted input. Nothing here recurses or allocates
//! beyond the size of the text, and anything not understood is an error
//! with a place in the file, not something passed over in silence.

use crate::{Part, Position, State, States};

/// Something wrong in a stylesheet, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fault {
    /// How far into the text the problem is, in bytes.
    pub(crate) offset: usize,
    /// What is wrong, written for the user.
    pub(crate) message: String,
}

impl Fault {
    pub(crate) fn new(offset: usize, message: impl Into<String>) -> Self {
        let message = message.into();
        Self { offset, message }
    }
}

/// The line and column of a place `offset` bytes into `source`.
pub(crate) fn position(source: &str, offset: usize) -> Position {
    let before = source.get(..offset).unwrap_or(source);
    let line = before.matches('\n').count() + 1;
    let column = before
        .rsplit('\n')
        .next()
        .map_or(0, |last| last.chars().count())
        + 1;
    Position {
        line: line as u64,
        column: column as u64,
    }
}

/// What a rule applies to: a part, or every part, in at least some states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Selector {
    /// The part, or `None` for `*`, which is every part.
    pub(crate) part: Option<Part>,
    pub(crate) states: States,
}

/// A property and what it was given, as written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Declaration {
    pub(crate) name: String,
    pub(crate) value: String,
    /// Where the property is named, in bytes into the text.
    pub(crate) at: usize,
    /// Where the value is, in bytes into the text.
    pub(crate) offset: usize,
}

/// Selectors and the properties they are given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rule {
    pub(crate) selectors: Vec<Selector>,
    pub(crate) declarations: Vec<Declaration>,
}

/// A stylesheet, taken apart.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Sheet {
    /// What `:root` rules declare: names beginning `--` and their values.
    pub(crate) variables: Vec<Declaration>,
    pub(crate) rules: Vec<Rule>,
}

impl Sheet {
    /// Takes a stylesheet apart.
    pub(crate) fn parse(source: &str) -> Result<Self, Fault> {
        let mut reader = Reader { source, at: 0 };
        let mut sheet = Self::default();
        loop {
            reader.skip_blank()?;
            if reader.rest().is_empty() {
                return Ok(sheet);
            }
            if reader.take(":root") {
                let declared = reader.block()?;
                sheet.variables.extend(variables(declared)?);
            } else {
                let selectors = reader.selectors()?;
                let declarations = reader.block()?;
                sheet.rules.push(Rule {
                    selectors,
                    declarations,
                });
            }
        }
    }
}

/// Checks that everything a `:root` rule declares is a variable.
fn variables(declared: Vec<Declaration>) -> Result<Vec<Declaration>, Fault> {
    let stray = declared.iter().find(|one| !one.name.starts_with("--"));
    match stray {
        Some(stray) => Err(Fault::new(
            stray.offset,
            format!(
                "`:root` only declares variables, whose names begin `--`, not `{}`",
                stray.name
            ),
        )),
        None => Ok(declared),
    }
}

/// A place in the text being read.
struct Reader<'a> {
    source: &'a str,
    at: usize,
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        self.source.get(self.at..).unwrap_or_default()
    }

    fn fault(&self, message: impl Into<String>) -> Fault {
        Fault::new(self.at, message)
    }

    /// Steps over `text` if it is next.
    fn take(&mut self, text: &str) -> bool {
        let found = self.rest().starts_with(text);
        if found {
            self.at += text.len();
        }
        found
    }

    /// Steps over white space and comments.
    fn skip_blank(&mut self) -> Result<(), Fault> {
        loop {
            let rest = self.rest();
            self.at += rest.len() - rest.trim_start().len();
            let start = self.at;
            if !self.take("/*") {
                return Ok(());
            }
            let Some(end) = self.rest().find("*/") else {
                let message = "this comment is never closed with `*/`";
                return Err(Fault::new(start, message));
            };
            self.at += end + 2;
        }
    }

    /// Reads a name: letters, digits, `-` and `_`. Empty if none is next.
    fn name(&mut self) -> &'a str {
        let rest = self.rest();
        let named = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        let length = rest.find(|c| !named(c)).unwrap_or(rest.len());
        self.at += length;
        rest.get(..length).unwrap_or_default()
    }

    /// Reads the selectors of a rule, up to its `{`.
    fn selectors(&mut self) -> Result<Vec<Selector>, Fault> {
        let mut selectors = Vec::new();
        loop {
            self.skip_blank()?;
            selectors.push(self.selector()?);
            self.skip_blank()?;
            if !self.take(",") {
                return Ok(selectors);
            }
        }
    }

    fn selector(&mut self) -> Result<Selector, Fault> {
        let start = self.at;
        let part = if self.take("*") {
            None
        } else {
            let name = self.name();
            let known = Part::ALL.into_iter().find(|part| part.name() == name);
            let unknown = || {
                let names: Vec<&str> = Part::ALL.iter().map(|part| part.name()).collect();
                let message = format!(
                    "`{name}` is not a part of the desktop; the parts are {}",
                    names.join(", ")
                );
                Fault::new(start, message)
            };
            Some(known.ok_or_else(unknown)?)
        };
        let mut states = States::NONE;
        while self.take(":") {
            let start = self.at;
            let name = self.name();
            let known = State::ALL.into_iter().find(|state| state.name() == name);
            let unknown = || {
                let message = format!(
                    "`:{name}` is not a state; the states are :focus, :active, :hover and :urgent"
                );
                Fault::new(start, message)
            };
            states = states.with(known.ok_or_else(unknown)?);
        }
        Ok(Selector { part, states })
    }

    /// Reads what is between `{` and `}`: properties and their values.
    fn block(&mut self) -> Result<Vec<Declaration>, Fault> {
        self.skip_blank()?;
        if !self.take("{") {
            return Err(self.fault("expected `{` here, to begin the properties"));
        }
        let mut declarations = Vec::new();
        loop {
            self.skip_blank()?;
            if self.take("}") {
                return Ok(declarations);
            }
            let at = self.at;
            let name = self.name();
            if name.is_empty() {
                return Err(self.fault("expected a property, or `}` to end the rule"));
            }
            self.skip_blank()?;
            if !self.take(":") {
                return Err(self.fault(format!("expected `:` after `{name}`")));
            }
            self.skip_blank()?;
            let rest = self.rest();
            let length = rest.find([';', '}', '{']).unwrap_or(rest.len());
            let value = rest.get(..length).unwrap_or_default().trim_end();
            if value.is_empty() {
                return Err(self.fault(format!("`{name}` is given no value")));
            }
            declarations.push(Declaration {
                name: name.to_owned(),
                value: value.to_owned(),
                at,
                offset: self.at,
            });
            self.at += length;
            // The last property of a rule may go without its `;`.
            if !self.take(";") && !self.rest().starts_with('}') {
                return Err(self.fault("expected `;` or `}` here"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(source: &str) -> (u64, u64, String) {
        let fault = Sheet::parse(source).unwrap_err();
        let at = position(source, fault.offset);
        (at.line, at.column, fault.message)
    }

    #[test]
    fn a_rule_gives_properties_to_the_parts_it_names() {
        let sheet = Sheet::parse("tab { color: #fff; height: 20px }").unwrap();
        assert_eq!(sheet.variables, []);
        assert_eq!(sheet.rules.len(), 1);
        let rule = &sheet.rules[0];
        let tab = Selector {
            part: Some(Part::Tab),
            states: States::NONE,
        };
        assert_eq!(rule.selectors, [tab]);
        let given = rule.declarations.iter();
        let given: Vec<(&str, &str)> = given
            .map(|one| (one.name.as_str(), one.value.as_str()))
            .collect();
        assert_eq!(given, [("color", "#fff"), ("height", "20px")]);
    }

    #[test]
    fn a_selector_may_name_states_several_parts_or_every_part() {
        let sheet = Sheet::parse("tab:active:focus, menu-item:hover , *{color:#fff;}").unwrap();
        let selectors = &sheet.rules[0].selectors;
        let both = States::NONE.with(State::Active).with(State::Focused);
        assert_eq!(selectors[0].part, Some(Part::Tab));
        assert_eq!(selectors[0].states, both);
        assert_eq!(selectors[1].part, Some(Part::MenuItem));
        assert_eq!(selectors[1].states, States::NONE.with(State::Hovered));
        assert_eq!(
            (selectors[2].part, selectors[2].states),
            (None, States::NONE)
        );
        // A state belongs to the part it is written against.
        assert_eq!(fault("tab {}\n* :focus {}").0, 2);
    }

    #[test]
    fn root_rules_declare_variables() {
        let source = ":root { --accent: #5b9cf5; --text:var(--accent) }\n:root{}";
        let sheet = Sheet::parse(source).unwrap();
        assert_eq!(sheet.rules, []);
        let declared = sheet.variables.iter();
        let declared: Vec<(&str, &str)> = declared
            .map(|one| (one.name.as_str(), one.value.as_str()))
            .collect();
        assert_eq!(
            declared,
            [("--accent", "#5b9cf5"), ("--text", "var(--accent)")]
        );
        let (line, column, message) = fault(":root {\n  color: #fff;\n}");
        assert_eq!((line, column), (2, 10));
        assert!(message.contains("only declares variables"), "{message}");
        assert!(message.contains("`color`"), "{message}");
    }

    #[test]
    fn comments_and_blank_space_may_be_anywhere_between_things() {
        let source = "/* a theme */\n\n tab /* the tabs */ {\n  /* ink */ color /**/ : /**/ #fff ;\n}\n/* end */";
        let sheet = Sheet::parse(source).unwrap();
        assert_eq!(sheet.rules[0].declarations[0].value, "#fff");
        assert_eq!(Sheet::parse("").unwrap(), Sheet::default());
        assert_eq!(Sheet::parse(" \n\t").unwrap(), Sheet::default());
    }

    #[test]
    fn a_value_is_kept_as_written_with_the_place_it_was_written() {
        let source = "tab {\n  font-family: \"DejaVu Sans\" ;\n}";
        let sheet = Sheet::parse(source).unwrap();
        let family = &sheet.rules[0].declarations[0];
        assert_eq!(family.value, "\"DejaVu Sans\"");
        let at = position(source, family.offset);
        assert_eq!((at.line, at.column), (2, 16));
    }

    #[test]
    fn mistakes_are_reported_where_they_are() {
        let cases = [
            ("tab { color: #fff; } /* never ends", 1, 22, "never closed"),
            ("tab {\n  /* nor this", 2, 3, "never closed"),
            ("tab /* or this", 1, 5, "never closed"),
            ("tab { color /* or this", 1, 13, "never closed"),
            ("tab { color: /* or this", 1, 14, "never closed"),
            ("tab, /* or this", 1, 6, "never closed"),
            (":root /* or this", 1, 7, "never closed"),
            ("button { }", 1, 1, "`button` is not a part of the desktop"),
            ("tab,\n { }", 2, 2, "`` is not a part"),
            ("tab:pressed { }", 1, 5, "`:pressed` is not a state"),
            ("tab color: #fff;", 1, 5, "expected `{`"),
            ("tab { color #fff; }", 1, 13, "expected `:` after `color`"),
            ("tab { : #fff; }", 1, 7, "expected a property"),
            ("tab { color: ; }", 1, 14, "`color` is given no value"),
            ("tab { color: #fff", 1, 18, "expected `;` or `}`"),
            ("tab { color: #fff { } }", 1, 19, "expected `;` or `}`"),
            ("tab {", 1, 6, "expected a property, or `}`"),
        ];
        for (source, line, column, expected) in cases {
            let (at_line, at_column, message) = fault(source);
            assert!(message.contains(expected), "{source:?}: {message}");
            assert_eq!((at_line, at_column), (line, column), "{source:?}");
        }
    }

    #[test]
    fn an_unknown_part_is_answered_with_the_ones_there_are() {
        let (_, _, message) = fault("toolbar { }");
        assert!(
            message.contains("notification, notification-title, notification-body"),
            "{message}"
        );
    }

    #[test]
    fn a_place_is_counted_in_characters_not_bytes() {
        let source = "/* żółw */ nope { }";
        assert_eq!(fault(source).1, 12);
        assert_eq!(position("ab", 99), Position { line: 1, column: 3 });
    }
}
