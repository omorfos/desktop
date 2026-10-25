# 54. Window icons served by omorfos, and an icon's lost pixels let go

- Status: accepted
- Date: 2026-10-23

## Context

Programs give their windows icons through `xdg-toplevel-icon`: a name of
an icon of a theme, or pixels in buffers of shared memory. omorfos draws
them in the switcher, and took the protocol from Smithay.

The protocol says a buffer given as an icon "must be kept alive for as
long as the xdg_toplevel_icon it is associated with is not destroyed,
otherwise a 'no_buffer' error is raised", and Smithay raises it, which
ends the program's connection. Visual Studio Code, as programs built on
Chromium do, destroys the buffer once it has set the icon and keeps the
icon. It ended seconds after it started, every time:

    code, process 3742157 was disconnected for asking what the Wayland
    protocol does not allow: xdg_toplevel_icon_v1@39: The provided buffer
    has been destroyed before the toplevel icon

The desktops these programs are tried on copy the pixels when they are
given, and never notice. Copying them here would mean reading a program's
shared memory on the CPU, which Smithay hands over only as a pointer, and
`unsafe` is kept to the one block ADR 40 allows.

## Decision

omorfos serves the protocol itself, in `icons.rs`, as Smithay did, but for
one thing: a buffer destroyed before its icon is let go, and the icon is
drawn as though it had never had those pixels. A window whose icon has
lost all of them is drawn with its application's icon, from its desktop
entry or its name, as one that gave none.

The rest of the protocol is kept as it is written. An icon changed once it
has been set on a window, and a buffer that is not in shared memory or
not square, are still refused with the errors the protocol names, and the
log says which program and why. The icon is double-buffered with the
window's surface, as the protocol says. The buffers are drawn by the
renderer, never read by omorfos, as before. What the renderer makes of
each is kept until the program destroys the buffer, as the protocol has
its pixels never change once given, so that it is handed to the graphics
card once and not with every frame the icon is drawn in.

## Consequences

- Visual Studio Code runs, and so should other programs built on
  Chromium and Electron that give icons the same way; only Visual Studio
  Code has been seen to.
- One protocol fewer is Smithay's: when Smithay changes, this does not
  follow, and its own tests are ours to keep.
- A program that destroys its icon's buffer gets its application's icon
  rather than its own, which is usually the same picture.
- To be taken back to Smithay if it lets a destroyed buffer go too, which
  is worth asking for.
