# 111. A question is answered only by what was meant for it

- Status: accepted
- Date: 2026-11-27
- Takes up: [ADR 92](0092-one-button-and-one-box-that-asks.md), for how a
  box that asks is answered

## Context

A review of the design for accessibility
(`docs/reviews/2026-11-26-accessibility-design-wcag.md`, #180) found, at
its highest rank, that a key meant for another program says yes to
sharing the screen (finding 3). The portal's question takes the keyboard
as it comes up, nothing on it shows where the keyboard is, Enter is
"Share" and every other key but Escape is dropped: a program being typed
into, as one in a sandbox may be, asks to share the screen, and the next
Enter the user types for it is the answer. A press is the same: the
buttons act as the pointer's button goes down (finding 5), so a box that
comes up under a pointer about to press is answered by that press. And
the boxes that ask answer each its own way (finding 14): which button the
keyboard starts on, whether Tab moves it, whether a ring shows it.

ADR 92 made one button and one box that asks, drawn and laid out once;
what a key or a press does to one was left to each program that asks.

What others do, as found on 26 November 2026, from their sources:

- **Chromium** keeps what is pressed from a prompt for about half a
  second after it is shown, the double-click interval in its code, longer
  while presses keep coming, and takes no key that repeats, "held when the
  dialog pops up". Its design notes ask that saying yes take an explicit
  choice.
- **Firefox** keeps a permission prompt from being answered for 500 ms,
  and a dialog for 1000 ms; a press too soon starts the wait again, up to
  twenty times it; what refuses works at once.
- **GNOME**: the portal's question of access starts on Deny, and its
  question of sharing has Share off until a screen is chosen; the shell
  takes a button's key as it is let go, and only if it saw it go down, so
  a key held as the dialog comes is nothing to it.
- **KDE**'s portal takes Enter as it goes down, as omorfos does.

## Decision

**A box that asks, every one, is answered only by what was meant for it:
the keyboard starts on the button that refuses, and is shown there; the
button that gives answers nothing until the box has been on the screen
for half a second; a key counts as it is let go, and only if it went down
while the box was up; and a press counts as it is let go, on the button
it went down on.** It is made once, in `omorfos_ui`, as the box is
(ADR 92), and every box that asks takes it up.

- **Where the keyboard starts.** On the button that leaves things as they
  are: Cancel, Keep, Don't, whatever refuses. It is `:focus`, which the
  theme rings. Tab and Shift+Tab, Left and Right move it among the
  buttons, which are side by side, and not round from the last to the
  first, nor by Up and Down: one arrow meant for a list in the program
  under the box would otherwise take it from refusing to giving. Enter
  and space press the one it is on; Escape refuses, from anywhere. Saying
  yes takes a move and Enter, or a press.
- **Half a second.** For half a second after the box is first drawn,
  not after it is made, which a busy compositor may make a while sooner,
  the button that gives answers nothing, by key or by press; one pressed
  too soon starts the half second again, to two seconds at most, so that
  a burst of presses meant for what was under it does not get through.
  What went down too soon gives nothing however late it is let go, as a
  press held across the half second would otherwise. And the keyboard
  moved to the button that gives has it give by Enter only half a second
  after it came there, so that Tab and Enter meant for a form in the
  program, however long after the box came, do not say yes. What refuses
  works at once, as in Firefox: there is no harm in refusing too soon.
- **Keys.** A key counts as it is let go, and only if it went down while
  the box was up, as in GNOME's shell; a key that repeats counts for
  nothing, as in Chromium: either was held as the box came, for something
  else.
- **Presses.** A press counts as the pointer's button is let go over the
  button it went down on; let go elsewhere, it is nothing.
- **A box with a field**, as the one that asks for a password, starts with
  the keyboard in its field, where Enter hands over what is typed: that
  is typing, not saying yes, and what is typed there is what was meant
  for it. Its buttons are as above by the pointer; by Enter they are
  pressed at once, as the field is, since a wait on Authenticate would
  guard nothing that Enter in the field does not give at once.
- **Taken up by** the portal's question, the question before leaving and
  the box that asks for a password; and by the question whether to keep
  the screens as they are, which has no buttons, for the half second and
  for keys: Enter keeps them, Escape puts them back. A box added later
  that asks takes it up as it takes up the box.

## What is left apart

- **The welcome card**, whose Back and Next step through a tour and give
  nothing away, keeps its own keys and its own order of what has the
  keyboard.

## Alternatives

- **The delay alone, Enter still giving.** The least change, and what
  Chromium and Firefox have; but a user who waits, or cannot see the box,
  still says yes with the next Enter meant for something else.
- **The keyboard on refusing alone, no delay.** What GNOME's question of
  access has; but a press, which is not the keyboard's, is still taken as
  the box comes up under it.
- **Each program its own.** As now: seven boxes answering four ways,
  which finding 14 counts against.
- **A longer delay**, Firefox's 1000 ms for its dialogs: safer against a
  slow press, and in the way of everyone who reads quickly; half a second
  is what both browsers keep their permission prompts to.

## Consequences

- Enter on a box that asks refuses unless the keyboard has been moved to
  the button that gives: a change to what people who have used these
  boxes are used to, which the guide says, and the reason it is an ADR.
- Every box that asks looks and answers alike, its keyboard shown.
- Its tests press as the box comes up, and too soon, and with a key held
  from before, and see nothing given.
- The question before leaving still says yes by itself after a minute;
  that is finding 7's, and a decision of its own.
