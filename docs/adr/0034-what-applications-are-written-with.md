# 34. What components and applications are written with

- Status: accepted
- Date: 2026-10-13

## Context

The panel, the launcher and the notifications are written with
`omorfos-ui`: a canvas, text, and the theme (ADR 18). It draws boxes and
words. Each component lays itself out by hand and takes clicks by asking
which box they fell in. That was right for three small programs.

What is wanted next is more (roadmap, stages 5 to 7): a bar with popovers
and a calendar, a dock, settings, a task manager, what asks for a
password, a clipboard's history, a grid of every program, and then a file
manager and an image viewer. Those need what a toolkit has: lists that
scroll, fields to type in, switches and sliders, focus that moves with
the keyboard, dragging and dropping, and being usable with a screen
reader.

Either that is written here, or the applications use a toolkit that
exists. The choice sets how large the applications are to write, how
they look beside the shell, and what the rules here (no `unsafe`,
everything covered, few dependencies) can still mean for them.

### What exists

Looked at in October 2026; the state of these moves quickly.

| Toolkit | For it | Against it here |
|---|---|---|
| GTK 4, through `gtk4-rs` | Mature. Accessible, input methods, drag and drop, all done. Styled with CSS. | A large C library and its own stack beneath our code. Looks like GNOME unless fought. Draws its own windows its own way: our glass, our titlebars and our theme file would each need a bridge. What it draws cannot be tested as the rest here is. |
| iced, and libcosmic on it | Rust throughout. COSMIC ships a whole desktop on it. | Accessibility is experimental and lives in forks. Hundreds of crates and a GPU renderer for a settings window. Its look is its own, or COSMIC's. |
| Slint | Accessible, input methods, small. | Licensed under the GPL or its own royalty-free terms, not a licence `deny.toml` allows, and not one to bind an MIT desktop to. Its own language for layouts. |
| egui | Simple, accessible through AccessKit. | Redraws everything every frame by design, which is the opposite of "no work when nothing changes". Looks like a tool, not a desktop. |
| Xilem, Floem, Freya and others | Promising. | Not finished. |

None is at once written in Rust, accessible, light, and able to wear the
design language without a second theme to keep in step.

## Decision

**One toolkit, ours: `omorfos-ui` grown into one, for the shell and for
the applications that come with omorfos.** It is grown by need, each
piece when the next thing on the roadmap wants it, not built whole and
then used.

What makes this less than it sounds:

- **Text is not ours to write.** `cosmic-text`, already what draws every
  word here, has an editor: the cursor, selection, lines that wrap,
  scripts that run right to left. A field to type in is that with a box
  round it.
- **Speaking to screen readers is not ours to write.** AccessKit is a
  library made for toolkits that draw themselves. The toolkit says what
  is on the screen as a tree (this is a button, called this, pressed or
  not); AccessKit speaks AT-SPI to Orca. Saying it is ours; it is said in
  the same place the widget is drawn, so it cannot be forgotten for one
  widget and not another.
- **Being a Wayland client is already written**, three times, and is
  gathered into `omorfos-shell`.
- **Decorations, glass and shadows are the compositor's.** An application
  draws its content. It has the same titlebar as every other window, and
  asks for blur behind a sidebar as any program may (ADR 33).

What it is:

- **Widgets are values.** What a window shows is made from its state by
  plain code and compared with what it showed before; only what differs
  is drawn again. This is how the panel's `Bar` already works, and it is
  what lets all of it be tested without a screen: state in, a tree of
  widgets out, and pixels from that.
- **Drawn on the CPU into shared memory**, as now. No GPU in an
  application, so none of its drivers, and a picture any test can look
  at.
- **Themed by the same `theme.css`.** A widget is a part with states, as
  a titlebar is. One file changes the shell and the applications
  together.
- **Every widget answers the design language's seven questions** (ADR 32)
  before it is done, the keyboard and the three settings among them.
- **Under the rules of everything else here**: no `unsafe`, all of it
  covered, a test on pixels for each widget and on the tree it gives to
  AccessKit.

### In what order

The roadmap's order is the toolkit's order. Each of these is a widget or
two more:

1. The bar and the dock: a button, a row, a popover, a tooltip, a grid
   for the calendar.
2. What asks for a password: a field, sealed. Small, and the place where
   being small and ours matters most.
3. Settings: a switch, a slider, a choice from a list, a field, a pane
   that scrolls, a sidebar.
4. The task manager: a table that sorts, a chart.
5. The clipboard's history and the grid of programs: a list and a grid of
   many things, dragging and dropping.
6. The file manager and the image viewer, last.

### Two trials, before much is built on it

- **Scrolling.** A list of ten thousand rows in a maximized window on a
  dense screen, drawn on the CPU, has to keep up with the screen. This is
  tried with the first list that is written. If it cannot be made to keep
  up by drawing only what changed, the drawing moves to the GPU behind
  the same widgets, and that is decided then with the numbers in hand.
- **The file manager.** It is the largest thing asked for and the test of
  the toolkit. If, when its turn comes, what it needs is still far off,
  the file manager and the image viewer alone are written with a toolkit
  that exists, or are left to the programs that exist, and nothing else
  changes. They are not required by anything.

## Alternatives

- **The shell on ours and the applications on GTK.** Applications sooner,
  and finished in ways ours will take years to be. But two looks, or the
  work of keeping two themes alike for ever; a large C library in what is
  to be secure and lean; and applications whose windows the tests cannot
  see. If the trials above go badly this is what the two large
  applications fall back to.
- **Everything on libcosmic.** A desktop made of another desktop's
  parts. COSMIC is the nearest thing to what is being built here, and
  using its toolkit would make omorfos a theme of it.
- **Write no applications; recommend ones that exist.** The least work
  and a sound answer for the file manager. But settings, what asks for a
  password, the task manager and the shell's own popovers are omorfos
  and have to be written with something.

## Consequences

- Stage 7 is larger than it would be on a toolkit that exists, by months,
  and that is chosen with open eyes. In return there is one look, one
  theme file, one way of testing, and nothing beneath the applications
  that is not either ours or already depended on.
- Accessibility becomes ours to get right. AccessKit carries the words;
  what is said, and the keyboard reaching everything, is work in every
  widget and is tested in every widget.
- Input methods, for writing Chinese, Japanese, Korean and more, are
  ours to speak (`text-input-v3`) in the one place fields are made.
- AccessKit and its link to AT-SPI are new dependencies, to be justified
  and passed by `cargo deny` when they are added.
- The components already written are moved onto the widgets as these
  appear, and lose their layout by hand.
- A task manager that also runs in a terminal has its logic apart from
  how it is shown, as every component here has, and the terminal is one
  more way of showing it, not a second toolkit.
- Programs written with other toolkits remain what most of a user's
  programs are, and are served by the protocols, the portals and the
  settings they are told (ADR 32).
