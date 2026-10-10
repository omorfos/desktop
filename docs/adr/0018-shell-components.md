# 18. Shell components are clients that share the compositor's drawing

- Status: accepted
- Date: 2026-10-10

## Context

The compositor draws what belongs to window management: decorations, the
menu, the pointer. A desktop also needs a panel, a launcher and
notifications. Fluxbox drew its toolbar itself; most Wayland desktops leave
such things to separate programs.

Three questions come with them: where they run, what they draw with, and
how they are themed. The last was raised as whether themes should be
written in CSS and HTML.

## Decision

**Each component is a program of its own.** It puts its surface on the
screen through layer-shell and learns what to show from the IPC socket,
exactly as a third-party panel would. Nothing a component needs is given
to it by a private route. A crash, a hang or a hostile theme in one of them
cannot take the session down, the compositor's handling of untrusted
clients covers them, and any of them can be replaced by another program.
It also keeps the IPC protocol honest: if our own panel cannot be built on
it, nobody else's can.

**They draw with what the compositor draws with.** The canvas, text and
scale move out of the compositor into `omorfos-ui`, which depends only on
`omorfos-config` (for colours and fonts) and on the text library. A
component hands the finished canvas to the compositor as a shared-memory
buffer; the compositor hands its own to its renderer. A window's titlebar
and the panel beside it therefore set text alike, round alike and scale
alike, and one set of pixel tests covers both. No toolkit is taken on.

**A component keeps its decisions apart from its plumbing.** What is on
the panel, where, how it is drawn and what a click asks for is a value
(`Bar`) made from an IPC `State`, with no knowledge of Wayland or sockets.
The client around it only carries: news in, pixels out, clicks back. The
first is tested on its own, the second against a real session.

**Themes will be written in CSS; what is on the screen will not be written
in HTML.** Structure stays in the components, where it can be tested and
kept small. CSS only says how the parts look. It is not adopted yet:

1. The panel is first built on the present `theme.yaml`.
2. Meanwhile the theme is reshaped from one palette into parts with states
   and properties, which is what selectors address.
3. With one working component to prove it on, CSS replaces `theme.yaml`.
4. The launcher and notifications are built against CSS from the start.

**A component is tested where the compositor is.** The compositor's
integration tests have the only harness that runs a real session on one
thread, so the panel's tests as a client live there, and the compositor
takes the panel as a development dependency. The panel never depends on
the compositor, so nothing goes round. Each side is stepped by hand
(`Panel::turn`), which keeps the exchange deterministic; one test runs the
panel as it runs in use, asleep on its descriptors in a thread of its own.

## Consequences

- The compositor depends on `omorfos-ui` for `Canvas`, `Text` and `Scale`,
  and still exports `Scale`, so backends are unaffected.
- A component only knows what the IPC `State` tells it. Whatever the panel
  comes to need (a window's icon, urgency) is an addition to the protocol
  that every other client gains too.
- A workspace button asks for `{"workspace": n}`, which the compositor
  applies to the screen the pointer is on. For a click that is the bar's
  own screen. A component acting without the pointer would need the action
  to name an output.
- The subset of CSS is ours to define: selectors for parts and states, and
  the properties the canvas can draw. The well-known CSS parsers are under
  the MPL, which `deny.toml` refuses, so the parser will be a permissively
  licensed one or our own.
- The IPC client can read without waiting (`Client::try_receive`), which
  any component with an event loop of its own needs. The connection a
  component subscribes on is used for nothing else, so what clicks ask for
  goes down a second one.
- The compositor starts the panel with the session, before the programs
  under `autostart`, unless `panel.enabled` is off. A backend says where
  the panel is: beside the compositor's own executable if there is one
  there, so that a build that was never installed does not start some
  other installed panel, and otherwise by name from `PATH`. The compositor
  does not start it again if it exits.
- The panel shows the time, which neither the compositor nor the standard
  library knows in the user's time zone. `jiff` reads the system's zone
  database; on Linux it brings in one further crate. The time is handed to
  `Panel::turn` by whoever runs it, so the bar and its tests do not depend
  on the clock, and the panel sleeps until the minute turns.
- `theme.yaml` will be retired in a breaking change. Until then it is what
  the panel is themed from, with the colours of the titlebars.
