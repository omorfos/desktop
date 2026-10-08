# Roadmap

Work proceeds one milestone at a time, top to bottom. Tick items as they
land and keep the "Next" pointer current.

**Next:** milestone 2, configuration.

## 1. Bootstrap

- [x] Licence, `.editorconfig`, `.gitignore`, README
- [x] Cargo workspace, pinned toolchain, workspace lints
- [x] `omorfos-core` with geometry primitives, proving the coverage gate
- [x] Agent context: `AGENTS.md`, architecture, ADRs, this roadmap
- [x] CI: format, Clippy, docs, `cargo-deny`, spelling, commit lint, tests
      with the coverage gate on amd64 and arm64
- [ ] Push to GitHub and confirm the workflows pass there (needs the
      repository URL; then set `repository` in the workspace manifest)

## 2. Configuration (`omorfos-config`)

- [ ] Choose a maintained YAML crate (`serde_yaml` is archived); record an ADR
- [ ] Schema and built-in defaults for `config`, `keys`, `rules`, `theme`,
      `menu`
- [ ] Validation with file, line and column diagnostics
- [ ] Last-known-good store: an invalid reload is rejected whole
- [ ] Directory watcher with debounce (editors save by atomic rename)
- [ ] Generated JSON Schema for editor completion

## 3. Core domain (`omorfos-core`)

- [ ] Windows, outputs and workspaces
- [ ] Focus model and stacking order
- [ ] Placement strategies
- [ ] Tab groups
- [ ] Key-chain state machine
- [ ] Per-application rules

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
