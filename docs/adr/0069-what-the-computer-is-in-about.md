# 69. What the computer is, in "About", and a copy of it

- Status: accepted
- Date: 2026-11-09

## Context

Someone reporting a fault is asked what they run it on: the processor,
the memory, the graphics card, the system and kernel, the screens. Each
desktop has a page that says so; omorfos had "About" (stage 4), which
said only what omorfos is and what it is made with. And what is shown
has to be typed out again unless it can be copied.

## Decision

"About" says what the computer is, in a table above the credits, and
has a button that puts the same on the clipboard as plain text, a line
each, after the version of omorfos.

- **What.** The processor's name and how many threads it runs; the
  memory the system has, in powers of 1024 (`31.1 GiB`), which is a
  little less than is fitted; each graphics card and its driver; each
  internal disk and its size, in powers of 1000 as disks are sold
  (`1.0 TB`), leaving out what can be taken out and what is the
  kernel's own (loop, memory, CD, device-mapper and RAID devices); the
  system as `/etc/os-release` names it and the kernel's version; and
  each screen, its connector, pixels, refresh and scale. A kind of
  which nothing could be read is left out.
- **Nothing that tells one machine from another.** No name of the
  machine or its user, no serial number, no address: the text is made to
  be pasted into public bug reports.
- **Where it comes from.** `/proc` and `/sys`, read once as "About"
  opens, not each time it is painted again, as it is under the pointer;
  the screens from the compositor. What to make of it is
  `omorfos_core::System`'s, on the text of those files, so that every
  case is tested on any machine, ARM's `cpuinfo` without a model name
  among them; the compositor only reads.
- **Graphics cards** are named from `pci.ids`, which `hwdata` or
  `pci.ids` installs, by the maker's short name and the name in square
  brackets the list gives the card, where it gives one: `Intel
  Corporation UHD Graphics 620`, `AMD/ATI Phoenix1`. Without the list,
  the numbers. The name a card is sold under, "Radeon 780M", is not in
  the list; only the graphics driver says it, through OpenGL, which the
  hardware backend alone has and no test can reach. That waits.
- **The clipboard.** The compositor offers the text itself, in the forms
  programs written for Wayland and for X11 ask for. The selection's data
  says whose it is: a program written for X11's, which the X server is
  asked for, or the compositor's own text, which is written straight to
  whoever pastes it. It is far shorter than a pipe holds, so it is
  written at once, on no thread of its own. Programs written for X11 are
  told of it as they are of what a program copies, since Smithay tells
  the compositor only of selections programs make.
- **The button** is the theme's `about-button`, under the close button at
  the foot of the mark, where it does not scroll with what is listed.

## Consequences

- A fault report can carry what it was found on in two clicks.
- `pci.ids` is recommended by the packages; without it a card is its
  numbers.
- Nothing says the text was copied, and Ctrl+C does not copy it; both
  are on the roadmap, with the cards' names from the driver and the
  screens' names from their EDID.
