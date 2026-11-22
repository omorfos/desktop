# The whole tree again, three days on

A review of 22 November 2026, of every crate, the guide, the documents
and the tests, in each of the eight dimensions of the review skill
(`.claude/skills/review/`), three days after
[the last of the whole tree](2026-11-19-after-the-redesign-of-windows.md).
Each finding is written so that it can be taken up on its own: where it
is, how it was shown, what to change, and the test to write first.

## What was asked, and of whom

One reviewer agent for each dimension read the whole tree at once, at
`37d015b`: security, architecture, components, performance, reliability,
experience, tests and docs. Each was told what was already known, every
finding of the earlier reviews in this folder and the roadmap's open
items, and was pointed at the 39 commits since 19 November.

The machine they ran on could not fetch crates, so nothing was built and
no test was run: every finding is **by reading** or **to be shown**, and
what a reviewer said of how Smithay, zbus, wayland-backend or the kernel
behave is from memory, which each finding says. The agent that started
them then opened the code each finding names and saw that it says what
the finding says. Nothing a reviewer said was found not to be so. What
two or more reviewers found from two sides is one finding here. Ranks
are the skill's: **high** stops or ends the session, or gives away what
is private; **medium** costs without bound or breaks a promise; **low**,
the rest. Where that differs from the rank a reviewer gave, it is said.

The four findings of the docs reviewer were put right the same day, by a
correction of words; they are marked **Fixed**. Changes of code to
findings 1, 2, 5, 7, 8, 9, 10 and 12, and tests to findings 4, 13 and
14, were written the same day but could not be built there; each is a
pull request of its own, to be built and tested before it is taken, and
is marked **Written, not built**.

## Findings

### 1. One connection can have the relay, and the compositor behind it, keep about a thousand million rectangles or kinds

**High. By reading; to be shown by the test below. Written, not built,
with 65,536 as the total, which is a decision.**

`wl_region.add` and `subtract`, and `offer` on the three kinds of
source, are `Rule::Adding` (`crates/omorfos-relay/src/journal.rs:116`),
kept to be made again. The only bound is on each object:
`kept.adding.len() >= most` (`journal.rs:579-585`), with `MOST_ADDED`,
4096, or `MOST_KINDS`, 256. A link may hold `objects::MOST`, 2^18
objects (`objects.rs:20`). Nothing counts what is added over all of a
link's objects, so the two bounds multiply: 2^30 kept messages for one
connection, each with two vectors of its own. Smithay keeps every
rectangle and kind as well (`docs/UPSTREAM.md`), so the compositor holds
as much again.

Any program, one in a sandbox included, needs only `wl_compositor`: it
makes regions and adds 4096 rectangles to each, destroying none. Each
add is 24 bytes on the wire, so a few seconds of writing fills a
desktop's memory; the relay or the compositor, the largest processes,
is then most likely what the kernel ends (from memory), and every
program loses what it had not saved.

This is not [finding 4 of 19 November](2026-11-19-after-the-redesign-of-windows.md#4-what-the-relay-holds-a-program-to-it-holds-each-connection-to-and-connections-are-not-counted),
which is that what holds a connection does not hold a program, nor
[finding 5 of 15 November](2026-11-15-what-a-client-can-cost.md), which
bounded each object: within one connection, the bound is in effect none.

To change: a running count in `Journal` of what is kept by
`Rule::Adding` over all of the link's objects, checked in `keep` beside
the check of each object, with `wire::ADDED_TOO_MUCH` past a total (say
65,536 additions, or 16 MiB), and taken down by `adding.len()` wherever
a `Kept` is let go. Once finding 4 of 19 November is fixed, the same
count can be summed over a process. The total is a decision.

Test first, in `crates/omorfos-relay/src/link.rs`, beside
`a_program_may_not_add_more_to_an_object_than_any_adds`: a program makes
32 regions and adds `MOST_ADDED` rectangles to each, 131,072 in all, and
is refused with `ADDED_TOO_MUCH` before the last region. Today every
read is `Ok`.

### 2. A configuration directory taken away and put back, or a link to one pointed elsewhere, is never watched again, by any program of the session

**Medium. By reading; to be shown by the test below. Written, not
built.**

`Watcher::with_buffer` (`crates/omorfos-config/src/watcher.rs:57-64`)
adds one watch, on the directory itself, without `DELETE_SELF` or
`MOVE_SELF`, and nothing adds it again; `is_relevant`
(`watcher.rs:188-191`) passes over `IGNORED`. Every program that watches
shares it: the compositor (`compositor.rs:192-194`), the panel, the
wallpaper, the notifications and the portal.

When `~/.config/omorfos` is removed and restored, renamed and made
again, or is a link pointed somewhere else (as home-manager does on each
switch, from memory), the kernel drops the watch, or it stays on what
the link pointed at before. No edit is picked up again for the rest of
the session, and the user is told nothing. `omorfosctl reload` reloads
the compositor alone; the other programs keep the old configuration
until the next login. ADR 3 records only a linked file edited in place.

To change, in one place for every program: watch the parent directory
for the directory's own name coming and going, add `DELETE_SELF` and
`MOVE_SELF`, and in `drain`, on `IGNORED`, `MOVE_SELF` or the name
appearing, add the watch again (passing over a failure while it is not
there) and say that something changed.

Test first, in `crates/omorfos-config/tests/live_reload.rs`: the
directory is removed and drained, made again with a theme in it, and
`drain` says it changed and the new accent loads. A second test does the
same with a link pointed from one directory to another.

### 3. A window's frame is decided again by each compositor, so a framed window can come back without one, and a tab with two titlebars

**Medium, where the window is a tab; low otherwise. By reading; to be
shown by the test below.** The reviewer ranked it low; two titlebars on
one window is what ADR 97 promises against.

`Kept` (`crates/omorfos-core/src/desktop.rs:34-58`) has no `decorated`,
and `add_window_kept` works it out again as
`effects.decorations.unwrap_or(wants_decoration)` (`desktop.rs:1759-1760`).
The relay replays a window's decoration object with the last mode the
program asked for (`journal.rs:132`) before the window is shown again,
so the new compositor decides afresh from today's rules and that mode
(`handlers.rs:519-531`). A program that asked to draw its own titlebar
once its window was shown, which since `0168467` keeps its frame, is
told after a change of compositor that it draws its own, and loses its
frame; if it was a tab, `add_window_kept` puts it back in its frame
(`desktop.rs:1779-1785`), and both titlebars are drawn. A rule edited
since the window was shown does the same, either way. A change of
compositor comes with every update (ADR 60).

To change: keep `decorated` in `Kept` and in the place text
(`crates/omorfos/src/kept.rs`), and have `add_window_kept` and
`decorate` answer a window being put back from it, ahead of rules and
preference. The place text then has a new version, which a compositor
before it must read as today; that is a decision.

Test first, in `crates/omorfos/tests/integration/relayed.rs`: a window
shown framed asks `set_mode(ClientSide)`, the compositor is changed, and
the window is still framed and is not told `server_side: false`. A
second: two windows of one program made tabs of one frame, one of them
asking the same.

### 4. The test said to wait "however long that takes" after a withdrawn picture gives up after about 350 ms

**Low. To be shown. Written, not built.** The reviewer ranked it
medium. Finding 16 of
19 November is ticked as fixed, and this part of it is not.

`9393e7b` put six asks in place of a wait of 300 ms
(`crates/omorfos/tests/integration/portal.rs:786-797`); each failed ask
is a few milliseconds and a sleep of 50, so the whole is about 350 ms,
which the portal's picking thread can outlast on a loaded machine, the
fault the finding recorded. The comment says "however long that takes".

To show it: a sleep of 500 ms before `told.send(Chose::Picture(picked))`
(`crates/xdg-desktop-portal-omorfos/src/lib.rs:459`) fails the test every
time at `next.unwrap()`, with the portal right.

To change: ask until a deadline of ten seconds, as `next_change` does,
naming each request as it is made.

### 5. While "About" is shown, Super with Return, space, Escape or an arrow does what "About" does, not what the key is bound to

**Low. By reading. Written, not built.**

`about_key` (`crates/omorfos/src/input.rs:974-991`) and `about_scroll`
(`input.rs:161-171`) look at the key alone, not at what is held, and are
asked before the bindings (`input.rs:513`). So with "About" open,
`Super+Return` copies its text, `Super+space` scrolls it instead of
opening the launcher, `Super+Escape` puts it away instead of opening the
menu, and `Super+Ctrl` with an arrow scrolls. The comment above it, the
guide (`docs/guide/keys.yaml.md`) and the roadmap say a binding still
does what it is bound to. The test
`about_has_the_keyboard_its_keys_scroll_it_and_a_window_sees_none_but_bindings_work`
tries only `Super+F2`, which "About" does not take.

To change: as `menu_key` does, only when nothing is held
(`bridge::modifiers(held) == Modifiers::NONE`).

Test first, in `crates/omorfos/tests/integration/about.rs`: with
`Super+Return` and `Super+space` bound to `about`, each pressed while
"About" is shown puts it away.

### 6. Without Xwayland installed, every login's notice says "No such file or directory (os error 2)"

**Low. To be shown;** it rests on Smithay's `XWayland::spawn` starting
`Xwayland` itself, so that a missing program is `NotFound`, which is
from memory.

`started_x11` (`crates/omorfos/src/x11.rs:571-574`) picks its message by
`cannot_run`, which looks at the sockets' directory alone. The README
calls Xwayland optional and `xwayland.enabled` is on by default; the
guide (`docs/guide/x11.md`) says omorfos says so when it is not
installed, and it says only what the error says, not what is missing or
what to do.

To change: `NotFound` said as a message of its own, `x11-not-installed`,
naming `Xwayland` and `xwayland: { enabled: false }`, in English and
Polish.

Test first: a unit test beside `cannot_run`'s, that a `NotFound` error is
said as `x11-not-installed`; and by hand, the nested compositor started
with an empty `PATH`.

### 7. Today is drawn as faintly as the days of another month when the month shown is not today's

**Low. By reading. Written, not built.**

`Month::day` gives today `State::Active` in any cell
(`crates/omorfos-shell/src/month.rs:260`), those spilled in from the
months beside it included, but only `popover-day:active` gives it the
accent (`crates/omorfos-config/src/default.css:311-316`).
`popover-day-outside` has no such rule and keeps `--text-faint`, a colour
DESIGN.md says is never to be read. Opened in a month's first or last
week and moved by one, the month shows today unmarked and faint, where
the guide says "today in the accent".

To change: `popover-day-outside:active` beside `popover-day:active` in
both rules, in `default.css` and in the copy in the guide
(`docs/guide/theme.css.md`).

Test first, in `month.rs`: a month opened on 3 November and moved back
one paints 3 November as `PopoverDay` active is painted.

### 8. A screen's global, kept five seconds after the screen goes, is let go of only when something else wakes the compositor

**Low. By reading. Written, not built.** Found by the components and
tests reviewers, and
seen by the docs reviewer against ADR 15.

`withdraw` keeps the global with no time (`outputs.rs:233-236`); the next
`forget_withdrawn` (`outputs.rs:246-252`, called from `compositor.rs:1303`)
gives it one, and only a later turn lets it go. `Compositor::deadline`
does not say when, so on a quiet session the global stays until
something wakes the loop: "nothing is polled; `deadline` says when" is
not kept, and the `None` is a state that could be modelled away. The
test of `cee9ca9`,
`a_program_told_of_an_output_just_before_it_went_may_still_bind_it`
(`tests/integration/outputs.rs:95`), passes with the global never let
go of, as a global put aside is not offered to a program that comes
after anyway.

To change: `withdraw` given `now`, an `Instant` kept and not an
`Option`, and the soonest of them in `deadline`, as `updates.retiring`
is. The test then asks the deadline, with nothing else due.

### 9. The user's locale is looked up by five copies of one lookup, and the fix of 21 November went into one

**Low. By reading. Written, not built.** Worse than the "fourth copy"
the review of pull
requests 44 to 48 recorded.

The first of `LC_ALL`, a category and `LANG` set to something is looked
up in `omorfos-i18n` (`lib.rs:200`), `omorfos-session` (`lib.rs:317`),
`omorfos-ask` (`lib.rs:134`), `omorfos-sums` (`lib.rs:116`) and
`omorfos-shell`'s month (`month.rs:38`), and each takes a locale's name
apart differently, so one `LANG` can name one territory for the calendar
and another reading for numbers. `ef31f6a` mended `omorfos-ask`'s by hand,
though it depends on `omorfos-i18n` already.

To change: one `omorfos_i18n::locale(category, get)`, which the three
call, and `first_weekday` and `Numbers::asked` given the locale's name
by their callers, which depend on `omorfos-i18n` already.

### 10. How many kinds a copy may come in, and how long each may be, is said in four places, and nothing holds the relay's and the compositor's alike

**Low. By reading. Written, not built.**

`MOST_KINDS` and `LONGEST_KIND` are 256 and 255 in the relay
(`journal.rs:58-60`) and again in the compositor (`handlers.rs:666-667`),
which takes their place under the nested backend; `x11.rs:445-448` and
`omorfos-clipboard` (`lib.rs:293`) have 32, which is another idea. The
tests use the numbers written out. Changed in one place, the session
does one thing with the relay and another without.

To change: one `omorfos_ipc` constant for each, which the relay, the
compositor and `x11.rs` take, with the two 32s saying they are under it.

### 11. Each frame drawn copies the screen's name, and so does each turn while the overview, the switcher or the sealed prompt is up

**Low. By reading; the count to be shown.**

Smithay's `Output::name()` makes a copy each time, which `Head::name`
(`outputs.rs:35-38`) is kept to spare, and the roadmap ticks "a
screen's name borrowed, not copied, to draw its glass". Four places were
missed: `compositor.rs:3202`, on every frame with damage, and
`overview.rs:819`, `switcher.rs:869` and `sealed.rs:229`, on every turn
while each is up, changed or not, which `quiet.rs` does not see as it
opens none of them.

To change: borrow `Head::name` in the four. The keys of glass stay the
same, as a `str` and a `String` hash alike.

Test first, in `quiet.rs`: a quiet turn with the overview open, its
allocations counted, and the count held below what it is today.

### 12. Every start of the X server reads `/proc` and looks at `/tmp/.X11-unix`, though only a start that failed needs them

**Low. By reading. Written, not built.** Seen by four reviewers.

`started_x11` (`x11.rs:572-573`), since `37d015b`, reads
`/proc/sys/kernel/osrelease` and asks whether the sockets' directory can
be written to before it knows whether the start failed, on the loop's
thread, at every start and start again.

To change: work out what is said only when the start failed.

### 13. A program that offers too many short kinds is never shown to be refused by the compositor

**Low. To be shown. Written, not built.** The reviewer ranked it
medium.

With the count taken out of `within` (`handlers.rs:690-691`), every test
passes: `clipboard.rs:32` offers 400 kinds of 4000 bytes, refused by
their length alone; the others offer 256 or one long one; and the 300
kinds of `about.rs` are passed over as the compositor's own. Under the
nested backend, a program offering thousands of short kinds could end
the next program given the keyboard again, as finding 3 of 19 November
was, and nothing would see it. The relay's limits are tested on each.

Test first: 2000 kinds, each short, are not offered to the next program
given the keyboard, and it is still connected.

### 14. Three fixes since 19 November are run by the tests and held by none

**Low. To be shown.** The first and the keypad's Enter, and the relay's
steps, written, not built; `super_as_ctrl` on the card is read in the
part of the welcome card no test runs, and wants that part taken out to
be tested, and `lost` a run first.

- The decoration rule applied again as a window is shown
  (`handlers.rs:182-186`), for a program that names itself after it
  asks: every test names it first, so taking the block out should pass
  them all (that Smithay keeps `app_id` as it is set is from memory).
  Test: `set_mode(ClientSide)` before `set_app_id("code")`, under a rule
  for `code`, is framed and told so.
- The manager of an X server kept once its connection is cut (`lost`,
  `x11.rs:1153`): whether
  `a_program_that_closes_the_window_managers_connection_ends_only_its_x_server`
  needs it is to be seen by setting `wm` to `None` there and running
  it. If it passes, a test cutting the connection during a paste
  between X11 and Wayland.
- Keypad Enter in "About" (`input.rs:980`), `super_as_ctrl` on the
  welcome card (`omorfos-welcome/src/lib.rs:124`), and the relay's test
  of a socket that is not one (`omorfos-relay/tests/relay.rs:1015`),
  which steps only a little longer than `AGAIN` and could pass with the
  fault in on a loaded machine: a tap of keypad Enter, a card with
  `super_as_ctrl: false`, and steps for three times `AGAIN`.

### 15. Four documents said what is no longer so

**Low. By reading. Fixed.**

The roadmap had the safe ways asked of Smithay and offered to the
PipeWire bindings as still to be asked, where both were on 14 and
15 November (Smithay#2288, pipewire-rs#124); ADR 11 gave the old rule
for `output.scale: auto` without saying ADR 100 replaced it; the doc
comment of `OutputScale::Auto` left out how near a screen is read; and
the README said the launcher is started again if it stops, where it is
started when it is asked for.

## Looked at and found sound

Each of the 39 commits since 19 November was read by at least two
reviewers. Found right: the sandbox's socket that is not one
(`4fe15cb`); the 256 kinds of 255 bytes and `LONGEST_OFFER` (`b2c6a1d`);
the kinds counted before each change of focus (`3209a6b`); the X server
whose manager is cut, and its successor never cut by a late word
(`ab3dc90`, `ec49487`, `eb36a6e`); the portal replaced, and a busy bus
(`75e2588`, `d652523`); what last loaded (`cf0e9d0`); the decoration
told as the domain has it within one compositor (`0168467`); the
buttons in `Button::ALL`'s order (`58395fc`); `View::pixels` (`7cb1c22`);
the scale of ADR 100 and the guide's examples (`0b44e5c`); the week's
first day (`3f70fc2`); glass said solid in messages (`70c203a`); and the
read-only sockets' notice (`37d015b`), but for finding 12. Every link
and anchor in every document was checked and none is broken. The
dependencies point inwards, `omorfos-icons` apart, which is known.
`omorfos-session`'s logind watcher, left unread on 19 November, takes
signals from `org.freedesktop.login1` alone.

## Not looked at

The sources of Smithay, zbus, wayland-backend and calloop, which were not
on the machine; what the findings say of them is from memory. The
portal's `feed.rs`, `stream.rs`, `prompt.rs` and `card.rs`; the relay's
`keep.rs` replay; `omorfos-hold`, `omorfos-lock` and the hardware
backend; Polish wording beyond its being there; contrast of the theme's
colours; and the roadmap's open items before stage 8 that nothing since
19 November touched.

## Seen in passing

- `cannot_run`'s test (`x11.rs:1203`) takes a directory of mode 0555 to
  refuse writing, which it does not for root: run as root, the suite
  fails. Seen by five reviewers.
- After three starts in a minute, the X server is not started again for
  the rest of the session (`restarts.rs:24`, `x11.rs:815-821`), and since
  `ab3dc90` any program written for X11 can bring that about with
  `KillClient`. Better than ending the compositor, which it did, but for
  good.
- Programs are told the third button is `maximize`
  (`bridge.rs:401`), where on omorfos's own titlebar it means fullscreen
  (`input.rs:815`).
- The notices for read-only sockets and WSL still carry Smithay's "Could
  not find a free socket"; `keyboard-layout-invalid` says "(variant ``,
  options ``)" when only a layout is set.
- The roadmap's WSLg item points at `docs/UPSTREAM.md` for asking Smithay
  to listen on the abstract socket alone, which has no entry yet.
- A title's every change reaches each lister of windows at once, not
  folded; nothing in the tree lists them yet but the panel.
- If asking the bus who holds the portal's name times out as it starts,
  a later portal is not heard of as `Replaced` (`bus.rs:312`).
