# 90. What a program is handed is written without waiting on it

- Status: accepted
- Date: 2026-11-15

## Context

What is on a Wayland clipboard is given, each time it is pasted, by
whoever put it there: the program pasting hands over a descriptor, and
what was copied is written into it. Three of omorfos's programs give
this way. The compositor gives text of its own, what "About" copies, the
launcher's answer and what `omorfosctl` asks it to copy (ADR 82);
`omorfos-clipboard` gives what it keeps (ADR 42); and
`omorfos-screenshot` gives its picture.

The descriptor is the pasting program's, and so is when it is read: at
once, late, or never. A review
([finding 1](../reviews/2026-11-15-what-a-client-can-cost.md#1-the-compositor-waits-for-ever-on-a-pipe-a-program-hands-it))
showed what that does to a write that waits. The compositor wrote its
text with one, on the thread that draws every screen, trusting that a
pipe takes a short text whole, as ADR 82 said. An empty pipe does. A
program that fills its pipe first and then asks for the text had the
compositor wait until the pipe was read, which was never: every screen
frozen and no key reaching anything, by any program with the keyboard.
The keeper wrote on a thread for each paste, which kept it going, but a
paste never read held its thread for good, and nothing counted them.
The screenshot tool wrote where it waits to hear that something else
has been copied, so a paste never read stopped it there, still holding
the clipboard.

Not waiting is less simple than asking the pipe not to. Whether a write
to a descriptor waits is a flag of what the descriptor names, not of the
descriptor: the program that handed it over holds the same thing, and
can take the flag off again between our setting it and our writing.

## Decision

**Whatever is handed to a program through a descriptor it gave goes
through one type, `omorfos_ipc::handing::Handing`, which never waits on
the program and never rests on what the program can change.**

- **Into a pipe, and nothing else.** A pipe is what programs paste
  through, and the one thing that can be written to without waiting
  whatever is said of it. A file may wait on what it is kept on and a
  socket on its other end; either is closed with nothing written, and
  its program reads nothing, as from one that had nothing to give.
- **On a thread of its own, and no more than sixteen at once**
  (`handing::MOST`). Whoever hands something over goes on at once: the
  compositor never touches the program's pipe on the thread it draws
  on. (Amended 16 November 2026: first a thread for each, and one more
  than sixteen handed nothing; now one thread for each that gives, which
  pours into every pipe at once, and one more than sixteen lets go of
  the one that has waited longest.)
- **Through a pipe of our own.** What is handed over is written into a
  pipe made here, asked not to wait, which nothing else holds and so
  nothing else can ask to wait again; and moved from there into the
  program's with `splice`, which is told at each move not to wait
  (`SPLICE_F_NONBLOCK`). The program's flag is never read or set.
- **For half a minute, all told.** While the program's pipe has no room
  the thread waits for some, by the clock (`poll`), with what is left of
  the half minute. A program that pastes reads at once; one that has
  not read in that time is given up on and its pipe closed.
- **Waited for where the giver is about to end.** `Handing::finish`
  waits until everything has been handed over or given up on, for
  `omorfos-screenshot`, which ends once something else is copied and
  would cut short what a program was still reading. The compositor
  never calls it: it ends without waiting on anyone.

The compositor, `omorfos-clipboard` and `omorfos-screenshot` all hand
over through it, so it is right, and tested, in one place.

## Alternatives

- **Ask the program's pipe not to wait, and write once** (`O_NONBLOCK`,
  then `write`), which the review first proposed. The flag is shared
  with the program, which can take it off again before the write; the
  test for this keeps a copy of the descriptor to show that nothing
  rests on it. One write also hands over no more than the pipe has room
  for at that moment, which a picture is more than.
- **Write when the loop says the pipe has room.** A pipe that has room
  when asked may have none by the time it is written to, as the program
  holds an end of it too; the write still has to be one that cannot
  wait, which is the same question.
- **A thread and a write that waits**, as the keeper had. Nothing stops
  the compositor, but a thread is held for as long as the program
  likes, and as many as it likes.
- **`pwritev2` with `RWF_NOWAIT`**, a write told at the call not to
  wait, which needs no pipe of our own. A pipe takes it from Linux 6.4
  on (it was tried on 6.18) and refuses it before; `splice` from one
  pipe into another has taken `SPLICE_F_NONBLOCK` since kernels far
  older than any omorfos runs on. One way that holds on every kernel
  was preferred to two.
- **Shorter text**, so that one write is all or nothing. All or nothing
  is of a pipe with room for it; a full one still waits.

## Consequences

- A program that hands over something that is not a pipe to paste into
  is given nothing. What pastes makes a pipe for it, as far as is
  known; a program found not to is a reason to look at this again.
- While programs do not read, one thread of each that gives holds up to
  sixteen pipes for up to half a minute each; it is started the first
  time something is handed over.
- A program asking again and again through pipes it does not read
  cannot keep a paste from going through: a paste is the newest, and
  the one that has waited longest makes room for it. Who asks is not
  told to whoever gives, by `ext-data-control` at all, so they cannot be
  counted by program; this needs no telling. (Amended 16 November 2026:
  with a thread for each, a program that kept all sixteen busy had
  every paste of that giver's come out empty until one of them ended.)
- The compositor's own text is no longer in the pipe by the time the
  request for it has been dispatched: a test that pastes it waits until
  nothing is being handed over (`Compositor::handing_over`).
- `splice` is Linux's, as the rest of omorfos is.
- ADR 82's sentence on this is put right there.
- (Amended 16 November 2026.) The same holds of a socket a program
  hands over to be listened on, as the relay is a sandbox's: whether
  taking a program from it waits is the program's to say as much as
  ours, and one that has it wait would have the relay wait for good.
  The relay listens on each from a thread of its own, which waits in
  its place and hands on what connects (`omorfos_relay::sandbox`).
- Reading what another program copied is not part of this: the launcher
  and the keeper each read a paste in their own way, and the keeper's
  holds a thread for each kind until its program closes the pipe. That
  is on the roadmap, to be made one beside this.
