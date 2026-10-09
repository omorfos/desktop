# 13. Login session

- Status: accepted
- Date: 2026-10-09

## Context

With the hardware backend omorfos can be the session a user logs in to,
but only by starting it by hand from a text console. A login screen offers
the sessions it finds described in `/usr/share/wayland-sessions`.

Being a whole session is more than drawing the screen. Programs need to
know there is a Wayland session and where, and not every program is
started by the compositor: D-Bus and systemd start some on the user's
behalf, from an environment set up before the compositor existed.

## Decision

**Ship a session entry,** `packaging/omorfos.desktop`, installed by the
packages. It runs `omorfos` with no arguments. No flag is needed to choose
the hardware backend: a login screen starts the session without
`WAYLAND_DISPLAY` or `DISPLAY`, which is what selects it (ADR 10).

**No wrapper script and no session manager.** The compositor is the
session. What a wrapper would do is one step, described next, and the
compositor does it itself, so starting from a login screen and from a
console behave the same.

**Programs are told about the session in one place.**
`Compositor::session_environment` lists what a program needs: where the
compositor and its IPC socket are, that the session is Wayland
(`XDG_SESSION_TYPE`), whose it is (`XDG_CURRENT_DESKTOP`), and the cursor
theme. Programs the compositor starts are given it. Without
`XDG_SESSION_TYPE`, a session started from a console is of type `tty`, and
toolkits that go by it look for an X server that is not there.

**The hardware backend also tells D-Bus and systemd,** by running
`dbus-update-activation-environment --systemd` with the same list, so
that programs they start can find the session. The nested backend does
not: it runs inside someone else's session and would redirect that
session's programs into its window.

**The desktop's name is one constant,** used for `XDG_CURRENT_DESKTOP` and
checked by a test against `DesktopNames` in the entry, since a login
screen sets the one from the other.

## Consequences

- On a system without D-Bus the announcement fails and is reported like
  any program that could not be started. It is harmless there.
- What D-Bus and systemd were told outlives the session. A later session
  of another kind overwrites what it sets, but `OMORFOS_SOCKET` lingers
  until logout ends the user's services.
- There is no `graphical-session.target` integration, so user services
  that wait for it do not start. That needs a session manager's worth of
  care about ordering and shutdown, and is left until something needs it.
- Nothing was started automatically at login at first. `autostart` in
  `config.yaml` now lists programs to start with the session. They are
  started once the backend has its sockets and screens, and never a second
  time: a reload that restarted them would run two of each.
