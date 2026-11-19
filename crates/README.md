# The crates

Each program and library of omorfos is a crate here, side by side.
[docs/ARCHITECTURE.md](../docs/ARCHITECTURE.md) says how they are
layered and which may depend on which; this is only what each is, so
that one can be found. Which of the programs are to run where omorfos
is not installed is in
[ADR 98](../docs/adr/0098-which-programs-stand-alone.md).

## The compositor, and what keeps its session

| Crate | What it is |
|---|---|
| `omorfos` | The compositor: windows and their frames, menus, the switcher, the overview, tiling, and everything it draws itself |
| `omorfos-hold` | Holds the session while the compositor is started again, so that a new version, or a crash, does not end the login |
| `omorfos-relay` | Stands between programs and the compositor, so that it can be started again without closing them |
| `omorfos-session` | What a session does besides draw: locking before sleep, starting programs at login, the volume, the brightness, what is playing |
| `omorfosctl` | The command line: what the compositor is asked and told |

## The shell, each a program of its own

| Crate | What it is |
|---|---|
| `omorfos-panel` | The bar: workspaces, windows and the clock along the edge of a screen |
| `omorfos-launcher` | Type a program's name to start it, or a sum to copy its answer |
| `omorfos-notifications` | What programs have to tell the user |
| `omorfos-wallpaper` | A picture or a colour behind the windows of each screen |
| `omorfos-lock` | The lock screen |
| `omorfos-welcome` | The card shown at a first login |
| `omorfos-ask` | What asks for a secret, in the sealed prompt; `omorfos-pinentry` is built beside it |
| `omorfos-screenshot` | Pictures of the screen, a window or a region |
| `omorfos-recording` | Says what is recording from a microphone or a camera |
| `omorfos-clipboard` | Keeps what was copied after its program has closed |
| `xdg-desktop-portal-omorfos` | What applications ask the desktop through the portals, answered for omorfos |

## What they are built from

| Crate | What it is |
|---|---|
| `omorfos-core` | The model: windows, frames, workspaces and screens, knowing nothing of Wayland, files or the clock |
| `omorfos-config` | The files of settings: their types, their checking, the built-in theme, and reading them again as they change |
| `omorfos-ipc` | What is said over the compositor's socket, a client for it, and the list of Wayland protocols it offers |
| `omorfos-ui` | Drawing: the canvas, text, and the parts every program draws alike |
| `omorfos-shell` | What the shell's programs share as Wayland clients |
| `omorfos-picker` | Picking a screen or a region of it, over still pictures of the screens |
| `omorfos-icons` | Finding and reading programs' icons, and drawing the signs of the interface |
| `omorfos-entries` | Reading the desktop entries programs install |
| `omorfos-i18n` | The words of every program in the user's language; the messages are in `i18n/` here |
| `omorfos-brand` | Draws the mark, the icon and the wallpapers from the rules of the brand |
| `omorfos-sums` | Works out sums as they are typed |
| `omorfos-journal` | Hands what a program writes to the systemd journal, under its name |
| `omorfos-pipewire` | Holds PipeWire's buffers while a picture is put in them: the one place with `unsafe` |
| `omorfos-shm` | Writes pixels into memory a program shares, through the file behind it |

A new crate is added to the workspace's `Cargo.toml`, to
`.github/release-please-config.json`, to this list, and, if it is a
program, to the `Makefile` and the packaging (see
[AGENTS.md](../AGENTS.md)).
