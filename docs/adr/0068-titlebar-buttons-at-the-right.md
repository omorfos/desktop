# 68. Titlebar buttons at the right, and programs told so

- Status: accepted
- Date: 2026-11-09

## Context

omorfos drew a titlebar's buttons at its left: close, then maximize,
then minimize. Most programs that draw their own titlebars, GTK and
libadwaita's, Firefox's and Chromium's, those of most Flatpaks, put
theirs at the right, where Windows, GNOME, KDE and most Linux desktops
have them. A desktop with both had them on two sides, depending on who
drew the frame. And those programs, not told otherwise, may fall back
to showing a close button alone: GTK's default for it under Wayland.

No setting of freedesktop's says where the buttons go. One for the
settings portal, two lists of buttons, those at the start and those at
the end, has been proposed twice in xdg-desktop-portal: the first, pull
request 996, was closed in 2025; the second, 1821, is a draft that no
toolkit reads yet. What toolkits do read is GNOME's: `button-layout` in
`org.gnome.desktop.wm.preferences`, as `menu:minimize,maximize,close`
writes the buttons at each end, either side of the colon. GTK 4 and
libadwaita read it through the settings portal, in a sandbox and out;
Qt's own Adwaita titlebars read it there too; and GTK under X11 reads
the same as `Gtk/DecorationLayout` in XSETTINGS.

## Decision

The buttons stand at the right-hand end of a titlebar, close outermost,
then maximize, then minimize: `Button::ALL` is in that order from the
right, and a titlebar too narrow for them keeps close the longest. The
built-in theme's wider margin is on close's right, away from the edge.

Programs are told the same, from the theme: `Theme::button_layout`
writes the buttons the theme gives a width, `:minimize,maximize,close`
by default, which the settings portal serves as `button-layout` and the
X11 settings as `Gtk/DecorationLayout`. A theme that does without a
button has programs do without it too.

The side is not a setting. Whether it should be one is for the review
of defaults; until then one side everywhere is what matters.

## Consequences

- Windows framed by omorfos and windows that frame themselves have their
  buttons in the same place, in the same order.
- A program that draws a titlebar of its own making, as Electron's
  custom titlebars do, reads none of this, and keeps its own.
- GTK 3 under Wayland, outside a sandbox, is thought to read GNOME's
  settings itself rather than through the portal, and is not told.
- Once freedesktop's setting is agreed, the portal serves it as well,
  from the same theme.
