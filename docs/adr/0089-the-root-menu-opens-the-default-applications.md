# 89. The root menu opens the default applications

- Status: accepted
- Date: 2026-11-15

## Context

The maintainer asked (15 November 2026) for the root menu's first section
to be Run, Browser, File Manager, Editor, Terminal and Settings, each
"pointing to default apps"; and for those that cannot be opened yet, such
as Settings, which comes with stage 7, to be there all the same and to say
so when chosen, rather than be left out.

What opens what by default is already the system's to say, in the
`mimeapps.list` files of freedesktop.org's specification, which the
welcome card reads to show on its last step.

## Decision

**An action `open`, of `browser`, `files`, `editor` or `settings`, opens
whatever the system has as the default for web links
(`x-scheme-handler/https`, then `http`, then `text/html`), folders
(`inode/directory`) or plain text (`text/plain`), as the first
`mimeapps.list` that names one says, by its desktop entry's `Exec`.**

- **Read when chosen,** not kept: a default changed in another desktop's
  settings is what the next choice opens.
- **One reader of the defaults,** moved from the welcome card into
  `omorfos-entries` (`defaults`), for the card to show and the compositor
  to open.
- **With none set, the user is told,** in the notice in the corner, each
  time it is chosen, with where to set one; `settings` always says that
  omorfos has none yet. The rows are shown whether or not a default is
  set, as the maintainer asked.
- **Each has a sign of its own,** Lucide's globe, folder, pen on a page and
  cog, rather than the default application's icon, so that the menu does
  not read the files each time it opens. Terminal keeps its program's.
- The label is "Files", as GNOME has it, which is shorter than "File
  Manager" in the menu's 240 units, in Polish too ("Pliki").

## Consequences

- The built-in root menu's first section is Run…, Browser, Files, Editor,
  Terminal and Settings.
- What the default is started with, a link or a folder, is nothing: the
  places in `Exec` where those would go are left out, so a browser opens
  its new window and a file manager its home.
- Settings is to open omorfos's own once stage 7 makes them.
