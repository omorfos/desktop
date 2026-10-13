# 27. Programs written for X11 run through Xwayland, as windows like any other

- Status: accepted
- Date: 2026-10-12

## Context

Plenty of programs are still written for X11 and nothing else. Without them
a desktop is not usable day to day, so the roadmap had XWayland on it from
the start, "behind a feature flag".

`Xwayland` is an X server that draws through Wayland. It is a client of the
compositor like any other, and it expects the compositor to be its window
manager: to say where each window goes, which has the keyboard, and to let
windows be shown at all. Smithay has the X side of that, `X11Wm`, and
leaves the decisions to a handler.

## Decision

**A window is a window.** The desktop model already knows nothing of
Wayland (ADR 1). A window of a program written for X11 is added to it with
its class as its application, its title, and its size, and from then on is
placed, framed, focused, tabbed, matched by rules and closed by the same
code. What differs is how the program is told things, and that is one
type, `Role`: an xdg toplevel or an X11 window, with `close`, `arrange` and
`focus` on it. Nothing else in the compositor asks which it is.

**It is framed unless it says it frames itself**, which a program written
for X11 says with the Motif hints.

**The keyboard goes to an X11 window by way of the X server too.** What has
the keyboard is now `Focus`, a surface or an X11 window, since giving an
X11 window the keyboard means telling the X server as well as the surface.
The X server is also told to put that window over its others.

**An X11 window is told where it is, not only how large.** Programs
written for X11 put their own menus on the screen, by where they believe
their window to be.

**Windows that manage themselves are drawn where they put themselves**,
over every window and under what is over everything. That is how X11 does
menus and tooltips. They are no part of the desktop: they are not framed,
focused or listed, and the pointer reaches them.

**A program asks; the desktop decides.** A window is whatever size it asks
for until it joins the desktop, and after that the size the desktop gives
it. Requests to be maximized, fullscreen, moved or resized are honoured as
they are for any window. A resize is by the corner nearest the pointer
whichever edge was named.

**It is a setting, not a build feature.** `xwayland.enabled` is on by
default and is read when the session starts. A feature flag would have made
two programs to test and to package, to save code that costs nothing when
the X server is not running. The X server is started with the session, by
the backend, before the programs to start with the session, which are told
where it is in `DISPLAY` at once: a program that connects before the X
server is ready waits.

**Failing to start it is a problem the user is told of**, and the session
goes on.

**An X server that stops is started again.** The compositor notices that
it is no longer a client, takes its windows off the desktop, and starts
another with the same number, so that a program already told where the X
server is finds the new one. The user is told, because the programs that
were using it have closed: nothing can keep an X11 program alive without
its server. The window manager of the one that stopped is kept until its
connection has said its last, since Smithay goes on handing it events
until then, and only after that is the next one started. An X server that
stops more than three times in a minute is given up on, and the user is
told that too.

**It is started with the session, not when first asked for.** Starting it
on demand means listening on the X sockets and handing them to `Xwayland`
when a program connects. Smithay makes those sockets itself, privately,
and the client data its window manager and the XWayland shell look for
cannot be made outside it. Short of a change there, the only other way is
to carry every byte between a socket of ours and the X server's, for the
life of every program. That is not worth what an idle X server costs.

**It is tested against a real `Xwayland`**, with a program that speaks X11
(`x11rb`, which Smithay already brings in). The tests need `Xwayland`
installed, as they need `dbus-daemon`. The program runs on a thread of its
own while the test steps the compositor, because the X server answers some
requests only after asking the compositor something.

## Consequences

- Programs written for X11 run, and need no special knowledge to use.
- They can read each other's windows and keystrokes. That is X11, and
  nothing here can prevent it. They cannot read those of programs written
  for Wayland. The setting is there for whoever would sooner do without.
- Copy and paste crosses between the two kinds of program, both the
  clipboard and what is selected. What a program written for X11 holds is
  offered by the compositor on its behalf, and what a program written for
  Wayland holds is handed to one written for X11 only while an X11 window
  has the keyboard, which is the rule programs written for Wayland live
  by. Among themselves programs written for X11 share it regardless.
- Smithay's window manager leaves some of what it asks of the X server
  unsent until it next asks something: that a selection is now held is
  one, and new settings are another. Nothing may be asked next for a long
  time, and until then no program written for X11 can paste what was just
  copied. The compositor follows each with a question that has an answer,
  which sends everything. It waits for that answer, as Smithay's own
  handling of what the X server says already waits for others.
- On a screen with a scale, programs written for X11 draw with as many
  pixels as the screen has: see ADR 30.
- Every window that does not manage itself is treated as an ordinary one:
  a dialog is not kept with its parent, and a dock or a splash screen gets
  a frame.
- The X server runs from the start of the session whether or not anything
  uses it.
- For the moment between an X server stopping and the next being ready to
  listen, a program started then is refused.
- The X server that stopped is kept until the instant the next is started,
  because it holds the number the next is to have. The number is free
  between the two for no longer than it takes to ask for it again. The
  tests, which start X servers side by side all the time, keep others from
  starting while one is being replaced; nothing else needs to.
- `Xwayland` is recommended by the packages, not required.
