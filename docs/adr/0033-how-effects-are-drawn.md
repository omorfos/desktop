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

### Corners

The corners of glass are cut as a window's are (ADR 16): the blurred
picture is drawn in strips that follow the curve, and the tint, which is
smooth, is over the join. One way for both renderers.

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
  notice.
- It draws them for the components' too. It already knows which part of
  the theme a component's surface is, by the name it gives its layer, and
  so what blur and shadow the theme gives it. The component draws its
  tint and nothing else; it cannot see what is behind it and is not given
  it.
- Other programs ask with `ext-background-effect-v1`, and are given blur
  of the same kind where they say. They are never given shadows wider
  than their own surface, or the picture.

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
4. Blur behind titlebars, which every window has, only once its cost has
   been measured on a modest laptop with a dozen windows.

Step 3 begins with a trial: that a picture drawn into through the manager
of several GPUs can be blurred by the renderer underneath and drawn back
in the same frame. Nothing read so far says it cannot; nothing has been
run.

### What it costs is shown

How long the last frame took to draw, and how many blurs it made, are in
the state published over IPC. A budget is not enforced by the compositor;
it is watched by people, and the fourth step waits on it.

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
