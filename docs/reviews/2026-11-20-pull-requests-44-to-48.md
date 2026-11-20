# Pull requests 44 to 48, reviewed

- Date: 2026-11-20
- Scope: five open pull requests, each one commit on `main` at 5805a01:
  #44 (the scale rule of ADR 100), #45 (the month's first day of the
  week), #46 (the review of the defaults and ADR 101, proposed), #47 (the
  protocols programs lack) and #48 (the compositor ending with an X
  server mid-transfer).
- Asked of: all eight reviewers (security, architecture, components,
  performance, reliability, experience, tests, docs). Performance,
  experience and docs ran on Fable; the other five on Opus, after Fable's
  limit was reached and its safeguards stopped the first security run.
- How it was shown: nothing of the workspace could be built where this
  was written. `scale.rs` was compiled and tested alone, with faults put
  in one at a time; `month.rs`'s date logic with a stand-in for jiff;
  `tools/scale-survey.sh` as it is; #45's tables against glibc 2.39's
  locales; the other systems at the commits the documents link. Each
  finding below was checked against the code it names before it was
  written up.

## Findings, highest first

### 1. Any program written for X11 can end the compositor at will

High. Of `main`; #48 writes it up as a race at an X server's end, which
is less than it is.

- **Where:** `crates/omorfos/src/x11.rs`: `disconnected` lets the manager
  go, and `xwm_state` `expect`s one. Smithay 0.7.0 writes the manager's
  `XwmId` into the X server's client data as it starts
  (`src/xwayland/xwm/mod.rs`, `insert_if_missing`) and never takes it
  out; its commit hook (`serial_commit_hook`,
  `src/wayland/xwayland_shell.rs`) asks for the manager on every commit
  of a surface that ever had a serial, which `Cacheable::commit` keeps
  for good.
- **What happens:** a program sends the core X request `KillClient`,
  naming the window that owns `WM_S0`, which X lets any client ask of
  any other. The manager's connection closes, `disconnected` lets it go,
  and the X server goes on: `manage_x11` sees a live client and starts
  nothing. The next frame any program written for X11 draws reaches
  `xwm_state` and ends the compositor. Done three times in a minute it
  ends the session (ADR 60). A program in a sandbox with X11 is enough,
  and very likely one with the network alone, as Xwayland listens on the
  abstract socket with no `-auth`.
- **Shown:** by reading omorfos and Smithay; `KillClient` closing
  another client's connection while the server runs was seen on Xvfb.
  Xwayland's own behaviour is from memory. To be shown end to end by the
  test below.
- **To change:** keep the last manager rather than drop it, and look one
  up by `XwmId`, which is never reused; when the manager of a running X
  server is lost, end that X server's Wayland client, as dropping
  `XWayland` does, so that it is started again as one that stopped is,
  within the restarts allowed. `manage_x11` waits on a mark that the old
  manager has heard the last of its X server, not on the field being
  empty, or a kept manager would hold the next X server back (findings
  of architecture and reliability). kwin ends its X server the same way.
  Worth raising with Smithay too: cosmic-comp has the same `unwrap`.
- **Test first:** in `tests/integration/x11.rs`, a program that kills
  the manager's connection and then maps a window: today the step
  panics with "no X11 window manager"; after, a new X server is there and
  a program can open a window on it. And the same in the middle of a
  paste, with the Wayland end held back until the manager is gone.

#48 should be reworded before it is merged: who (any program written
for X11, on purpose), when (at will, and at any commit afterwards), the
to-do above in place of "until nothing of it is under way" (Smithay
cannot be asked, and a paste that never ends would keep X11 down), and
the commit hook added to the `docs/UPSTREAM.md` entry.

### 2. #44: windows come back at their old places when the new rule is taken up mid-session

Medium. Taking up an update under the relay starts the next compositor,
which guesses each `auto` screen by the new rule; screens beside one
whose scale changed move, and windows are put back at their places in
the old layout (`crates/omorfos/src/kept.rs`, `Desktop::add_window_kept`).
Some land on the next screen, or off every screen. By reading; to be
shown by a relayed test with two screens and a scale that changes.
To change: a kept frame whose origin is not on its own screen goes to
that screen's usable origin; or build the guess kept (the roadmap's
next step) before this rule ships.

### 3. #46: ADR 101 as written breaks ADR 73 twice, and puts keys.yaml at config.yaml's mercy

Medium, as a proposal. Reading `super_as_ctrl: false` as `keys: ordinary`
gives a promised value another meaning in the release that does it, not
the one after; and taking the key away later would have every
config.yaml that names it refused, and the session put on the defaults
mid-session by the swap, as the kept copy is refused too. A key taken
away has to be read and passed over with a notice. And a way chosen in
config.yaml would decide which `unbind` in keys.yaml is a mistake,
against ADR 3; `defaults: ordinary` in keys.yaml itself does not.

### 4. #46: what the review says of omorfos today, and what it leaves out

Medium for what is left out, low for the rest.

- Four statements of omorfos are not so: three fingers up already open
  the overview; a tap of Super is forwarded to the program
  (`input.rs`, held by a test); the shell's fields already read Super
  and Ctrl alike (`typing.rs`); `Shift+Print` is the picker and `Print`
  saves. Two proposals are of what is already so.
- Decisions handed to this review by name are not in it: `Super+Shift+E`
  ends the session without asking (ADR 88), and in the Super way it is
  Ctrl+Shift+E in programs; the side of a titlebar's buttons (ADR 97);
  the question's minute (ADR 88); half the built-in bindings.
- `Super+Shift+S` for a picture takes Save As from every program,
  against the ADR's own rule; Super+Ctrl+S is free.
- Cells to correct: GNOME's fingers (three or more), GNOME's Shift+Print
  (copies and saves), KDE's launcher (Meta tapped), KDE's display key
  (kscreen, Meta+P), KDE's switcher (Meta+Tab too), and both lock with
  `XF86ScreenSaver`.

### 5. #47: three protocols are not "a matter of offering", and others need holding to a role

Medium, as a proposal.

- `fifo`, `commit-timing` and `linux-drm-syncobj` hold a program's
  commits back until the compositor lets them go, which nothing in
  omorfos does; offered as they are, a program stops at its first
  frame. Once built, a window not shown needs something to wake for it,
  and each needs a bound: commits kept without one, and with syncobj a
  descriptor each. Under the relay, a release point no compositor will
  signal holds a program's buffer for good.
- `xwayland-keyboard-grab` built on Smithay's default handler gives any
  program written for X11 every key typed anywhere; it is to be held as
  kwin holds it, never moving the keyboard.
- `ext-workspace` to the `panel` role, `drm-lease` to `non-desktop`
  screens and revoked as the session locks, `pointer-warp` to the
  surface the pointer is in, and the bell to the ringing window.
- `tablet-v2` and `drm-lease`: what the compositor makes by events is
  not made again by the relay, which the relay's guard does not check.
- Cells: sway has no pointer-warp; COSMIC no wlroots screencopy;
  Hyprland offers `wl_drm`; niri offers the virtual pointer. Rows left
  out: `wlr-data-control`, `xdg-session-management`,
  `color-representation`, `wl_fixes`. A pen gets nothing on omorfos
  today, not a mouse. GTK asks only by KDE's decoration protocol.

### 6. #44 and #45: low

- The guide says a screen read from near is drawn "a little larger"; it
  is smaller. "The number it was" is nowhere to be seen; say how to
  find it.
- ADR 100 still says "still to be built", its "Today" column and the
  script's `omorfos_today` name a rule #44 takes away, and the roadmap
  speaks of it as today's.
- The guide's link to ADR 100 is dead where the guide is installed.
- A `mode` changed while running keeps the guess for the old one, under
  the 720 units promised.
- The rule is written twice, in Rust and in the awk the ADR is held to,
  and nothing ties them: one test reading ADR 100's tables, and the ADR
  reaching `omorfos-ui` in `affected.sh`, would.
- Faults that pass every test of `scale.rs` (shown): the 20-inch line
  anywhere from 18 to 23, the television line, a size on its side, a
  missing width, `KEPT_BACK` at 700, the desk's aim, the lines to a
  scale, a guess kept back below 1 or a half at a time. The tests
  reviewer wrote six groups that catch every one a test can, and a test
  of the `u32` overflow over 2^30 lines.
- #45's guide leaves out `LC_ALL`, which is asked first; the roadmap's
  gap names what is done; the order the day names are drawn in is
  tested only through a private helper; the panel's integration tests
  read the runner's locale; the locale is read by a fourth copy of the
  same lookup (to share in `omorfos-i18n`); `first` names three things.

## Dropped

- Nothing was dropped as untrue. Where two reviewers found the same
  thing, it is written once above.

## Looked at and sound

- #44's rule against ADR 100: the same answer as the script's on some
  900,000 screens; never outside 1 to 4 for any `u16` mode; no
  allocation; asked once a screen.
- #45's tables against glibc 2.39: every one of 148 territories.
- #46's GNOME and KDE cells, but for those above.
- #48's two paths: the only ways in after the connection closes.

## Not looked at

- Anything on real hardware or a screen; Xwayland's own source; jiff,
  calloop and GTK, which are not on the machine; Apple's and Microsoft's
  pages, from memory.

## Seen in passing, for later

- Any program written for X11 can likely freeze the compositor with
  `GrabServer`, as the roadmap's RandR freeze did.
- Xwayland on the abstract socket with no `-auth` lets a sandbox with
  the network reach every program written for X11.
- The welcome card says Super+C copies whatever `super_as_ctrl` is.
- `omorfos-ask` takes an empty `LC_ALL` as set and never reads
  `LC_MESSAGES`.
