# 108. The bar's menu is the system's, and the root menu the user's

- Status: accepted
- Date: 2026-11-24

## Context

The root menu, on a right click on the desktop, holds everything today:
the launcher, the default applications, the terminal, staying awake,
"About", locking, sleep, logging out, restarting and shutting down. It is
arranged in `menu.yaml`, each row with an `icon` if its own is not
wanted (ADR 70). Stage 5 gives the bar a menu of its own, as a Mac's
Apple menu, and the roadmap asked whether the root menu stays something
to arrange once it has.

On a Mac, the Apple menu is the system's and is not arranged by anyone:
About, Settings, sleep, restarting, shutting down, locking and logging
out. A right click on the desktop gives what is to do with the desktop.
Programs are started from the Dock and Spotlight.

## Decision

**The bar's menu is the system's, fixed, as the Apple menu: "About",
Settings, staying awake, locking, sleep, logging out, restarting and
shutting down. The root menu stays the user's, arranged in `menu.yaml`,
with `icon`; its default becomes what is to do with the desktop, a new
workspace, the screens, the wallpaper and Settings, and the programs the
user puts there.**

- What a session cannot go without is always on the bar's menu, so no
  `menu.yaml` can leave it out.
- Until the bar has its menu, the root menu keeps what it has.

## Alternatives

- **One menu, in both places**, `menu.yaml` arranging the bar's menu,
  and the right click opening the same. One thing to arrange, but what
  the session cannot go without could be left out of it.
- **Both fixed, and no `menu.yaml`**, as the Mac: programs only from the
  launcher and the dock. The leanest, but a menu kept in a file is what
  some come to omorfos for.

## Consequences

- Stage 5's bar menu is built fixed, and the root menu's default changes
  with it, in the same release, said as a change.
- `menu.yaml` and its `icon` stay as they are.
