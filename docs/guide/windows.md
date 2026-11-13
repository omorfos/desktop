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

## Workspaces

The workspaces listed are always there. More are made as they are wanted,
after those: by `Super+Ctrl+n`, by sending a window to a new one
(`Super+Ctrl+Shift+n`, or "Send to workspace" in the menu of its titlebar), or
by going to the number one past the last, so that `Super+2` in a session
with one workspace makes a second. A workspace that was made is called by
its number, and goes again once nothing is on it and another is being
shown. A screen has 32 at most.

Going to another workspace slides it in from the side it lies on: the
next from the right, the one before from the left. The row does not go
round. There is nothing before the first workspace; after the last, if a
window is on it, `Super+Right` makes a new one, and goes no further than
that while it is empty. A swipe towards where there is nothing gives a
little, and goes back.

On a touchpad, three fingers swiped left carry the workspace with them
and bring the next in from the right, and swiped right bring in the one
before, each with its own wallpaper, as when a key changes the
workspace. Lift them past half way, or flick, and the one that came in is
shown; lift them short of that, or turn back, and the one that was there
stays. Three fingers swiped up open the overview (see [The overview](windows.md#the-overview)),
and down close it. Every other gesture, as a pinch to zoom, is the
program's under the pointer.

Each screen has its own workspaces, and shows one of them: switching
workspace on one screen leaves the others as they are. A window dragged
onto another screen joins the workspace that screen is showing. When a
screen is unplugged, its windows move to the workspace of the same number
on the first screen that is left, which is made if it is not there.

## Fullscreen and tiles

A window made fullscreen, by the second button of its titlebar, by
`Super+Ctrl+f`, or because its program asked, goes alone to a workspace made
for it, directly after the one it was on, with no panel over it and no
titlebar. That workspace has no number: `Super+Right` and `Super+Left` go
along the whole row and reach it, and the number keys count desktops only.
It goes when the window closes or leaves fullscreen, which takes the
window back where it was. A dialog that belongs to it opens over it; any
other window, another of the same program included, opens on the desktop
it came from. That desktop is shown with it if you asked for it, by
starting it from omorfos (the launcher, a key, the menu) or from a
program that says you did; otherwise it waits there, in the taskbar,
and you stay where you were. Maximizing is another thing: it fills the desktop the window
is on and keeps the panel and the titlebar.

Up to four windows can share such a workspace. "Tile" in the menu of a
titlebar, or the `tile` action, has a window take a half of where it is,
and the next window to open there has the other half: the launcher, used
there, puts a program beside the one already there. From a desktop,
"where it is" becomes a new workspace. A window that already has a half
can take a half of that, the other way, which makes room for a third; and
the same on the other side makes room for a fourth. Nothing is cut more
than twice, so the arrangements are two side by side or one above the
other, one beside two, one above or below two, and four in quarters.
Asked for a half it cannot take, a window goes to that half instead,
changing places with what was there.

Rest the pointer on the second button of a window's titlebar and small
pictures of the seven arrangements come up under it, with an arrow to
the button they belong to and a line of words that says what the part
under the pointer is. Click a part of one
and a workspace is made cut up that way, with the window in that part and
the others waiting for windows to open in them. A click on the button
itself is still for fullscreen.

Or drag a window by its titlebar until the pointer is against an edge of
the screen. The half it would take is washed over while it is held:
left or right at the sides, top or bottom at the top and bottom, and a
quarter near a corner. Let go there and it is put in that part of a new
workspace. Drag it away from the edge again and nothing happens. An edge
with another screen beyond it is the way to that screen and offers
nothing.

A window that is open already is put beside a fullscreen one, or into a
part that has nothing in it, from the menu of its own titlebar: under
"Send to workspace", each workspace that has room is listed as "Beside"
and the windows there. Beside a fullscreen window it takes the right
half. You are taken there with it.

`Super+Ctrl` and an arrow gives focus to the window beside the one that
has it, and `Super+Ctrl+Shift` and an arrow changes the two over. With the
pointer, drag a window by its titlebar onto another part: the part is
washed over while it is held, and let go there the two change places.

Windows that share a workspace keep their titlebars, each across the top
of its own part, so that which has focus can be seen and each can be
closed, sent back to the desktop by its second button, or dragged. Only
the panels are hidden. A window that has a workspace to itself has
nothing over it, as fullscreen means.

There is a line wherever two parts meet: drag it to give one more room
and the other less, down to a sixth of the screen or to the least a
window says it can be, and double-click it to put it back in the middle,
which it and the windows either side of it go to as a window sent
somewhere does.
Dragged, it keeps pace with the windows, going on as they have drawn
themselves the size it gave them, and is let go where the pointer is.
The theme draws it as `tile-line`, in a window's border colour unless
told otherwise. When a window closes or leaves, the one beside it has
its room. A window that opens when every part is taken opens on the
desktop.

A part with nothing in it shows the windows of the desktop the workspace
was made from, small, each in a box with its icon and its title under
it, which the theme draws as `offer`: its border round the box, and its
colour and font for the title, `offer:hover` under the pointer. The icon
is the one the program gives its window, or else its application's, as
in the switcher. The pictures of a row stand on one line and their titles
are on another. Click anywhere in a box to put its window there. A window
a rule makes `private` is shown as a blank, in the colour of the
`capture` part, with its icon large in the middle and its title under it
all the same, since that is how it can be told.

On such a workspace, rest the pointer against the top of the screen for
six tenths of a second, or hold it there and push on against the top a
good way, and whatever panels are along the top come back at once. A
quick flick at the top and away brings nothing. Where
one window has the workspace to itself its titlebar comes back under
them: its buttons, its menu, and the second button to leave fullscreen. Against the bottom, the panels along the
bottom come back. They slide in from that edge, the titlebar with the
panels, over the time the theme's `panel` transition gives one to come
in, and out the same way over the time `panel:closing` gives one to go:
180 ms and 120 ms in the built-in theme. With `panel { transition: none;
}`, or `reduce_motion`, they are there and gone at once. They are laid
over the window, which does not change size. They stay while the pointer is on them or still moving, so that a
titlebar further down the screen can be reached, and go a second after
the pointer has stopped somewhere else, or when a key is pressed or a
button is pressed in a window. This is
omorfos's doing and no program can turn it off or imitate it, so it is
also how to tell a fullscreen program from the desktop.
