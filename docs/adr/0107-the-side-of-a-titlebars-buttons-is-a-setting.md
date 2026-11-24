# 107. The side of a titlebar's buttons is a setting

- Status: accepted
- Date: 2026-11-24
- Takes up: [ADR 97](0097-a-windows-frame-bulkier-glass-buttons-at-the-left.md),
  for the side the buttons stand on

## Context

ADR 97 put a titlebar's buttons at the left, close outermost, and has
programs that draw their own titlebars told the same, through the portal
and GTK 3's settings. It is not a setting: the maintainer prefers the
left, and accepted that programs that listen to nothing, as Discord,
Steam and Spotify, keep theirs at the right. ADR 101 left whether the side
is a setting to the review of the defaults, which decided it on
24 November 2026.

## Decision

**`appearance.buttons` in `config.yaml` says which side a titlebar's
buttons stand on: `left`, as now and by default, or `right`.**

- omorfos's own frames draw them on that side, close outermost on
  either, and its tabs and title share what they leave, as now.
- Programs that draw their own titlebars are told that side as they are
  told the left now, so that they follow it, and told again when it
  changes, as a change of any other part of the look is.
- The theme's parts are as they are; `button-close` keeps its margin on
  the side away from the titlebar's edge, whichever that is.

## Alternatives

- **Left, fixed, until there is a Settings application.** One less
  setting, as ADR 97 had it; the maintainer chose the setting now.
- **Following the theme**, a property of `titlebar` saying the side. The
  side is the user's, not the look's, and programs are told it apart from
  the theme.

## Consequences

- A new key of `config.yaml`, and its schema; a value added is nothing
  broken (ADR 73).
- The guide's page of `config.yaml` and its example say it.
- Settings, once there is one, shows it.
