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

Beside these stands `omorfos-ui`, which draws: the compositor uses it for
decorations and menus, and each shell component for its own surface. A
component such as `omorfos-panel` is a client like any other. It depends
on `omorfos-ipc`, `omorfos-config` and `omorfos-ui`, and never on the
compositor or the domain.

## Crates

| Crate | Status | Responsibility |
|---|---|---|
| `omorfos-core` | done | Pure domain: `Desktop` (windows, tab groups, workspaces, stacking, focus), `Decoration` (frame layout and hit testing), `Menu` (pop-up menu layout and behaviour), placement, per-app rules, key-chain state machine, geometry. No I/O, no dependencies. |
| `omorfos-config` | done | YAML schema, defaults, validation with file, line and column diagnostics, directory watcher, debouncer, last-known-good store, JSON Schema generation (`schema` feature). The theme is a stylesheet in a small part of CSS, laid over a built-in one and worked out when loaded; it answers how a `Part` looks in its `States` with a `Style` and a `Font`, which is all that anything drawing asks of it. Depends on no other workspace crate. |
| `omorfos-ipc` | done | The IPC protocol's message types (one line of JSON each), where to find the socket, and a small blocking client. Depends on no other workspace crate. |
| `omorfos-ui` | done | Drawing on the CPU, independent of where the pixels go: `Canvas` (fills, and a part drawn as a box with its borders and rounded corners, in units of layout at any `Scale`), `Text` (measuring and drawing a line), `Scale`. Depends only on `omorfos-config`, for colours and fonts. |
| `omorfos-panel` | started | The panel. `Bar` decides what is on one screen's bar from an IPC `State`, where, how it is drawn and what a click asks for. `Panel` is the Wayland client around it: a layer-shell surface per screen, pictures handed over as shared memory, clicks sent back over the socket. |
| `omorfos` | started | The compositor. `Compositor` holds the Smithay protocol state and the `Desktop`, translates client requests and `Input` into domain commands, and mirrors domain state back into Smithay (`sync`). `Session` is the event loop around it. `bridge` maps configuration onto the domain. `backend::winit` runs it in a window and `backend::hardware` on the screen itself. |
| `omorfosctl` | done | Command-line client for the IPC socket: state, actions, configuration check and reload. |

Crates are created when their first task starts, not before.

## Principles

- **The domain is a state machine.** Adapters call `Desktop` methods as
  things happen (a window was mapped, a titlebar was clicked) and read back
  what to show with `frames()` and `focused()`. This keeps window management
  deterministic and testable without a display. The frame (a tab group of
  one or more windows) is the unit of position, stacking and state. See
  [adr/0004-domain-model.md](adr/0004-domain-model.md).
- **Hardware sits behind the session.** DRM/KMS, libinput and the seat are
  a backend that feeds `Input` in and draws with `render`, so every policy
  decision is tested without them. See
  [adr/0010-hardware-backend.md](adr/0010-hardware-backend.md).
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
  mode `0600`, and the peer's credentials are checked. Peers may act as the
  user they are, but cannot make the compositor wait or grow: I/O is
  non-blocking and sizes are capped. See [IPC.md](IPC.md) and
  [adr/0008-ipc-and-components.md](adr/0008-ipc-and-components.md).

## How the compositor is put together

A `Session` owns the event loop, the Wayland display and the `Compositor`.
A backend owns the screen and the input devices, and adds both to the
session. The loop sleeps until something happens. Each turn:

1. the backend's input sources feed `Compositor::input` with `Input`
   values, a small enum that hides where input comes from;
2. client requests are dispatched, reach the handlers in `handlers.rs` and
   become `Desktop` method calls;
3. `Compositor::refresh` reloads configuration if it changed, answers IPC
   peers, and then makes Smithay's view match the `Desktop`: which windows
   are mapped, where, in what order and state, and which has keyboard
   focus;
4. clients are sent what they are owed, and the backend draws with
   `Compositor::render`, generic over the renderer.

Nothing is polled. The compositor registers what it waits for besides its
clients (the configuration directory, IPC peers, programs it started) with
the loop, and reports through `Compositor::deadline` when it next needs a
turn regardless. See
[adr/0009-session-and-event-loop.md](adr/0009-session-and-event-loop.md).

The compositor draws, from its own scene rather than Smithay's `Space` so
that decorations interleave with windows correctly: client surfaces, frame
decorations, layer-shell surfaces (panels, wallpapers), the root menu, and
a notice listing configuration problems. The stacking order, top to bottom,
is: notice, menu, overlay layer, fullscreen windows, top layer, ordinary
windows, bottom layer, background layer. Pointer input follows the same
order.
Decorations and the menu are painted on the CPU by `paint` into a `Canvas`
and uploaded as textures, repainted only when their appearance changes. See
[adr/0007-decorations-and-menu.md](adr/0007-decorations-and-menu.md). A
frame's bottom corners are rounded by drawing its window in strips under a
painted border, which any renderer can do; see
[adr/0016-rounded-corners.md](adr/0016-rounded-corners.md).

Everything above is in units of layout, not pixels, in one space that
all the outputs show a part of. Each output has a position in it and a
scale, automatic or configured, that turns units into pixels when drawing;
applications are told the scale so that they draw sharply. Outputs come
and go as screens are plugged in. See
[adr/0011-scaling.md](adr/0011-scaling.md) and
[adr/0015-several-outputs.md](adr/0015-several-outputs.md).

Because the renderer and the input source are both parameters, the tests in
`crates/omorfos/tests/integration` run the whole compositor against a real
Wayland client with the pixman software renderer and assert on pixels. See
[adr/0005-compositor-adapter.md](adr/0005-compositor-adapter.md).

## Configuration

Files live in `$XDG_CONFIG_HOME/omorfos/` (default `~/.config/omorfos/`):
`config.yaml`, `keys.yaml`, `rules.yaml`, `theme.css`, `menu.yaml`. All are
optional; built-in defaults apply when a file is absent. The format is
described in [CONFIGURATION.md](CONFIGURATION.md).

Live reload is three small pieces the compositor wires together: `Watcher`
(an inotify descriptor that wakes the event loop), `Debouncer` (a pure
timer state machine) and `Store` (the configuration in effect).
