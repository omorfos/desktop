# 8. IPC and desktop components

- Status: accepted
- Date: 2026-10-09

## Context

The project wants "some sort of API or components system where default
desktop apps can integrate well". Panels, launchers, notification daemons
and wallpaper setters should be separate programs, and scripts should be
able to query and drive the desktop.

## Decision

**Two layers, standard first.** Components use Wayland protocols where one
exists, so the existing ecosystem works unchanged: layer-shell for where a
component sits on screen. Our own socket covers what those protocols do
not: listing windows and workspaces, performing actions, configuration
status.

**The socket speaks lines of JSON.** It is trivial to use from a shell, any
language and `jq`, and easy to inspect. A binary protocol would save
nothing that matters at this message rate.

**Updates are whole snapshots.** A subscriber is sent the entire state each
time it changes, rather than a vocabulary of events. A desktop has few
windows, so a snapshot is small, and a client cannot get out of step or
miss an event.

**Actions travel in the form `keys.yaml` uses.** There is then one spelling
of every action, in configuration, on the command line and on the wire, and
the protocol crate does not depend on the configuration crate.

**Peers are trusted to act, not to behave.** Any peer is the same user, so
letting it perform actions, including starting programs, grants nothing
new. What it must not be able to do is stall or bloat the compositor: all
socket I/O is non-blocking, sizes are capped, and a peer that breaks a
limit is dropped. The socket is owner-only and peers' credentials are
checked. Sandboxed applications do not see the socket unless it is
deliberately shared.

**Layers and windows share one stacking order**, used for both drawing and
pointer input: overlays, fullscreen windows, the top layer, ordinary
windows, the bottom layer, the background. A right click on the background
layer opens the root menu, because a wallpaper covers the bare desktop that
would otherwise take that click.

**Reserved space is the desktop model's business.** Layer surfaces'
exclusive zones become an output's usable area in `omorfos-core`, which
places and maximizes windows within it.

**Problems are shown where the user is looking.** A notice on screen lists
what is wrong, and `omorfosctl config check` prints the same list.

## Consequences

- Existing layer-shell components work without knowing omorfos exists.
- A subscriber is sent more bytes than a diff would need; negligible here.
- The IPC protocol is a compatibility surface from now on; it carries a
  version number that rises only on breaking changes.
- Not yet offered, so panels cannot yet list windows or workspaces through
  standard protocols and must use our socket: foreign-toplevel and
  ext-workspace. Also outstanding: xdg-activation, idle-notify,
  session-lock, the primary selection, popup grabs, and popups of layer
  surfaces.
