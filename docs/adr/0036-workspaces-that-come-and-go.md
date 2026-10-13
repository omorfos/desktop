# 36. Workspaces that come and go

- Status: accepted
- Date: 2026-10-13

## Context

Each screen has the workspaces the configuration names, four unless told
otherwise, and they are always there (ADR 17). Two things are wanted that
this cannot do.

- Workspaces made when they are needed and gone when they are not, from
  the dock or a key, without editing a file.
- Fullscreen as macOS has it: the button on the titlebar puts the window
  alone on a workspace of its own, with no bar and no titlebar, and that
  workspace goes when the window closes or leaves fullscreen. The bar and
  the titlebar come back while the pointer is at the top of the screen.

## Decision

### Two kinds of workspace

Each screen has a row of workspaces, in an order, and shows one. A
workspace is one of two kinds.

**A desktop** is a workspace as now: any number of windows, the bar and
the dock in place.

- The ones named in `workspaces` in the configuration are always there
  and come first. By default there is one, where there were four.
- More are made as wanted: by the `new_workspace` action (`Super+n`), by
  "New workspace" in the dock, or by sending a window to a new one
  (`move_to_new_workspace`, `Super+Shift+n`, and the titlebar's menu).
  They are named by their number. A screen has 32 at most.
- One that was made, not named, is removed once it is empty and another
  is being shown. It is not removed from under the user: an empty one
  that is being shown stays until it is left.

**A solo** is a workspace made for one fullscreen window, and it is that
window's and nobody else's.

- Making a window fullscreen, by its button, by a key, or because the
  program asked (a film in a browser, `F11`, a game), makes a solo for
  it, directly after the workspace it was on, and shows it.
- When the window leaves fullscreen it goes back to the workspace it
  came from, where and as large as it was, and the solo is gone. If that
  workspace has gone meanwhile, it goes to the one now in its place.
- When the window closes, the solo is gone and the workspace it came
  from is shown.
- The program's own dialogs and menus open over it. A window of any
  other program opens on the workspace the solo came from, and that is
  shown if the user asked for the window.
- Windows kept on every workspace (ADR 31) are not carried onto a solo,
  where they would cover what was made fullscreen to be seen alone.
- A frame of several tabs goes as one, with the tab that had focus
  showing.

Fullscreen and maximized are now plainly different things. Maximized
fills the desktop it is on and keeps the bar and the titlebar.
Fullscreen leaves the desktop.

### The button, and the titlebar

- The titlebar's second button means fullscreen, as on macOS. Maximize
  stays where it also was: a double click on the titlebar, the key, and
  the titlebar's menu, which gains "Fullscreen".
- On a solo that button means "leave fullscreen".

### Finding the bar and the titlebar again

A solo shows the window and nothing else. Pushing the pointer against
the top of the screen brings down the bar, and under it the window's
titlebar with its buttons and its menu. Pushing against the bottom
brings up the dock.

- **Pushing, not touching.** They come when the pointer is moved on
  against the edge a little way, or rests on it for a third of a second.
  A pointer that only reaches the top, on its way to something the
  program has there, brings nothing.
- **They are laid over the window**, which is not told to be any other
  size. They go when the pointer has left them for a moment, or a key is
  pressed.
- **They are the compositor's.** The bar is shown by the compositor and
  the titlebar is drawn by it, over anything the program draws. A
  fullscreen program can draw something that looks like a desktop; it
  cannot answer a push at the top of the screen. That is how a user
  tells, and why this cannot be turned off by a program.
- **Not while a program holds the pointer.** A game that has locked or
  confined the pointer is not interrupted. Its user leaves by the key.
- **The keys always work.** Key bindings are the compositor's in a solo
  as anywhere: leave fullscreen, switch workspace, the launcher.

### Getting about

- `next_workspace` and `previous_workspace`, and a swipe on the
  touchpad, go along the row, solos included, in order.
- `workspace: N` goes to the Nth desktop, counting desktops only. If N
  is one past the last, it makes that desktop and goes there, so
  `Super+2` on a fresh session gives a second workspace. Further than
  that is refused, as going past the last is now.
- `move_to_workspace: N` likewise, and takes the window along.
- A rule's `workspace: N` puts the window on the Nth desktop, or on a new
  one at the end if there are fewer. The desktops between are not made:
  empty and not shown, they would go again at once.
- Each screen still has its own row (ADR 17). A window dragged to
  another screen joins the desktop being shown there; if a solo is being
  shown there, the desktop it came from.

### What the IPC says

A workspace gains an identity that lasts as long as it does and is never
used again in a session, its place in the row, its kind, and, for a
solo, its window. `number` goes on meaning what a key would reach, and a
solo has none. Clients are told when a workspace comes or goes, as they
are told of everything else.

### What was built first

The desktops, their identities and the actions came first, then solos,
and the edges that answer to a push after that. Until those are built a
solo is left by the key, or by whatever the program has for it.

Some things were settled in building solos that the above leaves open:

- "The program's own dialogs" are the windows that say they belong to the
  fullscreen one (`xdg_toplevel.set_parent`). Going by the program's name
  was tried first and was wrong at once: a second terminal is the same
  program and is not a dialog. Programs written for X11 are not asked
  yet, so a dialog of theirs opens on the desktop.
- Any other window opens on the desktop the solo came from, and that is
  shown, whether the user asked for the window or not. The compositor
  cannot yet tell the two apart. Activation tokens can, and when they are
  heeded a window nobody asked for should open there unseen.
- A fullscreen window sent to a desktop or to another screen leaves
  fullscreen to go.
- Minimizing a fullscreen window shows the desktop it came from; its solo
  waits, and bringing the window back shows it again.
- A window on every workspace waits on the desktop last shown while a
  solo is shown, and is on the next desktop shown.
- Over IPC a solo has the number 0 and is named by its window's title.

### More than one window on a workspace of their own

Wanted next, and not designed yet: two, three or four windows sharing
such a workspace in a grid, side by side or one above another, resized
by dragging the line between them; a window dragged to an edge of the
screen making one; and the titlebar's button, under the pointer,
offering the arrangements. A solo is then the simplest arrangement of
several. This ADR is to be amended, or followed by another, when that is
designed.

When the number of desktops that are always there is made smaller,
nothing is moved: one that is no longer kept stays for as long as it has
a window on it or is being shown. And `Super+1` to `Super+9` go to
desktops by default, where it was 1 to 4.

## Alternatives

- **A row always ending in one empty workspace**, as GNOME has. No
  action to make one is needed. But there is always one more workspace
  than is in use, and "the last one" moves as windows open.
- **Fullscreen in place**, as now: the window covers its workspace and
  the others stay under it. Simpler. It leaves the user with windows
  they cannot reach without leaving fullscreen, and no room of its own
  for the thing they chose to see alone.
- **A setting to choose between the two.** Two behaviours to keep
  working, in the part of the model that everything else stands on. If
  solos prove wrong for films, which are made fullscreen and left again
  quickly, this is where it would be revisited.
- **Put a solo at the end of the row**, as macOS does. Then "previous"
  from a solo is not where the user came from.

## Consequences

- The default changes from four workspaces to one, and the titlebar's
  second button from maximize to fullscreen. Both are breaking changes
  to the defaults, to be marked as such when made.
- The model gains workspaces with identities, made and removed; today
  they are a count. Everything that addresses a workspace by number,
  keys, rules, IPC, the bar, is gone over.
- The compositor gains edges that answer to being pushed, and showing a
  component's surface over a fullscreen window when it decides to.
- The bar's row of workspaces shows what there is, a solo by its
  program's icon.
- Going to a solo and back is a switch of workspace each time, and with
  stage 4 a slide. For a film made fullscreen that is more movement than
  now.
- Nothing is remembered across sessions: workspaces that were made are
  not made again at login.
