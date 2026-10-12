# 24. Colours change over time, and nothing else does

- Status: accepted
- Date: 2026-10-12

## Context

Everything a theme describes changed in a single frame: a window's border
on gaining focus, a menu row under the pointer, the launcher's chosen row.
It looks abrupt beside what people are used to, and CSS has a word for the
cure, `transition`, which themes written for ADR 20 could not use.

There are three places the cure could have gone wrong.

- **Layout.** CSS will animate a width or a padding. Here that would move
  what the pointer is aiming at while it is on its way, and every component
  would have to lay itself out again each frame.
- **Idle work.** The compositor and the shell components do nothing while
  nothing changes (ADR 18). An animation that keeps a loop turning after it
  has finished, or that redraws what is not changing, breaks that.
- **Tests.** Nearly every test that looks at pixels changes a state and
  looks straight away.

## Decision

**A `transition` is a time and an easing.** `Style` has a `Transition`: how
many milliseconds, up to ten seconds, and one of the five easings CSS
names, worked out from the same curves CSS defines. The `transition`
shorthand takes them in either order with an optional `all`, or `none`;
`transition-duration` and `transition-timing-function` set one each. It
cascades like any property, so `* { transition: 150ms; }` covers every
part and a rule for one part or state overrides it. As in CSS, the
transition that applies is that of the state being entered.

**Only colours change over time**: background, text and the border on each
side. Sizes, fonts and rounding belong to the new state from the first
frame. A `transition` therefore names no property; naming one is an error
that says so. Colours are mixed by the light they give, so one that fades
in from transparent keeps its hue and does not pass through grey.

**`Motion`, in `omorfos-ui`, is the only thing that knows about time.**
Whatever draws parts that change state keeps one. It begins a picture by
telling it the time, asks it for each part's look where it used to ask the
theme, giving a number that tells that part from others of its kind (a
window's identifier, a row's place), and ends the picture. `Motion`
remembers the state each part was last drawn in. When that differs it
blends from what the part looked like at that instant, so a change
reversed half way turns round without a jump. A part drawn for the first
time has the look of its state at once, and one left out of a picture is
forgotten.

**A picture with something on its way asks to be drawn again 16ms later,
and one without does not.** `Motion::deadline` is merged into the deadline
each event loop already sleeps to: the compositor's, and the panel's,
launcher's and notifier's. Once everything has arrived there is no
deadline and the loop sleeps as before. The compositor's cache of painted
titlebars and menus is keyed on the looks `Motion` gives, so a frame that
is not changing is not repainted while another is.

**The built-in theme changes colours over 120ms, easing out.** A theme
turns it off with `* { transition: none; }`.

**Tests are not about time unless they say so.** The test session lays
`* { transition: none; }` under whatever theme a test gives, which a test
outranks by giving a `transition` to the parts it is about. Time is passed
in everywhere, never read, so those tests set the clock and look at the
first, a middle and the last frame.

## Consequences

- A theme can make state changes as quick or slow as it likes, per part
  and per state, and the default no longer snaps.
- While something changes colour the component that draws it paints it
  again on the CPU every 16ms: a titlebar, a menu panel, the bar. These
  are small, and it lasts as long as the transition does. It is not tied
  to the screen's refresh; a frame callback would be the way if it ever
  needs to be.
- Things that appear and disappear (a window, the menu, a notification)
  still do so at once. Fading them needs the whole of a drawn part made
  see-through, in the compositor for windows and in each component for its
  own surface, and is the next step. `Motion` is where its timing will
  live.
- A transition's delay, `cubic-bezier()` and `@keyframes` are not there.
  `Easing` is an enumeration, so the first two can be added without
  changing what uses it.
