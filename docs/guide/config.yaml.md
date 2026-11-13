# `config.yaml`

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
  super_as_ctrl: true     # Super+C reaches programs as Ctrl+C
  terminals: [foot, footclient, com.mitchellh.ghostty, kitty, Alacritty, org.wezfurlong.wezterm, org.gnome.Terminal, org.gnome.Ptyxis, org.gnome.Console, org.kde.konsole, com.gexperts.Tilix, XTerm]
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
  forget: []              # forgotten as the session locks: ssh, gpg
cursor:
  theme: omorfos          # omorfos's own, or a cursor theme by name
  size: 24                # 8 to 128
icons:
  theme: Adwaita          # an icon theme by name
appearance:
  scheme: dark            # dark | light | auto, light from sunrise to sunset
  # location: system      # for auto: where the sun is watched; or Europe/London, or { latitude: 51.5, longitude: -0.13 }
  # day: { from: '07:00', to: '19:00' } # for auto: light between these hours instead
  accent: cyan            # cyan | blue | teal | purple | pink | orange, or a colour as '#rrggbb'
  reduce_transparency: false # every surface the theme makes glass solid
  reduce_motion: false    # nothing moves: what comes and goes only fades
night_light:
  enabled: false          # the screens warmer after dark, by appearance's day and night
  temperature: 4000       # kelvin at night, 1900 to 6500: lower is warmer
launcher:
  icons: false            # show each program's icon
  descriptions: false     # show, and search, what each program says of itself
switcher:
  style: diagonal         # the windows one behind another; or flat, in rows
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
update:
  wait: 3600              # seconds omorfos's new version waits for a quiet moment; 0 however long
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
see [Who may do what](permissions.md) below.

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

`cursor.theme` is `omorfos` for omorfos's own pointer, navy rimmed with
cream in the dark scheme and cream rimmed with navy in the light one. The
compositor draws it as shapes, sharp at any size and scale, and programs
are told it as the cursor theme `omorfos-dark` or `omorfos-light`, which
come with omorfos, as the scheme is. Or it names a cursor theme installed
in `~/.local/share/icons`, `~/.icons` or `/usr/share/icons` (or the
directories in `XCURSOR_PATH`), `default` being the one the system has
set up. Shapes the theme lacks come from the themes it inherits from and
then from `default`; failing those, the pointer is the theme's ordinary
arrow, and failing that, omorfos's own. It is the name of the
theme's directory, with no `/` in it and no `.` to begin it. The theme
and `cursor.size` are told to every program, so that
the pointer is as large over each as it is over the desktop: in
`XCURSOR_THEME` and `XCURSOR_SIZE`, with where to find themes in
`XCURSOR_PATH`, the places above, so that a program finds those
installed in `/usr/local` too; to programs written for X11 in the
settings their toolkits follow and in the X server's resources
(`Xcursor.theme`, `Xcursor.size`), as many of the X server's pixels as
it takes; and through the settings portal, as `cursor-theme` and
`cursor-size`, which GTK reads in a sandbox. A change is told as soon as
`config.yaml` is saved, or with `scheme: auto` as the scheme changes, to
D-Bus and systemd too for the programs they start, though a program that
read it once as it started keeps what it read.

`icons.theme` names the icon theme the icons programs name are looked up
in, for windows, the launcher and notifications: in it, then in the
themes its `index.theme` says it inherits from, then in `hicolor`, where
programs install their own, each in `~/.local/share/icons` before the
system's; and then among pictures with no theme, as in
`/usr/share/pixmaps`. It is a directory's name, as a cursor theme's is,
and is told to programs as the font is: in the settings toolkits follow
under X11, `Net/IconThemeName`, and through the settings portal's
`icon-theme`.

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
after those: by `Super+Ctrl+n`, by sending a window to a new one
(`Super+Ctrl+Shift+n`, or "Send to workspace" in the menu of its titlebar), or
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
before, each with its own wallpaper, as when a key changes the
workspace. Lift them past half way, or flick, and the one that came in is
shown; lift them short of that, or turn back, and the one that was there
stays. Three fingers swiped up open the overview (see [The overview](windows.md#the-overview)),
and down close it. Every other gesture, as a pinch to zoom, is the
program's under the pointer.

A window made fullscreen, by the second button of its titlebar, by
`Super+Ctrl+f`, or because its program asked, goes alone to a workspace made
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
window says it can be, and double-click it to put it back in the middle,
which it and the windows either side of it go to as a window sent
somewhere does.
Dragged, it keeps pace with the windows, going on as they have drawn
themselves the size it gave them, and is let go where the pointer is.
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
bottom come back. They slide in from that edge, the titlebar with the
panels, over the time the theme's `panel` transition gives one to come
in, and out the same way over the time `panel:closing` gives one to go:
180 ms and 120 ms in the built-in theme. With `panel { transition: none;
}`, or `reduce_motion`, they are there and gone at once. They are laid
over the window, which does not change size. They stay while the pointer is on them or still moving, so that a
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
