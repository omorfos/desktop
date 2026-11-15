# The panel

`omorfos-panel` puts a bar along the bottom of every screen: that screen's
workspaces, then the windows on the one it is showing, and the time at the
end. Click a workspace to show it, or a window to bring it up. The
workspace being shown and the window with focus are underlined; a
minimized window's title is in brackets. The time is the local time, in
24 hours.

Click the time to open the month above it, today in the accent: six weeks
from Monday, the days of the months before and after it faded, so that it
is as large whichever month it shows and its arrows stay where they are.
The week and the weekday of the day under the pointer stand out, that day
the most. The arrows at its top, or Left and Right on the keyboard, go a
month back or on; Escape, a click anywhere else or on the time again puts
it away, and so does saving the theme, after which it opens as the theme
now has it.

Rest the pointer on the time for half a second and a tooltip says the whole
date; on a window whose title is cut short, the whole title; and on what
says the session is kept awake, that a click lets the screen sleep again.
Once one is showing, the next shows as soon as the pointer is on it. A
click puts it away until the pointer leaves what was clicked.

It is a program of its own, started with the session, and started again if
it crashes. How it looks is in `theme.css`, under `panel`, `workspace`,
`task` and `clock`, the month under `popover`, `popover-item`,
`popover-day` and `popover-day-outside`, and its tooltips under `tooltip`;
it follows the file as you save it.

To go without it, or to use another panel that speaks layer-shell, such as
`waybar`, turn it off in `config.yaml`:

```yaml
panel:
  enabled: false
autostart:
  - [waybar]
```

Like `autostart`, this takes effect the next time you log in.
