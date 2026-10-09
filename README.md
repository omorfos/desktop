# omorfos

A lightweight Wayland compositor for Linux, written in Rust and inspired by
[Fluxbox](https://github.com/fluxbox/fluxbox): tabbed window groups, chained
keybindings, per-application rules and a small footprint, with a modern look.

> **Status:** early development. The compositor runs in a window on an
> existing desktop, with decorations, tabs, a root menu, the clipboard,
> panels and wallpapers through layer-shell, and an IPC socket. It cannot
> drive real hardware yet. See [docs/ROADMAP.md](docs/ROADMAP.md).

## Goals

- **Secure:** no `unsafe` in our own crates, least-privilege IPC, vetted
  dependencies.
- **Fast:** damage-tracked rendering, no work when nothing changes.
- **Lean:** the compositor does window management; panels, launchers and
  notifications are separate clients.
- **Friendly:** YAML configuration in `~/.config/omorfos/`, reloaded live, and
  a broken config never takes the session down.

Supported targets: Linux on `x86_64` and `aarch64`.

## Trying it

```sh
cargo run -p omorfos
```

This opens omorfos in a window on your current Wayland or X11 session and
prints the name of its socket. Start a program inside it with, for example,
`WAYLAND_DISPLAY=wayland-1 foot`. Key bindings using Super may be taken by
the host desktop before they reach the window. Configuration is described in
[docs/CONFIGURATION.md](docs/CONFIGURATION.md), and scripting and component
integration in [docs/IPC.md](docs/IPC.md).

## Development

Building needs the development files for libxkbcommon and pixman
(`libxkbcommon-dev libpixman-1-dev` on Debian and Ubuntu). Running also needs
EGL and OpenGL ES (`libegl1 libgles2`).

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage
cargo deny check
```

`cargo coverage` is an alias for the coverage gate, defined in
`.cargo/config.toml`. The toolchain is pinned in `rust-toolchain.toml`. Commits follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).
Architecture and conventions live in [AGENTS.md](AGENTS.md) and
[docs/](docs/).

## License

[MIT](LICENSE)
