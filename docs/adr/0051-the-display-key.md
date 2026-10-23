# 51. The display key tries the screens another way for now

- Status: accepted
- Date: 2026-10-23

## Context

A laptop's display key (`XF86Display`) is pressed at a projector, or at a
desk with a monitor, to have the screens show one picture, or another
side by side, or only one of them. It is also what is pressed when a
screen has gone dark and nothing can be seen to put it right. Mirroring
and turning a screen off are in the configuration (ADR 15); the key was
still to be given to them.

## Decision

**The key goes through the ways the screens can stand, in an order fixed
when it is first pressed:** as the configuration says; all showing what
the one that comes first shows; that one alone; each of the others alone;
and round again. Each screen keeps the mode and the turn the
configuration gives it.

**Each way is only tried.** The compositor asks, in the middle of every
screen that is on, whether to keep the screens as they are, with the
seconds left. Enter keeps them, Escape puts them back as they were, and
fifteen seconds without either does too. A way that left the only screen
anyone can see dark comes back by itself. The key pressed again goes on
to the next way, and the way to go back to stays the one before the
first press. While the question is asked, Enter and Escape are its own;
other keys reach programs as ever.

**Nothing is written to the configuration.** What is kept is for the
screens plugged in when it was, until another is plugged in or one taken
out, or the session ends; the configuration's way is followed again
after that. The configuration's way, when kept, is kept by keeping no
other, so that a change to the file is followed at once. Writing what
the key chose into `config.yaml` would change a file the user writes,
behind the user's back (ADR 15 chose arrangements written by the user
for that reason); a setting that remembered it would have the screens
come up some other way at the next login than the file says.

## Consequences

- `next_screens` is an action like any, so the key can be bound
  elsewhere, and `omorfosctl action next_screens` does it from a
  terminal.
- `XF86Display` is bound by default, and is among the defaults to be
  reviewed with the rest.
- The question is drawn with the notice's look; it has no part of its own
  in the theme yet.
- A change to the screens made from Settings (stage 7) is to be kept the
  same way.
