# The defaults, reviewed together

- Date: 2026-11-20
- Scope: omorfos's built-in key bindings, the questions about its
  defaults gathered on the roadmap, how many fingers swipe, and Super as
  the key omorfos is driven by, against what GNOME, KDE, macOS and
  Windows do.
- What it leads to: [ADR 101](../adr/0101-super-drives-the-desktop-and-the-ordinary-way-is-a-setting.md),
  proposed, which takes up [ADR 78](../adr/0078-super-belongs-to-the-desktop.md).

The roadmap asked for the defaults to be reviewed together, keys and
settings, rather than one at a time as each feature came, and for every
common chord to be gone through and found on Super. This is that
inventory, and a proposal for each choice. The choices are the
maintainer's.

Each thing said of another system is marked by how it is known:

- **read**: in the system's own code, at the commit linked under
  [Sources](#sources);
- **documented**: its maker says so in its own documents;
- **reported**: users or the press say so, and nothing better was
  found.

GNOME's and KDE's are read; Apple's and Microsoft's documented. Nothing
was run on any of them.

## How each system holds its keys

- **GNOME** gives Super to the desktop for its own shortcuts and leaves
  Ctrl to programs. Super alone, tapped, opens the overview (`overlay-key`
  is `Super`). Its window shortcuts are on Super and, from older days,
  Alt (`Alt+F4`, `Alt+F7`, `Alt+F8`, `Alt+F10`).
- **KDE** does the same with Meta, and keeps Alt for the switcher and
  `Alt+F4`.
- **macOS** gives Command to programs, and their shortcuts are on it
  (copy is Cmd+C); the desktop's own are mostly on Control and on the
  function row (Mission Control is Ctrl+Up).
- **Windows** gives programs Ctrl, and the desktop the Windows key.

omorfos took macOS's way with Super in the place of Command (ADR 78):
what nothing binds reaches the program as Ctrl, so that Super+C copies.
It is the only system here where Super is both the desktop's and the
programs'. That is what makes some chords the others have free taken
here: Super+L is the address bar, Super+F find, Super+A choose all.

## Chord by chord

What omorfos has today, what the others have, and what is proposed. A
blank is a chord the system leaves unbound by default. What is proposed
is the Mac's first, with Super for Command (ADR 101), and the others'
only where the Mac has nothing.

| What | omorfos today | GNOME (read) | KDE (read) | macOS (documented) | Windows (documented) | Proposed |
|---|---|---|---|---|---|---|
| The launcher | Super+Space | Super+A (applications) | Meta tapped, Alt+Space, Alt+F2 | Cmd+Space | Super | Super+Space, as now |
| The overview | `XF86LaunchA` | Super, tapped | Meta+W | Ctrl+Up | Super+Tab | Super+Up, and `XF86LaunchA` |
| A program's windows | none | none | none | Ctrl+Down | none | Super+Down, once the overview can |
| Super tapped alone | nothing | the overview | the launcher | nothing | the Start menu | nothing |
| Switching | Alt+Tab, Super+Tab: windows | Alt+Tab, Super+Tab: programs | Alt+Tab, Meta+Tab: windows | Cmd+Tab: programs | Alt+Tab: windows | Super+Tab: programs |
| The program's next window | none | Super+\`, Alt+\` | none | Cmd+\` | none | Super+\` |
| The tabs of a group of windows | Super+\` | none | none | Ctrl+Tab | none | Super+Ctrl+Tab |
| Closing a window | Super+Q | Alt+F4 | Alt+F4 | Cmd+W | Alt+F4 | Super+W, the program's (Ctrl+W); Alt+F4 in the ordinary way |
| Quitting a program | none | none | none | Cmd+Q | none | Super+Q |
| Maximizing | Super+M | Super+Up | Meta+PgUp | Fn+Ctrl+F, to fill | Super+Up | Super+Alt+Up, to fill |
| Full screen | Super+Ctrl+F | none | none | Ctrl+Cmd+F | none | as now |
| Minimizing | Super+H | Super+H | Meta+PgDown | Cmd+M | Super+Down | Super+M |
| Hiding a program, and the others | none | none | none | Cmd+H, Cmd+Option+H | none | Super+H, Super+Alt+H |
| Half a screen | none | Super+Left, Super+Right | Meta+arrows | Fn+Ctrl and the arrows | Super+arrows | Super+Alt+Left and Right |
| The next workspace | Super+Left, Super+Right | Super+PgUp, PgDown | Meta+Ctrl+Left, Right | Ctrl+Left, Ctrl+Right | Super+Ctrl+Left, Right | as now |
| A window to a workspace | Super+Shift and a digit | Super+Shift+Home, End, PgUp, PgDown | none | none | none | Super+Ctrl and a digit |
| Locking | none | Super+L, `XF86ScreenSaver` | Meta+L, `XF86ScreenSaver` | Ctrl+Cmd+Q | Super+L | Super+Ctrl+Q, and `XF86ScreenSaver` |
| Logging out | `Super+Shift+E`, without asking | Ctrl+Alt+Delete | none | Shift+Cmd+Q, asking | none | Super+Shift+Q, asking |
| Every screen's picture, saved | Print | Shift+Print copies and saves it | Shift+Print | Shift+Cmd+3 | Super+Print | Print, and Super+Shift+3 |
| The picker | Shift+Print | Print | Print (Spectacle) | Shift+Cmd+4, 5 | Super+Shift+S | Shift+Print, and Super+Shift+4 and 5 |
| A window's picture | Alt+Print | Alt+Print | Meta+Print | Shift+Cmd+4, Space | Alt+Print | as now |
| The next layout | none | Super+Space | Meta+Alt+K | Ctrl+Space, Ctrl+Option+Space | Super+Space | Super+Alt+Space, and `XF86Keyboard` |
| The clipboard's history | none | none | Meta+V | none | Super+V | none: Super+V is paste |
| The screens' arrangement | `XF86Display` | `XF86Display`, Super+P | `XF86Display`, Meta+P (kscreen) | none | Super+P | as now |
| Volume, brightness, playing | their keys | their keys | their keys | their keys | their keys | as now |
| The touchpad on and off | none | `XF86TouchpadToggle` | `XF86TouchpadToggle` | none | none found | `XF86TouchpadToggle`, once the action is built |
| The star key | none | none | none | none | none | none, and written down as the user's |

The Mac's keys for the desktop use Ctrl alone, which here is the
programs'. Those take Super instead, or Super+Ctrl where Super alone is a
program's shortcut as Ctrl:

- **Switching and the program's windows.** Cmd+Tab goes between
  programs and Cmd+\` between the windows of the one in front; omorfos
  goes between windows with Super+Tab, and through a group's tabs with
  Super+\`. Both become the Mac's, and the tabs move to Super+Ctrl+Tab,
  as a Mac's windows' tabs are Ctrl+Tab.
- **Closing and quitting.** Cmd+W is the app's: it closes a tab where
  there are tabs, the window where there are not. Super+W already
  reaches the program as Ctrl+W, which does the same in most programs,
  so it stays theirs, and tabs can still be closed from the keyboard.
  Cmd+Q quits the app; Super+Q, which closes one window today, closes
  every window of the program instead.
- **Minimizing and hiding.** Cmd+M minimizes and Cmd+H hides the app;
  omorfos has Super+M maximize and Super+H minimize. Both become the
  Mac's, and filling the screen goes with the halves, to Super+Alt+Up.
- **Locking** is Super+L on three of the four, and Super+L is Ctrl+L
  here, the address bar. macOS's Ctrl+Cmd+Q is Super+Ctrl+Q here, which
  no program has.
- **Logging out** is Shift+Cmd+Q, which asks; Super+Shift+Q does the
  same. `Super+Shift+E`, which ends the session without asking and is
  Ctrl+Shift+E in programs as well, goes.
- **The overview** is Ctrl+Up on the Mac and Super+Up here, as the
  workspaces are already Super+Left and Right for Ctrl+Left and Right.
  Neither a Mac nor omorfos does anything with a tap of Command or Super
  alone, and that stays: a tap opens the overview on GNOME and the
  launcher on KDE and Windows, and either under a resting thumb.
- **Pictures** are Shift+Cmd+3, 4 and 5 on a Mac, which has no Print
  key; Super+Shift+3, 4 and 5 do the same here, beside Print. A window
  goes to a workspace with Super+Ctrl and a digit, so that Super+Shift
  with one is free for them.
- **The next layout** is Ctrl+Space on a Mac, a program's here;
  Super+Alt+Space follows its Ctrl+Option+Space, and `XF86Keyboard`
  where a keyboard has it.
- **Half a screen** is Fn+Ctrl and the arrows on a Mac. Super+Alt and
  the arrows is free, and Super+Ctrl and the arrows already go between
  windows beside each other.
- **The clipboard's history** is Meta+V on KDE and Super+V on Windows,
  and Super+V is paste here. It is left unbound, to be opened from the
  panel once there is a history (the roadmap's own item).

## The rest that has gathered

- **Copy, cut, paste and undo on Super, with Shift too.** Done by ADR 78
  in programs: any chord with Super nothing binds reaches the program as
  Ctrl, Shift kept. The shell's own fields, the launcher's and the
  boxes that ask for a password, read Super and Ctrl alike today
  (`omorfos-shell`'s `typing.rs`). Proposed: Super only, as a Mac's
  fields read Command, with Ctrl left for what a Mac's fields do with it,
  Ctrl+A to the start of the line and Ctrl+E to the end; Ctrl in the
  ordinary way.
- **Choosing all in a field for a secret**: Super+A, by the same rule.
- **`Shift+Print` for the picker and `Print` for every screen, saved**:
  kept. GNOME has them the other way about, and its Shift+Print copies
  as it saves; KDE takes the whole screen with Shift+Print and opens
  Spectacle with Print. The Mac's own are Super+Shift+3, 4 and 5, above.
- **`clipboard.forget`**: kept at 0, for as long as the session. KDE
  keeps a history across the session by default, Windows keeps none
  until it is turned on, and what omorfos keeps is what was last copied
  and nothing more, which forgetting after a time would only make
  surprising.
- **The launcher's icons and descriptions**: kept on, as was asked on
  15 November 2026.
- **The theme's parts made along the way**: not looked at here. They
  are the design review's, against `docs/DESIGN.md`, rather than a
  default to choose.

## The touchpad

How many fingers swipe across the touchpad from one workspace to the
next, and up to the overview:

| | Workspaces | The overview |
|---|---|---|
| omorfos today | three | three, up |
| GNOME (read) | three or more | three or more, up |
| KDE (read) | three or four | four, up |
| macOS (documented) | four, three in some versions | four, up; down for the app's windows |
| Windows (documented) | four (three switches windows) | three, up |

A Mac also pinches its thumb and three fingers together for its
programs, the Apps that took Launchpad's place in macOS 26, and spreads
them for the desktop; and its settings show each gesture moving, to turn
it off or change it.

Proposed, as the Mac: **three or four fingers, either**, left and right
for the workspaces, up for the overview and down for the program's
windows; **the pinch** for the launcher with every program shown, and
**the spread** for the desktop; and a page of settings for the touchpad
that shows each, for the roadmap. Three fingers held still are not taken
from programs: only a swipe is, as now.

## Super, everywhere, and the ordinary way

The roadmap asks for a way back to Ctrl and Alt as Windows, GNOME and
KDE have them. ADR 78 has one switch, `keyboard.super_as_ctrl`, which
stops Super chords reaching programs as Ctrl but leaves every built-in
binding where it was. The proposal, in ADR 101, is one setting with two
ways, each of which moves the bindings with it:

- **`super`**, the default and ADR 78's: what is above, the Mac's.
- **`ordinary`**: what GNOME and KDE have. Super reaches no program as
  Ctrl, and Ctrl chords are the programs' as typed; Alt+Tab switches
  windows and Alt+F4 closes one, Super+L locks, Super+Left and Right take
  half the screen, and the workspaces move to Super+PgUp and PgDown, as
  GNOME; the shell's fields read Ctrl.

Alt+Tab and Alt+F4 are the ordinary way's alone: in the `super` way
Super+Tab and Super+Q do what they would. Alt+Print stays in both.

## What was not looked at, and what is not sure

- Nothing was run. GNOME's and KDE's defaults were read in their code at
  the commits below; a distribution may change them.
- What Print does on Windows has changed: Microsoft's own pages say both
  that it copies the screen and that it opens the Snipping Tool, which
  newer versions of Windows 11 do by default. The table has the copy.
- Not gone through: the side of a titlebar's buttons (ADR 97), the
  question's minute (ADR 88), a tooltip's half second (ADR 87), a key
  for the screens where there is no Display key, and the other built-in
  bindings (`Super+Return`, `Super+Escape`, `Super+Ctrl+F` and `N`,
  `Super+grave`, `Super+Shift+R`, Super with the digits). Super+M,
  which maximizes, is Ctrl+M in programs, as in some mail programs.
- Apple's Fn+Ctrl and the arrows, and Fn+Ctrl+F to fill, are from
  memory of its pages, not read again.
- The ADR is proposed, not accepted: each choice above is the
  maintainer's.

## Sources

- GNOME's window shortcuts: [gsettings-desktop-schemas, `org.gnome.desktop.wm.keybindings`](https://github.com/GNOME/gsettings-desktop-schemas/blob/b7c009e73dab7095372e399d8e33e9f1b282ee89/schemas/org.gnome.desktop.wm.keybindings.gschema.xml.in)
- GNOME's tiling, overlay key and screens: [mutter, `org.gnome.mutter`](https://github.com/GNOME/mutter/blob/9bc8cd8427a5aa9bf8976acaef4426ce09a08537/data/org.gnome.mutter.gschema.xml.in)
- GNOME's shell, screenshots and applications: [gnome-shell, `org.gnome.shell`](https://github.com/GNOME/gnome-shell/blob/17cac9968d45402ba91e253170b96e9e641512fe/data/org.gnome.shell.gschema.xml.in), and its touchpad's fingers in [`swipeTracker.js`](https://github.com/GNOME/gnome-shell/blob/17cac9968d45402ba91e253170b96e9e641512fe/js/ui/swipeTracker.js)
- GNOME's locking and media keys: [gnome-settings-daemon, `media-keys`](https://github.com/GNOME/gnome-settings-daemon/blob/d3e717002a6f89aa1f350f4c6267ceeaff6d2cc6/data/org.gnome.settings-daemon.plugins.media-keys.gschema.xml.in)
- KDE's window shortcuts: [kwin, `useractions.cpp`](https://github.com/KDE/kwin/blob/c24a5f4e85683de94108fc3ca6ff48ea726f5037/src/useractions.cpp); its overview, [`overvieweffect.cpp`](https://github.com/KDE/kwin/blob/c24a5f4e85683de94108fc3ca6ff48ea726f5037/src/plugins/overview/overvieweffect.cpp); its workspaces' fingers, [`virtualdesktops.cpp`](https://github.com/KDE/kwin/blob/c24a5f4e85683de94108fc3ca6ff48ea726f5037/src/virtualdesktops.cpp)
- KDE's screens: [kscreen, `org.kde.kscreen.desktop`](https://github.com/KDE/kscreen/blob/8c0dab459e2c626b02698cf32c37e9c9e28b8d09/osd/org.kde.kscreen.desktop); Meta tapped for the launcher: [plasma-workspace, `shellcorona.cpp`](https://github.com/KDE/plasma-workspace/blob/6aa1cf7fa3b613db25595592e91a07a60c7c53e4/shell/shellcorona.cpp)
- KDE's locking: [kscreenlocker, `kscreensaversettings.cpp`](https://github.com/KDE/kscreenlocker/blob/c5d125c820f248a9d2346ae0692ecde56e51a3e7/settings/kscreensaversettings.cpp); screenshots, [spectacle, `SpectacleCore.cpp`](https://github.com/KDE/spectacle/blob/8a12ab75612d45346994d133f4e79cb44543a5cc/src/SpectacleCore.cpp); the clipboard, [plasma-workspace, `klipper.cpp`](https://github.com/KDE/plasma-workspace/blob/6aa1cf7fa3b613db25595592e91a07a60c7c53e4/klipper/klipper.cpp); the launcher, [`org.kde.krunner.desktop`](https://github.com/KDE/plasma-workspace/blob/6aa1cf7fa3b613db25595592e91a07a60c7c53e4/krunner/org.kde.krunner.desktop.cmake); the layouts, [plasma-desktop, `bindings.cpp`](https://github.com/KDE/plasma-desktop/blob/07bdbbd67b548fc703e26131899007dfeceb2593/kcms/keyboard/bindings.cpp); the touchpad, [`org.kde.touchpadshortcuts.desktop`](https://github.com/KDE/plasma-desktop/blob/07bdbbd67b548fc703e26131899007dfeceb2593/kcms/touchpad/org.kde.touchpadshortcuts.desktop)
- Apple: [Mac keyboard shortcuts](https://support.apple.com/102650) and [Use Multi-Touch gestures on Mac](https://support.apple.com/102482)
- Microsoft: [Keyboard shortcuts in Windows](https://support.microsoft.com/en-us/help/12445/windows-keyboard-shortcuts), [Touch gestures for Windows](https://support.microsoft.com/help/17210) and [Best shortcuts for PC users](https://www.microsoft.com/en-us/windows/learning-center/best-shortcuts-for-pc-users-save-time-on-windows-11)
