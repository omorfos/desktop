# Accessibility: the design, the brand and the shell

A review of 26 November 2026, of what omorfos draws and how it is
driven, for the people [ACCESSIBILITY.md](../ACCESSIBILITY.md) is
written for: those who see little or nothing, who cannot use a pointer
or cannot use a keyboard, who cannot hold a button down or several keys
at once, who read slowly, or who do not hear. Each finding is written so
that it can be taken up on its own: where it is, how it was shown, what
to change, and the test to write first. The roadmap lists them in the
order to take them, and
[issue 180](https://github.com/omorfos/desktop/issues/180) keeps them as
a list to tick.

Nothing was changed. The findings are for another day; this is so that
none of them has to be found again. Two of them are high, and are to be
put right before a release of any kind.

## What was asked, and of whom

Two things, one after the other.

**First, of the design.** Where the visual design puts someone at a
disadvantage, and where what [DESIGN.md](../DESIGN.md),
[BRAND.md](../BRAND.md) and ACCESSIBILITY.md say of contrast is wider
than what is so. One reviewer read those three and the decisions behind
them (ADRs 32, 34, 55, 66, 70, 71, 75, 76, 97, 105 and 109), and
findings 1 and 2 came of it.

**Then, whether that was all.** It was not. The first pass looked at
contrast alone: parts of three of the 55 criteria WCAG 2.2 has at levels
A and AA, worked out from the documents' own tables rather than from
what is drawn. It took DESIGN.md's "nothing that can be clicked is
smaller than 24 by 24" and ADR 71's reduced motion as sound without
opening the code, and looked at nothing of how the desktop is driven:
the keyboard, the pointer, time, mistakes, secrets, settings, or the
tools a person may have to bring with them. Its reviewer had been given
a long list of known gaps to pass over and told to prefer few findings.
The second pass did what the first only planned: four readers went
through the code for facts (the pointer and drags; time; text and
fields; settings and assistive technology), and each claim was checked
against the code it names before it was kept. Findings 3 to 26 are its,
the last found by looking for the third the other way round, and it
measured the first two again.

**Against what.** [WCAG 2.2](https://www.w3.org/TR/WCAG22/), levels A
and AA, read for a desktop as
[WCAG2ICT](https://www.w3.org/TR/wcag2ict-22/) reads it for software;
and what EN 301 549 asks of a platform besides, which is what a desktop
is. [WCAG 3.0](https://www.w3.org/TR/wcag-3.0/) is a working draft, and
is used only to see which way things are going. None of this is a claim
that omorfos conforms to anything, nor a count of where it does not: the
criteria are used as a list of the ways people are shut out, so that
none is forgotten.

**What was read.** `main` as it stood on 26 November 2026, with the
launcher's modes and top hit in it (ADR 109, part 1): the line numbers
are those of the commit this one follows. The launcher's new look (part
2, pull request 177) was read on its branch as it was being finished;
what is said of it is marked **#177**, and is not on `main`. The
switcher going through a program's windows (pull request 176) was merged
while this was written, and was read no further than the lines named
here.

**How each was shown** is said with it: **shown by a test** (one that is
already there and is named, or one written, run and taken out again,
which is given, to be put back); **by reading** (the code that does it
is named); **measured from the theme** (the colours
`Theme::built_in` gives every part in every state, in both schemes and
all six accents, asked for by a small program kept outside the
repository, laid over what is behind them as the theme's own test lays
them, and compared as WCAG 2 does: every figure is in
[the contrast of every part](assets/2026-11-26-accessibility-contrast-of-every-part.md));
or **computed from the constants** (a layout worked out from its sizes;
the root menu was laid out by `omorfos_core::Menu` itself). But for the
tests of findings 3 and 26, nothing was run to show a finding, and
nothing was looked at on a screen, so each of the rest wants its test
before it is believed, and names one.

**Ranks** are the review skill's, read for accessibility: **high** gives
away what is private, or ends the session without that being meant;
**medium** shuts someone out of something, or breaks a promise DESIGN.md
or ACCESSIBILITY.md makes; **low** is the rest.

**The pictures** are sheets drawn by hand from the built-in theme's
colours and sizes, with what there is now on the left and one direction
on the right. They are for talking over: nothing in them is decided, and
none is a picture of a screen.

### Two findings are of security, and are written here

Findings 3 and 26 are of the kind [SECURITY.md](../SECURITY.md) has
reported in private, not in an issue or a document, and this review
first left them out. The maintainer decided on 26 November 2026 that
they are written here with the rest: omorfos has had no release, so
there is nobody yet for keeping them quiet to protect, and what is
written down is not forgotten. Both are to be put right before a release
of any kind, and the roadmap holds the first release to that.

As SECURITY.md has it, a program outside a sandbox is not walled in from
the rest of the user's session, and could have the screen by other ways.
So finding 3 counts for a program in a sandbox, which is held to its own
windows and may ask the portal for the screen. Finding 26 counts for
every program: the administrator's password is what no program of the
user's has, and the seal round the prompt is there to keep it so.

## In short

Twenty-six findings: two of them high, fourteen medium and ten low. The
two that are high are written first, whatever their numbers.

| # | Finding | Rank | Shown |
|---|---|---|---|
| 3 | A key meant for another program says yes to sharing the screen | **high** | by tests that are there |
| 26 | A box for a secret that is withdrawn hands the keyboard straight back | **high** | by a test |
| 1 | Dim text on glass is under 4.5 to 1 at its worst | medium | measured |
| 2 | An accent of one's own can take away the focus ring and every lit row | medium | measured |
| 4 | A window is moved and resized only by dragging | medium | by reading |
| 5 | Nearly everything acts as the button goes down | medium | by reading |
| 6 | Which program is taking pictures is told only to a pointer at rest | medium | by reading |
| 7 | Times that cannot be changed, and one that says yes by itself | medium | by reading |
| 8 | A passphrase cannot be pasted, nor anything looked up while it is asked | medium | by reading |
| 9 | The lock screen does not say why a password failed | medium | by reading |
| 10 | Larger text is cut, and a larger scale runs off the screen | medium | computed |
| 11 | States said by marks under 3 to 1, and one not said at all | medium | measured |
| 12 | Accessibility settings are turned on only by editing a file | medium | by reading |
| 13 | Assistive technology has no place in the model of roles yet | medium | by reading |
| 14 | The boxes that ask do not answer alike | medium | by reading |
| 15 | The contrast test is a list kept by hand, and the checklist does not ask | medium | by reading |
| 16 | The faint colour is used for text | low | measured |
| 17 | An offered window's title is written straight onto the wallpaper | low | by reading |
| 18 | Four things are smaller than 24 by 24 | low | by reading |
| 19 | Reduced motion leaves two things moving | low | by reading |
| 20 | What Enter starts follows a pointer that did not move | low | by reading |
| 21 | Chords of four and five keys, and things with no key or no place to press | low | by reading |
| 22 | States the design language has no look for | low | by reading |
| 23 | Words | low | by reading |
| 24 | What is listed as not there yet leaves out | low | by reading |
| 25 | Documents that say more than is so | low | by reading |

Already on record, and not counted again: the shown tab of a frame
without focus, a question's buttons at 1.06 and 1.12 to 1 against their
box, and a critical notification marked by a red edge alone, all in
[finding 28 of 16 November](2026-11-16-the-whole-tree.md) ("smaller
things the user meets", low, open); a titlebar's buttons on cream and
without focus, and components' glass not made solid (findings 9 and 8 of
[19 November](2026-11-19-after-the-redesign-of-windows.md)); the
launcher's grid needing about 700 units (roadmap); `focus_notifications`
without a key (ACCESSIBILITY.md); the portal's question said in English
only (roadmap). Where a finding below leans on one of these it says so.
The three from finding 28 are measured again in finding 11, and are
better taken with it than left among the smaller things.

### What each criterion was looked at for

Levels A and AA of WCAG 2.2. The numbers are findings.

| Criterion | First pass | Second pass |
|---|---|---|
| 1.1.1 Non-text content | – | 6, 22; names for a screen reader are a known gap |
| 1.3.1, 1.3.2 Structure, sequence | – | 13; a known gap, with the screen reader |
| 1.3.3 Sensory characteristics | – | sound: no message names a control by colour, shape or side alone |
| 1.3.4 Orientation | – | not looked at beyond: a turned screen is drawn turned |
| 1.4.1 Use of colour | – | 11 |
| 1.4.3 Contrast | 1, 2 | 16, 17; the whole theme measured |
| 1.4.4 Resize text, 1.4.10 Reflow | named, not done | 10 |
| 1.4.11 Non-text contrast | 2 | 11 |
| 1.4.12 Text spacing | – | not applicable as written: the theme has no line or letter spacing to set |
| 1.4.13 Content on hover or focus | – | 6 |
| 2.1.1 Keyboard | – | 4, 6, 21 |
| 2.1.2 No keyboard trap | – | sound: every box that takes the keyboard gives it back at Escape |
| 2.1.4 Character key shortcuts | – | sound: no built-in binding is a letter alone |
| 2.2.1 Timing adjustable | – | 7 |
| 2.2.2 Pause, stop, hide | – | sound: nothing loops; the cursor blinks ten seconds and stops |
| 2.3.1 Three flashes | – | sound: nothing flashes; the shake is a box moved 8 units for 300 ms |
| 2.4.3 Focus order | – | sound where there is Tab; 14 where there is none |
| 2.4.6 Headings and labels | – | the lock screen's field has no label (roadmap, known) |
| 2.4.7 Focus visible | 2 | 3, 11, 14 |
| 2.4.11 Focus not obscured | – | 10: rows of a menu the keyboard is on, below the screen |
| 2.5.1 Pointer gestures | – | 21 |
| 2.5.2 Pointer cancellation | – | 5 |
| 2.5.7 Dragging movements | – | 4 |
| 2.5.8 Target size | taken as sound | 18 |
| 3.2.1 On focus | – | 20 |
| 3.2.3, 3.2.4 Consistency | – | 14 |
| 3.3.1, 3.3.3 Errors | – | 9, 23 |
| 3.3.4 Error prevention | – | 3, 7 |
| 3.3.8 Accessible authentication | – | 8 |
| 4.1.2 Name, role, value; 4.1.3 Status | known gap | 13; 6 and 11 for what is not said at all |
| 1.2.x, 1.4.2, 1.4.5, 2.4.1, 2.4.2, 2.4.4, 2.4.5, 2.5.3, 2.5.4, 3.1.x, 3.2.2, 3.2.6, 3.3.2, 3.3.7 | – | not applicable, or nothing found |

And of EN 301 549, which WCAG does not have: 5.2 turning accessibility
features on (12), 5.6 lock keys (9), 5.7 keys that repeat (sound: it can
be set, and turned off), 5.8 keys struck twice (24), 5.9 several keys at
once (21, and sticky keys, known), 11.5 working with assistive
technology (13), 11.7 the user's preferences told to programs (13, 24).

## Findings

### 3. A key meant for another program says yes to sharing the screen

**High. Shown by tests that are already there, and by reading.**
Security as much as accessibility: "sharing a window or screen without
the user's choice" is the first thing SECURITY.md counts. WCAG 3.3.4,
2.4.7.

The portal's question, before a picture of the screen is taken or the
screen is shared, goes up as a surface that asks for the keyboard to
itself (`crates/xdg-desktop-portal-omorfos/src/prompt.rs:681-682`). The
portal has the `overlay` role (`crates/omorfos/src/compositor.rs:5633`),
so it is given the keyboard at once, and has it for as long as the
question is up (`compositor.rs:3270-3284`, `demanding`). From then Enter
is `giving()`, Escape keeps, and nothing else is a key
(`prompt.rs:441-460`). The box is painted in two states, plain and the
button that gives filled with the accent, and no button is ever shown
with the keyboard on it (`prompt.rs:300-316`). With one screen, and a
program that asks for a screen and no window, there is nothing to pick,
and the share begins at that key
(`crates/xdg-desktop-portal-omorfos/src/lib.rs:483-490`).

So a program that is being typed into asks to share the screen, which
any program may, one in a sandbox through the portal, that being what
the question is for. The letters the user goes on typing are dropped,
and the first Enter is "Share". The program knows when Enter is coming:
it is what is being typed into. Someone who cannot see the box, or is
looking at their hands, has said yes without knowing that a question was
put: nothing is said to a screen reader, the box makes no sound, and
nothing on it shows where the keyboard is. The mark and the line round
the screen say afterwards that it is shared, and nothing on the desktop
stops it (a known gap). The same with the pointer: the buttons act as
the button goes down (finding 5), so a box that comes up under a pointer
about to press, where the program has drawn something to press, is
answered by that press.

The tests are there already, and pass.
`a_call_is_given_the_screen_to_read_once_the_user_says_so`
(`crates/omorfos/tests/integration/sharing.rs:710`) opens a window, has
a call ask for the screen, and presses Enter as soon as the question is
drawn: the answer is yes, and the screen is read.
`an_application_is_given_a_picture_once_the_user_says_so`
(`portal.rs:323`) types a letter first, which is passed over. Neither
moves the pointer, or presses anything else. The code guards one form of
this already, a key pressed on a box that has since been taken down: "it
would answer yes to an application they did not know asked"
(`prompt.rs:444-447`). A key pressed as the box comes up is the same
fault.

What to change, in `omorfos_ui::question`, so that every box that asks
has it (finding 14):

- the keyboard starts on the button that refuses, and is shown there;
  Enter presses the button the keyboard is on; Tab and the arrows move
  it. Saying yes takes a move and Enter, or a press;
- no key and no button counts until the box has been drawn and on the
  screen for a moment, half a second being usual, nor a key that was
  already down when it came;
- a press acts when it is let go on the button it went down on
  (finding 5).

Not small: it changes what Enter does in a box people have used, and is
a decision for [ADR 37](../adr/0037-who-may-do-what.md) or ADR 92.

The test to write first: a program asks to share; `hello` and Enter are
typed as the question comes up: nothing is shared, and the question is
still up with the keyboard on Cancel. And a press where "Share" is, in
the frame the box first appears in, is not taken.

![Now: a box asking to share the screen comes up over a window a message
is being typed into, with Share filled in; the next Enter typed begins
the share. Direction: the keyboard starts on Cancel, shown by a ring,
and Enter cancels. Below, the five boxes that ask and what Enter does in
each, and one rule for all of
them.](assets/2026-11-26-accessibility-questions.svg)

### 26. A box for a secret that is withdrawn hands the keyboard straight back

**High. Shown by a test, written, run and taken out again.** "Reading or
injecting input meant for another program" is the second thing
SECURITY.md counts. It was found by looking for finding 3 the other way
round.

While the sealed prompt is up it has the keyboard, and "no binding, no
other program and no input method sees a key"
(`crates/omorfos/src/sealed.rs:4-9`,
`crates/omorfos/src/compositor.rs:3287-3303`). The turn after it has
gone, the keyboard is given to whatever would have it otherwise, which
is the window with focus (`compositor.rs:3315-3337`): at once, with
nothing held back until the keys are let go or a moment has passed.
And a prompt goes not only when it is answered. polkit withdraws a
request that the program which made it has ended, and the box is taken
down (`crates/omorfos-ask/src/lib.rs:286-292`); the box for a passphrase
goes with the `gpg-agent` that asked (`lib.rs:299-305`).

So a program with the keyboard has polkit ask for the administrator's
password, as `pkexec` does, or as a service does on its behalf; waits
while some of it is typed; and ends its request. The box goes, the
program's window has the keyboard again, and what is typed next, the
rest of the password and Enter, is typed into it. Asked again with
another wait, it has another part. The dim and the blur going are the
only sign; someone who is not looking at the screen, or cannot see it,
has none. It needs no ill will either: where what asked only fails, the
end of a password is typed into a terminal, where Enter runs it as a
command and the shell keeps it in its history.

The test, to be put back in
`crates/omorfos/tests/integration/sealed.rs`. It fails today: the window
is told the keyboard is its again, and then Enter, pressed and let go.

```rust
#[test]
fn what_is_typed_as_a_prompt_is_withdrawn_does_not_reach_the_window_under_it() {
    let (mut session, mut app) = busy();
    let asking = ask(&mut session, Roles::ASK);
    session.roundtrip(&mut app);
    app.take_events();
    let tap = |session: &mut Session, code| {
        for pressed in [true, false] {
            session
                .compositor_mut()
                .input(Input::Key { code, pressed }, 0);
        }
    };
    // Some of a secret is typed into the prompt.
    tap(&mut session, key::A);
    // What asked gives up, as polkit does when the program that wanted
    // leave ends its request, and the prompt is taken down.
    answered(&mut session, asking);
    // The rest is typed before whoever types has seen it go.
    tap(&mut session, key::RETURN);
    session.roundtrip(&mut app);
    let told = app.take_events();
    assert!(
        !told.iter().any(|event| matches!(event, Event::Key { .. })),
        "the rest of the secret was typed into the window: {told:?}"
    );
}
```

What to change: when a box that had the keyboard to itself goes other
than by the user's own answer, the keyboard goes to nothing until every
key is up and a moment has passed, or until the pointer is pressed, and
what is typed meanwhile is dropped. Small in code; what it decides is
how long, and whether the same holds once a box has been answered.

### 1. Dim text on glass is under 4.5 to 1 at its worst

**Medium. Measured from the theme.** WCAG 1.4.3.

DESIGN.md asks 4.5 to 1 of "text on the surface it is on"
(`docs/DESIGN.md:152-154`), and then lets glass down to 3 to 1 over the
worst that can be behind it, a white window in the dark scheme and a
black one in the light (`:199-210`,
[ADR 97](../adr/0097-a-windows-frame-bulkier-glass-buttons-at-the-left.md));
the theme's test holds text to the same two figures
(`crates/omorfos-config/src/theme.rs:1798-1820`). So `--text-dim` on
`--glass-thick`, which is the surface at four fifths, is `#9aa4c7` on
`#43495f` over a white window, 3.6 to 1; in the light scheme `#5c6888`
on `#d9d7cf` over a black one, 3.8. A title without focus is 3.9 and
4.1. WCAG 2 asks 4.5 of text of this size whatever is behind it; 3 to 1
is what it asks of large text and of marks.

ADR 97 calls the worst ground rare. It is a browser's page under a menu,
a notification or the launcher in the dark scheme, and a terminal or a
film in the light one. And what is dim is not only "what is beside the
point", as the colour's own line has it: the body of a notification,
the keys beside a menu's rows, the heading over the programs the
launcher found, what a program says of itself there, what "About" says
under its heading, the title of every window without focus and of every
tab not shown. Only a question's detail is dim on a box that is solid,
at 6.8 and 5.4. `reduce_transparency` is what DESIGN.md offers whoever
needs more, which is a file to edit (finding 12).

What to change: text that is read meets 4.5 to 1 over the worst ground,
by one of three ways, or by more than one: a dim colour for glass that
is nearer the text's own; the glass thicker behind the words than round
them; or dim text kept for what truly is beside the point, with the body
of a notification in `--text`. The test then holds every text to 4.5 at
the worst, and the documents stop calling 3 to 1 a floor for text. It is
a weighing of how much of what is behind shows against how well the
words are read, and ADR 97 is where it was last weighed.

The test to write first: in `meets_contrast`, each text over the worst
ground at 4.5 to 1. It fails today on the dim text of every part on
glass.

![Now: a panel of glass over a bright window, its second and third lines
in dim text at 3.6 to 1, and round its button a ring that is hard to see
where an accent of one's own is near the surface's colour. Direction:
the same glass, the dim text in a colour that keeps 4.9 to 1, and a ring
of two colours that shows whatever the accent
is.](assets/2026-11-26-accessibility-glass-before-after.svg)

### 2. An accent of one's own can take away the focus ring and every lit row

**Medium. Measured from the theme, for eight colours.** WCAG 1.4.11,
2.4.7, 1.4.3.

`appearance.accent` is one of six that come with omorfos, "or any
colour" (`docs/DESIGN.md:351`), used as it is in both schemes
(`crates/omorfos-config/src/scheme.rs:60-61, 86`). The accent is the
ring round what the keyboard is on
(`crates/omorfos-config/src/default.css:439-445`), the fill of what is
lit in a menu, the launcher and the month, and the edge of the field and
of the notification that have the keyboard (`default.css:218, 399,
453`). One near the surface's own colour leaves none of them: `#141c38`
in the dark scheme and `#fffdf4` in the light are 1.0 to 1 against the
box they ring, and `#fde047`, the brand's own yellow, 1.3 on cream. What
is written on an accent of one's own is whichever of the scheme's two
colours stands out more (`scheme.rs:94-112`), with no least: on
`#777777` it is 4.1 to 1 in the dark scheme and 4.4 in the light, and on
GNOME's blue, `#3584e4`, 4.9.

What to change: the accent stays the user's, and what says where the
keyboard is stops resting on it alone. Either the ring is drawn in two
colours, the accent and one that stands out from the surface whatever
the accent is, as a ring of two lines does; or an accent under 3 to 1
against the surface is taken with a word in the notice of what is wrong
and the ring falls back to the text's colour. What is written on an
accent is black or white where neither of the scheme's two reaches 4.5.

The test to write first: `#777777`, and an accent equal to each scheme's
surface: the ring is 3 to 1 against the box, and what is written on a
lit row 4.5, or the notice says the accent was not taken.

The picture is with finding 1.

### 4. A window is moved and resized only by dragging

**Medium. By reading.** WCAG 2.1.1, 2.5.7. ACCESSIBILITY.md says "every
action of the desktop can be bound to a key", which is so of the actions
there are; these have none.

Nothing in `Action` moves a window, resizes one, opens its menu, puts
windows together as tabs or moves the line between tiles
(`crates/omorfos-config/src/action.rs:74-253`), and every way into them
is a button held (`crates/omorfos/src/input.rs:896-926`, `press_frame`).
The edge a window is resized by is 6 units outside the frame, 24 at a
corner (`crates/omorfos-core/src/decoration.rs:9-15`); the line between
tiles 4 either side (`crates/omorfos/src/tiles.rs:36-38`). Tabs are put
together and taken apart from a drag alone (`input.rs:1507-1508`), a
region is a press, a move and a release
(`crates/omorfos-picker/src/area.rs:159-164`), the notice of what is
wrong goes at a click (`input.rs:1375-1380`), and the bar takes no
keyboard (`crates/omorfos-panel/src/panel.rs:920-921, 970-975`).

| What | With a pointer | Without dragging | With keys |
|---|---|---|---|
| Move a window | drag the titlebar, or Alt and drag anywhere | no | no |
| Resize a window | drag an edge 7 units thick, or Alt and right-drag | no | no |
| The line between tiled windows | drag a strip 9 units thick; a double press puts it back | no | no |
| Put windows together as tabs, and take one out | the middle button, dragged | no | no |
| A region to take a picture of, or to share | drag | no | no |
| A window's menu | a right press on its titlebar | yes | no: there is no action |
| The root menu | a right press on bare desktop | yes, where any is bare | `show_menu`, which has no key |
| Thirds and quarters of a screen | rest on the third button 400 ms, press a picture | yes | halves only, twice over |
| The notice of what is wrong | a press on it | yes | no key puts it away |
| The bar: a window, the month, "Awake" | a press | yes | no |
| A colour from the screen (portal) | the pointer chooses, a press takes | yes | the pointer still chooses |
| Leave a full screen, in the ordinary way | push at the top edge, then the button | yes | no key is bound (`crates/omorfos-config/src/keys.rs:254-278`) |
| Show the desktop | four fingers spread | no | `show_desktop`, which has no key |

Alt and a drag makes the whole window the handle, which is the right
answer for a hand that cannot find 7 units; it is not said in
ACCESSIBILITY.md, is not a setting, and in the Mac's way sits on Alt
where the rest sits on Super.

What to change: three actions, `move`, `resize` and `window_menu`, the
first two entering a mode the arrows drive (Shift for a larger step,
Enter keeps, Escape puts back), with rows for them in a window's menu,
as GNOME's Alt+F7 and Alt+F8 are; `grow` and `shrink` for a tile's line;
`attach` and `detach` for tabs; the arrows moving a region's corner in
the picker; a key for the notice. The roadmap's "everything in the shell
reachable from the keyboard" becomes this table, ticked a row at a time.
Not small: each is an action, a row of the guide and a line of the
schemas.

The test to write first: a window moved 40 units by `move`, Right four
times and Enter, with no pointer event sent.

![Now: a window's menu with no row to move or resize it, and the edge of
a window, 7 units thick, beside a circle of 24. Direction: the menu with
Move and Resize and their keys, and a window outlined with handles,
resized by the arrows.](assets/2026-11-26-accessibility-without-dragging.svg)

### 5. Nearly everything acts as the button goes down

**Medium. By reading.** WCAG 2.5.2, level A.

A menu does what its row does at the press (`crates/omorfos/src/input.rs:1383-1396`:
`if pressed { menu.press(position) }`), where a titlebar's buttons, a
few lines on, do "what it does if the pointer is let go on the button it
was pressed on" (`input.rs:1407-1413`). The same at the press: a window
on the bar (`crates/omorfos-panel/src/panel.rs:970-975`), a program in
the launcher (`crates/omorfos-launcher/src/client.rs:872-879`), a
notification (`crates/omorfos-notifications/src/client.rs:750-760`), the
lock screen's buttons (`crates/omorfos-lock/src/screen.rs:1259-1265`),
the box that asks for a password (`crates/omorfos-ask/src/prompt.rs:1114-1118`),
the portal's question (`crates/xdg-desktop-portal-omorfos/src/prompt.rs:1046-1050`),
the question before leaving (`crates/omorfos/src/leaving.rs:182-201`),
the overview (`overview.rs:626-651`) and the switcher
(`switcher.rs:878-895`).

A hand that shakes, a touchpad tapped by a palm, or a press that landed
one row off cannot be taken back by sliding away, as it can on a
titlebar's buttons and in every toolkit a user knows. A notification
pressed by mistake is gone for good, there being nowhere to read it
again. The theme has no look for "pressed", so nothing shows that a
press landed before it is acted on. And a box that comes up under a
pointer about to press is answered by that press, which for the portal's
question is finding 3.

What to change: one rule where presses are taken, in `omorfos_ui` and in
the compositor's own for its menus: the press arms what is under it,
drawn pressed; letting go acts if the pointer is still on it; moved off,
nothing happens. And a state for it in the theme: `:active` is taken
here for "in use", so it needs a name, which
[ADR 105](../adr/0105-a-titlebars-buttons-in-their-window-controls.md)
has already had to think about. Not small: every component, and a state
of the theme.

The test to write first: a press on a row of a menu, the pointer moved
off the menu, the button let go: the menu is still open and nothing was
performed.

![Now: the button goes down on Sleep, and the machine sleeps at once.
Direction: going down, the row is drawn pressed; slid away and let go,
nothing happens; let go on the row, it is
done.](assets/2026-11-26-accessibility-press-and-release.svg)

### 6. Which program is taking pictures is told only to a pointer at rest

**Medium. By reading.** WCAG 1.4.13, 2.1.1, 4.1.3. It is the one piece
of what is private that a person who cannot use a pointer is not told.

The mark at the corner of a screen says that something is captured, and
of what kind. Which program, it says "when the pointer rests on it", and
only then (`crates/omorfos/src/compositor.rs:3565-3566, 3631-3675`), in
a tooltip drawn 4 units below the mark, so that the pointer cannot go
onto it. The mark is 16 units tall, and 24 wide for one sign
(`crates/omorfos/src/paint.rs:1141-1149`). The state a script or a
panel reads has no field for it
(`crates/omorfos-ipc/src/message.rs:178-212`). There is no key for it,
no row of a menu, and no notice when it begins.

The same holds of the rest a tooltip says: the date, the whole of a
title cut short on the bar, and what a click on "Awake" does
(`crates/omorfos-panel/src/bar.rs:264-275`). A component's tooltip
comes after half a second (`crates/omorfos-shell/src/tooltip.rs:12-13`)
and "nowhere on it takes the pointer" (`popover.rs:150-153`): it goes as
the pointer leaves what it is of, which matters to anyone who reads
through a magnifier and must bring the pointer to the words, and none
goes at Escape.

What to change: what is captured, by which program and since when, in
the IPC state; the mark opens a list at a press and at a key, each line
with a way to stop it, which the roadmap has among the known gaps of
sharing ("the user cannot stop sharing from the desktop"); a tooltip
stays while the pointer is on it and goes at Escape; and whatever has a
tooltip shows it while the keyboard is on it, as the eye in the password
box already does (`crates/omorfos-ask/src/prompt.rs:377`). Not small.

The test to write first: with a screen being captured, `omorfosctl
state` names the program; and a key opens the list with no pointer event
sent.

![Now: a small mark at the corner of a shared screen, a tooltip that
names the program only while the pointer rests on the mark, and a state
that does not name it. Direction: the mark as a button with a ring,
opening a list of what is captured, by which program and since when,
each with Stop, and the state naming
it.](assets/2026-11-26-accessibility-who-is-watching.svg)

### 7. Times that cannot be changed, and one that says yes by itself

**Medium. By reading.** WCAG 2.2.1, level A: a time limit can be turned
off, set over a range of ten times, or put off when it is about to end.

| What | How long | When it is up | Can it be changed | Does being there hold it |
|---|---|---|---|---|
| Restart, shut down, log out asked (`crates/omorfos/src/leaving.rs:27-28, 208-213`) | 60 s | **it is done** | no; `power.ask: false` takes the question away | no |
| "Keep the screens like this?" (`standing.rs:25`) | 15 s | put back | no | no; and only Enter and Escape answer, there being no buttons |
| The lock screen left alone (`crates/omorfos-lock/src/screen.rs:81-83, 654`) | 30 s | the field is emptied | no | a key or a button, not the pointer moving |
| Volume and brightness shown (`crates/omorfos-session/src/shown.rs:15`) | 1.5 s | gone | no: it is the sender's own time | no |
| A notification with its sender's time (`crates/omorfos-notifications/src/note.rs:264-268`) | as sent, with no bounds | gone, kept nowhere | no: it beats `notifications.timeout: 0` | no |
| Any notification (`note.rs:433, 463-470`) | 5 s | gone, kept nowhere | `notifications.timeout`, 0 to 3600 | no: the pointer on it does not hold it |
| Two presses as one (`crates/omorfos/src/input.rs:276-278`) | 400 ms, 4 units | two presses | no | – |
| A full screen's bar and titlebar (`reveal.rs:33-55`) | rest 600 ms, or push 100 units in a second; gone after 1 s | – | no | the pointer on them |
| A tooltip (`crates/omorfos-shell/src/tooltip.rs:12-13`) | 500 ms | – | no (known) | – |

The first is the one that matters: a question raised by a press that
slipped (finding 5) ends the session a minute later if the person has
not found the box, read it and reached Escape.
[ADR 88](../adr/0088-restart-and-shut-down-ask-first.md) gives the
reason ("whoever walked away from a machine being turned off meant it to
go off"), and
[ADR 101](../adr/0101-super-drives-the-desktop-and-the-ordinary-way-is-a-setting.md)
kept the minute fixed. The lock screen's own form of the same question
does the opposite: no count, the keyboard on Cancel, and forgotten after
thirty seconds.

What to change: the minute a setting (`power.wait`, 0 for no count), and
any key or movement of the pointer starting it again; the same for the
screens' fifteen seconds, with buttons in the box; a notification held
while the pointer or the keyboard is on it; the user's "until clicked"
over a sender's time, or at the least a shortest time; the level shown
as long as `notifications.timeout` where that is longer; and
`mouse.double_click` a setting, as every desktop has it. Not small: four
settings, and ADR 88 and ADR 101 to say so.

The test to write first: with the question before leaving up, a key
other than Enter at 59 seconds; at 61 the session is still there.

![Now: the question before restarting with the keyboard on Restart and a
count that restarts at nought; the screens' question with no buttons; a
notification that goes at five seconds under the pointer. Direction: the
keyboard on Cancel and a count a hand starts again; buttons on the
screens' question; a notification held while it is pointed
at.](assets/2026-11-26-accessibility-time.svg)

### 8. A passphrase cannot be pasted, nor anything looked up while it is asked

**Medium. By reading.** WCAG 3.3.8, level AA and new in 2.2: no step of
signing in rests on memory or on copying by eye unless there is another
way, or something to help, and it names "copy and paste" and password
managers as the help.

Paste is among what does nothing in the box that asks
(`crates/omorfos-ask/src/prompt.rs:650-662`: "nothing to copy or undo")
and on the lock screen (`crates/omorfos-lock/src/screen.rs:819-831`);
[ADR 82](../adr/0082-the-compositor-copies-for-the-launcher.md) has the
fields that ask for a secret "take no paste, as they give nothing to
copy" (`:43-44`). And while the box is up no other program has a key or
the pointer (`crates/omorfos/src/sealed.rs:6-8`).

The reason given is about copying out. Pasting in is another thing, and
gives nothing away. The password of the account is remembered; what
`omorfos-pinentry` asks for is the passphrase of a GnuPG or SSH key,
which is often long, made by a program and kept in a password manager.
With the box up the manager cannot be looked at, and what it holds
cannot be pasted: the passphrase is copied onto paper, or held in the
head. A finger is another way for the account where PAM is set up for
one; for a key there is none.

What to change: Paste taken by the box that asks, its first line only,
as the launcher takes it (the clipboard is already offered to whatever
has the keyboard, `crates/omorfos/src/handlers.rs:623-632`); nothing
copied out, as now. The lock screen can stay as it is: nothing can be
copied behind it. Small in code; a decision, as ADR 82 and
[ADR 48](../adr/0048-asking-for-a-secret.md) say otherwise.

The test to write first: text on the clipboard, the box up, Super+V: the
field holds as many dots as the text has characters.

The picture is with finding 9.

### 9. The lock screen does not say why a password failed

**Medium. By reading.** WCAG 3.3.1, 3.3.3; EN 301 549 5.6.2. With
`pam_faillock` counting, three such failures lock the account.

Each of these is, by itself, a wrong password with "That was not right"
as all that is said:

- **Caps Lock is on.** Only Ctrl, Alt, Logo and Shift are read of the
  keyboard's state (`crates/omorfos-shell/src/typing.rs:162-167`), so
  nothing says so, on the lock screen or in either box.
- **The layout is not the one the password is in.** Nothing painted says
  which it is (`crates/omorfos-lock/src/screen.rs:871-876, 987-1031`),
  and no binding is acted on while locked
  (`crates/omorfos/src/lock.rs:236-237`), so `next_layout` on its key
  does nothing.
- **The first key woke the screen and was not typed**
  (`screen.rs:756-762`; `docs/adr/0052-the-lock-screen.md:109-111`:
  "whoever starts typing at once loses the first character, and finds
  out from a wrong password").
- **A key bounced.** The lock screen has no way to look at what was
  typed; the boxes have the eye.
- **Two typings of a new passphrase differ**: "GnuPG is told nothing was
  given" (`crates/omorfos-ask/src/pinentry.rs:239-241`), the box says
  nothing, and what was being done is given up.
- In the boxes, what went wrong is one line in `question-detail`, the
  dim text (`crates/omorfos-ask/src/prompt.rs:298, 374-375`), with no
  sign; and only polkit's path shakes
  (`crates/omorfos-ask/src/lib.rs:538`), so with motion reduced, or for
  a key, nothing draws the eye to it.

What to change: Caps Lock said in words in every field for a secret; the
layout's name beside the field where there is more than one, changed by
a press on it and by the layout's key while locked; the first key typed
as well as waking, or the field said in words to be asleep; the eye on
the lock screen as in the boxes; "The two were not the same" in the box;
and what went wrong written in `--danger`, which is 5.5 and 5.4 to 1 on
the box, after a sign. Not small; the first key is
[ADR 52](../adr/0052-the-lock-screen.md)'s decision.

The test to write first: Caps Lock on, a key typed on the lock screen:
the words say so.

![Now: the lock screen says only that the password was not right, and a
box for a passphrase takes no paste. Direction: the lock screen says
Caps Lock is on, shows the layout and an eye, and writes what went wrong
in red after a sign; the box for a passphrase takes a
paste.](assets/2026-11-26-accessibility-secrets.svg)

### 10. Larger text is cut, and a larger scale runs off the screen

**Medium. Computed from the constants, the root menu laid out by
`omorfos_core::Menu` itself; not run.** WCAG 1.4.4, 1.4.10, 2.4.11.
ACCESSIBILITY.md offers both as the ways to see better.

**The text.** `font-size` may be 6pt to 72pt. A line of text is five
thirds of its points tall (`crates/omorfos/src/paint.rs:818-821`,
`crates/omorfos-notifications/src/note.rs:735-738`), and only what falls
inside a box is drawn (`crates/omorfos-ui/src/canvas.rs:1154-1163`).

| Row | Tall | About the largest text that fits | Where the height is |
|---|---|---|---|
| A question's lines, the portal's and the one before leaving | 20 | 12pt | `crates/omorfos-ui/src/question.rs:12-14` |
| "About" | 20 | 12pt | `crates/omorfos/src/paint.rs:472-473` |
| A tooltip, the notice, an arrangement's caption, the picker's help | 22 | 13pt | `crates/omorfos-ui/src/tooltip.rs:10-11`, `paint.rs:416-417`, `palette.rs:30-32`, `crates/omorfos-picker/src/lib.rs:55-58` |
| The bar | 28 | 16pt | the theme |
| A menu's row, a day of the month | 32 | 19pt | the theme |
| The box that asks for a password | fixed throughout | 11pt as drawn | `crates/omorfos-ask/src/prompt.rs:39-58` |
| The lock screen | 64pt and 20pt whatever the theme says | – | `crates/omorfos-lock/src/screen.rs:50-66` |

So text one point larger than the built-in 11 is already cut in the
boxes that ask, and twice as large is cut everywhere but a titlebar. A
theme can raise the heights it owns, to 64 at most; the rows in code it
cannot. Nothing tells programs to make their own text larger
(`text-scaling-factor` is not served; known, with "larger text" as a
setting).

**The scale.** `output.scale` goes to 4. At 2 on a laptop of 1366 by
768, which is 683 by 384 units:

| What | Needs | Then |
|---|---|---|
| The root menu | 436 tall | starts at the top and does not scroll: "Restart…" is 19 of its 32 units below the screen and "Shut down…" wholly so; the arrows still go to them (`crates/omorfos-core/src/menu.rs:455-460`) |
| The welcome card | 600 by 432 | Skip, Back and Next are at 383 to 419, below the screen; Enter and Escape still work (`crates/omorfos-welcome/src/card.rs:16-21, 216-253`) |
| The grid of every program | 698 wide | 15 units off the right (`crates/omorfos-launcher/src/picker.rs:29-30, 156-164`; known as a gap, not as this) |
| The lock screen | 324, and the buttons' 60 | what a way of unlocking says under the field is drawn over the power buttons (`crates/omorfos-lock/src/screen.rs:893-898`) |

At 2.5 on 1920 by 1080 the menu's last row is cut; at 3, three rows are
gone, with all of the above. Whoever enlarges the most has the smallest
screen, in units.

And what is cut has nowhere to be read whole: a title on a titlebar or
in a menu, a notification's body past three lines (`note.rs:17-22`), and
the one line that says what a program asking for the screen would see,
which is not wrapped
(`crates/xdg-desktop-portal-omorfos/src/prompt.rs:271-274`) and will be
cut in a language longer than English.

What to change: a row as tall as its text needs, from one function
(`line_height` is in three places already); a menu taller than its
screen scrolls, with a sign that there is more; the card and the lock
screen laid out in the height they are given; the grid as many across as
fit; a question's words wrapped. Not small.

The test to write first: the root menu opened on a screen 384 units
tall: every row is on it, or it scrolls to the row the keyboard is on.
Then one for each part at 384 units tall, and at 22pt.

![Now: on a screen of 683 by 384 units the root menu's last two rows are
below the screen, and a question's text at 18pt is cut top and bottom
and cut short. Direction: the menu scrolls to the row the keyboard is
on, and the question's rows grow and its words
wrap.](assets/2026-11-26-accessibility-larger.svg)

### 11. States said by marks under 3 to 1, and one not said at all

**Medium. Measured from the theme; the last row by reading.** WCAG
1.4.11, 1.4.1. DESIGN.md: "anything that says a state without words: at
least 3 to 1", and "colour is never the only sign".

| State | How it is said | Dark | Light | On record |
|---|---|---|---|---|
| The tab shown, in a frame without focus | an underline in `--border`; its title as dim as the rest | 1.1 to 1.7 | 1.0 to 1.2 | 16 Nov, finding 28 |
| A row of a menu that is on (`menu-sign:active`) | the sign in the accent where it was the text's colour, on a square of a fifth of the accent | sign against sign 1.5, and 1.5 to 2.3 over the six accents; the square 1.4 to 1.5 | 3.5; the square 1.3 | new |
| What Enter takes in the launcher | on `main`, nothing but its place, alone above the heading; **#177**, one unit of the accent at a fifth round it (`launcher-top`) | 1.4 to 1.5 | 1.3 | new |
| How loud, how bright (`notification-level`) | a bar 6 units tall on a track in `--border`; no number; 1.5 s | the track 1.2 to 1.6 | 1.1 to 1.3 | new |
| The keyboard on a notification | its edge of one unit turns the accent | 4.9, a unit thick | 3.7 | new |
| A critical notification | its edge of one unit turns red | 2.9 to 5.9 | 3.8 to 5.4 | 16 Nov, finding 28 |
| Where a dragged window would land | a wash of the edge's colour at 31%, no line round it (`crates/omorfos/src/tiles.rs:44, 292-301`) | white on a white window: nothing | navy on a dark window: nothing | 19 Nov, finding 10, "its colour is left" |
| A window that wants attention | **nothing** | – | – | new |

The theme's test holds the sign that is on to 3 to 1 against its square
(`crates/omorfos-config/src/theme.rs:1855-1859`), which says the sign
can be seen, not that on can be told from off.
[ADR 70](../adr/0070-signs-and-pictures-in-menus.md) weighed a tick and
a switch for it and left both; that is the decision to look at again.

The last row: DESIGN.md says "what is urgent has a mark as well as a red
edge", and [ADR 37](../adr/0037-who-may-do-what.md) that a window
refused the keyboard "is marked as wanting attention". No state for it
is in the desktop model or on the bar, and the X11 hint is not read
(`crates/omorfos/src/handlers.rs:795-803`, `x11.rs:1032-1048`). A
program that needs its user says so to nobody.

What to change: the picture has one way for each. And a rule for the
theme's test: for each state said without words, the mark that says it
against what is beside it, and against itself in the other state. Each
is small; together they are a pass over the built-in theme.

The test to write first: in `meets_contrast`, `menu-sign` on against
`menu-sign` off, 3 to 1; then each row above.

![Now: the shown tab of a frame without focus, a row of a menu that is
on, the launcher's top hit as pull request 177 draws it, the bar that
shows the volume and a critical notification, each told by a mark
between 1.1 and 2.9 to 1. Direction: each by a mark of 3 to 1 or more,
or by a shape: a tick, a number, a
sign.](assets/2026-11-26-accessibility-state-marks.svg)

### 12. Accessibility settings are turned on only by editing a file

**Medium. By reading.** EN 301 549 5.2: an accessibility feature can be
turned on without needing what it is there to give.

No action sets the scheme, the scale, the pointer's size, motion or
transparency (`crates/omorfos-config/src/action.rs:74-253`);
`omorfosctl` has no `set` (`crates/omorfosctl/src/cli.rs:19-56`); the
welcome card offers a scheme and a wallpaper
(`crates/omorfos-welcome/src/card.rs:100-107`); and "Settings" in the
menu says there are none yet
(`crates/omorfos/src/compositor.rs:947-972`).

So to have the screen at twice the size, a person reads the guide to
`config.yaml` and edits a file, at the size they cannot read. The same
for reduced motion, reduced transparency and the pointer. Nothing on the
lock screen or the welcome card, the two places a person meets first,
offers any of it.

What to change, before there is Settings: `set` in `omorfosctl`, through
`omorfos_config::edit`, which the welcome card already uses; actions
that step the scale and turn each setting on and off; and one menu of
them, "Accessibility", opened by a key and from the bar, the root menu,
the welcome card's first step and the lock screen. Not small: a new
menu, and a decision on its key.

The test to write first: `omorfosctl set appearance.reduce_motion true`,
and the theme is still, with the rest of the file as it was.

![Now: a terminal with config.yaml open, to reduce motion, raise the
scale and enlarge the pointer. Direction: a menu named Accessibility,
with steps for the scale, the text and the pointer, and switches for
reduced motion, reduced transparency, high contrast, a screen reader and
sticky keys, those not built marked as to
come.](assets/2026-11-26-accessibility-menu.svg)

### 13. Assistive technology has no place in the model of roles yet

**Medium. By reading.** EN 301 549 11.5; WCAG 4.1.2. The screen reader
is a known gap. What is new is that the plan for it is one item where it
is three, and that the model of roles would refuse the tools.

There are eight roles, none for a tool that reads or drives the session
(`crates/omorfos/src/roles.rs:26-48, 80-95`;
`docs/adr/0037-who-may-do-what.md:75-84`). No virtual pointer is
offered, no touch and no tablet
(`crates/omorfos-ipc/src/protocols.rs:39-351`). Nothing of AT-SPI is in
the session's environment (`crates/omorfos/src/compositor.rs:2643-2670`),
and the packages do not recommend Orca (`packaging/aur/PKGBUILD:18-30`).
The state says nothing of captures, the layout, lock keys, notifications
or a window that wants attention
(`crates/omorfos-ipc/src/message.rs:178-212`).

What it is, apart:

1. **Programs that already speak AT-SPI, read by Orca.** Cheap, and not
   started. It needs the interface Orca asks a compositor for its keys
   by, `org.freedesktop.a11y.KeyboardMonitor` on the name
   `org.freedesktop.a11y.Manager`, which Mutter, KWin and niri serve;
   niri is Smithay and Rust as omorfos is, and says what it took
   ([its page](https://niri-wm.github.io/niri/Accessibility.html)). And
   the key that starts Orca, which the roadmap has. And it tried once.
2. **What omorfos draws, said aloud.** niri began with a few sentences
   through AccessKit, not a toolkit's tree: the workspace gone to, the
   title of the window the switcher is on, a box that asks, a mistake in
   the configuration. That order would give a blind user the lock
   screen, the questions and the switcher long before stage 7.
3. **Tools that drive the pointer and the keys**: a press by resting, a
   switch, pointers moved by the eyes or the head, a voice. They need a
   virtual pointer, which `input` is said to give ("type and click as
   the user", ADR 37) and does not, and they need to be seen over the
   lock screen and the sealed prompt, which nothing of another program's
   is.

The seal is the hard part: the prompt that no program may imitate or
watch must still be read aloud, and typed into, by a tool. A virtual
keyboard of the `input` role can type into it already, Smithay handing
its keys to whatever has the keyboard, as it types into the lock screen
by [ADR 47](../adr/0047-input-methods.md)'s own words ("typing as the
user is what the role is"); no test holds either, either way.

What to change: an ADR, before stage 5 fixes how the bar is reached: a
role for assistive tools, what it may read and drive, what it may do
while locked and while sealed, and the three parts above in that order.
A decision first.

The test to write first, for the first part: a GTK program's button
named by a client of AT-SPI in a test session.

### 14. The boxes that ask do not answer alike

**Medium. By reading.** WCAG 3.2.4;
[ADR 92](../adr/0092-one-button-and-one-box-that-asks.md) set out to
make them one.

| Box | Enter | Tab | A ring on what the keyboard is on | Buttons | Left alone |
|---|---|---|---|---|---|
| Before leaving: restart, shut down, log out (`crates/omorfos/src/leaving.rs:166-213, 312-319`) | **yes** | no | on the yes | yes | **it is done**, at 60 s |
| The same, asked on the lock screen (`crates/omorfos-lock/src/power.rs:164-215`) | **no**: the keyboard is on Cancel | yes | yes | yes | forgotten at 30 s |
| The portal's: share, take a picture (`crates/xdg-desktop-portal-omorfos/src/prompt.rs:441-460`) | **yes** | no | none | yes | waits |
| The screens' (`crates/omorfos/src/input.rs:597-610`, `standing.rs`) | keeps them | – | – | none at all | puts back at 15 s |
| A password (`crates/omorfos-ask/src/prompt.rs:611-703`) | hands it in | yes | yes | yes | waits |

The same question, "Restart?", is answered yes by Enter in the session
and no on the lock screen.

What to change: one rule in `omorfos_ui::question`, where ADR 92 put the
drawing: the keyboard starts on the answer that changes nothing, every
box takes Tab and shows the ring, a box with a count says what it will
do and when, and every box has buttons a pointer can press. To be taken
with finding 3, which is in the same code.

The test to write first: the question before leaving up, Enter: the
session is still there, and the keyboard was on Cancel.

The picture is with finding 3; the one with finding 7 draws two of the
boxes by the one rule.

### 15. The contrast test is a list kept by hand, and the checklist does not ask

**Medium. By reading.** It is why most of the above was not caught.

`written` lists, one by one, the texts held to 4.5 to 1
(`crates/omorfos-config/src/theme.rs:1725-1791`), and `meets_contrast`
six edges (`:1834-1860`). Of the 58 parts, the first list has no
`launcher-tile`, `notification-title`, `popover-item`, `popover-day`,
`popover-day-outside`, `offer` or `overview-title`, and the second none
of `capture`, `notification-level` or a notification's edge, each of
which says a state by a mark. Each was added after the test was written,
and nothing made its author add it; `popover-day-outside` fails on a
solid surface (finding 16). **#177** adds four more that are written on,
`launcher-top`, `launcher-heading`, `launcher-mode` and
`launcher-mode-key`, and the list has none of them: the last, a mode's
key in its chip, is 4.45 to 1 over the ordinary ground in the dark
scheme, which the test would refuse.

"What a new part must answer" (`docs/DESIGN.md:358-368`) has seven
questions. They ask of the keyboard and of the three settings, and
nothing of time, of a pointer that cannot drag or rest, of text twice as
large, of what is said without words, or of what a screen reader is
told.

What to change:

- each `Part` says in the code what it is written on, as it says its
  name, so that a part cannot be added without it, and the test walks
  `Part::ALL`; the same for "says a state without words";
- DESIGN.md's list grows by: Is any state of it said by colour alone, or
  by a mark under 3 to 1? Does anything in it go by itself, and can that
  be held? Can all of it be done by one press, without dragging, resting
  or a second finger? Does a press on it act when it is let go? What
  does it do at twice the text, and on a screen 384 units tall? What is
  a screen reader told of it, and when?

The test is the change of a day; the list is words.

The test to write first: the one that walks, which fails today on
`popover-day-outside`.

### 16. The faint colour is used for text

**Low. Measured from the theme.** WCAG 1.4.3.

The days of the months beside the one shown are numbers, written in
`--text-faint` (`crates/omorfos-config/src/default.css:313`): 4.0 to 1
on a solid surface in the dark scheme and 2.95 in the light, 2.1 on
glass at its worst, and 2.9 and 3.0 under the pointer. DESIGN.md calls
the colour "only there to fill, never to be read"
(`docs/DESIGN.md:119`), and says of its contrast that the rules "hold
for every pair in the tables above" (`:149-150`), of which this is one.
A date is read.

What to change: the days in `--text-dim`, told from the month's own by
weight or by having no box; or the colour kept, and the rule saying
plainly that it is for what is not text.

The picture is with finding 22.

### 17. An offered window's title is written straight onto the wallpaper

**Low. By reading.** WCAG 1.4.3.

The title of each window offered in an empty part of a tiled workspace
is `--text-dim` on whatever the wallpaper is there: `offer_title` draws
the words and nothing behind them
(`crates/omorfos/src/paint.rs:775-797`,
`crates/omorfos-config/src/default.css:121-126`). On Prism's spill, or a
photograph, it is as readable as the picture lets it be, which is what
the design language says glass is for. The overview's title has a ground
of its own (`overview-title`, solid).

What to change: the same ground under an offer's title.

The picture is with finding 22.

### 18. Four things are smaller than 24 by 24

**Low. By reading.** WCAG 2.5.8; DESIGN.md: "nothing that can be clicked
is smaller than 24 by 24, however small it is drawn".

| What | Size, in units | Where |
|---|---|---|
| A part of an arrangement's picture | a quarter is 21 by 14, a half 21 by 30 or 44 by 14 | `crates/omorfos/src/palette.rs:23-27` |
| A window's edge, to resize it by | 7 thick, 6 at the top; 1 where the frame is at the screen's edge | `crates/omorfos-core/src/decoration.rs:9-12` |
| The line between tiled windows | 9 thick | `crates/omorfos/src/tiles.rs:36-38` |
| The mark that says what is captured, to rest on | 24 by 16 | `crates/omorfos/src/paint.rs:1141-1149` |

What to change: pictures of 72 by 48, so that a quarter is 35 by 23 and
a half more; the edges are met by finding 4, and the mark by finding 6.

The picture is with finding 22.

### 19. Reduced motion leaves two things moving

**Low. By reading; the second is to be shown.** DESIGN.md: "with
`reduce_motion`, things that move change opacity only".

On the lock screen, `move_from_here` asks only whether the theme's time
is more than nothing, and with motion reduced the theme's time is 120
ms, not nothing (`crates/omorfos-lock/src/screen.rs:683-691`): the time
still travels from the middle of the screen to its place, and what is
under it rises 16 units (`:902, :915`). The windows set aside to show
the desktop glide back over 240 ms: `unspread` has no such check where
line 85 has one (`crates/omorfos/src/aside.rs:111-135`).

And ACCESSIBILITY.md says `* { transition: none; }` turns off "every
change of colour, fade and movement": the slide between workspaces, the
switcher's turn and the overview read no theme
(`crates/omorfos/src/slide.rs:92-94`).

The test to write first: motion reduced, a key on the lock screen: the
time is drawn in its place in the next frame.

### 20. What Enter starts follows a pointer that did not move

**Low. By reading.** WCAG 3.2.1.

The launcher grows as what is found comes. A pointer resting under where
it grows is told that it has entered, which the launcher takes as its
having moved (`crates/omorfos-launcher/src/client.rs:857-866`); the row
under it is lit, and "over a program, the programs have the keyboard"
(`picker.rs:529-535`), so Enter now starts that program and not the
first. The compositor tells a still pointer when something comes under
it (`crates/omorfos/src/input.rs:985-1013`), as it should. Typing puts
it right, so it takes the last key to be the one that grew the list.
Someone whose pointer follows their eyes or their head has it at every
glance.

What to change: a row is lit by the pointer once the pointer has moved a
few units since the launcher last changed size.

### 21. Chords of four and five keys, and things with no key or no place to press

**Low. By reading** (`crates/omorfos-config/src/keys.rs:176-320`). EN
301 549 5.9; WCAG 2.5.1. Sticky keys are a known gap, which this makes
sharper.

- Going to the window beside is Super+Ctrl+Alt and an arrow; changing
  places with it adds Shift: five keys held. Of the built-in chords, 27
  hold three keys, 8 hold four and 4 hold five.
- `show_desktop` has no key and no row of a menu in either way: only
  four fingers do it. The overview and the next workspace have keys and
  no place to press.
- In the ordinary way nothing is bound to `fullscreen`, to `minimize`,
  to a workspace by its number or to a new one.
- A chain of keys can stand in for any chord and has no time limit,
  which is right, and shows nothing while it waits for its next key
  (`crates/omorfos/src/input.rs:676`).

What to change: with the menu of finding 12 and the bar of stage 5, a
place to press for each; and a chain offered beside each chord of four
keys or more.

### 22. States the design language has no look for

**Low. By reading.**

- **Disabled.** DESIGN.md asks what a part looks like disabled; the
  theme has five states, and that is not one
  (`crates/omorfos-config/src/style.rs:373-389`). The box that asks for
  a password takes no key and no press while a password is checked, and
  its buttons look as they did (`crates/omorfos-ask/src/prompt.rs:550-579`).
- **Chosen, as against where the keyboard is.** "A row that is chosen,
  or that the keyboard is on, is the accent": one look for two things,
  which a list with several chosen, as Files will have, needs apart.
- **More below.** No list shows that it goes on: the launcher's, its
  grid, "About", a menu taller than its screen.
- **Pressed** (finding 5), and **waiting for the next key of a chain**
  (finding 21).
- **Where the next character goes** is a line one unit wide
  (`crates/omorfos-launcher/src/picker.rs:849-856`,
  `crates/omorfos-lock/src/screen.rs:1022-1029`), with no setting; with
  motion reduced it is all that says a field has the keyboard.

![Now: the days of other months in a faint colour, at 2.1 to 4 to 1; a
window's title in dim text straight on the wallpaper; pictures of
arrangements whose quarters are 21 by 14 units; and a list in which
chosen rows and the keyboard's row look the same. Direction: the days in
dim text without a box; the title on a ground of its own; larger
pictures; chosen rows ticked, and the keyboard's row
ringed.](assets/2026-11-26-accessibility-smaller-things.svg)

### 23. Words

**Low. By reading the 208 English messages.** WCAG 3.3.3; WCAG 3's draft
asks for common words.

Most are short and plain, which is the house's way and a good one. Some
are the house's own where a user knows another word: "With the day" (the
scheme that follows the sun), "What opens what" (default applications),
"There is only one screen, so it stands only one way", "Kept from
sleeping by …", "Go on" and "Set" on the buttons of a new passphrase.
Someone who speaks to their computer says what a button is called
elsewhere. "Shut Down" and "Log Out" on the lock screen are "Shut down"
and "Log out" in the menu; "Maximize" keeps company with "licence" and
"recognised". The clock is of twenty-four hours whatever the locale
(`crates/omorfos-shell/src/clock.rs:46`). A few say what is wrong and
not what to do: "That was not right", "Nothing matches".

What to change: a list of words in DESIGN.md, the usual word first; and
one spelling.

### 24. What is listed as not there yet leaves out

**Low. By reading**, against what GNOME, KDE, macOS and Windows have in
their accessibility settings, from knowing them and not from reading
them again. A list for ACCESSIBILITY.md and the roadmap.

On ACCESSIBILITY.md's list: screen readers, high contrast, larger text,
a magnifier, sticky and slow keys, an on-screen keyboard, everything
from the keyboard, a window's focus said by more than colour. Not on it:

| Missing | For whom | What there is today |
|---|---|---|
| The bell: seen, or heard at all | deaf; blind | a terminal's bell is dropped; `xdg-system-bell` is on the roadmap as a protocol only |
| Keys struck twice passed over (bounce keys) | a hand that shakes | nothing; EN 301 549 5.8 |
| The pointer moved by keys (mouse keys) | no pointing device | nothing |
| Caps Lock and Num Lock said, seen and heard | everyone; blind | nothing (finding 9) |
| The buttons changed over; a drag locked | one hand; cannot hold a button | four settings of libinput, none of these (`crates/omorfos/src/backend/hardware.rs:1200-1209`) |
| The time for two presses to be one | slow hands | fixed at 400 ms |
| A press by resting the pointer | cannot press | nothing; needs finding 13's virtual pointer |
| Where the pointer is, shown on asking | low vision | nothing; its size goes to 128 |
| Colours told apart without colour; filters | colour blind | nothing |
| The text cursor thicker, blinking or not | low vision | one unit; blinking follows `reduce_motion` only |
| Notifications held back for a while | attention | `notifications.enabled` only |
| Text larger in programs | low vision | `font-name` only; no `text-scaling-factor` |
| High contrast told to programs | low vision | `contrast` is always 0 (known) |
| All of it reachable before logging in | everyone above | nothing on the lock screen |
| A sound when the session is ready, locked, asked | blind | omorfos makes no sound at all |

### 25. Documents that say more than is so

**Low. By reading.**

- DESIGN.md, principle 1: text meets the contrast "on every surface,
  over any wallpaper" (findings 1 and 17).
- `docs/DESIGN.md:149-150`: the rules "hold for every pair in the tables
  above" (finding 16).
- `:159`: "what is urgent has a mark as well as a red edge; what has
  focus has a ring as well as a tint" (finding 11; a notification with
  the keyboard has neither ring nor tint).
- `:87-88`: nothing clicked is smaller than 24 by 24 (finding 18).
- `:304-305`: a sign has words or a tooltip (the overview's plus and the
  month's arrows have neither).
- `:276`: nothing but a workspace sliding is longer than 300 ms (the
  scheme turns over a minute, the night light over twenty; both meant).
- `docs/ACCESSIBILITY.md:49-51`: "windows are switched with Alt and Tab"
  (as built in it is Super and Tab, between programs); the menus "are
  steered with the arrows" (neither can be opened from the keyboard as
  built in).
- `:46-48`: the button the keyboard is on has a ring in the boxes "about
  what a program wants" (the portal's has none).
- `:35-38`: `* { transition: none; }` turns off every movement
  (finding 19).
- `:19-20`: text from 6pt to 72pt (finding 10).
- `docs/adr/0037-who-may-do-what.md:83, 117-119`: `input` may "type and
  click"; a window "is marked as wanting attention" (findings 13 and
  11).
- The guide and the code call the button with the pictures the second in
  some places (`docs/guide/windows.md:267, 299`,
  `crates/omorfos/src/palette.rs:218`) and the third in others
  (`windows.md:11, 14, 211, 237`, `palette.rs:2`).

## Looked at and found sound

- The brand's five colours are kept for the mark and the pictures, and
  none is used for text on cream (`docs/BRAND.md:100-104`); the light
  scheme's accent is a deeper cyan, the brand's being too pale there.
- ACCESSIBILITY.md says omorfos is early, and lists what is not there
  rather than claiming to conform to anything.
- The pointer: its rim or its inside is 3 to 1 or more against any
  ground, in both schemes.
- The six accents: what is written on each is 5.1 to 10.3 to 1, and each
  against a solid surface 5.1 to 9.3.
- The terminal's colours: every one 4.9 to 1 or more on navy but black,
  which is the ground's own by custom.
- The notice's red heading on its thicker glass: 4.8 to 5.6.
- **#177:** "Search" in the empty launcher is dim, but at 20pt, where 3
  to 1 is asked; 3.6 at worst.
- No box that asks for a secret has a time limit, GnuPG's own is passed
  over, and what went wrong stays written until the next try.
- The eye in the box that asks for a password: 24 by 24, reached by Tab,
  and its tooltip shows with the keyboard on it as with the pointer.
- A finger or a key unlocks beside the password, and neither is the only
  way (EN 301 549 5.3).
- Keys repeat as set, or not at all; a chain of keys has no time limit.
- How long a notification stays is a setting, 0 for until clicked.
- The switcher opened other than by keys held stays open, and takes the
  arrows, Enter and Escape.
- A swipe carried slowly past half way does what a flick does.
- Focus goes by a click unless set otherwise, and a program cannot take
  it later.
- Nothing flashes, pulses or loops; the cursor's blink ends after ten
  seconds.
- No message names a control by its colour, its shape or its side alone.

## Not looked at

- Anything running: no screen was looked at, no key pressed, no screen
  reader started. The colours are the built-in theme's own; how a card
  draws them is not known.
- Programs written with GTK, with Qt and for X11 under omorfos, and what
  they do with what they are told.
- The switcher going through a program's windows (pull request 176): its
  ring, its fainter windows, and the quick press that is told from a
  held one by time.
- Languages written from right to left: every layout here is from the
  left, with buttons, signs and counts placed by hand. And translations
  longer than the English, of which the menu of 240 units and a
  question's one line are what would be found first.
- Colours as each kind of colour blindness sees them, beyond their
  luminance: every state found that rests on colour is under 3 to 1 in
  luminance too, and a titlebar's three buttons are told by their place.
- The guide and the manual pages as documents: their headings, their
  links, their own words.
- EN 301 549 clause by clause; its words here are from a summary of it,
  not from the standard.
- WCAG 3.0 beyond its draft's headings, several of which are where
  findings 6, 7, 14 and 23 point.

## Seen in passing, for other reviewers

- **Security:** what a virtual keyboard of the `input` role may type
  into, the sealed prompt and the lock screen among it (finding 13), is
  to be decided, not closed: it is a role the user gives, and a keyboard
  on the screen will need exactly this.
- **Reliability:** the compositor takes up its own new version after
  `update.wait` "regardless" (`crates/omorfos/src/versions.rs:82-88`),
  which by reading includes the launcher, or a box that asks for a
  password, being open. What becomes of what is typed across it was not
  looked at, and is finding 26's question again.
- **Docs:** `docs/guide/launcher.md:28-33` and `keys.yaml.md:55-56`
  disagree on whether Ctrl copies in the launcher in the Mac's way.
- **Experience:** Tab and Shift+Tab go the same way in the picker
  (`crates/omorfos-picker/src/lib.rs:585-589`, meant); a secret longer
  than 1024 characters is cut without a word.

## The order to take them

1. **3 and 26**, before a release of any kind; and **14** with 3, as
   they are one rule in one place.
2. **15**: a day's work, after which the gate finds the next part that
   is written too faintly, and DESIGN.md asks of the next part what was
   not asked of these.
3. **1, 2, 11, 16, 17, 22**: one pass over the built-in theme, before
   the bar and the dock of stage 5 wear it.
4. **13**: the decision, before stage 5 fixes how the bar is reached.
5. **5 and 7** together: a press that slipped, and the minute that then
   says yes.
6. **6**: who is watching.
7. **4, 21, 12**: what the roadmap already has as "everything in the
   shell reachable from the keyboard", and the settings without a file.
8. **8, 9**: secrets.
9. **10**, with larger text as a setting, which the roadmap has.
10. **18, 19, 20, 23**, as the code beside them is touched.
11. **24, 25**: words in documents, at any time.

## So that it is not found again

More will come as omorfos grows: stage 5 alone is a bar, a dock and a
tray, each with states, times and things to press. Three things would
find them as they are made, and not in a review after:

- finding 15, which makes the gate ask of every new part what this
  review asked of the 58 there are;
- accessibility as a dimension of its own in the review routine
  (`.claude/skills/review`), where it is one word among those of
  "experience" today, and was given one reader's part of a first pass
  here;
- a session walked through by hand before each stage is closed, which
  nothing here had: the keyboard alone from logging in to logging out;
  Orca with one program of GTK and one of Qt; a screen of 1366 by 768 at
  a scale of 2; and the theme at 22pt.
