# 113. Everything acts as the pointer's button is let go

- Status: accepted
- Date: 2026-11-27
- Takes up: [ADR 111](0111-a-question-is-answered-by-what-was-meant-for-it.md),
  which made it so for the boxes that ask, for everything else

## Context

A review of the design for accessibility
(`docs/reviews/2026-11-26-accessibility-design-wcag.md`, #180) found that
nearly everything acts as the pointer's button goes down (finding 5;
WCAG 2.5.2, level A): a menu's row, a window or a workspace on the bar, a
program in the launcher, a notification and its buttons, the lock
screen's buttons, the overview and the switcher. A hand that shakes, a
touchpad tapped by a palm, or a press that landed a row off cannot be
taken back by sliding away, as it can on a titlebar's buttons and in every
toolkit people know. A notification pressed by mistake is gone, there
being nowhere to read it again; and the theme has no look for pressed, so
nothing shows that a press landed before it is acted on.

The titlebar's buttons already act as the button is let go over the one
it went down on, and ADR 111 made the boxes that ask do so.

## Decision

**What the pointer presses acts as its button is let go over what it went
down on, and is drawn pressed, `:active`, meanwhile,** as the maintainer
decided on 27 November 2026. It is made once, in `omorfos_ui`, as
`Pressing`, and every place that takes a press takes it up.

- **A press marks what is under it,** which the theme draws `:active`, the
  state CSS gives what is being pressed; since ADR 110's first step, what
  is picked or shown is `:checked`, so the name is free.
- **Moved off while held, it is drawn as not pressed, and letting go does
  nothing; moved back on, it is pressed again,** as in GTK and on the web,
  and as the titlebar's buttons are.
- **A menu is picked from by pressing, dragging and letting go,** as every
  desktop's menus are: the press that opens a menu holds it, letting go on
  a row picks the row, and letting go where it opened does nothing, the
  menu staying open to be clicked in.
- **What goes while it is held is forgotten:** a notification whose time
  runs out, a window that closes, a menu taken away. Letting go then acts
  on nothing.

What is left apart: a drag that begins with a press, as a titlebar's or
the launcher field's choosing of what is typed, and two presses counted as
one, which are what they are by the press; and the keyboard, which acts
as it does (ADR 111 has the boxes that ask take keys as they are let go).

## Not yet

As this was first made, on 27 November 2026, what is drawn `:active`
while held is a menu's row, and a notification's and the lock screen's
buttons, by the part every box's button is drawn as. Acting as the button
is let go, but drawn pressed nowhere yet: the overview's and the
switcher's windows and buttons, which are ringed as picked under the
pointer already; the bar's items, which have no look for the pointer at
all; the launcher's rows, lit as chosen under the pointer; the buttons of
the boxes that ask (ADR 111); and "About"'s buttons and the notice. Their
pressed looks go with the looks of the built-in theme being unified.

## Alternatives

- **Each place its own:** what was, and finding 14's and this finding's
  cause: seven places, each pressed its own way.
- **Acting as it goes down, drawn pressed for a moment first:** shows that
  a press landed, and still cannot be taken back.
- **Sliding off cancelling for good:** simpler, and unlike every toolkit;
  a press slid back onto its button is meant.

## Consequences

- A theme can draw what is pressed, `:active`, on every part that can be.
- Press, drag and release in menus, which the root menu and the window
  menu did not have: a right press on the desktop, a drag to a row and
  letting go picks it.
- A test that pressed without letting go no longer acts; tests press and
  let go, as people do.
