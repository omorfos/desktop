# 20. Themes are stylesheets, read by a parser of our own

- Status: accepted
- Date: 2026-10-11

## Context

ADR 18 settled that themes will be written in CSS, and ADR 19 gave
everything that draws one question to ask: how a part looks in its states.
What remained was to answer that question from a stylesheet instead of the
palette in `theme.yaml`.

Three things had to be decided: how much of CSS, what reads it, and what
becomes of the old file.

## Decision

**The theme is `theme.css`, in a small part of CSS that we define.**

- Selectors are the parts of ADR 19 by name (`titlebar`, `panel-button`),
  `*` for all of them, and the states as pseudo-classes (`:focus`,
  `:active`, `:hover`). A rule may list several selectors.
- Properties are those the canvas can draw: `background`, `color`,
  `border-color`, `border-width`, `border-bottom-color`,
  `border-bottom-width`, `border-radius`, `height`, `font-family` and
  `font-size`. Sizes are whole pixels within the ranges the old file
  allowed.
- Variables are declared in `:root` and used as `var(--name)`.
- There is no tree, so nothing is inherited and there are no combinators.
  `*` does what inheritance would have.

**The built-in theme is a stylesheet too**, compiled into the program, and
the user's is laid over it. As in a browser, the user's rules always win
over the built-in ones; among the user's own, the one naming more states
wins, then the later. The built-in theme leans on a handful of variables,
so the old palette survives as the quickest way to retheme: give `--accent`
another value.

**Everything is worked out when the theme is loaded**, into a table of a
style and a font for each part in each combination of states. Asking the
theme a question while drawing is a lookup. A theme is shared by reference,
so copying one and comparing it with its copy cost nothing, which the
compositor's caches rely on.

**Nothing is passed over.** A browser ignores what it does not understand;
here a part, state, property or value that is not understood is an error
with a line and column, the file is refused as a whole, and the previous
theme stays in effect. That is what the other configuration files do, and
a typo that silently did nothing would be worse than one that is reported.
A bad value given to a variable is reported at the variable, not at the
built-in rule that happens to use it.

**The parser is ours.** The well-known CSS parsers are under the MPL, which
`deny.toml` refuses. `simplecss` is permissive, but it reports no positions
and skips what it cannot read, which is the opposite of the rule above. The
subset is small: the parser and the cascade are a few hundred lines with no
recursion, and their work is in proportion to the size of the file, which
is capped like every configuration file.

**`theme.yaml` is no longer read.** While one is in the configuration
directory the user is told so, as a problem like any other, rather than
left to wonder why it has no effect. The two things in it that were not
about how parts look have moved or gone: the pointer's theme and size are
`cursor` in `config.yaml`, since they are what programs are told as much as
what is drawn, and `animations`, which nothing had ever used, is removed
until there is something to animate.

## Consequences

- This is a breaking change to the configuration. A theme has to be
  rewritten, though for most that is a `:root` rule with a few variables.
- Parts have fonts of their own, and the panel, the menu rows and the
  titlebar have heights of their own, where there was one of each before.
- The sizes of a window's frame and titlebar are taken from the rules
  without a state. A rule that gave a focused window a thicker border would
  change the colour's width in the titlebar but not the window's geometry;
  the guide says not to.
- There is no JSON Schema for the theme any more, so editors offer nothing
  for it beyond their own CSS support, which does not know our parts.
- Adding a property is an arm in one `match` and a field in `Style`; adding
  a part is a variant, a name and a rule in the built-in stylesheet.
  Shorthands (`border: 1px solid ...`), named colours and `rgb()` are not
  there yet and can be added the same way.
- The launcher and notifications will be built with parts of their own
  from the start.
