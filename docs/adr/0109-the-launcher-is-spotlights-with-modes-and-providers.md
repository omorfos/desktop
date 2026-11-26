# 109. The launcher is Spotlight's, with modes, and others' results through providers

- Status: accepted
- Date: 2026-11-25

## Context

The launcher (ADR 22) starts programs by name, works out sums (ADR 86),
and since #157 opens onto every program as a grid. Tried on 25 November
2026, the maintainer asked for it to become one launcher in the manner of
macOS's Spotlight, and more: to look, feel and behave as Spotlight does,
and to be a frame that more can be added to, by omorfos and by other
programs, rather than a list of features built in one by one.

What was asked, in the maintainer's words where they were given:

- One launcher on `Super+Space`, as Cmd+Space is a Mac's.
- Modes chosen with `Super+1`, `2` and on, while it is open: programs
  (the grid), files, actions, the clipboard's history, and commands,
  which move from `>` (planned on the roadmap, not built) to `Super+5`.
- The modes also picked by the pointer: they show as soon as it is over
  the launcher.
- In each mode, words of its own in the empty field, and its icon before
  the field. Large text, and one glass, uniform.
- By default a generic mode, which finds whatever matches and shows it
  in sections, as Spotlight does: chosen, of the two ways offered, over
  one that guesses a single kind and shows only that.
- Each mode turned on or off, and put in the order wanted, in the
  configuration.
- Actions: omorfos's own, and programs' own, as "Create a new note",
  "Open a new terminal tab", "Play the next song"; and the user's own,
  written in the configuration with a title, an icon and a description
  where wanted.
- The look and feel of Spotlight, enhanced, not copied.

Already settled before this (#166, #167): while the field has the
keyboard, nothing is lit and Enter starts the first that matches; Down,
or the pointer over a result, lights it and takes the keyboard to the
results; the cursor blinks. Every feature of the launcher has a key under
`launcher` in `config.yaml`, on unless turned off (ADR 83). What is agreed
is used before anything of omorfos's own (ADR 99).

What exists to build on, as found on 25 November 2026:

- **Agreed, across desktops** (freedesktop.org): the desktop entry
  specification's actions (`[Desktop Action …]`, as Firefox's "New
  Private Window"), which the launcher reads the files of and today
  passes over; and MPRIS, which every media player speaks over D-Bus.
- **Not agreed**: a way for programs to give a launcher results. GNOME's
  `org.gnome.Shell.SearchProvider2` is spoken by Files (Nautilus), which
  searches files through LocalSearch, and by Calculator, Characters,
  Clocks, Contacts, Settings, Software, Passwords and Keys, Weather and
  others. KDE's `org.kde.krunner1` is spoken mostly by KDE's own plugins,
  and gives each result actions of its own and a relevance. The two do
  not understand each other. A shared XDG interface was suggested in KDE
  in 2018 and never written.
- **The clipboard**: `omorfos-clipboard` keeps only what was copied
  last, not a history.

## Decision

**One launcher, `omorfos-launcher`, with a generic mode and modes, fed by
sources: some built in, the rest other programs' through GNOME's and
KDE's provider interfaces, behind one interface of the launcher's own.**

### What is shown

- **The generic mode**, what `Super+Space` opens onto: what is typed is
  looked for in every source that is on, and the results are shown in
  sections, under a heading each, in the order of the modes: first a
  **top hit**, the best match of all, then up to three of each section,
  and a row at the end of each, "Show all", that goes to its mode. A
  section with nothing in it is not shown. A sum's answer, where there is
  one, is the top hit. Other programs' providers that are no mode's (a
  calculator, the settings, the weather) each have a section of their
  own after the modes', under the program's name, in the order of their
  names.
- **A mode** shows its own source alone, all it found, as the programs'
  grid (#157) shows every program. A mode whose source is missing on the
  machine (files, with no program installed that searches them) is still
  shown, and says what would fill it, so that the numbers of the modes
  never change with what is installed. Its icon stands before the field, and
  its words in the field while it is empty ("Search programs", "Search
  files"…, translated, ADR 79).
- **The modes, as icons in a row**, appear when the pointer comes over
  the launcher, and go when it leaves; a click on one goes to it. The one
  shown is lit.
- **Picking**: as #166 has it. While the field has the keyboard nothing
  is lit, and Enter takes the top hit. Down, or the pointer, takes the
  keyboard to the results; the arrows walk them across sections; Up past
  the first goes back to the field. Enter on a result does what it is
  for: starts a program, opens a file, does an action, pastes from the
  history, runs a command.

### The keys

- `Super+Space`, `show_launcher`, opens it in the generic mode;
  `show_programs` (#157) opens it in the programs mode. An action to open
  it in any mode, `show_launcher: { mode: files }`, is for the builder to
  add if it costs little; the four-finger pinch (#138) stays on
  `show_programs`.
- `Super+1` and on, while it is open, go to the first mode and on, in the
  order the configuration gives; `Super+0`, or Escape in a mode, goes
  back to the generic one, and Escape there closes the launcher. The
  compositor hands Super with a digit to the launcher while it has the
  keyboard, as it hands it the shell's fields' Super (#152), rather than
  going to a workspace. In the ordinary way (ADR 101) it is Ctrl with a
  digit, as Super there is the system's.

### The sources

Built in, in the launcher, behind one interface each answers a query by:

1. **Programs**: the desktop entries, as now.
2. **Actions**:
   - omorfos's own, every action `omorfosctl action` lists that needs
     nothing more (lock, log out, the overview, night light…), by their
     translated names;
   - programs' own, from their desktop entries' `[Desktop Action]`
     groups, under the program's name and icon;
   - the user's own, from `config.yaml` (below);
   - the media playing, through MPRIS: play, pause, next, previous, for
     each player there is.

   An action that ends the session or the computer (log out, restart,
   shut down, quit) always asks first, whatever `power.ask` says, as Enter
   takes the top hit. An action on a window (close it, minimize it) is
   done to the window that had the keyboard before the launcher opened.
   The top hit is a program, a sum's answer or one of the user's own
   actions; never another action.
3. **Commands**: what is typed, run by the user's shell in a new window
   of the terminal, which stays open after it, as the roadmap's item had
   it after `>`. Shown only in its own mode, never in the generic one, so
   that nothing typed in search is ever run by Enter by mistake.
4. **The clipboard's history**: what `omorfos-clipboard` has kept, asked
   for over omorfos's socket by the launcher alone, which the compositor
   gives the right to when it starts it, as it gives `clipboard`; no other
   peer of the socket may read it. Text only to begin with, a bounded
   number of entries. It keeps the history in memory only, never on disk;
   never what a password manager marks as secret (as now); and forgets as
   `clipboard.forget` says. Picked, the entry is put on the clipboard, and
   the user pastes it: the compositor types nothing into another program,
   as the maintainer chose over pasting for them.
5. **Sums**, as now (ADR 86), in the generic mode.

From other programs, each a source of its own, by the interface it
speaks:

6. **GNOME's search providers**: each found by its file in the
   `gnome-shell/search-providers` directories of `XDG_DATA_DIRS` and
   `XDG_DATA_HOME`, asked over the session's D-Bus. Files come from here
   to begin with, through Nautilus, where it is installed: the files mode
   is Nautilus's provider, and any other that says it searches files, the
   builder to settle how that is told. omorfos's own search of the home
   directory, on the roadmap, is a later source of the same mode.
7. **KDE's runners**: each found by its file in `krunner/dbusplugins`,
   asked the same way; each result's own actions are shown with it.

A provider's results are another program's words: each is untrusted
input. A query is sent only to the sources of the mode shown, or, in the
generic mode, to all that are on; each provider is given a time to answer
in, 150 ms to begin with, after which its section is shown without it,
and filled in if it answers later while the same words are typed; and its
text, its icons and how many results it gives are bounded, as the desktop
entries are (ADR 22). An icon is taken by name from the icon theme, or as
a file only under the provider's own data directories, never another
path. A provider is never the top hit: what a program says of its own
relevance is not trusted over omorfos's own matches.

### The configuration

Under `launcher` in `config.yaml`, as ADR 83 has it:

```yaml
launcher:
  icons: true
  descriptions: true
  sums: true
  # The modes, in the order of Super+1 and on. One left out comes after
  # those named, in this order; turned off, it is not shown at all.
  order: [programs, files, actions, clipboard, commands]
  files: true
  actions:              # the actions mode
    enabled: true       # false: no actions mode, the user's own included
    omorfos: true       # omorfos's own
    programs: true      # programs' own, from their desktop entries
    media: true         # the players, through MPRIS
  clipboard: true
  commands: true
  providers:            # other programs' results, each on unless named
    gnome: true
    krunner: true
    off: [org.gnome.Weather]   # by the provider's desktop entry
  # Actions of the user's own, in the actions mode and the generic one.
  shortcuts:
    - title: New note
      description: A note in the notes folder   # optional
      icon: accessories-text-editor             # optional, a name or a path
      action: { spawn: [gnote, --new-window] }   # any action keys.yaml takes
```

Other programs' providers are on unless turned off, all of a kind at
once, or each by its desktop entry under `off`, as the maintainer chose:
the guide says that what is typed in the generic mode goes to each, and
that some go on to the network (a weather provider) or start a large
program the first time (Software).

The built-in actions are on unless turned off, each kind by its key
under `actions`; the user's own, under `shortcuts`, are shown whenever
the actions mode is on, so that with every kind of built-in action off
the mode holds the user's alone, as the maintainer asked. `actions` is a
mapping with `enabled` beside the kinds, as ADR 83 has a feature that
needs more than on or off.

Each mode is on unless turned off, by a key of its own, and `order`
only orders: a mode added in a later release, and so missing from a list
written before it, comes after those named rather than being off, which
is what ADR 83 asks of every feature. The user's own actions take the
same `action` as `keys.yaml` and `menu.yaml`, a `spawn` among them with
what it `may` do (ADR 69), so nothing new is allowed by them; their title
and description are the user's words, not translated.

### The look

As Spotlight's, enhanced: one pane of glass, the field across its top in
large text with the mode's icon before it, and the results right under
it, in sections with their headings, the top hit first, outlined while
the field has the keyboard as what Enter takes. With nothing typed in the
generic mode, nothing is shown under the field. The row of modes is at
the bottom, under a hairline, and only while the pointer is over the
launcher and nothing is typed. The launcher is as tall as what it shows,
up to the half of the screen it has today. The theme has parts for each
that is new (the headings, the modes' row and a mode in it, the top hit),
each with its rule in the built-in theme (the handover's list for a
part). Agreed with the maintainer on 25 and 26 November 2026 from the
mockups and a prototype of the motion.

It moves, as the maintainer asked, with times the theme can change:

- it grows a little and fades in as it opens, and the reverse as it
  closes, by Escape or a click beside it (the compositor's, as now: 220
  ms in, 140 ms out);
- its height follows what it shows, as results come and go, and as the
  modes' row comes and goes (200 ms);
- the lit row, or tile, slides from one to the next rather than jumping
  (150 ms);
- the mode's icon turns and fades into the next mode's as the mode
  changes (220 ms), but is simply there as the launcher opens.

With `appearance.reduce_motion` all of it only fades, or is at once, as
everything else in omorfos does.

### Standards

`docs/STANDARDS.md` gets the gap: no agreed interface for a program to
give a launcher results, with GNOME's and KDE's as what is used today.
Once both are spoken here, and what a shared one needs is known from
that, a proposal is drafted for `xdg-specs`, taking GNOME's shape, which
most programs speak, with KRunner's actions for a result and its
relevance.

## Alternatives

- **One best guess in the generic mode**, the kind that matched best
  shown alone, the mode switching itself. Magic when right; hides every
  other match when wrong. The maintainer chose sections.
- **Every source a D-Bus provider, omorfos's own too**: one way for
  everything, each in a process of its own. A round trip before even the
  programs are shown, and a process for each source to start and keep
  alive: typing would feel it.
- **KRunner's interface alone for other programs**: richer, but far fewer
  of the programs installed on most systems speak it; Nautilus's files
  would be lost.
- **An interface of omorfos's own**: against ADR 99, and nobody would
  speak it.
- **`order` as what is on**, a list that leaves out what is off: shorter,
  but a mode added later would be off for whoever had written a list
  (ADR 83).

## Consequences

- The launcher talks to the session's D-Bus, which it has not before,
  and to programs it does not know; the first query of a provider may
  start its program (Nautilus for files), tens of megabytes once a
  session.
- The compositor gives Super with a digit to the launcher while it is
  open; the workspaces' keys are not reachable then, as the bindings are
  not while a field of the shell has the keyboard.
- `omorfos-clipboard` keeps a history, in memory: more of what was copied
  stays in the session than does today, which the guide says, and how to
  turn it off.
- Commands run what is typed: only in their own mode, and only on Enter.
- New keys of `config.yaml`, each in its schema, `promised.txt` and the
  guide; new theme parts; new messages to translate.
- Being built in parts, the launcher is usable after each; until a mode's
  source exists, its mode is not shown.

## The parts, in order

1. The model: the sources' interface, the generic mode's sections and
   the top hit, the modes and `Super` with a digit, with programs and
   sums as the first two sources. The configuration's `order`.
2. The look, from mockups agreed: the field, the icon, the words, the
   sections, the modes' row, the theme's parts.
3. Actions: omorfos's own, programs' own from their entries, the user's
   own from the configuration.
4. Commands, in their mode.
5. GNOME's search providers, and with them the files mode.
6. KDE's runners.
7. MPRIS, among the actions.
8. The clipboard's history, in `omorfos-clipboard` and in its mode.
9. `docs/STANDARDS.md`, and the proposal once 5 and 6 are in.

Left to the builder: the time a provider is given, and the bounds on
what it may say; how many of each section the generic mode shows; how a
provider is told apart as searching files; whether `show_launcher` takes
a mode.
