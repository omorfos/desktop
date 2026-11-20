# The protocols programs lack

- Date: 2026-11-20
- Scope: the Wayland protocols omorfos does not offer, against what
  GNOME, KDE, sway, niri, COSMIC and Hyprland offer and what Smithay 0.7
  has, with what each is for and what it waits on.
- What it leads to: an order for the roadmap's item on smaller protocols,
  for the maintainer to take or leave.

omorfos offers 41 protocols today, as `crates/omorfos-ipc/src/protocols.rs`
lists them. The roadmap's item on smaller protocols named three:
`keyboard-shortcuts-inhibit`, content type and tearing control. The
first is offered already, since ADR 78 settled the key that always gets
back out (`Super+Escape`); the item had not caught up.

Each other compositor's protocols were found by searching its code for
each protocol's name where it registers what it offers, at the commits
under [Sources](#sources): GNOME's `mutter/src/wayland`, KDE's
`kwin/src/wayland`, sway's `sway/`, niri's and COSMIC's `src/`, and
Hyprland's `ProtocolManager.cpp`. A search finds a name, not a protocol
working, so a "yes" below is that the compositor speaks of it where it
offers protocols; each was looked at where a name could have been
another's, as `xdg_output_manager_v1` for wlroots' output management. No
compositor was run.

## What the others offer that omorfos does not

| Protocol | GNOME | KDE | sway | niri | COSMIC | Hyprland | Smithay 0.7 |
|---|---|---|---|---|---|---|---|
| `linux-drm-syncobj` | yes | yes | yes | | yes | yes | yes |
| `tablet-v2` | yes | yes | yes | yes | yes | yes | yes |
| `drm-lease` | yes | yes | yes | yes | yes | yes | yes |
| `xdg-dialog` | yes | yes | | | yes | yes | yes |
| `xdg-toplevel-tag` | yes | yes | yes | | | yes | yes |
| `xdg-session-management` | yes | yes | | | | | |
| `xdg-system-bell` | yes | yes | | | | yes | yes |
| `fifo` | yes | yes | | | | yes | yes |
| `commit-timing` | yes | yes | | | | yes | yes |
| `content-type` | | yes | yes | | | yes | yes |
| `xwayland-keyboard-grab` | yes | yes | | | yes | | yes |
| `color-management` | yes | yes | yes | | | yes | |
| `color-representation` | yes | yes | yes | | | | |
| `wl_fixes` | yes | yes | yes | | | | |
| `pointer-warp` | yes | yes | | | yes | yes | |
| `tearing-control` | | yes | yes | | | yes | |
| `ext-workspace` | | | yes | yes | yes | yes | |
| `xdg-toplevel-drag` | yes | yes | | | | | |
| `kde-server-decoration` | | yes | yes | yes | yes | yes | yes |
| `wayland-drm` | yes | | | yes | yes | yes | |
| `wlr-output-management` | | | yes | yes | yes | yes | |
| `wlr-screencopy` | | | yes | yes | | yes | |
| `wlr-foreign-toplevel-management` | | | yes | yes | | yes | |
| `wlr-virtual-pointer` | | | yes | yes | | yes | |
| `wlr-gamma-control` | | | yes | yes | | yes | |
| `wlr-output-power-management` | | | yes | | yes | yes | |
| `wlr-data-control` | | | yes | yes | yes | yes | yes |

Twenty-seven, in all. Smithay has twelve of them.

## What each is for, and what it waits on

Smithay has it, and it is little more than offering it:

- **`xdg-dialog`, `xdg-system-bell` and `xdg-toplevel-tag`**: a window
  saying it is a modal dialog, which GTK 4 says; a program ringing its
  bell, as terminals do, for the compositor to show or sound; and a
  window naming itself for rules to find it by. Small, and each has a
  place to go: the dialog dims and holds its parent, the bell marks the
  window of the program that rang it, never another's, and the tag is
  something `rules.yaml` can match.
- **`content-type`**: what a window shows, a film or a game, for a
  screen to be driven to suit it. As the roadmap had it: offered once
  omorfos does something with it, not before.
- **`xwayland-keyboard-grab`**: a program written for X11 asking for
  every key, as `keyboard-shortcuts-inhibit` is asked by one written for
  Wayland: a virtual machine or a remote desktop under X11. It would go
  by ADR 78's rule, with `Super+Escape` to have the keys back, and be
  held as kwin holds it: only while the window that asked has the keys
  already, never moving them to it. Only Xwayland can bind it.

Smithay has it, but more is needed than offering it:

- **`linux-drm-syncobj`**: explicit synchronisation, which NVIDIA's
  driver needs for what programs draw to reach the screen without
  flicker, and which Mesa's drivers use where offered. Every other
  compositor but niri has it. A commit is held until its point is
  reached, and nothing in omorfos lets a held commit go yet (Smithay's
  blockers, cleared as each point is reached); and under the relay
  (ADR 60) each point a program set has to be kept for the compositor
  started anew. It touches what the renderer waits on before drawing a
  program's buffer, so it is tried on real hardware and on more than one
  card.
- **`fifo` and `commit-timing`**: a program's frames shown one per
  refresh in order, and at a time it asks for, which Mesa's Vulkan
  drivers use for their FIFO present mode, so that games pace
  themselves without a thread of their own. GNOME, KDE and Hyprland have
  them. Both hold commits back as syncobj does, so need the same
  letting go; and a window not shown, on another workspace or minimized,
  has to be paced some other way, or a game there stops, with a bound
  for each surface on how long it is held.
- **`tablet-v2`**: drawing tablets and their pens, for Krita, GIMP and
  the like. On omorfos today a pen gets nothing, not even a mouse's
  pointer: the hardware backend does not take libinput's tablet events.
  Everyone else has it. It needs those events taken in, a tablet to try
  it with, and the relay to make again the tablets and tools the
  compositor told a program of by events, as it does seats.
- **`drm-lease`**: a screen handed to a program whole, which is how a
  headset is driven for VR. Everyone has it. Offered only for screens
  that say they are not for the desktop (`non-desktop`), never one the
  desktop is drawn on, and taken back as the session locks. It is the
  hardware backend's, needs the relay to make leasable screens again as
  it does outputs, and wants a headset to try it with.

Smithay does not have it yet, and it waits on Smithay or on work of
omorfos's own:

- **`color-management`**: what colours a program draws in, and what a
  screen can show; the roadmap's item on HDR is where it belongs.
- **`tearing-control`**: a game asking to be shown as soon as it draws,
  tearing and all, for less delay. As the roadmap had it, it waits on
  Smithay.
- **`pointer-warp`**: a program moving the pointer, within its own
  window, which games and remote desktops ask for. New, and GNOME, KDE,
  COSMIC and Hyprland already have it. Honoured only within the surface
  the pointer is already in, so a program cannot take it from another's.
- **`ext-workspace`**: the workspaces told to a panel such as Waybar,
  and switched from it. omorfos's own panel is told them over its socket;
  this is for others', and only for programs given the `panel` role
  (ADR 37), as switching the workspace is the desktop's.
- **`xdg-session-management`**: a program's windows put back where they
  were when it is started again. New; GNOME and KDE have it.
- **`color-representation`**: how a buffer's colours are encoded, as a
  video's, for the compositor to turn them into the screen's. Beside
  `color-management`, for the item on HDR.
- **`wl_fixes`**: a way for a program to destroy the registry it bound,
  in `wayland` itself rather than wayland-protocols. Smithay's
  `wayland-server` is where it would come from.
- **`xdg-toplevel-drag`**: a window dragged with what is dragged out of
  it, as a browser's tab; only GNOME and KDE have it.

Not proposed, as omorfos does the same another way or holds it to a role:

- **The wlroots protocols** (output management, screen copy, the list of
  windows to manage, a virtual pointer, gamma, output power) let any
  program that binds them arrange the screens, take pictures of them,
  close any window or move the pointer. omorfos has its own for each
  where it has the thing at all: screens set in `config.yaml` and listed
  by `omorfosctl`, pictures by `ext-image-copy-capture` to the programs
  given the role (ADR 37), the list of windows by `ext-foreign-toplevel-list`,
  night light of its own, and what was copied by `ext-data-control` to
  the `clipboard` role, which wlroots' `data-control` is the older form
  of. Each could be offered to a role, as the
  others are, if a program worth having needs it; none is proposed here.
- **`kde-server-decoration`**: KDE's older way of asking who draws the
  titlebar, which `xdg-decoration` replaces and Qt asks by first. GTK,
  as far as is known without reading it again, asks only by KDE's, so a
  GTK program draws its own titlebar here; as GTK draws its own by
  default, that is what it would ask for anyway.
- **`wayland-drm`**: Mesa's oldest way of handing over buffers, which
  `linux-dmabuf` replaces.

## Proposed order

1. `xdg-dialog`, `xdg-system-bell` and `xdg-toplevel-tag`: small, in
   Smithay, each with a visible use, and testable through the real client
   the compositor's tests have.
2. `xwayland-keyboard-grab`, beside `keyboard-shortcuts-inhibit` and by
   the same rule.
3. Held commits let go as what they wait on is ready, and then `fifo`
   and `commit-timing`, testable with a client that commits frames and
   watches when they are shown.
4. `linux-drm-syncobj` and `tablet-v2`, which need the hardware backend,
   the relay and real hardware, an NVIDIA card and a tablet.
5. `content-type`, `drm-lease`, `color-management`,
   `color-representation`, `tearing-control`, `pointer-warp`,
   `ext-workspace`, `xdg-session-management` and `wl_fixes`, as their
   reasons come or Smithay has them.

Each, as any protocol offered, goes on the list in
`crates/omorfos-ipc/src/protocols.rs` with how a program's state in it is
made again under the relay (ADR 60).

## What was not looked at, and what is not sure

- Which programs complain of each was not gathered from their logs or
  trackers: what each is for is what the protocol is made for, with the
  programs best known to use it.
- A protocol the others offer behind a setting, or only on one backend,
  is a "yes" here all the same.
- The list of protocols that exist was taken from what the compositors
  above build, not from wayland-protocols itself, whose repository could
  not be reached from where this was written; a protocol none of them
  builds is not here.

## Sources

- GNOME: [mutter](https://github.com/GNOME/mutter/tree/9bc8cd8427a5aa9bf8976acaef4426ce09a08537/src/wayland)
- KDE: [kwin](https://github.com/KDE/kwin/tree/c24a5f4e85683de94108fc3ca6ff48ea726f5037/src/wayland)
- sway: [sway](https://github.com/swaywm/sway/tree/1652c54b73f67df17b7b4ab0b0f7048204aa8104/sway)
- niri: [niri](https://github.com/YaLTeR/niri/tree/a855fd519f7341d378577da6a7b2763baeb70f79/src)
- COSMIC: [cosmic-comp](https://github.com/pop-os/cosmic-comp/tree/abb6ebc49ba58daa14163872954cdef3f830a04a/src)
- Hyprland: [`ProtocolManager.cpp`](https://github.com/hyprwm/Hyprland/blob/49b566a9780413a2498b7e8b925337ddb41a41a7/src/managers/ProtocolManager.cpp), and the protocols it builds, in its [`CMakeLists.txt`](https://github.com/hyprwm/Hyprland/blob/49b566a9780413a2498b7e8b925337ddb41a41a7/CMakeLists.txt)
- Smithay 0.7.0: [`src/wayland`](https://github.com/Smithay/smithay/tree/a166cf4c94b5aedc332a65aa1dd753e8148829c3/src/wayland)
