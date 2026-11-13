# Using windows

Windows that support it get a titlebar and border drawn by omorfos.
Applications that insist on drawing their own, such as GTK ones, are left
alone; a rule with `decorations: true` forces a frame onto them anyway.

| To | Do this |
|---|---|
| Close a window, make it fullscreen, or minimize it | Click the outermost, middle or innermost button at the right of its titlebar |
| Put two windows side by side | Rest the pointer on the middle button of one's titlebar and click a half of the first picture, then open the other. Or right-click the titlebar and choose Tile |
| Put a window in a half or a quarter of the screen | Drag it by its titlebar to an edge of the screen, or to an edge near a corner, and let go |
| Give one of two windows side by side more room | Drag the line between them; double-click it to halve the screen again |
| Leave fullscreen | `Super+Ctrl+f`; or rest the pointer against the top of the screen and click the second button of the titlebar that comes back |
| Reach the panel from a fullscreen window | Rest the pointer against the edge of the screen the panel is along |
| Maximize a window, or put it back | Double-click its titlebar |
| Roll a window up into its titlebar, keep it above the others, show it on every workspace, or send it to one | Right-click its titlebar and choose from the menu |
| Move a window | Drag its titlebar, or hold Alt and drag anywhere on it |
| Resize a window | Drag a border or corner, or hold Alt and drag with the right button |
| Group windows as tabs | Drag a titlebar with the middle button onto another titlebar |
| Switch tab | Click the tab |
| Ungroup a tab | Drag it with the middle button and drop it off any titlebar |

## Switching windows

`Super+Tab`, or `Alt+Tab`, opens the switcher in the middle of the
screen the pointer is on: every window, on every workspace and minimized or not, as a picture
of itself, in the order they were last used. A window smaller than its
place is shown as large as it is, not made larger. It opens at the
window used before the one you are in, so one `Super+Tab` goes back to it
and another comes back again. It is shown once Super, or Alt, has been
held for 200 ms, so that a quick `Super+Tab` goes back without the switcher
showing for a moment; opened by something other than keys, it is shown
at once. Shown, it fades in over 120 ms, with the dim and blur behind
it, and goes at once. A click before it is shown closes it, choosing
nothing.

The windows stand one behind another on a diagonal: the one picked in
front, with its title and icon under it, and those after it going back
up and to the right, each smaller and fainter, six at most. Going on to
the next window turns the queue: the one in front fades out and goes to
the back, and the next comes forward, over a quarter of a second, or at
once with `appearance.reduce_motion`. With `style: flat` they
stand side by side in rows instead, each small over its title, in a box:

```yaml
switcher:
  style: flat
```

While Super, or Alt, is held, Tab goes on to the next window and
`Shift+Tab` back, as do the arrows; letting go of it goes to the window
picked, bringing
it back if it was minimized and showing its workspace. Enter does the
same, Escape closes the switcher where you were, and a click on a window
goes to it. `Super+Shift+Tab`, or `Alt+Shift+Tab`, opens it at the
window used longest ago.

The order stands still while the switcher is open: a window passed over
on the way has not been used. Each window's icon is beside its title:
the one its program gives it, or else the one its application is
installed with (the `Icon` of its desktop entry, or the icon named as
its `app_id` is). A window a rule says is `private` is shown as it is
in pictures of the screen: blank, in the background colour of the
`capture` part, with its icon large in the middle; or, with `private:
blur`, its picture blurred, and blank where blur cannot be drawn. How the switcher looks is `switcher`,
`switcher-item` (the one picked is `:active`, and `min-width` is how wide
each is in rows) and `switcher-title` in `theme.css`. On a diagonal,
`switcher` is the box under the queue that the title is in, and each
window is a `switcher-window`: its border round its picture, its
`padding` away, and the accent round the one picked. Behind the
switcher, its screen is dimmed by `switcher-backdrop`'s background and
blurred out of focus by its `backdrop-filter`, in rows as on a diagonal;
with `appearance.reduce_transparency` it is the dim alone. The actions are
`switch_windows` and `switch_windows_back`; `focus_next` and
`focus_previous` still go from window to window on a workspace without
it.

## The overview

The overview shows every window of the workspace on the screen the
pointer is on, laid out small side by side, under a strip of that
screen's workspaces. Three fingers swiped up the touchpad open it, the
windows moving up and shrinking to their places with the fingers and the
screen dimming as they go; lift them past half way, or flick, and it
opens the rest of the way, or short of that and it sinks back. Swiped
down, they close it the same way. With `appearance.reduce_motion` the
fingers carry nothing, and it opens or not as they lift. The Mission
Control key of a Mac's keyboard (`XF86LaunchA`, F3 on a MacBook) opens
it and closes it. On a keyboard without one, bind the `overview` action
in `keys.yaml`:

```yaml
bindings:
  - keys: Super+Up
    action: overview
```

Opened, each window glides from where it is to its place, and the screen
behind dims and blurs; closed, each glides back. With
`appearance.reduce_motion` they fade in their places instead. A window
smaller than its place is shown as large as it is, and a minimized one
fades in where it goes. The window under the pointer, or reached with the
arrows, is picked: a border round it and its title under it. Enter or a
click goes to it, once the overview has closed; Escape, or a click where
nothing is, closes it where you were. Any other key is a binding as
ever, so `Alt+Tab` opens the switcher in its stead.

The strip has each desktop of the screen as a small picture of itself,
its wallpaper and its windows where they stand, the one shown in the
accent. Point at one and it is lit; click it and the overview shows its
windows, which glide in from where they are. The plus after the last
makes a new workspace and goes to it, empty, for windows to come. A
private window is blank or blurred, as in pictures of the screen.

How it looks is `overview-window` (the border round the window picked,
`:active`), `overview-title`, `overview-workspace` (the one shown is
`:active`, one under the pointer `:hover`), `overview-add` (the plus, in
its `color`, `:hover` under the pointer) and `overview-backdrop`, whose
background dims the screen and whose `backdrop-filter` blurs it.
