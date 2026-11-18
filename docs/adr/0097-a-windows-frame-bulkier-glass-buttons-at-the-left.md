# 97. A window's frame: taller, glass, its buttons at the left

- Status: accepted
- Date: 2026-11-18

## Context

Windows were to be redesigned so that every program's looks the same
(stage 4). They cannot be made the same: a program that draws its own
frame, as GTK's, Firefox's, Chromium's, Discord's, Steam's, Telegram's
and Spotify's do, draws what it likes, and the two ways to change that
were both refused by the maintainer.

- **A theme for each toolkit**, GTK 2, 3 and 4 and Qt, kept in step
  with omorfos's. That is what makes a Linux desktop tiring: three or
  four themes to find, install and keep agreeing, and omorfos one more
  standard beside them. omorfos is to be where that ends, not another
  of it.
- **A frame forced on programs that draw their own.** It gives them two
  titlebars and two buttons that close, and breaks those that lay
  themselves out round their own.

What is left is to fit in beside what programs draw: to tell them what
every desktop tells them, and to draw omorfos's own frame, on the
windows that ask for one, so that it is at home among theirs.

That frame was plain: a titlebar 28 tall, solid, with three dots 12
across at its right and a hairline round the window. The maintainer
asked for it to be bulkier, to be glass as the design language says a
titlebar is, for its buttons to be larger and at the left, for a sign
on each of them once the pointer is on any, and for a fainter edge;
tried the choices in a mock in a browser, and settled these.

## Decision

**Programs are told, never themed.** The settings portal and the X11
settings say the scheme, the accent, the order of the buttons, the
fonts and the pointer, as they did. omorfos ships no theme for a
toolkit and forces no frame; a rule with `decorations: true` still
does, for whoever asks.

**A program that says it draws its own titlebar is left to.** ADR 7
answered every program that asked who decorates it "the compositor",
whatever it preferred, since frames carry the tabs. Some ask and say
they prefer their own, and draw it whatever they are told: Visual
Studio Code, as Electron's programs with a titlebar of their own
making do, then had two, as the maintainer found on 18 November 2026.
One that says so before its window is shown is now told it may, and
gets no frame; one that asks and states no preference, or prefers
ours, gets ours as before. A window that is shown keeps the frame it
was shown in, or none, whatever it asks then: a frame does not come
and go under a window in use, nor against a rule that forced it. Such
a window cannot be a tab, which is the price ADR 7 would not pay and
this one does.

**The titlebar is 48 tall and thick glass**: the surface at nine tenths
of itself over what is behind it blurred 32 pixels, with the light
along its top in the dark scheme, as DESIGN.md has had a titlebar from
the start. It is the first thing the built-in theme makes glass besides
"About". Without focus it is the dimmer surface, at nine tenths in the
dark scheme and at 97 in a hundred in the light one: dim text on the
dimmer cream keeps 4.5 to 1 over a black window at no less, and the
floors of glass are worked out, not chosen. `reduce_transparency` makes
it solid, as it does everything.

**A tab has no ground of its own.** The titlebar's glass is behind
every tab; the one shown is told by its underline and its text.

**The buttons are at the left: close, minimize, maximize**, close
outermost, 16 across, 12 from the edge and 8 apart, each hit in its
share of the whole height of the titlebar. This takes back ADR 68's
side and its order; what ADR 68 says of telling programs stands, and
they are now told `close,minimize,maximize:`. They are red, amber and
green in both schemes, colours of their own (`--close`, `--minimize`,
`--maximize`) and no longer `--danger`, `--warning` and `--success`,
which the light scheme has deeper so that they can be read as text; on
cream each has a faint darker edge, since amber on cream is not seen
without.

**All three show their signs while the pointer is on any of them**, and
have their colours then, with focus or without. A button's place is
learnt; the signs are for telling which is which before pressing, and
that is wanted as the pointer arrives among them, not on the one it
happens to be on. All three are `:hover` together: a theme cannot
style the one under the pointer apart from the others any more.

**A title alone stands in the middle of the titlebar**: of what the
buttons leave of it, less as much again at the other end, so that it
is in the middle of the window and not of what is beside the buttons.
Tabs share what the buttons leave, as before.

**The window's edge is a fainter hairline, the one round everything
that floats.** It was first built with none, as chosen in the mock;
tried on real hardware, the maintainer asked for the hairline after
all, in `--border-faint`, and for what is the same on every surface
to be said once. So the built-in theme has `--edge`, `1px solid
var(--border-faint)`, which is the border of a window, a menu, a
tooltip, the panel's popovers, the launcher, a notification, the
switcher and "About"; and `--blur`, 32 pixels, which is how far what
is behind glass is blurred wherever there is glass, beside the tint it
already had one name for, `--glass-thick`. `--edge: none` does without
the hairline everywhere, and a `border` of a part's own changes that
part alone. Menus and the rest had `--border`, a little stronger; they
are now as faint as the window's. What asks for a password keeps its
own tint and its dimming: those are there for safety, not for looks.
The bar has no edge, as before.

**The hairline is seen through.** With `--border-faint` on real
hardware it was still more than was wanted, and the maintainer asked
for one subtler yet. It is now light at 8 in a hundred in the dark
scheme, `rgba(255, 255, 255, 0.08)`, and navy at 10 in the light: a
line that takes its colour from what is under it, so that it is there
over anything and loud over nothing, where one colour is either lost
on a ground like itself or stands out on one unlike it. Round a window
it is over what is behind the window; along the top of a titlebar,
over the titlebar's glass. A theme that wants a hairline of one colour
gives `--edge` one.

**GTK 3 is told to read the portal.** Outside a sandbox GTK 3 reads
the desktop's settings from GNOME's own store, where the buttons are
whatever GNOME was last told, and from the portal only with
`GTK_USE_PORTAL=1`, which the session now sets for what it starts:
Firefox's buttons then stand where omorfos's do. GTK 4 reads the
portal unasked, which ADR 68 had the other way about: measured on
18 November 2026 with GTK 3.24.49 and 4.18.6. A program already
running keeps the layout it started with.

**omorfos's own programs will put their controls in the titlebar**, as
GNOME Files and the Finder do: back and forward, where one is, a field
to search in, where a plain window has its title. Each will draw its
own, in this look and taller by what a control needs. That is built
with the first of them (stage 7), and is why the plain titlebar is as
tall as it is: a control of 32 has room in it.

## Alternatives

- **The buttons kept at the right**, where ADR 68 put them so that
  every window had them on one side. Programs that listen to nothing,
  Discord, Steam and Spotify among them, keep theirs at the right, so
  there are two sides again. The maintainer prefers the left and
  accepts that.
- **Other buttons**: signs always shown, rounded squares, rings filled
  under the pointer, grey circles with signs as GNOME has them, signs
  alone. Tried in the mock; the dots were chosen.
- **No hairline at all.** Chosen in the mock and built; taken back once
  seen on real hardware.
- **Writing GNOME's own setting**, for GTK 3 to read. It would change
  a GNOME session of the same user's.
- **The side as a setting.** Left for the review of the defaults, as
  ADR 68 left it.

## Consequences

- A frame's window is 20 pixels shorter in the same frame than it was.
- Glass behind titlebars is drawn for every framed window that can be
  seen. What that costs has not been measured on a modest laptop
  (ADR 33, and the roadmap's item for it); it is to be, with `omorfosctl
  frames`, and the built-in titlebar made solid again if it costs a
  frame.
- Where nothing changes, a glass titlebar costs a turn 7 allocations
  for each window, beside the 17 of a window in a solid frame: the
  tests of a quiet session hold both, the second as a ceiling of its
  own, and both are to come down.
- In a picture of a window by itself, where nothing is behind it, its
  titlebar is solid.
- A theme that takes the edge away has the window's bottom corners cut
  to the curve and not smoothed, nothing being drawn along it; and a
  frame of its against the edge of the screen cannot be held by that
  side to resize it.
- A colour that can be seen through and is not black was drawn far too
  strong by the compositor wherever it filled with one, a border's
  straight sides among them: a hairline of light at 8 in a hundred came
  out white. Nothing built in had been such a colour. The renderer is
  now given such colours as it takes them.
- The welcome card's dots took their colour from the launcher's
  hairline, which is now too faint for them; they take a separator's.
- GTK 3's programs ask the portal for their file dialogs too, which
  the backend for GTK answers, as it did for those in a sandbox.
- The light scheme's unfocused titlebar is all but solid.
- Windows framed by omorfos, GTK's, libadwaita's and Qt's own titlebars
  have their buttons at the left in the same order, and GTK 3's, Firefox
  among them. Whether Chromium follows is to be tried by hand.
- "About" and the welcome card still have their closing buttons at the
  right; they are to follow.
