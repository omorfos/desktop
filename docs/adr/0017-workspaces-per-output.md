# 17. Workspaces per output

- Status: accepted
- Date: 2026-10-09
- Amends: [4. Domain model](0004-domain-model.md),
  [15. Several outputs](0015-several-outputs.md)

## Context

With several screens, one workspace was shown across all of them, as in
Fluxbox: switching workspace changed every screen at once. With a laptop
and a monitor that is rarely what is wanted. The work on one screen should
stay put while the other is flipped through.

There are two usual ways to arrange this. One pool of workspaces can be
shared, each screen showing one of them; or each screen can have a set of
its own. The second was chosen: workspace 2 on the laptop and workspace 2
on the monitor are different places, and nothing on one screen is ever
pulled to the other by switching.

## Decision

**Each output has its own set of workspaces and shows one of them.** Every
set has the same number, with the same names, from the one `workspaces`
list in the configuration.

**A frame belongs to an output's set, and stays in the one space.** Frames
keep the positions they had: nothing is stored relative to an output. What
changes is which frames are shown: those on the workspace their own output
is showing. If an output moves, because another changed size or scale, its
frames move with it.

**A frame goes where most of it is.** Dragged until most of it is on
another output, it joins the workspace that output is showing, so it never
vanishes under the pointer. It can also be sent with the
`move_to_next_output` action.

**One output is current.** Its focused frame has the keyboard, and focus
cycles among its frames. It becomes current when a window opens on it, one
of its windows is clicked or activated, a frame is moved onto it, or its
workspace is switched.

**Switching workspace applies to the output the pointer is on,** which
then becomes current. The pointer says where the user is looking; the
keyboard may still be in a window on the other screen, and moving the
pointer over does not take it away under a click-to-focus model.
`move_to_workspace` moves the focused window among the workspaces of its
own output.

**Outputs are told apart by their order.** The model has no names for
them: the adapter gives the list in the order outputs appeared, and says
which one has gone before giving a shorter list.

**When an output goes, its frames move to the first that remains, each to
the workspace of the same number.** They do not go back if it returns.
Hiding them until it does would leave windows that cannot be reached.

**The IPC state names the output of each workspace and window.**
Workspaces are listed for each output in turn, so numbers repeat, and
several can be active at once. The protocol version stays at 1: nothing
has been released against it.

## Consequences

- A client that keyed workspaces by number alone has to key them by output
  and number.
- A rule's `workspace: N` means workspace N of whichever output the window
  opens on. A rule cannot name an output.
- There is no action to move the keyboard to the other screen without
  moving the pointer, a window or a workspace there.
- With one screen nothing changes, and the model's earlier tests pass as
  they were, but for the one that described frames gathering on the first
  output's current workspace when a screen went.
- Windows do not return to a monitor that is plugged back in. Remembering
  where they were, by the output's name, would need the model to know
  names.
