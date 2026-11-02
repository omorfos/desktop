# 61. How the relay is built

- Status: accepted
- Date: 2026-10-26
- Builds on: [60. The session outlives the compositor](0060-the-session-outlives-the-compositor.md)

## Context

ADR 60 puts a relay, `omorfos-relay`, between programs and the
compositor: it keeps each program's connection and the state of the
protocol on it, and makes that state again for a compositor started
anew, so that a restart closes no window. It left how the relay is built
to the building. Four things have to be settled before any of it is.

Who a program is. The compositor decides by a program's process
whether a window it opens may take the keyboard (ADRs 8 and 36),
names the program it refused something (ADR 37), and says which are
taking pictures of the screen (ADR 43). It asks the socket, by
`SO_PEERCRED`. Behind the relay every socket is the relay's, and every
program would be the relay.

What the relay reads. It is a second reader of what untrusted programs
send, before the compositor's, and ADR 60 has it fuzzed from the first.
It has to know every message of every protocol the compositor offers, to
know which carry descriptors and which make objects, and only those
(the list in `omorfos-ipc`, ADR 60).

How it is started, and what happens when it ends. The holder starts the
compositor; the relay has to be started, given the socket, and joined to
each compositor in turn, and a relay that crashes must not end the
session the holder was made to keep.

How it is built in steps a user can try. Programs are kept only once the
relay can make again everything they made, and a relay that does that
for every protocol is a large piece of work.

## Decision

**The relay tells the compositor who each program is.** The relay and
the compositor are joined by a connection of their own, which only the
holder makes and hands out, one end to each: no program can reach it.
For each program that connects, the relay reads its process, user and
group from its socket, makes a pair of sockets, and hands the
compositor one end with what it read. The compositor takes that end as
the program's connection and what it was told as who the program is,
and asks the socket of no connection that came through the relay. The
relay is as trusted as the holder that started it; what it says is not
checked again.

**The relay reads with a reader of its own**, small and bounded, driven
by the descriptions of each protocol's messages that the Wayland crates
already in the tree carry (`wayland_backend::protocol::Interface`), so
that nothing is added to the tree. It reads every message both ways,
keeps a table of each program's objects and of what each is, and drops
a program that sends what does not parse, names an object it has not
made, or breaks a bound. A global not in the list cannot be bound
through it. A test checks that each protocol in the list has a
description at or above the version listed.

**It is fuzzed with `cargo-fuzz`**, in a crate of its own beside the
workspace, run by a CI job of its own on a nightly toolchain, which
`cargo-fuzz` needs; everything else stays on the stable toolchain the
project is pinned to. What the fuzzer runs is the reader, the table and
the bounds, fed what a program might send.

**The holder holds the relay as it holds the compositor.** It starts
the relay first, hands it the socket, and keeps it running: found ended
as the next compositor is about to be started, it is started again at
once, as the relay holds nothing, such as the seat, that would need time
to be let go; three times a minute, and then given up. For each compositor it starts, it makes a new connection
between the two and hands one end to each, the relay's by the
connection the relay was started with. A compositor started from the
version held, which may be older than the relay, is handed the socket
itself, as before there was one, and the relay is stopped until the
next login. When the relay ends, the programs that came through it lose
their connections, as they would with a compositor that crashed. The
compositor hears its connection to the relay close and steps down, and
the holder starts a new relay and then a compositor joined to it: the
windows are gone either way, and the cost of starting the compositor
again too is a blink, where handing a running compositor a new relay
would need words the holder does not otherwise say. A relay that keeps
failing, three times a minute, is given up, and the compositor is
handed the socket itself for the rest of the login.

The relay's connections, to the holder and to the compositor, carry
records rather than a stream (`SOCK_SEQPACKET`): each hand, its words
and its descriptors, arrives whole and alone, however many come at
once, so that nothing has to find where one ends.

**Components and sandboxes reach the compositor as they do now**, at
first. A component is handed a connection the compositor made, and is
started again with the compositor, as ADR 28 says; a program in a
sandbox connects where its sandbox listens, which the compositor serves.
Both are taken over by the relay in later steps: components kept as
programs are, which ADR 60 asks for, and the sandbox's listener handed to
the relay with what the sandbox is, so that a program in one is still
shown no more than before.

**It is built in three steps, each tried on real hardware.**

1. The relay in the path: started by the holder, reading every message
   both ways and passing it on, telling the compositor who each program
   is. A user sees nothing change but one more process; a restart
   still closes every window.
2. Programs such as `foot` kept: the protocols it uses made again for a
   compositor started anew. A program holding anything the relay
   cannot yet make again is closed when the compositor restarts, as
   before. Each step after this keeps more programs, and none keeps
   fewer.
3. The rest: every protocol in the list, components kept, sandboxes
   listened to by the relay, and the compositor taking up its own new
   version at a quiet moment, as ADR 60 says.

## Alternatives

- **The compositor asks the relay who a program is**, by the connection,
  when it needs to. It would need an answer while handling a request,
  which would wait on another process. Telling it once, as the program
  connects, costs nothing later.
- **`SCM_CREDENTIALS` on each connection the relay makes**, the program's
  process passed as the sender's. The kernel lets a process name another
  only with `CAP_SYS_ADMIN`, which the relay is not to have.
- **wayland-backend's own reader.** It is private to the crate, and its
  server side would have the relay hold the program's objects as a
  compositor does, which is the whole of Smithay's work over again.
- **A relay that passes bytes and descriptors on without reading them.**
  The kernel keeps descriptors with the bytes they came with, so
  passing works; but nothing could then be made again, and the reader is
  what every later step stands on. It is built and fuzzed first.
- **Property tests in place of fuzzing**, which need no other toolchain.
  They are not guided by what the code reaches, and ADR 60 asks for
  fuzzing.
- **`cargo-afl`**, which builds on the stable toolchain. Less used in Rust
  than `cargo-fuzz`, and its tool has to be installed to run it at all.

## Consequences

- Every program's messages are read once more, by the relay. An idle
  desktop sends none.
- The relay sees all that programs send, the clipboard and what is typed
  among it, as the compositor does. It is a process of the user's, as
  the compositor is, started by the holder, and its connection to the
  compositor cannot be reached by programs.
- A program's process is what the relay says it is. A relay that lied
  could let a window take the keyboard it should not; the relay is ours
  and is started only by the holder.
- The compositor still serves the socket itself where there is no relay:
  run nested, run without a holder, or held without a relay that works.
  Both paths are tested.
- The fuzzer's crate is outside the workspace and outside the coverage
  gate (ADR 2), and its CI job is the only one on a nightly toolchain.
- Until step 2, the relay costs a little and gives nothing a user can
  see.

## Step 2: how what a program had is made again

Settled as step 2 was started, and built in two parts: the means, with
the protocols the tests' client speaks (2a), then the rest of what `foot`
speaks, a protocol at a time (2b). Programs drawn on the graphics card,
as GTK's are, come in step 3.

- **A program's connection outlives the compositor.** When the compositor
  ends, the relay keeps each program's connection and reads nothing more
  from it, so that what it sends waits, until the holder hands it the
  next compositor. A program holding an object of a protocol the relay
  cannot yet make again is let go then, as before; a global it has bound
  and made nothing of can always be made again, by binding it. A
  compositor ending closes its connections one at a time, and another
  process may hold a copy of one for a moment, so a program whose
  connection closes, with no error said, waits up to half a second for
  the compositor to be seen gone before it is taken to have been let go
  by a compositor that goes on, which omorfos does only with an error.
  Half a second was not always enough: on real hardware, a compositor
  stepping down once closed every program's connection more than that
  before its own, and the relay let every program go. A compositor
  stepping down now says `going` before it closes anything, and a
  program whose connection closes after that is the compositor going,
  however long its own connection stays open; the wait is left for one
  that says nothing, as an older one does, or one that crashes, whose
  connections all close at once.
  A program that is busy, as one playing a video is, is as likely to find
  the compositor gone by a request that cannot be written to it, which is
  taken the same way; taken for the program's own end, it let Firefox go.
- **What each program has made is written down as it is made**: for each
  object living, the request that made it, with its descriptors held,
  and what has been asked of it since that lasts, as a table for each
  protocol says: the last of a request that sets something, every one of
  one that adds to it (a region's), and, for what applies on a commit,
  what the last commit applied. A message's description says what each
  argument is but not what it means, so which number is a serial, a name
  or a size is in those tables, written by hand for each message.
- **It is made again in the order it was made**, under the same numbers.
  A compositor, as wayland-backend has it, takes a number for an object
  only if it is the next past those it has, or one it has freed; a
  program's have gaps where it destroyed something, so before an object
  is made again each number below it not yet taken is taken by a sync of
  the relay's own, which the compositor answers and frees. So is each
  past the last made again up to the highest the program has ever taken:
  libwayland takes a freed number again, the last freed first, so the
  program may next take one above every number it still holds. The relay's
  syncs take the next number past all of them, which the program cannot
  take meanwhile, as it is not read until the relay has caught up. What
  the compositor makes, such as an offer of what is on the clipboard, it
  makes again itself; the program, which may still hold the compositor
  before's under the same numbers, and which takes them under the same
  rule, is given each under the lowest number free to it, and the relay
  puts each number in the place of the other as messages pass.
- **Until the new compositor has caught up, what it says is sifted.** The
  relay ends what it sends again with a `wl_display.sync` of its own,
  under a number from the top of the program's range; until that is
  answered, what the compositor says is passed to the program only if the
  program has not been told it already. Its globals are taken in and
  named toward the program as they were; its configure of a window is
  acked by the relay, which then commits what the window last showed, and
  is passed on only if it differs from what the program last acked, whose
  own ack of it is then kept from the compositor, as it is acked already.
- **A global has one name for the program, however many registries it
  has.** A library a program draws with, as Mesa's EGL, makes a registry
  of its own, and each global is told to it by the name the program's
  first registry was told. A global taken away is told so on each, by
  that name, which stays the program's for it until a new global takes
  that name or the compositor's for it. Named anew for each registry, the program
  had more names than the next compositor had globals, and every program
  that draws with the card was let go, as ghostty and Firefox ESR were
  when 3a was first tried.
- **What was focused leaves, and comes back.** When the compositor ends,
  the relay tells the program that the pointer, the keyboard and any field
  typed into have left its surfaces, and the new compositor tells it they
  entered, with its own serials. The program does then what it does on
  any enter: it names the pointer's shape again, under the new serial, and
  enables its field again. Keeping the leave and the enter from the
  program would need every serial it holds put in place of the new
  compositor's, both ways, in every request that carries one; that was
  meant for 2b, and was left to step 3, as it spares only a flicker of
  focus and `foot` comes back without it. Since 3b only a field is told
  it left, as below. The program was also told the
  new compositor's first configure of its window where it differed from
  the one it last acked, as the first of a window not yet focused does;
  since 3b it is not, as below.
- **A field's count of its commits is the program's** (2b). The
  compositor's `done` to a field carries how many times the field has
  been committed, and the program sets aside what is typed for an older
  state than its own. A new compositor counts from when it made the field
  again; the relay counts both, and adds the difference.
- **What says how something is is not said again as it was** (2b). The
  scale and transform a surface is best drawn at are told only as they
  change, and the relay keeps the same again from the program at any
  time, as the new compositor tells them once a surface is shown, after
  the relay has caught up. A decoration's mode and how large a window may
  be come with a configure, and are kept from it only while the relay
  catches up.
- **A surface's parts are stacked as they last were** (2b). A part
  (`wl_subsurface`) put over or under another names it, and that one may
  since have gone, so the requests are not kept; how each surface's parts
  are stacked is, and the relay puts each over or under its neighbour
  again, outwards from the surface. What a part shows is shown with what
  it is part of, so each surface is committed after its parts, the
  deepest first.
- **A program let go is said to be**, named by the file it runs and its
  process, with everything it held that cannot be made again, so that a
  program that did not come back on real hardware says why. One the
  compositor lets go of without a word is said to be with what the relay
  was doing for it: whether it was making the program again, the last
  request it sent, and how much the program had not read. A compositor
  lets a program go silently where what it is sent cannot be read, or
  where it will keep no more for the program to read (ADR 45); a
  protocol error it says itself, and the compositor writes it in its log.
- **A window comes back as a new one would**: placed by the new
  compositor, on the workspace in front. Carrying where each was is the
  compositor's to hand on, keyed by a number the relay gives each program
  and keeps across compositors, which the `program` record carries from
  now on. That is step 3's; it was to be through the holder, and is
  through the relay (3c).
- **The relay is handed over when the holder takes up its own new
  version**, rather than stopped: its connection to the holder, and its
  process as a line, as the holder cannot know it from anything handed
  over that a process becoming another program keeps. The new version
  waits on it by its number.

## Step 3: the rest of what programs make

Settled as step 3 was started, and built in parts, the programs most used
first: those drawn with GTK and the browsers (3a); the serials a program
holds, and what was on the clipboard, with the clipboard keeper kept as
programs are, brought forward from 3d for it (3b); a window back where and
as it was (3c); programs in a sandbox, then layer surfaces, then the
components the compositor starts (3d, in that order); every other
protocol in the list made again (3e), the pointer held first, as games
hold it, then what tells a program the user is idle and the keyboards
programs type with, then windows known to another program by a handle,
then input methods, then capturing the screen, then the lock; and the
compositor taking up its own new version (3f). A test lists what a
program can make with what is offered that the relay cannot make again,
and fails while that is other than what 3e still has to come.

- **What lasts only a moment ends with the compositor** (3a): a popup, a
  token to hand focus on, and word asked of a picture being shown. Each is
  told it has ended as that compositor would have told it: the popup that it
  is dismissed, the token not yet given a token no compositor takes, the
  word that the picture was discarded. None is made again; from then on the
  relay answers for each, keeping what is asked of it from the compositor and
  freeing its number when the program lets it go. A popup's surface is made
  again showing nothing, so that a toolkit may make it a popup again, which
  a surface that has shown something may not become.
- **A gesture under way is told it was cancelled** (3a), as one may be, and
  the gesture objects are made again.
- **Buffers on the graphics card are made again from the descriptors the
  relay holds** (3a), as Mesa makes them, by `create_immed`. One the
  compositor made, by `create`, has the compositor's number, under which a
  program cannot make it again; its program is let go, and the log says so.
  Mesa does not make them so.
- **Every number a program may take next is free to the next compositor**
  (3a). What is not made again leaves numbers above the highest that is, and
  a program may take any number it has been told is free, where a compositor
  takes only the next or one it has freed; the relay takes every number up
  to the program's highest with syncs of its own, which the compositor frees.
- **What the relay tells a program of its own accord is read into the table
  of its objects** (3a), as what the compositor tells it is, so that a
  number it frees can be taken again.
- **A program is told a number is free once** (3a). The compositor's word
  that the number of something destroyed is free is withheld while
  something kept still names it, as it is made again with that, and the
  relay frees the number once nothing does. When the compositor goes,
  nothing is withheld any longer: the next compositor makes it again,
  destroys it again and frees its number itself. Told twice, as of a
  menu's region at first, a program's Wayland library takes it for a
  fault, which Firefox ends on.
- **A window's icon is made again but not given to the window again** (3a):
  the request that gives it names the window and is made of the manager,
  where what lasts is kept for the object a request is made of. Since 3c it
  is the window's to keep, as below.
- **A window exported is told the new compositor's handle** (3a), which the
  program takes as its own; one it passed on before is the old one's, and
  refused where it is imported by only after, as a stale handle is. What
  was imported by it before is imported again (3e).
- **Serials keep their meaning** (3b). Each compositor counts its serials
  from its own start, so the next's would go back under a program and mean
  nothing to it of the one before's. Until the first change of compositor
  they pass as they are; from then on the next compositor's are told past
  every serial the program has been told, by adding one offset to each,
  and taken back as they are asked with. A serial of a compositor before
  is asked with as the next one's that stands for it: for the pointer's
  cursor, which Smithay sets only under the serial of the pointer's last
  enter, that enter's; for anything else, which Smithay does not check,
  the last. Which argument is a serial is a table written by hand, as a
  description says only that it is a number, and some arguments called a
  serial count something of the program's own: a field's `done`, which
  the relay already counts, and an input method's `commit`. A test
  checks each entry against the protocol's description.
- **The pointer and keyboard are not said to have left if they come back**
  (3b). When the compositor ends they are held, as the program still has
  them; if the next gives them back to the same surface, the pointer at
  the same place and no key held, its enter is kept from the program, and
  the pointer's frame with it. Back elsewhere, the program is told they
  left and then entered; not back once the relay has caught up and the
  compositor has had a turn more, which it gives a window shown again the
  keyboard and the pointer in, they left. Two syncs of the relay's own
  measure that turn, the second answered after it; the program waits to
  be read until then, as a sync of the relay's takes a number the program
  may take next. A destroyed surface has nothing to be said to have left,
  and what had it is forgotten; nor has a pointer, keyboard or field the
  program destroyed anything to be said to have left. Where a menu is ended as the compositor
  goes, the pointer and keyboard are told they left at once, as a menu
  ends as focus moves: Firefox, told only that its menu was dismissed,
  took the next click for the one that closes it, and opened no other.
  A field is still told
  it left and entered: it has no serial, and is enabled again by the
  program as it enters, which a field kept from the enter would not be.
  A key the keyboard holds as the compositor goes is let go of then,
  under the last serial the program was told, as no compositor will say
  so: a program repeats a key held itself, and Return, pressed to start
  the compositor anew and let go of while none was there, was typed on
  and on, the keyboard's enter that would have ended it kept from the
  program.
- **What the next compositor first says of a window is the relay's to
  answer** (3b). It is said of a window not yet shown again, with no size
  and no focus; told it, Firefox took its window for one that had lost
  focus, which with the keyboard's enter kept from it it did not take
  back. The relay acks it, as it did, and shows the window as it was;
  what the compositor says once the window is shown is told.
- **A buffer the compositor held is released as it goes** (3b), unless
  the next is shown it again. A program drawing with two buffers in turn
  waits for one to be released before it draws again; one shown last
  before the compositor went was never released, and Firefox, waiting for
  it, drew no more. Found as 3b was tried; it was so since step 2.
- **What was on the clipboard is there again** (3b). The keeper, which
  holds what was copied once the program it came from may close, is kept
  as programs are. The compositor starts it as before, and hands the
  relay its connection, with its process and its roles, which the relay
  hands back, and again to each compositor after; the next gives it its
  `clipboard` role on the relay's word, as it takes who a program is on
  it. ADR 37 has a role come only from a connection the compositor made
  for a program it started, and so it does: the relay hands back only
  what a compositor handed it, over a connection no program can reach.
  The relay says once it has handed every program it kept, and a
  compositor joined to it starts the keeper only then, and none if one
  was among them. The keeper's device and sources are made again, and
  its selection set again once its source is, through
  `ext-data-control-v1`, which needs neither focus nor serial: the
  clipboard is there at once. A selection whose source was since told
  it is held no longer is not set again. Only the keeper is kept so;
  the rest of what the compositor starts ends with it until 3d.
- **What a program selected is selected again as it next has the
  keyboard** (3b): the primary selection, and what the keeper does not
  keep. Smithay lets a program set either only while it has the
  keyboard. The next compositor says what is selected as it gives a
  program the keyboard: if nothing, that is kept from the program and
  its own is set again; if something else, its source is told it was
  cancelled, as it would have been.
- **A window comes back where and as it was** (3c), in two parts: first
  each window alone, then the tiles and tab groups windows share. The
  compositor tells the relay, as it changes, where each window of a
  program the relay keeps is: the output, by name, the desktop of that
  output, the frame's place and size, whether it is maximized,
  fullscreen, minimized, rolled up, kept above or on every workspace, how
  many frames of its workspace are under it, and whether it has the
  focus there; and where the session is: which desktop each output
  shows, which has the keyboard, and where the pointer is. The relay keeps
  each as text it does not read, a window's under its program's number
  and the program's number for its toplevel, which it keeps across
  compositors, and lets one go once the toplevel is. To the next
  compositor it hands where the session was first, then each program,
  with where its windows were; the compositor puts each window back as it
  is shown again, under the frames that stood over it whatever order they
  come back in, and only the one focused takes the focus. The relay, not
  the holder, carries it: it keeps the programs, survives the compositor
  ending however it ends, and has no record's size to fit in, where the
  holder keeps as little as ADR 60 says. An output not there is taken to
  be the one with the keyboard. In the second part, a window that shares
  a frame says what the frame is known by, where it is among the tabs and
  whether it is the one shown; one that shares a workspace says what the
  workspace is known by, how it is cut and which part it fills; and
  where the session is says which such workspace each output shows. What
  each is known by is the compositor's before: the next keeps, while
  windows come back, what it has made for each, so the first of a frame
  or a workspace to come back makes it again and the rest join it, in
  whatever order they come. A part another window has already, or one
  the workspace does not have, sends the window to its desktop alone.
  The place begins `v2`, and one without, written before, is read as a
  window of neither, so that windows kept across the update still come
  back. A window fullscreen alone says too whether the workspace made for
  it is the one shown: put back fullscreen, its workspace is made again
  beside the desktop it came from, and is shown only if it was. Where
  the session is says only which desktop each output shows, and that
  desktop for one showing a fullscreen window's workspace; read as it
  was, the desktop a fullscreen window came from, shown as the window
  came back, gave way to the window's workspace, and with it the
  keyboard, though the user had gone back to the desktop. A place
  written before says it was not.
- **A window's icon is given it again** (3c): what gives it is kept as
  the window's own, each window its own, and made again once everything
  else is, as the icon may have been made after the window.
- **A sandbox is the relay's** (3d): what a program such as Flatpak hands
  over to make one, the socket it listens on and the descriptor whose
  closing ends it, is kept by the relay, which answers for the sandbox
  being made and listens on the socket itself; the compositor is never
  told of it. Each program that connects there is relayed like any
  other, numbered and kept through a change of compositor, and handed to
  each compositor by a hand of its own, `sandboxed`, as one inside a
  sandbox, so that it is shown no more than it was. A compositor takes
  for a new object only the next number or one it has freed, so the
  number of the sandbox being made is taken up there by a sync of the
  relay's, whose answers are kept from the program, and the relay frees
  it when the program destroys the sandbox. What a sandbox says it is,
  its engine, program and instance, is not kept, as the compositor takes
  nothing from it; the portals asking the user by the program's name will
  need it carried in that hand. Once the sandbox ends, nothing more comes
  in that way, and what came in stays. At most 64 are listened on at
  once, one for each program Flatpak has running, and one made past that
  is let go, and said to be, as each holds two descriptors and a program
  may make as many as it likes.
- **A layer surface is made again as a window is** (3d): with its last
  size, anchor, margins, exclusive zone, keyboard interactivity and layer,
  on the screen and layer it was made on, committed with nothing shown,
  and shown as it was once the new compositor has said how it is to be,
  which the relay answers. Its configure is told to the program only where
  the size differs from what it was last told, as on a screen of another
  size; the relay has acked it, and the program's ack is kept from the
  compositor. A layer surface is put on the screen only by a program with
  a role, which the compositor starts with a connection of its own, so
  that is kept as the keeper is: each program the configuration starts
  with a role, from `autostart` or a key's `spawn`, has its connection
  handed to the relay, with its roles, and is handed back to each
  compositor after. One of those is marked as the user's, by a bit beside
  its roles, which the roles leave free, so that the next compositor does
  not take one with the role of the clipboard for the keeper and leave the
  keeper unstarted; the keeper is the one unmarked, as each was handed
  before there was the mark, so that a keeper kept across the update is
  still known. A part is not known by the file its process runs, as one
  that is a script runs its interpreter. The launcher is not kept, as it
  speaks to the socket for scripts, as the components do; the locker is,
  from 3e, as they are.
- **The components that come with omorfos are kept as the keeper is**
  (3d): the panel, the wallpaper, the notifications and the portal. Each
  is known again by the next compositor by its process, which the desk
  record names beside what it is, as it is said to the relay whenever a
  component is started; the relay hands the desk back before any part, so
  each part is known as it comes. Not by a new hand, nor a word added to
  `started`: the relay outlives each compositor it is updated with, and
  one that does not know a hand lets it go with its descriptor, so the
  component would lose its connection as it was started. A desk written
  before names none, so the keeper is still known by its role where the
  desk does not name it, until no relay holds one handed before. A
  component kept is not the next compositor's child: it is watched by a
  descriptor that says when its process has ended, which cannot say how,
  so its end is taken for a failure and it is started again as one that
  crashed, as often as that is allowed. Each that speaks to the socket for
  scripts makes its connection again once the next compositor is serving
  it, at the same place, asking again what it asked first, its news or
  what is wrong; the portal, which holds the session from going idle or
  to sleep for programs, asks that again as soon as the compositor is
  back, looking at its connection now and then while it holds something.
  The wallpaper's see-through surface is made again with its last
  multiplier. What asks for a secret is not kept, as a prompt kept through
  a change of compositor would not be known to the next as one it showed;
  nor are the watcher of what records and the program that watches over
  the session, which are not Wayland programs.
- **A pointer held is let go of as the compositor goes, and held again by
  the next** (3e): each lock and confinement is made again with its
  region, where the pointer is to be left, and its lifetime, and one held
  at that moment is told it is let go of, as when focus leaves; the next
  compositor holds it again as it gives the surface back the pointer and
  the keyboard, and says so. The relay does not keep that from the
  program, as it keeps the enter of the pointer, for what the program
  does when let go of, showing the pointer, is what it should do while
  no compositor has the pointer. One held once only that has been held is
  over, as the compositor before would have had it: it is not made
  again, and the relay answers for it after and frees its number, so
  that the program may ask anew. How far the pointer moves is told again
  once each relative pointer is made again for its pointer.
- **A user said to be away is said to be back as the compositor goes**
  (3e): each notification of the user being away is made again with its
  timeout, which the next compositor counts from when it starts. One that
  had said the user is away is told they are back, as the next compositor
  would not say so if they came back before its timeout, and a program
  such as swayidle would leave the screen off while the user types; it
  says they are away again once the timeout has passed. Keeping that from
  the program instead would need the relay to see the user's input, which
  only the compositor does.
- **A keyboard a program types with is made again with its keymap**
  (3e), from the descriptor the relay holds, as a key sent before a
  keymap is refused. Which modifiers it last said are held is not sent
  again: the journal keeps the last of each request in the order each was
  last sent, so a keymap sent again after the modifiers would be sent
  after them, and they would be refused.
- **A window listed is the one the program has listed** (3e). The list
  of windows (`ext-foreign-toplevel-list`) gives each window an
  identifier, which Smithay draws at random, and an object the
  compositor makes, so the next compositor lists every window anew,
  under numbers and identifiers of its own, and nothing in what it
  says names the window's surface. The compositor hands the relay,
  beside the list, which window it lists by each identifier, by the
  program's number and its toplevel's, before the programs are told
  of it. As the compositor goes, a window in a program's list that is
  kept is awaited from the next, and the program is told the rest have
  closed. A window the next lists is held back from a program awaiting
  one until the relay knows which it is, from the hand, which may come
  before or after: the one awaited is the program's own under the next
  compositor's number, and the program is told only what changed of
  it; another is new, and told as it was said. What the compositor
  hands is read before what it says to programs, so that the hand is
  most often there first. The identifier the program knows stays the
  first compositor's; one listing windows anew, as a portal picking a
  window does, is told the next's, and so is anyone asking over IPC.
- **A window imported is imported again by the next compositor's handle**
  (3e). A program shares a window with another (`xdg-foreign`) by a
  handle Smithay draws at random, which the other imports it by, as a
  portal does to make its dialog belong to the window it is for. The
  next compositor shares the window made again by a handle of its own,
  which the program sharing it is told, as it is since 3a, and knows
  nothing of the old one. As the compositor goes, each window imported by
  the handle of a window that is kept is awaited; the import is made
  again as the rest is, by the old handle, and the next compositor's
  answer that it is gone is kept from the program. Once the window is
  shared anew, the relay, which reads both handles from the program
  sharing it, tells every program, and each awaiting it imports it again
  by the new handle, under the number the program has, with the window
  made to belong to it: the import refused is destroyed first, and the
  number that frees is kept from the program, which still has it. No
  program waits for another, so two that import each other's windows
  are made again as any are; the cost is a moment in which the dialog
  belongs to no window, which matters only to one first shown then. A
  window imported whose window was not kept is told it is gone, as the
  next compositor says; one whose window is kept and never shared anew,
  as when its program is let go as it is made again, is left as Smithay
  leaves one whose window has gone, untold. A handle passed on before,
  and imported by only after, is the old compositor's, and refused.
- **An input method typing as the compositor goes is let go of the
  field** (3e). An input method is made again for its seat, with its
  keyboard grab and the surfaces of its popups. One typing in a field as
  the compositor goes is told it is not, and that what it was told is
  whole, as when the field loses the keyboard: what it was composing
  cannot be carried, as the next compositor knows nothing of it. Each key
  held in its grab is let go of, with no serial, as no compositor will
  say so. fcitx5 and IBus let go of the grab then, and take it again once
  the next compositor says they are typing, which it does once the field,
  entered again, is enabled by its program, as on any enter. The field is
  entered again whichever program is made again first, as Smithay enters
  the field that has the keyboard as an input method is made. An input
  method commits what it typed with how many times it has been told what
  it was told is whole, which Smithay checks against its own count,
  discarding what does not match; the relay counts as the program does
  and as the compositor does, the next from when it made the input method
  again, and puts the compositor's count in each commit. One told another
  is the input method is over, as a pointer held once only is: not made
  again, nor what it made, and the relay answers for each.
- **Pictures of a screen go on, and of a window once it is back** (3e).
  A source of a screen is made again for the screen the program bound,
  and each session on it, and each following the pointer, with it; one
  whose screen has gone is stopped by the next compositor, as when it is
  unplugged. A picture asked for as the compositor goes cannot be
  carried, as the next would take it of its own screen with damage of
  its own: it is told it failed, as a picture may, and the program asks
  for another, under the next compositor. One made and not yet asked for
  is made again with its buffer and what is stale in it. Each following
  the pointer is told it left, and the next says it came back. What a
  buffer has to be the next compositor says again, which a program may
  take as the screen having changed: the first it says is held back
  until it is whole, and passed on, all of it, only if it is not what
  the program was told. A source of a window names the window by its
  handle in a list of windows, which the portal lets go of as soon as the
  source is made, and the next compositor lists the window under a handle
  and an identifier of its own. As the compositor goes, the source, and
  what was made of it, is held back from being made again with the rest,
  and what the program asks of it meanwhile is kept but not sent, as for
  what no compositor has. Once the next compositor lists a window that is
  kept, which it hands the relay as it lists any window, the relay binds
  a list of windows of its own on the program's connection, under the
  next number, while the program waits so that it cannot take that
  number; finds the window there by the identifier it is listed by; makes
  the source of it under the program's number, and what was made of it,
  then asks again for each picture asked for meanwhile; and destroys its
  list and what is in it. The program waits for no other program, only
  for that roundtrip, and its pictures of the window do not come until
  the window is back. What takes pictures of a window that is not kept,
  whose program goes, which it closes, or which is listed and gone before
  it is found, is told it stopped, as when the window closes, and a
  picture of it asked for that it failed; none of it is made again, and
  the relay answers for each. One whose window is hidden, rather than
  closed, while it is awaited waits until the window is shown again.
- **A locked session is locked again by the locker it had** (3e), settled
  as the last part of 3e was started: the locker kept, rather than one
  started anew, so that what was typed into it and how it looked go on,
  where starting one anew, as the session did before, would have been
  simpler. The locker the compositor starts is handed to the relay as the
  components are, and called `locker` in the desk, so that the next
  compositor knows it for its locker and watches over it as it does them.
  Its lock is made again, and each lock surface with it, on the screen it
  was on; the next compositor, come up locked as the holder said (ADR 60),
  gives the lock nobody holds to the first program with the role to lock
  that asks for it, which is that one. The locker is not told again that
  the session is locked. A lock surface may not be committed before its
  first configure is acked, so it is not committed at all as it is made
  again: told the size it was, the relay acks it and shows what was
  shown; told another, as on a screen of another size, it is the
  locker's to ack and draw for, as what was shown would be refused at
  that size. A lock told it is over, refused as a second locker is, is
  not made again, nor what it made. Under a relay the compositor starts
  a locker only once the relay has handed over all it kept, and only if
  the locker was not among it; one kept that has not locked the session
  again within five seconds is taken to be stuck, ended, and another
  started. Whether the session is locked stays the holder's word: a
  locker that does not come back leaves it locked, and the next
  compositor starts one. A program a key starts with the role to lock is
  kept as the user's, and its lock made again too; the next compositor
  does not know it for the locker, and may start its own beside it, the
  second of them to ask for the lock being refused.
- **The compositor takes up its own new version at a quiet moment**
  (3f), settled as 3f was started, as ADR 60 has it: heard of as ADR 59
  hears of the others', and taken up by its stepping down to be started
  again, as `restart_compositor` does. A quiet moment is one with nothing
  fullscreen, nothing recording or sharing a screen, nothing keeping the
  session awake or from sleeping, as what plays does, the sealed prompt
  and the launcher closed, which go with the compositor, and no input for
  five seconds. Only once the relay has handed over all it kept, and only
  under the relay; where nothing keeps the programs, the user is still
  told the new version is used once they log in again. After an hour,
  which `update.wait` sets, and 0 makes for ever, it is taken up
  regardless, and the next compositor says so: the compositor says
  `updated` to the holder before `stepping down`, which a holder that does
  not know the word still acts on, and the holder tells it to the next
  compositor alone, carried through its own new version taken up between.
  A window of a program written for X11 holds it back however long, as
  the program would close with its X server; the user is told it waits
  for them, or for the next login. So do the screens being off, as the
  next compositor would turn them on, as at night after an install no one
  watched; the wait then starts again once the user is back, so that it
  is not taken up regardless at the first key. Otherwise the wait is from
  when the new version was first heard of, so that installing again does
  not put it off. Before any of that, the new version is run, asked only
  for its version, which it says and ends without touching the session,
  to see that it starts at all: one that cannot be run, as a file half
  written or missing what it links, or that fails, is not taken up, and
  the user is told so, while the one running stays until another is
  installed. Without this, the holder would start it, see it fail, and
  start the version it holds instead, as after a crash, with the programs
  waiting through both. A check still running as another version is
  installed is ended, and the new one checked.
- **What was kept is shown only once it is back** (ADR 62, the first
  step): the relay tells the compositor `ready` once each program it kept
  has caught up and all of it is written, and the compositor draws nothing
  until then, or for a second at most.
