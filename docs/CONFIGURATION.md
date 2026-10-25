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
  lock: 0                 # seconds without input; 0 never
  sleep: 0                # seconds without input; 0 never
autostart: []             # programs to start with the session
panel:
  enabled: true           # start the panel that comes with omorfos
wallpaper:
  enabled: true           # start the wallpaper that comes with omorfos
  fit: fill               # fill | fit | stretch | center | tile
  every: 600              # seconds each picture of a directory is shown
  fade: 1000              # milliseconds one picture fades into the next; 0 at once
  order: name             # name | shuffled
  screens: []             # particular screens, by name
lock:                     # how omorfos-lock may be unlocked besides the password
  fingerprint: true       # a finger, through the omorfos-lock-fingerprint PAM service
  key: true               # a security key, through omorfos-lock-key
  power: true             # Sleep, Restart, Shut Down and Log Out without unlocking
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
  forget: 0               # seconds what is kept stays; 0 for as long as the session
portals:
  enabled: true           # tell applications how the desktop looks
xwayland:
  enabled: true           # run programs written for X11
  scaling: stretched      # stretched | native, on a screen with a scale
```

The values shown are the defaults, except `workspaces`, which defaults to
`["1"]`.

`idle.screen_off` turns the screens off after that many seconds without
input, and on again at the next key or movement. `idle.lock` locks the
session after that many, with the `locker` named below, and `idle.sleep`
puts the machine to sleep, locking it first. Each happens once for each
time you are away. Something that asks for the session to stay awake, such
as a film being played, holds off all three. They are off by default.

`autostart` lists programs to start when the session does, each as a
program and its arguments, as for `spawn`:

```yaml
autostart:
  - [foot, --server]
  - { run: [waybar], may: [panel] }
```

A program written with `may` is let do what an ordinary program is not;
see "Who may do what" below.

They are started once, in order. Changing the list takes effect the next
time you log in; reloading the configuration does not start them again. A
program that cannot be started is reported like any other problem.

Then, where omorfos is your whole session and not a window inside
another desktop, the programs that other desktops start too: those whose
desktop entries are in `~/.config/autostart`, as programs put themselves
when you ask them to start at login, and in `/etc/xdg/autostart`, as the
system's are installed. One of yours takes the place of the system's of
the same file name, so a copy with `Hidden=true` in it turns the
system's off. An entry that says it is only for other desktops
(`OnlyShowIn`), or not for this one (`NotShowIn=omorfos`), is passed
over, as is one turned off with `X-GNOME-Autostart-enabled=false`, one
that needs a terminal, and one whose `TryExec` program is not installed.
These are ordinary programs; one that needs a role goes in `autostart`
here instead. One that cannot be started is said in the session's log.

Where systemd runs for you, as it does on most systems, each of these and
each of `autostart` is started in a scope of its own, part of the
graphical session, so that it is stopped when you log out, as a program
that does not show windows is otherwise not: one left over would still be
running at your next login, and some refuse to start twice.

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

To have the screens stand one way at a desk and another away from it,
list each way under `arrangements`, as `outputs` is listed: the first
that names just the screens plugged in, in whatever order, is used in
place of `outputs`, and is followed again as soon as a screen is plugged
in or taken out.

```yaml
arrangements:
  - [{ name: eDP-1, scale: 2 }]
  - [{ name: HDMI-A-1, primary: true }, { name: eDP-1, place: below }]
```

One listed screen may be `primary: true`: the notice of what is wrong is
shown on it, and the windows of a screen that is unplugged go to it.
Without one, the first screen plugged in is.

A screen may instead show what another shows, as a projector does a
laptop's, with `mirror` and the other's name:

```yaml
arrangements:
  - [{ name: eDP-1 }, { name: HDMI-A-1, mirror: eDP-1 }]
```

It shows the other's picture as large as fits on it, in the middle, with
black either side where the two are not of a shape. It stands nowhere of
its own: the pointer and windows do not go onto it, and programs are not
told of it, so it has no panel or wallpaper of its own; what was on it
goes to the other. It may have a `mode` and a `rotate`, but not a
`place`, `align`, `offset`, `scale` or `primary`, and a screen it mirrors
cannot mirror another. While the one it mirrors is not plugged in, it
stands as any screen does. `omorfosctl screens` lists it after those that
stand, where the one it mirrors stands.

A screen is turned off with `enabled: false`, as a laptop's own is under
its lid at a desk:

```yaml
arrangements:
  - [{ name: eDP-1, enabled: false }, { name: HDMI-A-1 }]
```

It shows nothing and stands nowhere, and programs are not told of it, as
one unplugged; what was on it goes to the others. It takes no `place`,
`align`, `offset`, `scale`, `primary` or `mirror`, and no screen may
mirror it. If every screen plugged in would be off, the first is left on
and you are told, so that a mistake never leaves you with no screen at
all. `omorfosctl screens` lists it last, as off.

The display key (`XF86Display`, the `next_screens` action) has the
screens stand another way for now, without changing this file: first as
the configuration says, then all showing what the one that comes first
shows, then that one alone, then each of the others alone, and round
again. Each screen keeps its `mode` and `rotate`. In the middle of each
screen that is on, a box says which way they are in, such as "Mirrored:
every screen shows what eDP-1 shows", and asks whether to keep them so:
Enter keeps them, Escape puts them back as they were, and so does
waiting 15 seconds, since it is the key you press when a screen has gone
dark. Pressing the key again goes on to the next way. What is kept is for
the screens plugged in when it was; plugging one in or taking one out
has them stand as this file says again, and so does logging in.

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

Going to another workspace slides it in from the side it lies on: the
next from the right, the one before from the left. The row does not go
round. There is nothing before the first workspace; after the last, if a
window is on it, `Super+Right` makes a new one, and goes no further than
that while it is empty. A swipe towards where there is nothing gives a
little, and goes back.

On a touchpad, three fingers swiped left carry the workspace with them
and bring the next in from the right, and swiped right bring in the one
before. Lift them past half way, or flick, and the one that came in is
shown; lift them short of that, or turn back, and the one that was there
stays. Every other gesture, as a pinch to zoom, is the program's under
the pointer.

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
was made from, small, each in a box with its icon and its title under
it, which the theme draws as `offer`: its border round the box, and its
colour and font for the title, `offer:hover` under the pointer. The icon
is the one the program gives its window, or else its application's, as
in the switcher. The pictures of a row stand on one line and their titles
are on another. Click anywhere in a box to put its window there. A window
a rule makes `private` is shown as a blank, in the colour of the
`capture` part, with its icon large in the middle and its title under it
all the same, since that is how it can be told.

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
  - keys: Super+Return     # in place of the built-in terminal
    action: { spawn: [foot] }
  - keys: Super+l
    action: lock
  - keys: Super+x t        # a chain: Super+x, then t
    action: { workspace: 2 }
unbind: [Super+h]          # a built-in binding done without
```

The bindings you write are made beside the built-in ones, which go on
working on every key you leave alone. One you write on keys a built-in
binding uses takes its place, and so does a chain that begins with them,
as `Super+Return t` would. `unbind` lists built-in bindings to do without,
by their keys; one that names no built-in binding is a mistake, and said
to be. To have none of the built-in bindings and only your own, say
`defaults: false`.

A chord is zero or more modifiers and a key, joined by `+`. Modifiers are
`Ctrl`, `Alt`, `Shift` and `Super` (case does not matter; `Control`, `Mod1`
and `Mod4` also work). Keys are XKB keysym names such as `Return`, `a`, `F1`
or `XF86AudioMute`. A binding names the key, not the symbol it types: write
`Super+Shift+r`, not `Super+R`. A binding whose key name does not exist is
ignored and reported.

Two of your bindings may not use the same keys, and a chain of yours may
not begin with keys you have bound on their own.

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
| `switch_windows`, `switch_windows_back` | Open the switcher, or go on to the next window in it or back. See "Switching windows". |
| `focus_next`, `focus_previous` | Cycle windows, without the switcher. |
| `next_tab`, `previous_tab` | Cycle tabs in the focused group. |
| `workspace: N` | Switch to workspace N (from 1) on the screen the pointer is on. One past the last is made first; further than that does nothing. |
| `new_workspace` | Make a workspace after the last on the screen the pointer is on, and switch to it. |
| `next_workspace`, `previous_workspace` | Go to the next or the previous workspace on the screen the pointer is on. They do not go round: past the last, if a window is on it, a new one is made. |
| `move_to_workspace: N` | Send the focused window to workspace N of its screen. One past the last is made first. |
| `move_to_new_workspace` | Send the focused window to a workspace made for it, after the last. |
| `join_workspace: ID` | Put the focused window beside those sharing a workspace made for windows, and show it. The `ID` is the one `omorfosctl state` gives the workspace. The titlebar's menu offers the same by name. |
| `move_to_next_output` | Send the focused window to the next screen. |
| `show_menu` | Open the root menu. |
| `lock` | Lock the session, with the program `locker` in `config.yaml` names. |
| `stay_awake` | Keep the session from locking, the screens from going off and the machine from sleeping when you are away, until it is chosen again, the session is locked by hand, or you log out. The panel says so while it lasts, and a click there ends it. The root menu offers it. |
| `stay_awake_for: N` | The same for N seconds at most, up to a day (86400). |
| `next_layout` | Type with the next of the layouts `keyboard.layout` lists, and after the last with the first. No key is bound to it. |
| `next_screens` | Have the screens stand the next way: as the configuration says, all showing what the first shows, and each alone; each kept only if Enter is pressed within 15 seconds. Bound to `XF86Display`. |
| `show_launcher` | Open the launcher. |
| `reload_config` | Re-read the configuration. |
| `quit` | End the session. The programs started in it are asked to close first, as any desktop asks them, and given two seconds to; asked again meanwhile, it ends at once. |

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
/* How full something a notification tells of is, such as the volume: the
 * background is the whole, the colour how much of it is full. */
notification-level {
  margin-top: 8px;
  background: var(--border);
  color: var(--accent);
  border-radius: 3px;
}

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
question-button:hover { background: var(--surface); }
question-button:active {
  background: var(--accent);
  color: var(--background);
}
question-button:focus { border: 2px solid var(--text); }
question-input {
  padding: 6px;
  background: var(--background);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 6px;
}
question-input:focus { border-color: var(--accent); }
/* What is typed, when all of it is chosen, as Ctrl+A chooses it. */
question-input:active {
  background: var(--accent);
  color: var(--background);
}

/* The switcher on Alt+Tab: the box in the middle of the screen, each
 * window's place in it, its picture over its title, and the title. The
 * window picked is :active. Each place is as wide as its min-width. */
switcher {
  padding: 12px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
}
switcher-item {
  min-width: 200px;
  padding: 8px;
  border: 2px solid transparent;
  border-radius: 8px;
}
switcher-item:active {
  background: var(--surface-dim);
  border-color: var(--accent);
}
switcher-title { color: var(--text); }

/* The screens shown still, to pick what a picture is to be of: what is not
 * picked is dimmed by the background, and what is drawn has the border. */
picker {
  background: #00000066;
  border: 2px solid var(--accent);
}

/* What dims the session while a prompt asks for a secret, such as the
 * administrator's password: nothing a program draws can dim it so. */
sealed { background: #000000aa; }
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
`notification-title`, `notification-body`, `notification-level`, `question`, `question-detail`,
`question-button`, `question-input`, `switcher`, `switcher-item`, `switcher-title`, `picker`
and `sealed`. `*` is every part. Several can share a rule:
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

## The wallpaper

`omorfos-wallpaper` shows a picture or a colour behind everything on each
screen. Until you name one, it shows Prism, the wallpaper that comes with
omorfos, in `share/backgrounds/omorfos/`: the dark one while the theme is
dark and the light one while it is light, changing as the theme does.
Without those installed, it shows nothing, and the theme's `desktop` is
what is behind the windows.

Four more come with it, each dark and light: Spill, Horizon, Facets and
Ground. Name one as `picture`, as
`/usr/share/backgrounds/omorfos/horizon-dark.png` with its `-light` pair
(below), or the directory itself, to have all five in turn.

```yaml
wallpaper:
  picture: ~/Pictures/hills.jpg
  fit: fill
```

`picture` is a PNG or a JPEG, written as a path from the root or from your
home directory, as `~/`. A photograph is turned the way the camera says it
was held. It may also be a directory of pictures, which are shown in turn,
each for `every` seconds (5 to a week), by their names or, with `order:
shuffled`, in any order, a different one each time through. A picture put
in the directory, or changed in it, is shown at once, and one taken out
while it is shown is followed at once by the next; those in directories
within it are not shown. A single picture named is read again at once when
it changes.

A picture may also be one for a dark desktop and one for a light one,
each a file or a directory, and the wallpaper shows the one that goes with
the theme, changing over when the theme does:

```yaml
wallpaper:
  picture:
    dark: ~/Pictures/night.jpg
    light: ~/Pictures/day.jpg
```

In a directory, two pictures named alike but for `-dark` and `-light` at
the end, as `hills-dark.png` and `hills-light.jpg`, are a pair, and are
shown in turn as one picture, each as the theme is. One without its other
half is shown whatever the theme.

The desktop is dark while the theme writes its menus in a colour brighter
than the one it writes them on, as the built-in theme does; this is what
applications are told too, so that they follow it.

`fit` says how a picture is laid on the screen:

- `fill`: as large as it must be to cover the screen, its shape kept, and
  what does not fit cut off equally on two sides.
- `fit`: as large as fits on the screen, its shape kept.
- `stretch`: made the screen's size and shape.
- `center`: as it is, a pixel of it to a pixel of the screen, in the
  middle.
- `tile`: as it is, again and again from the top left corner.

`color` is the colour wherever the picture does not cover the screen, or
of the whole screen without one; without it, the background of the
theme's `desktop`. Write it in quotes, as `'#203040'`, since YAML reads
what follows a bare `#` as a comment; a `color:` left with nothing after it
is an error that says so.

A screen can show something else, named as `omorfosctl state` names it.
One listed with a `picture` or a `color` shows only its own; one listed
with only a `fit` shows the rest as every screen does:

```yaml
wallpaper:
  picture: ~/Pictures/hills.jpg
  screens:
    - { name: HDMI-A-1, picture: ~/Pictures/sea.png, fit: fit }
    - { name: DP-1, color: '#203040' }
```

It follows the configuration as you save it. A picture that cannot be
read, or is larger than 16384 pixels along a side, is not shown: the
colour is, and why is written to the session's log. A PNG is read a row
at a time and made no larger than the screen needs as it is, so a picture
of 16K takes under a second and little memory. A JPEG, or a PNG that is
interlaced, is read whole first, and may have 67 million pixels at most.

What each screen was given is kept in `~/.cache/omorfos/wallpaper/`, one
file for each screen, as large as the screen's pixels at four bytes each.
When you log in, a screen whose picture has not changed, laid the same way
on a screen of the same size, is shown that at once, without the picture
being read again. A directory starts from the picture kept, if it is still
there. While a directory's pictures are shown in turn, a screen's is kept
again at most every half an hour, so that pictures changing every few
seconds are not each written to the disk. The next picture of a directory
is read half the time it is shown for, or ten seconds, before it is due,
so that it changes the moment it is. Deleting the directory is safe: what
is in it is made again.

A change of picture, colour or fit fades from the old to the new over
`fade` milliseconds, eased in and out: 1000 unless you say otherwise, up
to 10000, and 0 for at once. So does going from the dark picture of a
pair to the light one as the theme changes. The fade begins once the new
picture is ready, so a large one that takes a while to read still fades
in for all of that time. The first picture of a session is shown at
once, as is one drawn again because a screen changed size or scale.

To use another program for it, such as `swaybg`, turn it off and start
that, which needs to be let put a surface behind the windows as a panel
does:

```yaml
wallpaper:
  enabled: false
autostart:
  - { run: [swaybg, -i, /usr/share/backgrounds/hills.png], may: [panel] }
```

Like `autostart`, turning it off or on takes effect the next time you log
in.

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
`notification-title`, `notification-body` and `notification-level`; a
critical one is `notification:urgent`. Their width is `min-width`, and
`margin` is the room between them and around them.

A notification can say how full something is, out of 100, by the usual
`value` hint, as the volume and brightness keys do: it is drawn as a bar
of `notification-level` under what it says. One sent with the hint
`synchronous` (or `x-canonical-private-synchronous`, or
`x-dunst-stack-tag`) takes the place of the one showing with the same
tag, so that holding a key down shows one notification that changes
rather than a pile of them:

```sh
notify-send -h int:value:40 -h string:synchronous:volume Volume
```

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

What is kept can also be forgotten after a while, for passwords copied
from somewhere that does not say they are secret: with `forget: 60`, the
clipboard is emptied a minute after something was copied, even if the
program that copied it is still open, unless something else was copied
since. 0, as it comes, keeps it for as long as the session lasts.

Like `autostart`, these take effect the next time you log in.

A program written for X11 that copies, as `xsel` and `xclip` do for an
editor in a terminal, goes on holding what it copied for the others
written for X11, so that copying and pasting among them works as it does
anywhere. A program written for X11 may read what a program written for
Wayland copied only while one of its windows has the keyboard, so `xsel`
pasting into an editor in a terminal, which has no window, gets nothing
from them: give such an editor `wl-copy` and `wl-paste` (`wl-clipboard`)
instead, as Neovim prefers them where they are installed.

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
| `input` | Be the input method: be shown what is typed into whatever has the keyboard, take the keys, and type as a keyboard would |
| `ask` | Ask for a secret in the sealed prompt: the session dimmed behind it, and the keyboard and pointer its alone, as `omorfos-ask` does |

`screens` is a name for what is to come, and is accepted so that a
configuration can say it already.

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
the clipboard at any time, input methods or the list of windows exist, so it cannot ask, and no setting gives it a role. Nothing needs
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

## The volume, the brightness and what is playing

The keys a keyboard has for these work as they are: louder, quieter and
mute (`XF86AudioRaiseVolume`, `XF86AudioLowerVolume`, `XF86AudioMute`),
the microphone muted (`XF86AudioMicMute`), the screen brighter and darker
(`XF86MonBrightnessUp`, `XF86MonBrightnessDown`), and play or pause,
next, previous and stop. How loud or bright it is then shows as a
notification with a bar, which the next press changes in place.

They run `omorfos-session`, which can be bound to other keys or run from
a script as it is:

```yaml
# keys.yaml
bindings:
  - keys: Super+Up
    action: { spawn: [omorfos-session, volume, up] }
```

| Command | Does |
|---|---|
| `omorfos-session volume up`, `down`, `mute` | The sound that plays five hundredths louder, up to full, or quieter, or muted and back |
| `omorfos-session microphone mute` | The microphone muted, or heard again |
| `omorfos-session brightness up`, `down` | The screen a twentieth brighter or darker, never quite dark |
| `omorfos-session media play-pause`, `next`, `previous`, `stop` | Told to the player that is playing, or else to one that is paused |

The volume is set by WirePlumber's `wpctl`, as other mixers set it, so it
is the same wherever it is looked at; without WirePlumber the volume keys
do nothing. The brightness is set through logind, which lets whoever is
logged in at the machine set it. Players are told over D-Bus, as players
listen for (MPRIS); browsers and most music players do.

The keys do nothing while the screen is locked, and a key held down
changes the level once.

The radios, Wi-Fi, Bluetooth and the rest, are followed as the kernel
knows them, whatever turned them off or on: the key the laptop has for
them, which the kernel acts on itself, a switch on the side, or another
program. Each change is shown as the volume is, "Bluetooth off", and
"Aeroplane mode on" when every radio is off; the panel says "Aeroplane
mode", or which are off, for as long as they are. omorfos binds no key to
them, as the kernel has already done what the key asks.

## Locking the screen

omorfos does not lock by itself; it starts a screen locker to. Name the
locker in `config.yaml`, and the `lock` action starts it, as the one
program let lock, as does being away for `idle.lock` seconds:

```yaml
# config.yaml
locker: [swaylock, -f]
idle:
  lock: 300
```

```yaml
# keys.yaml
bindings:
  - keys: Super+l
    action: lock
```

An idle daemon of your own, as `swayidle`, still works: it is told when
you have been away, and asks omorfos to lock with `omorfosctl action
lock`. It does not start the locker itself: a locker it started would be
an ordinary program, and refused.
While the screen is locked nothing else is shown and no key binding
works, except Ctrl+Alt+F1 to F12. If the locker ends with the screen still
locked, as one that crashed does, omorfos starts it again, up to three
times in a minute; after that the screen stays black and locked, and
`omorfosctl action lock` from another virtual terminal starts it again.

omorfos comes with a lock screen of its own, `omorfos-lock`. Name it as
the locker to use it:

```yaml
# config.yaml
locker: [omorfos-lock]
lock:
  fingerprint: true
  key: true
```

It shows the time on every screen until you press a key or a mouse
button. Then the rest comes into view: your face, your name, a field for
your password and what each way of unlocking says. The key that wakes it
types nothing, so start typing your password after it. Backspace held
down goes on taking out, at the pace of `keyboard.repeat_rate` and
`repeat_delay`, and Ctrl+A, or Super+A, chooses all of what you typed,
which `question-input:active` marks, for Backspace to take it out at once
or what you type next to take its place. Escape, or being
left alone for 30 seconds, empties the field and shows only the time
again, the rest going the way it came. Your name is
your full name where your system has one (the first part of the comment
in `/etc/passwd`, which `chfn` sets), and your face is the picture your
login screen shows (`/var/lib/AccountsService/icons/` and your name),
or else `~/.face` or `~/.face.icon`, a PNG or a JPEG. Without one, your
initials are shown in a circle instead. However long your name, it is
shown whole, and cut short only where it would be wider than the screen.

It asks PAM, so whatever your system's login accepts unlocks it. A wrong
password shakes the field, as the administrator's prompt does, unless the
theme has nothing move (`question { transition: none; }`), and then
nothing comes into view or goes gradually either. A finger on
the reader and a security key, such as a YubiKey, are asked for at the
same time, each through a PAM service of its own, and whichever you use
first unlocks: you never wait for a finger to time out before typing.
Each is offered only where its module is installed and you have
something enrolled: `pam_fprintd` with a finger enrolled by `fprintd-enroll`,
and `pam_u2f` with a key registered by `pamu2fcfg`. Turn either off with
`fingerprint: false` or `key: false` under `lock:`, as in the example
above. The password is always
offered, so that a reader that breaks or a key that is lost never keeps
you out. The screen says nothing of the other ways until they speak: a
reader once it asks for a finger, and a key once it is plugged in and asks
to be touched, so that it does not tell whoever is at your desk what else
would unlock it.

Along the bottom are Sleep, Restart, Shut Down and Log Out, so that the
machine can be put to sleep or turned off, or the session ended, without
unlocking. Each is there only where your system lets it be done without a
password, as it does for whoever is at the machine when nobody else is
logged in; a password could not be asked for over the lock screen.
Restart, Shut Down and Log Out ask first, since the programs that are open
close and what is not saved in them is lost; Cancel or Escape says no.
Sleep is done at once, and you come back to the lock screen. Tab moves
from the field to each button in turn, and Shift+Tab back; Enter presses
the one you are on, which looks `question-button:focus`. Asked whether to
go ahead, you are on Cancel, so that pressing Enter twice loses nothing;
typing goes back to the field. When a
program keeps the machine from turning off, such as a download, the
screen says which, and that button is not there. On a machine others use,
`power: false` under `lock:` takes them all away:

```yaml
# config.yaml
lock:
  power: false
```

The services are files the package installs in `/etc/pam.d`, yours to
change: `omorfos-lock` for the password (as your system's login asks),
`omorfos-lock-fingerprint` and `omorfos-lock-key`. Any one way unlocks, so
the session is as hard to get into as the weakest of them. To need a key
and the password both, put the key's module in `omorfos-lock`, before the
password's, and turn `lock.key` off: they are then asked in turn. A key
that asks for its PIN belongs there too, since only the password's field
is typed into.

## Asking for the administrator's password

When a program wants to do what needs the administrator, installing
software or changing the date, polkit asks the desktop to ask you, and
`omorfos-ask` does, which omorfos starts with the session. A box in the
middle of the screen says what is asked for and whose password it wants:
yours, if you are allowed to say yes, as an administrator is, and
otherwise the first of those who are. Type it and press Enter; Escape or
Cancel gives up. Backspace held down and Ctrl+A or Super+A work as on the
lock screen. The eye at the end of the field shows what you typed, or
hides it again, and says so when the pointer rests on it. Tab moves from
the field to the eye, Cancel and Authenticate in turn, and Shift+Tab
back; Enter presses the one you are on, which looks
`question-button:focus`, and typing goes back to the field. A wrong
password is asked for again, and the box shakes; a theme that has the
`question` part change at once, `question { transition: none; }`, keeps it
still.

While it asks, everything else is dimmed, and the keyboard and the
pointer reach the box alone: no key binding works, no other program and
no input method sees a key. Only a program omorfos started with the `ask`
role can do that, which is how you know the box is omorfos's own and not
a window imitating it. The dim is the theme's `sealed` part, and the box
is drawn as the `question` parts, its field as `question-input`.

What you type goes only to polkit's own helper, which checks it, and is
not kept once it has been handed over. Programs written for X11, and one in a
sandbox, ask through polkit like any other.

## Asking for a key's passphrase

GnuPG asks for the passphrase of a key, to sign, decrypt or use a key it
holds for SSH, through a pinentry. Name omorfos's in `gpg-agent.conf`, and
the same sealed box asks:

```sh
echo "pinentry-program /usr/local/bin/omorfos-pinentry" >> ~/.gnupg/gpg-agent.conf
gpg-connect-agent reloadagent /bye
```

It says only what omorfos says: that GnuPG asks for the passphrase of a
key, or for a new one twice, with the key's name and fingerprint as GnuPG
lists them. What GnuPG gives the box to say is not shown, as any program
can give it words to say there, such as "Type your login password"; it is
in the session's log, under `omorfos-ask`, if you want to read it. A wrong
passphrase is asked for again. Asked to confirm something, it says no,
for now, and GnuPG goes on as if you had: so a new passphrase GnuPG thinks
too weak, which it asks you to confirm, is not taken. Choose a stronger
one, or set `min-passphrase-len` and `min-passphrase-nonalpha` in
`gpg-agent.conf`.

Outside an omorfos session, or with `omorfos-ask` not running, it asks
nothing and GnuPG says so; name another pinentry there. Only
`omorfos-pinentry` started by `gpg-agent` is heard.

## Typing in other languages

Two things help with typing what a keyboard has no keys for.

**Several layouts**, for languages written with their own letters: list
them in `keyboard.layout`, as `layout: 'us,ru'`, and bind `next_layout`
to a key to go from one to the next. No key is bound to it by default.
XKB's own options work too, as `options: 'grp:alt_shift_toggle'`.

```yaml
# keys.yaml
bindings:
  - keys: Super+space
    action: next_layout
```

Key bindings go by the first layout that writes in Latin letters, so
`Super+t` is the same key whichever layout is being typed with.

**An input method**, for languages with more characters than a keyboard
has keys, such as Chinese, Japanese or Korean: a program such as
`fcitx5` turns what is typed into the characters meant, and shows those
it could mean beside the text. Start it with the `input` role:

```yaml
# config.yaml
autostart:
  - { run: [fcitx5, --replace], may: [input] }
```

Not with `-d`: a program that puts itself in the background as it starts
lets go of the connection it was given the role on. It then works in any
program that says where text is typed (GTK, Qt and the browsers do), in
windows and on panels, and its popup is shown under the text, or above
it where there is no room. While the session is locked it is not shown a key:
what is typed then is your password. One input method at a time: should
a second be started, the first is told it may no longer be one, so
start only one.

## Sleep, restart and shut down

Whatever puts the machine to sleep, the lid, the sleep key or a command,
omorfos locks the screen first with the `locker` you named, and the
machine sleeps only once nothing of the session is on any screen, so
that it never wakes unlocked. logind waits a few seconds for that, five
unless it is set otherwise; a locker that has not locked by then is let
be, and you are told when the machine wakes. With no `locker` named the
machine sleeps at once. `loginctl lock-session` locks the same way;
`loginctl unlock-session` does nothing, since only your password opens a
locked screen.

| Command | Does |
|---|---|
| `omorfos-session sleep` | Sleeps |
| `omorfos-session restart` | Starts the machine again |
| `omorfos-session shut-down` | Turns it off |

Each is asked of logind, which decides whether you may. What the lid,
the power button and the sleep key do is logind's to say, in
`/etc/systemd/logind.conf` (`HandleLidSwitch`, `HandlePowerKey`,
`HandleSuspendKey`); omorfos binds none of them, since logind has
already acted on them, and a binding as well would sleep twice. To
have one ask something else, set it to `ignore` there and bind the key:

```yaml
# keys.yaml
bindings:
  - keys: XF86PowerOff
    action: { spawn: [omorfos-session, shut-down] }
```

All this needs logind; without it, nothing locks before sleep.

## What programs say

What omorfos and the programs that come with it say goes where omorfos's
own output goes: a configuration it cannot read and why, a program it sent
away for asking what Wayland does not allow, and why. That is a file if
you start omorfos as `omorfos > ~/omorfos.log 2>&1`, or wherever your login
screen keeps what a session says.

What the programs started in the session say, those you open and those
that start with it, is kept in the journal, each under its own name, where
systemd's journal runs:

```sh
journalctl --user -t foot
journalctl --user -t code --since today
```

So a program that complains, as most do now and then, does not fill
omorfos's log, and what it said is still there after you log out and in
again. Where there is no journal, they say it where omorfos does.

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

### Switching windows

`Alt+Tab` opens the switcher in the middle of the screen the pointer is
on: every window, on every workspace and minimized or not, each as a
small picture of itself over its title, in the order they were last
used. It opens at the window used before the one you are in, so one
`Alt+Tab` goes back to it and another comes back again.

While Alt is held, Tab goes on to the next window and `Shift+Tab` back,
as do the arrows; letting go of Alt goes to the window picked, bringing
it back if it was minimized and showing its workspace. Enter does the
same, Escape closes the switcher where you were, and a click on a window
goes to it. `Alt+Shift+Tab` opens it at the window used longest ago.

The order stands still while the switcher is open: a window passed over
on the way has not been used. Each window's icon is beside its title:
the one its program gives it, or else the one its application is
installed with (the `Icon` of its desktop entry, or the icon named as
its `app_id` is). A window a rule says is `private` is shown by its
title and its icon, large where its picture would be. How the switcher looks is `switcher`,
`switcher-item` (the one picked is `:active`, and `min-width` is how wide
each is) and `switcher-title` in `theme.css`. The actions are
`switch_windows` and `switch_windows_back`; `focus_next` and
`focus_previous` still go from window to window on a workspace without
it.
