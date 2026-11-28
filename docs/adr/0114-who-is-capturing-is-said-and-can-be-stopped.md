# 114. Who is capturing the screen is said, and can be stopped

- Status: accepted
- Date: 2026-11-28

## Context

A review of the design for accessibility
(`docs/reviews/2026-11-26-accessibility-design-wcag.md`, #180) found that
which program is capturing the screen is told only to a pointer at rest
(finding 6; WCAG 1.4.13, 2.1.1, 4.1.3). The red mark at a screen's corner
says that something is captured, and of what kind; which program, it says
in a tooltip, while the pointer rests on it, and nowhere else: not in the
state a script or a panel reads, not to a key, not in a menu, and not as
it begins. And the roadmap has, among the known gaps of sharing, that the
user cannot stop it from the desktop.

## Decision

**What is captured, by which program and since when, is in the IPC state,
said as it begins, and listed where a key and a press reach it, each
capture of the screen with a way to stop it at once,** as the maintainer
decided on 28 November 2026.

- **The state** has `capturing`: for each program taking pictures of a
  screen or a window, and each recording the microphone, the sound or the
  camera, what it is, its process, what it takes, of which screen, and
  since when, in seconds since the epoch.
- **As sharing begins it is said once,** among what has happened ("firefox
  has begun taking pictures of your screen"), which the notifications show:
  once a program has asked for a second picture, as the line round what
  is shared is drawn, so that a screenshot is not said; and said again only
  once what was said has gone.
- **The list is a menu**, "Screen sharing", a row for each program taking
  pictures, "Stop Firefox", which stops it at once. A press on the mark
  opens it there, as it is let go (ADR 113); the action `show_capturing`
  opens it, for a key anyone binds; and the root menu has a row "Screen
  sharing" opening it, while something is captured. Being a menu, it is
  gone through with the arrows and chosen from with Enter, as every menu
  is.
- **Stopping** is the action `stop_capturing`, with the program's process:
  every picture it is taking is ended, as when what it took goes, and it is
  told so, and so is its following the pointer; it may ask again, and is
  asked about again as any program is.
- **Through the portal**, a browser is shown the screen by the portal,
  which takes the pictures. The list, the notice and the state name the
  browser, as the mark does, from what PipeWire says; and
  `stop_capturing` takes the browser's process as well as the portal's.
  Every share the portal makes comes through one connection, so the
  compositor cannot tell whose a picture is: stopping one browser stops
  every program the portal shares with at that moment, which the row
  names together, "Stop firefox, obs". Stopping one share alone needs the
  portal to say which is whose, which is left for when it is wanted.
- **No new key by default**: ADR 101's review of the defaults stands.

What is not the compositor's to stop, a program recording the microphone,
the sound or the camera, is in the state and on the mark, and not in the
list: PipeWire carries it, not omorfos.

Tooltips, which are told only to a pointer at rest everywhere, staying
while pointed at, going at Escape and shown while the keyboard is on what
they are of, are a decision of their own, after this.

## Alternatives

- **A card of its own** for the list: more room for "since when", and a
  second kind of list to make answer keys and presses as menus do.
- **Stop asking first:** stopping loses nothing; the program may ask again.
- **A default key:** zero configuration for a keyboard alone, against the
  defaults' review; the root menu, on its own key, reaches the list.

## Consequences

- A field added to the IPC state, which a client too old to know it
  passes over (docs/IPC.md), and two actions, each a line added to what
  the schemas promise.
- The roadmap's "the user cannot stop sharing from the desktop" is closed.
