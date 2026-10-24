# The design language

What omorfos looks like out of the box, and the rules a part of it is
drawn by. The reasons are in
[ADR 32](adr/0032-the-design-language.md). The built-in theme and every
component and application that comes with omorfos follow this; a user's
`theme.css` may do anything it likes.

Not all of this can be drawn yet. What waits for something is marked with
the stage of the [roadmap](ROADMAP.md) that brings it.

## In one paragraph

Quiet glass. Surfaces are sheets of tinted, frosted glass over the
wallpaper, with rounded corners, a hairline edge and a soft shadow. There
is one accent colour and it means one thing: this has focus, or this is
what will happen. Text is Inter. Things arrive quickly and leave quicker.
Nothing is decoration only: if an effect makes something harder to read,
slower, or less safe, the effect goes.

## Principles, in order

1. **Legible before lovely.** Text meets the contrast below on every
   surface, over any wallpaper. Glass is given as much tint as that takes.
2. **Honest.** Nothing drawn by omorfos can be imitated by a program:
   what asks for a password, and what says which program has the keyboard,
   is drawn by the compositor or by a component it started, above
   everything a program can draw.
3. **Calm.** One accent. No gradients for their own sake, no glow, nothing
   that moves without having been caused.
4. **The same everywhere.** One scale of sizes, one of spacing, one of
   radii. A menu in the dock is the menu of a titlebar.
5. **Cheap.** Nothing is drawn when nothing changes. An effect that costs
   a frame on a modest laptop is off there.
6. **Yours.** Every colour is a variable and every part can be restyled
   or done without.

## Type

| Use | Family | Falls back to |
|---|---|---|
| The interface | Inter | `sans-serif` |
| Code, figures in columns, the terminal | JetBrains Mono | `monospace` |

Both are under the SIL Open Font Licence. Packages of omorfos recommend
them and do not require them: without them the system's `sans-serif` and
`monospace` are used and nothing breaks. Inter has Latin, Greek and
Cyrillic; other scripts come from whatever fontconfig finds, which on
most systems is Noto.

Sizes, in points, at a weight of 400 unless said:

| Name | Size | Weight | Used for |
|---|---|---|---|
| caption | 9pt | 400 | What is beside the point: a time, a count, a hint of a key |
| body | 11pt | 400 | Nearly everything |
| strong | 11pt | 600 | The title of a notification, a heading in a menu |
| title | 14pt | 600 | The heading of a dialog or a pane |
| display | 22pt | 600 | The clock of the lock screen, and little else |

Numbers that change in place (a clock, a percentage) use tabular figures,
so that nothing beside them shifts. Nothing is set in capitals, and
nothing is underlined that is not a link.

## Space and size

Everything is a multiple of 4 pixels of layout.

| Name | Pixels | Used for |
|---|---|---|
| hair | 1 | An edge |
| tight | 4 | Between an icon and its label |
| snug | 8 | Padding inside a control; between rows that belong together |
| room | 12 | Padding inside a surface |
| wide | 16 | Between groups |
| far | 24 | Between sections; from a floating surface to the edge of the screen |

| What | Height |
|---|---|
| A row: a menu entry, a titlebar, a task | 28 |
| A control: a field, a button | 32 |
| The top bar | 28 |
| The dock | 56, with icons of 40 |

Nothing that can be clicked is smaller than 24 by 24, however small it is
drawn: the buttons of a titlebar are 12 across and are hit in 20 by 28.

## Corners

| Name | Radius | Used for |
|---|---|---|
| small | 6 | A row that is picked out, a field, a button |
| medium | 10 | A menu, a tooltip, a notification, the launcher |
| large | 12 | A window, a dialog, the dock |
| round | half the height | A dot, a pill, a switch |

A corner inside another is the outer radius less the room between them,
so that the two curves are parallel. A window that is maximized or
fullscreen has square corners, and so has the top bar.

## Colour

Two schemes, dark and light. Dark is the default, light can be chosen,
and `auto` goes by the sun (see below). Each colour is a variable of the
theme; the names are the same in both. The grounds are the brand's
([BRAND.md](BRAND.md)): navy in the dark and a warm cream in the light,
not white.

| Variable | Dark | Light | Used for |
|---|---|---|---|
| `--background` | `#0c1226` | `#fcf7e3` | Behind everything, where there is no wallpaper |
| `--surface` | `#141c38` | `#fffdf4` | Glass: bars, menus, titlebars with focus |
| `--surface-dim` | `#10172f` | `#f6f0da` | The same where focus is elsewhere |
| `--surface-raised` | `#243364` | `#ffffff` | A field, a row under the pointer |
| `--text` | `#e6eaf5` | `#0c1226` | |
| `--text-dim` | `#9aa4c7` | `#5c6888` | What is beside the point |
| `--border` | `#2b3a6b` | `#e4ddc4` | A hairline between or around surfaces |
| `--border-strong` | `#7482b3` | `#7a8099` | The edge of something that takes input |
| `--accent` | `#22d3ee` | `#0e7490` | Focus, and what is picked out |
| `--on-accent` | `#0c1226` | `#fffdf4` | Text on the accent |
| `--danger` | `#f8616d` | `#c4303a` | Close, delete, an error |
| `--warning` | `#fde047` | `#8a5a00` | Minimize; something to look at |
| `--success` | `#a3e635` | `#2f7a35` | Maximize; something that went well |

The accent is a setting. The ones that come with omorfos, each with a
dark and a light value that meets the contrast below. The brand's cyan
is too pale to be seen on cream, so the light scheme has a deeper one.

| Accent | Dark | Light |
|---|---|---|
| Cyan (default) | `#22d3ee` | `#0e7490` |
| Blue | `#5b9cf5` | `#2563d9` |
| Teal | `#4fc3b5` | `#0f766e` |
| Purple | `#a78bfa` | `#6d3fd4` |
| Pink | `#f27fb0` | `#c02a72` |
| Orange | `#f29b5c` | `#b84a0e` |

The brand's other colours, the five of its spectrum together, are for
the logo, the wallpapers and the lock screen's picture, not for the
interface, which has one accent.

### Contrast

Measured as WCAG 2 does. These hold for every pair in the tables above
and must hold for anything added:

- Text on the surface it is on: at least **4.5 to 1**. With the values
  above: text 13.9 (dark) and 18.2 (light), dim text 6.8 and 5.4, dim
  text on a raised row 4.9 and 5.5, text on the accent 10.3 and 5.3.
- The edge of something that takes input, and anything that says a state
  without words: at least **3 to 1**. `--border-strong` is 4.5 and 3.8 on
  the surface; every accent is at least 4.9 on the background.
- Colour is never the only sign. A titlebar's buttons show a sign under
  the pointer; what is urgent has a mark as well as a red edge; what has
  focus has a ring as well as a tint.

### Following the sun

With `appearance.scheme: auto` the scheme is light from sunrise to sunset
and dark otherwise.

- Sunrise and sunset are worked out, from the date and from where the
  user says they are: `appearance.location`, a latitude and a longitude.
  omorfos does not ask the network where the machine is; that would tell
  someone else where the user is, to choose a colour.
- Without a location, light is from 07:00 to 19:00, and both hours can be
  set.
- Where the sun does not rise or does not set that day, the scheme is
  the one that suits: dark through the polar night, light through the
  midnight sun.
- The change is a transition of two seconds, and it waits until nothing
  is fullscreen. It is not made at a moment the user is in the middle of.
- Programs that ask are told when it changes, as they are told the
  scheme.

## Glass

(Stage 4: needs blur. Until then every surface is drawn solid in its
colour, which is also what "reduce transparency" does for good.)

A surface is glass when what is behind it is worth being reminded of and
not worth reading: bars, the dock, menus, notifications, the launcher,
the titlebar of a window. The content of a window is never glass.

| Glass | Tint | Blur | Used for | What may be written on it |
|---|---|---|---|---|
| Thin | 70% | 24 px | The top bar, the dock | `--text` only, never `--text-dim` |
| Thick | 90% | 32 px | A titlebar, a menu, a tooltip, a notification, the launcher | Anything |
| Sealed | 95%, and everything else dimmed | 40 px | What asks for a password | Anything |

- The tint is the surface colour at that opacity, and these are floors
  that were worked out, not chosen: the least, rounded up, at which the
  text allowed on the glass keeps 4.5 to 1 over the worst thing that can
  be behind it, a white window in the dark scheme and a black one in the
  light. Text needs 67% in the dark scheme and 54% in the light; dim text
  needs 87% and 89%. Blur makes the worst case rare; it does not make it
  impossible, so it is not counted on.
- No setting of the built-in theme takes glass below its floor.
- Glass has a hairline edge of `--border`, and in the dark scheme a
  second, inner hairline of white at 6% along the top, as light on the
  rim of a sheet.
- The picture behind is given a little more saturation as it is blurred,
  so that glass over grey is not mud.
- The blur is what tells a sheet of glass from what is under it. A tint
  alone over a busy picture leaves the eye to work out where the sheet
  ends; blurred, what is behind becomes colour and light and nothing that
  can be read, and the edge of the sheet is plain. So the blur is wide
  enough that no letter behind keeps its shape, and no wider, since a
  wider blur costs more to draw.
- Where blur cannot be drawn, or would cost too much, the surface is
  solid. Nothing is ever see-through and not blurred: that is text over
  text.

## Depth

(Stage 4: needs shadows.)

Three levels, and a shadow says which. Shadows are black, soft, and fall
straight down.

| Level | What is on it | Shadow |
|---|---|---|
| 0 | The wallpaper, the top bar | none |
| 1 | Windows, the dock | 0 8px 24px at 28%; half that for a window without focus |
| 2 | Menus, tooltips, notifications, the launcher, dialogs | 0 12px 32px at 36% |

## Focus

- The window with focus has its titlebar in `--surface` and its text in
  `--text`; the others are in `--surface-dim` and `--text-dim`. Its edge
  is `--border-strong`, not the accent: a ring of accent round every
  focused window is louder than it need be once there are shadows. (Until
  stage 4 the edge stays the accent, as it is the only sign there is.)
- Inside a surface, what the keyboard is on has a ring: 2 pixels of the
  accent, 2 pixels out from the thing. It shows when the keyboard is
  being used and not when the pointer is.
- A row under the pointer is `--surface-raised`. A row that is chosen, or
  that the keyboard is on, is the accent with `--on-accent`.

## Movement

| What | How long | Curve |
|---|---|---|
| A colour changing with a state | 120 ms | ease-out |
| Something coming onto the screen | 180 ms | ease-out, from 96% of its size and no opacity |
| Something leaving | 120 ms | ease-in, to 98% and no opacity |
| A window moving to where it was sent: maximize, a workspace sliding | 240 ms | ease-in-out |

- Nothing is longer than 300 ms, nothing bounces, nothing loops.
- What the user drags follows the hand with no easing at all.
- Leaving is quicker than arriving: what has been dismissed should not
  be in the way of what is next.
- Movement never holds input up. A menu can be clicked the frame it
  appears.
- With `reduce_motion`, things that move change opacity only, over 120
  ms, and things that slide appear.

## Icons

Two kinds, never mixed in one row.

- **Signs** are ours: one colour, lines with round ends, taking the
  colour of the text beside them. They are for actions and states: close,
  a tick, a submenu's arrow, battery, network, volume. They are taken
  from [Lucide](https://lucide.dev) (ISC licence), which is drawn on a
  grid of 24 with lines 2 wide; here they are shown at 16 with lines 1.5
  wide, and only the ones used are built in.
- **Pictures** are a program's own icon, from the icon theme in use, in
  full colour, at 16 in a row, 24 in a list and 40 in the dock. Where a
  program has none, a plain rounded square with its initial.

A sign is always beside words or has a tooltip, except the three buttons
of a titlebar, whose places are learnt.

What is not ours is credited. Everything drawn or designed by someone
else that omorfos ships or recommends is named, with its authors and its
licence, in [CREDITS.md](../CREDITS.md); the licence texts go into every
package; and "About" in the menu shows the same list. Nothing of someone
else's is built in before it is in that file.

## The pointer

The cursor theme and size are settings (`cursor.theme`, `cursor.size`),
24 by default and the same over every program, including those written
for X11. Packages recommend the Adwaita cursors, which are drawn at every
size needed.

omorfos is to have a pointer of its own in time (stage 4), built in and
used when no theme is named. It would be drawn as shapes and not as
pictures, so that it is sharp at any size and any scale, which pictures
at a few fixed sizes are not, and so that a session with no cursor theme
installed still has every pointer. It is some thirty shapes with their
hot spots, and one that turns: work for the look, not a risk. Until then
the user's theme is used as now.

## Settings for those who need them

They are in `config.yaml` from the start and every part honours them.
(Stage 4 brings the settings; the rules are binding on what is drawn
before then.)

| Setting | What it does |
|---|---|
| `appearance.scheme` | `dark`, `light`, or `auto`: light while the sun is up |
| `appearance.location` | A latitude and a longitude, for `auto` to work out sunrise and sunset from |
| `appearance.accent` | One of the accents above, or any colour |
| `appearance.reduce_motion` | As under Movement |
| `appearance.reduce_transparency` | Every surface solid, no blur |
| `appearance.high_contrast` | Black and white surfaces, `--border-strong` for every edge, a ring on the window with focus, signs on the titlebar's buttons always |

A program that asks is told the scheme, the accent and whether motion is
reduced, through the settings portal, so that applications can match.

## What a new part must answer

1. Which level is it on, and so which shadow and which glass?
2. Which sizes of type, from the five?
3. Is everything on the 4 pixel grid, with radii from the four?
4. Does every piece of text meet 4.5 to 1, with the tint at its least?
5. What does it look like with focus, under the pointer, under the
   keyboard, disabled, and urgent?
6. What does it do under each of the three settings above?
7. Can it be worked with the keyboard alone?
