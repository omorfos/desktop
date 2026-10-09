# 4. Domain model

- Status: accepted
- Date: 2026-10-09

## Context

`omorfos-core` holds window-management policy with no I/O, so it can be
tested without a display. The shape of its model decides how much of the
compositor's behaviour is testable that way, and how the Smithay adapter
talks to it.

## Decision

**The frame is the unit, not the window.** Fluxbox's defining feature is
tabbing: several windows share one position and one is shown. So position,
stacking, workspace, maximized, fullscreen and minimized belong to a tab
group (shown to adapters as a `Frame`), and a lone window is a group of one.
Tabbing is then not a special case anywhere.

**Workspaces span all outputs.** One workspace is shown at a time across
every monitor, as in Fluxbox. Outputs are only rectangles used for placement
and for sizing maximized and fullscreen frames. Per-output workspaces were
rejected as a larger model than the project needs now.

**Adapters call methods and read state back.** `Desktop` exposes commands
(`add_window`, `clicked`, `attach`, ...) and queries (`frames`, `focused`).
It does not emit events. A compositor redraws from state anyway, and the
adapter can diff what it last told each client against `frames()` to decide
which configure events to send. This keeps the domain free of an event
vocabulary that would have to track Wayland's.

**Unknown windows are ignored, not errors.** Wayland is asynchronous, so an
event for a window that has just closed is normal.

**Focus cycles in creation order.** Cycling by stacking order breaks as soon
as focusing raises a frame, and most-recently-used order needs to know when
the modifier is released. Creation order is stable and predictable. When the
focused frame goes away, focus passes to the topmost remaining frame.

**Identifiers are allocated by the domain** and never reused, so a stale
identifier can never refer to a different window.

**Key chains are generic** over the chord and action types. The core does
not define keys or modifiers; the compositor instantiates `KeyChains` with
real keysyms.

**The core does not depend on the configuration crate**, nor the reverse.
Each has its own small enums (`Placement` exists in both); the compositor
maps one to the other. The duplication is a few lines and keeps the domain
free of serialization concerns.

## Consequences

- Rules, placement, focus and tabbing are covered by fast unit tests, plus a
  randomized test that checks the desktop's invariants after thousands of
  arbitrary operations. That test found a real focus bug during development.
- The adapter must do the diffing between domain state and client state.
- Not modelled yet, to be added when the features that need them arrive:
  fullscreen frames stacking above others, always-on-top, usable area
  reduced by panels, and per-frame size constraints.
