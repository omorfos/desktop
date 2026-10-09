# Roadmap

Work proceeds one milestone at a time, top to bottom. Tick items as they
land and keep the "Next" pointer current.

**Next:** milestone 4, compositor skeleton.

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

- [ ] Smithay event loop with winit and headless backends
- [ ] `xdg-shell`, seat and input handling
- [ ] Damage-tracked rendering
- [ ] Integration harness driving real Wayland clients against the headless
      backend

## 5. Release pipeline

- [ ] Semver releases from Conventional Commits (release-please)
- [ ] amd64 and arm64 binaries with build provenance attestation
- [ ] `.deb` via `cargo-deb`
- [ ] AUR `PKGBUILD` and publishing

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
