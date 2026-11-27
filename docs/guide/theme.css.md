# `theme.css`

The theme is a stylesheet, in a part of CSS. Everything the desktop draws
is an element in a tree, as a web page's are: a `button` in a titlebar's
`controls`, an `item` in the launcher's `list`. A rule names elements as
CSS does, by type, class, state and where they are, and gives them
properties: `button { border-radius: 8px; }` rounds every button there
is. Yours is laid over the built-in theme, so you only write what you want
changed. A rule of yours always wins over a built-in one, however
specific the built-in one is; among your own, the more specific wins, as
in CSS, and then the later one.

The whole built-in theme, which also names every element there is, is
folded away here; open it to see how an element is drawn before changing
it.

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
  --surface: #141c38;        /* what is solid; the glass below is this, seen through */
  --surface-dim: #10172f;    /* the same, where focus is elsewhere */
  --surface-raised: #243364; /* a field, and a row under the pointer */
  --text: #e6eaf5;
  --text-dim: #9aa4c7;       /* what is beside the point */
  --text-faint: #727b9c;     /* what is only there to fill, as other months' days */
  --border: #2b3a6b;         /* a hairline between or around surfaces */
  --border-faint: #222e57;   /* fainter, as the lines between days */
  --border-strong: #7482b3;  /* the edge of what takes input */
  --accent: #22d3ee;         /* what has focus, and what is picked out */
  --on-accent: #0c1226;      /* text on the accent */
  --accent-tint: rgba(34, 211, 238, 0.2); /* behind what is on, faintly */
  --danger: #f8616d;         /* what is urgent or wrong */
  --warning: #fde047;        /* what to be wary of */
  --success: #a3e635;        /* what is done, or well */
  --capture: #f8616d;        /* the screen is being captured */
  /* Glass: the surface, at as much of itself as text on it needs to be
   * read over anything, and light along its top as on the rim of a sheet.
   * Everything that floats is glass, and a titlebar, and the bar:
   *   menu {
   *     background: var(--surface);
   *     backdrop-filter: none;
   *   }
   * makes a menu solid again. Give the surface another colour, and these
   * too. */
  --glass-thin: rgba(20, 28, 56, 0.7);    /* the panel: text on it, never dim */
  --glass-thick: rgba(20, 28, 56, 0.8);   /* menus, notifications, the launcher */
  --glass-sealed: rgba(20, 28, 56, 0.95); /* a notice, which is to be read */
  --glass-rim: rgba(255, 255, 255, 0.06); /* the light along the top */
  --glass-thick-dim: rgba(16, 23, 47, 0.8); /* a titlebar, where focus is elsewhere */
  /* What is the same for everything that floats, and for a window: the
   * hairline round it, and how far what is behind it is blurred where it
   * is glass. The hairline is light that can be seen through, so that it
   * is there over anything and loud over nothing. `--edge: none` does
   * without it everywhere; a `border` or a `backdrop-filter` of a part's
   * own changes that part alone. */
  --edge: 1px solid rgba(255, 255, 255, 0.08);
  --blur: 32px;
  /* A titlebar's buttons, which are the same colours in the light scheme,
   * where they have an edge to be seen by on cream. */
  --close: #f8616d;
  --minimize: #fde047;
  --maximize: #a3e635;
  --on-button: #0c1226;      /* the sign on one */
  --button-rim: transparent;

  /* What programs are told to write code and terminals in, at the size of
   * the rest: omorfos writes nothing in it itself. */
  --font-monospace: JetBrains Mono, monospace;
}

/* Colours take a moment to change when a part's state does. To have them
 * change at once, write `*, *::backdrop { transition: none; }` in your
 * theme. */
*, *::backdrop {
  font-family: Inter, sans-serif; /* Inter where it is installed */
  font-size: 11pt;
  transition: 120ms ease-out;
}

/* What comes onto the screen takes a little longer than a colour does to
 * change, and leaves quicker than it came, so that what has been dismissed
 * is not in the way of what is next: it comes over its own transition,
 * from how it is in @starting-style, a little small and not yet seen; and
 * it goes over the one it has :state(closing), to how it is then, a little less
 * small. `scale` is only for this: how large a part is drawn as it comes
 * and goes. */
window,
menu,
notice,
tooltip,
panel,
notification,
launcher,
switcher,
dialog.question,
dialog.about {
  transition: 180ms ease-out;
}
window:state(closing),
menu:state(closing),
notice:state(closing),
tooltip:state(closing),
panel:state(closing),
notification:state(closing),
launcher:state(closing),
switcher:state(closing),
dialog.question:state(closing),
dialog.about:state(closing) {
  transition: 120ms ease-in;
  scale: 98%;
  opacity: 0;
}
@starting-style {
  window,
  menu,
  notice,
  tooltip,
  panel,
  notification,
  launcher,
  switcher,
  dialog.question,
  dialog.about {
    scale: 96%;
    opacity: 0;
  }
}
/* What dims the screen behind the launcher only fades, being as large as
 * the screen. */
launcher:state(closing)::backdrop { opacity: 0; }
@starting-style {
  launcher::backdrop { opacity: 0; }
}

desktop { background: var(--background); }

/* A window's frame. Its sizes are taken from the rule without a state.
 * Its edge is the hairline of everything that floats, with focus or
 * without: focus is in the titlebar, and in the shadow; `border: none`
 * does without it.
 * The shadow says the window floats over the desktop, and how far: the one
 * with focus is lifted higher. A shadow is black and falls straight down;
 * `box-shadow: none` does without it. */
window {
  border: var(--edge);
  border-radius: 12px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.14);
  /* What is behind a program, blurred where it asks to be see-through.
   * `backdrop-filter: none` gives programs no blur. */
  backdrop-filter: blur(24px);
}
window:focus-within { box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28); }

/* The line between windows that share a workspace. */
desktop > separator { color: var(--border); }

/* A window offered in a part with nothing in it: the box round its picture
 * and its title, and the title, brighter under the pointer. */
desktop > button.offer {
  color: var(--text-dim);
  border: 1px solid var(--border);
  border-radius: 10px;
}
desktop > button.offer:hover { color: var(--text); }

/* A titlebar is thick glass: what is behind it blurred, under the surface
 * at as much of itself as text on it needs. `backdrop-filter: none` and a
 * background that cannot be seen through make it solid. */
window > titlebar {
  height: 48px;
  background: var(--glass-thick-dim);
  color: var(--text-dim);
  backdrop-filter: blur(var(--blur));
  box-shadow: inset 0 1px var(--glass-rim);
}
window:focus-within > titlebar {
  background: var(--glass-thick);
  color: var(--text);
}

/* A title in a titlebar: in the middle of it where it is alone. Where
 * there are several, each is a tab, and the border on the left is the
 * line between one tab and the next. */
window > titlebar > tab {
  padding: 0 12px;
  color: var(--text-dim);
  border-left: 1px solid var(--border);
}
window > titlebar > tab:checked { border-bottom: 2px solid var(--border); }
window:focus-within > titlebar > tab:checked {
  color: var(--text);
  border-bottom-color: var(--accent);
}

/* The buttons at the left of a titlebar: close, minimize, maximize. Each
 * is a box in its share of the titlebar, as wide as its min-width with its
 * margin around it, and its color is that of the sign drawn on it. They
 * are in a window that is :focus-within, and each is :hover while the
 * pointer is on it. They are in the titlebar's controls, which are :hover
 * while the pointer is on any of them: named before a button, as below,
 * they have all three show their signs at once. Give one a min-width of 0
 * to do without it. */
window > titlebar > controls > button.close,
window > titlebar > controls > button.minimize,
window > titlebar > controls > button.maximize {
  min-width: 16px;
  margin: 16px 4px;
  border: 1px solid var(--button-rim);
  border-radius: 8px;
  background: var(--border);
  color: transparent;
}
window > titlebar > controls > button.close { margin-left: 12px; }
window:focus-within > titlebar > controls > button.close,
window > titlebar > controls:hover > button.close { background: var(--close); }
window:focus-within > titlebar > controls > button.minimize,
window > titlebar > controls:hover > button.minimize {
  background: var(--minimize);
}
window:focus-within > titlebar > controls > button.maximize,
window > titlebar > controls:hover > button.maximize {
  background: var(--maximize);
}
window > titlebar > controls:hover > button.close,
window > titlebar > controls:hover > button.minimize,
window > titlebar > controls:hover > button.maximize {
  color: var(--on-button);
}

/* What floats over windows casts a deeper shadow than they do. */
menu {
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: var(--edge);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
/* A sign is 8 from the row's start, from its top and bottom, and from its
 * label. */
menu > item {
  height: 32px;
  padding: 0 12px 0 8px;
  color: var(--text);
}
menu > item:hover { background: var(--surface-raised); }
/* The row the pointer's button is held on, chosen as it is let go there
 * (ADR 113). */
menu > item:active { background: var(--accent-tint); }
menu > separator { color: var(--border); }
/* What stands before a row's label, in the accent on a faint square of it
 * while what the row turns on and off is on. */
menu > item > icon {
  padding: 3px;
  border-radius: 6px;
  color: var(--text);
}
menu > item:checked > icon {
  background: var(--accent-tint);
  color: var(--accent);
}
/* The keys that do what a row does, at its end. */
menu > item > kbd { color: var(--text-dim); }
/* The row the keyboard is on, in the accent: seen from across the room,
 * where the pointer's is only raised. */
menu > item:focus {
  background: var(--accent);
  color: var(--on-accent);
}
menu > item:focus > icon { color: var(--on-accent); }
menu > item:focus > kbd { color: var(--on-accent); }

/* What is wrong with the configuration, when it was last read. */
/* Its glass is as thick as the prompt's: what is wrong is said in red,
 * which is read over a white window at no less. */
notice {
  padding: 12px;
  background: var(--glass-sealed);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: 1px solid var(--danger);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
notice > heading { color: var(--danger); }

tooltip {
  padding: 4px 8px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: var(--edge);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}

/* The mark shown while a program takes pictures of the screen. */
capture {
  background: var(--background);
  color: var(--capture);
  border-radius: 6px;
}

/* The line round a screen or a window while it is shared or recorded,
 * which is in no picture of it. */
frame.capture { border: 2px solid var(--capture); }

/* The panel, and what is on it: the workspaces, a task for each window on
 * the one being shown, and the clock. A margin on the panel moves it in
 * from the edges of the screen. All that is written on it is in the full
 * colour of text, as it is to be when it is thin glass. */
panel {
  height: 28px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
}
panel > workspace, panel > task {
  padding: 0 12px;
  color: var(--text);
}
panel > workspace { min-width: 28px; }
panel > task { max-width: 240px; }
panel > workspace:checked, panel > task:checked {
  background: var(--surface-raised);
  color: var(--text);
  border-bottom: 2px solid var(--accent);
}
panel > clock {
  padding: 0 12px;
  color: var(--text);
}

/* What hangs from a bar, as the month from the clock: a box as a menu's,
 * and its rows and buttons. The days of a month are a faint grid, with
 * room between them; the week and the weekday of the day under the
 * pointer stand out more, that day the most, and today is in the accent.
 * The days of the months beside it, filling its weeks, are faded. */
popover {
  padding: 8px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: var(--edge);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
popover > item {
  color: var(--text);
  border-radius: 6px;
}
popover > item:hover { background: var(--surface-raised); }
popover > calendar > day, popover > calendar > day.outside {
  height: 32px;
  min-width: 32px;
  margin: 2px;
  color: var(--text);
  border: 1px solid var(--border-faint);
  border-radius: 6px;
}
popover > calendar > day.outside { color: var(--text-faint); }
popover > calendar > day:state(in-line),
popover > calendar > day.outside:state(in-line) { border-color: var(--border); }
popover > calendar > day:hover, popover > calendar > day.outside:hover {
  background: var(--surface-raised);
  border: 2px solid var(--accent);
}
popover > calendar > day:state(today),
popover > calendar > day.outside:state(today) {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--on-accent);
}
popover > calendar > day:state(today):hover,
popover > calendar > day.outside:state(today):hover {
  border-color: var(--text);
}

/* The launcher: where a name is typed, and the programs that match. The
 * launcher is as wide as its min-width. */
launcher {
  min-width: 480px;
  padding: 12px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: var(--edge);
  /* With nothing typed, a bar with half circles for ends, round about
   * its field's: half as tall as the two of them, its field and its
   * border and padding. */
  border-radius: 41px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
  /* A little slower to come than the rest, and slower again to leave. */
  transition: 220ms ease-out;
}
launcher:state(closing) {
  transition: 140ms ease-in;
  scale: 96%;
}
/* The field is large, and of the launcher's own glass, as Spotlight's. */
launcher > input {
  height: 56px;
  margin: 0 0 8px 0;
  padding: 0 8px;
  color: var(--text);
  font-size: 20pt;
  border-radius: 28px;
}
/* What is typed, when all of it is chosen, as Ctrl+A chooses it. */
launcher > input::selection {
  background: var(--accent);
  color: var(--on-accent);
}
/* A row has two lines: the name, and under it what the program says of
 * itself, if the launcher shows that; its icon is as tall as both. */
launcher > list > item {
  height: 56px;
  padding: 8px 14px;
  color: var(--text);
  border-radius: 28px;
}
launcher > list > item:checked {
  background: var(--accent);
  color: var(--on-accent);
}
/* What a program says of itself, where the launcher is set to show it. */
launcher > list > item > description { color: var(--text-dim); }
launcher > list > item:checked > description { color: var(--on-accent); }
/* A program in the grid of every program: its icon, half as wide as the
 * room inside its padding, over its name. It is as wide as its min-width. */
launcher > grid > item {
  min-width: 112px;
  padding: 8px;
  color: var(--text);
  border-radius: 16px;
}
launcher > grid > item:checked {
  background: var(--accent);
  color: var(--on-accent);
}
/* The best of all, alone at the top of what is found: outlined while the
 * field has the keyboard, as what Enter takes, and lit as any row. */
launcher > list > item.top {
  height: 56px;
  padding: 8px 14px;
  color: var(--text);
  border: 1px solid var(--accent-tint);
  border-radius: 28px;
}
launcher > list > item.top:checked {
  background: var(--accent);
  color: var(--on-accent);
}
/* The heading over each section of what is found. */
launcher > list > heading {
  height: 24px;
  padding: 0 8px;
  color: var(--text-dim);
  font-weight: 600;
}
/* The row of modes at the bottom, under a hairline, while the pointer is
 * over an empty launcher; and a mode in it. */
launcher > modes {
  height: 56px;
  padding: 11px 12px 12px 12px;
  border-top: var(--edge);
}
launcher > modes > button {
  padding: 0 12px;
  color: var(--text);
  border-radius: 16px;
}
launcher > modes > button:hover { background: var(--accent-tint); }
launcher > modes > button:checked {
  background: var(--accent);
  color: var(--on-accent);
}
/* The mode the keyboard is on, after Down past the last row, has a ring
 * of the accent a little way out, as a button of a question has. */
launcher > modes > button:focus {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
/* The key that goes to a mode, after its name, in a faint chip of its
 * own: dim, and a size smaller, as a menu's keys are. `display: none`
 * leaves it out. */
launcher > modes > button > kbd {
  padding: 2px 5px;
  color: var(--text-dim);
  font-size: 9pt;
  background: var(--glass-rim);
  border: var(--edge);
  border-radius: 6px;
}
launcher > modes > button:checked > kbd { color: var(--on-accent); }
/* The rest of the screen while the launcher is open. A click on it closes
 * the launcher. Give it a see-through colour to dim what is behind. */
launcher::backdrop { background: transparent; }

/* A notification. The margin is the room between one and the next, and
 * between them and the edges of the screen. One a program marks critical
 * is :state(urgent). */
notification {
  min-width: 340px;
  margin: 8px;
  padding: 12px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: var(--edge);
  border-radius: 10px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
notification:hover { border-color: var(--border-strong); }
notification:focus { border-color: var(--accent); }
notification:state(urgent) { border-color: var(--danger); }
notification > title {
  color: var(--text);
  font-weight: 600;
}
notification > body { color: var(--text-dim); }
/* How full something a notification tells of is, such as the volume: the
 * background is the whole, the colour how much of it is full. */
notification > meter {
  margin-top: 8px;
  background: var(--border);
  color: var(--accent);
  border-radius: 4px;
}

/* What the portals ask: whether an application may see the screen, and
 * what asks for a password. The button that gives is :default. The border
 * is also the ring of the loupe a colour on the screen is picked with. */
dialog.question {
  padding: 20px;
  background: var(--surface);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36);
}
dialog.question > detail { color: var(--text-dim); }
dialog.question > button {
  height: 36px;
  padding: 0 20px;
  background: var(--surface-dim);
  color: var(--text);
  border-radius: 6px;
}
dialog.question > button:hover { background: var(--surface-raised); }
dialog.question > button:default {
  background: var(--accent);
  color: var(--on-accent);
}
/* What the keyboard is on has a ring of the accent, a little way out from
 * it, which shows round a button filled with the accent as well as round
 * one that is not. */
dialog.question > button:focus {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
dialog.question > input {
  padding: 8px;
  background: var(--surface-raised);
  color: var(--text);
  border: 1px solid var(--border-strong);
  border-radius: 6px;
}
dialog.question > input:focus { border-color: var(--accent); }
/* What is typed, when all of it is chosen, as Ctrl+A chooses it. */
dialog.question > input::selection {
  background: var(--accent);
  color: var(--on-accent);
}

/* The switcher on Super+Tab: its box, each window's place in rows, and the
 * title. In rows, the box is in the middle of the screen, each place in it
 * with its picture over its title, as wide as its min-width. On a diagonal,
 * the box is under the queue, with the title of the one picked. The one
 * picked is :checked. */
switcher {
  padding: 12px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  border: var(--edge);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
switcher > item {
  min-width: 200px;
  padding: 8px;
  border: 2px solid transparent;
  border-radius: 6px;
}
switcher > item:checked {
  background: var(--surface-raised);
  border-color: var(--accent);
}
switcher > item > title { color: var(--text); }
/* On a diagonal, each window in the switcher: its border round its picture,
 * its padding away. The one picked is :checked. */
switcher > thumbnail {
  padding: 4px;
  border: 2px solid transparent;
  border-radius: 10px;
}
switcher > thumbnail:checked { border-color: var(--accent); }
/* The screen behind the switcher: dimmed, and blurred out of focus. */
switcher::backdrop {
  background: rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(16px);
}
/* The overview: each window in it, its border round its picture, its
 * padding away, the one picked :checked; the title of the one picked,
 * under it; each workspace in the strip along the top, the one shown :checked
 * and one under the pointer :hover; the button after them that makes a
 * new workspace, its plus in its colour; and the screen behind it all,
 * dimmed and blurred out of focus. */
overview > thumbnail {
  padding: 4px;
  border: 2px solid transparent;
  border-radius: 10px;
}
overview > thumbnail:checked { border-color: var(--accent); }
overview > title {
  padding: 4px 10px;
  background: var(--surface);
  color: var(--text);
  border-radius: 6px;
}
overview > workspace {
  border: 2px solid var(--border);
  border-radius: 6px;
}
overview > workspace:hover { border-color: var(--text); }
overview > workspace:checked { border-color: var(--accent); }
overview > button.add {
  border: 2px solid var(--border);
  border-radius: 6px;
  color: var(--text);
}
overview > button.add:hover { border-color: var(--text); }
/* The button on a window, or a workspace, in the overview that closes it:
 * a disc of a menu's glass, ringed and crossed in the colour of closing. */
overview > thumbnail > button.close {
  background: var(--glass-thick);
  color: var(--close);
  border: 2px solid var(--close);
  border-radius: 13px;
}
overview::backdrop {
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
 * is dimmer, as is the line at its foot that says how it is put away. It
 * is glass, as glass is drawn everywhere; Escape, a press beside it or
 * the overview's button that closes, on its top left corner while the
 * pointer is on it, coming and going as a menu does, puts it away,
 * and its button copies what the computer is. */
dialog.about {
  padding: 20px;
  background: var(--glass-thick);
  backdrop-filter: blur(var(--blur));
  color: var(--text);
  border: var(--edge);
  border-radius: 12px;
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.36), inset 0 1px var(--glass-rim);
}
dialog.about > heading {
  color: var(--text);
  font-weight: 600;
}
dialog.about > detail { color: var(--text-dim); }
dialog.about > button {
  padding: 4px 12px;
  background: var(--surface-dim);
  color: var(--text);
  border-radius: 6px;
}
dialog.about > button:hover { background: var(--surface-raised); }
/* For a moment once it has copied, it says so, in the colour of what is
 * done, with a tick. */
dialog.about > button:state(copied) {
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
  --text-faint: #8d95a8;
  --border: #e4ddc4;
  --border-faint: #efead7;
  --border-strong: #7a8099;
  --danger: #c4303a;
  --warning: #8a5a00;
  --success: #2f7a35;
  --capture: #c4303a;
  --glass-thin: rgba(255, 253, 244, 0.7);
  /* More of itself than in the dark scheme: dim text, and the accent as
   * text, are read on cream over an ordinary ground at no less. */
  --glass-thick: rgba(255, 253, 244, 0.85);
  --glass-sealed: rgba(255, 253, 244, 0.95);
  --glass-rim: transparent; /* light on cream is not seen */
  /* Dim text on the dimmer cream is read over an ordinary ground only
   * at this much of it. */
  --glass-thick-dim: rgba(246, 240, 218, 0.93);
  --button-rim: rgba(12, 18, 38, 0.14);
  --edge: 1px solid rgba(12, 18, 38, 0.1); /* navy, on cream */
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

Two of them are what every surface shares: `--edge`, the hairline round
a window and round everything that floats (menus, tooltips, the panel's
popovers, the launcher, notifications, the switcher and "About"), which
is seen through, so that it takes its colour from what is under it, and
`--blur`, how far what is behind glass is blurred wherever there is
glass. Change either once and it is changed everywhere; a rule for one
part changes that part alone:

```css
:root {
  --edge: none;   /* no hairline round anything */
  --blur: 24px;
}
menu { border: 1px solid var(--border); } /* but round menus, a stronger one */
```

A panel that floats clear of the screen's edges, see-through, with rounded
buttons, is a few rules more:

```css
panel {
  margin: 6px 8px;
  border-radius: 10px;
  background: rgba(27, 29, 35, 0.8);
}
panel workspace, panel task {
  margin: 3px 2px;
  border-radius: 6px;
}
panel > workspace:checked, panel > task:checked { border-bottom: none; }
```

**Elements** are these, each at its place in the tree, with the states
it can be in and the element each is written on. Nothing else is drawn;
`list`, `grid`, `calendar`, `modes` and the like are there to be named
before what is in them.

| Element | States |
|---|---|
| `desktop` | — |
| `window` | `:focus-within`, `:state(closing)` |
| `desktop > separator` | — |
| `desktop > button.offer` | `:hover` |
| `window > titlebar` | `window:focus-within` |
| `window > titlebar > tab` | `:checked`, `window:focus-within` |
| `window > titlebar > controls` | `window:focus-within`, `:hover` |
| `window > titlebar > controls > button.close` | `window:focus-within`, `:hover`, `controls:hover` |
| `window > titlebar > controls > button.maximize` | `window:focus-within`, `:hover`, `controls:hover` |
| `window > titlebar > controls > button.minimize` | `window:focus-within`, `:hover`, `controls:hover` |
| `menu` | `:state(closing)` |
| `menu > item` | `:hover`, `:focus` |
| `menu > separator` | — |
| `menu > item > icon` | `item:hover`, `item:focus`, `item:checked` |
| `menu > item > kbd` | `item:hover`, `item:focus` |
| `notice` | `:state(closing)` |
| `notice > heading` | — |
| `capture` | — |
| `frame.capture` | — |
| `tooltip` | `:state(closing)` |
| `panel` | `:state(closing)` |
| `panel > workspace` | `:checked` |
| `panel > task` | `:checked`, `:state(closing)` |
| `panel > clock` | `:state(closing)` |
| `popover` | `:state(closing)` |
| `popover > item` | `:hover` |
| `popover > calendar > day` | `:hover`, `:state(today)`, `:state(in-line)` |
| `popover > calendar > day.outside` | `:hover`, `:state(today)`, `:state(in-line)` |
| `launcher` | `:state(closing)` |
| `launcher > input` | `::selection` |
| `launcher > list > item` | `:checked` |
| `launcher > list > item > description` | `item:checked` |
| `launcher > grid > item` | `:checked` |
| `launcher > list > item.top` | `:checked` |
| `launcher > list > heading` | — |
| `launcher > modes` | — |
| `launcher > modes > button` | `:checked`, `:hover`, `:focus` |
| `launcher > modes > button > kbd` | `button:checked` |
| `launcher::backdrop` | `launcher:state(closing)` |
| `notification` | `:state(urgent)`, `:hover`, `:focus`, `:state(closing)` |
| `notification > title` | — |
| `notification > body` | — |
| `notification > meter` | — |
| `dialog.question` | `:state(closing)` |
| `dialog.question > detail` | — |
| `dialog.question > button` | `:default`, `:hover`, `:focus` |
| `dialog.question > input` | `:focus`, `::selection` |
| `switcher` | `:state(closing)` |
| `switcher > item` | `:checked` |
| `switcher > item > title` | — |
| `switcher > thumbnail` | `:checked` |
| `switcher::backdrop` | — |
| `overview > thumbnail` | `:checked` |
| `overview > title` | — |
| `overview > workspace` | `:checked`, `:hover` |
| `overview > button.add` | `:hover` |
| `overview > thumbnail > button.close` | — |
| `overview::backdrop` | — |
| `picker` | — |
| `sealed` | — |
| `dialog.about` | `:state(closing)` |
| `dialog.about > heading` | — |
| `dialog.about > detail` | — |
| `dialog.about > button` | `:hover`, `:state(copied)` |

**Naming them** is as in CSS:

- `button` is every button, wherever it is; `*` every element but what
  is behind something over the whole screen, which `*::backdrop` is.
- `button.close` is a button of the class `close`: the classes are in
  the table, and a theme adds none.
- `window button` is a button anywhere in a window, and `window >
  titlebar` a titlebar directly in one.
- `launcher::backdrop` is what is behind the launcher over the screen.
- A state follows the element it is of, which may be one further out:
  `window:focus-within > titlebar` is a titlebar whose window has the
  keyboard, `controls:hover button.close` the close button while the
  pointer is on any of the three, and `button.close:hover` while it is on
  that one. In this step a state can be named of the element itself, and
  of those the table names it with; `menu:state(closing) item` is not read
  yet (ADR 110, step 2).
- Rules may be nested, as CSS Nesting has them, eight deep:
  `launcher { modes { button { … } } }` is `launcher modes button`, and
  `button { &.close { … } &:hover { … } > kbd { … } }` is `button.close`,
  `button:hover` and `button > kbd`.
- Several can share a rule: `panel workspace, panel task { ... }`.

Of two rules for one element, the one naming more classes and states
wins, then the one naming more types, then the later, as CSS counts them.
A selector that reaches no element is an error, which says why and names
the nearest there are; one that reaches some elements and not others is
not: `button:checked` is the launcher's mode shown, and no other button.

**States** are the web's, meaning what they mean there: `:hover` (the
pointer is on it), `:focus` (the keyboard is on it), `:focus-within` (a
window whose content has the keyboard), `:checked` (the one picked or
shown, as a radio button is, or what is turned on, as a box ticked),
`:default` (a question's button that gives, the one Enter takes) and
`::selection` (what is chosen of what is typed). Those the web has no word
for are written as CSS writes a state of one's own, `:state(name)`:
`:state(urgent)` (a notification its sender marked critical),
`:state(closing)` (it is going from the screen), `:state(today)`,
`:state(in-line)` (a day in the week or on the weekday of today) and
`:state(copied)`. `:checked` is, as in CSS, pressed, which nothing is
drawn as yet. Two are not quite the web's yet: `::selection` is a state
of its field, so that what reaches the field reaches what is chosen in
it; and `:root` declares variables alone (ADR 110, step 3).

**Coming and going.** A part that comes and goes, as a window, the menu,
the notice or a component's panel, notification or launcher does, comes
over its own `transition`, from how it is in `@starting-style`, and leaves
over the one it has `:state(closing)`, for how it is then; so that it can leave
quicker than it came:

```css
menu { transition: 180ms ease-out; }
menu:state(closing) { transition: 120ms ease-in; scale: 98%; opacity: 0; }
@starting-style {
  menu { scale: 96%; opacity: 0; }
}
```

Of each, only how see-through the part is (`opacity`) and how large it is
drawn (`scale`) are taken so far: its colours stay as they are while it
comes and goes. What it draws is made larger or smaller about its middle,
its shadow and its glass with it. A part with no `@starting-style` is there
at once, as in CSS, and one whose `:state(closing)` look can be seen stays as it
is until its time to leave is up. The built-in theme gives both to the
parts that come and go, as above, and has `launcher::backdrop` fade in and
out without changing size, so yours keeps them unless it says otherwise.
`@starting-style` holds rules for elements in no state, and nothing else;
rules may be nested within it.

**Nothing moving.** With `appearance.reduce_motion: true` in `config.yaml`,
whatever the theme says, a part that comes and goes keeps its size and
only fades, and no change, of a colour or of a part coming and going,
takes longer than 120 ms. Workspaces change without sliding, whether by
a key or by three fingers or four, which change it as they lift if they went far
enough or fast enough; the box that asks for a password, and the lock
screen's field, do not shake at a wrong one, and the cursor in a field
does not blink; nor does it where the theme has the box it is in move
nothing, `launcher` or `dialog.question`. Programs are told, to move
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
| `color` | a color | text; for `menu > separator` and `desktop > separator`, the line |
| `opacity` | `0` to `1`, or a percentage | of everything the part draws |
| `display` | `none`, `block` or `inline` | `none` leaves the part out, and the room it took; `block` and `inline` put it back. Read by `launcher > modes > button > kbd` only |
| `scale` | `0` to `2`, or `0%` to `200%` | how large the part is drawn as it comes and goes, as a share of its size: in `@starting-style`, and while it is `:state(closing)`, and nowhere else |
| `border` | a width, `solid` or `none`, a color | any of them, in any order |
| `border-color`, `border-width` | a color; `0` to `16px` | |
| `outline` | a width, `solid` or `none`, a color | a line round the outside of the box, any of them in any order; its corners are rounded by as much more than the box's as it is out, unless the box's are square. A ring round what the keyboard is on, as `dialog.question > button:focus` has it; the eye in the password field has its ring close round it, there being no room to stand off |
| `outline-color`, `outline-width`, `outline-offset` | a color; `0` to `16px`; `0` to `16px` | how far out the outline is, with nothing drawn between |
| `border-radius` | `0` to `64px` | rounds the corners of the box, no more than half as round as the box is wide or tall, so that one as round as it is tall is rounded whole; a `menu`'s rows are kept to its corners, so the first and last, highlighted, follow the curve, as `overflow: hidden` would have it |
| `padding`, `margin` | one to four sizes, `0` to `64px` | all; top and bottom, then sides; top, sides, bottom; or top, right, bottom, left. A `menu`'s rows begin inside its border and padding: with none, as in the built-in theme, the first and last reach its border; `menu { padding: 4px 0; }` leaves room above and below them |
| `height` | `16px` to `64px` | of `titlebar`, `menu > item`, `panel`, `launcher > input`, `launcher > list > item`, `popover > calendar > day` and `dialog.question > button` |
| `min-width`, `max-width` | `0` to `4096px` | of `workspace`, `task` and `clock`, and of `popover > calendar > day`, whose height or `min-width`, whichever is more, is how large each day of a month is, and each day of the months beside it, each arrow and each day of the week; `max-width: 0` is no limit. The launcher is as wide as its `min-width`, and so is each program in its grid, `launcher > grid > item`, and so is each button of a titlebar, which is not there at all if that is `0` |
| `font-family`, `font-size` | a name, or names apart by commas; `6pt` to `72pt` | the first of the names that is installed, or a generic one such as `sans-serif`, is used, and `sans-serif` if none is: the built-in theme's `Inter, sans-serif` is Inter where it is installed. Programs are told the font of `menu > item`, which is as most text is, as GTK writes one, `Inter, sans-serif 11`: in the settings toolkits follow under X11 and through the settings portal's `font-name`. The variable `--font-monospace`, names as `font-family` takes them, is the monospaced font programs are told for code and terminals, at the same size, `JetBrains Mono, monospace 11`, through the settings portal's `monospace-font-name`; X11 has no setting for it |
| `font-weight`, `font-style` | `normal`, `bold`, `100` to `900`; `normal`, `italic` | |
| `transition` | a time up to `10s`, perhaps a second up to `10s`, and perhaps `linear`, `ease`, `ease-in`, `ease-out`, `ease-in-out` or `cubic-bezier(x1, y1, x2, y2)`; or `none` | how long the part's colours take to change when its state does, how long they wait first, and how; also `transition-duration`, `transition-delay` and `transition-timing-function`. In `cubic-bezier()` each x is from 0 to 1 and each y from -10 to 10, as in CSS; a curve that goes before its start or past its end goes no further than either |
| `box-shadow` | `none`, or across and down, perhaps how soft and how far it spreads, in px, then a color | the shadow the part casts, as `0 8px 24px rgba(0, 0, 0, 0.28)`: across and down from `-64px` to `64px`, soft from `0` to `64px`, spread from `-32px` to `32px`. After a comma, or alone, one `inset`: a sharp line inside the border along the side it falls from, as wide as it falls, following the corners, as `inset 0 1px rgba(255, 255, 255, 0.06)` is light along the top, with neither a soft edge nor a spread; drawn wherever the part is drawn as a box, its titlebar among them. Cast by `window`, but not while it is maximized or fullscreen, nor by a program that draws its own frame; by `menu`, and by the pictures of arrangements, which are drawn as a menu; by `notice`, and by `tooltip` over the mark that says the screen is captured, though not by a component's tooltip, such as the panel's; by `switcher`, under the switcher on Super+Tab and the windows shown in it; by the parts the components show, such as `launcher`; and by `dialog.question` under the box that asks for a password, which it follows as the box shakes, and under what an application asks through the portals. Each notification casts its own; the shadow of one panel or notification does not fall on another beside it. It changes with the part's state, as its colours do |
| `backdrop-filter` | `none`, or `blur()` of `0px` to `64px` | makes the part glass: what is behind it, blurred by as much as a Gaussian's deviation, as CSS has it, but never less than `16px`, below which what is behind could still be read, is drawn under the part where its `background` lets it be seen. A part whose background cannot be seen through has none. Under `menu`, and the pictures of arrangements, which are drawn as a menu; `notice`; `tooltip`, under what the mark says; `switcher`; `dialog.question`, under the box that asks for a password, and what an application asks through the portals; `sealed`, under the dim of every screen while a password is asked for, which is blurred and is the dim alone where blur cannot be drawn; `switcher::backdrop`, likewise under the dim of the screen the switcher is on; `overview::backdrop`, under the dim of the screen the overview is on; and the parts the components show, such as `panel` and `notification`, under each surface. Under `titlebar`, with its top corners rounded as the window's are; a `tab` in a frame of several is drawn over it, and covers it unless its own `background` can be seen through too. On `window`, how far what is behind a program is blurred where it asks to be see-through (`ext-background-effect-v1`, or KDE's `org_kde_kwin_blur`, which Ghostty's `background-blur` asks through); `none` gives programs no blur, and those that ask through the first are told so. Such glass has no tint, the program drawing its own, and is nothing at all where blur cannot be drawn. A part keeps its glass as it fades out. Glass is drawn solid in the part's `background`, or halfway along its gradient where it has no colour, made opaque, where blur cannot be drawn: on a graphics card whose blur, tried as omorfos starts, is not as it should be, which you are told once; and with `appearance.reduce_transparency` |

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
`menu > item:hover { transition: none; }` lights a row at once and lets it
fade when the pointer leaves. Everything takes 120ms unless you say
otherwise; `*, *::backdrop { transition: none; }` turns it all off. A
second time is how long the colours wait before they change, as in CSS:
`menu > item:hover { transition: 80ms ease-out 300ms; }` lights a row only
once the pointer
has rested on it. Only colours
change this way. Sizes change at once, so that nothing moves under the
pointer, and a `transition` names no property: all the colours of a part
go together. Something drawn for the first time has its colours from the
start.

**What comes onto the screen fades in, and what goes fades out**, over
the same time: a window with its frame over `window`'s `transition`, the
menu over `menu`'s, the notice over `notice`'s, and the panel, the launcher
with what is behind it, and the notifications over `panel`'s, `launcher`'s,
`launcher::backdrop`'s and `notification`'s. A window goes and comes each
time it is hidden and shown: when its workspace is left and returned to, or
when it is minimized and brought back, so that one workspace fades into
the next. Showing another tab of a frame is neither. `window { transition:
none; }` has windows there and gone at once and leaves everything else as
it was. A panel of a program that is not part of omorfos comes and goes at
once, and so does one notification among others that stay, and the
question an application has asked, whatever `dialog.question`'s `transition`, so
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
  cross, a bar and a plus. They stand at the left, the cross outermost,
  and programs that draw their own titlebars are told the same. Each is
  `:hover` while the pointer is on it, and `window > titlebar > controls`, which holds
  them, while it is on any of them.
- `padding` is honoured by `tab`, `menu > item`, `menu > item > icon`, `notice`,
  `panel`, `workspace`, `task`, `clock`, `dialog.question` (the room round what
  a box that asks holds) and `dialog.question > button` (the room either side of
  its words, which with them makes it as wide as it is).
- `border-radius` is honoured by `window`, `tab`, `menu`, `menu > item`,
  `menu > item > icon`, `notice`, `capture`, `panel`, `workspace`, `task` and
  `clock`.
- A window's border is the same on every side: its `border-top` stands for
  all four. The sizes of a window's frame and titlebar are taken from the
  rules without a state, since a window does not change size when it gains
  focus.
- Layout (`position`, `flex`, and `display` but for `none`),
  `radial-gradient()`, more than
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
