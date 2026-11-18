# 98. Which programs stand alone

- Status: accepted
- Date: 2026-11-18

## Context

omorfos is a compositor and the programs round it: those of the shell
(ADR 18), and the applications of stage 7. Some of them would be of use
to someone who runs another compositor; the maintainer does not want
omorfos to be one more desktop that can only be had whole, and went
through every program that exists and every one that is planned, asking
of each whether it should work on a machine where omorfos is not
installed.

What decides it is less the will than what a program stands on.

- **An application is an ordinary window.** It asks for nothing a
  compositor does not have, and runs on any of them.
- **A part of the shell is not.** It puts its surface on the screen
  through layer-shell, locks the session through `ext-session-lock`,
  reads the clipboard through `ext-data-control`, lists windows through
  `ext-foreign-toplevel-list`. Sway, Hyprland, niri, labwc and their
  like offer these, and KDE most of them. GNOME offers none of the
  three first: nothing of a shell can be put on it from outside.
- **Some of what a program of ours shows is the compositor's.** Glass
  and shadows are drawn by omorfos under each part (ADR 33), a window's
  picture in a dock is lent by it (ADR 38), and what asks for a secret
  is sealed by it (ADR 37). On another compositor a program has none of
  these: it is solid, casts no shadow unless it draws one, and is one
  surface among the others.
- **Some talk to omorfos alone.** The portal's backend turns what
  programs ask into what omorfos is asked, as every desktop's does for
  its own; recording is done by the compositor and only asked for.

## Decision

A program **stands alone** if it is built, packaged and tested to run
where omorfos is not installed: it finds its theme and its settings
without omorfos's, does without what only omorfos gives it, and says so
nowhere but in what it then looks like. The others are omorfos's own,
and may lean on it as they like.

### On any desktop

Ordinary windows, each small and doing one thing: the **task manager**,
which needs nothing of a compositor at all and is to have a terminal's
face as well as a window's; **Files**; the **image viewer**; the
**calculator**; the **player**; the **camera**; the **sound recorder**;
the **Markdown viewer**; the **text editor**.

**The greeter** (stage 8) is here too. It asks who is logging in and
starts the session they choose, GNOME's and KDE's as much as omorfos's:
which desktop is started is nothing to it. It is a greeter for `greetd`
and needs something to draw on, which is omorfos or any small
compositor that will show one window.

**Settings**, of which more below: where omorfos is not installed it
is the settings of whichever of these programs are.

**`omorfos-pinentry`**, which is what GnuPG asks for a passphrase. It
is built beside `omorfos-ask` and draws the same box. Where the
compositor is not omorfos the box is not sealed: it is an ordinary
pinentry, drawn as ours is.

### On compositors that offer what a shell needs

**The lock screen**, which already locks through the standard protocol;
**the wallpaper**; **the dock**, without the pictures of windows omorfos
lends it, and raising and closing windows only where the compositor
lets a program; **Launchpad**; **the on-screen keyboard**, which is a
layer surface that does not take the keyboard and types through the
virtual keyboard's protocol, so that the window typed into keeps the
focus; and **`omorfos-clipboard`**, which keeps what was copied after
its program has gone, by the standard protocol alone.

**The launcher**, with them, and on GNOME too if it is worth the second
way of being shown: where there is no layer-shell it can open as an
ordinary window, as other launchers do there, which cannot dim the
screen, be put where it likes, or be sure of the keyboard. That is
decided when it is built, by trying it.

### omorfos's own

**The bar**, which `omorfos-panel` is today and which the bar and the
dock of stage 5 take the place of; **notifications**; **the welcome
card**; **`omorfos-ask`**, sealed; **`omorfos-screenshot`** and
**`omorfos-recording`**, and the one program with a window that stage 7
makes of them; **the portal's backend**; a clipboard with a history,
until it is wanted; and the compositor's own, `omorfos-hold`,
`omorfos-relay`, `omorfos-session` and `omorfosctl`.

### Settings

**Settings stands alone, on any desktop, and changes whatever of
omorfos's is on the machine**, and nothing else: a page for each
program of ours that is installed and none for what is not, made from
the schema every file of settings already has (ADR 73). With omorfos
installed that is the compositor's settings too; without it, the
dock's, the lock screen's, the greeter's, whichever are there. It is
not a place to set the machine's network or its users: those are the
system's, and the desktop's it is run on.

Some of what is ours is not the user's own to change, as the greeter's
settings are the machine's. Those are locked until the user unlocks
them, as they are asked to, by the system's own way of asking. How
that is done without anything of ours running as root for longer than
one change takes (ADR 35) is for an ADR of its own, and a review for
security, before any of it is written.

## What standing alone obliges

- **Settings and a theme of its own.** A program that stands alone
  reads `theme.css` and its own file of settings where omorfos's are,
  and is whole without them; it does not need `config.yaml`.
- **Nothing asked of omorfos that cannot be done without.** What it
  learns from omorfos's socket, the scheme above all, it learns
  elsewhere where there is no socket, or does without.
- **Tried where it is said to run.** At least one other compositor in
  the tests or by hand before a release says so, and named in the
  program's manual page.
- **Packaged by itself**, with omorfos's libraries built in and no
  dependency on the compositor's package.
- **Said plainly what is missing there**: no glass, no shadow, no seal,
  no pictures of windows.

## Alternatives

- **Everything omorfos's own.** Least work, and what most desktops do.
  It is the divergence the maintainer set out to end: a good lock
  screen or file manager that can only be had with a whole desktop.
- **Everything standing alone.** The bar, notifications and the portal
  would have to speak every compositor's own language, each a backend
  to keep; what they would bring others already have.
- **Screenshot and recording standing alone**, taking pictures by the
  standard protocol and recording through the portal every desktop
  has. Possible, and another program inside from today's two. Not
  wanted for now.

## Consequences

- Each program that stands alone is a promise beyond omorfos, kept at
  every release, and a package more.
- The programs of stage 7 draw their own titlebars (ADR 97) and are
  solid where the compositor gives them no glass: what they look like
  elsewhere is to be tried before they are said to be beautiful there.
- `omorfos-lock`, `omorfos-wallpaper` and `omorfos-clipboard` exist and
  talk to omorfos for little; making that little optional is the first
  of this to be done, and is on the roadmap.
- GNOME gets the applications, Settings, the greeter and the pinentry,
  and nothing of the shell but perhaps the launcher.
