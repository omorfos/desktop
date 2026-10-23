# omorfos

A Wayland desktop for Linux, written in Rust: small, fast, driven by text
files, with windows that can share a frame as tabs, and growing into a
whole desktop with a modern look. *Omorfos* is Greek for "beautiful".

> **Status:** early development, and not released yet. It runs as the
> session you log in to, on real hardware, but has been tried on very
> few machines. Expect rough edges and changing defaults. What is done and what is next is in
> [docs/ROADMAP.md](docs/ROADMAP.md).

## What there is

- **Windows** with titlebars drawn by omorfos: close, fullscreen and
  minimize buttons, a menu on a right click, tabs, rounded corners,
  shading, keeping above, and showing on every workspace.
- **Workspaces made as you need them**, per screen, and gone when they
  are empty. A fullscreen window gets one to itself; rest the pointer
  against the top of the screen to get its titlebar back.
- **A panel, a launcher and notifications**, each a small program of its
  own, started again if it stops, and replaceable by any other.
- **Programs written for X11**, through Xwayland, with the clipboard
  shared both ways.
- **Text files for everything**, in `~/.config/omorfos/`: settings and
  keys in YAML, the look in CSS. They are read again the moment they are
  saved, and a mistake in one is reported on screen and never takes the
  session down.
- **Several screens**, each at its own scale, fractional scales included.
- **A socket to script it by**, with `omorfosctl`.

## What it holds to

- **Secure.** No `unsafe` in our own code but for one block, kept in a
  crate of its own; every program, every peer on
  the socket and every configuration file is treated as untrusted.
- **Correct.** Every line is covered by a test, outside the two files
  that need a real screen to run. The compositor is tested through a
  real Wayland client, looking at the pixels it draws.
- **Fast.** Nothing is drawn when nothing changes.
- **Lean.** The compositor manages windows. What can be a program of its
  own is one.

Linux on `x86_64` and `aarch64`.

## Trying it

Build it, then run it from inside the desktop you already use. It opens
in a window and prints the name of its socket:

```sh
cargo run -p omorfos
```

Start a program inside it with, for example, `WAYLAND_DISPLAY=wayland-1
foot`. Key bindings that use Super may be taken by the host desktop
before they reach the window.

To use it as your session, build it, install the programs, the entry a
login screen reads and the lock screen's PAM services, then pick omorfos
when you log in:

```sh
make
sudo make install
```

The programs go in `/usr/local/bin`, or wherever `PREFIX` says; what
login screens, the portal and systemd read goes where they look for it.
`install` builds nothing, so build first as yourself, and it keeps the PAM
services in `/etc/pam.d` if they are there already. `sudo make uninstall`
takes it all away but those.

It can also be started from a text console by running `omorfos`.
`Ctrl+Alt+F1` to `F12` switch virtual terminal.

A few keys to begin with:

| Key | Does |
|---|---|
| `Super+Return` | A terminal (`foot`) |
| `Super+Space` | The launcher |
| `Super+Escape` | The menu |
| `Super+q` | Close the window |
| `Super+f` | Fullscreen, and back |
| `Super+n` | A new workspace |
| `Super+1` to `Super+9` | Go to a workspace |
| `Print` | A picture of every screen, saved in `Pictures/Screenshots` |
| `Shift+Print` | Pick a window, a screen or a region, on the clipboard |
| `Alt+Print` | A picture of the window with focus |
| `Super+Shift+r` | Read the configuration again |
| `Super+Shift+e` | End the session |

Everything else, and how to change any of it, is in
[docs/CONFIGURATION.md](docs/CONFIGURATION.md).

## What it needs

To build: Rust (the version is pinned in `rust-toolchain.toml`),
`pkg-config`, `clang`, and the development files for libxkbcommon, pixman,
udev, libinput, libseat, GBM, PipeWire and PAM. On Debian and Ubuntu:

```sh
sudo apt install pkg-config libxkbcommon-dev libpixman-1-dev libudev-dev \
  libinput-dev libseat-dev libgbm-dev libpipewire-0.3-dev libpam0g-dev \
  libclang-dev clang
```

To run: EGL and OpenGL ES (`libegl1 libgles2`), and a seat from logind or
seatd. The rest is wanted, and done without if it is not there:

| | For |
|---|---|
| `Xwayland` | Programs written for X11 |
| `foot` | The terminal the default keys start |
| A D-Bus session, and `dbus-update-activation-environment` | Notifications, and telling D-Bus and systemd of the session |
| `xdg-desktop-portal` | Applications following the look of the desktop, and asking for screenshots |
| `xdg-desktop-portal-gtk` | Choosing files, which omorfos leaves to it |
| `gdbus` (`libglib2.0-bin`) | Starting the portal at login once omorfos's own part of it is there |
| PipeWire | Sharing the screen in a browser or a call |
| `wpctl` (WirePlumber) | The volume keys |
| `wl-copy` and `wl-paste` (`wl-clipboard`) | Copying and pasting in an editor in a terminal, such as Neovim, which otherwise goes through X11 and cannot read what programs written for Wayland copy |

To run the tests as well: `dbus-daemon`, `pipewire`, and `Xwayland`,
which needs `libEGL`; `pam_wrapper` (`libpam-wrapper`), which the lock
screen's tests ask PAM through; and the DejaVu fonts to draw as CI does.

## Documentation

| | |
|---|---|
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | Every setting, key, rule and part of the theme, and how to use the desktop |
| [docs/IPC.md](docs/IPC.md) | The socket and `omorfosctl`: scripting, and writing your own panel |
| [docs/ROADMAP.md](docs/ROADMAP.md) | What is done, what is next, and in what order |
| [docs/DESIGN.md](docs/DESIGN.md) | What it is to look like, and the rules a part is drawn by |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | How the code is laid out |
| [docs/adr/](docs/adr/) | Each decision that shaped it, and why |
| [CREDITS.md](CREDITS.md) | Whose work it is drawn with |

## Working on it

[AGENTS.md](AGENTS.md) is the place to start, for people as much as for
agents, and [docs/TESTING.md](docs/TESTING.md) says how things are
tested. Before a change goes in, all of these pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo coverage
cargo test --workspace --all-features --doc
cargo deny check
```

`cargo coverage` is the coverage gate, an alias in `.cargo/config.toml`.
The tests also need `dbus-daemon`, `Xwayland` and `pam_wrapper`, and
`Xwayland` needs `libEGL`. Commits follow
[Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/),
and releases are made from them: see
[docs/RELEASING.md](docs/RELEASING.md).

## License

[MIT](LICENSE)
