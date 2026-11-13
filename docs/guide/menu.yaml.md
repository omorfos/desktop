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

Before each label stands what it does. An entry that starts a program has
the program's icon: the one its desktop entry names, found by the name of
the program's file (`foot` finds `foot.desktop`), or else the icon theme's
icon of that name. Most others have a sign, such as a magnifying glass
for `show_launcher`; and one that turns something on and off, such as a
cup for `stay_awake`, has its sign in the accent on a faint square of it
while it is on, as `menu-sign:active` in the theme has it. Give an item
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

The menu a right click on a titlebar opens is the same: a sign for what
each row does, those of "Maximize", "Shade", "Keep above" and "Show on
every workspace" in the accent while the window is so, choosing the row
again undoing it. A submenu opens beside its row, a little over the edge
of the menu.

The menu opens with a right click on the desktop or the `show_menu` action
(`Super+Escape` by default). Click an entry to choose it; click elsewhere or
press Escape to close the menu.

The keyboard steers it too, and so the menu a right click on a titlebar
opens: Up and Down go through the rows and round, Right or Enter goes into
a submenu, Left comes back out of one, and Enter chooses an entry. While a
menu is open these keys are the menu's and no window sees them; with a
modifier held they are a binding's as ever.
