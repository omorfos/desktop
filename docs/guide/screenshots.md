# Pictures of the screen

`Print` takes a picture of every screen and saves each as a PNG in a
folder called `Screenshots` among your pictures: `~/Pictures`, or wherever
`~/.config/user-dirs.dirs` says pictures go. A picture is named after
when it was taken, and after its screen when there is more than one. One
that is there already is never written over.

`Alt+Print` takes the window with focus by itself, as large as it is, in
the frame omorfos draws round it and with the shadow it casts.

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
whole of it, however large, in its frame with the shadow it casts, and
see-through where nothing is, as the shadow is. A program says which window it
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
`capture` part of the theme; one it says is `private: blur` is blurred
in a picture of the screen, and blank in one of itself. The menus of a
program written for X11 with a private window are left out of a picture
of the screen, as are those of a Wayland program. A window that is out of sight, minimized or
on another workspace, can have its picture taken all the same.

Nothing is given while the session is locked. The pointer is in a picture
only if the program asks for it, and `omorfos-screenshot` does not.
