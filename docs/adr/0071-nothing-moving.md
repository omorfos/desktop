# 71. Nothing moving, with one setting

- Status: accepted
- Date: 2026-11-10

## Context

ADR 32 made reduced motion a setting from the first day, and DESIGN.md
says what it does: with it, things that move change opacity only, over
120 ms, and things that slide appear. The setting was not there.
Meanwhile more moved: parts grow as they come and shrink as they go,
workspaces slide by keys and with three fingers, and the box that asks
for a password and the lock screen's field shake at a wrong one. More is
to come with the rest of "Movement". Movement on a screen makes some
people feel sick or dizzy, and others cannot follow what is going on
for it; for them, it is the movement that has to go, not the change.

## Decision

`appearance.reduce_motion` in `config.yaml`, off unless set.

- **The theme is still, whatever it says.** The configuration builds the
  theme `Theme::still()` makes of it: every part comes and goes as large
  as it is, only fading, and no change takes longer than 120 ms, the
  time a colour takes to change with a state in the built-in theme. It
  is done where the theme is built, not where each part is drawn, so
  that the compositor and every component, which all have their theme
  from the configuration, are still alike, and a part added later is
  still without being told. A theme of the user's that moves is made
  still too: the setting is the user saying so, and wins.
- **Workspaces do not slide.** A key changes the workspace at once.
  Three fingers carry nothing, and change it as they lift if they went
  far enough or fast enough, by the rule they would have slid by, so
  that a swipe still does what it did.
- **Nothing shakes.** The box that asks for a password and the lock
  screen's field keep still at a wrong one, and say so in words, as they
  did besides shaking.
- **Programs are told,** to move nothing either: through the settings
  portal, as `org.freedesktop.appearance` `reduced-motion`, the key the
  portal's specification has for it, and as GNOME's
  `org.gnome.desktop.interface` `enable-animations`, which GTK and
  libadwaita read, in a sandbox and out; and to programs written for
  X11, as `Gtk/EnableAnimations` in their settings. Each is told again
  when the setting changes.
- **What changes without moving is left alone.** Day turning into night
  under `scheme: auto` still takes its minute: it is colours changing,
  slowly, which is what reduced motion keeps in place of movement.

## Consequences

- What moves from now on is written to stand still under the setting,
  as the rest of "Movement" will be.
- A theme cannot make anything move while the setting is on, nor make a
  change take longer than 120 ms.
- A program that reads neither key, as most that draw their own
  interface without a toolkit, moves as it likes.
