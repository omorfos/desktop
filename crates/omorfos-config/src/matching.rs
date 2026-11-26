//! Which elements of the desktop a selector reaches (ADR 110).
//!
//! Every part of the theme is an element at a fixed path in a tree of
//! elements, and says how each of its states is written, and on which
//! element of its path (`Part::path`, `Part::spellings`). A selector is
//! matched against each part's path once, as a theme is loaded: what comes
//! of it is the parts it reaches, each with the states it needs the part,
//! and the part the part is in, to be in. Nothing is matched while drawing.
//!
//! A selector that reaches nothing is an error, with the first reason found
//! reading it from the left: a type or class that no element has; then a
//! state no element of its type can be in; then a path not in the tree, or
//! a state of an element further out than step 1 of ADR 110 reads.

use crate::css::{Combinator, Compound, Fault, Selector};
use crate::{Node, Part, Slot, States};

/// A part a selector reaches, and the states it needs of it and of the
/// part it is in, as [`Part::holder`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Reached {
    pub(crate) part: Part,
    pub(crate) states: States,
    /// The states the part it is in has to be in, if the selector names
    /// any.
    pub(crate) within: Option<States>,
}

/// How a selector weighs against another reaching the same element, as
/// CSS counts it: its classes and states, then its types and its
/// pseudo-element. `*` and the combinators count nothing.
pub(crate) fn specificity(selector: &Selector) -> (u32, u32) {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    selector.compounds().fold((0, 0), |(classes, types), one| {
        let named = count(one.classes.len() + one.pseudo.len());
        let typed = count(usize::from(one.kind.is_some()) + usize::from(one.element.is_some()));
        (classes.saturating_add(named), types.saturating_add(typed))
    })
}

/// Every part `selector` reaches, or why it reaches none.
pub(crate) fn reach(selector: &Selector) -> Result<Vec<Reached>, Fault> {
    let mut reached = Vec::new();
    for part in Part::ALL {
        if let Ok(one) = matched(selector, part) {
            reached.push(one);
        }
    }
    if reached.is_empty() {
        return Err(why(selector));
    }
    Ok(reached)
}

/// Why a part is not reached by a selector whose elements fall on its
/// path: a state written on the element styled that it is never in, or
/// one written on an element further out.
enum Missed {
    /// The elements do not fall on its path.
    Path,
    /// The state, at the place in the text it is written, of compound
    /// `at` of the selector, the last being the one styled.
    State { compound: usize, at: usize },
}

/// Whether `compound` is `node`: its type, its classes among the node's,
/// and the same pseudo-element.
fn is(compound: &Compound, node: &Node) -> bool {
    compound
        .kind
        .as_deref()
        .is_none_or(|kind| kind == node.kind)
        && compound
            .classes
            .iter()
            .all(|class| node.classes.contains(&class.as_str()))
        && compound.element.as_deref() == node.pseudo
}

/// Every way the compounds of `selector` can fall on `path`: for each,
/// which element of the path each compound is, in order, the last the
/// element styled, each `>` between two that are next to each other and
/// each `a b` between two in that order. A compound of no type, as `*`,
/// may fall on more than one element, and which it is may turn on its
/// states, so each way is kept; a path is short, a few elements, so they
/// are few.
fn placements(selector: &Selector, path: &[Node]) -> Vec<Vec<usize>> {
    let compounds: Vec<&Compound> = selector.compounds().collect();
    let combinators: Vec<Combinator> = selector.rest.iter().map(|(one, _)| *one).collect();
    let length = path.len().min(8);
    let fits = |at: &Vec<usize>| {
        let last = at.last() == Some(&length.saturating_sub(1));
        let each = at
            .iter()
            .zip(&compounds)
            .all(|(node, compound)| path.get(*node).is_some_and(|node| is(compound, node)));
        let joined = at.windows(2).zip(&combinators).all(|(pair, combinator)| {
            *combinator == Combinator::Descendant || pair[1] == pair[0] + 1
        });
        last && each && joined
    };
    (0..1_u32 << length)
        .filter(|mask| mask.count_ones() as usize == compounds.len())
        .map(|mask| (0..length).filter(|node| mask >> node & 1 == 1).collect())
        .filter(fits)
        .collect()
}

/// The states `selector` needs of `part`, if it reaches it in some way.
fn matched(selector: &Selector, part: Part) -> Result<Reached, Missed> {
    let ways = placements(selector, part.path());
    let mut missed = Missed::Path;
    for nodes in ways {
        match states_at(selector, part, &nodes) {
            Ok(reached) => return Ok(reached),
            // The first way's, as what is said of it.
            Err(why) if matches!(missed, Missed::Path) => missed = why,
            Err(_) => {}
        }
    }
    Err(missed)
}

/// The states `selector` needs of `part`, its compounds on the elements
/// `nodes` of its path.
fn states_at(selector: &Selector, part: Part, nodes: &[usize]) -> Result<Reached, Missed> {
    let mut states = States::NONE;
    let mut within: Option<States> = None;
    for (index, (compound, node)) in selector.compounds().zip(nodes).enumerate() {
        for (written, at) in &compound.pseudo {
            let spelled = part
                .spellings()
                .iter()
                .find(|one| one.node == *node && one.name == written);
            match spelled.map(|one| one.slot) {
                Some(Slot::Own(state)) => states = states.with(state),
                Some(Slot::Holder(state)) => {
                    within = Some(within.unwrap_or(States::NONE).with(state));
                }
                None => {
                    let at = *at;
                    return Err(Missed::State {
                        compound: index,
                        at,
                    });
                }
            }
        }
    }
    Ok(Reached {
        part,
        states,
        within,
    })
}

/// Why `selector` reaches nothing, the first reason found reading it from
/// the left.
fn why(selector: &Selector) -> Fault {
    let compounds: Vec<&Compound> = selector.compounds().collect();
    for compound in &compounds {
        if let Some(fault) = unknown(compound) {
            return fault;
        }
    }
    for compound in &compounds {
        if let Some(fault) = stateless(compound) {
            return fault;
        }
    }
    let last = compounds.len().saturating_sub(1);
    let further = Part::ALL
        .into_iter()
        .find_map(|part| match matched(selector, part) {
            Err(Missed::State { compound, at }) if compound < last => Some((part, compound, at)),
            _ => None,
        });
    if let Some((part, compound, at)) = further {
        let state = written_at(compounds[compound], at);
        let outer = compounds[compound].kind.as_deref().unwrap_or("*");
        let styled = part.path().last().map_or("", |node| node.kind);
        let message = format!(
            "`{state}` of `{outer}`, an element further out than the one holding `{styled}`, is \
             not read yet (ADR 110, step 2)"
        );
        return Fault::new(at, message);
    }
    let never = Part::ALL
        .into_iter()
        .find_map(|part| match matched(selector, part) {
            Err(Missed::State { at, .. }) => Some((part, at)),
            _ => None,
        });
    if let Some((part, at)) = never {
        let state = written_at(compounds[last], at);
        let message = format!(
            "`{}` is never `{state}`; it can be {}",
            part.selector(),
            states_of(part)
        );
        return Fault::new(at, message);
    }
    let subject = selector.subject();
    // Those of the type styled, those sharing most of the selector's
    // types first.
    let kinds: Vec<&str> = compounds
        .iter()
        .filter_map(|one| one.kind.as_deref())
        .collect();
    let shared = |part: &Part| {
        let path = part.path();
        kinds
            .iter()
            .filter(|kind| path.iter().any(|node| node.kind == **kind))
            .count()
    };
    let mut near: Vec<Part> = Part::ALL
        .into_iter()
        .filter(|part| part.path().last().is_some_and(|node| is(subject, node)))
        .collect();
    near.sort_by_key(|part| std::cmp::Reverse(shared(part)));
    let near: Vec<String> = near
        .into_iter()
        .take(4)
        .map(|part| format!("`{}`", part.selector()))
        .collect();
    let message = if near.is_empty() {
        let kind = subject.kind.as_deref().unwrap_or("*");
        format!("nothing is drawn as a `{kind}` itself: name what is in it")
    } else {
        format!(
            "this reaches no element of the desktop; there are {}",
            near.join(", ")
        )
    };
    Fault::new(selector.at, message)
}

/// The state written at `at` in `compound`.
fn written_at(compound: &Compound, at: usize) -> &str {
    let mut written = compound.pseudo.iter();
    written
        .find(|(_, here)| *here == at)
        .map_or("", |(state, _)| state.as_str())
}

/// How the states of `part` are written, for a message.
fn states_of(part: Part) -> String {
    let path = part.path();
    let written: Vec<String> = part
        .spellings()
        .iter()
        .map(|one| {
            let last = path.len().saturating_sub(1);
            let on = path.get(one.node).map_or("", |node| node.kind);
            if one.node == last {
                format!("`{}`", one.name)
            } else {
                format!("`{on}{}`", one.name)
            }
        })
        .collect();
    // Only said of a part in some state, as an element of its type is.
    written.join(", ")
}

/// Every element of the tree, each once.
fn nodes() -> impl Iterator<Item = &'static Node> {
    Part::ALL.into_iter().flat_map(|part| part.path().iter())
}

/// Why `compound` is no element of the tree, if it is none: a type no
/// element has, a name the theme used to have, or a class no element of
/// its type has.
fn unknown(compound: &Compound) -> Option<Fault> {
    if let Some(kind) = compound.kind.as_deref()
        && !nodes().any(|node| node.kind == kind)
    {
        let former = Part::ALL.into_iter().find(|part| part.former() == kind);
        let message = former.map_or_else(
            || {
                let mut kinds: Vec<&str> = nodes().map(|node| node.kind).collect();
                kinds.sort_unstable();
                kinds.dedup();
                format!(
                    "`{kind}` is no element of the desktop; there are {}",
                    kinds.join(", ")
                )
            },
            |part| format!("`{kind}` is now `{}` (ADR 110)", part.selector()),
        );
        return Some(Fault::new(compound.at, message));
    }
    let mut of_kind = nodes().filter(|node| {
        compound
            .kind
            .as_deref()
            .is_none_or(|kind| kind == node.kind)
            && compound.element.as_deref() == node.pseudo
    });
    let classes = &compound.classes;
    let fits = of_kind.any(|node| {
        classes
            .iter()
            .all(|class| node.classes.contains(&class.as_str()))
    });
    if fits {
        return None;
    }
    let kind = compound.kind.as_deref().unwrap_or("*");
    let message = if classes.is_empty() {
        format!("there is no `{kind}::backdrop`")
    } else {
        format!("no `{kind}` is of the classes `.{}`", classes.join("`, `."))
    };
    Some(Fault::new(compound.at, message))
}

/// A state written of `compound` that no element of its type and classes
/// is ever in, if there is one, said with what it may be instead.
fn stateless(compound: &Compound) -> Option<Fault> {
    for (written, at) in &compound.pseudo {
        let can = Part::ALL.into_iter().any(|part| {
            let path = part.path();
            part.spellings().iter().any(|one| {
                one.name == written.as_str()
                    && path.get(one.node).is_some_and(|node| is(compound, node))
            })
        });
        if can {
            continue;
        }
        let kind = compound.kind.as_deref().unwrap_or("*");
        let hint = match written.as_str() {
            ":active" => " `:active` is pressed, which nothing is drawn as yet; the one picked or \
                          shown is `:checked`."
                .to_owned(),
            ":urgent" | ":closing" => format!(
                " A state of omorfos's own is written `:state({})`.",
                written.trim_start_matches(':')
            ),
            ":focus" => " What has the keyboard within it is `:focus-within`.".to_owned(),
            _ => String::new(),
        };
        let spellings = spellings_of(compound);
        let can_be = if spellings.is_empty() {
            "is in no state".to_owned()
        } else {
            format!("can be {}", spellings.join(", "))
        };
        let message = format!("`{kind}` is never `{written}`: it {can_be}.{hint}");
        return Some(Fault::new(*at, message));
    }
    None
}

/// How the states of every element `compound` is can be written of it.
fn spellings_of(compound: &Compound) -> Vec<String> {
    let mut all: Vec<String> = Part::ALL
        .into_iter()
        .flat_map(|part| {
            let path = part.path();
            part.spellings()
                .iter()
                .filter(move |one| path.get(one.node).is_some_and(|node| is(compound, node)))
                .map(|one| format!("`{}`", one.name))
        })
        .collect();
    all.sort_unstable();
    all.dedup();
    all
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::State;
    use crate::css::Sheet;

    /// The selector of a rule that is `written` and nothing else.
    fn selector(written: &str) -> Selector {
        let sheet = Sheet::parse(&format!("{written} {{}}")).unwrap();
        sheet.rules[0].selectors[0].clone()
    }

    /// The parts `written` reaches.
    fn parts(written: &str) -> Vec<Part> {
        let reached = reach(&selector(written)).unwrap();
        reached.into_iter().map(|one| one.part).collect()
    }

    /// Why `written` reaches nothing.
    fn why_not(written: &str) -> String {
        reach(&selector(written)).unwrap_err().message
    }

    /// The one part `written` reaches, and what it needs of it.
    fn only(written: &str) -> Reached {
        let reached = reach(&selector(written)).unwrap();
        assert_eq!(reached.len(), 1, "{written}: {reached:?}");
        reached[0]
    }

    #[test]
    fn a_type_reaches_every_element_of_it_anywhere() {
        assert_eq!(
            parts("button"),
            [
                Part::Offer,
                Part::ButtonClose,
                Part::ButtonMaximize,
                Part::ButtonMinimize,
                Part::LauncherMode,
                Part::QuestionButton,
                Part::OverviewAdd,
                Part::OverviewClose,
                Part::AboutButton,
            ]
        );
        // Every element but what is behind them over the screen.
        assert_eq!(parts("*").len(), Part::ALL.len() - 3);
        assert_eq!(
            parts("*::backdrop"),
            [
                Part::LauncherBackdrop,
                Part::SwitcherBackdrop,
                Part::OverviewBackdrop
            ]
        );
    }

    #[test]
    fn a_child_combinator_reaches_only_what_is_directly_in() {
        assert!(why_not("launcher > button").contains("`launcher > modes > button`"));
        assert_eq!(parts("launcher button"), [Part::LauncherMode]);
        assert_eq!(parts("launcher > modes > button"), [Part::LauncherMode]);
        assert_eq!(parts("menu > item > kbd"), [Part::MenuKeys]);
        assert_eq!(parts("window controls button.close"), [Part::ButtonClose]);
    }

    #[test]
    fn an_element_of_no_type_is_put_where_the_rest_of_the_selector_lets_it_be() {
        // `*` could be the titlebar or the controls; only the titlebar is
        // directly in the window.
        assert_eq!(parts("window > * button.close"), [Part::ButtonClose]);
        let focused = only("*:focus-within button.close");
        assert_eq!(focused.part, Part::ButtonClose);
        assert_eq!(focused.states, States::NONE.with(State::Focused));
        assert_eq!(parts("window * * button.close"), [Part::ButtonClose]);
        // Tried on the window and the titlebar before the controls.
        assert_eq!(
            only("*:hover button.close").within,
            Some(States::NONE.with(State::Hovered))
        );
        assert!(why_not("window * * * button.close").contains("reaches no element"));
    }

    #[test]
    fn a_class_narrows_a_type() {
        assert_eq!(
            parts("button.close"),
            [Part::ButtonClose, Part::OverviewClose]
        );
        assert_eq!(parts(".close"), [Part::ButtonClose, Part::OverviewClose]);
        assert_eq!(parts("day.outside"), [Part::PopoverDayOutside]);
        assert_eq!(parts("day"), [Part::PopoverDay, Part::PopoverDayOutside]);
        assert!(why_not("button.shut").contains("no `button` is of the classes `.shut`"));
    }

    #[test]
    fn a_state_is_the_state_its_part_spells_it_as() {
        let focused = States::NONE.with(State::Focused);
        let hovered = States::NONE.with(State::Hovered);
        assert_eq!(
            only("window:focus-within > titlebar"),
            Reached {
                part: Part::Titlebar,
                states: focused,
                within: None,
            }
        );
        assert_eq!(
            only("controls:hover button.close"),
            Reached {
                part: Part::ButtonClose,
                states: States::NONE,
                within: Some(hovered),
            }
        );
        let closing = only("launcher:state(closing)::backdrop");
        assert_eq!(closing.part, Part::LauncherBackdrop);
        assert_eq!(closing.states, States::NONE.with(State::Closing));
        let chosen = only("launcher > input::selection");
        assert_eq!(chosen.states, States::NONE.with(State::Active));
        let both = only("window:focus-within titlebar controls:hover button.close:hover");
        assert_eq!(
            (both.states, both.within),
            (focused.with(State::Hovered), Some(hovered))
        );
        assert_eq!(
            only("dialog.question > button:default").part,
            Part::QuestionButton
        );
        assert_eq!(
            only("day.outside:state(today)").states,
            States::NONE.with(State::Active)
        );
    }

    #[test]
    fn a_state_some_elements_of_a_type_have_is_no_error() {
        assert_eq!(parts("button:checked"), [Part::LauncherMode]);
        assert_eq!(parts("item:checked").len(), 4);
        let message = why_not("tab:hover");
        assert!(message.contains("`tab` is never `:hover`"), "{message}");
        assert!(message.contains("`:checked`"), "{message}");
        let message = why_not("window > titlebar > controls > button:checked");
        assert!(message.contains("is never `:checked`"), "{message}");
    }

    #[test]
    fn a_selector_that_reaches_nothing_says_why_first_found_from_the_left() {
        let message = why_not("launcher field");
        assert!(
            message.contains("`field` is no element of the desktop"),
            "{message}"
        );
        assert!(message.contains("kbd"), "{message}");
        let message = why_not("panel:state(urgent) task");
        assert!(
            message.contains("`panel` is never `:state(urgent)`: it can be"),
            "{message}"
        );
        let message = why_not("tooltip:hover");
        assert!(message.contains("`tooltip` is never `:hover`"), "{message}");
        assert!(why_not("sealed:hover").contains("it is in no state"));
        assert_eq!(
            why_not("launcher > list"),
            "nothing is drawn as a `list` itself: name what is in it"
        );
        let message = why_not("launcher > button");
        assert!(
            message.contains("reaches no element of the desktop; there are"),
            "{message}"
        );
        // Unknown before wrong state, wherever each is.
        assert!(why_not("tab:nope field").contains("`field` is no element"));
        assert!(why_not("menu::backdrop").contains("there is no `menu::backdrop`"));
        assert_eq!(
            why_not("notice > separator"),
            "this reaches no element of the desktop; there are `desktop > separator`, \
             `menu > separator`"
        );
        assert_eq!(
            why_not("window > modes"),
            "this reaches no element of the desktop; there are `launcher > modes`"
        );
        let fault = reach(&selector("x { } tab:hover")).unwrap_err();
        assert_eq!(fault.offset, 0);
    }

    #[test]
    fn a_state_further_out_than_step_1_reaches_is_an_error_for_now() {
        assert_eq!(
            why_not("menu:state(closing) item"),
            "`:state(closing)` of `menu`, an element further out than the one holding `item`, \
             is not read yet (ADR 110, step 2)"
        );
    }

    #[test]
    fn an_old_name_is_answered_with_its_new_path() {
        let message = why_not("launcher-mode-key");
        assert_eq!(
            message,
            "`launcher-mode-key` is now `launcher > modes > button > kbd` (ADR 110)"
        );
        assert!(why_not("button-close").contains("`window > titlebar > controls > button.close`"));
        let message = why_not("tab:active");
        assert!(message.contains("`:active` is pressed"), "{message}");
        assert!(message.contains("`:checked`"), "{message}");
        assert!(why_not("notification:urgent").contains("`:state(urgent)`"));
        assert!(why_not("window:closing").contains("`:state(closing)`"));
        assert!(why_not("window:focus").contains("`:focus-within`"));
    }

    #[test]
    fn specificity_is_css_s() {
        assert_eq!(specificity(&selector("*")), (0, 0));
        assert_eq!(specificity(&selector("button")), (0, 1));
        assert_eq!(specificity(&selector("button.close:hover")), (2, 1));
        assert_eq!(
            specificity(&selector("controls:hover button.close")),
            (2, 2)
        );
        assert_eq!(specificity(&selector("launcher::backdrop")), (0, 2));
        assert_eq!(specificity(&selector("*.close > *")), (1, 0));
    }
}
