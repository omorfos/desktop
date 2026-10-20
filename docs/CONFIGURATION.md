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
workspaces: [web, code, chat, media]   # 1 to 32 that are always there
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
  natural_scroll: true    # content follows the fingers
  speed: 0                # -1 slowest to 1 fastest
  acceleration: adaptive  # adaptive | flat
mouse:
  natural_scroll: false   # content follows the wheel
  speed: 0                # -1 slowest to 1 fastest
  acceleration: adaptive  # adaptive | flat
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
screenshots:
  destination: file       # file | clipboard | both
clipboard:
  enabled: true           # keep what is copied after its program closes
portals:
  enabled: true           # tell applications how the desktop looks
xwayland:
  enabled: true           # run programs written for X11
  scaling: stretched      # stretched | native, on a screen with a scale
```

The values shown are the defaults, except `workspaces`, which defaults to
`["1"]`.

`idle.screen_off` turns the screens off after that many seconds without
input, and on again at the next key or movement. Something that asks for
the session to stay awake, such as a film being played, keeps them on. It
is off by default.

`autostart` lists programs to start when the session does, each as a
program and its arguments, as for `spawn`:

```yaml
autostart:
  - [swayidle, -w, timeout, "300", omorfosctl action lock]
  - { run: [waybar], may: [panel] }
```

A program written with `may` is let do what an ordinary program is not;
see "Who may do what" below.

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
  - { name: HDMI-A-1, scale: 1.25, mode: 2560x1440@144 }
  - { name: eDP-1, place: below, offset: -30% }
```

The screens listed stand in that order, each to the right of the one
listed before it unless its `place` says `left`, `above` or `below`: side
by side their tops are in line, and one above or below another is in the
middle of it. `align` says otherwise: `start` lines up the tops, or the
left sides, `middle` the middles and `end` the bottoms, or the right
sides. `offset` then moves it along the edge the two share, in units of
layout, such as `-300`, or as a share of its own length along that edge,
such as `-30%`: less than nothing to the left, or up. The first listed
has none before it, and you are told if it is given a `place`, an
`align` or an `offset`.

A screen turned on its stand is said with `rotate`, as `xrandr --rotate`
says it: `left`, with the top of the picture on the left as the screen
was made, `right`, or `inverted`; `normal` is as it was made. It then
stands as tall as it was wide, and what it shows, and pictures of it,
are the right way up. Inside another desktop, in a window, `rotate` is
not used. Those that are connected but not listed stand to the right of
all the rest, and each listed may have a `scale` of its own in
place of `output.scale`, and a `mode`: the size it is driven at in
pixels, and how many times a second it is drawn if that matters. Without
a rate, the fastest the screen offers at that size; without a mode, what
the screen says suits it best. A mode the screen does not offer is not
used, and what it is driven at instead is said among the problems, as a
mistake in this file is, until the mode is put right. A change of mode
takes effect as soon as the file is saved. `omorfosctl screens` lists
the modes each screen offers, written as `mode` takes them. A screen's name is the `output` shown by
`omorfosctl state`. One that is listed but not plugged in is skipped.

The workspaces listed are always there. More are made as they are wanted,
after those: by `Super+n`, by sending a window to a new one
(`Super+Shift+n`, or "Send to workspace" in the menu of its titlebar), or
by going to the number one past the last, so that `Super+2` in a session
with one workspace makes a second. A workspace that was made is called by
its number, and goes again once nothing is on it and another is being
shown. A screen has 32 at most.

A window made fullscreen, by the second button of its titlebar, by
`Super+f`, or because its program asked, goes alone to a workspace made
for it, directly after the one it was on, with no panel over it and no
titlebar. That workspace has no number: `Super+Right` and `Super+Left` go
along the whole row and reach it, and the number keys count desktops only.
It goes when the window closes or leaves fullscreen, which takes the
window back where it was. A dialog that belongs to it opens over it; any
other window, another of the same program included, opens on the desktop
it came from. That desktop is shown with it if you asked for it, by
starting it from omorfos (the launcher, a key, the menu) or from a
program that says you did; otherwise it waits there, in the taskbar,
and you stay where you were. Maximizing is another thing: it fills the desktop the window
is on and keeps the panel and the titlebar.

Up to four windows can share such a workspace. "Tile" in the menu of a
titlebar, or the `tile` action, has a window take a half of where it is,
and the next window to open there has the other half: the launcher, used
there, puts a program beside the one already there. From a desktop,
"where it is" becomes a new workspace. A window that already has a half
can take a half of that, the other way, which makes room for a third; and
the same on the other side makes room for a fourth. Nothing is cut more
than twice, so the arrangements are two side by side or one above the
other, one beside two, one above or below two, and four in quarters.
Asked for a half it cannot take, a window goes to that half instead,
changing places with what was there.

Rest the pointer on the second button of a window's titlebar and small
pictures of the seven arrangements come up under it, with an arrow to
the button they belong to and a line of words that says what the part
under the pointer is. Click a part of one
and a workspace is made cut up that way, with the window in that part and
the others waiting for windows to open in them. A click on the button
itself is still for fullscreen.

Or drag a window by its titlebar until the pointer is against an edge of
the screen. The half it would take is washed over while it is held:
left or right at the sides, top or bottom at the top and bottom, and a
quarter near a corner. Let go there and it is put in that part of a new
workspace. Drag it away from the edge again and nothing happens. An edge
with another screen beyond it is the way to that screen and offers
nothing.

A window that is open already is put beside a fullscreen one, or into a
part that has nothing in it, from the menu of its own titlebar: under
"Send to workspace", each workspace that has room is listed as "Beside"
and the windows there. Beside a fullscreen window it takes the right
half. You are taken there with it.

`Super+Ctrl` and an arrow gives focus to the window beside the one that
has it, and `Super+Ctrl+Shift` and an arrow changes the two over. With the
pointer, drag a window by its titlebar onto another part: the part is
washed over while it is held, and let go there the two change places.

Windows that share a workspace keep their titlebars, each across the top
of its own part, so that which has focus can be seen and each can be
closed, sent back to the desktop by its second button, or dragged. Only
the panels are hidden. A window that has a workspace to itself has
nothing over it, as fullscreen means.

There is a line wherever two parts meet: drag it to give one more room
and the other less, down to a sixth of the screen or to the least a
window says it can be, and double-click it to put it back in the middle.
The theme draws it as `tile-line`, in a window's border colour unless
told otherwise. When a window closes or leaves, the one beside it has
its room. A window that opens when every part is taken opens on the
desktop.

A part with nothing in it shows the windows of the desktop the workspace
was made from, small, each in a box with its title under it, which the
theme draws as `offer`: its border round the box, and its colour and font
for the title, `offer:hover` under the pointer. The pictures of a row
stand on one line and their titles are on another. Click anywhere in a
box to put its window there. A window a rule makes `private` is shown as
a blank, in the colour of the `capture` part, with its title under it all
the same, since that is how it can be told.

On such a workspace, rest the pointer against the top of the screen for
six tenths of a second, or hold it there and push on against the top a
good way, and whatever panels are along the top come back at once. A
quick flick at the top and away brings nothing. Where
one window has the workspace to itself its titlebar comes back under
them: its buttons, its menu, and the second button to leave fullscreen. Against the bottom, the panels along the
bottom come back. They are laid over the window, which does not change
size. They stay while the pointer is on them or still moving, so that a
titlebar further down the screen can be reached, and go a second after
the pointer has stopped somewhere else, or when a key is pressed or a
button is pressed in a window. This is
omorfos's doing and no program can turn it off or imitate it, so it is
also how to tell a fullscreen program from the desktop.

Each screen has its own workspaces, and shows one of them: switching
workspace on one screen leaves the others as they are. A window dragged
onto another screen joins the workspace that screen is showing. When a
screen is unplugged, its windows move to the workspace of the same number
on the first screen that is left, which is made if it is not there.

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
| `maximize`, `minimize` | Toggle or apply that state. |
| `fullscreen` | Put the focused window alone on a workspace made for it, filling the screen, or bring it back to the desktop it came from. |
| `tile: left` | Have the focused window take the left half of where it is; also `right`, `top` and `bottom`. The next window to open there has the other half. From a desktop, where it is becomes a workspace made for it. |
| `focus: left` | Among windows that share a workspace, give focus to the one beside the focused one; also `right`, `up` and `down`. |
| `swap: left` | Among windows that share a workspace, change the focused one over with what is beside it; also `right`, `up` and `down`. |
| `shade` | Roll the focused window up into its titlebar, or let it down. |
| `keep_above` | Keep the focused window above the others, or stop. |
| `stick` | Have the focused window on every workspace of its screen, or only the one it is on. |
| `focus_next`, `focus_previous` | Cycle windows. |
| `next_tab`, `previous_tab` | Cycle tabs in the focused group. |
| `workspace: N` | Switch to workspace N (from 1) on the screen the pointer is on. One past the last is made first; further than that does nothing. |
| `new_workspace` | Make a workspace after the last on the screen the pointer is on, and switch to it. |
| `next_workspace`, `previous_workspace` | Cycle workspaces on the screen the pointer is on. |
| `move_to_workspace: N` | Send the focused window to workspace N of its screen. One past the last is made first. |
| `move_to_new_workspace` | Send the focused window to a workspace made for it, after the last. |
| `join_workspace: ID` | Put the focused window beside those sharing a workspace made for windows, and show it. The `ID` is the one `omorfosctl state` gives the workspace. The titlebar's menu offers the same by name. |
| `move_to_next_output` | Send the focused window to the next screen. |
| `show_menu` | Open the root menu. |
| `lock` | Lock the session, with the program `locker` in `config.yaml` names. |
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
  - match: { title: "*Bank*" }
    private: true
  - match: { app_id: "*notes*" }
    above: true
    sticky: true
```

A rule needs `app_id`, `title` or both; `*` stands for any run of
characters. Every matching rule applies, in order. Properties are
`workspace`, `maximized`, `fullscreen`, `decorations`, `private`, and
`shaded` (rolled up into its titlebar), `above` (kept above the windows
that are not) and `sticky` (on every workspace of its screen), which are
what the titlebar's menu changes too. A window without a titlebar is not
rolled up.
Without a `decorations` rule, a window is framed if it asks omorfos to
decorate it.

The others are looked at when a window opens. `private` holds for as
long as the window matches: a window whose title comes to match is
private from then on, and one whose title no longer does is not. A
private window is left blank in any picture a program takes of the
screen, titlebar and all, and so is a window that shares its frame as a
tab. You see it as ever.

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
  --capture: #f0524f;      /* the screen is being captured */
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

/* The line between windows that share a workspace. */
tile-line { color: var(--border); }

/* A window offered in a part with nothing in it: the box round its picture
 * and its title, and the title, brighter under the pointer. */
offer {
  color: var(--text-dim);
  border: 1px solid var(--border);
}
offer:hover { color: var(--text); }

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

/* The buttons at the left of a titlebar: close, maximize, minimize. Each
 * is a box in its share of the titlebar, as wide as its min-width with its
 * margin around it, and its color is that of the sign drawn on it. They
 * are :focus with their frame and :hover under the pointer. Give one a
 * min-width of 0 to do without it. */
button-close, button-maximize, button-minimize {
  min-width: 12px;
  margin: 8px 4px;
  border-radius: 6px;
  background: var(--border);
  color: transparent;
}
button-close { margin-left: 10px; }
button-close:focus, button-close:hover { background: #e06c75; }
button-maximize:focus, button-maximize:hover { background: #98c379; }
button-minimize:focus, button-minimize:hover { background: #e5c07b; }
button-close:hover, button-maximize:hover, button-minimize:hover {
  color: var(--background);
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

tooltip {
  padding: 6px 10px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 6px;
}

/* The mark shown while a program takes pictures of the screen. */
capture {
  background: var(--background);
  color: var(--capture);
  border-radius: 8px;
}

/* The line round a screen or a window while it is shared or recorded,
 * which is in no picture of it. */
capture-border { border: 2px solid var(--capture); }

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

/* What the portals ask: whether an application may see the screen. The
 * button that gives is :active. The border is also the ring of the loupe
 * a colour on the screen is picked with. */
question {
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--accent);
  border-radius: 8px;
}
question-detail { color: var(--text-dim); }
question-button {
  background: var(--surface-dim);
  color: var(--text);
  border-radius: 6px;
}
question-button:active {
  background: var(--accent);
  color: var(--background);
}

/* The screens shown still, to pick what a picture is to be of: what is not
 * picked is dimmed by the background, and what is drawn has the border. */
picker {
  background: #00000066;
  border: 2px solid var(--accent);
}
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

**Parts** are `desktop`, `window`, `tile-line`, `offer`, `titlebar`, `tab`, `button-close`,
`button-maximize`, `button-minimize`, `menu`, `menu-item`,
`menu-separator`, `notice`, `notice-heading`, `capture`, `capture-border`, `tooltip`, `panel`,
`workspace`, `task`,
`clock`, `launcher`, `launcher-input`, `launcher-item`,
`launcher-description`, `launcher-backdrop`, `notification`,
`notification-title`, `notification-body`, `question`, `question-detail`,
`question-button` and `picker`. `*` is every part. Several can share a rule:
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
| `color` | a color | text; for `menu-separator` and `tile-line`, the line |
| `opacity` | `0` to `1`, or a percentage | of everything the part draws |
| `border` | a width, `solid` or `none`, a color | any of them, in any order |
| `border-color`, `border-width` | a color; `0` to `16px` | |
| `border-radius` | `0` to `32px` | rounds the corners of the box |
| `padding`, `margin` | one to four sizes, `0` to `64px` | all; top and bottom, then sides; top, sides, bottom; or top, right, bottom, left |
| `height` | `16px` to `64px` | of `titlebar`, `menu-item`, `panel`, `launcher-input` and `launcher-item` |
| `min-width`, `max-width` | `0` to `4096px` | of `workspace`, `task` and `clock`; `max-width: 0` is no limit. The launcher is as wide as its `min-width`, and so is each button of a titlebar, which is not there at all if that is `0` |
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
  the screen), by `workspace`, `task` and `clock` (room around each on
  the bar), and by the buttons of a titlebar (room around each in the
  titlebar, which is how one is made smaller than the titlebar is tall).
- The buttons of a titlebar are drawn from `background`, `border`,
  `border-radius` and `color`, which is the colour of the sign on each: a
  cross, a plus and a bar. They stand at the left in that order.
- `padding` is honoured by `tab`, `menu-item`, `notice`, `panel`,
  `workspace`, `task` and `clock`.
- `border-radius` is honoured by `window`, `tab`, `menu`, `menu-item`,
  `notice`, `capture`, `panel`, `workspace`, `task` and `clock`.
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
(`Super+Escape` by default). Click an entry to choose it; click elsewhere or
press Escape to close the menu.

The keyboard steers it too, and so the menu a right click on a titlebar
opens: Up and Down go through the rows and round, Right or Enter goes into
a submenu, Left comes back out of one, and Enter chooses an entry. While a
menu is open these keys are the menu's and no window sees them; with a
modifier held they are a binding's as ever.

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

`Super+Space`, or **Run…** in the root menu, opens the launcher: a box in the
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

## What applications are told of the desktop

Applications ask the desktop some things through the portals
(`xdg-desktop-portal`), and omorfos answers what only it can. So far that
is how the desktop looks: whether it is light or dark, and the colour
that picks things out. An application that follows the desktop, as most
written with GTK or Qt do, is then dark when your theme is and light when
it is.

An application can also ask for a picture of the screen, as a browser or
a chat in a sandbox does. Each time, a question goes up in the middle of
the screen, over everything, saying which application asks, by the name
its desktop entry gives it, or by what its sandbox calls it if it has
none. Escape or **Cancel** gives it nothing. Enter or **Take picture**
takes a picture of every screen, and then you pick what of it the
application is given, as `Shift+Print` picks: drag to draw a region,
click a window to take it whole or anywhere else to take that screen,
Enter to take every screen, as one, and Escape or another button to give
it nothing after all. What is picked is
saved among your pictures as `Print` saves them. The question is not in
the picture, and the red mark shows while it is taken. The question is drawn as `question`, what giving would let
the application see as `question-detail`, and the buttons as
`question-button`, the one that gives `:active`.

An application can ask for a colour on the screen too, as a drawing
program's colour picker does. Nothing is asked: a still picture of each
screen goes up over it, and a click, or Enter, gives the application the
colour under the pointer, on whichever screen it is, and nothing else; Escape or
another button gives it none. The pointer is a crosshair meanwhile, and
a round loupe beside it shows the pixels round it larger, with the one
it would give outlined in the middle.

An application can ask to share the screen too, as a browser or a call
does. The same question goes up, saying that it wants to share your
screen: **Cancel** or Escape gives it nothing, and **Share** or Enter
gives it the screen as a live stream, through PipeWire, for as long as
the application keeps it. With more than one screen, or if the
application can take a window as well, you then pick what: the screens
go still and dim but for what is under the pointer, a window or, beside
every window, the screen, and a click or Enter shares that, Escape
nothing; Tab reaches a window behind another, as it does for a picture.
Drag to share a region of a screen instead: the application is told it
is a screen as large as the region, and if the screen changes size the
sharing ends.
A window is shared by itself, whatever covers it, at its size
as that changes, with the pointer drawn in or sent beside where it is
over the window, as the application asks. While anything is
shared the red mark stays on its screen. To stop sharing, stop it in the
application.

How the desktop looks is read from the theme, so there is nothing to set. The desktop is
dark if menus are written in something lighter than what they are
written on, and the colour is the one the row of a menu under the
pointer is lit with. Change `theme.css` and applications are told at
once.

This needs `xdg-desktop-portal` installed, and the two files that tell it
of omorfos, which a package of omorfos puts in place. What omorfos does
not answer is left to `xdg-desktop-portal-gtk` if that is installed,
choosing a file above all. To have another program answer in omorfos's
place, or none:

```yaml
portals:
  enabled: false
```

Like `autostart`, this takes effect the next time you log in. Run inside
another desktop, in a window, omorfos leaves the portals to that desktop.

## The clipboard

What you copy is kept after the program you copied it from closes, by
`omorfos-clipboard`, which omorfos starts with the session. It reads what
is copied, in every form the program offers it in, as soon as it is
copied, and offers it in the program's place.

Some things are left with the program that copied them, and go when it
closes: what a password manager copies, which it marks as secret, so that
it goes when the password manager takes it away; what is only selected,
for the middle button; and what is larger than 64 MiB, or not handed over
in any form within two seconds. To keep nothing, or have another clipboard manager do
it, with `may: [clipboard]`:

```yaml
clipboard:
  enabled: false
```

Like `autostart`, this takes effect the next time you log in.

## Who may do what

A program sees and works its own windows. Some things reach further, and
are only for a program omorfos started itself and was told may do them:

| Role | Lets a program |
|---|---|
| `panel` | Put surfaces on the screen outside windows, as a bar, a dock, a wallpaper or a notification does, and be told what windows there are |
| `overlay` | The same, and take the keyboard from whatever has it while one of its surfaces asks, as a launcher does |
| `lock` | Lock the session |
| `capture` | Take pictures of the screen |
| `clipboard` | Read what is copied and set it at any time, not only while one of its windows has the keyboard, as a clipboard manager does |

`ask`, `input` and `screens` are names for what is
to come, and are accepted so that a configuration can say them already.

A role is given where the program is started: in `autostart`, or in a
`spawn` bound to a key or put in the menu.

```yaml
# config.yaml
autostart:
  - { run: [waybar], may: [panel] }
  - { run: [swaybg, -i, wall.png], may: [panel] }
```

```yaml
# keys.yaml
bindings:
  - keys: Super+d
    action: { spawn: { run: [fuzzel], may: [overlay] } }
```

A program started any other way, from a terminal say, is an ordinary
program whatever it is called. One that asks for what it has no role for
is refused, and you are told once what asked and what to write to let
it. The panel, the launcher and the notifications that come with omorfos
have what they need; started by hand, they are ordinary programs too.

A program in a sandbox, such as one installed with Flatpak, is held to
less again. It is not shown that panels, locking, pictures of the screen,
the clipboard at any time or the list of windows exist, so it cannot ask, and no setting gives it a role. Nothing needs
to be set up for this: the sandbox tells omorfos which programs are
inside it.

This keeps a program that is merely running from putting a false panel
or a false lock screen in front of you. It is not a wall against a
program that can already change your files: such a program could write
itself into `autostart`.

## Pictures of the screen

`Print` takes a picture of every screen and saves each as a PNG in a
folder called `Screenshots` among your pictures: `~/Pictures`, or wherever
`~/.config/user-dirs.dirs` says pictures go. A picture is named after
when it was taken, and after its screen when there is more than one. One
that is there already is never written over.

`Alt+Print` takes the window with focus by itself, as large as it is and
without the frame omorfos draws round it.

`Shift+Print` lets you pick what the picture is of first, and puts it on
the clipboard, to be pasted, and saves nothing. The screens go still and
dim, the pointer a crosshair, and the window under the pointer is
outlined: click it to take it whole, as `Alt+Print` does, drag to draw a
region, click where there is no window to take that screen, press Enter
to take every screen, side by side, or Escape or another button to take
nothing. A region or a screen is cut from the screens as they were when
you pressed the key; a window is taken as it is when you click, the parts
of it others covered too. A window behind another is reached with Tab:
each press outlines the next window down under the pointer, shown as it
is over what covers it, and round again to the top, and moving the
pointer starts from the top. The dim is the theme's `picker` part, and a
line at the bottom of each screen, drawn as `tooltip`, says what to do. To have `Print` put its picture on the clipboard, or both, say so
in `config.yaml`:

```yaml
screenshots:
  destination: both       # file | clipboard | both
```

It is taken by `omorfos-screenshot`, which the keys start as a program
that may:

```yaml
# keys.yaml
- keys: Print
  action: { spawn: { run: [omorfos-screenshot], may: [capture, clipboard] } }
- keys: Shift+Print
  action:
    spawn:
      run: [omorfos-screenshot, --pick, --to, clipboard]
      may: [capture, clipboard, panel, overlay]
- keys: Alt+Print
  action:
    spawn:
      run: [omorfos-screenshot, --window]
      may: [capture, clipboard, panel]
```

`--to file`, `--to clipboard` or `--to both` says where one picture goes,
whatever the configuration says, `--window` that it is of the window
with focus, and `--pick` that you pick what it is of, which needs the
`panel` and `overlay` roles to be put over the screen. Where the windows
are to be picked from is asked at the socket for scripts; a program the
compositor starts is told where that is. `--to` can be said
with either of the other two. A picture of a window is the
whole of it, however large, and see-through where the window is, as the
shadow some programs draw round their windows is. A program says which window it
wants a picture of by the window's entry in the list of windows, so one
that takes pictures of windows needs the `panel` role as well, to be
given that list. A picture put on the clipboard is kept by
`omorfos-clipboard`, and `omorfos-screenshot` stops as soon as it has
handed it over. With the clipboard not kept, it is given by
`omorfos-screenshot` itself each time it is pasted, which stays running,
unseen, until you copy something else.

Any program that speaks `ext-image-copy-capture-v1` can be let take
pictures the same way, with `may: [capture]`. Started without it, it is
given none, and you are told once what asked.

While a program is taking pictures of a screen, and for a moment after, a
red dot is shown in the top right corner of that screen, under
any panel along the top. It is drawn by
omorfos over everything else, it is not in the pictures, and nothing can
turn it off: its colours can be changed, with the `capture` part of the
theme. So a picture cannot be taken of your screen without your seeing
that it was. Rest the pointer on it to be told which programs are taking
pictures, of the screen or of a window on it, by the name of each
program's file. It is themed as `tooltip`.

While the screen is shared or recorded, a red line goes round it as well,
which is no more in what is shared than the dot is; while a window is, the
line goes round that window's frame, under any window over it. It is the
`capture-border` part's border; `border: none` takes it away, and the dot
stays.

The same mark shows a microphone while something records sound, and a
camera while something records from one, on every screen, beside the dot
for the screen, as `omorfos-recording` finds them in PipeWire. Its
tooltip names each program and what it records, and a program a screen
is shared with through the portal is named as that, with the portal. A
program that opens a camera itself, and not through PipeWire, as Firefox
does, is seen by having it open, for as long as it does.

A window that a rule in `rules.yaml` calls `private` is blank in every
picture, of the screen or of itself, in the background colour of the
`capture` part of the theme. A window that is out of sight, minimized or
on another workspace, can have its picture taken all the same.

Nothing is given while the session is locked. The pointer is in a picture
only if the program asks for it, and `omorfos-screenshot` does not.

## Locking the screen

omorfos does not lock by itself; it starts a screen locker to, and tells
an idle daemon when you have been away. Name the locker in `config.yaml`,
and the `lock` action starts it, as the one program let lock:

```yaml
# config.yaml
locker: [swaylock, -f]
autostart:
  - [swayidle, -w, timeout, "300", omorfosctl action lock]
```

```yaml
# keys.yaml
bindings:
  - keys: Super+l
    action: lock
```

The idle daemon asks omorfos to lock and does not start the locker
itself: a locker it started would be an ordinary program, and refused.
While the screen is locked nothing else is shown and no key binding
works, except Ctrl+Alt+F1 to F12.

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
| Close a window, make it fullscreen, or minimize it | Click the first, second or third button at the left of its titlebar |
| Put two windows side by side | Rest the pointer on the second button of one's titlebar and click a half of the first picture, then open the other. Or right-click the titlebar and choose Tile |
| Put a window in a half or a quarter of the screen | Drag it by its titlebar to an edge of the screen, or to an edge near a corner, and let go |
| Give one of two windows side by side more room | Drag the line between them; double-click it to halve the screen again |
| Leave fullscreen | `Super+f`; or rest the pointer against the top of the screen and click the second button of the titlebar that comes back |
| Reach the panel from a fullscreen window | Rest the pointer against the edge of the screen the panel is along |
| Maximize a window, or put it back | Double-click its titlebar |
| Roll a window up into its titlebar, keep it above the others, show it on every workspace, or send it to one | Right-click its titlebar and choose from the menu |
| Move a window | Drag its titlebar, or hold Alt and drag anywhere on it |
| Resize a window | Drag a border or corner, or hold Alt and drag with the right button |
| Group windows as tabs | Drag a titlebar with the middle button onto another titlebar |
| Switch tab | Click the tab |
| Ungroup a tab | Drag it with the middle button and drop it off any titlebar |
