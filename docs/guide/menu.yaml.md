# `menu.yaml`

```yaml
items:
  - label: Terminal
    action: { spawn: [foot] }
  - separator
  - label: Session
    items:
      - { label: Reload configuration, action: reload_config }
      - { label: Quit, action: quit }
```

An item is the word `separator`, or a `label` with either an `action` or
nested `items`.

Without a `menu.yaml` the menu has the launcher, the web browser, what
manages files, the text editor, a terminal and Settings, then staying
awake, "About", locking the screen, sleep, logging out, restarting and
shutting down, said in the user's language; the last three ask first, as
`power.ask` in `config.yaml` says. Browser, Files and Editor open
whatever your system has as the default for web links, folders and plain
text, as its `mimeapps.list` says (the action `open`, of `browser`,
`files`, `editor`, `terminal` or `settings`); with none set, or for
Settings, which omorfos does not have yet, a notice in the corner says
so. Terminal opens the first terminal an `xdg-terminals.list` in
`~/.config` or `/etc/xdg` names, as `xdg-terminal-exec` reads it, with
one named `omorfos-xdg-terminals.list` read before it; with none named,
`foot`. So is an entry of `menu.yaml` named and
doing just as one of those, as `{ label: Sleep, action: sleep }` is; one named otherwise is shown as it is named, in whatever language it
is written.

Before each label stands what it does. An entry that starts a program has
the program's icon: the one its desktop entry names, found by the name of
the program's file (`foot` finds `foot.desktop`), or else the icon theme's
icon of that name. Most others have a sign, such as a magnifying glass
for `show_launcher`; and one that turns something on and off, such as a
cup for `stay_awake`, has its sign in the accent on a faint square of it
while it is on, as `menu > item:checked > icon` in the theme has it. Give an item
an `icon`, the name of one of the icon theme's icons or the path of a
picture, to have that instead; a submenu has nothing unless it has one:

```yaml
items:
  - { label: Browser, action: { spawn: [flatpak, run, org.mozilla.firefox] }, icon: org.mozilla.firefox }
  - label: Session
    icon: system-shutdown
    items:
      - { label: Quit, action: quit }
```

At the end of a row stand the keys that do what it does, as `keys.yaml`
binds them: Super, Ctrl, Alt and Shift by their signs, four squares, a
caret, the option sign and a broad arrow up, and Enter, Space, Tab,
Backspace and the arrows by theirs; other keys as written on them, as `Q`,
`Esc` or `Del`; each a little apart, where a `+` would be. So Run… has the
sign of Super and of Space beside it. A row whose action has no binding
has none. Their colour is `menu > item > kbd` in the theme.

The menu a right click on a titlebar opens is the same: a sign for what
each row does, those of "Maximize", "Shade", "Keep above" and "Show on
every workspace" in the accent while the window is so, choosing the row
again undoing it. A submenu opens beside its row, a little over the edge
of the menu.

The menu opens with a right click on the desktop or the `show_menu` action,
which has no key unless you bind one. Click an entry to choose it; click elsewhere or
press Escape to close the menu.

The keyboard steers it too, and so the menu a right click on a titlebar
opens: Up and Down go through the rows and round, Right or Enter goes into
a submenu, Left comes back out of one, and Enter chooses an entry. While a
menu is open these keys are the menu's and no window sees them; with a
modifier held they are a binding's as ever.
