# 105. A titlebar's buttons in their window controls

- Status: accepted
- Date: 2026-11-24
- Takes up: [ADR 97](0097-a-windows-frame-bulkier-glass-buttons-at-the-left.md),
  for how a theme styles the buttons

## Context

ADR 97 has all three buttons of a titlebar show their signs while the
pointer is on any of them, and did it by making all three `:hover`
together. That cost a theme the button under the pointer: nothing could
style it apart from the other two, and `button-close:hover` did not mean
what CSS means by it. The maintainer asked for `:hover` to keep its
meaning, and for the three to be hovered as a group by a part of their
own, and not by giving `:active` another meaning: `:active` is pressed,
in CSS.

## Decision

**The buttons are in a part of their own, `windowcontrols`, named as GTK
names the box that holds them; it is `:hover` while the pointer is on any
of them, and each button is `:hover` only while the pointer is on it. A
rule names the part a button is in before the button, as CSS writes a
part within another: `windowcontrols:hover button-close`.**

- Nothing of `windowcontrols` is drawn. It is `:focus` with its frame and
  `:hover` while the pointer is on any of its buttons, and is there to be
  named before them.
- A part is named within another only where it is in it, as
  `Part::holder` says: today the three buttons in `windowcontrols`. A
  rule names a part, or a part and the one it is in, and no more; any
  other is an error that says which are in another.
- Of two rules naming as many states, counting those of the part named
  before, the one that names that part wins, as CSS ranks a selector
  with more parts in it.
- The built-in theme writes its rules for the buttons with the pointer
  among them as `windowcontrols:hover button-…`, and looks as it did.
- The theme works the looks of a part that is in another out ahead, as
  it does every part's, for each of its states with each of the other's:
  three parts, 32 by 32 looks each. Drawing asks for one, as before, and
  nothing is matched while drawing.

## Alternatives

- **`:active` for the button under the pointer**, with `:hover` left for
  all three. No new part, but `:active` would mean pressed to anyone who
  writes CSS, and here under the pointer.
- **A state of its own for the one under the pointer**, as `:pointed`. A
  name no stylesheet knows, which ADR 99 would have omorfos not invent.
- **Every part in a tree, as GTK has its widgets**, `window > titlebar >
  windowcontrols > button.close`, with classes and the child combinator.
  Read as GTK's or a web page's, and able to style one kind of thing apart
  by where it is. But most of what omorfos draws is not in anything,
  each part's name already says what it is part of, every theme written
  so far would be renamed, and looks would have to be matched as they are
  drawn rather than worked out once. Parts are named within others as the
  need is met, without renaming any; were GTK's names to be taken, it
  would be before the first release, while a rename costs only being
  marked.

## Consequences

- A theme's `button-close:hover` styles the close button with the pointer
  on it alone, where it styled it with the pointer on any of the three:
  a breaking change, marked as one (ADR 73). A theme that wants the old
  look writes `windowcontrols:hover button-close`.
- What is drawn eases from one look to the next when either the button's
  states or its controls' change, as the theme's transition says.
- `windowcontrols` is a part of the theme's list, which a version
  promises (ADR 73).
