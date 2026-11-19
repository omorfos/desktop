# Upstream

What is to be taken back to the projects omorfos is built on: faults in
them that omorfos works around, and what it waits on them for. Each says
what is wrong, what omorfos does meanwhile, and how to find out whether
it is still so, so that a workaround goes as soon as it can and a report
is not made of something already fixed.

Nothing here has been reported yet unless it says so. Reporting is done
by the maintainer, or by an agent the maintainer has asked to.

When a dependency is updated, run through the checks of what it is in.
When one shows the fault gone, take the workaround out, tick the roadmap
item it names if there is one, and remove the entry.

## Smithay

The checks read the source of the version in `Cargo.lock`:

```sh
S=$(ls -d ~/.cargo/registry/src/*/smithay-$(awk '/^name = "smithay"/{getline; print $3}' Cargo.lock | tr -d '"'))
```

### What is added to an object is kept without end

- **Wrong:** Smithay keeps every rectangle added to a `wl_region`
  (`RegionAttributes::rects`) and every kind a clipboard source offers,
  with no most: a program that adds a million costs the compositor a
  million. The protocols set no most, but a compositor may end a program
  that asks for more than it will keep, with `no_memory`.
- **Meanwhile:** the relay ends a program that adds more than 4096 to one
  object (`journal::MOST_ADDED`, finding 5 of
  `docs/reviews/2026-11-15-what-a-client-can-cost.md`), so a program
  connected through it never reaches Smithay with more. One connected
  without the relay, as in the nested backend or once the relay has been
  given up, is not held to it.
- **Check:** `grep -n 'rects.push\|fn add' "$S"/src/wayland/compositor/handlers.rs "$S"/src/wayland/selection/*/source.rs`
  shows whether anything counts them first.

### Every kind a program offers is sent on, however many and however long

- **Wrong:** every kind a source of the clipboard, of the selection or of
  a drag is given, when it is made or at any time after, is sent to the
  program next given the keyboard or dragged over, and to every program
  that may read the clipboard at any time. A few hundred kinds of four
  thousand bytes are more than a socket holds beside what the compositor
  keeps for a program that is not reading (ADR 45), so that program is
  ended, with whatever was not saved in it (finding 3 of
  `docs/reviews/2026-11-19-after-the-redesign-of-windows.md`). A
  compositor is told of a selection only before it is set
  (`SelectionHandler::new_selection`), cannot refuse it there, and is not
  told of a kind at all.
- **Meanwhile:** the relay ends a program that offers more than 256 kinds
  from one source, or one longer than 255 bytes (`journal::MOST_KINDS`,
  `LONGEST_KIND`), whatever the source is for. For a program connected
  without the relay, the compositor takes off the clipboard, and out of
  the selection, what is offered in more or longer, just before another
  program is given the keyboard (`Compositor::hold_what_is_offered`);
  such a program can still end the keeper of the clipboard, which is told
  as soon as it is set, and a program it drags over.
- **Check:** `grep -n 'mime_types.push' -B8 "$S"/src/wayland/selection/*/source.rs`
  shows whether a kind is counted or measured before it is kept.

### Nothing holds a program to a number of descriptors

- **Wrong:** every pool of shared memory a program makes keeps its
  descriptor in the compositor for as long as the pool lasts
  (`wl_shm.create_pool`, Smithay's `Pool`), and every buffer of the
  graphics card one for each plane (`zwp_linux_buffer_params_v1.add`);
  Smithay counts neither by program. A program that makes a thousand
  small pools has taken a thousand of the compositor's descriptors. A
  way to say the most a client may keep, or a count of what each client
  holds, ended with `no_memory` past it, would let a compositor hold
  each program to its share.
- **Meanwhile:** the compositor raises its limit to the hard one
  (ADR 93) and, out of descriptors, takes no new program on until one is
  free rather than ending; and the relay holds each program that
  connects through it to 4096 descriptors (`link::MOST_DESCRIPTORS`,
  finding 4 of `docs/reviews/2026-11-15-what-a-client-can-cost.md`). A
  program connected without the relay, as in the nested backend or once
  the relay has been given up, is held only by the limit.
- **Check:** `grep -rn 'OwnedFd\|RawFd' "$S"/src/wayland/shm/ "$S"/src/wayland/dmabuf/ | grep -i 'count\|most\|limit'`
  shows whether anything counts them.

### A sandbox's socket is taken from as though it could not wait

- **Wrong:** `SecurityContextListenerSource` sets the socket a program
  hands over not to wait, once, and takes from it until it would
  (`listener_source.rs`). Whether a socket waits belongs to every
  descriptor of it, and the program that made the sandbox holds one: it
  sets it to wait again, connects once, and the compositor's loop waits
  in `accept` for good. Taking from it on a thread, or asking `poll` each
  time with the program unable to change the answer, would not rest on
  the flag.
- **Meanwhile:** the relay answers for every sandbox made through it and
  listens on each from a thread of its own (finding 1 of
  `docs/reviews/2026-11-16-what-the-first-review-did-not-look-at.md`), so
  the compositor is never handed one. One made by a program connected
  without the relay, as in the nested backend or once the relay has been
  given up, still is.
- **Check:** `grep -n 'set_nonblocking\|accept' "$S"/src/wayland/security_context/listener_source.rs`
  shows whether it still rests on the flag.

### A second input method puts out the first

- **Wrong:** asked for a second input method while there is one,
  `InputMethodHandle::add_instance` (`src/wayland/input_method/input_method_handle.rs`)
  sends `unavailable` to the one there already and leaves the second
  untold. The protocol has the second told and the first kept. The fix is
  a line: send `unavailable` to the new object, not the one held.
- **Meanwhile:** `crates/omorfos/src/typing.rs` answers for the input
  method manager itself and refuses a second while there is one, handing
  the rest to Smithay.
- **Check:** `grep -n -A10 'fn add_instance' "$S"/src/wayland/input_method/input_method_handle.rs`
  shows which object is told. To be sure, put `delegate_input_method_manager!`
  back in place of the compositor's own `Dispatch` for the manager and the
  input method, and run
  `cargo test -p omorfos --test integration typing::a_second_input_method_is_refused_and_the_first_stays_the_one`.

### A window icon whose buffer is destroyed ends its program

- **Wrong:** `xdg-toplevel-icon` raises `no_buffer` when a buffer given as
  an icon is destroyed before the icon, which ends the program's
  connection. Programs built on Chromium and Electron, Visual Studio Code
  among them, destroy the buffer once the icon is set, and other desktops
  let it go.
- **Meanwhile:** omorfos serves the protocol itself, in
  `crates/omorfos/src/icons.rs`, and lets the destroyed buffer go
  (ADR 54).
- **Check:** `grep -n 'no_buffer\|NoBuffer' "$S"/src/wayland/xdg_toplevel_icon.rs`.
  If it is no longer raised for a buffer destroyed after the icon was set,
  serve the protocol by Smithay's again and run
  `cargo test -p omorfos --test integration switcher::a_program_that_destroys_its_icons_pixels_keeps_its_window_with_its_applications_icon`.

### Buffers in shared memory kept until the next is attached

- **Wrong:** a buffer in shared memory is kept until the program attaches
  another, with no way to let it go once its pixels have been copied to
  the card. foot says so: "compositor is not releasing buffers
  immediately".
- **Meanwhile:** nothing; it costs foot a copy of what changed. The
  roadmap item "Buffers in shared memory released as soon as they are
  drawn" says why it is not done in omorfos.
- **Check:** run `foot` in omorfos and look in the journal for that line;
  and look in Smithay's changelog, or in
  `"$S"/src/backend/renderer/utils/wayland.rs`, for a buffer released
  once imported.

### What the X11 window manager asks is left unsent

- **Wrong:** `X11Wm` leaves some of what it asks of the X server, such as
  that a selection is now held, unsent until it next asks something with
  an answer, which may be a long time. Until then no program written for
  X11 can paste what was just copied.
- **Meanwhile:** `sent()` in `crates/omorfos/src/x11.rs` asks something
  short with an answer after each, which sends everything (ADR 27).
- **Check:** take the calls to `sent()` out and run
  `cargo test -p omorfos --test integration x11::what_any_other_program_copies_or_selects_can_be_pasted_by_one_written_for_x11`.
  If it passes as often as it does with them, Smithay flushes for itself.

### Each kind offered to programs written for X11 is a wait on the X server

- **Wrong:** when a program written for X11 asks which kinds what was
  copied comes in (`TARGETS`), `X11Wm` asks the X server for an atom for
  each kind, one at a time, waiting for each answer
  (`intern_atom(false, …).reply_unchecked()` in `src/xwayland/xwm/mod.rs`).
  The X server can be waiting on the compositor meanwhile, as above.
- **Meanwhile:** the compositor hands on at most 32 kinds, each written as
  a kind is (`for_x11` in `crates/omorfos/src/x11.rs`, finding 6 of
  `docs/reviews/2026-11-15-what-a-client-can-cost.md`), so the waits are
  few.
- **Check:** `grep -n 'intern_atom' "$S"/src/xwayland/xwm/mod.rs` shows
  whether they are still asked one by one and waited on; asked all at
  once and the answers read after, the most could go.

### The primary screen can only be asked by waiting

- **Wrong:** the X server's primary screen is asked by
  `X11Wm::get_randr_primary_output`, which waits for the X server's
  answer; the X server can be waiting on the compositor at the time, which
  froze the tests when it was tried.
- **Meanwhile:** programs written for X11 are not told which screen is
  primary (the roadmap item that "Waits on: Smithay").
- **Check:** `grep -n 'randr\|primary' "$S"/src/xwayland/xwm/mod.rs` for
  a request that does not wait for its answer.

### The X11 window manager keeps its event loop, and with it the X server

- **Wrong:** `X11Wm::start_wm` (`src/xwayland/xwm/mod.rs`) puts in the
  event loop a source whose callback holds a `LoopHandle` of that loop,
  and keeps no token to take it out again. calloop 0.14 does not let go
  of its sources when the `EventLoop` is dropped, so the loop holds
  itself, and nothing in it is ever dropped: a compositor's display,
  with every program's connection, the X server's among them. The X
  server runs on until the process ends, and Smithay's thread that
  reads its events and the one that waits for it with it. A compositor
  runs once a process, but the tests start hundreds: each X server kept
  a display number of the 33 Smithay tries, and the tests ran out.
- **Meanwhile:** `Session`'s `Drop` (`crates/omorfos/src/session.rs`)
  takes the display's own source out of the loop, which drops it, so
  the X server's connection closes and it ends. The loop itself, with
  the window manager's source, is still never dropped: an event loop and
  a thread a session with an X server, in the tests.
- **Check:** `grep -n -B2 -A8 'let event_handle = handle.clone()' "$S"/src/xwayland/xwm/mod.rs`
  shows the callback holding the handle; and calloop's `EventLoop`
  having a `Drop` that clears its sources would mend it as well. To be
  sure, take the `Drop` out and run
  `cargo test -p omorfos --test integration x11::an_x_server_ends_with_its_session`.

### The surfaces of an X server that stopped are kept, and taken for the next one's

- **Wrong:** `XWaylandShellState` keeps the surface of each window of a
  program written for X11 by the number the X server gave it
  (`by_serial` in `src/wayland/xwayland_shell.rs`), to pair the two
  whichever is heard of first, and never lets go of one: not once they
  are paired, as each later commit puts it back, nor once the surface or
  the X server has gone. An X server started again in the same
  compositor numbers its windows from one again. A window of its that
  says its number over X11 before its surface does over Wayland is then
  paired with the surface of a window of the X server before, which is
  gone, and is never shown. It failed
  `x11::an_x_server_that_dies_is_started_again_where_programs_expect_to_find_it`
  in four runs of sixty in the nightly stress of 19 November 2026, on
  both architectures. A session can lose the same race: it takes hearing
  of the window over X11 first. It also keeps a little for every such
  window there has ever been, menus and tooltips among them, for as long
  as the compositor runs.
- **Meanwhile:** `recover_x11` in `crates/omorfos/src/x11.rs` takes the
  protocol's global away and makes the state anew whenever an X server
  has stopped, so the next starts with nothing kept. What is kept while
  one X server runs is left as it is.
- **Check:** mended in Smithay by `253c154` of 28 April 2026 ("xwm: clean
  up by_serial hash map when surface is destroyed"), which was in no
  release on 19 November 2026, 0.7.0 being the last.
  `grep -n 'add_destruction_hook' "$S"/src/wayland/xwayland_shell.rs`
  shows a surface taken out again as it is destroyed. Then take the two
  lines out of `recover_x11`; the count in
  `x11::an_x_server_that_dies_is_started_again_where_programs_expect_to_find_it`
  stays, and says whether it still holds.

### Xwayland cannot be started when first asked for

- **Wrong:** Smithay makes the X sockets itself, privately, as it starts
  `Xwayland`, so the compositor cannot listen on them first and start the
  X server only when a program connects.
- **Meanwhile:** `Xwayland` is started with the session (ADR 27).
- **Check:** `grep -n 'pub fn spawn\|listen\|socket' "$S"/src/xwayland/xserver.rs`
  for a way to hand it sockets already listened on.

### A window manager started against a running X server takes up none of its windows

- **Wrong:** `X11Wm::start_wm` manages the windows it hears of from then
  on. It does not ask the X server for those already there
  (`QueryTree`), nor read which surface each one is. A compositor
  started again under a running X server (ADR 60) would find its
  windows unmanaged.
- **Meanwhile:** the X server is started again with the compositor, and
  its programs close (ADR 27).
- **Check:** `grep -n -i 'query_tree' "$S"/src/xwayland/xwm/mod.rs` for
  the windows already there being taken up when the window manager
  starts.

### No safe way to write into a buffer a program shares

- **Wrong:** a program's buffer in shared memory is handed over only as a
  pointer, so writing a picture into it, as a screenshot does, needs
  `unsafe`, or a roundabout way to its file.
- **Reported:** <https://github.com/Smithay/smithay/issues/2288>, on
  14 November 2026, asking for a function that copies bytes into the
  buffer, checked to lie inside it, or else for the pool's descriptor.
- **Meanwhile:** `omorfos-shm` finds the file by where it is mapped, in
  `/proc/self/maps`, and opens again the descriptor of it that Smithay
  keeps (ADR 61). Its pictures fail if Smithay stops keeping one.
- **Check:** `grep -n -A3 'pub fn with_buffer_contents_mut' "$S"/src/wayland/shm/mod.rs`.
  In 0.7 its closure is given `*mut u8`, a pointer; it is fixed when there
  is a safe way to copy bytes into the buffer, or to be given its file.
  Not a `&mut [u8]`: the program can change the memory at any time, so a
  reference into it would be unsound. Meanwhile,
  `grep -n -A3 'struct InnerPool' "$S"/src/wayland/shm/pool.rs` must still
  show `fd: OwnedFd`; the capture tests fail if it does not.

### Tearing control is not there

- **Wrong:** no `wp_tearing_control_v1`, which games ask for to show a
  frame as soon as it is drawn.
- **Meanwhile:** not offered (the roadmap's "Smaller protocols").
- **Check:** `ls "$S"/src/wayland | grep -i tearing`.

### Importing a buffer with no damage uploads all of it

- **Wrong:** the documentation of `ImportMemWl::import_shm_buffer` says
  that empty damage lets the renderer skip the upload; with no surface
  given, the GLES renderer uploads the whole buffer each time instead,
  and keeps no texture between calls.
- **Meanwhile:** the textures of window icons are kept by omorfos, per
  buffer, in `crates/omorfos/src/icons.rs`. Nothing else here imports
  without a surface.
- **Check:** `grep -n -A40 'fn import_shm_buffer' "$S"/src/backend/renderer/gles/mod.rs`
  for what is done when `surface` is `None` and `damage` is empty. Either
  the documentation or the code is to change; the cache is worth keeping
  whichever does.

### The version a protocol is offered at cannot be chosen

- **Wrong:** each protocol's state, as `XdgShellState::new`, makes its
  global at a version written into Smithay, so a compositor offers
  whatever version its Smithay has. Only `CompositorState` lets one be
  chosen, by `new` or `new_v6`. A compositor that is to raise a version
  on purpose, as omorfos is to with its relay (ADR 60), cannot: an
  update of Smithay raises it.
- **Meanwhile:** the test of what the compositor offers,
  `protocols::a_program_is_offered_what_is_listed_at_the_version_listed_and_nothing_else`,
  fails when an update raises a version, and the update brings the list
  in `crates/omorfos-ipc/src/protocols.rs` up to date, as the
  `dependencies` routine says, rather than be held back.
- **Check:** `grep -rn 'pub fn new[a-z_0-9]*' "$S"/src/wayland | grep -i version`
  for constructors that take one. As each protocol's does, offer it at
  the version in the list, and take this entry out once all do.

### What has changed can hold empty rectangles

- **Wrong:** `OutputDamageTracker::damage_output` can give rectangles of
  no width or height. A program may add an empty rectangle to its opaque
  region, as Firefox 153 does under its toolbars; it is kept as opaque,
  and each time the regions are compared it is given back as damage, since
  `Rectangle::overlaps`, and so `intersection`, take an empty rectangle
  inside another to overlap it. The pixman renderer's `map_texture` then
  builds a slice from a null pointer when asked to read such a part, which
  is undefined behaviour.
- **Meanwhile:** `Capture::due` in `crates/omorfos/src/capture.rs` drops
  empty rectangles from what has changed before a program is told or
  anything is read, and takes a change of nothing but them for none.
- **Check:** `grep -n -A10 'pub fn overlaps' "$S"/src/utils/geometry.rs`
  for whether an empty rectangle can overlap, and
  `grep -n 'opaque_regions' "$S"/src/backend/renderer/element/surface.rs`
  for whether empty opaque rectangles are dropped. Once neither lets one
  through, take the filter out and run
  `cargo test -p omorfos --test integration sharing::a_shared_window_that_says_an_empty_part_of_it_is_opaque_goes_on_being_shared`.

### A picture stretched in software fades at its edges

- **Wrong:** the pixman renderer draws a texture with
  `set_repeat(Repeat::None)`, so its bilinear filter blends what is beyond
  the texture's edge in as nothing, and a texture stretched fades toward
  its edges; one a pixel across, stretched, fades along all of its length.
  The GLES renderer clamps to the edge, and draws it whole. `Repeat::Pad`
  is what GLES does.
- **Meanwhile:** each piece of a shadow is cut, in
  `crates/omorfos-ui/src/shadow.rs`, with a border of one pixel copied from
  its edge, and drawn from within it (ADR 33). Other pictures it stretches
  are left as they are.
- **Check:** `grep -n 'set_repeat' "$S"/src/backend/renderer/pixman/mod.rs`.
  Once it pads, the border may go: cut the pieces without it and run
  `cargo test -p omorfos --test integration shadows::a_side_of_a_shadow_is_as_dark_to_its_ends_as_in_its_middle`.

### A texture made to draw into samples round to its other side

- **Wrong:** `Offscreen<GlesTexture>::create_buffer` makes a texture
  without setting how it is sampled beyond its edges, so it is OpenGL's
  default, `GL_REPEAT`: a texture drawn into and then sampled near its
  edge, as a blur does, takes in pixels from the opposite edge. The
  textures it imports are set to `GL_CLAMP_TO_EDGE`.
- **Meanwhile:** the blur's shader, `crates/omorfos/src/shader.rs`, folds
  each sample back over the edge, as the software blur mirrors, and then
  keeps it within the middles of the pixels at the edge, which is what
  clamping to the edge gives (ADR 33).
- **Check:** `grep -n -A30 'impl Offscreen<GlesTexture> for GlesRenderer' "$S"/src/backend/renderer/gles/mod.rs | grep -n WRAP`.
  Once it clamps, the shader's `clamp` may go: take it out and run
  `cargo test -p omorfos --all-features --test integration -- --include-ignored gpu::the_cards_shader`.

### Reading back a picture of no pixels in software is undefined

- **Wrong:** `ExportMem::map_texture` of the pixman renderer
  (`src/backend/renderer/pixman/mod.rs`) makes its slice with
  `slice::from_raw_parts` from the image's data, which for an image of
  no pixels is a null pointer: undefined behaviour even for an empty
  slice, and a build with debug assertions aborts there. It should give
  an empty slice for an empty mapping.
- **Meanwhile:** glass makes no picture of an area with nothing in it
  (`Picture::new` in `crates/omorfos/src/glass.rs`), as on a screen that
  has gone, so nothing of no size is read back.
- **Check:** `grep -n -B2 -A8 'fn map_texture' "$S"/src/backend/renderer/pixman/mod.rs`.
  Once it handles an empty mapping, the guard may stay: it also saves
  making a picture of nothing.

## The PipeWire bindings (`pipewire`)

### A listener let go of after what it listens to writes freed memory

- **Wrong:** a listener (`StreamListener`, `NodeListener`,
  `ClientListener`, the registry's `Listener`) unhooks itself from what it
  listens to when it is dropped (`spa::utils::hook::remove`), but holds
  nothing that keeps that alive. Dropped after its stream or proxy, which
  frees the list it is hooked into, it writes into freed memory, from
  code with no `unsafe` of its own: unsound. A struct with the stream
  written before its listener does it, as Rust drops fields in order.
  AddressSanitizer catches it on every run; glibc only now and then, as
  `corrupted size vs. prev_size` (issue #33).
- **Reported:** <https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/work_items/123>,
  on 14 November 2026, with a fix for streams offered as merge request
  !280 (<https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/merge_requests/280>);
  for proxies the maintainers are asked which way they would have it.
- **Meanwhile:** each listener is written before what it listens to, with
  why: the portal's `Share`, the recording watcher's `Followed`, and the
  tests' own. A listener made into a local variable after what it
  listens to is dropped first already.
- **Check:** in the version in `Cargo.lock`,
  `grep -n -B2 -A3 'impl<D> std::ops::Drop for StreamListener' $(ls -d ~/.cargo/registry/src/*/pipewire-*)/src/stream/mod.rs`
  and the structs it names: fixed when a listener keeps what it listens
  to alive, or its drop does nothing once that is gone. Then the order no
  longer matters, but it does no harm.

### No safe way to hold a stream's buffer or give one memory

- **Wrong:** a buffer of a stream is held, and one is made, only through
  raw pointers (`dequeue_raw_buffer`, `queue_raw_buffer`, `add_buffer`,
  `remove_buffer`), so sharing the screen without copying needs `unsafe`.
- **Reported:** <https://gitlab.freedesktop.org/pipewire/pipewire-rs/-/work_items/124>,
  on 15 November 2026, as five changes that together take every
  `unsafe` block out: a buffer that keeps its stream and can be queued
  later, `Data::fd` as a `BorrowedFd`, a setter for a chunk's flags,
  writable cursor metadata with its bitmap, and a safe `add_buffer` that
  takes the memory's descriptor. The maintainers are asked how they
  would have the first shaped before it is written.
- **Meanwhile:** `omorfos-pipewire`, the one place `unsafe` is
  allowed (ADR 41). Wanted soon.
- **Check:** in the version in `Cargo.lock`,
  `grep -n -A3 'pub fn dequeue_buffer\|pub fn add_buffer' $(ls -d ~/.cargo/registry/src/*/pipewire-*)/src/stream/mod.rs`.
  In 0.10 `dequeue_buffer` gives a `Buffer<'_>` borrowed from the stream,
  which cannot be held past the callback, and `add_buffer`'s callback is
  given `*mut pw_buffer`. It is fixed when a dequeued buffer can be held
  and queued later without a raw pointer, and a buffer can be given
  memory of the caller's own safely.
