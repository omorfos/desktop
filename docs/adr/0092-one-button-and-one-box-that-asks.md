# 92. One button and one box that asks

- Status: accepted
- Date: 2026-11-15

## Context

Stage 9 begins with what the components each made their own of being made
once, before any of it is offered to others (asked on 15 November 2026).
A review of the buttons and the boxes that ask found seven that drew the
theme's `question-button` each its own way: the leave question and the
welcome card copied the keyboard's ring by hand, the password box and the
portal's question set their words to the left within insets of their
own and lit nothing under the pointer, and the buttons were sized five
ways, fixed at 120 by 32 or 36, or their words with 16 or 20 either side.
The boxes that ask were laid out by constants of their own, 20 round,
their question 24 or 26 tall, ignoring the theme's padding.

## Decision

**`omorfos_ui` draws a question's button (`button`, `button_look`,
`button_size`) and lays out and draws a box that asks
(`question::lay_out`, `paint_question`, `button_at`), and the components
take them up.**

- **A button is as the theme sizes it:** as wide as its words and the
  `padding` of `question-button` either side, held to its `min-width` and
  `max-width`, as tall as its `height`, which the built-in theme sets to
  `0 20px` and `36px`; its words in the middle; whatever has the keyboard
  in `:focus`, which the theme rings, rather than a ring copied onto it.
- **A box is laid out from the theme's `padding` of `question`** (20 in
  the built-in theme): what is asked, the lines under it, a band of the
  asker's own height for a field or the like, and the buttons at the
  bottom right, the one that goes on last, each as large as its words
  make it. Its buttons are a fixed number, so that each has its place
  without indexing.
- Taken up by: the leave question (box and buttons), the portal's
  question (box and buttons, its surface sized and its clicks found from
  the layout), the welcome card's Back and Next, the lock screen's power
  buttons and the password box's buttons.

## What is left apart, and why

- **The password box's layout** (`omorfos-ask`): its size, its buttons'
  places and its field's are public, the compositor sizing its surface
  and clicking in its tests by them, and it is the sealed prompt no
  program may imitate (ADR 48); it is to take up the box, its field in the
  band, as a change of its own. Its buttons are the shared button already.
- **The welcome card's** Back and Next stay 120 wide, so that the dots
  between them do not move as Next becomes Finish.
- **The lock screen's** buttons stand in a row along the bottom of the
  screen, in no box, faded in with it.
- **"About"'s Copy** is `about-button`, and draws a tick as it copies.
- **The panel's** workspaces, windows and clock are a bar's, not a
  question's.
- **The portal's question** has no hover, as it is not drawn again as the
  pointer moves, and its words are English in the code, not translated.

## Consequences

- The lock screen's buttons are 36 tall and 20 either side of their words,
  from 32 and 16; the password box's and the portal's buttons have their
  words in the middle; the portal's box is laid out as the leave
  question's is.
- A theme sizes every question's button, and the room in every box that
  asks, in one place each.
