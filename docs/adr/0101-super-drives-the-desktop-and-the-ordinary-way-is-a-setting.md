# 101. Super drives the desktop, and the ordinary way is a setting

- Status: accepted, as changed below, on 24 November 2026
- Date: 2026-11-20
- Takes up: [ADR 78](0078-super-belongs-to-the-desktop.md)

## Context

ADR 78 gave Super to the desktop and handed any chord with Super that
nothing binds to the program as Ctrl, so that Super+C copies as Cmd+C
does on macOS. What it left was a list: the roadmap's review of the
defaults, and its item on Super as the key omorfos is driven by,
everywhere, with a way back to Ctrl and Alt as Windows, GNOME and KDE
have them. [The review of the defaults](../reviews/2026-11-20-the-defaults.md)
goes through each chord against those four, and through what else had
gathered: locking, the overview, half a screen, the next layout, the
shell's own fields, how many fingers swipe.

Super already stands where Command does. The rest follows from taking
that seriously: a hand that knows a Mac should find the desktop where
it left it.

## Decision

**macOS first.** Each chord is the Mac's, with Super for Command, and
with Super+Ctrl wherever the Mac uses Ctrl alone for the desktop, as
Ctrl here is the programs'. GNOME, KDE and Windows are followed only
where the Mac has nothing, and then only where a program's shortcut
would not be taken.

Programs and their windows:

- **Super+Tab switches programs**, as Cmd+Tab: the switcher shows each
  program once, by the window of it used last, and choosing one brings
  all its windows on the workspace forward. Super+Shift+Tab goes back.
  Today it switches windows; that becomes new actions, `switch_programs`
  and `switch_programs_back`, and `switch_windows` stays for whoever
  binds it.
- **Super+\` goes to the next window of the program in front**, as
  Cmd+\`, and Super+Shift+\` to the one before: new actions,
  `next_window` and `previous_window`, among the program's windows on
  the workspace.
- **The tabs of a group of windows move to Super+Ctrl+Tab** and
  Super+Ctrl+Shift+Tab, where Super+\` had them: a Mac goes through its
  windows' tabs with Ctrl+Tab, which here is the program's own.
- **Super+Q quits the program**, as Cmd+Q: every window of the program
  in front is asked to close, each as the close button asks, so that a
  program may ask to save. A new action, `close_program`; `close` stays,
  for one window. A program that keeps running with no window, as one
  in the tray does, goes on running.
- **Super+W is the program's**, as Cmd+W is the app's: it reaches the
  program as Ctrl+W, which closes a tab where there are tabs and the
  window where there are not, as a Mac's apps do. A window that does
  nothing with it is closed by its button or with Super+Q.
- **Super+M minimizes**, as Cmd+M, where Super+H does today. **Super+H
  hides the program**, as Cmd+H: every window of it minimized, a new
  action `hide_program`; and **Super+Alt+H hides every other program**,
  as Cmd+Option+H, `hide_others`. Maximizing leaves Super+M for **filling
  the screen with Super+Alt+Up**, below.
- **Full screen: Super+Ctrl+F**, as Ctrl+Cmd+F, as now.

The desktop:

- **The launcher: Super+Space**, as Cmd+Space, as now.
- **The overview: Super+Up**, as Ctrl+Up opens Mission Control, and
  `XF86LaunchA`, the Mission Control key, as now. **Super+Down shows the
  program's windows**, as Ctrl+Down shows App Exposé, once the overview
  can show one program's alone.
- **Super tapped alone does nothing**, as Command alone does nothing. A
  thumb resting on it opens nothing.
- **Workspaces: Super+Left and Right**, as Ctrl+Left and Right, as now,
  and Super with a digit to go to one, as Ctrl with a digit. **A window
  is sent to one with Super+Ctrl and the digit**, where Super+Shift has
  it today; the Mac has no key for it, and Super+Shift+3, 4 and 5 are
  wanted below.
- **Half a screen: Super+Alt+Left and Right, and Super+Alt+Up to fill
  it**, as macOS's tiling has Fn+Ctrl with the arrows: Super+Ctrl and
  the arrows already go between windows beside each other.
- **Locking: Super+Ctrl+Q**, as Ctrl+Cmd+Q, and the keyboard's own lock
  key (`XF86ScreenSaver`).
- **Logging out: Super+Shift+Q, which asks first**, as Shift+Cmd+Q, with
  `log_out` (ADR 88). `Super+Shift+E`, `quit`, which ends the session
  without asking, is taken away.
- **The next layout: Super+Alt+Space**, as Ctrl+Option+Space, and
  `XF86Keyboard`. Ctrl+Space, the Mac's, is a program's here.
- **Pictures of the screen: Super+Shift+3 for every screen, Super+Shift+4
  to pick, Super+Shift+5 for the picker's choices**, as Shift+Cmd+3, 4
  and 5, beside Print, Shift+Print and Alt+Print as now. Ctrl+Shift+3, 4
  and 5 are hardly any program's.
- **The touchpad on and off: `XF86TouchpadToggle`**, once there is the
  action.

The touchpad, as a Mac's:

- **Three fingers or four**, either, left and right between workspaces,
  up to the overview and down to the program's windows; three do today.
- **A pinch of the thumb and three fingers** opens the launcher with
  every program shown, as macOS opens its Apps, which took Launchpad's
  place in macOS 26; **a spread** shows the desktop.
- **A page of settings for the touchpad**, which shows each gesture
  moving and turns it off or changes its fingers, as the Mac's has: for
  the roadmap.

The shell's own fields, the launcher's and the boxes that ask for a
password, **read Super as a Mac's read Command**: Super+C copies, Super+A
chooses all. Ctrl is left for what a Mac's fields do with it, Ctrl+A to
the start of the line and Ctrl+E to the end, once that is built. Today
they read both alike.

**The ordinary way is one setting, in `keys.yaml`**: its `defaults`,
which already chooses the built-in bindings, takes `ordinary` beside
`true` and `false`, `true` staying the `super` way. So keys.yaml stands
alone (ADR 3): which `unbind` is a mistake is told as the file is read,
and every reader of the bindings, the compositor, the welcome card and
the manual, has them from one `Keys`. ADR 73 counts a value added as
nothing broken.

- In `ordinary`, a chord with Super that nothing binds reaches the
  program as it is, not as Ctrl, and Ctrl chords are the programs' as
  typed. Alt+Tab switches windows and Alt+F4 closes one, Super+L locks,
  Super+Left and Right take half the screen, the workspaces move to
  Super+Page Up and Page Down, as GNOME; the shell's fields read Ctrl.
  The launcher stays on Super+Space and the overview on Super+Up.
- Alt+Tab and Alt+F4 are the ordinary way's alone: in the `super` way
  Super+Tab and Super+Q do what they would.
- `keyboard.super_as_ctrl` keeps its meaning, what programs are given,
  and nothing else, for as long as it is there. It is not read as the
  ordinary way, which would move the bindings of whoever set it false
  in the release that did so (ADR 73). Taken away later, it is read and
  passed over with a notice that it is no longer read, never refused
  with its file: a refused config.yaml, and its kept copy that says the
  same, would put the session on the defaults as a new version is taken
  up.
- A chord in `keys.yaml` is the user's in either way.

Not decided here, and still the review's to take up: whether the side
of a titlebar's buttons is a setting (ADR 97), the question's minute
(ADR 88), a tooltip's half second (ADR 87), a key for the screens on a
keyboard without the Display key, and the built-in bindings with no
Mac's to follow (`Super+Return`, `Super+Escape`, `Super+Ctrl+N`,
`Super+Shift+R`), some of which are programs' shortcuts as Ctrl. A Mac's
Cmd+Option+Escape, to end a program that does not answer, would be
Super+Alt+Escape, once there is a list to end one from.

The star key, `XF86Favorites`, stays unbound, written down in the guide
as the user's. What is kept on the clipboard stays for the session
(`clipboard.forget: 0`), and the launcher's icons and descriptions stay
on.

## As accepted

The maintainer went through the proposal on 24 November 2026 and took all
of it, as written above, but for these:

- **Super+W** is the program's, as proposed; to be clear about it, it
  reaches the program as the Ctrl+W it knows, the program deciding what
  that does, as a Mac's app decides what Cmd+W does. A raw Super+W would
  reach programs that bind nothing to it. Ctrl+W typed is the program's
  as typed.
- **The next layout has no chord by default**: Super+Alt+Space is not
  bound, and the user binds one in `keys.yaml`, or in Settings once there
  is one. `XF86Keyboard`, a key that is there for nothing else, keeps it.
- **The root menu has no chord by default**: Super+Escape is no longer
  bound to it. It opens on a right click on the desktop, and the bar's
  menu of stage 5 takes its place for the system's things (ADR 108).
- **Reloading the configuration has no chord by default**: Super+Shift+R
  is no longer bound to it, as it is Ctrl+Shift+R in a browser, a reload
  without the cache, and Cmd+Shift+R on a Mac. The configuration is read
  again as its files are saved, and `reload_config` stays in the menu.
- **Super+F1 has the screens stand the next way**, beside the Display key
  (`XF86Display`), which many keyboards have not: the Mac's Cmd+F1 turns
  mirroring on and off.

And what the proposal left to the review:

- The side of a titlebar's buttons becomes a setting (ADR 107).
- The question's minute (ADR 88) and a tooltip's half second (ADR 87)
  stay as they are, fixed.
- `Super+Return`, the terminal, and `Super+Ctrl+N`, a new workspace, stay
  as they are.

## Alternatives

- **The best of each**, GNOME's overview on a tap of Super, KDE's three
  or four fingers, Windows' Super+Shift+S: what was first proposed. It
  asks a hand to learn a mixture that no one already knows, where Super
  as Command makes one of them the natural one to follow.
- **Super+Tab switching windows, as now**, and Super+\` the tabs of a
  group. It keeps what is built, but a Mac's hand presses Super+\` for
  the program's next window and gets a tab, or nothing.
- **Super+Q closing one window, as now.** It is what a single window
  wants, but a program with several is quit with one press on a Mac, and
  one by one here.
- **Super+W closing the window, as the desktop's.** Tabs could no longer
  be closed from the keyboard, as Ctrl+W is gone from Super.
- **Super+L to lock, as GNOME, KDE and Windows.** Ctrl+L, the address bar
  in every browser, would be gone from Super.
- **Super tapped opens the overview, as GNOME, or the launcher, as KDE
  and Windows.** Neither is the Mac's, and either opens under a resting
  thumb.
- **`super_as_ctrl` kept as the only switch.** It leaves the bindings
  where the `super` way wants them, so one who turns it off has Super+Q
  close and no Alt+F4: neither way whole.
- **The way in `config.yaml`**, as `keyboard.keys`. It would decide
  which `unbind` in keys.yaml is a mistake from another file.

## Consequences

- Keys move that are under hands already: Super+M minimizes rather than
  maximizes, Super+H hides the program rather than the window, Super+\`
  goes through windows rather than tabs, Super+Q closes every window of
  the program, Super+Shift with a digit is no longer a workspace's, and
  `Super+Shift+E` no longer ends the session. The release that does it
  says so, and a user's `keys.yaml` can put any back.
- Seven actions are new: `switch_programs`, `switch_programs_back`,
  `next_window`, `previous_window`, `close_program`, `hide_program` and
  `hide_others`, each in the guide's table and the schemas.
- The switcher learns to show programs, and the overview, later, one
  program's windows.
- A terminal reads Ctrl+W as deleting a word, so Super+W there does not
  close its tab, as it does on a Mac; that is ADR 78's matter of
  terminals, not this.
- `defaults: ordinary` is a new value of keys.yaml. `super_as_ctrl`
  stays as it is; if it is taken out later, that release is marked
  breaking (ADR 73) and the key is passed over with a notice.
- Every chord is to be tested in both ways, as the roadmap asks.
- Nothing of this is built yet; until it is, the bindings are ADR 78's.
  The roadmap's stage 4 lists what is to be built.
