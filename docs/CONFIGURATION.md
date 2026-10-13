# Configuration

omorfos reads YAML files, and a stylesheet for the theme, from
`$XDG_CONFIG_HOME/omorfos/`, which is usually `~/.config/omorfos/`. Every file is optional. Changes are picked up as soon
as you save.

If a file has a mistake, omorfos keeps using that file's previous contents
and reports the problem with its file, line and column. The other files
still apply. Problems are shown among the notifications, at the top right
of the screen, each until you click it away or put it right, and are
printed by `omorfosctl config check`. Where no notifications are being
shown by omorfos, they are in a notice of the compositor's own in the same
corner.

General rules:

- Unknown settings are errors, so typos are caught.
- Booleans are `true` or `false` only.
- Files larger than 1 MiB are refused.

For completion and inline validation in editors that use
`yaml-language-server`, point a file at its schema in
[`schemas/`](../schemas), for example:

```yaml
# yaml-language-server: $schema=/path/to/schemas/keys.schema.json
```

## `config.yaml`

```yaml
workspaces: [web, code, chat, media]   # 1 to 32 names
focus:
  model: click            # click | sloppy
  raise_on_focus: true
placement: cascade        # cascade | center | pointer
keyboard:
  layout: us              # XKB layout; several as "gb,pl"
  variant: ""
  options: ""             # e.g. "caps:escape"
  repeat_delay: 400       # milliseconds
  repeat_rate: 30         # per second; 0 disables repeat
touchpad:
  tap_to_click: true
  natural_scroll: false
output:
  scale: auto             # auto | 1 to 4 in steps of 0.25
outputs: []               # particular screens, by name
idle:
  screen_off: 0           # seconds without input; 0 never
autostart: []             # programs to start with the session
panel:
  enabled: true           # start the panel that comes with omorfos
cursor:
  theme: default          # a cursor theme by name
  size: 24                # 8 to 128
launcher:
  icons: false            # show each program's icon
  descriptions: false     # show, and search, what each program says of itself
notifications:
  enabled: true           # show what programs have to tell you
  timeout: 5              # seconds one stays; 0 until it is clicked
xwayland:
  enabled: true           # run programs written for X11
  scaling: stretched      # stretched | native, on a screen with a scale
```

The values shown are the defaults, except `workspaces`, which defaults to
`["1", "2", "3", "4"]`.

`idle.screen_off` turns the screens off after that many seconds without
input, and on again at the next key or movement. Something that asks for
the session to stay awake, such as a film being played, keeps them on. It
is off by default.

`autostart` lists programs to start when the session does, each as a
program and its arguments, as for `spawn`:

```yaml
autostart:
  - [swayidle, -w, timeout, "300", swaylock -f]
  - [waybar]
```

They are started once, in order. Changing the list takes effect the next
time you log in; reloading the configuration does not start them again. A
program that cannot be started is reported like any other problem.

`cursor.theme` names a cursor theme installed in `~/.local/share/icons`,
`~/.icons` or `/usr/share/icons` (or the directories in `XCURSOR_PATH`).
Shapes the theme lacks come from the themes it inherits from and then from
`default`. If no theme has a shape, omorfos draws a plain arrow of its own.

With more than one screen, they are placed side by side from left to
right in the order they were plugged in, a laptop's own first. To say
otherwise, list them under `outputs`:

```yaml
outputs:
  - { name: HDMI-A-1, scale: 1.25 }
  - { name: eDP-1 }
```

The screens listed stand in that order from the left, ahead of any that
are connected but not listed, and each may have a `scale` of its own in
place of `output.scale`. A screen's name is the `output` shown by
`omorfosctl state`. One that is listed but not plugged in is skipped.
Screens can only be side by side for now, not one above another.

Each screen has its own set of the workspaces listed, and shows one of
them: switching workspace on one screen leaves the others as they are. A
window dragged onto another screen joins the workspace that screen is
showing. When a screen is unplugged, its windows move to the workspace of
the same number on the first screen that is left.

`output.scale` is how large everything is drawn, on every screen that
`outputs` does not give a scale of its own. With `auto`, omorfos
chooses for each screen from how dense it is, so that a laptop with a sharp panel
is as readable as a desktop monitor. If the choice does not suit you, give
a number: `1` is one pixel to a pixel, `1.5` half as large again, `2`
twice the size.

## `keys.yaml`

```yaml
bindings:
  - keys: Super+Return
    action: { spawn: [foot] }
  - keys: Super+q
    action: close
  - keys: Super+x t        # a chain: Super+x, then t
    action: { workspace: 2 }
```

Giving `bindings` replaces all built-in bindings; `bindings: []` removes
them.

A chord is zero or more modifiers and a key, joined by `+`. Modifiers are
`Ctrl`, `Alt`, `Shift` and `Super` (case does not matter; `Control`, `Mod1`
and `Mod4` also work). Keys are XKB keysym names such as `Return`, `a`, `F1`
or `XF86AudioMute`. A binding names the key, not the symbol it types: write
`Super+Shift+r`, not `Super+R`. A binding whose key name does not exist is
ignored and reported.

Two bindings may not use the same keys, and a chain may not begin with keys
that are already bound on their own.

### Actions

| Action | Effect |
|---|---|
| `spawn: [program, arg, ...]` | Start a program. No shell is involved. |
| `close` | Close the focused window. |
| `maximize`, `fullscreen`, `minimize` | Toggle or apply that state. |
| `focus_next`, `focus_previous` | Cycle windows. |
| `next_tab`, `previous_tab` | Cycle tabs in the focused group. |
| `workspace: N` | Switch to workspace N (from 1) on the screen the pointer is on. |
| `next_workspace`, `previous_workspace` | Cycle workspaces on the screen the pointer is on. |
| `move_to_workspace: N` | Send the focused window to workspace N of its screen. |
| `move_to_next_output` | Send the focused window to the next screen. |
| `show_menu` | Open the root menu. |
| `show_launcher` | Open the launcher. |
| `reload_config` | Re-read the configuration. |
| `quit` | End the session. |

To run a shell command, spawn the shell: `spawn: [sh, -c, "grim - | wl-copy"]`.

## `rules.yaml`

```yaml
rules:
  - match: { app_id: firefox }
    workspace: 1
  - match: { app_id: "org.gnome.*", title: "*Preferences" }
    maximized: false
    decorations: true
  - match: { app_id: mpv }
    fullscreen: true
```

A rule needs `app_id`, `title` or both; `*` stands for any run of
characters. Every matching rule applies, in order. Properties are
`workspace`, `maximized`, `fullscreen` and `decorations`. Without a
`decorations` rule, a window is framed if it asks omorfos to decorate it.

## `theme.css`

The theme is a stylesheet, in a small part of CSS. A rule names parts of
the desktop, perhaps in some state, and gives them properties. Yours is
laid over the built-in theme, so you only write what you want changed. A
rule of yours always wins over a built-in one; among your own, the one that
names more states wins, and then the later one.

This is the whole built-in theme, which is also every part there is:

```css
/* The built-in theme of omorfos.
 *
 * Your own theme.css is laid over this one: a rule of yours always wins
 * over a rule here, so you only write what you want changed. The quickest
 * change is to give the variables below other values. */

:root {
  --background: #14161b;   /* behind everything */
  --accent: #5b9cf5;       /* what has focus, and what is picked out */
  --surface: #23262e;      /* titlebars, menus and what is lit */
  --surface-dim: #1b1d23;  /* the same, where focus is elsewhere */
  --text: #e6e8ee;
  --text-dim: #8b90a0;
  --border: #2c303a;
}

/* Colours take a moment to change when a part's state does. To have them
 * change at once, write `* { transition: none; }` in your theme. */
* {
  font-family: sans-serif;
  font-size: 11pt;
  transition: 120ms ease-out;
}

desktop { background: var(--background); }

/* A window's frame. Its sizes are taken from the rule without a state. */
window {
  border: 1px solid var(--border);
  border-radius: 8px;
}
window:focus { border-color: var(--accent); }

titlebar {
  height: 28px;
  background: var(--surface-dim);
  color: var(--text-dim);
}
titlebar:focus {
  background: var(--surface);
  color: var(--text);
}

/* A title in a titlebar. Where there are several, the border on the left
 * is the line between one tab and the next. */
tab {
  padding: 0 10px;
  background: var(--surface-dim);
  color: var(--text-dim);
  border-left: 1px solid var(--border);
}
tab:focus { border-left-color: var(--accent); }
tab:active { border-bottom: 2px solid var(--border); }
tab:active:focus {
  background: var(--surface);
  color: var(--text);
  border-bottom-color: var(--accent);
}

menu {
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
}
menu-item {
  height: 28px;
  padding: 0 10px;
  color: var(--text);
}
menu-item:hover {
  background: var(--accent);
  color: var(--background);
}
menu-separator { color: var(--border); }

notice {
  padding: 10px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--accent);
}
notice-heading { color: var(--accent); }

/* The panel, and what is on it: the workspaces, a task for each window on
 * the one being shown, and the clock. A margin on the panel moves it in
 * from the edges of the screen. */
panel {
  height: 28px;
  background: var(--surface-dim);
  color: var(--text-dim);
}
workspace, task {
  padding: 0 10px;
  color: var(--text-dim);
}
workspace { min-width: 28px; }
task { max-width: 240px; }
workspace:active, task:active {
  background: var(--surface);
  color: var(--text);
  border-bottom: 2px solid var(--accent);
}
clock {
  padding: 0 10px;
  color: var(--text);
}

/* The launcher: where a name is typed, and the programs that match. The
 * launcher is as wide as its min-width. */
launcher {
  min-width: 480px;
  padding: 8px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--accent);
  border-radius: 8px;
}
launcher-input {
  height: 32px;
  margin: 0 0 8px 0;
  padding: 0 10px;
  background: var(--surface-dim);
  color: var(--text);
  border-radius: 6px;
}
launcher-item {
  height: 28px;
  padding: 0 10px;
  color: var(--text);
  border-radius: 6px;
}
launcher-item:active {
  background: var(--accent);
  color: var(--background);
}
/* What a program says of itself, where the launcher is set to show it. */
launcher-description { color: var(--text-dim); }
launcher-description:active { color: var(--background); }
/* The rest of the screen while the launcher is open. A click on it closes
 * the launcher. Give it a see-through colour to dim what is behind. */
launcher-backdrop { background: transparent; }

/* A notification. The margin is the room between one and the next, and
 * between them and the edges of the screen. One a program marks critical
 * is :urgent. */
notification {
  min-width: 340px;
  margin: 8px;
  padding: 10px 12px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 8px;
}
notification:hover { border-color: var(--accent); }
notification:urgent { border-color: #e06c75; }
notification-title {
  color: var(--text);
  font-weight: bold;
}
notification-body { color: var(--text-dim); }
```

The quickest change is to give the built-in theme's variables other
values:

```css
:root {
  --accent: #e0af68;
  --background: #101014;
}
```

A panel that floats clear of the screen's edges, see-through, with rounded
buttons, is a few rules more:

```css
panel {
  margin: 6px 8px;
  border-radius: 10px;
  background: rgba(27, 29, 35, 0.8);
}
workspace, task {
  margin: 3px 2px;
  border-radius: 6px;
}
workspace:active, task:active { border-bottom: none; }
```

**Parts** are `desktop`, `window`, `titlebar`, `tab`, `menu`, `menu-item`,
`menu-separator`, `notice`, `notice-heading`, `panel`, `workspace`, `task`,
`clock`, `launcher`, `launcher-input`, `launcher-item`,
`launcher-description`, `launcher-backdrop`, `notification`,
`notification-title` and `notification-body`. `*` is every part. Several can share a rule:
`workspace, task { ... }`.

**States** follow a part: `:focus` (it has the keyboard, or its window
has), `:active` (the tab or workspace being shown, or the task of the
window with focus, or the program the launcher would start), `:hover` (the
pointer is over it) and `:urgent` (a notification its sender marked as
critical). A part can be in several: `tab:active:focus`.

**Every part is a box.** From the outside in it has a margin, which is room
kept clear around it; a border; padding, which is room between the border
and what the part holds; and its content. Its height, where it has one,
is measured to the outside of its border.

| Property | Value | Notes |
|---|---|---|
| `background` | a color | also written `background-color` |
| `color` | a color | text; for `menu-separator`, the line |
| `opacity` | `0` to `1`, or a percentage | of everything the part draws |
| `border` | a width, `solid` or `none`, a color | any of them, in any order |
| `border-color`, `border-width` | a color; `0` to `16px` | |
| `border-radius` | `0` to `32px` | rounds the corners of the box |
| `padding`, `margin` | one to four sizes, `0` to `64px` | all; top and bottom, then sides; top, sides, bottom; or top, right, bottom, left |
| `height` | `16px` to `64px` | of `titlebar`, `menu-item`, `panel`, `launcher-input` and `launcher-item` |
| `min-width`, `max-width` | `0` to `4096px` | of `workspace`, `task` and `clock`; `max-width: 0` is no limit. The launcher is as wide as its `min-width` |
| `font-family`, `font-size` | a name; `6pt` to `72pt` | a generic name such as `sans-serif` works |
| `font-weight`, `font-style` | `normal`, `bold`, `100` to `900`; `normal`, `italic` | |
| `transition` | a time up to `10s`, and perhaps `linear`, `ease`, `ease-in`, `ease-out` or `ease-in-out`; or `none` | how long the part's colours take to change when its state does, and how; also `transition-duration` and `transition-timing-function` |

`border`, `padding` and `margin` can name one side: `border-bottom`,
`border-left-color`, `padding-top`, `margin-right`.

Colors are written as CSS writes them: `#rgb` or `#rrggbb`, with opacity
as a fourth digit or pair; `rgb(91, 156, 245)` and `rgba(91, 156, 245,
0.5)`; `hsl(215, 89%, 66%)`; a name such as `steelblue`; or `transparent`.
Sizes are whole numbers of `px`, and of `pt` for a font. A variable is
declared in `:root` with a name beginning `--` and used as `var(--name)`,
alone or as part of a value: `border: 1px solid var(--accent)`.

**Colours change over time.** When a part changes state, as a window does
on gaining focus or a menu row when the pointer comes onto it, its
background, text and border colours go from the old to the new over the
time its `transition` gives, written as `150ms` or `0.2s`. It is the
`transition` of the state being entered that counts, so
`menu-item:hover { transition: none; }` lights a row at once and lets it
fade when the pointer leaves. Everything takes 120ms unless you say
otherwise; `* { transition: none; }` turns it all off. Only colours
change this way. Sizes change at once, so that nothing moves under the
pointer, and a `transition` names no property: all the colours of a part
go together. Something drawn for the first time has its colours from the
start.

**What comes onto the screen fades in, and what goes fades out**, over
the same time: a window with its frame over `window`'s `transition`, the
menu over `menu`'s, the notice over `notice`'s, and the panel, the launcher
with what is behind it, and the notifications over `panel`'s, `launcher`'s,
`launcher-backdrop`'s and `notification`'s. A window goes and comes each
time it is hidden and shown: when its workspace is left and returned to, or
when it is minimized and brought back, so that one workspace fades into
the next. Showing another tab of a frame is neither. `window { transition:
none; }` has windows there and gone at once and leaves everything else as
it was. A panel of a program that is not part of omorfos comes and goes at
once, and so does one notification among others that stay.

Not every part uses every property yet:

- `margin` is honoured by `panel` (it moves the bar in from the edges of
  the screen), and by `workspace`, `task` and `clock` (room around each on
  the bar).
- `padding` is honoured by `tab`, `menu-item`, `notice`, `panel`,
  `workspace`, `task` and `clock`.
- `border-radius` is honoured by `window`, `tab`, `menu`, `menu-item`,
  `notice`, `panel`, `workspace`, `task` and `clock`.
- A window's border is the same on every side: its `border-top` stands for
  all four. The sizes of a window's frame and titlebar are taken from the
  rules without a state, since a window does not change size when it gains
  focus.
- Layout (`display`, `position`, `flex`), gradients and shadows are not
  there, and nor are a transition's delay, `cubic-bezier()` or
  `@keyframes`.

Nothing is passed over in silence: a part, state, property or value that
is not understood is a mistake, reported with its line and column, and the
previous theme stays in use. A `theme.yaml` from an earlier version is no
longer read, and omorfos says so while it is there.

## `menu.yaml`

```yaml
items:
  - label: Terminal
    action: { spawn: [foot] }
  - separator
  - label: Session
    items:
      - { label: Reload configuration, action: reload_config }
      - { label: Quit, action: quit }
```

An item is the word `separator`, or a `label` with either an `action` or
nested `items`.

The menu opens with a right click on the desktop or the `show_menu` action
(`Super+space` by default). Click an entry to choose it; click elsewhere or
press Escape to close the menu.

## The panel

`omorfos-panel` puts a bar along the bottom of every screen: that screen's
workspaces, then the windows on the one it is showing, and the time at the
end. Click a workspace to show it, or a window to bring it up. The
workspace being shown and the window with focus are underlined; a
minimized window's title is in brackets. The time is the local time, in
24 hours.

It is a program of its own, started with the session, and started again if
it crashes. How it looks is in `theme.css`, under `panel`, `workspace`,
`task` and `clock`, and it follows the file as you save it.

To go without it, or to use another panel that speaks layer-shell, such as
`waybar`, turn it off in `config.yaml`:

```yaml
panel:
  enabled: false
autostart:
  - [waybar]
```

Like `autostart`, this takes effect the next time you log in.

## The launcher

`Super+d`, or **Run…** in the root menu, opens the launcher: a box in the
middle of the screen the pointer is on. Type part of a program's name and
the list narrows to the programs that have it, those that begin with it
first. Up and Down choose among them, Enter starts the chosen one, and
Escape closes the launcher. A click on a program starts it too, and a
click anywhere beside the launcher closes it.

The programs are those with a desktop entry installed, in
`~/.local/share/applications` and the system's `applications` directories.
One that needs a terminal is started in `foot`.

It shows names alone unless `config.yaml` asks for more:

```yaml
launcher:
  icons: true
  descriptions: true
```

`icons` puts each program's icon before its name. Icons are looked for by
name in the `hicolor` theme of `~/.local/share/icons` and the system's
`icons` directories, and in `/usr/share/pixmaps`; PNG and SVG are read.
Icons a program does not bring itself are taken from `Adwaita`.
`descriptions` writes what each program says of itself after its name, and
has that searched too, after the names. Both take effect the next time the
launcher is opened.

How it looks is in `theme.css`, under `launcher`, `launcher-input`,
`launcher-item` and `launcher-description`. `launcher-backdrop` is the rest
of the screen while it is open: give it a see-through background, such as
`rgba(0, 0, 0, 0.4)`, to dim what is behind. To open the launcher with
another key, bind `show_launcher` in `keys.yaml`.

## Notifications

What programs have to tell you is shown in the top right-hand corner of the
screen the pointer is on, the newest first: a title, up to three lines
broken to fit, and a
picture or the program's icon if it sent one. A click dismisses one, and
tells the program that sent it, which is how clicking a browser's
notification brings up its tab; a click with the right button dismisses it
without that. Otherwise it goes after as long as its sender asked for, or
after `notifications.timeout` seconds. One its sender marked as critical
stays until it is clicked.

What omorfos itself has to say is wrong, such as a mistake in a
configuration file, is shown here too, headed `omorfos` and looking as a
critical notification does. It is given up to six lines, stays until you
click it away or put it right, and is never the one to go when there are
too many. What has merely happened and needs nothing done, such as the X
server having been started again, is shown the same way but looking as
an ordinary notification does, and goes by itself like one. If you turn
these notifications off, or use another program for them, the compositor
shows both in a notice of its own.

A picture may be a PNG or an SVG, given by its path, or an icon by name.
Icons are looked for in the `hicolor` and `Adwaita` themes; where a theme
has one only as a shape, it is drawn in the colour of the text.

`omorfos-notifications` is a program of its own, started with the session,
and started again if it crashes. If it stops more than three times in a
minute, as it will when another program is already showing notifications,
omorfos leaves it stopped and says so. It answers to the usual desktop notification service, so `notify-send` and
applications work with it as they are. It needs the session's D-Bus, which
a session started from a login manager has.

How notifications look is in `theme.css`, under `notification`,
`notification-title` and `notification-body`; a critical one is
`notification:urgent`. Their width is `min-width`, and `margin` is the
room between them and around them.

Only one program can show notifications. To use another, or none, turn this
one off in `config.yaml`:

```yaml
notifications:
  enabled: false
```

Like `autostart`, this takes effect the next time you log in.

## Locking the screen

omorfos does not lock by itself; it lets a screen locker do it, and tells
an idle daemon when you have been away. With `swaylock` and `swayidle`
installed, for example:

```yaml
bindings:
  - keys: Super+l
    action: { spawn: [swaylock] }
  - keys: Super+Shift+i
    action: { spawn: [swayidle, -w, timeout, "300", swaylock -f] }
```

The second starts the daemon that locks after five minutes. To have it
running from login, list it under `autostart` in `config.yaml` instead. While the screen is locked nothing else
is shown and no key binding works, except Ctrl+Alt+F1 to F12.

## Programs written for X11

Programs that have not been written for Wayland run through `Xwayland`,
which omorfos starts with the session if it is installed. Their windows
are windows like any other: they get a frame, can be moved, resized,
maximized, grouped as tabs and matched by rules, where `app_id` is the
window's class. Programs started from omorfos are told where the X server
is, in `DISPLAY`.

If the X server stops, the programs using it close, since they cannot
outlive it, and omorfos starts another in its place at once and says so
in the notice. Programs you start afterwards find it where the last one
was. An X server that stops more than three times in a minute is given up
on until you next log in.

What is copied in one kind of program can be pasted in the other, and so
can what is selected and pasted with the middle button. As between
programs written for Wayland, a program is given what was copied only
while it has the keyboard.

X11 has no notion of a screen with a scale, so on one something has to
give, and `xwayland.scaling` says what:

- `stretched`, which is the default, has them draw at one pixel to a unit
  and stretches what they draw. All of them are the right size, and
  blurred.
- `native` has such programs draw with as many pixels as the screen has,
  and tells them how dense it is. Those that take that in are sharp and
  the right size: programs made with GTK or Qt, and browsers. Those that
  do not are sharp and small, by as much as the screen is dense: `xterm`
  with its usual font is one, and on a screen at 2.5 it is two fifths of
  the size it should be.

Under `native`, with several screens, they draw for the densest and are
scaled down on the others; a program that is already running when the
scale changes may not notice until it is started again; and over a window
of such a program the pointer may be drawn smaller than elsewhere.

Programs written for X11 can read each other's windows and what is typed
into each other, which programs written for Wayland cannot do to anything.
To do without them:

```yaml
xwayland:
  enabled: false
```

This takes effect the next time you log in. If it is on and `Xwayland` is
not installed, omorfos says so in the notice and carries on.

## Using windows

Windows that support it get a titlebar and border drawn by omorfos.
Applications that insist on drawing their own, such as GTK ones, are left
alone; a rule with `decorations: true` forces a frame onto them anyway.

| To | Do this |
|---|---|
| Move a window | Drag its titlebar, or hold Alt and drag anywhere on it |
| Resize a window | Drag a border or corner, or hold Alt and drag with the right button |
| Group windows as tabs | Drag a titlebar with the middle button onto another titlebar |
| Switch tab | Click the tab |
| Ungroup a tab | Drag it with the middle button and drop it off any titlebar |
