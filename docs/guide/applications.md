# What applications are told of the desktop

Applications ask the desktop some things through the portals
(`xdg-desktop-portal`), and omorfos answers what only it can. So far that
is how the desktop looks: whether it is light or dark, and the colour
that picks things out. An application that follows the desktop, as most
written with GTK or Qt do, is then dark when your theme is and light when
it is, as `appearance.scheme` makes it or your theme does, in the accent
`appearance.accent` gives.

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
