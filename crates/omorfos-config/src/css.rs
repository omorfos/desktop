//! Reading a stylesheet: the small part of CSS that themes are written in.
//!
//! A stylesheet is a list of rules. A rule names elements of the desktop
//! as CSS does, by type, class, state and where they are, perhaps nested
//! as CSS Nesting has it, and gives them properties:
//!
//! ```css
//! :root { --accent: #5b9cf5; }
//! window:focus-within > titlebar tab:checked { border-bottom-color: var(--accent); }
//! button { &.close { color: red; } }
//! ```
//!
//! This only takes the text apart. Which elements a selector reaches is
//! for matching to work out (ADR 110), and what the values mean, and which
//! rule wins, for the theme.
//!
//! A stylesheet is untrusted input. Nothing here recurses or allocates
//! beyond the size of the text, and anything not understood is an error
//! with a place in the file, not something passed over in silence.

use crate::Position;

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

/// One element of a selector, as written: a type or `*`, classes,
/// states, and a pseudo-element, `::backdrop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Compound {
    /// Its type, or `None` for `*` or for none written.
    pub(crate) kind: Option<String>,
    pub(crate) classes: Vec<String>,
    /// Its states as written, `:hover`, `:state(urgent)`, `::selection`,
    /// each with how far into the text it is.
    pub(crate) pseudo: Vec<(String, usize)>,
    /// The pseudo-element it names, `backdrop`.
    pub(crate) element: Option<String>,
    /// How far into the text it begins.
    pub(crate) at: usize,
}

/// How one element of a selector is from the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Combinator {
    /// Anywhere within it: `a b`.
    Descendant,
    /// Directly within it: `a > b`.
    Child,
}

/// What a rule applies to: elements, each from the one before it as its
/// combinator says, the last the element styled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Selector {
    pub(crate) first: Compound,
    pub(crate) rest: Vec<(Combinator, Compound)>,
    /// How far into the text it begins.
    pub(crate) at: usize,
}

impl Selector {
    /// Every element of it, the first first.
    pub(crate) fn compounds(&self) -> impl Iterator<Item = &Compound> {
        std::iter::once(&self.first).chain(self.rest.iter().map(|(_, one)| one))
    }

    /// The element it styles: its last.
    pub(crate) fn subject(&self) -> &Compound {
        self.rest.last().map_or(&self.first, |(_, last)| last)
    }
}

/// How deep rules may be nested in rules.
const DEEPEST: usize = 8;

/// The most selectors a rule may come to, once those of the rules it is
/// nested in are put before its own.
const MOST_SELECTORS: usize = 256;

/// The most selectors a stylesheet's rules may come to in all: rules
/// nested in a long list come to as many selectors as it has each, so that
/// a short text could otherwise come to more than the theme could work out
/// in time or hold.
const MOST_IN_ALL: usize = 4096;

/// The states a tree fixed while omorfos runs cannot answer.
const STRUCTURAL: [&str; 13] = [
    "first-child",
    "last-child",
    "only-child",
    "nth-child",
    "nth-last-child",
    "first-of-type",
    "last-of-type",
    "only-of-type",
    "nth-of-type",
    "nth-last-of-type",
    "empty",
    "root",
    "scope",
];

/// A selector as written inside a rule: whether it begins with `&`, or
/// with `>`, which stands for `& >`.
struct Written {
    selector: Selector,
    nested: bool,
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
    /// What `@starting-style` gives: how parts look as they arrive, from
    /// which they change to how they look once there.
    pub(crate) starting: Vec<Rule>,
}

impl Sheet {
    /// Takes a stylesheet apart.
    pub(crate) fn parse(source: &str) -> Result<Self, Fault> {
        let mut reader = Reader {
            source,
            at: 0,
            selectors: 0,
        };
        let mut sheet = Self::default();
        loop {
            reader.skip_blank()?;
            if reader.rest().is_empty() {
                return Ok(sheet);
            }
            if reader.take(":root") {
                let declared = reader.block()?;
                sheet.variables.extend(variables(declared)?);
            } else if reader.rest().starts_with('@') {
                reader.starting(&mut sheet.starting)?;
            } else {
                reader.rule(&mut sheet.rules, false)?;
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
    /// How many selectors its rules have come to so far.
    selectors: usize,
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

    /// Reads a rule, and the rules nested in it, into `rules`: each with
    /// its selectors written out in full, the rule before those nested in
    /// it. In `@starting-style` if `starting`. Nested rules are kept on a
    /// stack of their own rather than read by calling this again, so that
    /// how deep a stylesheet nests costs nothing but that stack.
    fn rule(&mut self, rules: &mut Vec<Rule>, starting: bool) -> Result<(), Fault> {
        let at = self.at;
        let written = self.selectors()?;
        self.count(written.len(), at)?;
        let mut selectors = Vec::with_capacity(written.len());
        for one in written {
            if one.nested {
                let message = "`&` stands for the rule this is nested in, and it is in none";
                return Err(Fault::new(one.selector.at, message));
            }
            selectors.push(one.selector);
        }
        arrives_in_no_state(&selectors, starting)?;
        self.open()?;
        // Each rule open: where its selectors are kept, and its place in
        // `rules`.
        let mut open = vec![rules.len()];
        rules.push(Rule {
            selectors,
            declarations: Vec::new(),
        });
        loop {
            self.skip_blank()?;
            // Never empty here: the loop ends as the outermost rule does.
            let within = open.last().copied().unwrap_or_default();
            if self.take("}") {
                open.pop();
                if open.is_empty() {
                    return Ok(());
                }
                continue;
            }
            if self.rest().starts_with('@') {
                let message = "`@starting-style` is written at the top of a stylesheet, \
                               not within a rule";
                return Err(self.fault(message));
            }
            if !self.nested_next() {
                let declaration = self.declaration()?;
                rules[within].declarations.push(declaration);
                continue;
            }
            if open.len() == DEEPEST {
                let message = format!("rules are nested {DEEPEST} deep at most");
                return Err(self.fault(message));
            }
            let at = self.at;
            let written = self.selectors()?;
            let parents = &rules[within].selectors;
            if written.len().saturating_mul(parents.len()) > MOST_SELECTORS {
                let message = format!(
                    "this rule comes to more than {MOST_SELECTORS} selectors, with those of \
                     the rules it is in"
                );
                return Err(Fault::new(at, message));
            }
            let mut selectors = Vec::new();
            for parent in parents {
                for one in &written {
                    selectors.push(nest(parent, one)?);
                }
            }
            self.count(selectors.len(), at)?;
            arrives_in_no_state(&selectors, starting)?;
            // Read as a rule for the `{` it reaches first, which ends its
            // selectors.
            self.take("{");
            open.push(rules.len());
            rules.push(Rule {
                selectors,
                declarations: Vec::new(),
            });
        }
    }

    /// Counts `more` selectors, of a rule begun at `at`, towards those the
    /// stylesheet may come to.
    fn count(&mut self, more: usize, at: usize) -> Result<(), Fault> {
        self.selectors = self.selectors.saturating_add(more);
        if self.selectors > MOST_IN_ALL {
            let message = format!(
                "the stylesheet comes to more than {MOST_IN_ALL} selectors in all by this rule, \
                 with those of the rules it is in"
            );
            return Err(Fault::new(at, message));
        }
        Ok(())
    }

    /// Steps over the `{` that begins a rule's properties.
    fn open(&mut self) -> Result<(), Fault> {
        self.skip_blank()?;
        if self.take("{") {
            Ok(())
        } else {
            Err(self.fault("expected `{` here, to begin the properties"))
        }
    }

    /// Whether what comes next in a rule is a rule nested in it rather
    /// than a property: a property is a name, then `:` and a blank, as a
    /// state never is, and a nested rule reaches its `{` before a `;` or
    /// a `}`.
    fn nested_next(&self) -> bool {
        let rest = self.rest();
        let named = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
        let after = rest.trim_start_matches(named).trim_start();
        let property = after
            .strip_prefix(':')
            .is_some_and(|value| value.is_empty() || value.starts_with(char::is_whitespace));
        let first = rest.find([';', '{', '}']).and_then(|at| rest.get(at..));
        !property && first.is_some_and(|stop| stop.starts_with('{'))
    }

    /// Reads the selectors of a rule, up to its `{`.
    fn selectors(&mut self) -> Result<Vec<Written>, Fault> {
        let mut selectors = Vec::new();
        loop {
            self.skip_blank()?;
            selectors.push(self.selector()?);
            if !self.take(",") {
                return Ok(selectors);
            }
        }
    }

    /// Reads a selector, and the blanks after it.
    fn selector(&mut self) -> Result<Written, Fault> {
        let at = self.at;
        // Within a rule, one may begin with `>`, which stands for `& >`.
        let leads = self.take(">");
        if leads {
            self.skip_blank()?;
        }
        let nested = leads || self.take("&");
        let first = self.compound(nested && !leads)?;
        let mut rest = Vec::new();
        loop {
            let before = self.at;
            self.skip_blank()?;
            let next = self.rest().chars().next();
            let combinator = match next {
                Some('>') => {
                    self.take(">");
                    self.skip_blank()?;
                    Combinator::Child
                }
                Some(sign @ ('+' | '~')) => {
                    let message = format!(
                        "`{sign}` is not read: the theme's elements are in a tree that has no \
                         order among those side by side"
                    );
                    return Err(self.fault(message));
                }
                Some(',' | '{') | None => break,
                Some(_) if self.at > before => Combinator::Descendant,
                Some(_) => return Err(self.fault("expected `{` here, to begin the properties")),
            };
            rest.push((combinator, self.compound(false)?));
        }
        let selector = Selector { first, rest, at };
        let selector = if leads {
            // `> b` is `& > b`: an empty first element, filled by nesting.
            let empty = Compound {
                kind: None,
                classes: Vec::new(),
                pseudo: Vec::new(),
                element: None,
                at,
            };
            let mut rest = vec![(Combinator::Child, selector.first)];
            rest.extend(selector.rest);
            Selector {
                first: empty,
                rest,
                at,
            }
        } else {
            selector
        };
        Ok(Written { selector, nested })
    }

    /// Reads one element of a selector: a type or `*`, then classes,
    /// states and `::backdrop`, in any order but the last last. After `&`
    /// if `amp`, which may be all there is.
    fn compound(&mut self, amp: bool) -> Result<Compound, Fault> {
        let at = self.at;
        let mut compound = Compound {
            kind: None,
            classes: Vec::new(),
            pseudo: Vec::new(),
            element: None,
            at,
        };
        let starred = self.take("*");
        if !starred {
            let name = self.name();
            compound.kind = (!name.is_empty()).then(|| name.to_owned());
        }
        loop {
            let here = self.at;
            let read = if self.take(".") {
                let class = self.name();
                if class.is_empty() {
                    return Err(Fault::new(here, "expected a class after `.`"));
                }
                compound.classes.push(class.to_owned());
                true
            } else if self.take("::") {
                self.pseudo_element(&mut compound, here)?;
                true
            } else if self.take(":") {
                let state = self.state(here)?;
                compound.pseudo.push((state, here));
                true
            } else if self.rest().starts_with('#') {
                return Err(self.fault("an element has no id to be named by: `#` is not read"));
            } else if self.rest().starts_with('[') {
                return Err(
                    self.fault("an element has no attributes to be named by: `[` is not read")
                );
            } else {
                false
            };
            if !read {
                break;
            }
            if compound.element.is_some() && self.rest().starts_with(['.', ':']) {
                let message = "nothing comes after `::backdrop`: the states of what it is behind \
                               are written before it";
                return Err(self.fault(message));
            }
        }
        let bare = compound.kind.is_none()
            && compound.classes.is_empty()
            && compound.pseudo.is_empty()
            && compound.element.is_none();
        if bare && !starred && !amp {
            return Err(Fault::new(
                at,
                "expected an element here, as `button` or `*`",
            ));
        }
        Ok(compound)
    }

    /// Reads a pseudo-element after its `::`, begun at `here`.
    fn pseudo_element(&mut self, compound: &mut Compound, here: usize) -> Result<(), Fault> {
        match self.name() {
            "backdrop" => compound.element = Some("backdrop".to_owned()),
            "selection" => compound.pseudo.push(("::selection".to_owned(), here)),
            name => {
                let message = format!(
                    "`::{name}` is not understood; there are `::backdrop` and `::selection`"
                );
                return Err(Fault::new(here, message));
            }
        }
        Ok(())
    }

    /// Reads a state after its `:`, begun at `here`, as it is written:
    /// `:hover`, `:state(urgent)`.
    fn state(&mut self, here: usize) -> Result<String, Fault> {
        let name = self.name();
        if name.is_empty() {
            return Err(Fault::new(here, "expected a state after `:`"));
        }
        if !self.take("(") {
            if STRUCTURAL.contains(&name) {
                let message = format!(
                    "`:{name}` is not read: the theme's elements are in a tree fixed as omorfos \
                     runs, which cannot answer it"
                );
                return Err(Fault::new(here, message));
            }
            return Ok(format!(":{name}"));
        }
        if name != "state" {
            let message = format!(
                "`:{name}()` is not read; a state of omorfos's own is written `:state(name)`"
            );
            return Err(Fault::new(here, message));
        }
        let inner = self.name();
        if inner.is_empty() || !self.take(")") {
            return Err(self.fault("expected a name and `)` here, as `:state(urgent)`"));
        }
        Ok(format!(":state({inner})"))
    }

    /// Reads `@starting-style` and the rules it holds, the one at-rule
    /// there is. Its rules are for elements as they arrive, which they do
    /// in no state.
    fn starting(&mut self, rules: &mut Vec<Rule>) -> Result<(), Fault> {
        let start = self.at;
        self.take("@");
        let name = self.name();
        if name != "starting-style" {
            let message = format!(
                "`@{name}` is not understood; the one at-rule there is is `@starting-style`"
            );
            return Err(Fault::new(start, message));
        }
        self.skip_blank()?;
        if !self.take("{") {
            return Err(self.fault("expected `{` here, to begin the rules"));
        }
        loop {
            self.skip_blank()?;
            if self.take("}") {
                return Ok(());
            }
            if self.rest().is_empty() {
                let message = "this `@starting-style` is never closed with `}`";
                return Err(Fault::new(start, message));
            }
            if self.rest().starts_with([':', '@']) {
                let message = "`@starting-style` holds rules for elements, as they arrive";
                return Err(self.fault(message));
            }
            self.rule(rules, true)?;
        }
    }

    /// Reads what is between `{` and `}`: properties and their values.
    fn block(&mut self) -> Result<Vec<Declaration>, Fault> {
        self.open()?;
        let mut declarations = Vec::new();
        loop {
            self.skip_blank()?;
            if self.take("}") {
                return Ok(declarations);
            }
            declarations.push(self.declaration()?);
        }
    }

    /// Reads a property and its value, and the `;` after it, if there is
    /// one before the `}` that ends the rule.
    fn declaration(&mut self) -> Result<Declaration, Fault> {
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
        let declaration = Declaration {
            name: name.to_owned(),
            value: value.to_owned(),
            at,
            offset: self.at,
        };
        self.at += length;
        // The last property of a rule may go without its `;`.
        if !self.take(";") && !self.rest().starts_with('}') {
            return Err(self.fault("expected `;` or `}` here"));
        }
        Ok(declaration)
    }
}

/// In `@starting-style`, that what `selectors` name arrives as it is
/// in no state.
fn arrives_in_no_state(selectors: &[Selector], starting: bool) -> Result<(), Fault> {
    let stated = selectors
        .iter()
        .find(|one| one.compounds().any(|each| !each.pseudo.is_empty()));
    match stated {
        Some(one) if starting => {
            let message = "what arrives is in no state, so a rule in `@starting-style` \
                           names none";
            Err(Fault::new(one.at, message))
        }
        _ => Ok(()),
    }
}

/// `child`, written within a rule for `parent`, written out in full: after
/// `parent` and a blank, or with `parent` where its `&` is.
fn nest(parent: &Selector, child: &Written) -> Result<Selector, Fault> {
    let written = &child.selector;
    if !child.nested {
        let mut rest = parent.rest.clone();
        rest.push((Combinator::Descendant, written.first.clone()));
        rest.extend(written.rest.iter().cloned());
        return Ok(Selector {
            first: parent.first.clone(),
            rest,
            at: written.at,
        });
    }
    // `&` and what is written with it are one element with the parent's
    // last.
    let mut whole = parent.clone();
    whole.at = written.at;
    let last = match whole.rest.last_mut() {
        Some((_, last)) => last,
        None => &mut whole.first,
    };
    let joined = &written.first;
    if last.element.is_some()
        && (joined.kind.is_some() || !joined.classes.is_empty() || !joined.pseudo.is_empty())
    {
        let message = "nothing comes after `::backdrop`: the states of what it is behind are \
                       written before it";
        return Err(Fault::new(joined.at, message));
    }
    if joined.kind.is_some() {
        let message = "`&` is joined to classes and states, not to a type: `& button` is a \
                       button within it";
        return Err(Fault::new(joined.at, message));
    }
    last.classes.extend(joined.classes.iter().cloned());
    last.pseudo.extend(joined.pseudo.iter().cloned());
    if joined.element.is_some() {
        last.element.clone_from(&joined.element);
    }
    whole.rest.extend(written.rest.iter().cloned());
    Ok(whole)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(source: &str) -> (u64, u64, String) {
        let fault = Sheet::parse(source).unwrap_err();
        let at = position(source, fault.offset);
        (at.line, at.column, fault.message)
    }

    /// A selector written back, compound by compound, as matching reads it.
    fn written(selector: &Selector) -> String {
        let one = |compound: &Compound| {
            let mut text = compound.kind.clone().unwrap_or_else(|| "*".to_owned());
            for class in &compound.classes {
                text.push('.');
                text.push_str(class);
            }
            for (state, _) in &compound.pseudo {
                text.push_str(state);
            }
            if let Some(element) = &compound.element {
                text.push_str("::");
                text.push_str(element);
            }
            text
        };
        let mut text = one(&selector.first);
        for (combinator, compound) in &selector.rest {
            text.push_str(match combinator {
                Combinator::Descendant => " ",
                Combinator::Child => " > ",
            });
            text.push_str(&one(compound));
        }
        text
    }

    /// Every rule's selectors, written back, and its properties' names.
    fn read(source: &str) -> Vec<(Vec<String>, Vec<String>)> {
        let sheet = Sheet::parse(source).unwrap();
        sheet
            .rules
            .iter()
            .map(|rule| {
                let selectors = rule.selectors.iter().map(written).collect();
                let names = rule
                    .declarations
                    .iter()
                    .map(|one| one.name.clone())
                    .collect();
                (selectors, names)
            })
            .collect()
    }

    fn strings(all: &[&str]) -> Vec<String> {
        all.iter().map(|one| (*one).to_owned()).collect()
    }

    #[test]
    fn a_selector_is_read_as_compounds_and_combinators() {
        let source = "window:focus-within > titlebar tab.x:checked { color: #fff }";
        let sheet = Sheet::parse(source).unwrap();
        let selector = &sheet.rules[0].selectors[0];
        assert_eq!(selector.first.kind.as_deref(), Some("window"));
        assert_eq!(selector.first.pseudo, [(":focus-within".to_owned(), 6)]);
        let combinators: Vec<Combinator> = selector.rest.iter().map(|(one, _)| *one).collect();
        assert_eq!(combinators, [Combinator::Child, Combinator::Descendant]);
        let tab = selector.subject();
        assert_eq!(tab.kind.as_deref(), Some("tab"));
        assert_eq!(tab.classes, ["x"]);
        assert_eq!(tab.pseudo, [(":checked".to_owned(), 36)]);
        assert_eq!(selector.compounds().count(), 3);
        assert_eq!(
            read("* , launcher:state(closing)::backdrop, input::selection, .close {}")[0].0,
            strings(&[
                "*",
                "launcher:state(closing)::backdrop",
                "input::selection",
                "*.close"
            ])
        );
    }

    #[test]
    fn space_and_comments_between_compounds_change_nothing() {
        let alike = [
            "window>titlebar{}",
            "window > titlebar {}",
            "window  >\n  titlebar\n{}",
            "window /* its bar */ > /* here */ titlebar {}",
        ];
        for source in alike {
            assert_eq!(
                read(source)[0].0,
                strings(&["window > titlebar"]),
                "{source:?}"
            );
        }
        assert_eq!(read("a /* b */ c {}")[0].0, strings(&["a c"]));
    }

    #[test]
    fn a_nested_rule_is_its_parent_and_itself() {
        let rules = read(
            "launcher { color: red; modes { button { color: blue } } height: 4px }\n\
             button { &.close { color: red } &:hover { color: red } & > kbd { color: red } \
             > icon { color: red } & title { color: red } }",
        );
        let selectors: Vec<&str> = rules.iter().map(|(all, _)| all[0].as_str()).collect();
        assert_eq!(
            selectors,
            [
                "launcher",
                "launcher modes",
                "launcher modes button",
                "button",
                "button.close",
                "button:hover",
                "button > kbd",
                "button > icon",
                "button title",
            ]
        );
        // A rule's own properties, before and after what is nested in it,
        // are all its own.
        assert_eq!(rules[0].1, strings(&["color", "height"]));
        assert_eq!(rules[1].1, [] as [String; 0]);
        assert_eq!(
            read("dialog.about { & > button:state(copied) {} }")[1].0,
            strings(&["dialog.about > button:state(copied)"])
        );
        assert_eq!(
            read("launcher modes { &.x {} }")[1].0,
            strings(&["launcher modes.x"])
        );
        assert_eq!(
            read("launcher { &::backdrop {} }")[1].0,
            strings(&["launcher::backdrop"])
        );
    }

    #[test]
    fn a_list_nests_as_every_pairing() {
        assert_eq!(
            read("a, b { c, d { color: red } }")[1].0,
            strings(&["a c", "a d", "b c", "b d"])
        );
        let wide = (0..17)
            .map(|n| format!("x{n}"))
            .collect::<Vec<_>>()
            .join(", ");
        let source = format!("{wide} {{ {wide} {{ }} }}");
        let (_, _, message) = fault(&source);
        assert!(message.contains("more than 256 selectors"), "{message}");
    }

    #[test]
    fn a_stylesheet_comes_to_so_many_selectors_at_most() {
        // Rules nested in a long list each come to as many selectors as it
        // has, which add up however short the text.
        let wide = (0..256)
            .map(|n| format!("x{n}"))
            .collect::<Vec<_>>()
            .join(",");
        let nested = "*{}".repeat(16);
        let (_, _, message) = fault(&format!("{wide} {{ {nested} }}"));
        assert!(
            message.contains("more than 4096 selectors in all"),
            "{message}"
        );
        let flat = vec!["a"; 4097].join(",");
        let (line, column, message) = fault(&format!("{flat} {{ }}"));
        assert!(
            message.contains("more than 4096 selectors in all"),
            "{message}"
        );
        assert_eq!((line, column), (1, 1));
        let enough = vec!["a"; 4096].join(",");
        assert!(Sheet::parse(&format!("{enough} {{ }}")).is_ok());
    }

    #[test]
    fn nesting_goes_eight_deep_and_no_further() {
        let eight = "a { b { c { d { e { f { g { h { } } } } } } } }";
        assert_eq!(read(eight).len(), 8);
        let nine = "a { b { c { d { e { f { g { h { i { } } } } } } } } }";
        let (line, column, message) = fault(nine);
        assert!(message.contains("8 deep at most"), "{message}");
        assert_eq!((line, column), (1, 33));
    }

    #[test]
    fn nesting_is_read_inside_starting_style_and_not_the_other_way() {
        let source = "@starting-style { launcher { &::backdrop { opacity: 0 } } }";
        let sheet = Sheet::parse(source).unwrap();
        let read: Vec<String> = sheet
            .starting
            .iter()
            .map(|rule| written(&rule.selectors[0]))
            .collect();
        assert_eq!(read, ["launcher", "launcher::backdrop"]);
        let (_, column, message) = fault("launcher { @starting-style { } }");
        assert!(message.contains("at the top of a stylesheet"), "{message}");
        assert_eq!(column, 12);
        let (_, _, message) = fault("@starting-style { menu { &:hover { } } }");
        assert!(message.contains("arrives is in no state"), "{message}");
    }

    #[test]
    fn what_this_css_does_not_have_is_an_error() {
        let cases = [
            ("#bar { }", 1, "no id"),
            ("tab[open] { }", 4, "no attributes"),
            ("a + b { }", 3, "`+` is not read"),
            ("a ~ b { }", 3, "`~` is not read"),
            ("a:not(b) { }", 2, "`:not()` is not read"),
            ("a:is(b) { }", 2, "`:is()` is not read"),
            ("a:where(b) { }", 2, "`:where()` is not read"),
            ("item:first-child { }", 5, "`:first-child` is not read"),
            ("item::before { }", 5, "`::before` is not understood"),
            (
                "launcher::backdrop:hover { }",
                19,
                "nothing comes after `::backdrop`",
            ),
            ("a:state(x { }", 10, "expected a name and `)`"),
            ("a: { }", 2, "expected a state after `:`"),
            ("a. { }", 2, "expected a class after `.`"),
            ("& { }", 1, "it is in none"),
            ("> a { }", 1, "it is in none"),
            ("a { &b { } }", 6, "not to a type"),
            (
                "launcher::backdrop { &:hover { } }",
                23,
                "nothing comes after",
            ),
        ];
        for (source, column, expected) in cases {
            let (line, at, message) = fault(source);
            assert!(message.contains(expected), "{source:?}: {message}");
            assert_eq!((line, at), (1, column), "{source:?}");
        }
    }

    #[test]
    fn a_rule_gives_properties_to_the_elements_it_names() {
        let sheet = Sheet::parse("tab { color: #fff; height: 20px }").unwrap();
        assert_eq!(sheet.variables, []);
        assert_eq!(sheet.rules.len(), 1);
        let given = sheet.rules[0].declarations.iter();
        let given: Vec<(&str, &str)> = given
            .map(|one| (one.name.as_str(), one.value.as_str()))
            .collect();
        assert_eq!(given, [("color", "#fff"), ("height", "20px")]);
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
    fn starting_style_holds_how_elements_look_as_they_arrive() {
        let source = "menu { scale: 1 }\n@starting-style {\n  menu, tooltip { scale: 96% }\n  /* and */ notice{opacity:0}\n}\n@starting-style{}";
        let sheet = Sheet::parse(source).unwrap();
        assert_eq!(sheet.rules.len(), 1, "a rule outside it is a rule");
        let read: Vec<Vec<String>> = sheet
            .starting
            .iter()
            .map(|rule| rule.selectors.iter().map(written).collect())
            .collect();
        assert_eq!(read, [strings(&["menu", "tooltip"]), strings(&["notice"])]);
        assert_eq!(sheet.starting[0].declarations[0].value, "96%");
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
            ("tab,\n { }", 2, 2, "expected an element here"),
            ("tab color: #fff;", 1, 10, "expected a state after `:`"),
            ("tab } {", 1, 5, "expected an element here"),
            ("tab /* never", 1, 5, "never closed"),
            ("tab { color #fff; }", 1, 13, "expected `:` after `color`"),
            ("tab { : #fff; }", 1, 7, "expected a property"),
            ("tab { color: ; }", 1, 14, "`color` is given no value"),
            ("tab { color: #fff", 1, 18, "expected `;` or `}`"),
            ("tab { color: #fff { } }", 1, 19, "expected `;` or `}`"),
            ("tab {", 1, 6, "expected a property, or `}`"),
            ("tab { a { ", 1, 11, "expected a property, or `}`"),
            ("tab { > /* never {", 1, 9, "never closed"),
            ("tab > /* never", 1, 7, "never closed"),
            ("tab} {", 1, 4, "expected `{` here"),
            (":root { /* never", 1, 9, "never closed"),
            (":root { --a }", 1, 13, "expected `:` after `--a`"),
            ("tab a", 1, 6, "expected `{` here, to begin the properties"),
            ("@media screen { }", 1, 1, "`@media` is not understood"),
            (
                "@starting-style menu { }",
                1,
                17,
                "expected `{` here, to begin the rules",
            ),
            ("@starting-style {\n  menu { }", 1, 1, "never closed"),
            (
                "@starting-style { :root { } }",
                1,
                19,
                "holds rules for elements",
            ),
            (
                "@starting-style { @starting-style { } }",
                1,
                19,
                "holds rules for elements",
            ),
            (
                "@starting-style { menu:focus { } }",
                1,
                19,
                "arrives is in no state",
            ),
            ("@starting-style /* never", 1, 17, "never closed"),
            ("@starting-style { /* never", 1, 19, "never closed"),
            (
                "@starting-style { menu { color } }",
                1,
                32,
                "expected `:` after `color`",
            ),
        ];
        for (source, line, column, expected) in cases {
            let (at_line, at_column, message) = fault(source);
            assert!(message.contains(expected), "{source:?}: {message}");
            assert_eq!((at_line, at_column), (line, column), "{source:?}");
        }
    }

    #[test]
    fn a_place_is_counted_in_characters_not_bytes() {
        let source = "/* żółw */ # { }";
        assert_eq!(fault(source).1, 12);
        assert_eq!(position("ab", 99), Position { line: 1, column: 3 });
    }
}
