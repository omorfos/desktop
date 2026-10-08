# 1. Fresh implementation on Smithay

- Status: accepted
- Date: 2026-10-09

## Context

The goal is a Fluxbox-like desktop on Wayland, in Rust. Fluxbox is a C++ X11
window manager built around Xlib. On X11 the window manager is one client of
the X server; on Wayland the compositor is the display server and owns
rendering, input, output management and protocol handling.

## Decision

Write omorfos from scratch rather than port Fluxbox, and build it on
[Smithay](https://github.com/Smithay/smithay).

Fluxbox contributes design, not code: tabbed window groups, chained
keybindings, per-application rules, the root menu, the toolbar and slit, and
a small footprint.

XWayland support is deferred and will arrive behind a feature flag.

## Consequences

- Almost no Fluxbox code would have survived a port, so nothing is lost, and
  the architecture is not shaped by X11 concepts.
- Smithay is pure Rust and proven in shipping compositors, which keeps our
  own crates free of `unsafe` and avoids binding to a C library.
- Smithay's API is not yet stable, so it is confined to the `omorfos` binary
  crate. The domain, configuration and IPC crates must not depend on it.
- X11-only applications will not run until XWayland lands.
