# Who may do what

A program sees and works its own windows. Some things reach further, and
are only for a program omorfos started itself and was told may do them:

| Role | Lets a program |
|---|---|
| `panel` | Put surfaces on the screen outside windows, as a bar, a dock, a wallpaper or a notification does, and be told what windows there are |
| `overlay` | The same, and take the keyboard from whatever has it while one of its surfaces asks, as a launcher does |
| `lock` | Lock the session |
| `capture` | Take pictures of the screen |
| `clipboard` | Read what is copied and set it at any time, not only while one of its windows has the keyboard, as a clipboard manager does |
| `input` | Be the input method: be shown what is typed into whatever has the keyboard, take the keys, and type as a keyboard would |
| `ask` | Ask for a secret in the sealed prompt: the session dimmed behind it, and the keyboard and pointer its alone, as `omorfos-ask` does |

`screens` is a name for what is to come, and is accepted so that a
configuration can say it already.

A role is given where the program is started: in `autostart`, or in a
`spawn` bound to a key or put in the menu. A program given one stays open
through `restart_compositor`, with its role, as other programs do.

```yaml
# config.yaml
autostart:
  - { run: [waybar], may: [panel] }
  - { run: [swaybg, -i, wall.png], may: [panel] }
```

```yaml
# keys.yaml
bindings:
  - keys: Super+d
    action: { spawn: { run: [fuzzel], may: [overlay] } }
```

A program started any other way, from a terminal say, is an ordinary
program whatever it is called. One that asks for what it has no role for
is refused, and you are told once what asked and what to write to let
it. The panel, the launcher and the notifications that come with omorfos
have what they need; started by hand, they are ordinary programs too.

A program in a sandbox, such as one installed with Flatpak, is held to
less again. It is not shown that panels, locking, pictures of the screen,
the clipboard at any time, input methods or the list of windows exist, so it cannot ask, and no setting gives it a role. Nothing needs
to be set up for this: the sandbox tells omorfos which programs are
inside it.

This keeps a program that is merely running from putting a false panel
or a false lock screen in front of you. It is not a wall against a
program that can already change your files: such a program could write
itself into `autostart`.
