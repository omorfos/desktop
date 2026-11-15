# 88. Restart and shut down ask first

- Status: accepted
- Date: 2026-11-15

## Context

The omorfos menu of stage 5 is to have Sleep, Restart, Shut Down and Lock
Screen, each with its key beside it. The maintainer chose (15 November
2026) to have the bar's button open the compositor's root menu rather
than a menu of the bar's own, so these go into the root menu; and that
Restart and Shut Down ask before they act, as GNOME does, with a setting
to have them not ask.

`omorfos-session sleep`, `restart` and `shut-down` already ask logind for
each, and a key could be bound to spawn them; nothing asked first.

## Decision

**`sleep`, `restart` and `shut_down` are actions of the compositor, and
the two that end what is open ask first, wherever they come from:** the
root menu, a key, or `omorfosctl action`. One setting, `power.ask` in
`config.yaml`, on by default, covers them all; set to false, they act at
once. Sleep never asks: nothing is lost by it, and it locks first.

- **The question is a card of the compositor's,** in the middle of the
  screen it was asked on, drawn as `question` and `question-button` say:
  what is asked, how many seconds are left, that what is not saved in
  open programs will be lost, and Cancel beside the button that goes on,
  which is filled with the accent and has the keyboard's ring. It has the
  pointer and the keyboard while it is up. **Enter goes on,** as the
  button with the ring does in GNOME's; Escape, Cancel or a click
  anywhere else does not.
- **Left alone, it is done after a minute,** counting the seconds down,
  as GNOME's is: the maintainer asked for it (15 November 2026), since
  whoever walked away from a machine being turned off meant it to go off.
  The minute is fixed until the defaults are reviewed together.
- **The session is sealed round it as round a prompt for a secret**
  (ADR 48): dimmed and blurred as `sealed` says, the card casting a
  question's shadow and wearing its glass if the theme makes it glass.
  The same code draws both, for whichever is up.
- **They ask `omorfos-session`, as the compositor's other requests of it
  do** (`ask_session`), through the program it was given; the tests give
  it a stand-in that notes what it is asked, so that no test can turn the
  machine off.
- **A locked session takes the question away,** as it does the menu and
  "About".
- **Each row of a menu has at its end the keys that do what it does,** the
  first binding of its action in `keys.yaml`: Super, Ctrl, Alt and Shift,
  in that order, by Lucide's signs (four squares, a caret up, the option
  sign, a broad arrow up), and Enter, Space, Tab, Backspace and the arrows
  by theirs; other keys as written on them, as `Q`, `Esc` or `Del`, as
  Lucide has no sign for Escape nor for a forward Delete. Each stands a
  little apart from the next, where a `+` would be, as the maintainer
  asked, and in `menu-keys`, whose colour, `--text-dim`, is held to the
  same contrast as the rest of the menu's text. A row without a binding
  has none; no keys are made up to fill the gaps, as the defaults are to
  be reviewed together.

## Consequences

- The root menu's built-in entries gain Lock screen, Sleep, Log out…,
  Restart… and Shut down…, each with a sign of its own (a moon, a door, an
  arrow turning back, a switch's), said in the user's language, in place
  of Quit, which the maintainer had taken out; the ellipsis is there
  because the last three ask. With `power.ask: false` it still is.
- **`log_out` asks as `restart` and `shut_down` do, and `quit` does not:**
  the lock screen's Log Out has asked already, and asks the compositor to
  `quit`, over a locked session no question could be answered on; and the
  tests end their sessions with it. `Super+Shift+E` is still `quit`, so the
  menu's Log out… has no keys beside it until the defaults are reviewed.
- The menu stays 240 wide: rather than widen it for Polish, the
  maintainer had Reload configuration and Welcome taken out of the
  built-in menu, where they were seldom wanted; `Super+Shift+R` and the
  `show_welcome` action still do them, and `menu.yaml` can put them back.
- Settings and Task Manager, which the roadmap's omorfos menu names, are
  left out until they exist.
- The bar's button, still to come, opens this menu hanging from the bar.
