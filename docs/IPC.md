# IPC

omorfos listens on a Unix socket for other programs of the same user:
panels, launchers, scripts, and `omorfosctl`. This is how a component
integrates beyond what Wayland protocols offer.

## `omorfosctl`

```sh
omorfosctl version                 # the compositor's version
omorfosctl state                   # workspaces, windows and problems, as JSON
omorfosctl watch                   # the state again, one line per change
omorfosctl action close            # any action from keys.yaml
omorfosctl action workspace 2
omorfosctl action spawn foot --title scratch
omorfosctl activate 7              # switch to the window with id 7
omorfosctl config check            # print problems; fails if there are any
omorfosctl config reload           # re-read the configuration, then check
```

It exits with 0 on success, 1 on failure or if `config check` found
problems, and 2 for a usage error.

`watch` is meant for piping into a panel or script:

```sh
omorfosctl watch | jq --unbuffered -r '.workspaces[] | select(.active) | .name'
```

## Finding the socket

In order:

1. `--socket PATH`, for `omorfosctl`.
2. `$OMORFOS_SOCKET`. The compositor sets this for programs it starts.
3. `$XDG_RUNTIME_DIR/omorfos-$WAYLAND_DISPLAY.sock`.

The socket is created readable and writable by its owner only, and the
compositor refuses a peer running as any other user.

## The protocol

Each message is one line of JSON, at most 64 KiB. A client sends a request
and reads one reply.

| Request | Reply |
|---|---|
| `"version"` | `{"version": {"protocol": 1, "compositor": "0.1.0"}}` |
| `"state"` | `{"state": {...}}` |
| `"subscribe"` | `{"state": {...}}` now, and again whenever it changes |
| `"reload"` | `"done"` |
| `{"perform": {"action": ...}}` | `"done"` |
| `{"activate": {"window": 7}}` | `"done"` |

Anything that cannot be understood or carried out is answered with
`{"error": {"message": "..."}}`, and the connection stays open. That
includes naming something that does not exist: activating a window that is
not there, or switching or moving to a workspace beyond the last one. An
action on the focused window when no window is focused does nothing and is
answered with `"done"`, as the same key binding would be.

An action is written exactly as in `keys.yaml`: `"close"`,
`{"workspace": 2}`, `{"spawn": ["foot", "--title", "scratch"]}`.

After `"subscribe"` the connection carries updates only. Each update is the
whole state, not a difference, so a client never has to reconstruct
anything.

### State

```json
{
  "workspaces": [
    { "output": "eDP-1", "number": 1, "name": "web", "active": true }
  ],
  "windows": [
    {
      "id": 7,
      "app_id": "foot",
      "title": "~",
      "output": "eDP-1",
      "workspace": 1,
      "focused": true,
      "shown": true,
      "minimized": false
    }
  ],
  "problems": []
}
```

Each screen has its own set of workspaces, numbered from 1, and shows one
of them. They are listed screen by screen, so numbers repeat and one
workspace per screen is `active`; tell them apart by `output` and `number`
together. A window's `output` and `workspace` say which it is on. `output`
is empty while no screen is connected. Windows are listed in a stable order (by
when their frame was created, then tab order), so a task list does not
reshuffle. `shown` is false for a tab that is not the one its frame is
showing.

### Versioning

`protocol` rises only when a change would break existing clients. Fields
and requests may be added without changing it, so clients should ignore
fields they do not know.

### Limits

The compositor never waits for a peer. A line longer than 64 KiB, a peer
that does not read what it is sent (more than 1 MiB pending), and a peer
that stops listening are each disconnected. At most 32 peers are served.

## From Rust

The `omorfos-ipc` crate has the message types and a small blocking client:

```rust
use omorfos_ipc::{Client, Reply, Request};

let path = omorfos_ipc::socket_path_from_env().expect("is omorfos running?");
let mut client = Client::connect(&path)?;
client.send(&Request::Subscribe)?;
while let Ok(Reply::State(state)) = client.receive() {
    // redraw from `state`
}
```
