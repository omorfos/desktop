# Roadmap

Work proceeds one milestone at a time, top to bottom. Tick items as they
land and keep the "Next" pointer current.

**Next:** milestone 8, real hardware.

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

- Clipboard: done in milestone 7. Drag-and-drop: see its known gaps.
- Popup grabs and the one-frame flash of rule-maximized windows: done in
  milestone 7.
- The mouse cursor is the host's; drawing our own, and client-set cursors,
  come with the hardware backend in milestone 8.
- Problems (bad configuration, a program that would not start) were only
  printed to standard error; they are shown on screen since milestone 7.
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

- [x] Server-side decorations with tabs (xdg-decoration; titlebar, borders
      and titles drawn by the compositor)
- [x] Interactive move and resize (titlebar and border drags, Alt-drag,
      and client requests)
- [x] Tabs by pointer: click to switch, middle-drag to attach and detach
- [x] Root menu (right click on the desktop, or `show_menu`)
- [x] Key chains and per-application rules wired to config (done in
      milestone 4)
- [x] Windows that resize themselves resize their frames; fullscreen
      frames are drawn above the rest

Still open, carried forward:

- Popup grabs and the one-frame flash of rule-maximized windows: done in
  milestone 7.
- Titlebar buttons (close, maximize, minimize), double-click to maximize,
  and keyboard navigation of the root menu.
- During a resize from the left or top edge, the window's content can lag
  the frame by a frame or two, because the frame moves before the window
  has redrawn at the new size.
- Only the titlebar's top corners are rounded; rounding the window itself
  needs a shader and waits for the hardware backend (milestone 8).
- Pointer cursor shapes (for example a resize arrow over a border) come
  with our own cursor in milestone 8.

## 7. IPC and ecosystem protocols

- [x] `omorfos-ipc` protocol and client library; the compositor serves it
- [x] `omorfosctl` (state, watch, actions, activate, config check/reload)
- [x] layer-shell: panels, docks, launchers, wallpapers, with reserved
      space respected by placement and maximize, their popups, and keyboard
      focus both exclusive and on demand
- [x] Clipboard (`wl_data_device`) and primary selection
- [x] Popup grabs: menus close on a click away and take the keyboard
- [x] xdg-activation, honoured only for tokens tied to user input
- [x] ext-foreign-toplevel-list, so taskbars can list windows
- [x] Windows that rules open maximized or fullscreen appear at their final
      size
- [x] Config errors surfaced on screen and via `omorfosctl config check`

Moved to milestone 8, with the reason:

- idle-notify needs a handle to the event loop, which today belongs to the
  backend. It moves with the event-loop work for real hardware.
- session-lock is security-critical: a lock screen that can be bypassed is
  worse than none. It needs input and drawing to be provably exclusive,
  which is best done once there is a real session to lock.

Not planned for now:

- ext-workspace and the wlr foreign-toplevel-management protocol. Smithay
  provides neither, and hand-writing them is not justified while the IPC
  socket already offers workspaces, window activation and actions.

Known gaps:

- Drag-and-drop between applications is enabled through Smithay's defaults
  but has no test; treat it as unverified.
- The IPC socket is polled each turn of the loop rather than waking it.

## 8. Real hardware

- [ ] DRM/KMS, libinput and libseat backend, with the compositor's event
      loop reworked so the core can register timers and sources
- [ ] Multi-monitor and hotplug
- [ ] Fractional scaling
- [ ] Our own pointer cursor, client-set cursors and cursor shapes
- [ ] idle-notify and session-lock (moved from milestone 7)
- [ ] A `wayland-sessions` entry for login screens
- [ ] Rounded window corners (needs a shader)

## 9. Shell components

- [ ] Panel, launcher and notifications as separate clients
- [ ] Theming polish and restrained animations

## 10. Later

- [ ] XWayland behind a feature flag
- [ ] Screen capture and portals
