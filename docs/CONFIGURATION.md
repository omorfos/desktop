# Configuration

omorfos reads YAML files from `$XDG_CONFIG_HOME/omorfos/`, which is usually
`~/.config/omorfos/`. Every file is optional. Changes are picked up as soon
as you save.

If a file has a mistake, omorfos keeps using that file's previous contents
and reports the problem with its file, line and column. The other files
still apply. Problems are shown in a notice at the top right of the screen
(click it to dismiss) and printed by `omorfosctl config check`.

General rules:

- Unknown settings are errors, so typos are caught.
- Booleans are `true` or `false` only.
- Colors are `"#rrggbb"` or `"#rrggbbaa"`, quoted.
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
autostart: []             # programs to start with the session
```

The values shown are the defaults, except `workspaces`, which defaults to
`["1", "2", "3", "4"]`.

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

With more than one screen, they are placed side by side from left to
right in the order they were plugged in, a laptop's own first. There is no
setting for the arrangement yet.

`output.scale` is how large everything is drawn, on every screen. With `auto`, omorfos
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
| `workspace: N` | Switch to workspace N (from 1). |
| `next_workspace`, `previous_workspace` | Cycle workspaces. |
| `move_to_workspace: N` | Send the focused window to workspace N. |
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

## `theme.yaml`

```yaml
font: { family: sans-serif, size: 11 }      # size 6 to 72
border: { width: 1, radius: 8 }             # width 0 to 16, radius 0 to 32
titlebar: { height: 28 }                    # 16 to 64
animations: { enabled: true, duration: 150 } # milliseconds, 0 to 1000
cursor: { theme: default, size: 24 }        # size 8 to 128
colors:
  background: "#14161b"
  accent: "#5b9cf5"
  focused: { border: "#5b9cf5", titlebar: "#23262e", text: "#e6e8ee" }
  unfocused: { border: "#2c303a", titlebar: "#1b1d23", text: "#8b90a0" }
```

These are the defaults. Anything you leave out keeps its default, except
that `focused` and `unfocused` must each give all three colors.

`cursor.theme` names a cursor theme installed in `~/.local/share/icons`,
`~/.icons` or `/usr/share/icons` (or the directories in `XCURSOR_PATH`).
Shapes the theme lacks come from the themes it inherits from and then from
`default`. If no theme has a shape, omorfos draws a plain arrow of its own.

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
