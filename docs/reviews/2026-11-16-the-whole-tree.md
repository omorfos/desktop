# The whole tree, in every dimension

A review of 16 November 2026, of every crate, the guide, the documents
and the gates, in each of the eight dimensions of the review skill
(`.claude/skills/review/`). Each finding is written so that it can be
taken up on its own: where it is, how it was shown, what to change, and
the test to write first. The roadmap lists them in the order to take
them.

## What was asked, and of whom

One reviewer agent for each dimension read the whole tree at once:
security, architecture, components, performance, reliability,
experience, tests and docs. The security reviewer left out what
[the review of the same day](2026-11-16-what-the-first-review-did-not-look-at.md)
read: the relay's dealings with the compositor, the portal and the
services on D-Bus, buffers of the graphics card, the hardware backend,
the helpers that check a secret, and the session's services. Each was
told what was already known, the open findings of
[the review of 15 November](2026-11-15-what-a-client-can-cost.md) and the
roadmap's open items, and found nothing again that they cover.

Each finding was then checked against the code by the agent that started
them, and where it says **shown by a test**, the test was run with the
fault as it is. What two reviewers found from two sides is one finding
here. Ranks are the skill's: **high** stops or ends the session, or gives
away what is private; **medium** costs without bound or breaks a
promise; **low**, the rest. Where that differs from the rank a reviewer
gave, it is said.

Dropped: that `AGENTS.md` says the `commit` tier runs every test. It was
changed with ADR 94 and says what the tier does.

## Findings

### 1. Any program can take the keyboard from the window being typed in, with a pop-up

**High. Shown by a test.**

`XdgShellHandler::grab` in `crates/omorfos/src/handlers.rs:451` ignores
the seat and the serial and keeps the pop-up as grabbing, and
`sync_keyboard_focus` (`crates/omorfos/src/compositor.rs:3051`) gives the
last grabbing pop-up the keyboard ahead of the focused window; only the
lock, the sealed prompt and an overlay that demands the keyboard come
before it. Smithay passes the grab on unchecked. Nothing asks whether the
serial was one the compositor gave, or whether the pop-up's window has
the focus.

`xdg_shell` is offered to every program, one in a sandbox too. A program
in the background makes a pop-up of a pixel on its own window, grabs with
serial 0, and is given every key the user types into the window they
are in: a password into a browser, `sudo` into a terminal. A click
elsewhere sends it `popup_done`, and it grabs again at once. ADR 37
promises that focus is not taken and that keys typed into a window go
to that window.

Shown by a test written for the purpose and taken out again, in
`popups.rs`: one program's window was focused, another made a pop-up of
its own window and grabbed with serial 0, and a key pressed went to the
second (`stolen=true kept=false`).

**To change**: in `grab`, honour a grab only if its serial is a press
delivered to that client and the pop-up's root is the focused window or
surface of a layer; `Compositor::may_move_focus`
(`crates/omorfos/src/asked.rs:110`) already asks the same of activation
(ADR 36). Otherwise send `popup_done` and keep nothing.

**Test first**, in `crates/omorfos/tests/integration/popups.rs`: as
above, and the second program is sent `PopupDone` and no key, and the
first is given the key. `an_open_menu_has_the_keyboard_and_hands_it_back`
keeps the honest case.

### 2. Whenever the compositor sends one program away, every other stops for half a second

**High. Shown by a test.**

When a program's connection to the compositor closes and the compositor
does not, the relay's `step` asks `compositor_gone(GRACE, log)`
(`crates/omorfos-relay/src/lib.rs:185`), which waits in `poll` on the
compositor's connection alone for up to `GRACE`, 500 ms, to learn
whether the compositor is going. Nothing else is read or written
meanwhile: no other program's requests, no input or frames to them. The
compositor sends a program away for any protocol error, so any program,
one in a sandbox too, can connect, make an error and connect again, and
freeze every other program's windows and input for as long as it goes
on. A program written by the reviewer that drives the relay as
`tests/relay.rs` does saw each such step take 500.6 ms, three times of
three; it was run again in checking, with the same. The relay's test of
it, `a_program_the_compositor_lets_go_while_it_goes_on_is_let_go`, waits
a second, so passes.

**To change**: take the wait out of `step`. On a program lost with the
compositor there and not going, keep the link marked as undecided until
`GRACE` from now, put that time into `wait`'s timeout, and decide at a
later step, as `lose_compositor` or `remove` would.

**Test first**, in `tests/relay.rs` with `Held`: two programs, one's
connection to the compositor closed, and one `held.step()` returns within
a few `STEP`s, with what the other wrote in that step passed on.

### 3. A window's name for its program is opened as a file, on the compositor's thread

**Medium (the reviewer: medium to high). The kernel's part shown by a
script; the rest by reading.**

`WindowIcons::application` (`crates/omorfos/src/icons.rs:189`) looks the
window's `app_id` up as an icon as it came, and `omorfos-icons` takes an
absolute name as a file (`locate`, `is_file()`), then opens it with a
plain `File::open` (`load`). A program sets its `app_id` to a file of its
own and holds a write lease on it: the next time the switcher is opened,
or windows are offered in an empty tile, the compositor waits in `open`
for the lease's time, 45 s here, for each such window. A script in the
reviewer's scratchpad showed the open wait under a lease, and fail at
once with `O_NONBLOCK`. Whether a sandbox's seccomp lets a lease be
taken is to be shown. Outside a sandbox, a file on a FUSE mount of the
program's own waits in `is_file()` itself; and any program can have any
picture the user can read shown as its window's icon. Programs written
for X11 reach it through their class. The notifications daemon has the
same through `image-path` and `app_icon` from any peer on the bus, and
the same between `is_file()` and `open` for a file swapped for a FIFO.

**To change**: in `application`, look nothing up as an icon that has a
`/` in it, as `set_name` already does not; and in `omorfos-icons`'
`load`, open with `O_NONBLOCK` and ask the open file whether it is a
plain file, which also closes the gap between the look and the open.

**Test first**: in `omorfos-icons`, a file held under a write lease by a
thread that ignores the signal, asked for by its absolute path, gives
`None` at once; and in the integration tests, a window whose `app_id` is
the path of a PNG is not shown with that picture in the switcher.

### 4. A program written for X11 can end every program written for Wayland that is given the keyboard

**Medium. Shown by reading.**

What a program written for X11 offers on the clipboard is passed to
Wayland as it came (`XwmHandler::new_selection`,
`crates/omorfos/src/x11.rs:1062-1077`): Smithay keeps up to 4096 kinds,
each as long as an X atom may be. The filter of finding 6 of 15 November
(`for_x11`) is for the other way only. A kind of more than 4 KB makes a
`wl_data_offer.offer` the relay refuses as longer than any message
(`wire::LONGEST`), and it ends the link: the keeper of the clipboard at
once, and every program written for Wayland as it is given the keyboard,
while the selection stands. Many shorter kinds take a program past
ADR 45's megabyte instead. ADR 37 says programs written for X11 cannot
reach programs written for Wayland.

**To change**: the same filter both ways: at most 32 kinds, each of
visible ASCII and no longer than 255 bytes, one function for both.

**Test first**, in `x11.rs`: a program written for X11 offers a kind of
5000 bytes, and a Wayland program then focused stays connected and is
offered nothing longer than 255 bytes.

### 5. On a graphics card, a session with nothing changing wakes at the screen's rate for good

**Medium (the reviewer: high; it costs without bound and breaks a
promise, but stops nothing). Shown by reading; to be shown on hardware.**

In `turn` (`crates/omorfos/src/backend/hardware.rs:785-802`), a screen
that drew nothing has a timer set a frame away, to tell programs a frame
has gone. The timer wakes the loop, `turn` draws again, finds nothing,
and sets another: for ever, on every screen, until they go dark. Nothing
asks whether a surface waits for a frame. Each wake costs a whole
`refresh` and its system calls (findings 9 and 10 among them). ADR 9
says an idle session makes no system calls, ADR 10 that an idle screen
draws nothing; the `quiet` tests cannot see it, as the timer is not in
`deadline()`. The nested backend draws continuously by design.

**To change**: decide it in the compositor, not the backend, which no
test runs (ADR 2): `Compositor::awaits_frame(&Output)`, true only if a
surface on that screen has frame callbacks waiting, and the tick set
only then.

**Test first**: `awaits_frame` is false for a quiet session once drawn,
true after a program asks for a frame and commits with nothing changed,
and false again after the frames are sent. On hardware,
`perf stat -e syscalls:sys_enter_epoll_wait -p $(pidof omorfos) -- sleep 10`
with nothing touched should count about none, not some 600.

### 6. A thread that cannot be started ends the program, the compositor among them

**Medium. Shown by reading.**

`std::thread::spawn` panics where the system will not start a thread,
as under the user's `RLIMIT_NPROC`; `thread::Builder::spawn` returns the
error, as `handing.rs` uses. It is used in fifteen places outside the
tests: the compositor's `x11.rs:687`, which sets X's resources at each
change of scale, and so ends the session; `omorfos-clipboard`'s `forget`;
`omorfos-lock`'s `ways.rs` and `power.rs`; `omorfos-ask`'s `helper.rs`
and `agent.rs`; `omorfos-notifications`' `bus.rs`; `omorfos-session`'s
`watch.rs`; and the portal's `bus.rs` and `lib.rs`. AGENTS.md: no panics
in code that is not a test. The compositor's thread also sets
`RESOURCE_MANAGER` read, changed and written without a lock, so two in
quick succession can leave the older, and waits on the X server without
bound.

**To change**: `std::thread::spawn` in `disallowed-methods` in
`clippy.toml`, the tests keeping theirs by a local `#[expect]`, and each
place made `Builder::new().spawn(..)` with its error taken where the
others are. One shared starter, in `omorfos-ipc`, would go with finding
17's queue.

**Test first**: the lint is the guard; a test of behaviour needs a child
under `setpriv --rlimit-nproc` at its count of threads, which says the
failure rather than abort.

### 7. A configuration broken when the compositor starts again is put aside for the defaults, not the last that loaded

**Medium. Shown by reading.**

`Store::open` loads with `Config::default()` to fall back on
(`crates/omorfos-config/src/store.rs:52`); the last good value of each
file lives in the process alone. A reload falls back to the last that
loaded, but a compositor started again, after a crash or to take up its
new version, which waits for five idle seconds, the moment someone has
saved and looked away, comes up with the defaults for a file with a
mistake: the user's keys gone, the keyboard's layouts the default's, so
the holder's index of a layout points elsewhere and kept programs are
typed at in another. ADR 60 promises the last that loaded (ADR 3). Every
component ADR 28 starts again does the same.

**To change**, in `omorfos-config` alone: keep a copy of each file that
loaded under the runtime directory, as `$XDG_RUNTIME_DIR/omorfos/last-good/`,
and have `Store::open` fall back on it before the defaults, keeping the
diagnostic.

**Test first**, in `store.rs`: a good `keys.yaml` opened, then made
wrong, and a second `Store` on the same directory has the good bindings
and a diagnostic.

### 8. After one failed hold on sleep at waking, the lock is never waited for again

**Medium. Shown by reading.**

At waking, `omorfos-session`'s watcher takes logind's delay again:
`Do::Hold => { held = inhibit(&bus).ok(); None }`
(`crates/omorfos-session/src/watch.rs:219`). If that fails, as with a
logind busy at resume, the error is dropped and nothing is held, tried
again or said. At the next suspend logind does not wait, and the machine
sleeps before the lock is up and wakes unlocked. The comment that a busy
logind is tried again holds only at start.

**To change**: treat it as the first: end the watcher, for it to be
started again and hold again, or keep a time to try again by and say
each failure.

**Test first**, with `stand_in::Logind`: `Inhibit` fails once after the
first; sleep and wake; at the next sleep a delay is held, or the watcher
has ended.

### 9. In the light scheme, which row of a menu the keyboard is on cannot be seen

**Medium (the reviewer: high; it breaks a promise, and stops nothing).
Shown by reading, the contrast reckoned.**

The row the keyboard is on is drawn as hovered (`paint.rs:246`), and a
hovered row is `--surface-raised` (`default.css:172`): in the light
scheme white on a cream menu, 1.02:1, with the same text; in the dark,
1.38:1. `Super+Escape` and the arrows choose rows of the root menu,
which has Log out, Restart and Shut down. DESIGN.md says such a row is
the accent with `--on-accent`. The part picked in the tiling pictures is
marked the same way, but a caption names it.

**To change**: a state of its own for the row the keyboard is on, drawn
`menu-item:focus { background: var(--accent); color: var(--on-accent); }`,
`:hover` left for the pointer.

**Test first**: in `meets_contrast` (`crates/omorfos-config/src/theme.rs`),
the keyboard's row against the menu, 3:1, in both schemes.

### 10. Every turn builds what is told to IPC peers, and copies it, though none listens

**Medium. Shown by reading.**

`publish_ipc` (`crates/omorfos/src/ipc.rs:367`) runs at the end of every
`refresh` the server exists for, which is always: it builds the whole
`State`, every workspace and window with their strings and every
screen's modes, sorted and written out, then copies it whole before
asking whether anything changed or anyone waits. `handle_of` finds each
window's handle by a scan. Some 200 allocations a turn for ten windows
and two screens, on every turn the pointer moves, and on every wake of
finding 5.

**To change**: a count of changes, moved wherever the desktop, the
screens, events, problems, radios, the lock or wakefulness change; the
state built only when it moved and a peer follows; the copy only then.

**Test first**: in `quiet.rs`, the same turns with a peer that has
subscribed and been told the state once, under the same ceiling.

### 11. Under the relay, every turn looks every window up again, and every move of the pointer is sent

**Medium. Shown by reading.**

`tell_relay` (`crates/omorfos/src/kept.rs:122-168`) asks
`desktop.kept(id)` for every window before it asks which are kept by the
relay, each a search of the desktop with an allocation, and builds
`desk()`: on every turn of every session under the holder, nothing
having changed. The pointer's place is in what is told, so every turn it
moved writes to the relay, which wakes, reads and keeps it, to put the
pointer back after a restart.

**To change**: ask only for windows the relay keeps, only once the count
of finding 10 has moved; send the pointer only with another change, or
when stepping down.

**Test first**: the quiet turns in `relayed.rs`'s session with one
window, under the same ceiling; and moving the pointer sends the relay
nothing new.

### 12. The relay allocates for every message it passes

**Medium. Shown by reading.**

Every message each way is copied out (`link.rs:500`, `:550`), copied
again into what is to be written (`:111`), and drained from the front of
what was read, moving the rest. `Journal::request` builds a record of a
commit, with its names, only to drop it (`journal.rs`). Some twenty
allocations for each frame each program draws, on the relay's one thread.

**To change**: read in place and copy once into what is to be written;
decide the rule before a `Message` is made, and make none for a commit;
keep an offset into what was read, and move it once a read.

**Test first**: `allocation-counter` as a dev-dependency of the relay, as
the compositor has; a recorded attach, damage, frame and commit through a
`Link` a hundred times, and what is allocated does not grow with them.

### 13. Two things the user is told come only in English

**Medium. Shown by reading.**

- The screenshot picker's line of help, at the foot of every screen on
  `Shift+Print` (`crates/omorfos-screenshot/src/lib.rs:63`, `HINT`), is a
  constant; the program is not among those translated, and its doc
  comment says the top. The folder and the files' names are English
  too.
- "cannot start `{}`: {error}" (`crates/omorfos/src/compositor.rs:1974`)
  and the two failures of X11 (`x11.rs:553`, `:750`) are written in the
  code, with the system's own words. With no configuration,
  `Super+Return` starts `foot`, which the packages only suggest: the
  first key the welcome card teaches answers, on a new install, "cannot
  start `foot`: No such file or directory (os error 2)", with nothing
  said of what to do. ADR 79: what a translated program says is a
  message.

**To change**: a `screenshot` program in `omorfos-i18n`, its `hint` a
message; and `cannot-start` and the two of X11 as messages of the
compositor, the first saying how to bind another program.

**Test first**: the hint in Polish differs from the English; and a key
bound to a program that is not there, in a Polish session, says the
Polish message.

### 14. The manual pages and the schemas say that starting the compositor again closes the windows

**Medium. Shown by reading.**

The doc comment of `Action::RestartCompositor`
(`crates/omorfos-config/src/action.rs:188`) ends "with the windows
closed", and it is the words of `schemas/keys.schema.json`,
`menu.schema.json`, `man/omorfos-keys.5` and `man/omorfos-menu.5`. The
guide, ADR 60 and 61 and the architecture say the windows stay.

**To change**: the comment as the guide says it, then the schemas and
the manual pages made again (`OMORFOS_BLESS=1`).

### 15. A program can map any number of windows, if the count of those shown were lost, and no test would know

**Medium. Shown by reading.**

`windows::a_program_is_given_no_more_than_the_most_windows_at_once` opens
256 windows that are never given a buffer, so every one is waiting and
none is shown, and `windows_of`'s count of those shown is never reached
by more than an empty list: without it, the test passes, and a program
could map 256, open 256 more, and so on.

**Test first**, in the same test: some windows opened and shown, the rest
to the most waiting, and one more not configured.

### 16. Putting a picture on a scaled surface is written fifteen times

**Medium. Shown by reading.**

`picture`, `attach`, `set_destination`, `damage_buffer`, `commit`, the
buffer kept: the same few lines in the password box, the wallpaper
(twice), the launcher (twice), the panel, the lock screen, the picker
(three times), the shell's pop-overs, the notifications, the portal
(twice) and the welcome card (twice); each with its own face of surface,
viewport, fractional scale, buffer, scale and size; and the scale taken
as `Scale::new(f64::from(scale) / 120.0)` in eight, beside `Scale`'s own
`numerator()`. They have begun to differ: the lock and the password box
ask for a frame, the rest do not, and the password box drops its error
(finding 25). The roadmap's item on letting go of buffers as soon as
they are drawn has to be made in each.

**To change**: `Scale::from_numerator` in `omorfos-ui`; one `Sheet` in
`omorfos-shell` with `put(shm, &Canvas, handle)`, taking its events by
`delegate_dispatch!` as `popover::Popovers` does, and `Popover::put` made
a use of it.

**Test first**: `Scale::from_numerator(180) == Scale::new(1.5)`; and a
`Sheet` told 180 draws its canvas at one and a half times its size.

### 17. Smaller things written more than once

**Low. Shown by reading.**

- **The keyboard's layout** is read by hand in the welcome card
  (`client.rs:283`), the portal (`prompt.rs:428`) and the picker
  (`lib.rs:547`), copies of `omorfos_shell::typing::Typing::learn_layout`,
  each with its own most; the picker and the portal never take the
  modifiers, so the group stays the first. Hold a `Typing` in each.
- **A queue that wakes a `poll`**, a channel and a byte in a pipe, is
  written five times: `omorfos-ask`'s `lib.rs` and `agent.rs`,
  `omorfos-notifications`' `bus.rs`, the lock screen and the portal's
  `bus.rs`. One `omorfos_ipc::woken`, with its order (drained first, so
  that one told meanwhile wakes again) tested once.
- **`whole`** rounds in five places and floors in two, the public
  `omorfos_shell::whole` among them, and the lock screen rounds with the
  floor and a half. One `nearest` in `omorfos-core`, and the shell's
  renamed for what it does.
- **The welcome card's client** is the launcher's, backdrop and all; it
  belongs on the roadmap's item for one backdrop.
- **`Failure`** is declared thirteen times beside
  `omorfos_shell::ended::Failure`, and the left button's code eight.

### 18. A program can put words of its own in the compositor's notices by asking for every key

**Low to medium. Shown by reading.**

`new_inhibitor` (`crates/omorfos/src/input.rs:1375-1384`) names a program
that asks for every key by its `app_id`, up to 255 characters of its
choosing, new lines among them, and says it through `Refused::say`
directly: a new name for each request is told again without end, and
drawn as the compositor's own notice. The fix of 15 November's finding 7
went through `refuse`, beside it.

**To change**: say it through `Compositor::refuse` with an `Asked` of its
own: once for each connection, the program named by its process.

**Test first**: one connection asks for every key three times under
three names, one with a new line, and one line is told, with none.

### 19. A window written for X11 can say it is another program and be activated in its place

**Low. Shown by reading.**

`asked_for` is given `x11.pid()` (`crates/omorfos/src/x11.rs:864`), which
is `_NET_WM_PID`, written by the program: a window that names a program
just started takes its activation, and the real one's window opens
without. Take the process from the X server (`get_client_pid`).

### 20. The lock screen shows the time the machine went to sleep for up to a minute after it wakes

**Low. Shown by reading.**

The lock screen reads the clock again only at a monotonic deadline
(`crates/omorfos-lock/src/screen.rs:505-513`), which does not move while
asleep, nor when the clock or the time zone changes. Compare the clock's
text each turn, or wait on a `timerfd` of the real clock with
`CANCEL_ON_SET`, as `Watcher::wake_at` does.

### 21. The relay takes a bind at a version the compositor does not offer

**Low. Shown by reading.**

`objects::what` (`crates/omorfos-relay/src/objects.rs:223-230`) and
`keep::registry` bound a version by wayland-protocols' description, not
by the list (`omorfos_ipc::protocols::OFFERED`): `xdg_wm_base` at 7
passes where 6 is offered. Bound both by the list, in `known::global`.
Test: a bind of `xdg_wm_base` at 7 is refused.

### 22. Dragging a window's edge takes it below what its program says is its least

**Low. Shown by reading.**

A drag is bounded by the adapter's `MINIMUM_WINDOW`
(`crates/omorfos/src/input.rs:242`), not by the window's least, which the
domain holds and tiling uses; a window written for X11 is configured
below its minimum, and one for Wayland is grown back by its commit with
its frame for a moment not. Have the domain resize a frame by its edges,
bounded by its least. Test: a window of least 300 by 200, dragged to 100
by 100, is never configured below its least.

### 23. What the relay hands the compositor is mostly written only in the code, and a role's bit can be taken for the user's

**Low. Shown by reading.**

Seven of the eleven records between the relay and the compositor are not
in `docs/IPC.md`, though they outlive a compositor's version; the roles'
byte is laid out in the compositor's `roles.rs`, and its last bit,
`USERS`, in `compositor.rs:5204`; `Roles::ALL` is every bit, that one
among them. Lay the byte out in `omorfos_ipc::relay`, make `ALL` the
roles alone, and list the records. Test: no role's bit, `ALL`'s
included, is `USERS`'.

### 24. The architecture says no component reaches the domain, and four do

**Low. Shown by reading.**

`omorfos-icons` depends on `omorfos-core` for `Sign` (ADR 70), so the
launcher, the welcome card, the notifications and the portal reach the
domain, against `docs/ARCHITECTURE.md`. Say so in the architecture, or
move `Sign`; and, if it is to be held to, check it over `cargo metadata`.

### 25. The password box drops a drawing that failed

**Low. Shown by reading.**

`crates/omorfos-ask/src/prompt.rs:918-922` drops `show`'s error with a
comment that the connection keeps it, but it fails on making the
picture's memory, which the connection never sees; the box is then not
drawn until something else changes. Pass it up as the portal's `turn`
does.

### 26. The selector of tests is not tested before a push

**Low to medium. Shown by reading.**

`affected.test.sh` runs only in `commit`; a commit that changes only
`affected.sh` reaches no crate by its own rules, so `commits` checks its
spelling and links alone, and neither `rest` nor CI runs it. Run it in
`commits` and `rest` as well.

### 27. Tests that would pass with less than they say

**Low. Shown by reading.**

- `session::text_drawn_first_when_no_descriptor_is_left_is_drawn` asserts
  no notice and no ink: it shows only that nothing panicked.
- No test refuses one connection two different things, so two kinds of
  `Asked` sharing a bit would go unseen.
- `input.rs:596-602` waits for a file, sleeps 50 ms and reads it, where
  the shell makes it before it writes; and `session.rs:55-62` takes 200
  steps of 5 ms for a timer of 100 ms. Wait for the thing itself.

### 28. Smaller things the user meets

**Low. Shown by reading.**

- The built-in root menu has Settings, which only says it is not there
  yet; ADR 88 left it out until it is.
- The notice for descriptors run out ends mid-sentence
  (`i18n/en/compositor.ftl:81`).
- The guide's table of actions has no `sleep`, `restart`, `shut_down`,
  `log_out` or `open`; and `launcher.md` names the icons' themes where the
  launcher follows `icons.theme`. A test that every action is in the
  table would hold it.
- The buttons of the password boxes, the lock screen and the questions
  are 1.06:1 and 1.12:1 against their box, and an unfocused window's tab
  is marked at 1.11:1 and 1.36:1, where DESIGN.md asks 3:1.
- An urgent notification is marked by a red edge alone, where DESIGN.md
  asks a mark as well.

### 29. Documents that say what is no longer so

**Low. Shown by reading.**

- Two records are numbered 61, `0061-how-the-relay-is-built.md` and
  `0061-written-through-the-file.md`; the second is to be 95, and a test
  that the numbers are each used once would hold it.
- ADR 42 says the keeper reads each form on a thread of its own and is
  held by one that never hands over: amended on 16 November, it reads
  32 at most on one thread and lets go in time.
- ADR 38 (a window's picture, now held to a reach of its frame), ADR 91
  (positions, found to saturate) and ADR 93 (the relay's most "to come",
  done) want a dated line each.
- The architecture's order of what is drawn over what is no longer
  `Compositor::elements`'; its rows for `omorfos-brand`, `omorfos-ipc`
  and `omorfos-i18n` say less than is so.
- `docs/guide/updating.md` says the screen blinks; ADR 62 that it stands
  still and fades.
- The roadmap's ticked item of 15 November's finding 1 says a thread for
  each; the comment on `MOST_TOLD` says a renaming program is told each
  time; the workspace manifest says duplicate versions do not fail;
  `CONTRIBUTING.md` says no `unsafe` without ADR 41's exception.
- The `ci-failure` skill's table has no row for the workflows, the
  promises or the fuzzing, and says commits are checked on push, where
  `committed` runs for pull requests alone (`lint.yml:107`): nothing in
  CI checks the messages of what is pushed to `main`.

### 30. Every window is placed in Smithay's space again on every turn

**Low. Shown by reading.**

`sync` (`crates/omorfos/src/compositor.rs:2650`) maps every shown window
into the `Space` each turn, each a search, a removal and a sort of all,
and past 64 windows an allocation each; and with tiles shown, `repoint`
collects the lines between them each turn, which the `quiet` tests do
not reach. Map only what moved; keep the lines with the layout.

## What was looked at and found sound

So that it is not looked at again for the same thing:

- **The roles**: each global filtered as ADR 37 says; a layer surface
  refused without a role; activation by `may_move_focus`; the most
  windows, titles and names held; `Refused::say` bounded and names shown
  without control characters; `set_name` refusing `/`; icons of
  `xdg_toplevel_icon` in shared memory and square; the keeper's 32 kinds;
  X11 given the clipboard only while one of its windows has the focus.
- **What is read from files**: desktop entries' names checked and read to
  256 KiB; icon themes held (`plain`, `inside`, the mosts of themes,
  found, file and side); a screenshot written with `File::create_new`;
  `Exec` split with no shell; the configuration's numbers bounded as
  they are read.
- **The layering**: `omorfos-core` with no dependencies and no files,
  time or processes; config and core apart; the compositor depending on
  components only to test them; `OFFERED` held both ways by tests; every
  global made in `crates/omorfos/src`; the promises of the schemas held
  by CI; IPC.md's requests and limits as the code has them.
- **Reliability**: the holder's socket, its starting the relay again and
  letting it go after three failures; `Restarts`; children reaped by
  pidfd; the X server started again; the relay's pause when it cannot
  accept; the watcher's state; a reload keeping the last that loaded.
- **Performance**: the panel drawing only when stale; `deadline()`
  allocating nothing; icon look-ups bounded with misses kept; layer
  arrangement only on a commit.
- **Experience**: a file with a mistake keeping the rest; a wrong layout
  not stopping the compositor; no locker not locking black;
  `reduce_motion` followed throughout; `--accent-tint` following the
  accent; every message said in English, and in Polish but one, which
  falls back by design; menus steered as the guide says.
- **Tests**: `affected.sh`'s list read by nothing but the links test;
  the `quiet` ceiling asserted; the hand-over's tests on real pipes; the
  waiting helpers bounded and failing at the caller.
- **Documents**: ADR 2's list as the coverage alias has it; every path
  and test name the documents cite; IPC.md's limits; every crate in the
  architecture and the release configuration; the tiers as the gates run
  them.

## What was not looked at

- `capture.rs` per frame, `glass.rs` and `shader.rs`, the overview and
  the switcher while they move: what is drawn hottest.
- Whether the blur under a pane of glass over a private window carries
  the window's pixels into a picture of the screen; to be decided by a
  test that compares the pane in such a picture with a blank.
- The relay's `link`, `journal`, `sandbox` and `serials` for architecture
  and reliability, and its `keep.rs` for everything.
- `omorfos-hold`'s policy and renewal, the hardware backend's seat and
  terminals beyond the roadmap.
- `xdg_foreign`, pointer constraints and gestures, the IPC server's
  answers in depth, and `omorfosctl`.
- The portal's, the panel's and the notifications' keyboard reach, the
  screen reader, and most of the guide's pages on windows, screens and
  the theme.
- Most ADRs in full, DESIGN.md, BRAND.md, RELEASING.md and README.md.

Seen in passing, to be weighed: `move_request` and `resize_request`
ignore the serial, so a program may start moving or sizing its own window
at any time; a program written for X11 sets the Wayland clipboard
without the focus (ADR 27), which lets one in a sandbox with the X
socket replace what was copied; and the secret typed into the lock
screen and the password box grows by `String::extend`, so each copy left
behind as it grows is not wiped.
