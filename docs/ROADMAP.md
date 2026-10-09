# Roadmap

Work proceeds one milestone at a time, top to bottom. Tick items as they
land and keep the "Next" pointer current.

**Next:** milestone 6, Fluxbox features.

## 1. Bootstrap

- [x] Licence, `.editorconfig`, `.gitignore`, README
- [x] Cargo workspace, pinned toolchain, workspace lints
- [x] `omorfos-core` with geometry primitives, proving the coverage gate
- [x] Agent context: `AGENTS.md`, architecture, ADRs, this roadmap
- [x] CI: format, Clippy, docs, `cargo-deny`, spelling, commit lint, tests
      with the coverage gate on amd64 and arm64
- [x] Pushed to `github.com/omorfos/desktop`; workflows pass there

## 2. Configuration (`omorfos-config`)

- [x] YAML crate chosen: `serde-saphyr` (ADR 3)
- [x] Schema and built-in defaults for `config`, `keys`, `rules`, `theme`,
      `menu`
- [x] Validation with file, line and column diagnostics
- [x] Last-known-good store, per file
- [x] Directory watcher and debouncer (editors save by atomic rename)
- [x] Generated JSON Schemas in `schemas/`, checked by a test

Deferred to later milestones, where the pieces they need exist:

- Checking key names against real XKB keysyms (milestone 4, with
  libxkbcommon). Today any non-empty key name is accepted.
- Wiring the watcher and debouncer into the event loop and creating the
  directory on first run (milestone 4).
- Showing diagnostics on screen and `omorfosctl config check` / `reload`
  (milestone 7). A manual reload also covers symlinked files whose target is
  edited in place, which the watcher cannot see.

## 3. Core domain (`omorfos-core`)

- [x] Windows, outputs and workspaces (`Desktop`)
- [x] Focus model and stacking order
- [x] Placement strategies (`place`)
- [x] Tab groups (attach, detach, cycle)
- [x] Key-chain state machine (`KeyChains`)
- [x] Per-application rules (`Rule`, `Pattern`, `resolve`)

Deferred until the features that need them:

- Mapping `omorfos-config` values onto the domain's settings, rules and key
  chains (milestone 4, in the compositor).
- Fullscreen frames stacking above others, usable area reduced by panels,
  size constraints and always-on-top (milestones 6 and 7).

## 4. Compositor skeleton (`omorfos`)

- [x] `Compositor` on Smithay: `wl_compositor`, `wl_shm`, `xdg-shell`
      (toplevels and popups), `wl_seat`, `wl_output`, `xdg-output`
- [x] Nested winit backend (`cargo run -p omorfos`)
- [x] Input: pointer focus, clicks, scrolling, keyboard, key bindings and
      chains, every configured action
- [x] Damage-tracked rendering, generic over the renderer
- [x] Configuration wired in: settings, rules, bindings, keyboard layout,
      background colour, live reload, key names checked against XKB
- [x] End-to-end test harness: a real Wayland client against the software
      renderer, asserting on protocol events and pixels

Not done yet, and where it is planned:

- Clipboard and drag-and-drop (`wl_data_device`): milestone 7.
- Popup grabs (dismissing a menu by clicking elsewhere), interactive move
  and resize, and decorations: milestone 6.
- The window's size is taken once, when it first shows content. A window
  that later resizes itself is drawn correctly, but restoring it from
  maximized uses the original size: milestone 6.
- A window that a rule maximizes shows one frame at its natural size before
  it is told the new size: milestone 6.
- The mouse cursor is the host's; drawing our own, and client-set cursors,
  come with the hardware backend in milestone 8.
- Problems (bad configuration, a program that would not start) are printed
  to standard error; showing them on screen is milestone 7.
- The nested backend has only been started under WSLg, not exercised with
  real applications.

## 5. Release pipeline

- [x] Semver releases from Conventional Commits (release-please)
- [x] amd64 and arm64 binaries, with build provenance attestation when the
      repository is public
- [x] `.deb` via `cargo-deb`
- [x] AUR `PKGBUILD` and opt-in publishing

Verified locally: the `.deb` and tarball packaging, and the AUR package
built and installed in an Arch Linux container. Not verifiable until the
first real release: the release-please configuration, the upload and
attestation steps, and the push to the AUR. See
[RELEASING.md](RELEASING.md) for the one-time setup on GitHub.

A `wayland-sessions` entry for login screens is left for milestone 8, when
the compositor can drive real hardware.

## 6. Fluxbox features

- [ ] Server-side decorations with tabs
- [ ] Interactive move and resize
- [ ] Root menu
- [ ] Key chains and per-application rules wired to config

## 7. IPC and ecosystem protocols

- [ ] `omorfos-ipc` protocol and client library, `omorfosctl`
- [ ] layer-shell, foreign-toplevel, ext-workspace, xdg-activation,
      idle-notify, session-lock
- [ ] Config errors surfaced on screen and via `omorfosctl config check`

## 8. Real hardware

- [ ] DRM/KMS, libinput and libseat backend
- [ ] Multi-monitor and hotplug
- [ ] Fractional scaling

## 9. Shell components

- [ ] Panel, launcher and notifications as separate clients
- [ ] Theming polish and restrained animations

## 10. Later

- [ ] XWayland behind a feature flag
- [ ] Screen capture and portals
