# What the coverage gate leaves out

A review of 23 November 2026, of the seven files the coverage gate does
not measure, asked for by the maintainer after asking what the true
coverage of the repository is. Each finding is written so that it can be
taken up on its own: where it is, how it was shown, what to change, and
the test to write first. The roadmap lists them in the order to take
them.

## What was asked, and of whom

The gate measures 100% of lines, functions and regions, with every
feature on, and nothing in the code is kept from it by an attribute.
Seven files are kept out by name, in the `coverage` alias of
`.cargo/config.toml`, each with its reason in
[ADR 2](../adr/0002-coverage-policy.md). ADR 2 lets a file out only if
"it contains no policy: only calls into the operating system or
Smithay", and `AGENTS.md` says such code is "kept free of decisions".
The question was how far that holds: what those files decide that no
test reaches, and where each decision could go so that one does.

Measured first, by counting:

| File | Lines of code | `if` and `match` | `?` |
|---|---|---|---|
| `crates/omorfos/src/backend/hardware.rs` | 977 | 56 | 60 |
| `crates/omorfos/src/shader.rs` | 324 | 0 | 7 |
| `crates/omorfos/src/backend/winit.rs` | 314 | 8 | 11 |
| `crates/xdg-desktop-portal-omorfos/src/card.rs` | 111 | 2 | 4 |
| `crates/omorfos-hold/src/seat.rs` | 49 | 3 | 0 |
| `crates/omorfos-hold/src/exec.rs` | 31 | 2 | 0 |
| `crates/omorfos/src/gpu.rs` | 25 | 0 | 0 |
| **All seven** | **1,831** | **71** | **82** |

The `if` and `match` are those of Rust, not counting comments and the
shader's own GLSL: as first written, the table counted those too, and
said 103 (put right on 24 November).

Of the 97,357 lines of code under the crates' `src`, 29,713 are test
modules, which count as covered by running. Of the 67,644 that are not,
the seven files are 2.7%: the rest is measured, and at 100%.

The files were then read in full by the tests reviewer, for the
decisions in them that are policy, as distinct from the `?` of a call
into the system, which cannot be taken out. Every finding was checked
against the code before it was written here; none was run, so each is
**shown by reading**: the code that does it is named.

Two rows of ADR 2's table no longer say what is so. That of `winit.rs`
says its translation of events into `Input` is covered; it is not
(finding 3). That of `hardware.rs` names three decisions taken in
covered code; the file holds many more (findings 1, 2, 4, 5, 6 and 10).

## Findings

### 1. A screen that could not be drawn on is not tried again until something else wakes the loop

**Medium. Shown by reading.**

In `turn` (`crates/omorfos/src/backend/hardware.rs:774-795`), a draw that
fails is said once, the screen's picture is marked `Stale`, and `drawn`
is taken to be true, so that no tick is set to tell waiting programs a
frame has gone. The comment says it is "tried again the next turn", but
nothing makes a next turn come: `Session::run`
(`crates/omorfos/src/session.rs:298-306`) turns only after
`dispatch_now` wakes, by an event or by `Compositor::deadline`, and
nothing among the deadlines stands for a draw that failed. The screen
shows what it showed, and every program waiting for a frame waits, until
input or some other source wakes the loop. The reset after a refusal
(814-820) is the same: every screen is turned off, marked `Stale`, and
nothing is set to wake for them.

How often a driver refuses so, as a commit refused after resuming or a
buffer that cannot be had, was not looked at. At startup the fade
(ADR 62) gives a deadline, so the window is narrow there.

**To change**: take what each screen is in out of `Screen` into one
plain type in `crates/omorfos/src/backend/mod.rs`, beside `driver` and
`asks_anew`: `picture`, `dark`, `ticking`, `failing`, `tinted`, and the
card's `lit`, `active` and `before`. Its methods take plain data and say
what to do: `turn(lit, dark, awaits_frame)` returns clear, reset, draw or
skip; `drew` returns nothing, tell in a frame, or try again in a frame.
A failed draw then sets the same timer as the tick, which both tries
again and tells those waiting. `hardware.rs` only acts on what it is
told.

**Test first**:
`a_screen_that_could_not_be_drawn_on_is_tried_again_a_frame_later_and_its_waiting_programs_are_told`,
on the new type: a failed draw asks to be tried again in a frame.

### 2. Back from another virtual terminal, a screen that is to be off is not turned off again

**Medium. Shown by reading.**

On `ActivateSession` (`hardware.rs:234-248`) each screen's picture and
tint are reset, but not `dark`, nor the card's `lit`. A screen is turned
off only when they change: `dark != screen.dark` (758) and
`wanted != now.lit` (726). Both are as they were before the switch, so a
screen the configuration has off, or every screen while they are off for
idleness, is never cleared again. Smithay's `activate(false)`
(`smithay-0.7.0/src/backend/drm/device/mod.rs:434-446`) neither resets
nor commits. Such a screen keeps whatever the other session left on it;
what the kernel shows then was not looked at. For idleness, that lasts
until the next input; for a screen the configuration has off, until the
configuration changes.

**To change**: on activation, take `dark` and `lit` as not known (as
`Option<bool>` in finding 1's type), so the next turn applies them. The
least that would do is setting each screen's `dark` to false and `lit`
to true in the `ActivateSession` arm, so that the next turn's comparison
turns off what is to be off.

**Test first**:
`coming_back_to_the_terminal_turns_off_again_a_screen_the_configuration_has_off`:
after activation, a turn with the screen dark clears it, and a turn with
every screen off clears each.

### 3. How a wheel, a pointer that reports positions and a touchpad's gestures become input is decided only where no test reaches

**Medium. Shown by reading.**

`on_input` and `gesture` (`crates/omorfos/src/backend/winit.rs:220-334`)
are called only from `winit.rs:120` and `hardware.rs:561`, both
excluded, and no test implements Smithay's `InputBackend`. They decide
that a device reporting positions is placed on the area of the screen
it is given (241-243), which is the first screen in the order of
finding 5 (`hardware.rs:558-559`), at the origin if there is none; that
a scroll is its amount, or else its notches times `WHEEL_NOTCH`, or
else nothing (264-274); and which gesture events are kept (292-333). A
mistake reaches every user of real hardware, where the nested window,
on a host that sends amounts, would not show it. ADR 2 says it is
covered.

**To change**: move `on_input`, `gesture` and `WHEEL_NOTCH` into a
covered module, as `crates/omorfos/src/backend/input.rs`. They are
generic over `InputBackend` already, but Smithay's events cannot be
made in a test without a small backend of the test's own, of about a
hundred lines. A first step that needs none: two plain functions,
`scrolled(amount, v120)` and `placed(area, position)`, that the generic
code calls. Then put ADR 2's row right.

**Test first**:
`a_wheel_that_reports_only_notches_scrolls_by_whole_notches`, and
`a_pointer_that_reports_positions_lands_on_the_screen_it_covers`.

### 4. What the session tells D-Bus and systemd as it starts, starts again and ends is not tested, nor how the values reach the shell

**Medium. Shown by reading.**

`announce_session` (`hardware.rs:651-675`) runs a script with `sh -c`,
the values after it as `"$@"`, and only updates the environment where
the compositor was started again (657-660); `tell_environment` (679-686)
tells D-Bus; `end_session` (690-700) stops the session's target with
`systemctl --user stop` unless the compositor is stepping down for a
new version, which keeps the screens; `set_components` (593-625) has
programs started through `systemd-run` where the user's manager runs. A
mistake leaves the session wrong in ways hard to trace: portals that
never answer, programs started by D-Bus with no `WAYLAND_DISPLAY`, the
last session's services left running, or the target stopped under the
next compositor. That the values stay arguments and never become part
of the script is what keeps a value from being run as a command; no
test says it does.

**To change**: plain functions in `crates/omorfos/src/backend/mod.rs`
returning `omorfos_config::Command`, which is data already:
`announcement(told, started_again)` and `ending(stepping_down)`, `None`
where the screens are kept. `hardware.rs` runs what they return.

**Test first**:
`a_value_with_the_shells_signs_in_it_reaches_the_session_as_an_argument_not_as_script`
(a value such as `$(x);` is among the arguments after `sh`, never in
the script), then `a_compositor_started_again_only_updates_the_environment`
and `one_that_steps_down_does_not_stop_the_session`.

### 5. A screen driven in a new mode, and the screens found, are given their size, scale, pace, order and names in code no test reaches

**Low. Shown by reading.**

`take_on` (`hardware.rs:1033-1060`) and `follow_modes` (1204-1216) each
offer the modes, resize the output, guess the scale by its pixels
(`Scale::for_screen`), take the frame's length from the mode's refresh,
make a new damage tracker and mark the picture stale: the same steps,
written twice. A mode refused is said and not tried again (1196-1202).
`connected` (955-1006) takes a screen with at least one mode, puts the
laptop's own (eDP, LVDS, DSI) first, which is the screen a pointer that
reports positions covers (finding 3), and names it `{kind}-{id}`, the
name the configuration knows it by. `open_card` (835-856) takes the
primary card or else the first, and its render node or else the primary
one. A mistake is silent: a screen the wrong size or scale after its
mode changes, a touch screen mapped onto the external screen, a name the
configuration no longer finds.

**To change**: the shared steps as one `Compositor` method, reached
through the headless backend; the order, the name, the frame's length
and the choice of card as plain functions in
`crates/omorfos/src/backend/mod.rs`.

**Test first**:
`a_screen_driven_in_a_smaller_mode_is_as_large_as_that_mode_and_its_scale_guessed_again`,
and `a_laptops_own_screen_is_the_first_whatever_order_the_card_lists_it_in`.

### 6. What is written to the log, and how often, is decided twice where no test reaches

**Low. Shown by reading.**

`report_problems` is the same in `hardware.rs:1222-1232` and
`winit.rs:382-392`: each problem once each time they change, and each
line to be logged once. `hardware.rs` also says once a tint that failed
(264-280), a draw that failed until one succeeds (779-783), buffers of
the card that cannot be offered (583-588), the frosting (900-904) and a
screen that cannot be used (946-949). A mistake floods the journal every
turn, or says nothing, and nothing fails.

**To change**: one `Compositor` method that returns what is to be
written, keeping what was last said; both backends write what it
returns. The flags that say a failure once go into finding 1's type.

**Test first**:
`problems_are_logged_once_each_time_they_change_and_not_again_while_they_stay`.

### 7. The nested window's empty frame is decided where no test reaches

**Low, as the nested window is for trying omorfos. Shown by reading.**

The empty frame begun where nothing was drawn, so that Mesa will swap
(`winit.rs:361-366`), was added on 22 November to make omorfos run
under WSLg, a decision put into an excluded file. If it goes, the window
ends at its first frame with nothing new, as it did. Any error in
binding, drawing or swapping (344, 355, 364-365, 369) ends the nested
session, where the hardware backend tries again; whether it should is a
decision either way.

**To change**: the empty frame in a covered function generic over
`Renderer`, tested with the software renderer, counting the frames
begun. Record in ADR 5 whether a nested drawing error ends the session.

**Test first**:
`a_turn_that_drew_nothing_still_begins_a_frame_on_the_window`.

### 8. A layout of the card agreed for a shared screen but not to be had gives no memory, where its comment says something else is made

**Low. Shown by reading.**

`make` (`crates/xdg-desktop-portal-omorfos/src/card.rs:71-96`) is said to
give memory on the card "if one was agreed and it can be had there; or
what `otherwise` makes". Where a layout was agreed and allocating fails,
it gives nothing, and `otherwise` is not tried. The comment or the code
is wrong; what the stream (`stream.rs:158-161`) does with nothing was
not looked at. A card that cannot be opened (48-59) is folded into no
card, without a word, and sharing falls back to copies.

**To change**: decide which is meant, and take it out of `card.rs` into
code that takes what allocating gave as plain data, keeping only the
calls into GBM there. The roadmap's item for a card in CI says `card.rs`
needs its errors made reachable first; this is the decision to make so.

**Test first**:
`an_agreed_layout_that_cannot_be_had_is_made_in_shared_memory`, or
`...gives_no_memory`, whichever is decided.

### 9. A blur by the card's shader that fails leaves what it had drawn so far behind glass

**Low. Shown by reading.**

`blur` (`crates/omorfos/src/shader.rs:226-239`) keeps its textures for
the next time whether or not the passes succeeded, and glass
(`crates/omorfos/src/glass.rs:702-709`) draws what was kept without
asking whether the blur succeeded. It is not blurred again until what is
under the glass changes. The software blur keeps nothing where it
fails, so the two differ, and what a failure shows on the card is
whatever the textures held.

**To change**: in `glass.rs`, which is covered, not draw what a blur
that failed left, so that both blurs behave alike and `shader.rs`
decides nothing.

**Test first**:
`glass_whose_blur_failed_is_drawn_solid_not_with_what_the_blur_left`,
in `glass.rs`'s tests, with a `Frosting` that keeps something and says
it failed.

### 10. Which pointers are touchpads, and when settings are given again, is not tested

**Low. Shown by reading.**

A pointer with fingers to tap with is a touchpad, any other a mouse
(`hardware.rs:538-555`); a device added has the settings given again,
and otherwise they are given only when they change (1154-1170). A
touchpad taken for a mouse, or a device plugged in that never has its
settings, fails nothing.

**To change**: a plain function in `crates/omorfos/src/backend/mod.rs`
saying what kind of pointer a device is, and a small type for whether
the settings are to be given.

**Test first**:
`a_device_plugged_in_after_the_settings_were_given_is_given_them_too`.

### 11. Nothing stops a decision being put into an excluded file again

**Low. Shown by reading.**

Every finding above came in one decision at a time, each reasonable
where it was made, as finding 7 was. ADR 2's rule is read by people, and
nothing checks it. Counting `if` and `match` alone would miss much:
`gpu.rs:35-39` and `card.rs:51-66` decide with `and_then`, `filter`,
`then` and `is_some_and`.

**To change**: a test that parses each excluded file with `syn` (in the
lock already, so only a dev-dependency) and counts, for each, the
branches (`if`, the arms of `match`, `let ... else`, `&&` and `||`,
comparisons, and calls of `filter`, `then`, `and_then`, `or_else`,
`unwrap_or`, `map_or`, `is_some_and` and the like) against a blessed
budget, as `schemas/promised.txt` is kept: a rise fails until blessed,
and the blessing is read in review against ADR 2. The `?` of a call
into the system, and a `match` that only hands the same call to one of
two kinds of seat (`hardware.rs:339-376`), are allowed by name. It also
fails where the coverage alias, ADR 2's table and the budget do not list
the same files. It cannot see a decision passed in as data; it is a
ratchet, not a proof.

**Test first**: the test itself, failing on today's tree until the
budget is blessed at today's counts, which the findings above then
bring down.

## What was looked at and found sound

- What is decided in covered code and only called from the excluded
  files: `backend::driver`, `asks_anew`, `holder_program`,
  `ScreenMode::choose`, `Scale::for_screen`, `Compositor::awaits_frame`,
  `screens_on`, `screens_turned_off` and `is_dark`.
- `Taken` and `Refused` in `hardware.rs` (309-376): each hands the call
  to libseat or to the holder, deciding nothing.
- `frame_shown` (`hardware.rs:1125-1149`): a flip from before the screen
  was taken away does not mark it ours. Sound, and it would go into
  finding 1's type.
- `crates/omorfos-hold/src/exec.rs`: three calls; what to become is
  decided in covered code (`renew::handing_back`, `Hold::renewal`).
- `crates/omorfos-hold/src/seat.rs`: calls only. It drops why libseat
  refused (45-49), takes an event source it could not add as not
  watching, without a word (58-61), and gives `EIO` for a failure with
  no error number (28); none is a decision.
- `crates/omorfos/src/gpu.rs`: nothing chosen beyond drawing everything
  each time. Its errors become `false`, so the caller cannot say why.
  The roadmap's item for a card in CI takes it off the list.
- `shader.rs`, beyond finding 9: its passes are checked at startup
  against the software blur (`glass::check`), and its textures are made
  again for any change of size or blur (`glass.rs:784-786`).

## What was not looked at

- What happens when the holder's connection closes
  (`hardware.rs:423-427` only removes the source; where the compositor
  then ends, in `hold.rs`, was not followed).
- What the kernel shows on a screen left as another session had it
  (finding 2).
- What the portal's stream does with no memory (finding 8).
- Whether real drivers refuse a draw in the ways finding 1 needs.
- `shown` (`hardware.rs:1067-1099`), the picture the compositor before
  left, read as plumbing only.
- Seen in passing, for later: `Input::Scroll` carries no source and no
  stop of the axis, so scrolling that goes on by itself cannot be told
  to programs (`winit.rs:264-279`); and `gpu.rs:39` waits on the card,
  on the compositor's thread, for every picture taken.
