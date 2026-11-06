# 64. Blur asked through KDE's protocol, for now

- Status: accepted
- Date: 2026-11-06
- Builds on: [33. How effects are drawn](0033-how-effects-are-drawn.md)
- To be undone: once the programs below ask through the standard

## Context

Programs ask for what is behind them to be blurred through
`ext-background-effect-v1`, the open standard, which omorfos offers
(ADR 33). Tried on real hardware, nothing asked through it. Ghostty
1.3.1, with `background-blur = true`, sees the protocol offered and does
not bind it: it knows only KDE's `org_kde_kwin_blur`, and so is drawn
see-through with nothing blurred behind it. Of the programs installed,
KDE's own ask through KDE's protocol too, through KWindowSystem
(`KF6WindowSystemKWaylandPlugin.so`), as does fcitx5's input panel
(`libclassicui.so`). The only one to name the standard at all is a
library inside 1Password.

The other desktops have nothing of the kind to offer a program: GNOME's
Mutter blurs nothing behind a program and has no protocol for one to ask,
and macOS is not Wayland.

## Decision

**omorfos offers `org_kde_kwin_blur_manager` at version 1, beside the
standard, until the programs that ask through it ask through the
standard.** What a program asks for through it is put where what is asked
through the standard is, a surface's double-buffered region, and drawn
by the same code: how far is the `window` part's `backdrop-filter`, cut
to the surface and its frame's rounded corners, with no tint and nothing
at all where blur cannot be drawn. Whichever of the two a surface was
asked through last is what it has.

It is held apart, in `crates/omorfos/src/kde_blur.rs` and its tests in
`kde_blur.rs` beside the standard's, each marked with a `TODO` naming
this record, so that taking it out is taking out a file, an entry in
each list of protocols, and the dependency.

Where KDE's protocol differs from the standard, it does as KWin does,
which the programs that ask through it are written for:

- A region is set on an object of its own and asked for once that object
  is committed, then the surface. A region of nothing, or none at all,
  asks for the whole surface.
- A surface may have many such objects. The last committed is what it
  asks for, and letting go of that one takes it back; letting go of an
  older one does not, so that a program that commits a new one before
  letting go of the one before keeps its blur. `unset` takes it back from the surface's next
  commit.
- Nothing is an error, as the protocol has none, and a program is never
  told whether it may have blur: where the theme gives programs none, or
  transparency is to be reduced, nothing is drawn.

## Alternatives

- **Offering only the standard**, and waiting for the programs. Blur
  asked for by Ghostty, KDE's programs and fcitx5 would go undrawn until
  each moves, which none has said it will by a given version.
- **Asking the programs to move first.** Worth doing, and not in the way
  of this: what is offered here goes once they have.
- **KDE's `org_kde_kwin_contrast`** as well, which changes a surface's
  contrast and colour rather than blurring. Nothing installed asks for it
  without also asking for blur, and it is a look omorfos's theme does not
  have.

## Consequences

- `wayland-protocols-plasma` is a direct dependency of the compositor
  and the relay, at the version Smithay's winit backend brings in
  already, so no crate is added to those built.
- The relay keeps a blur object's `set_region` and `commit` and sends
  them again in the order last asked. A region set after the last commit
  is set again uncommitted, and what that commit asked for becomes all of
  the surface until the program commits it again. Not seen with a real
  program: the region of a blur is set and committed together in what is
  known of how they ask. Making this work showed
  the relay could send an object's later request before an earlier one
  that waited for an object made after it; the requests after one that
  waits now wait too.
- A program asking through both, which none does, has whichever it asked
  through last.

## To undo

Run each program named above, as installed, and see whether it binds the
standard:

```sh
WAYLAND_DEBUG=1 ghostty 2>&1 | grep -E 'bind\(.*(background_effect|kwin_blur)'
```

or look for the name in what is installed:

```sh
grep -rlas ext_background_effect_manager_v1 /usr/bin /usr/lib/x86_64-linux-gnu
```

Once Ghostty, KWindowSystem and fcitx5 ask through the standard, take
out `kde_blur.rs` and its tests, `org_kde_kwin_blur_manager` from
`omorfos-ipc`'s list and the relay's, the relay's rules for
`org_kde_kwin_blur`, `Asked::Whole` if nothing else asks for a whole
surface, and the dependency; tick the item in `docs/ROADMAP.md`, and
mark this record superseded.
