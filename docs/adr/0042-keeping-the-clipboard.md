# 42. Keeping the clipboard

- Status: accepted
- Date: 2026-10-15

## Context

On Wayland, as on X11, what is copied stays with the program it was
copied from, which hands it over each time it is pasted. When the
program closes there is nothing to paste: text copied from a terminal
that is then closed is gone. `omorfos-screenshot` stayed running, unseen,
to give a picture it had put on the clipboard.

The `clipboard` role (ADR 37) lets a program read and set the clipboard
at any time, through `ext-data-control-v1`, which is how a clipboard
manager does its work.

## Decision

**A program of its own, `omorfos-clipboard`, started with the session
with the `clipboard` role, keeps what is copied.** The compositor stays
as it is: it holds nothing it was not given, and another manager can
take the place of ours with one line of configuration.

- **It takes over at once.** As soon as something is copied it asks for
  all of it, in every form the program offers, reads it, and offers it
  in that program's place. The program is then told something else was
  copied, and is free to close; `omorfos-screenshot` stops at once. The
  alternative, reading it at once but offering it only when the program
  closes, leaves the clipboard empty for the moment between the program
  going and the copy being offered, and the program the owner of what it
  copied for as long as it runs, which is what keeps the screenshot tool
  running.
- **It knows its own** by a form of its own beside the others,
  `application/x-omorfos-kept`, and does not read what it offers itself.
- **A secret is not kept.** What a password manager copies says so with
  `x-kde-passwordManagerHint`, and is left with it, so that it goes when
  the password manager takes it away. What is only selected, for the
  middle button, is not kept either: it is meant to be fleeting.
- **It is read all at once, and within limits.** Every form is asked for
  together, once each, and read on threads of their own, so that one slow
  form does not hold up the rest. What comes within two seconds is kept,
  and a form that does not come is gone without: tried by hand, Firefox
  offered forms through the X server that never came, and waiting for all
  of them kept nothing at all. What a program written for X11 offers that
  is not data, such as `SAVE_TARGETS`, is not asked for. Together it must
  amount to no more than 64 MiB, or none of it is kept and it is left with
  the program that copied it. A program that never hands over what it is
  asked for holds a thread until it closes.
- **What changed while it was read is not offered.** Before offering it,
  the keeper makes sure, with a round trip, that nothing else has been
  copied and the clipboard has not emptied, as it does when the program
  that copied closes; and what is empty in every form is not worth
  offering. Otherwise the clipboard would be set back to something older,
  or to a copy cut short.

## Alternatives

- **Keep it in the compositor.** It has every copy already, and could
  hold the last when its program goes. But then the compositor reads
  pipes from programs, holds large pictures, and is where a password
  could be kept by mistake; and a better manager cannot replace it.
- **Only when the program closes.** As above: a moment with nothing on
  the clipboard, and the screenshot tool kept running.

## Consequences

- A program that copied is told at once that something else was copied,
  and takes it as having lost the clipboard. One that shows what it
  copied differently while the clipboard is its own stops doing so at
  once. That is to be watched for in real programs.
- What is copied is in the keeper's memory until something else is
  copied. The `clipboard` role is what reads it; it was always the
  manager's to have.
- A history of what was copied, which ADR 34 draws, can grow from the
  same program.
