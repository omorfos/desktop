# The whole tree again, after the redesign of windows

A review of 19 November 2026, of every crate, the guide, the documents
and the gates, in each of the eight dimensions of the review skill
(`.claude/skills/review/`), three days after
[the last of the whole tree](2026-11-16-the-whole-tree.md). Each finding
is written so that it can be taken up on its own: where it is, how it
was shown, what to change, and the test to write first.

## What was asked, and of whom

One reviewer agent for each dimension read the whole tree at once, at
`2a57541`: security, architecture, components, performance, reliability,
experience, tests and docs. Each was told what was already known, every
finding of the three earlier reviews and the roadmap's open items, and
was pointed at what is new since 16 November: the redesign of windows
(ADR 97), glass on everything that floats, the portal saying its
settings as it starts, "About" without its button, and the repository's
top tidied. Two tests had each failed once in the day before, one in CI
and one here; the reviewers of reliability and of tests were asked to
explain them from the code, not to run them again.

A gate's coverage run had the machine while they read, so none ran
anything: every finding came back **by reading** or **to be shown**.
The agent that started them then opened the code each names, and ran the
tests of those said below to be **shown by a test**, with the fault as
it was. What two reviewers found from two sides is one finding here.
Ranks are the skill's: **high** stops or ends the session, or gives away
what is private; **medium** costs without bound or breaks a promise;
**low**, the rest. Where that differs from the rank a reviewer gave, it
is said.

Ten of the findings were put right the same night, each by a test
written first, or by a correction of words; they are marked **Fixed**
and kept here for how they were shown. Nothing a reviewer said was found
not to be so. One was narrower than said: finding 2 needs a window that
asks for blur behind itself, and a titlebar and a panel alone, which was
tried first, do not bring it about.

Measured on the way, on the maintainer's machine, with `omorfosctl
frames`: a frame with three blurs in it took 15 to 17 ms, the whole of
what a screen at 60 Hz allows, and the compositor had 22% of a
processor while a terminal wrote. That is the cost of glass the roadmap
has still to measure properly, on a quiet screen.

## Findings

### 1. Any program on the bus could say what the bus says, and so become the portal, let go what others hold, or end the portal and the agent that asks for passwords

**Medium. Shown by a test. Fixed.**

`lost`, `left_the_bus` and `portal_changed` in
`crates/xdg-desktop-portal-omorfos/src/bus.rs`, and `lost` in
`crates/omorfos-ask/src/agent.rs`, took a message for the bus's word if
it was a signal called `NameOwnerChanged` or `NameLost` with the right
words in it. None asked who sent it. A match rule only says which
broadcasts to receive: a signal sent straight to a program's name is
delivered whatever the rule.

So a program on the session's bus could tell the portal's backend that
it was now the portal, and be answered what only the portal may ask,
which undid the fix of finding 2 of
[what the first review did not look at](2026-11-16-what-the-first-review-did-not-look-at.md);
that another program had left, which let go what that program held
against sleep; or that the backend had lost its name, which ended it.
On the system's bus, where any user may send a signal, another user of
the machine could end `omorfos-ask`, and after four times in a minute it
is given up for the session.

Shown by three tests, each of which failed first:
`a_program_that_says_what_the_bus_says_is_not_believed` and
`a_program_another_says_has_left_the_bus_keeps_what_it_holds` in the
portal, and `the_agent_does_not_believe_another_program_that_polkit_has_gone`
in `crates/omorfos/tests/integration/ask.rs`. The bus writes who sent
each message, which no program can write for itself; each of the four
now asks that it is `org.freedesktop.DBus`.

Left: the comment at the top of `bus.rs` says only the portal is
answered. A program outside a sandbox may be able to take the portal's
name honestly, if `xdg-desktop-portal` lets its name be replaced, which
was not checked. That is within what ADR 37 says of programs outside a
sandbox, but the comment should say it.

### 2. A window that asked for blur behind it, beside glass of another blur, made the screen draw for ever

**Medium, high as the reviewer ranked it. Shown by a test. Fixed.**

`Glass` in `crates/omorfos/src/glass.rs` kept one blurred picture of a
screen's desktop, for one amount of blur. A sheet of glass with nothing
but the desktop behind it took that picture if it was for its own blur,
and otherwise threw it away and made its own. With two such sheets of
different blurs on one screen, each threw the other's away on every
drawing: two whole-screen pictures made and blurred, each a new
element, so the screen was damaged and drawn again, without end.

The built-in theme has both: glass is blurred by 32, and what is behind
a window that asks (`ext-background-effect`, or KDE's protocol) by 24.
A theme that gives one part its own `backdrop-filter` does the same
with no program involved.

Shown by
`glass_of_two_blurs_over_the_desktop_alone_is_not_blurred_again_while_nothing_changes`
in `crates/omorfos/tests/integration/glass.rs`: before the change every
drawing was damaged and the count of blurs rose by two each time. The
desktop's picture is now kept by the screen and the blur, and one no
sheet uses is dropped.

Whether this is what the maintainer's machine was doing is not known:
it depends on whether a program there asks for blur.

### 3. A program that copies in very many kinds ends each program next given the keyboard, and the keeper of the clipboard

**Medium. By reading, then shown by a test. Fixed, but for the keeper of
the clipboard and a drag where there is no relay.** The test showed more:
kinds named after the selection is set are sent on as well, so a count
made as it is set would have been got past.

Smithay keeps every kind a `wl_data_source` offers
(`wayland/selection/data_device/source.rs:68`) and sends all of them to
whoever is given the focus. `new_selection` in
`crates/omorfos/src/handlers.rs` bounds what goes on to X11 alone, and
the relay's journal lets a source have 4096 additions of up to 4096
bytes each (`crates/omorfos-relay/src/journal.rs:51`). A source of a few
hundred long kinds is more than a socket holds beside ADR 45's
mebibyte, so the program the user has just gone to is disconnected,
with what was unsaved in it, and `omorfos-clipboard` with it. Finding 4
of the last review closed the way from X11; from Wayland to Wayland it
is open, and a program in a sandbox can do it.

To change: in the relay, hold `offer` on the three kinds of source to
256 a source and 255 bytes each, ending the offender; for a compositor
with no relay, the same asked of Smithay in `docs/UPSTREAM.md`, and
meanwhile a selection past the bound cleared in `new_selection`.

Test first, in `clipboard.rs`: one program offers 400 kinds of 4000
bytes and sets the selection; a second is given the keyboard and its
window is still there after a round trip.

### 4. What the relay holds a program to, it holds each connection to, and connections are not counted

**Medium. By reading; the last step to be shown.**

`Link::held` (`crates/omorfos-relay/src/link.rs:157`, `:526`) is the
link's, as are `objects::MOST` and `journal::MOST_ADDED`, and `admit`
(`lib.rs:662`) takes every connection. Finding 4 of
[the review of 15 November](2026-11-15-what-a-client-can-cost.md) is
recorded as fixed by "the relay holds a program to 4096 descriptors";
one process with 128 connections has 128 times that, which is the
relay's hard limit. The relay then stops listening and says so, as it
was made to; whether a descriptor an honest program sends meanwhile is
dropped by the kernel, and that program let go for it, is to be shown.

To change: a most number of links for each process and each sandbox,
and in all; and what is held summed over the links of one process
against the limit.

Test first, in `crates/omorfos-relay/tests/relay.rs`: one process on
three connections hands over 2000 pools on each, and one is let go.

### 5. A portal started again leaves every screen it was sharing still shared, with nothing left that can end it

**Medium. By reading.**

When `org.freedesktop.portal.Desktop` changes owner, the bus thread
clears its table of sessions (`bus.rs`, where `portal_changed` is
handled) and tells nobody. The main loop's shares, each with its feed
and its PipeWire stream, are ended only by a `Session.Close`, which the
new portal cannot send as it knows none of the old paths. A question on
the screen, or a picker, from the portal that went stays up too. The
roadmap's ticked item for finding 2 of the second review of 16 November
says sessions are "forgotten when the portal starts anew"; what they
were sharing is not.

To change: where the owner is seen to change, tell the main loop that
each session has ended before clearing them, and withdraw what is on
the screen.

Test first, in `bus.rs`: a session made, another program takes the
portal's name, and the service is told the session ended.

### 6. While a screen is recorded with a private window on it, glass is made and blurred again on every drawing

**Medium. By reading; to be shown.**

The pass that draws a picture of the screen and the pass that draws the
screen both call `Glass::frost` under the screen's name
(`crates/omorfos/src/compositor.rs:3245`), and share each sheet's
picture, though a private window is a blank or under a veil in one and
itself in the other. A sheet within reach of the private window is
blurred twice a drawing, each damaging the other's, and the veil's own
sheet, which is in one scene only, is thrown away by the other. The
roadmap's "known gaps in a blurred private window" does not have it.

To change: tell `frost` which drawing it is for, and keep by it.

Test first, in `capture.rs`: a private window under a glass panel, a
program asking for a picture after each one; once still, a further step
damages nothing and blurs nothing.

### 7. A sheet of glass with a window behind it has every texture made anew on each frame its size changes

**Medium. By reading; to be shown.**

A covered sheet's picture is exactly as large as the sheet and its
reach, within the screen (`glass.rs`, `around` and `is_for`). Whenever
that differs from the frame before, the target, its tracker and one
texture for each pass of the blur are all made again: on every frame of
a window, menu or notification opening over another window, of a window
resized over another, and of a titlebar dragged near a screen's edge.
The blur is owed; the allocation is not.

To change: keep a picture while what is needed fits in what it has,
made in sizes rounded up.

Test first: a `Frosting` that counts the times it is given nothing kept;
a glass menu opened over a window through ten frames, and the count is
one.

### 8. The welcome card, the bar's popovers and a component's tooltips are see-through with nothing blurred behind them, and `reduce_transparency` does not make them solid

**Medium: what ADR 97 and DESIGN.md promise. By reading; to be shown.**

The compositor lays glass under a layer surface whose namespace is a
part's name (`compositor.rs`, `fade_layers` and where panes are made
for layers). The welcome card's surfaces are named `welcome` and
`welcome-backdrop`, which no part is, though it is drawn as the
launcher; the bar's popovers and tooltips are pop-ups of the bar's
surface, and a pane is made for the layer's own geometry alone. Each is
drawn in the theme's colour at 80 or 85 in a hundred over sharp
windows, and no component reads `reduce_transparency`, so "whoever
needs more has `reduce_transparency`, which makes all of it solid" does
not hold for them. The welcome card has no shadow and no fading either.

To change: let a component say where its glass is, through
`ext-background-effect`, in `omorfos_shell::Sheet`, and draw its box
solid where it is told there is no blur; which is what ADR 98 asks of
programs that stand alone. The smallest stopgap names the welcome
card's surfaces as the launcher's.

Test first, in `welcome.rs`: with `reduce_transparency: true`, a red
window under the card, and the card's ground is its colour exactly.

### 9. In the light scheme two of a titlebar's three buttons can barely be seen, and without focus none can in either scheme

**Medium. By reading, the contrast reckoned.**

The buttons have the dark scheme's colours in both
(`crates/omorfos-config/src/default.css`, `--close`, `--minimize`,
`--maximize`). On the light titlebar, close is about 2.5 to 1,
minimize 1.1 and maximize 1.25; the rim added for the purpose
(`light.css`, `--button-rim`) is 1.35. Without focus the buttons are
`--border`: about 1.3 on glass in the dark and 1.1 in the light.
DESIGN.md asks 3 to 1 of what takes input, and `meets_contrast` in
`theme.rs` holds no button to it. The signs show only once the pointer
is on a button, and `high_contrast` is not built.

To change: a rim that itself reaches 3 to 1 on the light titlebar, or
deeper fills there; `--border-strong` for the buttons without focus.

Test first: in `meets_contrast`, each button, with focus and without,
in both schemes, at 3 to 1 against the titlebar by its fill or its rim.

### 10. The wash that shows where a dragged window would land was made faint twice

**Low. Shown by a test. Fixed; its colour is left.**

`crates/omorfos/src/tiles.rs` made each channel of the wash as faint as
its share itself, and since colours were made to be drawn premultiplied
(`56c385b`) `color()` did so again: the half a window would take was a
shade off the desktop, about 1.2 to 1. The existing test asked only
that the pixel was not the background. It now asks that it is plainly
lighter, and the wash is its colour at its share, once.

Left: the wash takes the colour of a focused window's border, which is
now a see-through white or navy whose alpha is ignored, so over a white
window in the dark scheme, or a dark one in the light, it does not
show. The accent, which DESIGN.md says means "this is what will
happen", would.

### 11. A program is told who draws its titlebar by one layer and framed or not by another; under a rule, or when it asks late, the two disagree

**Low. By reading; to be shown.**

`decorate` in `crates/omorfos/src/handlers.rs` settles what a program
is told from its preference alone; whether it has a frame is settled in
the domain, where a rule comes first
(`crates/omorfos-core/src/desktop.rs`, `effects.decorations`). A
program that prefers its own titlebar under a rule `decorations: true`
is told it draws its own and is framed as well. A window shown with no
decoration object that makes one afterwards is told the compositor
draws it, and has no frame. The one test covers a window that asked
before it was shown, in one direction.

To change: one decision, in the domain, asked before the first
configure, and what a shown window is told taken from what the domain
has for it.

Test first, in `decorations.rs`: a program a rule frames is told the
compositor decorates it; a window shown without a frame that asks then
is not told so.

### 12. A private window that is to be blurred shows sharp, in pictures of the screen, whatever it draws outside its frame

**Medium until shown, and high if it is; the reviewer ranked it low. By reading.**

With `private: blur` the window is drawn whole under a pane of glass
the size of its frame (`compositor.rs`, where the veil covers
`shown.outer`). Its pop-ups are left out; its subsurfaces are not, and
with no rounded frame of ours the whole tree of surfaces is drawn
uncut. A part on a surface of its own beyond the window's geometry, as
some toolkits' tooltips and menus are, would be in every picture and
share of the screen unblurred. `private: blank` draws nothing of the
window and is not touched.

To change: cut what such a window draws to its frame, or leave out
surfaces that reach beyond it.

Test first: a window under `private: blur` with a red subsurface 40
units to its right; a picture of the screen has no red there.

### 13. Every sheet of glass looks at everything under it on every drawing, changed or not

**Low, medium on hardware if measured so. By reading.**

Each covered sheet hands the whole list under it to a tracker of its
own (`glass.rs`, `Picture::draw`), with a framebuffer made, checked and
deleted, though nothing changed: the square of the number of windows a
drawing, and on hardware on every turn that finds no frame queued. The
roadmap has the count of allocations for a titlebar; not that they grow
with what is under.

To change: fold what decides a sheet's picture into one number, and
skip the tracker when it is as it was.

Test first, in `quiet.rs`: the bytes allocated over quiet turns with
eight cascaded windows are no more than four times those with two.

### 14. Two tests failed by the order of things, not by the code

**Low. Shown by a test each. Fixed.**

- `portal::an_application_is_given_the_colour_the_user_picks_from_the_screen`
  failed once in CI on 18 November. Each question was asked on a
  connection of its own that took the portal's name, and in one place a
  second was made before the first had sent: the backend then rightly
  refused the first, as no longer the portal. With a delay of 300 ms
  before each question is sent, it failed every time.
  `portal.rs`'s `Daemon` now has one portal connection, as
  `sharing.rs`'s had, and the tests pass with the delay in.
  The backend was read for the same race and has none: it reads signals
  and calls from one stream, in order.
- `windows::a_program_that_places_its_parts_at_the_ends_of_the_numbers_stops_nothing`
  failed once here, on a loaded machine. It closed one program and
  opened the next at once; where the first had not yet been seen to go,
  the new window was placed a step along the cascade, for good, away
  from the pixel looked at. With the order turned round it failed
  every time. It now waits for the first to be gone.

### 15. The test that "About" has no button pressed five units beside where the button was

**Low. Shown by a fault put in. Fixed.**

`about_has_no_button_that_closes_it_and_a_press_where_one_was_does_nothing`
in `about.rs`, written the night before, pressed in the padding. It
presses the middle of where the button was, and fails with a press
there made to close.

### 16. Four more tests say less than their names

**Low. By reading; each to be shown as said.**

- `decorations.rs`, the light scheme's hairline: asked to be "less than
  cream" as a number, which the shadow under it is too. Give the test's
  theme no shadow and compare by channel.
- `portal.rs`, after a picture is withdrawn: the next question is asked
  after 300 ms by the clock, which the portal's thread can outlast, for
  good. Ask until not refused at once.
- `bus.rs`, the announcement at start, and the older test of what has
  changed: `next_change` waits without bound, so the fault hangs the
  run where it should fail it, and the eleven is written out.
- `portal.rs`, `a_key_pressed_before_a_question_is_up_does_not_answer_it`:
  the question that finishes is never joined, so it need not have been
  refused for the reason the test is about.

### 17. The portal's backend can wait for ever as it starts, holding its name, on a busy bus

**Low. By reading; to be shown.**

`Portal::open` makes a reader of the connection's queue of 64 messages
that nothing drains until its thread starts, then adds a match for
every `NameOwnerChanged` on the bus and makes four calls with no
timeout. A burst of 64 messages before the last reply stops the reader,
and the call never returns: a backend that holds its name and answers
nothing, and is not started again as it has not ended.

To change: room for more than a burst and a timeout on calls, so that
the worst is a start that fails and is made again.

### 18. What a starting portal says of every setting may not reach programs when it takes its name from a copy still running

**Low. To be shown, with the real portal.**

Programs hear settings through `xdg-desktop-portal`, which, as far as
is remembered of GLib, drops signals from a sender that is not the
owner it knows until it has asked the new owner for its properties. The
eleven signals are sent as the name is taken, before that. Where the
old copy had already ended, as at an upgrade, they pass. Beside that,
the look said at the start is read before the compositor is connected
to, and a change between the two is never said.

To show: `dbus-monitor` on the settings interface in a session, a
second backend started while the first runs, and the signals counted.

### 19. The copy kept as "what last loaded" is read from the disk a second time

**Low. By reading.**

`Store::keep` (`crates/omorfos-config/src/store.rs:127`) reads each
file that loaded again and writes that as the last good. A save between
the two reads is kept though it was never parsed, and a compositor
started again meanwhile comes up with the defaults for that file:
finding 7 of the last review, through a window of its fix.

To change: keep the text that was parsed.

### 20. A sandbox whose socket is not one has every program let in late, and the log say descriptors ran out

**Low. By reading.**

`listen` in `crates/omorfos-relay/src/sandbox.rs` takes any descriptor;
`accept` on one that is no socket fails ten times a second, each
pausing the relay's own listening for a tenth of a second and saying,
once, what is not so.

To change: end that sandbox's thread on an error that is not one of
running out.

### 21. Part of a notice the user reads is English written in the code

**Low. By reading.**

`crates/omorfos/src/glass.rs:265` and `:276`, and
`compositor.rs:2075`, make the `$why` of the message `glass-solid`
from English; `compositor.rs:1462` pushes a whole English sentence
about the keyboard's layout. Finding 13 of the last review named
others.

### 22. "About" has the keyboard, and its one button cannot be pressed with it

**Low. By reading.**

`about_key` in `crates/omorfos/src/input.rs` takes Escape and the keys
that scroll; Copy answers a press of the pointer alone, and is the only
control left on the card.

To change: Enter copies, and the guide's row says so.

### 23. A titlebar over a fullscreen window is said to be square, and only its bottom is

**Low. By reading; to be shown.**

`compositor.rs:6277` sets the radius to nothing for a fullscreen frame,
which reaches the bottom corners alone; `paint::titlebar` rounds the
top by the theme's.

### 24. The side and order of a titlebar's buttons are said three times

**Low. By reading.**

`Button::ALL` in `omorfos-core`, `BUTTONS` in
`crates/omorfos/src/bridge.rs`, and `Theme::button_layout` in
`omorfos-config`, which is what programs are told. Nothing holds one to
another.

To change: build `BUTTONS` from `Button::ALL`, and a test in `bridge.rs`
that what programs are told is the order omorfos draws.

### 25. The gates' quick checks are listed four times and have drifted; `uncovered` by itself cannot see that tests failed

**Low. By reading.**

`tools/gates.sh`: `commit`, `light`, `checked` and `commits` each write
out typos, the hook, the formatter and clippy; `commit` and `light`
also run the gates' own test and the links, and `checked`, which is
what `push` runs on a commit that built something, does not, which is
why finding 26 of the last review is half open. `uncovered` run as a
tier of its own reads a log it has just emptied.

To change: one `quick()` called by all three; `uncovered` given the log
to read.

### 26. What is written more than once in the compositor's drawing

**Low. By reading. One finding for the roadmap's "made once" item.**

- An area turned into pixels by hand in six places beside
  `View::pixels` (`glass.rs`, `compositor.rs`, `overview.rs`,
  `sealed.rs`, `switcher.rs`): small and safe.
- A card's pixels, shadow and glass laid by the same lines in about ten
  places, in two orders of the same sum; a `Card` would hold it.
- "Lines of text in a box" three times: `omorfos_ui::tooltip`,
  `paint::notice`, `paint::keep_screens`.
- The height of a line said three ways, and the notice's, "About"'s and
  the tooltip's rows fixed where the font may be larger.
- A part's border alone, the line round what is shared, a domain corner
  as a Smithay point, each written out several times; `Chrome::radius`
  a second copy of `appearance.radius`.
- `box-shadow` and the three lines of glass repeated on seven parts of
  the built-in theme, where the hairline and the blur are one variable
  each; a new variable is a new promise (ADR 73).
- Which half or quarter a dragged frame is offered is worked out in the
  adapter (`tiles.rs`, `offered`), where it is geometry the domain has.
- `compositor.rs` and `paint.rs` each hold several ideas; in `paint.rs`
  "sign" names four things.

### 27. Documents and comments that said what is no longer so

**Low. By reading. Those of a line corrected; the rest left.**

Corrected: the built-in theme's comments on `--surface`, `--danger`,
`--warning`, `--success`, `--glass-sealed` and "About", with the
guide's copy and the manual page; two comments on how far glass is
blurred; `paint.rs`'s first line; CONTRIBUTING.md on what `push` runs;
the `ci-failure` skill's "script beside this file"; the README's "two
files"; the nightly workflow's "two crates allowed `unsafe`";
ARCHITECTURE.md's ext-workspace, which is not offered; ADR 33's status;
DESIGN.md's "nothing is longer than 300 ms" and its change of scheme
over two seconds; `docs/README.md` on what the manual pages are made
from. The commit hook searched no new file with a space in its name.

Left, as each wants more than a line or a decision:

- ACCESSIBILITY.md says reduced motion is not a setting, the theme is
  yet to be redrawn, focus is in the border and keys are bound in
  `config.yaml`: four passages to rewrite.
- `--glass-thin` is said to be the panel's and is used by nothing, at a
  tint under the floor DESIGN.md gives; and DESIGN.md and ADR 97 say
  thin glass is not built because the bar writes dim text, of which
  none was found. To decide which is so; the variable is promised.
- DESIGN.md's "Following the sun" on the location, and its first
  principle's "over any wallpaper" against the rule as relaxed.
- TESTING.md's times for the tiers, a third of what the logs show, and
  its table without `all`; ADR 94 on what `scripts` runs.
- `Part::ButtonClose`'s doc, which is the schema's words, on when it is
  under the pointer; ADR 99's "each entry says how to check".
- ARCHITECTURE.md's rows for `omorfos-ask`, `omorfos-shell`,
  `omorfos-screenshot`, `omorfos-ipc` and `omorfos-core`, with finding
  29 of the last review.
- That the guide's copy of the theme is held to the built-in one by
  what it parses to, not by its words, so its comments can differ.

## What was looked at and found sound

- **The owner the portal's backend follows**: the match is added before
  the owner is asked and the stream made before both; signals and calls
  are read in one order. No order by which a real portal started again
  is refused was found.
- **Glass over a private window in a picture**, which the last review
  did not look at: with no blur, or a picture that cannot be made, the
  veil is solid; a ghost of a private window is left out. But for
  findings 6 and 12.
- **The new titlebar and `Decoration`**: every subtraction saturating
  or of a smaller from a larger; a frame narrower than its buttons laid
  out without wrapping.
- **The grab of a pop-up**, finding 1 of the last review: still held.
- **Text on glass at 0.8 and 0.85**: the relaxed rule holds for text
  and dim text on every glass part, and `meets_contrast` runs it for
  both schemes and all six accents.
- **`reduce_transparency`** where the compositor lays the glass: every
  pane solid, programs told they may have no blur. But for finding 8.
- **Keyboard reach** of what the frame offers the pointer, and of the
  welcome card without its button.
- **Words**: nothing the redesign shows is new text; English and Polish
  have the same messages.
- **Dependencies between crates**: `omorfos-core` has none; no
  component depends on the compositor; Smithay is in three crates.
- **The moves of 18 and 19 November**: one stale place found, in the
  `ci-failure` skill; the two indexes list every document and crate.
- **ADR 97 against the code**: the titlebar's height, the buttons'
  sizes, order and gaps, the tints, the hairline, the blur and the
  contrast rule.
- **`Session::eventually`**: its patience grows with load, so a failure
  in it means a state that cannot come, as in both of finding 14.
- **The relay's doubtful links and its threads for sandboxes**, the
  fixes of findings 2 and 3 of the last review: still held.

## What was not looked at

- The relay's `keep.rs` and `journal.rs` beyond the rules for `offer`;
  the hardware backend; the X server started again and `x11.rs`;
  `omorfos-hold`, `omorfos-lock`, `omorfos-session`.
- The portal's `feed.rs`, `stream.rs`, `screencast.rs`, `prompt.rs` and
  `card.rs`.
- Anything on real pixels: the title's centring, the signs at
  fractional scales, the blur itself; screen readers.
- Most bounded waits in the tests: about a dozen of some hundred were
  read.
- Whether `xdg-desktop-portal` lets its name be replaced (finding 1),
  whether the kernel drops descriptors as finding 4 says, and GLib's
  handling of a new owner's signals (finding 18): each from memory.
- Whether any other signal a program acts on is taken without asking
  who sent it: `omorfos-session`'s watcher was said to match by sender
  and was not read again.
- The roadmap's ticked items one by one, IPC.md, BRAND.md, UPSTREAM.md,
  and the ADRs but for 7, 33, 65, 94 to 99.
