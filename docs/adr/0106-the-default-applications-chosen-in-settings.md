# 106. The default applications chosen in Settings

- Status: accepted
- Date: 2026-11-24

## Context

The root menu opens the default browser, file manager and editor as the
`mimeapps.list` files say (ADR 89), and its Terminal row, and Super+Return,
the terminal an `xdg-terminals.list` names (#120). Trying them on 24
November 2026, the maintainer asked how the Settings application of stage
7 (wish 7) is to choose them: whether it writes those lists, who keeps them
as programs are installed and taken away, and whether it moves the lines
there are or writes out every program that could do the work.

What opens what is already said in two places of the same shape, each a
list of desktop entries in order of preference, the first that is
installed being the one:

- **`mimeapps.list`**, freedesktop.org's MIME Applications specification:
  for each type of file or kind of link, as `inode/directory` for a folder,
  `text/plain` for text and `x-scheme-handler/https` for a link, the
  entries that open it, `type=first.desktop;second.desktop;`. Every
  toolkit's "open with" and `xdg-open` read it.
- **`xdg-terminals.list`**, of the Default Terminal Execution proposal,
  which `xdg-terminal-exec` reads: one entry to a line, for the terminal,
  which no type names. It is not yet an agreed standard
  ([STANDARDS.md](../STANDARDS.md), "The default terminal").

Each is read from layers, the user's own first: `~/.config`, then the
system's (`/etc/xdg`), then the data directories'
(`/usr/share/applications` for `mimeapps.list`). In each layer a list of
the desktop's own, `omorfos-mimeapps.list` or `omorfos-xdg-terminals.list`,
named by `XDG_CURRENT_DESKTOP`, comes before the one every desktop reads.
`omorfos-entries` reads both in that order (the terminal's since #126),
when an application is asked for, and keeps nothing of them.

## Decision

**The files are the defaults; Settings is one way of writing the user's
own, as a person could by hand.** Nothing keeps a list up to date: each is
read when an application is wanted, so one installed since is there at
once, and one taken away is passed over for the next.

- **What it offers**, for each kind, the browser, the file manager, the
  editor, the terminal, the players of video and of music, the viewer of
  pictures, the mail program and the rest the roadmap names: the desktop
  entries installed that say they can do it, by their `MimeType=` for a
  type or a scheme and by `Categories=TerminalEmulator` for a terminal.
  Worked out when the page is opened, from what is installed then, and
  written nowhere.
- **What it shows as chosen**: the one that would open now, found by the
  same reader as the root menu's `open` and Super+Return, so that the page,
  the menu and the key never disagree.
- **Choosing one moves it to the top of the user's own list and keeps the
  rest below.** It does not write out what is installed. The lines kept are
  the choices before: a browser taken away leaves the one before it, not
  nothing. Lines it does not understand, and types it does not offer, are
  kept as they are, as `config.yaml` is kept by `omorfos_config::edit`
  (ADR 81).
- **It writes the files every desktop reads**, `~/.config/mimeapps.list`
  and `~/.config/xdg-terminals.list`, as GNOME's and KDE's settings write
  theirs, so that a program's own "open with" opens what the menu does.
  A list of omorfos's own in `~/.config` that names another first would
  win over it; the page says so where there is one, rather than seem not
  to take the choice.
- **A kind of several types is chosen as one**: a browser is written first
  for `x-scheme-handler/https`, `x-scheme-handler/http`, `text/html` and
  `application/xhtml+xml`, so that a link and a page saved open in the same
  place; an editor for `text/plain`; a file manager for `inode/directory`.
- **omorfos's own defaults go in the system's layer, never in the user's
  files**: its package may carry an `omorfos-xdg-terminals.list` naming
  foot, and an `omorfos-mimeapps.list`, so that a session with nothing set
  has them, a choice of the user's wins over them, and taking it away
  brings them back.
- **Settings itself** is no type and in no list: `open: settings` opens
  omorfos's own Settings, as ADR 89 says, once there is one.

## Alternatives

- **A list of every program that can do the work, written out by Settings
  and kept up to date as programs come and go.** It would need something
  to watch what is installed, and would write over what the user put
  there by hand. The lists already pass over what is not installed.
- **The choice kept in `config.yaml`** and the lists left alone: the menu
  and the key would follow it, every program's "open with" would not, and
  two places would say what opens what.
- **Only omorfos's own lists written**, `omorfos-mimeapps.list` and
  `omorfos-xdg-terminals.list`: what the user chose would hold only while
  `XDG_CURRENT_DESKTOP` names omorfos, and a program started with another
  would open something else.
- **A terminal of our own choosing in `config.yaml`**, as `terminal:`,
  beside the proposal that terminals and GNOME already follow (ADR 99).

## Consequences

- The default applications page of Settings writes `mimeapps.list` and
  `xdg-terminals.list` and nothing of its own; changed by hand or by
  another desktop's settings, what they say is what the page shows next.
- The reader in `omorfos-entries` is the one source of what is chosen, for
  the menu, the key, the welcome card and Settings.
- The package carries omorfos's defaults as lists of the system's, once
  there are any to carry.
- `xdg-terminals.list` is followed while it is a proposal, and is listed in
  `STANDARDS.md` until it is agreed.
