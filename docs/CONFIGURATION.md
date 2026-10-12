# Configuration

omorfos reads YAML files, and a stylesheet for the theme, from
`$XDG_CONFIG_HOME/omorfos/`, which is usually `~/.config/omorfos/`. Every file is optional. Changes are picked up as soon
as you save.

If a file has a mistake, omorfos keeps using that file's previous contents
and reports the problem with its file, line and column. The other files
still apply. Problems are shown in a notice at the top right of the screen
(click it to dismiss) and printed by `omorfosctl config check`.

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

* {
  font-family: sans-serif;
  font-size: 11pt;
}

desktop { background: var(--background); }

/* A window's frame. Its sizes are taken from the rule without a state. */
window {
  border-width: 1px;
  border-color: var(--border);
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

/* A title in a titlebar that has several. The border is the line between
 * one tab and the next. */
tab {
  background: var(--surface-dim);
  color: var(--text-dim);
  border-width: 1px;
  border-color: var(--border);
}
tab:focus { border-color: var(--accent); }
tab:active {
  border-bottom-width: 2px;
  border-bottom-color: var(--border);
}
tab:active:focus {
  background: var(--surface);
  color: var(--text);
  border-bottom-color: var(--accent);
}

menu {
  background: var(--surface);
  color: var(--text);
  border-width: 1px;
  border-color: var(--border);
}
menu-item {
  height: 28px;
  color: var(--text);
}
menu-item:hover {
  background: var(--accent);
  color: var(--background);
}
menu-separator { color: var(--border); }

notice {
  background: var(--surface);
  color: var(--text);
  border-width: 1px;
  border-color: var(--accent);
}
notice-heading { color: var(--accent); }

panel {
  height: 28px;
  background: var(--surface-dim);
  color: var(--text-dim);
}
panel-button {
  background: var(--surface-dim);
  color: var(--text-dim);
}
panel-button:active {
  background: var(--surface);
  color: var(--text);
  border-bottom-width: 2px;
  border-bottom-color: var(--accent);
}
clock { color: var(--text); }
```

The quickest change is to give the built-in theme's variables other
values:

```css
:root {
  --accent: #e0af68;
  --background: #101014;
}
```

**Parts** are `desktop`, `window`, `titlebar`, `tab`, `menu`, `menu-item`,
`menu-separator`, `notice`, `notice-heading`, `panel`, `panel-button` and
`clock`. `*` is every part. Several can share a rule: `tab, clock { ... }`.

**States** follow a part: `:focus` (it has the keyboard, or its window
has), `:active` (the tab or workspace being shown, or the window with
focus on the panel) and `:hover` (the pointer is over it). A part can be in
several: `tab:active:focus`.

**Properties:**

| Property | Value | Notes |
|---|---|---|
| `background` | a color | also written `background-color` |
| `color` | a color | text; for `menu-separator`, the line |
| `border-color`, `border-width` | a color; `0` to `16px` | the line around a part; for `tab`, the line between tabs |
| `border-bottom-color`, `border-bottom-width` | a color; `0` to `16px` | a line along the bottom, inside the part |
| `border-radius` | `0` to `32px` | of `window` |
| `height` | `16px` to `64px` | of `titlebar`, `menu-item` and `panel` |
| `font-family`, `font-size` | a name; `6pt` to `72pt` | a generic name such as `sans-serif` works |

Colors are `#rgb` or `#rrggbb`, with opacity as a fourth digit or pair, or
`transparent`. Sizes are whole numbers. A variable is declared in `:root`
with a name beginning `--` and used as `var(--name)`; it can stand for
another variable.

The sizes of a window's frame and titlebar are taken from the rules without
a state, since a window does not change size when it gains focus.

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

It is a program of its own, started with the session. How it looks is in
`theme.css`, under `panel`, `panel-button` and `clock`, and it follows the
file as you save it.

To go without it, or to use another panel that speaks layer-shell, such as
`waybar`, turn it off in `config.yaml`:

```yaml
panel:
  enabled: false
autostart:
  - [waybar]
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
