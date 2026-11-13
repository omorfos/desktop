# 78. Super belongs to the desktop

- Status: accepted
- Date: 2026-11-13

## Context

The built-in bindings already hold most chords with Super: close,
maximize, the launcher, workspaces. A chord with Super that nothing
bound went to the focused program as it was, where it meant whatever
that program made of it, which was mostly nothing: programs bind their
shortcuts to Ctrl, and leave Super to the desktop.

The maintainer, coming from macOS, asked for copy, paste, undo and the
rest on Super, as Cmd has them there, and for the switcher on Super+Tab,
as Cmd+Tab is. Each could have been a binding of its own, but together
they ask what Super is for.

## Decision

**Super is the desktop's.** Every chord with Super held is omorfos's to
read, and a program never sees a key pressed while Super is held:

- **A chord bound**, built in or in `keys.yaml`, does its action, as
  before.
- **Any other** is handed to the focused program as the same chord with
  Ctrl in place of Super: Super+C copies, Super+V pastes, Super+Z undoes,
  Super+Shift+T is Ctrl+Shift+T, in every program, as Cmd is on macOS.
  The program is told the modifiers changed for that key alone, Super
  taken out and Ctrl put in, and as they are once it is let go.
- **In a terminal**, where a Ctrl chord goes to the shell and Ctrl+C
  interrupts, the chord is handed on as Ctrl+Shift, which is what
  terminals copy, paste and open tabs with. A window is a terminal by its
  program, from a list in `config.yaml` that has the common ones.
- **It can be turned off** in `config.yaml`, for a chord with Super that
  nothing bound to reach the program as it is, as before.

The built-in bindings follow from it. What macOS gives programs on Cmd is
left to them: fullscreen moves to Super+Ctrl+F (find is Super+F), a new
workspace to Super+Ctrl+N and sending a window to one to
Super+Ctrl+Shift+N (a new window is Super+N, a private one in a browser
Super+Shift+N). Super+Q, Super+H and Super+M stay, as they mean there what
they mean here. The switcher is on Super+Tab and Super+Shift+Tab, as well
as Alt+Tab; going through the tabs of a group moves to Super+\` and
Super+Shift+\`, as Cmd+\` goes through an application's windows. The
overview is on Apple's Mission Control key (`XF86LaunchA`).

## Alternatives

- **Only a few chords handed on**, as Super+C, X, V and Z. Less to
  surprise anyone, but every other shortcut a program has (find, save,
  print, new tab) would be on Ctrl again, which is half of what was asked.
- **An action that sends keys**, as `{ send: Ctrl+c }`, bound chord by
  chord. It would let a binding send anything, but each of a program's
  shortcuts would need a binding of its own, and the built-in ones would
  be a long list of the same thing.
- **Super left to programs, as it was.** Nothing to build, but a chord
  with Super would mean one thing where it is bound and something else,
  or nothing, where it is not, and no shortcut of a program would be on
  it.

## Consequences

- A program that needs Super itself, as a virtual machine, a remote
  desktop or a game may, never gets it. The way for one to have the
  whole keyboard is `keyboard-shortcuts-inhibit`, which omorfos does not
  offer yet; it is on the roadmap.
- A chord bound in `keys.yaml` takes it from every program: Super+S bound
  there is no longer save anywhere.
- Ctrl chords reach programs as they are typed: Ctrl+C in a terminal
  still interrupts.
- Those used to Super+F for fullscreen and Super+N for a new workspace
  find them on Super+Ctrl.
