# The omorfos guide

## Your first login

The first time, a card in the middle of the screen goes through a few
steps, one at a time: a welcome; what the few keys that matter do; a
light or dark desktop, or one that follows the day, each shown as a
small picture of the desktop; the wallpapers that come with omorfos; and
the applications that open the web, files, mail, pictures, music and
video, as the system has them (Settings is to change them). A choice is
saved to `config.yaml` as it is made, the rest of the file kept as it is
written, so the desktop behind the card changes at once. Next goes on and
Back goes back; the arrows, Tab and Enter, or the pointer, choose. Skip,
Escape, or Finish on the last step closes it, keeping what was chosen, and it is not shown at a login
again. The `show_welcome` action shows it again, from a key or an entry
of `menu.yaml`. Where `config.yaml` is written in a way it will not change, as a
section written on one line, it says so and leaves the file alone. It is
drawn with the launcher's parts of the theme, its buttons as a question's
(`question-button`).

There is a panel along the bottom of every screen, with its workspaces,
the windows open and the time; notifications come in at the top right.
`Super+Return` starts a terminal, `Super+space` the launcher, to start
anything else by name, and a right click on the desktop opens the menu.

Super is the desktop's (see [Super](keys.yaml.md#super)), and its keys
are a Mac's, with Super for Command: a chord with it that the desktop does
not use reaches the program as the same chord with Ctrl, so `Super+c`
copies, `Super+v` pastes, `Super+z` undoes and `Super+w` closes a tab in
every program, as Cmd does on a Mac.

The desktop speaks the language your session is in, as `LANG` says,
where it has the words: English, and Polish for the panel, the lock
screen, the boxes that ask for passwords, the welcome card, the notices
of the session and of the compositor, the menus, "About", and the names
the launcher gives programs, as their desktop entries give them. The
manual pages and `--help` are in English.

## The keys that matter

| Keys | What they do |
|---|---|
| `Super+Return` | Start a terminal |
| `Super+space` | Open the launcher |
| `Super+Tab` | Go back to the program before, or hold Super to choose one |
| `Super+grave` | Go to the program's next window (the key above Tab) |
| `Super+q` | Quit the program: close every window of it |
| `Super+m` | Minimize the window |
| `Super+h` | Hide the program; `Super+Alt+h` every other |
| `Super+Alt+Up` | Maximize the window, or restore it |
| `Super+Alt+Left` | Have the window take the left half; `Super+Alt+Right` the right |
| `Super+Ctrl+f` | Make the window fullscreen, or not |
| `Super+Ctrl+Up` | See every window of the workspace |
| `Super+Ctrl+Down` | See the windows of the program in front alone |
| `Super+1` | Go to the first workspace; up to `Super+9` |
| `Super+Ctrl+1` | Send the window to the first workspace |
| `Super+Ctrl+Right` | Go to the next workspace; `Super+Ctrl+Left` the one before |
| `Super+Shift+3` | Take a picture of every screen; `Super+Shift+4` pick what of |
| `Super+Ctrl+q` | Lock the screen |
| `Super+Shift+q` | Log out, asking first |

Every key there is before you write any is in the table of
[the built-in bindings](keys.yaml.md#built-in-bindings), and what windows
do with the pointer, as moving, resizing and tiling them, in
[Using windows](windows.md).

## Where the files are

omorfos reads YAML files, and a stylesheet for the theme, from
`$XDG_CONFIG_HOME/omorfos/`, which is usually `~/.config/omorfos/`. Every
file is optional, and every change is taken up as soon as it is saved:

| File | What it holds |
|---|---|
| [`config.yaml`](config.yaml.md) | How the desktop behaves: screens, keyboard, touchpad, workspaces, its parts |
| [`keys.yaml`](keys.yaml.md) | The keys, and what each does |
| [`rules.yaml`](rules.yaml.md) | How particular windows are treated |
| [`menu.yaml`](menu.yaml.md) | The menu |
| [`theme.css`](theme.css.md) | How it looks |

If a file has a mistake, omorfos keeps using that file's previous contents
and reports the problem with its file, line and column. The other files
still apply. If omorfos starts again before the mistake is put right, after
a crash or to take up a new version, it still uses the previous contents,
which it keeps until you log out; at your next login, a file with a mistake
starts out with the defaults. Problems are shown among the notifications, at the top right
of the screen, each until you click it away or put it right, and are
printed by `omorfosctl config check`. Where no notifications are being
shown by omorfos, they are in a notice of the compositor's own in the same
corner.

General rules:

- Unknown settings are errors, so typos are caught.
- Booleans are `true` or `false` only.
- Files larger than 1 MiB are refused.

Each file has a manual page as well, made from the same schemas, with
every key it may have: `man 5 omorfos-config` for `config.yaml`, and
`omorfos-keys`, `omorfos-rules`, `omorfos-menu` and `omorfos-theme` for
the others. `omorfos-keys` lists the built-in bindings too, and
`omorfos-menu` the built-in menu, written as the file would write them.
Every program has one in section 1, as `man omorfos`.

For completion and inline validation in editors that use
`yaml-language-server`, point a file at its schema in
[`schemas/`](../../schemas), for example:

```yaml
# yaml-language-server: $schema=/path/to/schemas/keys.schema.json
```

## Binding what has no key

Some actions have no key until you give them one: the overview, locking
the session, tiling a window to one half of the screen. `omorfosctl
action` lists every action, and `keys.yaml` binds them:

```yaml
bindings:
  - keys: Super+F3
    action: overview
  - keys: Super+l
    action: lock          # with `locker` set in config.yaml
  - keys: Super+Alt+Left
    action: { tile: left }
```

A key you bind with Super is the desktop's from then on, and no longer the
program's as Ctrl: `Super+l` bound is no longer what `Ctrl+l` is in a
browser.

## Everything else

- [Using windows](windows.md): moving, resizing, tiling and grouping them,
  the switcher, the overview, workspaces and fullscreen
- [Screens](screens.md): placing them, mirroring one, turning one off, and
  how large things are drawn
- [The panel](panel.md), [the wallpaper](wallpaper.md),
  [the launcher](launcher.md) and [notifications](notifications.md)
- [Pictures of the screen](screenshots.md): screenshots, recording, and
  sharing the screen
- [The clipboard](clipboard.md)
- [Sound and light](sound-and-light.md): the volume, the brightness, what
  is playing, and night light
- [Locking the screen](locking.md)
- [Passwords and passphrases](passwords.md) programs and keys ask for
- [Typing in other languages](typing.md)
- [Sleep, restart and shut down](power.md)
- [What applications are told of the desktop](applications.md)
- [Who may do what](permissions.md)
- [Programs written for X11](x11.md)
- [Under WSL](wsl.md), in a window on the Windows desktop
- [Installing a new version](updating.md)
- [When something is wrong](troubleshooting.md)
