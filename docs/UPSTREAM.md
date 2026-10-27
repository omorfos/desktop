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

### The primary screen can only be asked by waiting

- **Wrong:** the X server's primary screen is asked by
  `X11Wm::get_randr_primary_output`, which waits for the X server's
  answer; the X server can be waiting on the compositor at the time, which
  froze the tests when it was tried.
- **Meanwhile:** programs written for X11 are not told which screen is
  primary (the roadmap item that "Waits on: Smithay").
- **Check:** `grep -n 'randr\|primary' "$S"/src/xwayland/xwm/mod.rs` for
  a request that does not wait for its answer.

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
- **Meanwhile:** `omorfos-shm` finds the file by where it is mapped, in
  `/proc/self/maps`, and opens again the descriptor of it that Smithay
  keeps (ADR 61). Its pictures fail if Smithay stops keeping one.
- **Check:** `grep -n -A3 'pub fn with_buffer_contents_mut' "$S"/src/wayland/shm/mod.rs`.
  In 0.7 its closure is given `*mut u8`, a pointer; it is fixed when there
  is a way given a `&mut [u8]` or the buffer's file instead. Meanwhile,
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

## The PipeWire bindings (`pipewire`)

### No safe way to hold a stream's buffer or give one memory

- **Wrong:** a buffer of a stream is held, and one is made, only through
  raw pointers (`dequeue_raw_buffer`, `queue_raw_buffer`, `add_buffer`,
  `remove_buffer`), so sharing the screen without copying needs `unsafe`.
- **Meanwhile:** `omorfos-pipewire`, the second place `unsafe` is
  allowed (ADR 41). Wanted soon.
- **Check:** in the version in `Cargo.lock`,
  `grep -n -A3 'pub fn dequeue_buffer\|pub fn add_buffer' $(ls -d ~/.cargo/registry/src/*/pipewire-*)/src/stream/mod.rs`.
  In 0.10 `dequeue_buffer` gives a `Buffer<'_>` borrowed from the stream,
  which cannot be held past the callback, and `add_buffer`'s callback is
  given `*mut pw_buffer`. It is fixed when a dequeued buffer can be held
  and queued later without a raw pointer, and a buffer can be given
  memory of the caller's own safely.
