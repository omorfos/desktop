# 76. The overview

- Status: accepted
- Date: 2026-11-12

## Context

The roadmap had an overview of every window on a workspace, and of the
workspaces, to come after the switcher (ADR 75), as GNOME's: the windows
laid out small side by side, to pick one from with the pointer or the
keys, and the workspaces along the top, to go to another. ADR 38 had
already said it is the compositor's to draw, as the switcher is, and that
a private window is shown in it as it is in pictures of the screen.

## Decision

**An `overview` action opens and closes it, on the screen the pointer is
on, and so do three fingers swiped up the touchpad and down.** No key
does by default: which key is for the review of the defaults, as every
new default has been; the gesture was asked for as it is.

- **Three fingers carry it**, as they carry the workspaces across: up
  opens it as far as they have gone, 300 of the touchpad's units for all
  of it, the windows moving and shrinking towards their places and the
  screen dimming with them; down closes it the same way. Let go past half
  way, or going fast enough that they would have got there in 120 ms, it
  goes on from where it is to open, and otherwise back to closed, over the
  time a glide takes. Which way a swipe goes is told once it has gone 8
  units: mostly across, it slides the workspaces as before; mostly up or
  down, it carries the overview; so the two never take the same swipe.
  With `reduce_motion` the fingers carry nothing, and it opens or not as
  they lift.

- **The windows of the workspace shown, laid out small** by the desktop
  model (`omorfos_core::overview`): in the order they are read where they
  stand, by their middles, in rows of as many across as shows the most of
  them, each as large as fits its share of the room and no larger than it
  is. A minimized window is among them; each frame of tabs is the tab it
  shows.
- **Each glides from where it is to its place over 240 ms, and back**,
  easing in and out, while the screen behind dims and blurs
  (`overview-backdrop`). What glides is the window's content, not its
  frame: its titlebar is gone as the glide begins and back as it ends. A
  minimized window has nowhere to glide from and fades in its place, as
  every window does with `appearance.reduce_motion`.
- **As they glide, the windows keep the stacking they have on the
  screen**, the topmost over the rest, so that nothing changes places as
  the overview sets out; the one gone to rises over the others as it
  closes, as it is raised once there. The border and title of the one
  picked are just over it, under whatever stands over it on the way.
- **The windows themselves are not drawn on that screen while it is
  open**, the scene being left as it is and each window whose frame's
  middle is on the screen passed over as it is drawn, so that a window
  opened meanwhile is passed over too.
- **The window picked**, by the pointer or the arrows, has a border round
  it (`overview-window`) and its title inside its foot
  (`overview-title`). Enter or a click goes to it, and it is given focus
  once the overview has closed, so that it does not come up over its own
  picture on the way back. Escape, or a click where nothing is, closes it
  as it was. Any other key is a binding, so that Alt+Tab opens the
  switcher in its stead; the switcher and the overview each close the
  other. The pointer is the overview's while it is open, and a swipe on
  the touchpad does nothing.
- **The strip is the screen's desktops**, each a small screen
  (`overview-workspace`): its wallpaper, drawn as the screen's background
  layer is, made small and cut to it, over the desktop's colour where
  there is none, and its windows where they stand on it, cut at its
  edges, the one shown in the accent and one under the pointer lit. A
  click goes to it without the slide, and its windows glide into the
  overview from where they are, each window keeping its own time of
  coming in. A workspace made for one fullscreen window is not a desktop,
  and cannot be gone to by its number, and is not in the strip.
- **After the strip, a plus makes a new workspace** (`overview-add`), at
  the end, and goes to it: one made and not gone to would go again at
  once, as an empty workspace does when another is shown.
- **Blanks and the desktop's colour are solid colours**, kept as large as
  they are drawn. A painted pixel stretched to a place, as was tried
  first, is filtered towards its transparent edge across the whole of it,
  and fades.

## Alternatives

- **Every workspace at once**, each a row of its windows. Busier, with
  smaller pictures; the user chose the one workspace and the strip.
- **The strip as names.** Cheaper, but tells less of what is where; live
  miniatures were chosen.
- **Fading in place.** Simpler, but loses where each window went; it is
  what `reduce_motion` gives.
- **Dragging a window onto another workspace in the strip**, in the
  first version. It is the next step, on the roadmap.
- **Hiding the windows by leaving them out of the scene.** The scene is
  what windows glide by when sent (ADR 72); passing over them as they are
  drawn leaves it as it is.

## Consequences

- `pictures.rs` holds what the switcher and the overview share: a
  window's picture made small, cut to where it may be drawn, with what is
  behind it blurred where its program asks, as on the screen (a terminal
  see-through over a blur, as Ghostty is, would otherwise be clear), and
  a private window as in pictures of the screen.
- The overview draws the screen's background layer a second time for each
  workspace in the strip, and each window there once more; while it is
  open, every window of the screen is told to keep drawing.
- On a workspace not shown, which window is over which is the order they
  were made in, which the desktop model keeps for none but the one shown.
