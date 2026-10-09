# 10. Hardware backend

- Status: accepted
- Date: 2026-10-09

## Context

To be a desktop, omorfos has to drive the screen and read the keyboard and
mouse itself, not borrow a window from another compositor. On Linux that
means DRM/KMS for the screen, libinput for input devices and a seat manager
for permission to use either. Smithay wraps all three.

Three constraints shape how. Our crates forbid `unsafe`. Code that needs a
real seat cannot run in CI, so it has to be thin (ADR 2). And the first
version has to be small enough to get right without being able to test it
automatically.

## Decision

**A second backend, `backend/hardware.rs`, beside the nested one.** It adds
libinput, the seat and the graphics card to the `Session`'s event loop
(ADR 9) and draws after each turn. Like the nested backend it is excluded
from coverage and holds no policy.

**The backend is chosen from the environment.** With `WAYLAND_DISPLAY` or
`DISPLAY` set there is a session to open a window on, so omorfos runs
nested; with neither it drives the hardware. The choice is a pure, tested
function, and there is no flag to get wrong.

**libseat for the seat.** It works with logind and with seatd, needs no
setuid binary of ours, and gives back the screen and devices when the user
switches virtual terminal.

**OpenGL through Smithay's multi-GPU manager, even with one card.**
Creating an EGL display or a GL renderer directly is `unsafe` in Smithay.
`GpuManager` with `GbmGlesBackend` does both behind a safe interface. It
is more machinery than one card needs, and it is what a second card will
need anyway.

**Buffers are swapped by `GbmBufferedSurface`.** It hands out a buffer and
its age, which is exactly what `Compositor::render` takes, so the
compositor draws the same way on hardware, in a window and in tests.
Smithay's newer `DrmCompositor` can put windows on hardware planes without
copying them, but wants to do the damage tracking and composition itself;
adopting it means reworking `render`, and is left until it is worth it.

**What a bare screen lacks is the compositor's job, and tested.** Three
things a host session used to provide are now in `Compositor`: relative
pointer motion that stops at the screen's edges (`Input::MotionBy`), a
drawn pointer (`Compositor::show_pointer`), and the keys that switch
virtual terminal, which are kept from applications and bindings and handed
to the backend (`Compositor::take_terminal_switch`).

**Frames are paced by the screen.** A frame is drawn when something has
happened and none is waiting to be shown; applications are told to draw
again when it reaches the screen. If nothing changed there is no such
moment, so they are told one frame's time later instead. An idle screen
draws nothing.

**One screen at first:** the first one found connected, at the mode it
prefers. ADR 15 added the rest, and hotplug.

## Consequences

- Building needs `pkg-config` and the development files for udev,
  libinput, libseat and GBM, in CI and for packagers. The binary links
  them, so the packages depend on them even for nested use.
- The backend has no tests. It has been run by hand on one laptop, where
  the screen, pointer, keyboard, a terminal and a web browser worked. The
  risk is contained by what it does not decide: everything it calls is
  covered.
- Applications can only give the compositor shared-memory buffers. Those
  that insist on the GPU need the dmabuf protocol, which is not offered
  yet.
- The pointer was at first always our arrow; ADR 12 gave applications and
  cursor themes their say. It is drawn into the frame instead of on a
  hardware cursor plane, so moving it redraws a little of the screen.
- Touchpad settings from `config.yaml` are given to touchpads when they
  appear and again when the configuration changes. This is glue and
  untested like the rest of the file.
- Everything was first drawn at one pixel per pixel, which is small on a
  dense screen. ADR 11 added scaling.
- The screen's refresh rate is reported to applications as 60 Hz whatever
  it is.
- Applications get a shared-memory buffer back only when they attach the
  next one, which is how Smithay's buffer handling works. Some keep a
  second buffer because of it; foot says so in a warning.
- A graphics card plugged in later is not noticed. Screens are, since
  ADR 15.
