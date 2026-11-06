# 33. How effects are drawn

- Status: accepted
- Date: 2026-10-13

## Context

The design language (ADR 32) wants frosted glass and shadows, and wants
them to look as good as they can. Blur of that quality is a shader, a
program for the GPU. Three things here have to be squared with that.

- **The tests draw in software.** `Compositor::render` is written once,
  for any renderer, and the tests run it with the software renderer and
  look at pixels. A shader runs on a GPU or not at all, so it cannot run
  in CI, and all of the code here is to be covered. Rounded corners met
  this first and did without a shader (ADR 16).
- **No `unsafe`.** Smithay compiles a shader and draws with it safely.
  Anything it has no call for means raw OpenGL, which is `unsafe`.
- **No work when nothing changes.** A blur done again on every frame is
  work when nothing changes, and a laptop's battery is the price.

A blur made only of what every renderer can draw, halving and doubling
pictures, was considered and would have been wholly testable. It is
plainer than a shader's and can show grain as things move beneath it. The
look was chosen over it.

## Decision

**Blur is a shader, in one file that is left out of coverage, and
everything that is a decision is outside that file and tested.**

### The line between what is tested and what is not

A trait says what the compositor needs of a renderer: *blur this picture
by this much*. It has two implementations.

- **The software one**, in `omorfos-ui`, is ordinary code with tests on
  its pixels. It is what the tests run, and it is the specification: the
  shader is right when it gives what this gives.
- **The GPU one** is the shader and the calls that run it. It is in a
  file of its own, listed in ADR 2 when it is added, and it holds no
  decision: not what is blurred, nor when, nor by how much, nor what to do
  if it cannot be.

Everything else is one code path for both: finding the sheets of glass,
drawing what is under each into a picture, knowing when that picture is
stale, cutting the corners, laying the tint over. The tests can say that
a red window under a menu makes the menu redder there and nowhere else,
and that a frame in which nothing moved drew nothing.

### The shader is checked where it runs

No test in CI sees the shader's output, and drivers differ. So the
compositor checks it itself, each time it starts on a GPU: a small known
picture is blurred by the shader, read back, and compared with what the
software implementation makes of the same picture. If they differ by more
than rounding allows, or the shader does not compile, blur is off for the
session, surfaces are solid, and the user is told once. The comparison is
ordinary, tested code. Reading a picture back from a GPU is slow, and is
done this once.

### The blur

Dual-filter blur, the kind known as dual Kawase: the picture is drawn
smaller in a few passes, each taking five samples, then larger again,
each taking eight. It is close to a Gaussian to the eye, and far cheaper
than one for the widths wanted, because most of the work is done on small
pictures. How wide the blur is decides the number of passes; that
arithmetic is on the tested side of the line.

The last pass also gives what is behind a little more saturation, as the
design language asks.

It is written with Smithay's custom texture shaders and its pictures to
draw into, which are safe. The hardware backend reaches the GPU through
Smithay's manager of several GPUs; it lends the renderer underneath, and
the shader is run on that.

### Shadows are pictures, not shaders

A shadow is a picture made by `omorfos-ui`, on the CPU, of a blurred
rounded rectangle, and drawn under its surface as any picture is. It is
made once for each radius, spread, offset, colour and scale in use, and
kept. It is cut into nine pieces, corners, edges and middle, so that one
picture serves a surface of any size and a window being resized makes no
new one. A shader would look no better, and this is tested.

An edge or the middle is one pixel across, stretched. Each piece carries a
border of one pixel, a copy of its edge, that is not drawn but is what its
edge is blended with: the software renderer takes what is beyond a picture
for nothing, where GLES takes the pixels at its edge, and a side stretched
without the border faded toward its ends, to about a third. Drawn so, both
renderers draw a side whole to its ends, and the tests can say so.

### How the software blur is made

`backdrop-filter: blur(24px)` asks, as in CSS, for what a Gaussian of
that deviation would make. The dual filter's width doubles with each
halving, and widens with how far apart its samples are; both were
measured, and a blur takes the fewest halvings that reach it with samples
no more than two pixels apart, and the spacing that reaches it exactly.
A test holds it to within a little over a tenth of what was asked, from
2 to 48 pixels. Each pass is rounded to bytes, as a pass drawn into a
texture is, so that the shader can match it.

What is behind a sheet is taken in as far round it as three deviations,
beyond which what there is counts for too little to be seen; beyond that,
and beyond the screen, is what is inside, mirrored. Taking the pixel at
the edge for all that is beyond it, as the blur first did, makes
whatever is along the edge endless: a bright line in a wallpaper along
the bottom of the screen, under the panel, stayed bright under any blur.
CSS's `backdrop-filter` was changed from the one to the other for the
same reason, and for colours flickering as a sheet moves along an edge
(the CSS working group, November 2023; Chrome 129). A line 10 pixels
high along the bottom, under a panel 70 high, at the bottom of the
panel and as bright as 255: 203 taking the edge and 182 mirrored at a
deviation of 9; 123 and 83 at 21; 86 and 65 at 28.

Mirrored, a narrow blur still shows such a line, which is under the
glass and is shown. So glass is never blurred less than 16 units of
layout, whatever the theme asks: two thirds of the thinnest glass in
`DESIGN.md`, which already says that nothing is see-through and not
blurred. Less leaves letters behind to be read, and a line along an
edge a line; 16 spreads it over about the height of a panel, a glow.
That is the one place glass is not as CSS has it: a theme asking for
`blur(5px)` gets 16. It costs little, the wider blur being made on
smaller pictures, though what is taken in round a sheet is wider. A
part's own shadow is not behind it, as in CSS: its
glass is under its pixels and over its shadow, and what is behind begins
under the shadow. A part whose background cannot be seen through has no
glass, and nothing is blurred for it.

What blurs it is the backend's to hand to each frame it draws, as a
`Frosting`: the trait of the line above, which blurs the picture of what
is behind a sheet and gives back what draws it. A backend that hands none
gets glass solid. The software one reads the picture back from the
renderer, which the tests draw and a graphics card would be made to wait
for. A sheet that cannot have a picture to draw into, or whose picture
was not blurred, is solid too.

### How the shader is made

The shader is in `shader.rs`, two programs Smithay compiles: one for a
pass down and one for a pass up, the last pass up with the saturation.
The sizes it draws at are the ones `Blur` gives the software blur, and
each is a texture drawn into whole, written as it is made rather than
laid over what was there. They are kept with the sheet, so a blur made
again draws into what it drew into before.

It cannot reach the GPU through the renderer the compositor is written
for, which may be any; and the hardware backend's, Smithay's manager of
several GPUs, can neither be asked whether it is OpenGL nor have a
texture of its own made from one the shader drew. So the backend, which
knows which renderer it has, hands the shader in; the shader reaches
the OpenGL renderer underneath, and draws what it made through the frame
underneath. Both backends that draw with a GPU hand it in, the nested
one too.

Textures Smithay makes to be drawn into are sampled round to their other
side beyond their edges (`docs/UPSTREAM.md`). The shader folds each
sample back over the edge, as the software blur mirrors, and keeps it
within the middles of the pixels along the edge, between which the
sampler would blend in the other side; the two together sample as a
texture told to mirror would.

On an Intel UHD 620 the shader blurs the picture it is tried on, and
others as wide as 200 by 120 blurred by 48, within 2 in 255 of the
software. The check allows 4.

### Corners

The corners of glass are cut as a window's are (ADR 16): the blurred
picture is drawn in strips that follow the curve, and the tint, which is
smooth, is over the join. One way for both renderers. A titlebar's glass
is rounded above only, as the titlebar is, its window being under it
below.

### Nothing is blurred twice

Each sheet of glass keeps its blurred picture, and a record of what was
under it when the picture was made, kept by the same damage tracker that
decides what of the screen to draw again. Each frame it is asked whether
anything under the glass, as far out as the blur reaches, has changed. If
not, the picture is used again and the glass is not drawn again either.

So a still screen costs nothing, a clock ticking in the bar costs the
bar's text, and a film playing under a menu costs a blur the size of the
menu each frame, which is the most it can cost.

### Glass that moves

A window dragged about has something new under its titlebar every frame,
so its blur is made again every frame. Three things keep that bounded.

- **Once a frame at most.** Frames are paced by the screen (ADR 10), not
  by the pointer. However fast the window is moved, the blur is made as
  often as the screen shows a frame and no oftener.
- **Nothing waits in a queue.** A frame is drawn from where things are
  when it is drawn. A frame that took too long is followed by one of
  where the window is by then; the places between are never drawn. The
  cost of a slow blur is a frame missed, never work that mounts up the
  longer the dragging goes on.
- **The pictures are kept and used again.** A sheet of glass has its
  pictures for as long as it is the size it is. Dragging allocates
  nothing.

What the picture is of makes no difference to the work: the blur takes
the same samples of a plain colour as of a detailed photograph.

### The wallpaper is blurred once

Most glass, most of the time, has only the wallpaper under it, and the
wallpaper does not change. Each output keeps its wallpaper blurred, made
when the wallpaper or the theme's blur changes. A sheet of glass with
nothing under it but wallpaper takes its picture from that, and makes no
blur at all. So a window dragged over the bare desktop costs what it
costs without glass. Only glass over other windows is blurred as it
goes.

### Who gets them

- The theme says: `backdrop-filter: blur(24px)` and `box-shadow`, on any
  part.
- The compositor draws them for its own parts: titlebars, menus, the
  notice, the switcher.
- It draws them for the components' too. It already knows which part of
  the theme a component's surface is, by the name it gives its layer, and
  so what blur and shadow the theme gives it. The component draws its
  tint and nothing else; it cannot see what is behind it and is not given
  it. So the shadow falls round the whole surface, and a component that
  shows several things apart, as the notifications do, puts each on a
  surface of its own.
- Surfaces on one layer stand at one height: their shadows are all drawn
  under all of them, so that the shadow of one notification falls under
  the next and not on it.
- The prompt that asks for a secret (ADR 48) is known by its program's
  role, not its layer's name, and the compositor draws it itself over the
  dim. Its surface is wider than its box, for the room the box shakes in,
  so a shadow round the surface would stand still beside a box that
  moves. It says where its box is by the region it takes the pointer in,
  moved with the box at each frame of the shake, and the compositor casts
  the `question` part's shadow round the bounds of that region, kept
  within the surface; round the whole surface where it says none.
- What an application asks through the portals is a panel named after
  the `question` part, and casts its shadow as the components' panels do.
  It comes and goes at once, not over the part's time as they do: what it
  leads to may be a picture of the screen, and it is not to be in that,
  nor is its shadow. The pictures it shows to pick a colour from are
  named after no part, and cast none.
- Other programs ask with `ext-background-effect-v1`, or, until those
  that do move to it, KDE's `org_kde_kwin_blur` (ADR 64), and are given
  blur of the same kind where they say. They are never given shadows wider
  than their own surface, or the picture. How far it blurs is the
  `window` part's `backdrop-filter`, the same for every program, and a
  program is told it may have none where that is `none` or transparency
  is to be reduced, and told again whenever that changes. Its glass has
  no tint, the program drawing its own over it, and where blur cannot be
  drawn it is nothing at all, the program left as it drew itself, as it
  would be on a compositor without the protocol: there is no colour to be
  solid in. What it asks for is cut to its surface, a window's to the
  window proper, not the shadow a program drawing its own frame draws
  round it on the same surface, and to its frame's rounded corners, and
  blurred as one picture however many rectangles it
  comes to, as a region with rounded corners is told as many. A panel
  whose part the theme makes glass has that, and not what its program
  asks for too, which would blur the same twice.

### When they are not drawn

- With `appearance.reduce_transparency`, or `backdrop-filter: none`.
- Where the check at startup failed, or a picture to draw into cannot be
  had.
- Then the surface is solid in its colour, as the design language says.
  Glass that is not blurred is never drawn.

### In what order

1. Shadows, which have no unknowns.
2. The software blur and everything around it, tested, with the hardware
   backend drawing solid.
3. The shader and its check, tried on real hardware, behind the bar, the
   dock, menus and notifications.
4. Blur behind titlebars, which every window has, where a theme asks for
   it; in the built-in theme only once its cost has been measured on a
   modest laptop with a dozen windows.

Step 3 began with a trial: that a picture drawn into through the manager
of several GPUs can be blurred by the renderer underneath and drawn back
in the same frame. It can: a test that needs a card does it, as the check
at startup does.

### What it costs is shown

How long the last frame took to draw, and how many blurs it made, are
told over IPC, as a request of their own (`"frames"`, and `omorfosctl
frames`): not in the state, which is sent to every subscriber whenever it
changes, and this changes with every frame. A budget is not enforced by
the compositor; it is watched by people, and glass behind titlebars in
the built-in theme waits on it.

## Alternatives

- **No shader: a pyramid of halvings and doublings.** As above: wholly
  tested, plainer to look at. Not chosen.
- **Blur only the wallpaper**, once, and show that behind every glass.
  Costs nothing ever. But a menu over a window would show the wallpaper
  through it, which is a lie about what is there.
- **Read the screen back and blur on the CPU.** All testable, but reading
  a picture back from a GPU stops it to wait, each time.
- **Shaders for shadows and corners as well.** More that no test sees,
  for no better picture.

## Consequences

- A third file is left out of coverage, for the first time one that
  draws what the user looks at. It is kept to the shader and the calls
  that run it, it is checked at every start against tested code, and
  ADR 2 lists it in the commit that adds it.
- The software implementation has to be the same blur as the shader, tap
  for tap, or the check means nothing. It is slow, which does not matter:
  it runs in tests, in the check, and nowhere a user waits.
- A driver on which the shader is wrong gets solid surfaces and a
  sentence, not a wrong picture.
- The theme gains `backdrop-filter` and `box-shadow`, and colours of
  surfaces with opacity.
- Every element under a sheet of glass is looked at once more each frame,
  to ask whether it changed. That is arithmetic on rectangles, not
  drawing.
- Memory: each sheet of glass keeps its blurred picture and the smaller
  ones on the way to it, and each kind of shadow a small picture.
