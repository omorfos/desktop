# How functional it is, and how solid that makes it

A review of 22 November 2026, asked for by the maintainer: how far the
code is written as functional code is, as values not changed in place,
decisions made by pure functions and effects kept at the edge, data
shaped so that what cannot happen cannot be said, and how solid it is,
or is not, because of that. What is cheap and safe to put right was put
right the same day, each in a pull request of its own; the rest is on
the roadmap, in stage 8.

## How it was done

Three reviewers read the tree at `origin/main` of 22 November at once:
the inner crates (`omorfos-core`, `omorfos-config`, `omorfos-ui`,
`omorfos-sums`, `omorfos-i18n`, `omorfos-shell`); the compositor; and
the relay, the holder, `omorfos-ipc` and the components. The machine
they ran on could not build, so every finding is by reading or to be
shown; each that was put right was then built and tested by CI.

## The verdict

**It is a functional core inside an imperative shell, written as plain
Rust, and the core is where it is solid.**

- **The domain is pure.** `omorfos-core` has no dependency, no clock, no
  file and no interior mutability; nor have `omorfos-config`,
  `omorfos-ui`, `omorfos-sums`, `omorfos-i18n` or `omorfos-shell`: not a
  `Cell`, `RefCell`, `Rc`, `Mutex`, `static` or `OnceLock` among them
  outside tests. Time is passed in nearly everywhere it is needed
  (`Motion::begin(now)`, `Repeat::due(now)`, `Config::loaded(.., now)`,
  `Store::at(now)`), configuration is checked as it is read (eighteen
  types read through `try_from`), and `Tiling` is a `Copy` value changed
  by returning a new one.
- **The components share one good shape.** Each of the eight that draw
  keeps its Wayland side in a `Shell` and its decisions in a type with no
  input or output, given the time and returning what it wants done as a
  value: `Picker`, `Stack`, `Card`, `Bar`, `Slides`, and the holder's
  `Policy::ended(failed, now, other) -> Next`. State is an enum in many
  places: the lock screen's `View` and `Held`, the relay's `Phase` and
  `Renewal`, the screenshot's `Stage`.
- **The mutation is in two places, and they are large.** `Desktop`, the
  domain's aggregate, holds 108 of the core's 121 methods taking
  `&mut self`, and finds what it holds by position: 130 indexings in it
  that would panic if an invariant slipped, kept by `repair`, `settle`
  and a random-operations test, not by types. `Compositor` has 83 fields,
  about 145 pieces of state with what they hold, and some 320 methods
  taking `&mut self`; `compositor.rs`, 6,555 lines, has no tests of its
  own, so its policy is tested through whole sessions. Its idiom of
  taking a field out to get past the borrow checker (28 times, and the
  IPC server, the switcher and the overview whole) made one fault below.
- **Where it is fragile, it is where a state is spread over fields that
  must agree.** Every fault this review found is of that kind: one flag
  saying two things, a position kept across a renumbering, a time read
  twice, a mark set before what it marks, a server taken out while it is
  asked about.

Counted by script over each crate's `src`, tests in it included, as a
rough picture:

| Crate | Lines | `&mut self` | `Cell` | `RefCell`, `Mutex` | `.clone()` |
|---|---:|---:|---:|---:|---:|
| `omorfos` | 31,988 | 537 | 3 | 15 | 213 |
| `omorfos-config` | 15,039 | 26 | 0 | 0 | 32 |
| `omorfos-core` | 11,259 | 121 | 0 | 0 | 24 |
| `omorfos-relay` | 10,734 | 157 | 2 | 0 | 27 |
| `xdg-desktop-portal-omorfos` | 6,281 | 49 | 8 | 0 | 62 |
| `omorfos-ui` | 5,528 | 48 | 0 | 0 | 17 |
| the eight components that draw | 21,679 | 221 | 0 | 4 | 147 |

The `Cell`s and `RefCell`s left are where a library asks for them
(PipeWire's callbacks, calloop's, Smithay's user data, wayland-client's)
or are caches filled while drawing through `&self`, but for one, which
carried a decision of security and is finding 2.

## Put right the same day

### 1. Every turn of the real event loop was told the time from before it slept

**Medium.** `Session::run` passed `Instant::now()` as an argument, read
before the event loop waited, so each refresh was told a moment before
its deadline: two turns for every timer and frame, and after a long
idle with `idle.screen_off` set, the first key turned the screens on and
the next turn, their time already past, off again. `Session::dispatch_now`
reads the clock after the wait; the hardware backend's wait for its
terminal uses it too.

### 2. A screen was counted as showing nothing of the session before its locked frame was drawn

**Medium,** ext-session-lock's promise. `Head::blanked` was set before
the frame was drawn, so a frame that failed, which the hardware backend
goes on after on purpose, still counted, and the locker could be told
the session was locked while the screen showed it. It is now set by
whether the frame drawn failed. Whether a frame queued is a frame shown
is on the roadmap.

### 3. Windows tiled side by side held the compositor's update back as a film would

**Low; perhaps the cause of the update not taken up for eight minutes
that the roadmap records, which is not shown.** The domain marks a
window filling a part of a shared workspace fullscreen too; three
readers of the flag told the two apart, and the one that asks whether
the session is in use did not. It does now. That one flag says both is
the first item on the roadmap.

### 4. Requests on the IPC socket were answered with the IPC server taken out of the compositor

**Low.** Everything started through the socket, as the launcher starts
every program, was not told `OMORFOS_SOCKET`; what peers keep the
session from read as nothing meanwhile. Requests are now read with the
server out and answered once it is back, in the order asked.

### 5. The slide after a key showed the wrong workspace beside, by a position the switch had renumbered

**Low.** `Set.beside` kept a position across the switch that may take
the empty workspace left away. It keeps a `WorkspaceId` now, as
`Set.desk` does, and nothing is drawn beside for one that has gone.

### 6. The bar read the clock twice for one picture

**Low.** At midnight it could show one minute with another day's date.
`clock::now` says the time and the day of the one moment it reads.

### 7. The relay told a window made under a used number what the one before was told

**Low.** Its map of what each window was told, the serials offered and
the size, was emptied by neither list of what ends or is made. It is now
emptied of a number as an object is made under it.

### 8. The relay kept whether its socket is paused twice

**Low.** An `Option` of until when, and a `bool` its one writer always set
to whether the `Option` was there. The `bool` goes.

## On the roadmap

In the order they are worth taking, each with its size.

1. **One flag, `fullscreen`, says both "fullscreen" and "tiled".**
   Medium, larger. `Group.fullscreen` and `Part::Tile` in the
   workspace's `Tiling` record one fact twice, every reader decodes the
   pair, and a tiled window that asks to be fullscreen cannot be: ADR 39
   promises it the whole workspace, and `set_fullscreen(id, true)` on it
   does nothing. An enum on `Group` and `Frame`, as `Fills { Own, Whole,
   Part }`, with fullscreen while in a part a state that can be.
   `crates/omorfos-core/src/desktop.rs:300`, `1150`, `2150-2158`.
2. **Each component keeps its Wayland globals as separate `Option`s.**
   Medium, larger. 54 `Option` fields in eight `Shell`s, combined by
   hand in 78 `zip`s, and a component under a compositor that lacks one
   draws nothing, for good, and says nothing. One
   `Globals::gather(&Connection) -> Result<Globals, Missing>` in
   `omorfos-shell`, held by each `Shell` in place of the `Option`s, goes
   with the one `Sheet` of finding 16 of 16 November.
3. **Time is still read inside decisions.** Low, larger.
   `Compositor::deadline` and `refresh` read `SystemTime::now()`
   (`compositor.rs:1263`, `1322`), night light `Zoned::now()`
   (`night.rs:118`), `Awake::asked` the wall clock (`lock.rs:119`), and
   `Awake.until` and `Animation.epoch` are `Option<Instant>`s that wait
   for a later `now`. One `Now { instant, wall }` given to `refresh` and
   `deadline` and to what starts a timer, so night light and staying
   awake can be tested by the test's clock. Mechanical, but every test
   call changes.
4. **The X server's life is six `Option`s.** Low, larger. `server`,
   `told`, `stopped`, `again`, `held`, `wm` (`x11.rs:392-416`), whose
   valid combinations are only implied, and which needed two fixes in
   November. An enum, `Off`, `Waiting`, `Starting`, `Running`,
   `Draining`, and `manage_x11` a function from one to the next.
5. **The relay reads the clock inside `step`.** Low, small.
   `relay/lib.rs:206`, `270`, `325`, `342`, `391`: its grace and its
   pause can only be tested by waiting in real time. `step` given a
   function of the time, as `doubt` runs after the wait.
6. **The relay's maps of what is told of each object are emptied by
   hand-written lists that differ.** Low, moderate. `Keeping` has 43
   maps by object number (`keep.rs:345-483`) and two lists that empty
   them, neither within the other; finding 7 above was one left out. One
   `forget(number)` that both call, with a test of a lock surface made
   again under a number.
7. **The clipboard keeper decides on a second thread from state the main
   thread may not have read.** Low, moderate. It can clear another
   program's fresh copy at the moment its own falls due
   (`omorfos-clipboard/src/lib.rs:262-288`). The second thread only
   wakes the main loop, which decides; the `Mutex` and the atomic go.
8. **States spread over pairs of fields that must agree.** Low, each
   small. The relay's `compositor`, `readying`, `going` and `listed`, one
   `Option<Compositor>`; the portal's `pending` and `picking`, one
   `Option<Busy>`; the screenshot's `wanted` and `found`; the panel's
   `month` and `on_month`; the portal stream's `layout` and `agreed`, where
   `None` says both "not agreed" and "shared memory"; the holder's
   `notifier` then `watching`. None is wrong today; each is kept in step
   by convention only.
9. **A screen locked once its frame is shown, not queued.** Low, larger.
   After finding 2, `blanked` follows a frame drawn; ext-session-lock
   asks for one shown, which is the backend's presentation, a decision
   across both backends.
10. **Positions that would panic.** Low, larger. `clippy::indexing_slicing`
    is not on, though panics are linted: 130 indexings in `desktop.rs`,
    `self.heads[..]` and `mirrors[..]` in the compositor. Turned on crate
    by crate, from `omorfos-core` out, each index made a `get` the type
    shapes away, or a typed handle.
11. **`Config.scheme` and `theme` are public fields worked out from
    `general.appearance` and the time** (`config.rs:35-52`), so a holder
    can make one that disagrees with itself. Private, with what works
    them out the only way in.

Not worth doing, as the review saw it: making `Desktop` or `Compositor`
immutable values. Both are the edge where a session's state lives, and
Rust's `&mut self` with the decisions drawn out into pure functions, as
`versions::InUse::own(since, wait, now)` already is, is the way on;
each item above draws one more out.

## Looked at and found sound

`Store` over `Config::loaded`; `Field`'s undo kept as values; `Tiling`;
the components' domain types; the relay's `Rc<OwnedFd>`, shared on one
thread; `omorfos-ipc`'s `handing`, its one counter shared between
threads; the compositor's `changes` count against `as_published`;
`menu_chrome` as a cache of `menu`; the window, desktop and space kept
in step in `remove_window`.
