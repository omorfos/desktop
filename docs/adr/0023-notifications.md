# 23. Notifications are heard over D-Bus with zbus, one message at a time

- Status: accepted
- Date: 2026-10-12

## Context

Programs tell the user things by sending notifications: a download has
finished, a message has arrived. On Linux they send them over D-Bus, by the
desktop notification specification, to whichever program has taken the name
`org.freedesktop.Notifications`. A session with no such program shows none,
and some programs wait a long time for an answer that never comes.
Notifications are the third shell component of ADR 18.

Nothing in omorfos spoke D-Bus. That is a wire protocol with its own
handshake and its own way of writing values down, spoken to a daemon that
drops a peer on its first malformed message.

## Decision

**`omorfos-notifications` is a program of its own**, started with the
session like the panel, unless `notifications.enabled` is off. It is a
Wayland client that puts what it is sent in the top right-hand corner of
the screen the pointer is on, the newest first.

**It speaks D-Bus through `zbus`.** Four ways were weighed: `zbus`,
`rustbus`, the binding to the C library, and a protocol implementation of
our own. Security and correctness come before leanness in this project, and
this is a parser of messages from other programs. `zbus` is safe Rust, is
what most Rust programs on the bus use, and is the likeliest to get on with
every sender. It costs some thirty crates and an executor of its own,
confined to this one program. The C binding would have put C that parses
bus messages in our process and added a build prerequisite; `rustbus` is
small but little used; our own would have had to be proved against every
sender there is.

**Messages are answered one at a time, by name, with no machinery
between.** `zbus` offers a macro that turns a Rust type into a service. It
is not used: what it generates cannot be seen, and its paths cannot all be
reached by a test, which the coverage gate requires. Instead a thread reads
each message and a single `match` answers the four methods of the
specification, `Introspect` and `Ping`, and refuses anything else with the
error the bus has for it. Every answer there is can be read in one place.

**The thread hands what it hears to the rest over a queue and a pipe.** A
method call becomes an event; the pipe makes the program's one event loop
wake, as a descriptor like any other, beside the Wayland connection and the
configuration watcher. Nothing but the answering runs on the thread.

**What is showing is a value.** `Stack` holds the notifications, when each
goes, how they are drawn and what a click does, with the time passed in. It
knows nothing of D-Bus or Wayland, as `Bar` and `Picker` do not.

**What is shown of a notification:** its title, up to three lines of its
body with the markup taken out, and a picture: the pixels it sent, or the
picture whose path it gave, or its program's icon. One marked critical is
`:urgent` in the theme and stays until it is dismissed. Others stay as long
as their sender asks, or `notifications.timeout` seconds. A click dismisses
one, and its sender is told why it went, as the specification asks.

**A click is reported to the sender.** The specification calls what a
click on a notification does its default action. Firefox will not use a
notification service that lacks actions at all: it falls back to drawing
notifications of its own. So the service reports that it has actions, and
a click with the left button on a notification whose sender asked tells it
so before dismissing; the right button dismisses without telling. Buttons
for further actions are not drawn yet, which is the cost of that: a program
may offer some that cannot be pressed.

**Icons come from the desktop's theme as well as the programs' own.**
Notifications name icons such as `dialog-information` that no program
installs. They are looked for by kind in `Adwaita` after `hicolor`, and
where a theme has one only as a shape, the shape is drawn in the colour of
the text beside it, as such icons are meant to be.

**Tests use a real bus.** Each test that needs one starts a `dbus-daemon`
of its own and sends to it as a program would. The daemon is the judge of
whether what is said is well formed, which no stand-in of ours could be.

## Consequences

- `dbus-daemon` is needed to run the tests, in CI and by hand.
- Only one program can have the name. If another notification program is
  running, this one says so and exits; `notifications.enabled: false`
  keeps it from being started.
- A session needs a bus for notifications at all. One started from a login
  manager has one; where there is none, this program says so and exits,
  and nothing else is affected.
- Finding and reading icons moves out of the launcher into
  `omorfos-icons`, and handing a picture to the compositor out of each
  component into `omorfos-shell`, now that three programs need both.
- Themes gain a state, `:urgent`, and three parts: `notification`,
  `notification-title` and `notification-body`.
- A body is broken into lines at its spaces to fit, up to three, keeping
  the breaks its sender made; what is left over is cut short. Buttons for
  actions beyond the click, sounds, pictures that are JPEG, and keeping a
  notification after it has gone are not there.
