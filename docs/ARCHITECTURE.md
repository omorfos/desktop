# Architecture

omorfos follows a hexagonal (ports and adapters) layout. Dependencies point
inwards: adapters depend on the domain, never the reverse.

```text
            Wayland clients          omorfosctl / components
                   |                          |
        +----------v--------------------------v----------+
        |  omorfos (binary): Smithay adapters            |
        |  backends, protocol handlers, renderer, IPC    |
        +----------+----------------+-----------+--------+
                   |                |           |
           +-------v------+  +------v-----+  +--v----------+
           | omorfos-core |  | omorfos-   |  | omorfos-ipc |
           | pure domain  |  | config     |  | wire types  |
           +--------------+  +------------+  +-------------+
```

## Crates

| Crate | Status | Responsibility |
|---|---|---|
| `omorfos-core` | done | Pure domain: `Desktop` (windows, tab groups, workspaces, stacking, focus), `Decoration` (frame layout and hit testing), `Menu` (pop-up menu layout and behaviour), placement, per-app rules, key-chain state machine, geometry. No I/O, no dependencies. |
| `omorfos-config` | done | YAML schema, defaults, validation with file, line and column diagnostics, directory watcher, debouncer, last-known-good store, JSON Schema generation (`schema` feature). Depends on no other workspace crate. |
| `omorfos-ipc` | planned | Versioned request, response and event types, plus a client library. |
| `omorfos` | started | The compositor. `Compositor` holds the Smithay protocol state and the `Desktop`, translates client requests and `Input` into domain commands, and mirrors domain state back into Smithay (`sync`). `bridge` maps configuration onto the domain. `backend::winit` runs it in a window. |
| `omorfosctl` | planned | Command-line client for the IPC socket. |

Crates are created when their first task starts, not before.

## Principles

- **The domain is a state machine.** Adapters call `Desktop` methods as
  things happen (a window was mapped, a titlebar was clicked) and read back
  what to show with `frames()` and `focused()`. This keeps window management
  deterministic and testable without a display. The frame (a tab group of
  one or more windows) is the unit of position, stacking and state. See
  [adr/0004-domain-model.md](adr/0004-domain-model.md).
- **Hardware sits behind traits.** DRM/KMS, libinput and the seat are
  adapters, so every policy decision can be tested against a headless
  backend.
- **Configuration is a value.** Reload builds a whole new validated value
  and swaps it in. Each file stands alone: one that fails to load keeps the
  value it had, the others still apply, and the problem is kept as a
  diagnostic to show the user. See
  [adr/0003-configuration-loading.md](adr/0003-configuration-loading.md).
- **The compositor maps configuration to the domain.** `omorfos-config` and
  `omorfos-core` do not depend on each other; the binary translates config
  values into domain commands.
- **Components are processes, not plugins.** Panels, launchers and
  notification daemons are ordinary Wayland clients. They integrate through
  standard protocols (layer-shell, foreign-toplevel, ext-workspace) and the
  IPC socket. Nothing third-party runs inside the compositor.
- **IPC is least-privilege.** The socket lives in `$XDG_RUNTIME_DIR` with
  mode `0600`, and the peer's credentials are checked.

## How the compositor is put together

A backend owns the event loop and the screen. Each turn it:

1. feeds input to `Compositor::input` as `Input` values, a small enum that
   hides where input comes from;
2. dispatches client requests, which reach the handlers in `handlers.rs` and
   become `Desktop` method calls;
3. calls `Compositor::refresh`, which reloads configuration if it changed
   and then makes Smithay's view match the `Desktop`: which windows are
   mapped, where, in what order and state, and which has keyboard focus;
4. draws with `Compositor::render`, generic over the renderer.

The compositor draws three kinds of thing, from its own scene (bottom to
top) rather than Smithay's `Space`, so that decorations interleave with
windows correctly: client surfaces, frame decorations, and the root menu.
Decorations and the menu are painted on the CPU by `paint` into a `Canvas`
and uploaded as textures, repainted only when their appearance changes. See
[adr/0007-decorations-and-menu.md](adr/0007-decorations-and-menu.md).

Because the renderer and the input source are both parameters, the tests in
`crates/omorfos/tests/integration` run the whole compositor against a real
Wayland client with the pixman software renderer and assert on pixels. See
[adr/0005-compositor-adapter.md](adr/0005-compositor-adapter.md).

## Configuration

Files live in `$XDG_CONFIG_HOME/omorfos/` (default `~/.config/omorfos/`):
`config.yaml`, `keys.yaml`, `rules.yaml`, `theme.yaml`, `menu.yaml`. All are
optional; built-in defaults apply when a file is absent. The format is
described in [CONFIGURATION.md](CONFIGURATION.md).

Live reload is three small pieces the compositor's event loop wires
together: `Watcher` (an inotify descriptor to poll), `Debouncer` (a pure
timer state machine) and `Store` (the configuration in effect).
