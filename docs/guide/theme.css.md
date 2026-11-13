# `theme.css`

The theme is a stylesheet, in a small part of CSS. A rule names parts of
the desktop, perhaps in some state, and gives them properties. Yours is
laid over the built-in theme, so you only write what you want changed. A
rule of yours always wins over a built-in one; among your own, the one that
names more states wins, and then the later one.

The whole built-in theme, which is also every part there is, is folded
away here; open it to see how a part is drawn before changing it.

<details>
<summary>The built-in theme</summary>

```css
/* The built-in theme of omorfos, drawn in its design language (DESIGN.md).
 *
 * Your own theme.css is laid over this one: a rule of yours always wins
 * over a rule here, so you only write what you want changed. The quickest
 * change is to give the variables below other values. */

:root {
  --background: #0c1226;     /* behind everything, where there is no wallpaper */
  --surface: #141c38;        /* titlebars, menus and the panel */
  --surface-dim: #10172f;    /* the same, where focus is elsewhere */
  --surface-raised: #243364; /* a field, and a row under the pointer */
  --text: #e6eaf5;
  --text-dim: #9aa4c7;       /* what is beside the point */
  --border: #2b3a6b;         /* a hairline between or around surfaces */
  --border-strong: #7482b3;  /* the edge of what takes input */
  --accent: #22d3ee;         /* what has focus, and what is picked out */
  --on-accent: #0c1226;      /* text on the accent */
  --accent-tint: rgba(34, 211, 238, 0.2); /* behind what is on, faintly */
  --danger: #f8616d;         /* close, and what is urgent or wrong */
  --warning: #fde047;        /* minimize */
  --success: #a3e635;        /* maximize */
  --capture: #f8616d;        /* the screen is being captured */
  /* Glass: the surface, at as much of itself as text on it needs to be
   * read over anything, and light along its top as on the rim of a sheet.
   * Nothing here is glass until what it costs has been measured, but for
   * "About", which is seldom shown;
   *   menu {
   *     background: var(--glass-thick);
   *     backdrop-filter: blur(32px);
   *     box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
   *   }
   * makes a menu glass. Give the surface another colour, and these too. */
  --glass-thin: rgba(20, 28, 56, 0.7);    /* the panel: text on it, never dim */
  --glass-thick: rgba(20, 28, 56, 0.9);   /* menus, notifications, the launcher */
  --glass-sealed: rgba(20, 28, 56, 0.95); /* what asks for a password */
  --glass-rim: rgba(255, 255, 255, 0.06); /* the light along the top */

  /* What programs are told to write code and terminals in, at the size of
   * the rest: omorfos writes nothing in it itself. */
  --font-monospace: JetBrains Mono, monospace;
}

/* Colours take a moment to change when a part's state does. To have them
 * change at once, write `* { transition: none; }` in your theme. */
* {
  font-family: Inter, sans-serif; /* Inter where it is installed */
  font-size: 11pt;
  transition: 120ms ease-out;
}

/* What comes onto the screen takes a little longer than a colour does to
 * change, and leaves quicker than it came, so that what has been dismissed
 * is not in the way of what is next: it comes over its own transition,
 * from how it is in @starting-style, a little small and not yet seen; and
 * it goes over the one it has :closing, to how it is then, a little less
 * small. `scale` is only for this: how large a part is drawn as it comes
 * and goes. */
window, menu, notice, tooltip, panel, notification, launcher, switcher, question, about {
  transition: 180ms ease-out;
}
window:closing, menu:closing, notice:closing, tooltip:closing, panel:closing,
notification:closing, launcher:closing, switcher:closing, question:closing, about:closing {
  transition: 120ms ease-in;
  scale: 98%;
  opacity: 0;
}
@starting-style {
  window, menu, notice, tooltip, panel, notification, launcher, switcher, question, about {
    scale: 96%;
    opacity: 0;
  }
}
/* What dims the screen behind the launcher only fades, being as large as
 * the screen. */
launcher-backdrop:closing { opacity: 0; }
@starting-style {
  launcher-backdrop { opacity: 0; }
}

desktop { background: var(--background); }

/* A window's frame. Its sizes are taken from the rule without a state.
 * Its edge is a hairline with focus or without, as programs that draw
 * their own draw it: focus is in the titlebar, and in the shadow. The
 * shadow says the window floats over the desktop, and how far: the one
 * with focus is lifted higher. A shadow is black and falls straight down;
 * `box-shadow: none` does without it. */
window {
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.14);
  /* What is behind a program, blurred where it asks to be see-through.
   * `backdrop-filter: none` gives programs no blur. */
  backdrop-filter: blur(24px);
}
window:focus { box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28); }

/* The line between windows that share a workspace. */
tile-line { color: var(--border); }

/* A window offered in a part with nothing in it: the box round its picture
 * and its title, and the title, brighter under the pointer. */
offer {
  color: var(--text-dim);
  border: 1px solid var(--border);
  border-radius: 10px;
}
offer:hover { color: var(--text); }

titlebar {
  height: 28px;
  background: var(--surface-dim);
  color: var(--text-dim);
}
titlebar:focus {
  background: var(--surface);
  color: var(--text);
}

/* A title in a titlebar. Where there are several, the border on the left
 * is the line between one tab and the next. */
tab {
  padding: 0 12px;
  background: var(--surface-dim);
  color: var(--text-dim);
  border-left: 1px solid var(--border);
}
tab:active { border-bottom: 2px solid var(--border); }
tab:active:focus {
  background: var(--surface);
  color: var(--text);
  border-bottom-color: var(--accent);
}

/* The buttons at the right of a titlebar: minimize, maximize, close. Each
 * is a box in its share of the titlebar, as wide as its min-width with its
 * margin around it, and its color is that of the sign drawn on it. They
 * are :focus with their frame and :hover under the pointer. Give one a
 * min-width of 0 to do without it. */
button-close, button-maximize, button-minimize {
  min-width: 12px;
  margin: 8px 4px;
  border-radius: 6px;
  background: var(--border);
  color: transparent;
}
button-close { margin-right: 12px; }
button-close:focus, button-close:hover { background: var(--danger); }
button-maximize:focus, button-maximize:hover { background: var(--success); }
button-minimize:focus, button-minimize:hover { background: var(--warning); }
button-close:hover, button-maximize:hover, button-minimize:hover {
  color: var(--background);
}

/* What floats over windows casts a deeper shadow than they do. */
menu {
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
/* A sign is 8 from the row's start, from its top and bottom, and from its
 * label. */
menu-item {
  height: 32px;
  padding: 0 12px 0 8px;
  color: var(--text);
}
menu-item:hover { background: var(--surface-raised); }
menu-separator { color: var(--border); }
/* What stands before a row's label, in the accent on a faint square of it
 * while what the row turns on and off is on. */
menu-sign {
  padding: 3px;
  border-radius: 6px;
  color: var(--text);
}
menu-sign:active {
  background: var(--accent-tint);
  color: var(--accent);
}

/* What is wrong with the configuration, when it was last read. */
notice {
  padding: 12px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--danger);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
notice-heading { color: var(--danger); }

tooltip {
  padding: 4px 8px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}

/* The mark shown while a program takes pictures of the screen. */
capture {
  background: var(--background);
  color: var(--capture);
  border-radius: 6px;
}

/* The line round a screen or a window while it is shared or recorded,
 * which is in no picture of it. */
capture-border { border: 2px solid var(--capture); }

/* The panel, and what is on it: the workspaces, a task for each window on
 * the one being shown, and the clock. A margin on the panel moves it in
 * from the edges of the screen. All that is written on it is in the full
 * colour of text, as it is to be when it is thin glass. */
panel {
  height: 28px;
  background: var(--surface);
  color: var(--text);
}
workspace, task {
  padding: 0 12px;
  color: var(--text);
}
workspace { min-width: 28px; }
task { max-width: 240px; }
workspace:active, task:active {
  background: var(--surface-raised);
  color: var(--text);
  border-bottom: 2px solid var(--accent);
}
clock {
  padding: 0 12px;
  color: var(--text);
}

/* The launcher: where a name is typed, and the programs that match. The
 * launcher is as wide as its min-width. */
launcher {
  min-width: 480px;
  padding: 12px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
launcher-input {
  height: 32px;
  margin: 0 0 8px 0;
  padding: 0 8px;
  background: var(--surface-raised);
  color: var(--text);
  border-radius: 6px;
}
launcher-item {
  height: 28px;
  padding: 0 8px;
  color: var(--text);
  border-radius: 6px;
}
launcher-item:active {
  background: var(--accent);
  color: var(--on-accent);
}
/* What a program says of itself, where the launcher is set to show it. */
launcher-description { color: var(--text-dim); }
launcher-description:active { color: var(--on-accent); }
/* The rest of the screen while the launcher is open. A click on it closes
 * the launcher. Give it a see-through colour to dim what is behind. */
launcher-backdrop { background: transparent; }

/* A notification. The margin is the room between one and the next, and
 * between them and the edges of the screen. One a program marks critical
 * is :urgent. */
notification {
  min-width: 340px;
  margin: 8px;
  padding: 12px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
notification:hover { border-color: var(--border-strong); }
notification:urgent { border-color: var(--danger); }
notification-title {
  color: var(--text);
  font-weight: 600;
}
notification-body { color: var(--text-dim); }
/* How full something a notification tells of is, such as the volume: the
 * background is the whole, the colour how much of it is full. */
notification-level {
  margin-top: 8px;
  background: var(--border);
  color: var(--accent);
  border-radius: 4px;
}

/* What the portals ask: whether an application may see the screen, and
 * what asks for a password. The button that gives is :active. The border
 * is also the ring of the loupe a colour on the screen is picked with. */
question {
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
question-detail { color: var(--text-dim); }
question-button {
  background: var(--surface-dim);
  color: var(--text);
  border-radius: 6px;
}
question-button:hover { background: var(--surface-raised); }
question-button:active {
  background: var(--accent);
  color: var(--on-accent);
}
/* What the keyboard is on has a ring of the accent, a little way out from
 * it, which shows round a button filled with the accent as well as round
 * one that is not. */
question-button:focus {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
question-input {
  padding: 8px;
  background: var(--surface-raised);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: 6px;
}
question-input:focus { border-color: var(--accent); }
/* What is typed, when all of it is chosen, as Ctrl+A chooses it. */
question-input:active {
  background: var(--accent);
  color: var(--on-accent);
}

/* The switcher on Alt+Tab: its box, each window's place in rows, and the
 * title. In rows, the box is in the middle of the screen, each place in it
 * with its picture over its title, as wide as its min-width. On a diagonal,
 * the box is under the queue, with the title of the one picked. The one
 * picked is :active. */
switcher {
  padding: 12px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
switcher-item {
  min-width: 200px;
  padding: 8px;
  border: 2px solid transparent;
  border-radius: 6px;
}
switcher-item:active {
  background: var(--surface-raised);
  border-color: var(--accent);
}
switcher-title { color: var(--text); }
/* On a diagonal, each window in the switcher: its border round its picture,
 * its padding away. The one picked is :active. */
switcher-window {
  padding: 4px;
  border: 2px solid transparent;
  border-radius: 10px;
}
switcher-window:active { border-color: var(--accent); }
/* The screen behind the switcher: dimmed, and blurred out of focus. */
switcher-backdrop {
  background: rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(16px);
}
/* The overview: each window in it, its border round its picture, its
 * padding away, the one picked :active; the title of the one picked, under
 * it; each workspace in the strip along the top, the one shown :active
 * and one under the pointer :hover; the button after them that makes a
 * new workspace, its plus in its colour; and the screen behind it all,
 * dimmed and blurred out of focus. */
overview-window {
  padding: 4px;
  border: 2px solid transparent;
  border-radius: 10px;
}
overview-window:active { border-color: var(--accent); }
overview-title {
  padding: 4px 10px;
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
}
overview-workspace {
  border: 2px solid var(--border);
  border-radius: 6px;
}
overview-workspace:hover { border-color: var(--text); }
overview-workspace:active { border-color: var(--accent); }
overview-add {
  border: 2px solid var(--border);
  border-radius: 6px;
  color: var(--text);
}
overview-add:hover { border-color: var(--text); }
overview-backdrop {
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(16px);
}

/* The screens shown still, to pick what a picture is to be of: what is not
 * picked is dimmed by the background, and what is drawn has the border. */
picker {
  background: #00000066;
  border: 2px solid var(--accent);
}

/* What dims the session while a prompt asks for a secret, such as the
 * administrator's password, and blurs it: nothing a program draws can dim
 * and blur it so. `backdrop-filter: none` dims it alone. */
sealed {
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(24px);
}

/* "About omorfos", from the root menu: the name and version, what the
 * computer is, and what is someone else's under its headings, whose each
 * is dimmer. It is glass, as glass is drawn everywhere; it is closed by a
 * window's button, and its own button copies what the computer is. */
about {
  padding: 20px;
  background: var(--glass-thick);
  backdrop-filter: blur(32px);
  color: var(--text);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
about-heading {
  color: var(--text);
  font-weight: 600;
}
about-detail { color: var(--text-dim); }
about-button {
  padding: 4px 12px;
  background: var(--surface-dim);
  color: var(--text);
  border-radius: 6px;
}
about-button:hover { background: var(--surface-raised); }
/* For a moment once it has copied, it says so, in the colour of what is
 * done, with a tick. */
about-button:active {
  background: var(--success);
  color: var(--background);
}
```

</details>

That is the theme in its dark scheme. With `appearance.scheme: light` in
`config.yaml`, these are laid over it, so that the same rules draw it
light:

```css
/* The light scheme, laid over the built-in theme by
 * `appearance: { scheme: light }`: the same variables, in a warm cream
 * where the dark scheme has navy. The accent, and what is written on it,
 * come from `appearance.accent`. */
:root {
  --background: #fcf7e3;
  --surface: #fffdf4;
  --surface-dim: #f6f0da;
  --surface-raised: #ffffff;
  --text: #0c1226;
  --text-dim: #5c6888;
  --border: #e4ddc4;
  --border-strong: #7a8099;
  --danger: #c4303a;
  --warning: #8a5a00;
  --success: #2f7a35;
  --capture: #c4303a;
  --glass-thin: rgba(255, 253, 244, 0.7);
  --glass-thick: rgba(255, 253, 244, 0.9);
  --glass-sealed: rgba(255, 253, 244, 0.95);
  --glass-rim: transparent; /* light on cream is not seen */
}
```

`appearance.accent` gives `--accent`, `--on-accent`, what is written on
it, and `--accent-tint`, the accent faint enough to stand behind what is
in the accent, in either scheme. Each of `cyan`, `blue`, `teal`, `purple`, `pink`
and `orange` has a colour for each scheme, deeper in the light one, as a
colour clear on navy is too pale on cream, and text on it that can be
read. A colour of your own, as `'#3584e4'`, is the same in both, and what
is written on it is whichever of the scheme's dark and light text stands
out from it more. Your own theme is laid over all of this, and has the
last word: a `--background` of yours is the background in either scheme.
A change of either is drawn as soon as `config.yaml` is saved, and
applications that follow the desktop are told it.

With `appearance.scheme: auto` the desktop is light by day and dark by
night: from sunrise to sunset, worked out to within a minute or two,
where `appearance.location` says. That is the time zone the system is set
to, `system`, unless you say otherwise: a time zone by its name, as
`Europe/London`, or your latitude and longitude in degrees, north and east
above 0. A time zone is taken to be where the time zone database puts its
principal city, which for most of a zone is within some minutes of the
sun there. Where the sun does not set or does not rise that day, it is
light or dark all day. Given `appearance.day` and no location, day is
those hours instead, as `{ from: '07:00', to: '19:00' }`, in the time zone
of the system; a day that ends earlier than it begins runs through
midnight. A system whose time zone has no city, as `UTC`, goes by the
hours, `07:00` to `19:00` unless you say otherwise. As day turns into
night, or night into day, the colours of what omorfos draws, its panel
and its notifications change over a minute, and applications that follow
the desktop are told as it begins. Each part asks again by every midnight,
and when the clock is set, as after a suspend. A scheme set by hand, as
`scheme: dark`, changes at once. The location is only read here, never told to any program. A
change of time zone is seen at the next of these, not at once; and the
lock screen and the box that asks for a password stay in the scheme they
were opened in.

The quickest change is to give the built-in theme's variables other
values:

```css
:root {
  --accent: #e0af68;
  --background: #101014;
}
```

A panel that floats clear of the screen's edges, see-through, with rounded
buttons, is a few rules more:

```css
panel {
  margin: 6px 8px;
  border-radius: 10px;
  background: rgba(27, 29, 35, 0.8);
}
workspace, task {
  margin: 3px 2px;
  border-radius: 6px;
}
workspace:active, task:active { border-bottom: none; }
```

**Parts** are `desktop`, `window`, `tile-line`, `offer`, `titlebar`, `tab`, `button-close`,
`button-maximize`, `button-minimize`, `menu`, `menu-item`,
`menu-separator`, `menu-sign`, `notice`, `notice-heading`, `capture`, `capture-border`, `tooltip`, `panel`,
`workspace`, `task`,
`clock`, `launcher`, `launcher-input`, `launcher-item`,
`launcher-description`, `launcher-backdrop`, `notification`,
`notification-title`, `notification-body`, `notification-level`, `question`, `question-detail`,
`question-button`, `question-input`, `switcher`, `switcher-item`, `switcher-title`,
`switcher-window`, `switcher-backdrop`, `overview-window`, `overview-title`,
`overview-workspace`, `overview-add`, `overview-backdrop`, `picker`,
`sealed`, `about`, `about-heading`, `about-detail` and `about-button`. `*` is every part. Several can share a rule:
`workspace, task { ... }`.

**States** follow a part: `:focus` (it has the keyboard, or its window
has), `:active` (the tab or workspace being shown, or the task of the
window with focus, or the program the launcher would start, or the sign of
a row of the menu whose window or session is so), `:hover` (the
pointer is over it), `:urgent` (a notification its sender marked as
critical) and `:closing` (it is going from the screen). A part can be in
several: `tab:active:focus`.

**Coming and going.** A part that comes and goes, as a window, the menu,
the notice or a component's panel, notification or launcher does, comes
over its own `transition`, from how it is in `@starting-style`, and leaves
over the one it has `:closing`, for how it is then; so that it can leave
quicker than it came:

```css
menu { transition: 180ms ease-out; }
menu:closing { transition: 120ms ease-in; scale: 98%; opacity: 0; }
@starting-style {
  menu { scale: 96%; opacity: 0; }
}
```

Of each, only how see-through the part is (`opacity`) and how large it is
drawn (`scale`) are taken so far: its colours stay as they are while it
comes and goes. What it draws is made larger or smaller about its middle,
its shadow and its glass with it. A part with no `@starting-style` is there
at once, as in CSS, and one whose `:closing` look can be seen stays as it
is until its time to leave is up. The built-in theme gives both to the
parts that come and go, as above, and has `launcher-backdrop` fade in and
out without changing size, so yours keeps them unless it says otherwise.
`@starting-style` holds rules for parts in no state, and nothing else.

**Nothing moving.** With `appearance.reduce_motion: true` in `config.yaml`,
whatever the theme says, a part that comes and goes keeps its size and
only fades, and no change, of a colour or of a part coming and going,
takes longer than 120 ms. Workspaces change without sliding, whether by
a key or by three fingers, which change it as they lift if they went far
enough or fast enough; the box that asks for a password, and the lock
screen's field, do not shake at a wrong one. Programs are told, to move
nothing either: through the settings portal's `reduced-motion` and GNOME's
`enable-animations`, which GTK and libadwaita read, in a sandbox too, and
as `Gtk/EnableAnimations` to programs written for X11. Day turning into
night under `scheme: auto` still takes its minute, being a change of
colour and not a movement.

**Every part is a box.** From the outside in it has a margin, which is room
kept clear around it; a border; padding, which is room between the border
and what the part holds; and its content. Its height, where it has one,
is measured to the outside of its border.

| Property | Value | Notes |
|---|---|---|
| `background` | a colour, a gradient, or a colour and a gradient over it | what the part is filled with; what it leaves out is none, as in CSS, so a gradient alone has no colour under it. `background-color` gives the colour alone and `background-image` the gradient alone, or `none` |
| `color` | a color | text; for `menu-separator` and `tile-line`, the line |
| `opacity` | `0` to `1`, or a percentage | of everything the part draws |
| `scale` | `0` to `2`, or `0%` to `200%` | how large the part is drawn as it comes and goes, as a share of its size: in `@starting-style`, and while it is `:closing`, and nowhere else |
| `border` | a width, `solid` or `none`, a color | any of them, in any order |
| `border-color`, `border-width` | a color; `0` to `16px` | |
| `outline` | a width, `solid` or `none`, a color | a line round the outside of the box, any of them in any order; its corners are rounded by as much more than the box's as it is out, unless the box's are square. A ring round what the keyboard is on, as `question-button:focus` has it; the eye in the password field has its ring close round it, there being no room to stand off |
| `outline-color`, `outline-width`, `outline-offset` | a color; `0` to `16px`; `0` to `16px` | how far out the outline is, with nothing drawn between |
| `border-radius` | `0` to `32px` | rounds the corners of the box; a `menu`'s rows are kept to its corners, so the first and last, highlighted, follow the curve, as `overflow: hidden` would have it |
| `padding`, `margin` | one to four sizes, `0` to `64px` | all; top and bottom, then sides; top, sides, bottom; or top, right, bottom, left. A `menu`'s rows begin inside its border and padding: with none, as in the built-in theme, the first and last reach its border; `menu { padding: 4px 0; }` leaves room above and below them |
| `height` | `16px` to `64px` | of `titlebar`, `menu-item`, `panel`, `launcher-input` and `launcher-item` |
| `min-width`, `max-width` | `0` to `4096px` | of `workspace`, `task` and `clock`; `max-width: 0` is no limit. The launcher is as wide as its `min-width`, and so is each button of a titlebar, which is not there at all if that is `0` |
| `font-family`, `font-size` | a name, or names apart by commas; `6pt` to `72pt` | the first of the names that is installed, or a generic one such as `sans-serif`, is used, and `sans-serif` if none is: the built-in theme's `Inter, sans-serif` is Inter where it is installed. Programs are told the font of `menu-item`, which is as most text is, as GTK writes one, `Inter, sans-serif 11`: in the settings toolkits follow under X11 and through the settings portal's `font-name`. The variable `--font-monospace`, names as `font-family` takes them, is the monospaced font programs are told for code and terminals, at the same size, `JetBrains Mono, monospace 11`, through the settings portal's `monospace-font-name`; X11 has no setting for it |
| `font-weight`, `font-style` | `normal`, `bold`, `100` to `900`; `normal`, `italic` | |
| `transition` | a time up to `10s`, perhaps a second up to `10s`, and perhaps `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` or `cubic-bezier(x1, y1, x2, y2)`; or `none` | how long the part's colours take to change when its state does, how long they wait first, and how; also `transition-duration`, `transition-delay` and `transition-timing-function`. In `cubic-bezier()` each x is from 0 to 1 and each y from -10 to 10, as in CSS; a curve that goes before its start or past its end goes no further than either |
| `box-shadow` | `none`, or across and down, perhaps how soft and how far it spreads, in px, then a color | the shadow the part casts, as `0 8px 24px rgba(0, 0, 0, 0.28)`: across and down from `-64px` to `64px`, soft from `0` to `64px`, spread from `-32px` to `32px`. After a comma, or alone, one `inset`: a sharp line inside the border along the side it falls from, as wide as it falls, following the corners, as `inset 0 1px rgba(255, 255, 255, 0.06)` is light along the top, with neither a soft edge nor a spread; drawn wherever the part is drawn as a box, its titlebar among them. Cast by `window`, but not while it is maximized or fullscreen, nor by a program that draws its own frame; by `menu`, and by the pictures of arrangements, which are drawn as a menu; by `notice` and `tooltip`; by `switcher`, under the switcher on Alt+Tab and the windows shown in it; by the parts the components show, such as `launcher`; and by `question` under the box that asks for a password, which it follows as the box shakes, and under what an application asks through the portals. Each notification casts its own; the shadow of one panel or notification does not fall on another beside it. It changes with the part's state, as its colours do |
| `backdrop-filter` | `none`, or `blur()` of `0px` to `64px` | makes the part glass: what is behind it, blurred by as much as a Gaussian's deviation, as CSS has it, but never less than `16px`, below which what is behind could still be read, is drawn under the part where its `background` lets it be seen. A part whose background cannot be seen through has none. Under `menu`, and the pictures of arrangements, which are drawn as a menu; `notice`; `tooltip`, under what the mark says; `switcher`; `question`, under the box that asks for a password, and what an application asks through the portals; `sealed`, under the dim of every screen while a password is asked for, which is blurred and is the dim alone where blur cannot be drawn; `switcher-backdrop`, likewise under the dim of the screen the switcher is on; `overview-backdrop`, under the dim of the screen the overview is on; and the parts the components show, such as `panel` and `notification`, under each surface. Under `titlebar`, with its top corners rounded as the window's are; a `tab` in a frame of several is drawn over it, and covers it unless its own `background` can be seen through too. On `window`, how far what is behind a program is blurred where it asks to be see-through (`ext-background-effect-v1`, or KDE's `org_kde_kwin_blur`, which Ghostty's `background-blur` asks through); `none` gives programs no blur, and those that ask through the first are told so. Such glass has no tint, the program drawing its own, and is nothing at all where blur cannot be drawn. A part keeps its glass as it fades out. Glass is drawn solid in the part's `background`, or halfway along its gradient where it has no colour, made opaque, where blur cannot be drawn: on a graphics card whose blur, tried as omorfos starts, is not as it should be, which you are told once; and with `appearance.reduce_transparency` |

`border`, `padding` and `margin` can name one side: `border-bottom`,
`border-left-color`, `padding-top`, `margin-right`.

Colors are written as CSS writes them: `#rgb` or `#rrggbb`, with opacity
as a fourth digit or pair; `rgb(91, 156, 245)` and `rgba(91, 156, 245,
0.5)`; `hsl(215, 89%, 66%)`; a name such as `steelblue`; or `transparent`.
Sizes are whole numbers of `px`, and of `pt` for a font. A variable is
declared in `:root` with a name beginning `--` and used as `var(--name)`,
alone or as part of a value: `border: 1px solid var(--accent)`.

**A part may be filled with a gradient**, `linear-gradient()`, as in
CSS: perhaps which way it runs, as an angle (`90deg`, `0.25turn`) or a
side or corner (`to right`, `to top left`), down if it does not say; then
two to eight colours, each perhaps with how far along it is (`40%`). A
colour placed nowhere is halfway between those either side. It runs across
what is inside the border, from one side to the other whatever the part's
shape, and is drawn over the part's colour, so `background: #222
linear-gradient(to right, transparent, #fff3)` brightens a dark part
towards its right. A gradient is drawn wherever a part is a box, its
titlebar among them; where a part is one colour across the screen, the
desktop and the dims behind the launcher, the prompt and a picture being
taken, or the colour of chosen text, it is the gradient's colour halfway
along. A gradient changes at once when the part's state does, its colour
under it as `transition` says.

**Colours change over time.** When a part changes state, as a window does
on gaining focus or a menu row when the pointer comes onto it, its
background, text and border colours go from the old to the new over the
time its `transition` gives, written as `150ms` or `0.2s`. It is the
`transition` of the state being entered that counts, so
`menu-item:hover { transition: none; }` lights a row at once and lets it
fade when the pointer leaves. Everything takes 120ms unless you say
otherwise; `* { transition: none; }` turns it all off. A second time is
how long the colours wait before they change, as in CSS: `menu-item:hover
{ transition: 80ms ease-out 300ms; }` lights a row only once the pointer
has rested on it. Only colours
change this way. Sizes change at once, so that nothing moves under the
pointer, and a `transition` names no property: all the colours of a part
go together. Something drawn for the first time has its colours from the
start.

**What comes onto the screen fades in, and what goes fades out**, over
the same time: a window with its frame over `window`'s `transition`, the
menu over `menu`'s, the notice over `notice`'s, and the panel, the launcher
with what is behind it, and the notifications over `panel`'s, `launcher`'s,
`launcher-backdrop`'s and `notification`'s. A window goes and comes each
time it is hidden and shown: when its workspace is left and returned to, or
when it is minimized and brought back, so that one workspace fades into
the next. Showing another tab of a frame is neither. `window { transition:
none; }` has windows there and gone at once and leaves everything else as
it was. A panel of a program that is not part of omorfos comes and goes at
once, and so does one notification among others that stay, and the
question an application has asked, whatever `question`'s `transition`, so
that it is not in the picture of the screen it leads to.

**A window that changes size fills its frame at once.** Until its
program has drawn it at the size it has been given, by being maximized,
tiled or dragged larger or smaller by its edge, its last picture is
stretched to fill the frame; once the program has drawn, that picture
fades into the new one over `window`'s `transition`, unless you were
dragging it, or the line beside it, when the new one is shown at once.
A program that has
drawn and chosen another size, as a terminal may to fit its lines, is
shown as it draws. `window { transition: none; }` leaves the picture as
it is, short of the frame or past it, until the program draws.

**A window sent somewhere eases there.** Maximized or restored, tiled,
arranged, changed over with another or given the room of one that
closed or left, a window's frame goes there over 240 ms, easing in and
out, its picture stretched to fill it on the way; the program is told
at once how large it is to be, and nothing in between. What you drag,
a window or the line between two, follows the pointer at once. A theme
whose `window` changes at once, or `appearance.reduce_motion`, has the
frame there at once.

Not every part uses every property yet:

- `margin` is honoured by `panel` (it moves the bar in from the edges of
  the screen), by `workspace`, `task` and `clock` (room around each on
  the bar), and by the buttons of a titlebar (room around each in the
  titlebar, which is how one is made smaller than the titlebar is tall).
- The buttons of a titlebar are drawn from `background`, `border`,
  `border-radius` and `color`, which is the colour of the sign on each: a
  cross, a plus and a bar. They stand at the right, the cross outermost,
  and programs that draw their own titlebars are told the same.
- `padding` is honoured by `tab`, `menu-item`, `menu-sign`, `notice`,
  `panel`, `workspace`, `task` and `clock`.
- `border-radius` is honoured by `window`, `tab`, `menu`, `menu-item`,
  `menu-sign`, `notice`, `capture`, `panel`, `workspace`, `task` and
  `clock`.
- A window's border is the same on every side: its `border-top` stands for
  all four. The sizes of a window's frame and titlebar are taken from the
  rules without a state, since a window does not change size when it gains
  focus.
- Layout (`display`, `position`, `flex`), `radial-gradient()`, more than
  one background, more than one shadow cast or inside, and a shadow
  inside with a soft edge or a spread are not there, and nor are
  `steps()` or `@keyframes`.

Nothing is passed over in silence: a part, state, property or value that
is not understood is a mistake, reported with its line and column, and the
previous theme stays in use, in the scheme and with the accent it was in:
an `appearance.scheme` or `appearance.accent` changed meanwhile waits until
the theme is put right. Where there was no theme of yours to keep, as when
the session starts with one that is wrong, it is the built-in theme in the
scheme and with the accent `config.yaml` gives. A `theme.yaml` from an
earlier version is no longer read, and omorfos says so while it is there.
