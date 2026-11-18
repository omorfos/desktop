# Standards

What omorfos and its programs say or read that is not yet an agreed
standard, and where each would be taken to become one
([ADR 99](adr/0099-standards-before-inventions.md)). Each says what it
is, what there is today, what omorfos does meanwhile, and where it
would be proposed.

Nothing here has been raised yet unless it says so. Raising one is done
by the maintainer, or by an agent the maintainer has asked to, in the
first person. Before one is raised, read what is said of it now in the
project it would go to: several of these have been proposed before, and
a proposal already open is joined, not made again. An entry goes when
what it asks for is agreed and omorfos speaks that.

## What is agreed, for reference

Checked on 18 November 2026.

- **Dark or light, the accent, high contrast, reduced motion**: the four
  keys of `org.freedesktop.appearance` in the settings portal
  (`color-scheme`, `accent-color`, `contrast`, `reduced-motion`),
  version 2 of the interface.
- **Blur behind a surface**: `ext-background-effect-v1`, in
  wayland-protocols' staging since May 2025; KDE Plasma 6.7 and GNOME 51
  have it. omorfos offers it, and KDE's own beside it until programs
  have moved (ADR 64).
- **Notifications, desktop entries, icon themes, where files go**:
  freedesktop's specifications.

## Settings every program should be able to ask

These are one proposal's worth each, to `xdg-desktop-portal`, as keys
of `org.freedesktop.appearance` or a namespace beside it. Today each is
GNOME's setting, which toolkits read through the portal under GNOME's
name, and which omorfos serves under those names (ADR 68, ADR 97).

### The side and order of a titlebar's buttons

- **Today:** GNOME's `button-layout` in
  `org.gnome.desktop.wm.preferences`; `Gtk/DecorationLayout` in
  XSETTINGS. GTK, libadwaita, Qt's own titlebars, Firefox and Chrome
  read it. Proposed to the portal twice: pull request 996, closed in
  2025, and 1821, a draft (ADR 68).
- **omorfos:** serves GNOME's key, and tells GTK 3 to read the portal.
- **To raise:** join 1821, or what has followed it, with what omorfos
  serves and what each toolkit was found to read.

### The fonts

- **Today:** GNOME's `font-name` and `monospace-font-name` in
  `org.gnome.desktop.interface`; `Gtk/FontName` in XSETTINGS.
- **omorfos:** serves both, from the theme.
- **To raise:** the interface's font and the monospaced one, as
  families and a size, as a setting of the portal's own.

### The pointer's theme and size

- **Today:** GNOME's `cursor-theme` and `cursor-size`; the environment's
  `XCURSOR_THEME` and `XCURSOR_SIZE`; and for a program that names a
  shape and draws none, `wp-cursor-shape-v1`, which needs neither.
- **omorfos:** all three.
- **To raise:** whether a setting is wanted at all once programs name
  shapes; if so, the two as a setting of the portal's own.

### The icon theme

- **Today:** the icon theme specification says what a theme is, and
  nothing says which is in use but GNOME's `icon-theme`.
- **omorfos:** serves GNOME's key from `icons.theme`.
- **To raise:** the theme in use, as a setting of the portal's own.

### Less transparency

- **Today:** nothing. High contrast is the nearest a program can ask.
- **omorfos:** `appearance.reduce_transparency`, its own; programs are
  not told.
- **To raise:** a key beside `reduced-motion`, for a program that is
  see-through of itself to be solid.

## Between a program and the compositor

To wayland-protocols, or to join what is proposed there.

### Surfaces of the shell: layer-shell

- **Today:** `wlr-layer-shell-unstable-v1`, wlroots's, which most
  compositors but GNOME's have taken up, and which is not in
  wayland-protocols. Every bar, dock, launcher, wallpaper and
  notification of every small desktop stands on it.
- **omorfos:** offers it; its own shell is drawn through it (ADR 18).
- **To raise:** find what has been proposed to bring it, or what
  follows it, into wayland-protocols, and say what omorfos needs of it.

### Raising and closing another program's windows

- **Today:** `ext-foreign-toplevel-list-v1` lists windows, and is
  agreed. Acting on one is `wlr-foreign-toplevel-management`, wlroots's
  and KDE's, and not agreed.
- **omorfos:** offers the list; its bar asks through omorfos's socket.
- **To raise:** with the dock (ADR 98), which needs it to stand alone.

### A window's picture, shown by a program that is not given it

- **Today:** nothing that lends a picture without giving it:
  `ext-image-copy-capture-v1` hands the pixels over, which is the
  `capture` privilege. Whatever shows windows small has to be the
  compositor's own or be trusted with every window's contents: a
  dock's previews, a taskbar's, a switcher or an overview that is a
  program of its own, and what lets the user choose which window to
  share or take a picture of.
- **omorfos:** the switcher and the overview are the compositor's, and
  a component says where and the compositor draws the picture there
  (ADR 38). Its own.
- **To raise:** once one of ours that is a program of its own asks for
  it, the dock first, and it is known what it needs to say.

### Who draws a window's titlebar

- **Today:** `xdg-decoration-unstable-v1`, in wayland-protocols and
  unstable since 2018, with only two answers: the program or the
  compositor.
- **omorfos:** offers it (ADR 7, ADR 97).
- **To raise:** nothing of ours yet; to follow what becomes of it.

### Controls in a titlebar

- **Today:** a program that wants its controls in the titlebar draws
  the whole of it. A specification for what a compositor's titlebar
  could hold for a program was proposed to xdg-specs in 2022 (issue 95)
  and not taken up.
- **omorfos:** its own programs are to draw their own (ADR 97).
- **To raise:** read what became of that proposal before stage 7.

### Typing without a keyboard, and input methods

- **Today:** `input-method-unstable-v2` and
  `virtual-keyboard-unstable-v1`, wlroots's, with `text-input-v3`:
  what an on-screen keyboard and an input method stand on, none of
  them stable, and GNOME and KDE each with their own.
- **omorfos:** offers all three.
- **To raise:** with the on-screen keyboard (ADR 98).

### The colour of a screen at night

- **Today:** `wlr-gamma-control-unstable-v1`, wlroots's, by which
  `gammastep` and `wlsunset` warm a screen. Not agreed.
- **omorfos:** warms its screens itself and does not offer it.
- **To raise:** whether to offer wlroots's; nothing to propose.

### A session that outlives its compositor

- **Today:** nothing agreed. Qt connects again if told to; omorfos
  keeps other programs by a relay between them and the compositor
  (ADR 60, ADR 61).
- **omorfos:** its own, and of no use to another compositor as it is.
- **To raise:** what the relay has to remember for each protocol says
  what a protocol for it would have to carry: to be written up once the
  relay has been lived with.

### A prompt nothing can imitate

- **Today:** nothing says a surface is the desktop's own asking for a
  secret, above everything a program can draw.
- **omorfos:** the sealed prompt, by a privilege of its own (ADR 37).
- **To raise:** not before another desktop wants the same.

## Beside the compositor

### The system tray

- **Today:** `StatusNotifierItem`, KDE's, on D-Bus, which programs with
  a tray icon speak and freedesktop never finished specifying.
- **omorfos:** not built; the bar is to speak it (stage 5).
- **To raise:** nothing; to speak what programs speak.
