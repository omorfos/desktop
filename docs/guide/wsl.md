# Under WSL

Under WSL 2, on Windows, omorfos runs in a window on the Windows desktop,
through WSLg, and programs written for Wayland run inside it. It cannot be
the whole desktop there: there is no screen of its own to take, nor a seat
with a keyboard and a mouse, as WSL has neither. What it is good for is
trying omorfos, its configuration and its theme, and working on it.

## Starting it

WSLg has to be on, as it is on Windows 11 unless `guiApplications=false`
is set in `.wslconfig`: `echo $WAYLAND_DISPLAY` in a WSL shell says
`wayland-0`. Then:

```sh
omorfos
```

opens a window, and says the name of its own socket, as
`omorfos: listening on wayland-1`. Programs started from inside it, as
the terminal `Super+Return` starts, are told that name. One started from
another WSL shell is told it by hand:

```sh
WAYLAND_DISPLAY=wayland-1 foot
```

## The Windows key

Windows keeps the Windows key for itself, and the chords made with it, as
`Win+L` and `Win+Tab`: omorfos's window never sees them, so none of the
bindings with Super can be had with it. WSLg has no setting that would
hand it on, and omorfos cannot ask Windows for it; that it should is
[microsoft/wslg#672](https://github.com/microsoft/wslg/issues/672).

What omorfos can do is make another key Super, as it reads the keys
itself, by the layout of its own configuration. In `config.yaml`:

```yaml
keyboard: { options: 'caps:super' }
```

has Caps Lock act as Super, and `altwin:menu_win` the Menu key, with
every binding as it was: `Caps Lock+Return` starts a terminal.

An option that puts Super on Alt, as `altwin:swap_lalt_lwin`, does so
too, but is less help than Caps Lock or the Menu key. Windows keeps
some chords of Alt for itself, `Alt+Tab` among them, and those never
arrive.

`Alt+F4` is WSLg's, too: it closes the window it is pressed in, which is
the whole of omorfos, with every program inside it. `Super+q` closes one
window, as everywhere.

There are ways on the Windows side as well. They have not been tried
with omorfos, and none of them is supported:

- PowerToys' Keyboard Manager, or AutoHotkey, can turn a chord with the
  Windows key into another, for `msrdc.exe` alone, which is the program
  WSLg's windows are shown by. `Win+L` and `Ctrl+Alt+Del` can never be
  turned into anything else, and programs that hook the keyboard so are
  known to leave keys held down in WSLg
  ([microsoft/wslg#1421](https://github.com/microsoft/wslg/issues/1421)).
- WSLg can show its whole desktop in one window instead, with
  `WSL2_WESTON_SHELL_DESKTOP=true` under `[system-distro-env]` in
  `%USERPROFILE%\.wslgconfig`. Full screen, that window is handed the
  chords with the Windows key, but WSLg's own desktop keeps some of them
  for itself, `Super+Tab` among them, before omorfos is told.

The menu opens with a right click on the desktop, with no key at all,
and `keys.yaml` can bind what Windows takes to other keys.

## The graphics card

Mesa draws in software by default under WSLg, with llvmpipe, which is
enough for a window but slow at a large size. To draw with the graphics
card instead, through Direct3D 12, as `/dev/dxg` lets programs in WSL:

```sh
GALLIUM_DRIVER=d3d12 omorfos
```

That needs a Mesa built with its `d3d12` driver, as Debian's and
Ubuntu's are. If glass cannot be blurred as it should with the card, it
is drawn solid, and omorfos says so in a notice.

Mesa says a few things as omorfos starts that are not a fault: that it
`failed to get driver name for fd -1`, that `ZINK: vkCreateInstance
failed`, and that it `failed to create dri2 screen`, as it tries the
ways of drawing WSL does not have before it finds one it does.

## Programs written for X11

WSLg keeps `/tmp/.X11-unix`, where an X server puts the socket programs
reach it by, read-only, for its own X server. omorfos cannot start its
own there, so it says in a notice that programs written for X11 cannot
run inside it, and carries on.

To have them, start omorfos where that directory is one of its own and
can be written to, with `bwrap` (bubblewrap):

```sh
bwrap --dev-bind / / --tmpfs /tmp/.X11-unix omorfos
```

Inside, the X server is omorfos's, and programs started from omorfos
reach it. Outside, nothing changes: other programs in WSL still reach
WSLg's and open on the Windows desktop. A program written for X11 that
is started from another WSL shell is not inside, so it opens on the
Windows desktop, not in omorfos.

See [Programs written for X11](x11.md) for the rest.
