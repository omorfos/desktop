# Roadmap

Work proceeds one milestone at a time, top to bottom. Tick items as they
land and keep the "Next" pointer current.

**Next:** milestone 9, shell components.

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
- The mouse cursor was the host's; our own, and client-set cursors, were
  done in milestone 8.
- Problems (bad configuration, a program that would not start) were only
  printed to standard error; they are shown on screen since milestone 7.
- The nested backend was run under GNOME with GTK applications and
  Firefox, driven over IPC, before milestone 8. Nobody has yet sat in front
  of it: drawing, and input from a real keyboard and mouse, are unchecked.
  It has not been run at all since the event loop moved into `Session`.

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

A `wayland-sessions` entry for login screens was left for milestone 8, when
the compositor could drive real hardware, and is done there.

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
- Only the titlebar's top corners were rounded. The window's own are
  since milestone 8, and without the shader that was expected to need.
- Pointer cursor shapes (for example a resize arrow over a border): done
  in milestone 8.

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
- The IPC socket was polled each turn of the loop rather than waking it.
  Fixed in milestone 8 with the event loop.

## 8. Real hardware

- [x] The event loop reworked so the core can register timers and
      sources: a tested `Session` owns it, and nothing is polled (ADR 9)
- [x] DRM/KMS, libinput and libseat backend (ADR 10): one screen, a drawn
      pointer, virtual terminal switching and touchpad settings. Run by
      hand on one laptop with foot and Firefox
- [x] Multi-monitor and hotplug (ADR 15): every connected screen, side by
      side, each with its own scale, coming and going
- [x] Scaling, whole and fractional: automatic from the screen's density
      or set in `config.yaml` (ADR 11)
- [x] Our own pointer cursor, client-set cursors and cursor shapes, from
      the installed cursor theme (ADR 12)
- [x] idle-notify, idle-inhibit and session-lock (moved from milestone 7;
      ADR 14)
- [x] A `wayland-sessions` entry for login screens, with programs told
      about the session however they are started (ADR 13)
- [x] Rounded window corners, without a shader (ADR 16)
- [x] Screens turned off when the user is away (`idle.screen_off`), and
      programs started with the session (`autostart`)

## 9. Shell components

Each component is a program of its own: a Wayland client that learns what
to show over the IPC socket (ADR 18). In this order:

- [x] `omorfos-ui`: the canvas, text and scale the compositor draws with,
      shared with the components
- [ ] Panel
  - [x] The bar itself: a screen's workspaces and the windows on the one it
        shows, how it is drawn and what a click asks for
  - [x] The program, `omorfos-panel`: a bar on each screen through
        layer-shell, kept up to date from an IPC subscription and following
        the theme, tested against a real session, and packaged
  - [x] A clock: the local time, from `jiff`, drawn again when the
        minute turns
  - [x] Started with the session unless `panel.enabled` is off
  - [x] Tried on real hardware
  - [ ] Where it is and what is on it, by configuration
- [x] The theme as parts with states and properties (`panel`, `button`
      that is `active`, and so on) rather than one palette, so that it maps
      onto selectors (ADR 19)
- [x] CSS as the theme format, `theme.css`, in place of `theme.yaml`
      (ADR 20). No HTML: what is on the screen stays decided by the
      components, CSS says how it looks
- [x] Values as CSS writes them (`rgb()`, `hsl()`, named colours,
      `opacity`, the `border` shorthand, font weight and style), and every
      part a box with a margin, per-side borders, padding and rounded
      corners (ADR 21)
- [x] Transitions: a part's colours take the time its `transition`
      gives to change when its state does, in the compositor's frames and
      menu and in the panel, the launcher and the notifications (ADR 24)
- [ ] More of CSS: `box-shadow`, gradients, a background image, and a
      transition's delay and `cubic-bezier()`
- [x] What comes onto the screen fades in: windows with their frames, the
      menu, the notice, and the panels of the shell components, each over
      its part's `transition` (ADR 25)
- [x] What goes from the screen fades out, as it was last drawn: windows
      with their frames, the menu, the notice and the panels of the shell
      components (ADR 26). Buffers of a single pixel are accepted, which
      it needed
- [ ] A notification coming in or going beside others that stay, which
      is the notifier's to do: its surface is there throughout
- [ ] Blur behind what is see-through (`backdrop-filter: blur()`), the
      frosted-glass look. The compositor has to do it, not the component:
      draw what is below to a buffer, blur it in a shader, draw the part
      over it. It is the first drawing that only the GPU can do, so decide
      first, in an ADR, how it is tested (a slow equivalent on the CPU, or
      an exclusion), how what changes behind a blurred part is redrawn, and
      how other programs ask for it (the background-effect protocol).
      `filter: blur()` of a part's own picture is small and can come with
      it
- [x] Launcher, `omorfos-launcher`: type a name, pick from the programs
      that match, opened with `Super+d` or from the menu (ADR 22)
  - [x] Tried on real hardware
  - [x] Closing with a click beside it, with a backdrop that can dim the
        screen
  - [x] Icons and descriptions, each a setting (`launcher.icons`,
        `launcher.descriptions`)
  - [x] Keys that repeat when held
  - [ ] Icons from the icon theme in use and not only `hicolor`; looser
        matching; translated names
- [x] Notifications, `omorfos-notifications`: the desktop notification
      service over D-Bus, shown in a corner of the screen (ADR 23)
  - [x] Tried on real hardware, with Firefox
  - [x] Bodies that wrap
  - [ ] Buttons for actions beyond the click, JPEG pictures, and a way to
        see what has gone
- [ ] Theming polish and restrained animations

## 10. Later

- [x] Workspaces per monitor: each screen with its own set, so that
      switching workspace on one leaves the others as they are (ADR 17)
- [x] Settings for particular outputs, by name: the order they stand in,
      and each one's scale. Side by side only; one above another is not
      done
- [x] Programs written for X11, through `Xwayland` (ADR 27): their
      windows are windows like any other, framed, focused, moved, tabbed
      and closed by the desktop; their menus and tooltips are shown where
      they put them. On unless `xwayland.enabled` is turned off
  - [x] Copy and paste between programs written for X11 and for Wayland,
        and what is selected and pasted with the middle button
  - [x] Drawn sharply on a screen with a scale, for whoever sets
        `xwayland.scaling` to `native`: they draw with as many pixels as
        the densest screen has and are told how dense it is. Stretched
        otherwise, since programs that take no notice are then too small
        to use
  - [ ] The pointer the right size over a program written for X11 on a
        screen with a scale, whatever toolkit the program uses
  - [ ] Dialogs kept with the window they belong to, and windows that
        are docks, splash screens and the like treated as such
  - [x] The X server started again if it stops, where programs expect
        to find it, and given up on if it keeps stopping
  - [ ] The X server started when the first program asks for it, and not
        with the session. The sockets a program connects to have to be
        handed to `Xwayland` when it is started, and Smithay makes them
        itself and keeps what it needs to itself, so this waits on a
        change there
  - [x] The panel and the notifications started again if they crash, as
        the X server is (ADR 28)
- [x] What is wrong with the session shown among the notifications, in
      the same stack and the same style, so that it and they cannot cover
      one another and each is there until it is clicked away or put right
      (ADR 29). The notice the compositor draws is what is fallen back on
      when nothing is showing notifications, which is also when it has to
      say that the notifications have stopped
  - [x] What merely happened, such as the X server having been started
        again, told apart from what is wrong, and gone by itself after a
        while
- [ ] Screen capture and portals
