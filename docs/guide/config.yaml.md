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

How the screens stand, and how large everything is drawn on them
(`outputs` and `output`), is on [Screens](screens.md); the workspaces,
and the workspaces made for a fullscreen window or for tiles, on
[Using windows](windows.md#workspaces).
