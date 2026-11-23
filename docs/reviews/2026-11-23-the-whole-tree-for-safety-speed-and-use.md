# The whole tree, for safety, speed and use

A review of 23 November 2026, of every crate, the guide and the
documents, for security, performance, architecture, reliability, the
tests, the documents and what omorfos is like to use, the last beside
macOS, Windows, GNOME and KDE. Each finding is written so that it can be
taken up on its own: where it is, how it was shown, what to change, and
the test to write first.

## What was asked, and of whom

Eight read-only reviewer agents read the tree at `7f27ec2`, each told
the open findings of the earlier reviews in this folder and the
roadmap's open items, so as not to find them again. Nothing was built or
run by them; every finding came **by reading**. The agent that started
them opened the code each finding names, and has since built and tested
each fix, so where a finding says **shown**, a test now fails without
its fix. Line numbers are those of `a266d9b`, where they had not moved.

Two findings were not so, and are under "Dropped", with why. Ranks are
the review skill's: **high** stops or ends the session, or gives away
what is private; **medium** costs without bound, or breaks a promise;
**low**, the rest. None is high.

## Findings

### 1. A lock screen started after a file has gone wrong uses the defaults, where the rest of the session uses what it kept

**Medium. Shown, by the test below. Put right in #95.**

`omorfos-lock` opened the configuration on disk as it was
(`crates/omorfos-lock/src/lib.rs:145`, `screen.rs:150`). The compositor,
and every other part started again (ADR 28), starts from the copy it
keeps of each file that loaded, in `$XDG_RUNTIME_DIR/omorfos/last-good/`
(ADR 3, ADR 60). With `config.yaml` broken, a lock screen started then
asked by every way and offered to sleep and shut down, as the defaults
do, and looked as the defaults have it, whatever the user had set.

What to change: open the configuration with `starting_from(kept)`, both
for what the lock screen offers and for how it looks, as the
notifications' `Client::new` takes the configuration and the kept copy
as one pair.

The test: `config.yaml` broken, motion reduced in the kept copy, and a
theme whose question moves: after a wrong password the field keeps
still. Without the kept copy it shakes.

### 2. A critical notification can be the one to go when there are too many

**Medium. Shown, by the test below. Put right in #100.**

When more than eight of programs' notifications are shown, the oldest
goes (`crates/omorfos-notifications/src/note.rs:245`), whether its
program marked it critical or not. The guide says one marked critical
"stays until it is clicked".

What to change: the oldest that is not critical goes, while there is
one; with every one critical, the oldest of those, so that there are
never more than eight. Its program is told, as before, by
`NotificationClosed`.

The test: one critical notification, then eight more: the one that goes
is the oldest of the others.

### 3. Each refresh looks every window up in two lists, so a quiet turn costs as the square of the windows

**Medium. By reading; to be shown by the benchmark below.**

For each window shown, `refresh` looks for its frame among all the
frames (`crates/omorfos/src/compositor.rs:2695`) and for it among the
last scene (`:2700`), and what is being captured looks each window up
again (`:2787`). Each is a walk of a list as long as the windows, so a
turn in which only the pointer moves does work as the square of the
windows before anything is drawn. Not measured: how many windows it
takes for this to be seen is not known, and the next item, the frame
loop, may cost more.

What to change: the frames and the last scene found by `WindowId`,
through a map made again each refresh from storage that is kept, or a
scene that carries what each window needs.

The test to write first: a benchmark of a quiet refresh with 1, 8, 32,
128 and 256 windows, which should grow as the windows do.

### 4. What is captured is worked out, and written, rectangle by rectangle

**Medium. By reading; to be shown by the benchmark below.**

`wanted` walks every shot and every taking, and keeps them apart with
`Vec::contains` (`crates/omorfos/src/capture.rs:555`); `due` walks
every shot for each taking (`:580`); and `hand` turns and writes each
damaged rectangle by itself, emptying its scratch space for each
(`:642`). Many programs capturing, or a damage of many small rectangles,
converts and writes the same pixels again and again, though capture was
made to send only what changed.

What to change: what is captured found by session and screen; scratch
space kept; and damage joined into the rectangle round it where many
small ones would cost more.

The test to write first: a benchmark with 1, 4 and 16 sessions and 1,
16, 256 and 4096 damaged rectangles, counting what is allocated, turned
and written.

### 5. The tests' Wayland client took a connection it could not read for one with nothing to read

**Low. Shown, by the tests below. Put right in #98.**

`Client::receive` threw away what `ReadEventsGuard::read` said had gone
wrong (`crates/omorfos/tests/integration/common.rs:1152`, and `:1373` in
`error`). A compositor gone looked like one with nothing to say, so a
test waiting on it went on until it ran out of time and said only that
"the expected state was never reached".

What to change: `receive` returns the error, and every test that waits
says so at once; `error` tells a protocol error, which it is looking
for, from a connection that cannot be read for another reason, which
fails the test.

The tests: a client whose compositor has gone is told it cannot read;
and `error` does not take that for no error.

### 6. The shell's pictures are handed to the compositor without checking that their size and bytes agree

**Low. Shown, by the test below. Put right in #97.**

`omorfos_shell::pixels` takes a width, a height and bytes from its
caller, and makes the length of the pool from the bytes and the stride
from the width, turning each to Wayland's signed numbers with
`unwrap_or(i32::MAX)` (`crates/omorfos-shell/src/lib.rs:73`). Bytes
short of the size make a pool the compositor refuses, and it sends the
program away. Every caller today passes a canvas's own size and bytes,
so this is a promise the function does not keep, not a fault seen.

What to change: refuse an empty size, one too large for Wayland, and
bytes not exactly four for each pixel, before any memory is made.

The test: a picture a byte short is refused, and the program is not
sent away, as it was; one that is right is drawn on a window.

### 7. The relayed clipboard test's last assertion holds whatever the keeper does

**Low. Shown by reading the assertion. Put right in #99.**

`what_was_copied_is_there_again_from_the_keeper_kept_through_a_restart`
ends with `!keeper.is_finished() || keeper.join().is_ok()`
(`crates/omorfos/tests/integration/relayed.rs:1718`). The relay and the
compositor's holder are still there when it is asked, so the keeper is
always still running, and a keeper that failed, or never ended, passed.

What to change: let the session go, wait a while for the keeper to say
why it ended, and assert it was the session ending.

### 8. Three documents said what is not so

**Low. Shown by reading. Put right in #96.**

- `docs/DESIGN.md:348` lists `appearance.high_contrast` among the
  settings there are; there is no such setting, and `ACCESSIBILITY.md`
  says high contrast is yet to come.
- `docs/guide/screens.md:60`, ADR 103 and the roadmap (`:537`) say
  `wallpaper.yaml` names screens; the wallpaper's screens are
  `wallpaper.screens` in `config.yaml`.
- `docs/guide/screens.md` did not say in which mode a kept way of
  standing comes back at the next login: the one `config.yaml` gives
  each screen, or else what the screen says suits it best, as
  `way_of` in `standing.rs` takes it.

## Dropped

- **That a process named `gpg-agent` may have the pinentry ask in the
  sealed prompt.** Ranked high by its reviewer. `omorfos-ask` knows the
  pinentry's parent by the name the kernel keeps of it
  (`crates/omorfos-ask/src/door.rs:116`), which any process may give
  itself. That is so, and [ADR 58](../adr/0058-passphrases-asked-in-our-words.md)
  says so, and why it is not the boundary: a program of the user's can
  start a real `gpg-agent` with a home of its own and a key made to look
  like the user's, and ask through that. The fix proposed, reading the
  parent's program from `/proc/<pid>/exe`, cannot be made:
  `gpg-agent` makes itself undumpable, and its `exe` is then not to be
  read by the user's own programs (tried: "Permission denied", with
  GnuPG 2.4.9). Asking the user's own agent for its process (`GETINFO
  pid`, on the socket `gpgconf` names) would turn away the first kind
  of program but not the second, and every agent of another home with
  it; that is a decision for ADR 58, not a fault.
- **That the notifications say they show actions, and show only the
  click.** Ranked medium by its reviewer: `GetCapabilities` says
  `actions` (`crates/omorfos-notifications/src/bus.rs:42`), and only
  the `default` action, the click, is kept. No longer saying `actions`
  was tried, and is worse: Firefox then sends no notification to the
  desktop that can be clicked, and shows its own in the browser
  (`toolkit/system/gnome/nsAlertsIconListener.cpp`), and Electron's
  programs give theirs no click at all, so clicking a browser's
  notification would no longer bring up its tab, as the guide says it
  does. Buttons for a program's actions are already on the roadmap,
  with the notification centre (stage 5), and `actions` stays said.

## What it could be, beside the others

Not faults, but what would most make omorfos more useful than the
desktops it is compared with, without putting policy in the compositor.
Each is for its own ADR before it is begun.

1. **Settings for screens that can be taken back.** A change of mode,
   scale or arrangement tried with a count down, and put back if it is
   not kept, as the display key already does; each screen's name, scale,
   mode, turn and mirror, and what is in effect, shown beside the file.
2. **One place for what is being shared.** Screen, window, microphone,
   camera and clipboard, each with which program, to where, since when,
   and a way to stop it, in the panel. The compositor knows most of it.
3. **A notification centre with programs' actions.** What has been
   notified, grouped, reached from the keyboard, put off for later, and
   kept by program; omorfos's own notices kept apart from programs'
   room.
4. **One tree of what is on the screen, for keys and for screen
   readers.** AccessKit for the lock screen, the sealed prompt, the
   launcher, the notifications, the picker and the panel, so that what
   is reached by Tab is what a screen reader reads.
5. **Touch and pens as themselves.** Touch focus and cancelling, input
   turned with its screen, and tablets; which edge gestures are the
   compositor's and which the programs'.
6. **Keys that can be found.** Super as Ctrl kept, with the usual way
   offered beside it, and a list of what each key does and what is in
   the way of what.
7. **Windows back where they were.** The relay's keeping extended to
   each window's screen, workspace, size and privacy, after a screen is
   plugged, a restart and a login, and a view of what could not be put
   back.
8. **Agreed protocols first.** Dialogs that belong to a window, the
   bell, toplevel tags, session management, workspaces a panel can
   show, and tablets, before anything of our own (ADR 99).

## Looked at and found sound

- `omorfos-core` knows nothing of I/O, the clock or Smithay, and depends
  on no other crate of the workspace.
- The configuration is checked as it is read, and the copy of what last
  loaded is kept by the compositor and started from by every part but
  the lock screen (finding 1).
- Who may do what by their role, what is asked before a screen is
  captured, the signs that something is, and how a screenshot's file is
  made, are as the security model says.
- The relay bounds what is added over all of a connection's objects,
  and stops being used after it has failed to start often enough
  (`Policy::relay_ended`); a loop of starting it again, suspected, is
  not there.
- The open findings of the earlier reviews were not found again.

## Not looked at

- How Smithay, zbus, PipeWire and the kernel behave, beyond what the
  code names; and anything on a real card: hardware drawing, touch
  screens, pens, HDR and colour, and screens plugged in and out.
- The portal's streams and their recovery on a card, which have open
  findings of their own.
- The other desktops themselves: what is said of them is from the
  survey in the repository, and from knowing them.
- Performance by measurement: findings 3 and 4 are by reading.

## Seen in passing

- `portal::it_ends_when_the_bus_does_and_needs_no_configuration_it_can_watch`
  failed twice in full runs of the coverage gate on this machine, on two
  branches, waiting for the portal to end after the bus has gone, and
  not in 45 runs of it under load. Alone it takes 0.2 seconds. Why it
  sometimes waits more than five is not known.
