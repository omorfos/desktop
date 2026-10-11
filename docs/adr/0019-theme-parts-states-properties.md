# 19. A theme is asked how a part looks in a state

- Status: accepted
- Date: 2026-10-10

## Context

`theme.yaml` is a small palette: an accent, a background, and three colours
each for focused and unfocused windows. Everything that drew anything
reached into it and made its own choice: the menu used the focused
titlebar's colour as its background, the panel the unfocused one, the
notice the accent as its outline. What a part looked like was therefore
decided in each painter, in the compositor and now in the panel too, and
nothing could be changed about one part without changing the palette for
all of them.

ADR 18 settled that themes will be written in CSS. A rule in CSS names a
part and perhaps a state and gives it properties. The painters had no such
question to ask.

## Decision

**Whatever draws asks the theme for the style of a part in its states**,
`Theme::style(Part, States) -> Style`, and draws from the answer alone.

- A `Part` is something a theme gives a look to: the desktop, a window's
  frame, a titlebar, a tab, the menu and its rows and separators, the
  notice and its heading, the panel, a button on it, the clock.
- A `State` is `Focused`, `Active` or `Hovered`, and a part can be in
  several at once. One a part cannot be in makes no difference to it.
- A `Style` is the properties the canvas can draw: a background, a text
  colour, a border, a line along the bottom edge, a corner radius and a
  height. What does not apply to a part is nothing.

These are the selectors, pseudo-classes and properties of the CSS to come,
under other names.

**For now the answer is worked out from the palette**, in one place, to
give exactly what was drawn before. The format of `theme.yaml` does not
change and neither does a single pixel; the tests that check pixels pass
untouched, which is how this was checked.

The model lives in `omorfos-config`, beside the theme it is asked of, since
the compositor and every component need it and both already depend on
that crate.

## Consequences

- The choices that were scattered through the painters are one table, in
  `style.rs`, with a test for each part. That table is the default theme
  that a stylesheet will later be laid over.
- Replacing `theme.yaml` with CSS is now a change to where `Theme::style`
  gets its answer, and to nothing that draws.
- Sizes that were the painters' own stay theirs for now: the padding
  around a label, the width of the menu. They join `Style` when a theme
  has reason to set them.
- The font is still one for the whole theme. Giving parts their own is a
  property to add, with inheritance, when CSS arrives.
- The palette's fields stay public, as the schema of the file. Nothing
  outside `omorfos-config` reads them any more except tests.
