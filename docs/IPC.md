# IPC

omorfos listens on a Unix socket for other programs of the same user:
panels, launchers, scripts, and `omorfosctl`. This is how a component
integrates beyond what Wayland protocols offer.

## `omorfosctl`

```sh
omorfosctl version                 # the compositor's version
omorfosctl state                   # workspaces, windows and problems, as JSON
omorfosctl screens                 # the screens, where they stand and their modes
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
| `"show_problems"` | as `"subscribe"`, from a program that shows the user what is in `problems` |
| `"reload"` | `"done"` |
| `{"perform": {"action": ...}}` | `"done"` |
| `{"activate": {"window": 7}}` | `"done"` |
| `"placed"` | `{"placed": [...]}`, where the windows that can be seen are |
| `{"hold": {"idle": true, "sleep": false}}` | `"done"` |
| `{"radios": {"radios": [...]}}` | `"done"` |

Anything that cannot be understood or carried out is answered with
`{"error": {"message": "..."}}`, and the connection stays open. That
includes naming something that does not exist: activating a window that is
not there, or switching or moving to a workspace more than one beyond the
last (one beyond the last is made). An
action on the focused window when no window is focused does nothing and is
answered with `"done"`, as the same key binding would be.

An action is written exactly as in `keys.yaml`: `"close"`,
`{"workspace": 2}`, `{"spawn": ["foot", "--title", "scratch"]}`.

`"hold"` says what the programs a client speaks for ask the session to be
kept from, in place of what it said before: going `idle`, which keeps it
from locking, turning the screens off and sleeping while the user is
away, but not once it is locked, since nobody is watching then; or only
`sleep`, which holds while it is locked too. It lasts for as long as the
connection does, so a client that ends lets go of it. The portal backend
that comes with omorfos speaks so for programs that ask the desktop over
D-Bus; a program that speaks Wayland asks by `idle-inhibit` instead.

After `"subscribe"` the connection carries updates only. Each update is the
whole state, not a difference, so a client never has to reconstruct
anything.

`"show_problems"` is `"subscribe"` with a promise: that the program shows
the user what is in `problems`, each until it is dismissed or put right,
and what is in `events`, each once.
For as long as such a program is connected the compositor does not show
them itself, and it shows them again the moment the last one goes. The
notifications that come with omorfos ask this. A notification program of
your own may too; one that only wants to know the state asks `"subscribe"`.

### State

```json
{
  "workspaces": [
    { "id": 1, "output": "eDP-1", "number": 1, "name": "web", "active": true, "kept": true }
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
      "minimized": false,
      "shaded": false,
      "above": false,
      "sticky": false,
      "handle": "5d3a0f6c1b2e4a7f8c9d0e1f2a3b4c5d"
    }
  ],
  "problems": [],
  "events": [],
  "screens": [
    {
      "name": "eDP-1",
      "x": 0,
      "y": 0,
      "width": 1536,
      "height": 864,
      "mode": "3840x2160@60",
      "preferred": "3840x2160@60",
      "modes": ["3840x2160@60", "3840x2160@48"]
    }
  ],
  "locked": false,
  "awake": { "until": 1761400800 },
  "radios": [
    { "kind": "wlan", "off": false, "switched": false },
    { "kind": "bluetooth", "off": true, "switched": false }
  ]
}
```

`screens` are the screens in the order they stand, as `outputs` in
`config.yaml` lists them and then the rest, each with where it stands and
how large it is in units of layout, and the modes it can be driven in,
written as a `mode` is in `config.yaml`: the one it is driven in, the one
it says suits it best, and every one it offers, each once, the largest
and fastest first. The modes are empty where the backend does not say,
as inside another desktop. A screen that mirrors another comes after
those that stand on their own, with `mirrors` naming the other, and is
said to stand where that one does; `mirrors` is empty, or absent from an
older compositor, for one that stands on its own. A screen that is off
comes last, with `off` true and no size; `off` is false, or absent from an
older compositor, for one that is on.

`locked` is true while the session is locked and nothing of it is on any
screen: a screen locker holds it, and a frame with only the locker on it
has been shown everywhere. It is false while a lock is still being put
up, so a program that has to wait for the screen to be locked, as
`omorfos-session` does before the machine sleeps, waits for it to be
true. Asking for the `lock` action is refused with an error when nothing
is set to lock the session with, so that such a program knows at once.

`radios` are the kinds of radio the machine has, as the kernel names
them (`wlan`, `bluetooth`, `wwan`, `gps`, `fm`, `nfc`, `uwb`), each `off`
when every radio of the kind is, and `switched` when a switch or the
firmware holds one off, which no program can undo. They are what
`omorfos-session` says with `"radios"`, which holds for as long as its
connection does; empty while nothing has said, or from an older
compositor.

`awake` is there while the user has asked for the session to stay awake,
by the `stay_awake` or `stay_awake_for` action, with `until` the time it
ends in seconds since the Unix epoch, or `null` if it lasts until it is
asked to end. It is `null`, or absent from an older compositor, otherwise.

`shaded`, `above` and `sticky` say how a window's frame stands apart, as
the titlebar's menu, the actions of those names and rules set it: rolled
up into its titlebar, kept above the windows that are not, and on every
workspace of its screen. A sticky window's `workspace` is the one it was
on when it was made so; it shows on whichever is shown.

A window's `handle` is what it is called in the list of windows that
`ext-foreign-toplevel-list-v1` gives a program with the `panel` role, so
that a program speaking both can tell which of those windows this is.

`problems` is what is wrong and stays so until it is put right: each is a
line for the user, such as `theme.css:2:13: ...`. `events` is what has
happened in the last few seconds that the user might want to know of and
need do nothing about, such as the X server having been started again
after it stopped. Each is there for about ten seconds and then is not, so
a program that shows them shows each once when it first sees it.

Each screen has its own set of workspaces, numbered from 1, and shows one
of them. They are listed screen by screen, so numbers repeat and one
workspace per screen is `active`. Workspaces are made and go while the
session runs, and a workspace's `number` changes when one before it goes:
its `id` does not, and is never used again in a session, so that is what
to tell them apart by. `kept` is true of those the configuration names,
which are always there; one that is not goes once it is empty and another
is shown. A workspace that was made has its number for a `name`.

A window made fullscreen is alone on a workspace made for it, and two can
share one, each filling half of the screen. Such a workspace has
`"windows"`, the `id`s of its windows in the order of their parts; its
`number` is 0, since no key counts to it; and its `name` is the titles of
its windows. A window on one has 0 for its `workspace`. Other workspaces
have no `"windows"`. One with `"room": true` can take another window:
`{"join_workspace": ID}`, with its `id`, puts the focused window there and
shows it, and is answered with an error where there is no room. A window's `output` and `workspace` say which it is on. `output`
is empty while no screen is connected. Windows are listed in a stable order (by
when their frame was created, then tab order), so a task list does not
reshuffle. `shown` is false for a tab that is not the one its frame is
showing.

### Where windows are

```json
{
  "placed": [
    { "id": 7, "handle": "5d3a0f6c1b2e4a7f8c9d0e1f2a3b4c5d", "output": "eDP-1",
      "x": 120, "y": 64, "width": 1280, "height": 800 }
  ]
}
```

`"placed"` is answered with each window that can be seen, the one on top
first, and with where it is on each screen it is on: the part of it on
that screen, in the screen's pixels from its top left corner, as a
picture of the screen has them. Only what the window draws is counted,
not its frame, and a window rolled up into its titlebar is not there.
This is how a program that takes pictures, the picker of `Shift+Print`
among them, knows which window is under the pointer.

It is not in the state, and not sent to subscribers, since it changes
each time a window moves.

### Versioning

`protocol` rises only when a change would break existing clients. Fields
and requests may be added without changing it, so clients should ignore
fields they do not know.

### Limits

The compositor never waits for a peer. A line longer than 64 KiB, a peer
that does not read what it is sent (more than 1 MiB pending), and a peer
that stops listening are each disconnected. At most 32 peers are served.

## From Rust

The `omorfos-ipc` crate has the message types and a small client:

```rust
use omorfos_ipc::{Client, Reply, Request};

let path = omorfos_ipc::socket_path_from_env().expect("is omorfos running?");
let mut client = Client::connect(&path)?;
client.send(&Request::Subscribe)?;
while let Ok(Reply::State(state)) = client.receive() {
    // redraw from `state`
}
```

A program with an event loop of its own calls `set_nonblocking(true)`,
waits for the client's descriptor to be readable, and then calls
`try_receive` until it returns `None`. The panel does this.
