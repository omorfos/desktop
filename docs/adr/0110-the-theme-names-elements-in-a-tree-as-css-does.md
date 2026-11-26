# 110. The theme names elements in a tree, as CSS does

- Status: accepted
- Date: 2026-11-26
- Takes up: [ADR 20](0020-themes-in-css.md), for selectors, and
  [ADR 105](0105-a-titlebars-buttons-in-their-window-controls.md), for
  how one part is named within another

## Context

`theme.css` names 59 parts, each flatly, its name saying what it is part
of: `launcher-mode-key`, `button-close`, `popover-day-outside`. ADR 20
chose that, with no tree, no combinators and no inheritance, and ADR 105
added one part named within another, `windowcontrols:hover button-close`.
ADR 105 weighed a tree, as GTK has its widgets, and kept the flat names,
but said that if a tree were to be taken it would be before the first
release, while a rename costs only being marked. 0.1.0 is near (#15), and
after it every rename is a breaking change (ADR 73).

The maintainer asked, on 26 November 2026 (#183), for a theme's syntax
that is simple, readable and familiar, which means names that nest
rather than a flat list:

- The same kind of thing, wherever it is, is one element: a `button` in a
  titlebar, a question, a card or a popover; an `input` in the launcher,
  the lock screen or a box that asks for a password. `button {
  border-radius: 8px; }` rounds every button there is, unless a rule for
  some of them, as `button.close` or `window > titlebar > controls >
  button`, says otherwise.
- `window > button` is a button directly in a window, and `window
  button` one anywhere in it, as CSS has them.
- Familiar to whoever has written CSS for the web, chosen over GTK's own
  names (`headerbar`, `button.titlebutton`), which most of what omorfos
  draws has none of, and which would suggest that GTK's themes work here.

Two ways of building it were weighed against the one taken (below).
Agreed with the maintainer on 26 November 2026, after a review of the
draft by one who would build it and by another of the maintainer's.

## Decision

**Everything omorfos draws is an element in a fixed tree: a type, perhaps
classes, and the element it is in. A selector names elements as CSS does,
by type, class, state and where they are, and is matched against the tree
once, as the theme is loaded.**

### The selectors

- **A type** names every element of it, anywhere: `button`, `input`,
  `item`. `*` names every element.
- **A class** names a kind of an element: `button.close`, `item.top`,
  `day.outside`, `dialog.about`. Classes are omorfos's, given in the tree;
  a theme cannot add one.
- **A state** is a pseudo-class, as today: `:focus`, `:active`, `:hover`,
  `:urgent`, `:closing`. It may be named on any element of a selector,
  not only the last: `window:focus titlebar`, `controls:hover button`.
  Each element has the states it can be in, given in the tree as each
  part's are in its documentation today; a selector naming a state that
  its element can never be in, as `panel:urgent task`, reaches nothing,
  and is an error (below).
- **`::backdrop`**, as CSS has it for a dialog, is what is behind an
  element over the whole screen while it is open: `launcher::backdrop`,
  `switcher::backdrop`, `overview::backdrop`. It has no states of its
  own and is in its element's, written as CSS writes them, before it:
  `launcher:closing::backdrop`, and `@starting-style { launcher::backdrop
  { … } }` for how it comes. It inherits nothing, as in CSS.
- **`:root`** is one element, the one every surface is in, as a
  document's root is in CSS: what is said of it, the font above all, is
  inherited by everything. Variables are declared in it alone, as today,
  and are the same on every surface; one declared in another rule is an
  error, as now.
- **Combinators**: `a b`, a `b` anywhere in an `a`, and `a > b`, a `b`
  directly in an `a`. A rule may list several selectors, `a, b`.
- **Nesting**, as CSS Nesting has it: a rule within a rule is relative to
  it, as though the outer selector and a space came before it, or the
  outer selector stood where `&` is: `launcher { modes { button { … } } }`
  is `launcher modes button`, and `button { &.close { … } &:hover { … } }`
  is `button.close` and `button:hover`. Nesting goes eight deep at most,
  so that the parser stays free of unbounded recursion (ADR 20).
- **Specificity** is CSS's, with no ids: a selector's classes and states
  counted, then its types and its `::backdrop`; `*` and the combinators
  count for nothing. Each selector of a list counts alone, as in CSS, and a
  nested rule counts as the selector it stands for once written out. Of
  two rules for the same element in the same origin, the one counting more
  wins, and then the later.
- **Nothing else**: no ids, no attributes, no `+` or `~`, no `:not()`,
  `:is()` or `:where()`, and no `:first-child` and its kin, which a fixed
  tree cannot answer (below). Each is an error, as anything not
  understood is (ADR 20).

### Origins

The built-in theme is the browser's own stylesheet, and `theme.css` the
page's: a rule of `theme.css` wins over a built-in one however specific
the built-in one is, as today. So `button { border-radius: 8px; }` in
`theme.css` rounds the close button too, though the built-in theme names
`button.close`; among the rules of one stylesheet, specificity decides,
then order. Origins are kept as an ordered list, so that a theme chosen
in the configuration can be put between the two later without changing
what either means, which is a decision of its own.

### Inheritance

`color`, and the font's properties (`font-family`, `font-size`,
`font-weight`, `font-style`), are inherited, as CSS has them: an element
no rule gives a colour has the colour of the element it is in, in the
states that element is in. Everything else, the box, its border and
padding, its background, is not.

As in CSS, a value given to an element, by either origin, wins over one
it would inherit: a description the built-in theme dims stays dim under
`launcher { color: red; }` in `theme.css`, which is said of the launcher,
and takes red only from `launcher item description { color: red; }`. So
the built-in theme says each colour and font once, on what holds them,
`:root` above all, and gives its own to an element only where it is meant
to differ, as a description is dimmer than its name: then a change at the
top reaches everything that is not meant to differ.

A state of an element further out that changes what is inherited, as
`launcher:focus { color: … }` would, counts as a state its elements are
named in, below, as though their own rules named it.

### The tree

Each part of today becomes an element at one path, and each surface's
elements are under one root. A component that more than one program draws
is the same elements wherever it is. Where it is a surface of its own, on
the screen by itself, it is at the root, with a class for whose it is;
where it is drawn within another surface, it is in that surface's
element. So the question of `omorfos-ui`,
which the lock screen, the boxes that ask for a password, the portal's
questions, the welcome card and "About" are drawn with, is a `dialog` at
the root for each of those, which stand alone, but in the lock screen,
which draws it on the `lock` that covers the screen, `lock > dialog`; and
its `input` and `button` are the same elements wherever it is.

| Today | Element, at its path |
|---|---|
| `desktop` | `desktop` |
| `tile-line` | `desktop > separator` |
| `offer` | `desktop > button.offer` |
| `window` | `window` |
| `titlebar` | `window > titlebar` |
| `tab` | `window > titlebar > tab` |
| `windowcontrols` | `window > titlebar > controls` |
| `button-close`, `-maximize`, `-minimize` | `window > titlebar > controls > button.close`, `.maximize`, `.minimize` |
| `menu` | `menu` |
| `menu-item` | `menu > item` |
| `menu-separator` | `menu > separator` |
| `menu-sign` | `menu > item > icon` |
| `menu-keys` | `menu > item > kbd` |
| `notice` | `notice` |
| `notice-heading` | `notice > heading` |
| `capture` | `capture` |
| `capture-border` | `frame.capture`, at the root, as it is round a screen, not in the mark |
| `tooltip` | `tooltip` |
| `panel` | `panel` |
| `workspace` | `panel > workspace` |
| `task` | `panel > task` |
| `clock` | `panel > clock` |
| `popover` | `popover` |
| `popover-item` | `popover > item` |
| `popover-day`, `popover-day-outside` | `popover > calendar > day`, `day.outside` |
| `launcher` | `launcher` |
| `launcher-input` | `launcher > input` |
| `launcher-heading` | `launcher > list > heading` |
| `launcher-item`, `launcher-top` | `launcher > list > item`, `item.top` |
| `launcher-description` | `launcher > list > item > description` |
| `launcher-tile` | `launcher > grid > item` |
| `launcher-modes` | `launcher > modes` |
| `launcher-mode` | `launcher > modes > button` |
| `launcher-mode-key` | `launcher > modes > button > kbd` |
| `launcher-backdrop` | `launcher::backdrop` |
| `notification` | `notification` |
| `notification-title`, `-body` | `notification > title`, `> body` |
| `notification-level` | `notification > meter` |
| (its buttons, drawn as `question-button`) | `notification > button`, alike through the built-in theme's `button`, and apart only where a theme says so |
| `question` | `dialog`, at the root, a class for whose it is: `dialog.question` the portal's, `dialog.password` the boxes that ask for a password, `dialog.welcome` the welcome card; and `lock > dialog`, the lock screen's, in the `lock` that covers the screen |
| `question-detail` | `dialog > detail` |
| `question-button` | `dialog > button` |
| `question-input` | `dialog > input` |
| `switcher` | `switcher` |
| `switcher-item`, `switcher-title` | `switcher > item`, `switcher > item > title` |
| `switcher-window` | `switcher > thumbnail` |
| `switcher-backdrop` | `switcher::backdrop` |
| `overview-window`, `overview-title` | `overview > thumbnail`, `overview > title` |
| `overview-workspace` | `overview > workspace` |
| `overview-add` | `overview > button.add` |
| `overview-backdrop` | `overview::backdrop` |
| `picker` | `picker` |
| `sealed` | `sealed` |
| `about`, `about-heading`, `about-detail`, `about-button` | `dialog.about`, at the root, and `dialog.about > heading`, `> detail`, `> button`: its button the same element as a question's, its own look, as the success colour while it has copied, from `dialog.about > button:active` |

The words are the web's where it has one for the same thing (`button`,
`input`, `kbd`, `meter`, `dialog`, `heading`, `title`, `icon`,
`separator`, `::backdrop`), and omorfos's own only for what the web has
no word for (`launcher`, `panel`, `overview`, `titlebar`). A window's
picture in the switcher and the overview is a `thumbnail`, not a
`window`, so that a rule for windows does not reach pictures of them.

A part added later joins the tree, with its path in the guide's list of
elements; what is added is a line of `promised.txt`, and what is taken
away or moved a breaking change, as for a part today (ADR 73).

### Matched once, as the theme is loaded

The tree is in the code, as the parts are today, and does not change
while omorfos runs. As a theme is loaded, each selector is matched
against each element's path, once; what comes of it, for each element,
is the rules that reach it and the states, of it and of the elements it
is in, that each needs. Its looks are worked out ahead, as today (ADR 20),
for each combination of its own states and of those states of the
elements it is in that some rule reaching it names. Most elements are
named with no state of what they are in, and have as many looks as they
have today; the titlebar's buttons, as since ADR 105, a few times that.

Only states an element can be in are counted, so what a theme can ask
for is bounded by the tree, not by how many states a selector names; and
an element whose looks would come to more than 4,096 is an error, which
says which states of which elements make them, so that no theme can make
the compositor work out more than it can hold. Drawing passes, for each
element, the states of it and of the elements it is in that it can be in.

Drawing asks for an element in its states and in the states of the
elements it is in, and gets a look, as it asks for a part today: nothing
is matched while drawing, and a theme is still shared by reference.

**A selector that reaches no element is an error**, with its line and
column, as an unknown part is today, and it names the elements nearest
to what was meant: `launcher > button` reaches nothing; there are
`launcher > modes > button`. Of the reasons it may reach nothing, the
first found, reading it from the left, is the one said: a type or class
that is in no element (`launcher field`); then a state that no element of its type
can be in (`panel:urgent task`: a panel is in no state); then a
path that is not in the tree. A typo that did nothing would be worse than
one reported (ADR 20). A selector that reaches some elements and not
others is no error: `window button` reaches the titlebar's three.

### The change

There has been no release, so the flat names go, with nothing to read
them still. The built-in theme, its copy in the guide, the guide's list
of parts, which becomes its tree of elements, the manual page and the
schemas' list of what is promised change at once, in the commit that
makes the tree, marked as breaking (ADR 73).

## Alternatives

- **A live tree, matched as it is drawn.** Each surface makes a tree of
  elements as it draws, as a browser has a document, and styles are
  matched against it, remembered by path and states. The most like the
  web, and it could answer `:first-child` and `:nth-child`. But it
  allocates and matches while drawing, against what omorfos puts third,
  no work when nothing changes; every surface would be drawn anew; and a
  selector that reaches nothing could not be told until it was drawn, if
  ever.
- **The new syntax, read into today's parts.** Each selector is turned
  into a part, its states, and the states of one part it is in, as ADR
  105 has it. The least work, but `window:focus titlebar controls
  button`, a state two elements out, could not be said, and the syntax
  would promise what the model could not keep.
- **GTK's names** (`headerbar`, `button.titlebutton.close`, `entry`).
  Familiar to who has themed GTK; but omorfos's launcher, panel, overview
  and switcher have no name in GTK, and GTK's names would suggest that
  its themes work here, which they would not.
- **Flat names, kept** (ADR 105). Nothing to change; but every new part
  is another name to learn, the same button is three names in three
  places, and `button { … }` cannot be said at all.

## Consequences

- One rule styles the same kind of thing everywhere: `button`, `input`,
  `item`; and what is in an element takes its colour and font from it,
  `launcher { color: … }`, but where the built-in theme gives it its own.
- Every theme written so far is renamed, the built-in one among them.
  None has been released.
- What a state of an element further out costs, in looks worked out
  ahead, is paid only where a theme names it. But it is paid by every
  element within, unseen in the rule that names it: `launcher:focus {
  color: … }` doubles the looks of all that is in the launcher, and a
  theme can come to the limit through a rule that is not the one that
  passes it. The error says which states of which elements make the
  looks too many, for that reason.
- Where a part is drawn, it is drawn now as an element at its path, in
  its states and in those of the elements it is in: the code that draws
  says, for each element, where it is.
- `:active` keeps the meaning omorfos gives it, the one of several in use
  or shown, which is not CSS's, pressed; whether the web's own words
  (`:checked`, `[aria-current]`, `[aria-selected]`) should take its place
  is left to a decision of its own.
- `:first-child`, `:nth-child`, siblings and `:not()` can not be had
  while the tree is fixed. A theme that needs them is a reason to weigh
  the live tree again.
