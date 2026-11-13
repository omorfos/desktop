# Typing in other languages

Two things help with typing what a keyboard has no keys for.

**Several layouts**, for languages written with their own letters: list
them in `keyboard.layout`, as `layout: 'us,ru'`, and bind `next_layout`
to a key to go from one to the next.
XKB's own options work too, as `options: 'grp:alt_shift_toggle'`.

```yaml
# keys.yaml
bindings:
  - keys: Super+space
    action: next_layout
```

Key bindings go by the first layout that writes in Latin letters, so
`Super+t` is the same key whichever layout is being typed with.

**An input method**, for languages with more characters than a keyboard
has keys, such as Chinese, Japanese or Korean: a program such as
`fcitx5` turns what is typed into the characters meant, and shows those
it could mean beside the text. Start it with the `input` role:

```yaml
# config.yaml
autostart:
  - { run: [fcitx5, --replace], may: [input] }
```

Not with `-d`: a program that puts itself in the background as it starts
lets go of the connection it was given the role on. It then works in any
program that says where text is typed (GTK, Qt and the browsers do), in
windows and on panels, and its popup is shown under the text, or above
it where there is no room. While the session is locked it is not shown a key:
what is typed then is your password. One input method at a time: should
a second be started, the first is told it may no longer be one, so
start only one.
