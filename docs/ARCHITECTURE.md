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
| `omorfos-core` | started | Pure domain: geometry, windows, workspaces, tab groups, focus, placement, key-chain state machine, per-app rules. No I/O, no Smithay types. |
| `omorfos-config` | done | YAML schema, defaults, validation with file, line and column diagnostics, directory watcher, debouncer, last-known-good store, JSON Schema generation (`schema` feature). Depends on no other workspace crate. |
| `omorfos-ipc` | planned | Versioned request, response and event types, plus a client library. |
| `omorfos` | planned | The compositor binary. Translates Smithay events into domain commands and domain state into rendering. |
| `omorfosctl` | planned | Command-line client for the IPC socket. |

Crates are created when their first task starts, not before.

## Principles

- **The domain is a state machine.** Adapters feed it commands (a key was
  pressed, a window was mapped) and act on what it returns. This keeps window
  management deterministic and testable without a display.
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

## Configuration

Files live in `$XDG_CONFIG_HOME/omorfos/` (default `~/.config/omorfos/`):
`config.yaml`, `keys.yaml`, `rules.yaml`, `theme.yaml`, `menu.yaml`. All are
optional; built-in defaults apply when a file is absent. The format is
described in [CONFIGURATION.md](CONFIGURATION.md).

Live reload is three small pieces the compositor's event loop wires
together: `Watcher` (an inotify descriptor to poll), `Debouncer` (a pure
timer state machine) and `Store` (the configuration in effect).
