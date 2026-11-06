//! Glass (`backdrop-filter`, ADR 33): what is behind a part, blurred and
//! drawn under it, where its background lets it be seen.
//!
//! The sheets of glass are found as the scene is laid out, each as where
//! in what is drawn its glass goes and where what is behind it begins.
//! Then, deepest first, so that glass over glass has the lower in its
//! picture, what is behind each is drawn into a picture of its own and
//! blurred. Each sheet keeps its picture and a damage tracker of its own,
//! which says whether anything behind it has changed since; if nothing
//! has, the picture is used again, and the screen's tracker sees nothing
//! to draw again. A sheet with nothing behind it but the desktop takes its
//! part of one picture of the whole desktop, blurred once.
//!
//! The blur is made by a [`Frosting`]: in software, read back from the
//! renderer, which is what the tests draw and what any other is held to;
//! by a shader, on a graphics card that blurred a picture at startup as
//! the software does; or not at all, when glass is drawn solid in its
//! part's colour.

use std::any::Any;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use omorfos_config::{Color, Style};
use omorfos_ui::Blur;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::utils::{Relocate, RelocateRenderElement};
use smithay::backend::renderer::element::{Element as _, Id, Kind, RenderElement};
use smithay::backend::renderer::utils::{CommitCounter, DamageSet, OpaqueRegions};
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, ImportAll, ImportMem, Offscreen, Renderer, Texture,
};
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale, Size, Transform};

use crate::capture::Held;
use crate::compositor::{Element, View, color};
use crate::corners::{self, Names, Strip};

/// What blurs what is behind glass, for a renderer `R` that draws what is
/// behind into a `P` (ADR 33). There are two: [`Software`], which is the
/// specification, and a shader, which is held to it at startup.
pub trait Frosting<R: Renderer, P> {
    /// Blurs `picture`, `size` pixels large, by `blur`, into `made`: what it
    /// made of the same picture the time before, if anything, which it may
    /// draw into again. Whether it did.
    fn blur(
        &mut self,
        renderer: &mut R,
        picture: (&mut P, Size<i32, Physical>),
        blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool;

    /// What draws `made`, what it blurred, at `area` of a screen that has
    /// `scale` pixels to a unit, `opacity` of it to be seen; none if it is
    /// not something it made.
    fn element(
        &self,
        renderer: &mut R,
        made: &dyn Any,
        at: (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<R>>>;
}

/// The blur made on the CPU, from a picture of what is behind glass read
/// back from the renderer: the specification, which the tests draw.
/// Reading a picture back from a graphics card makes it wait, so it is not
/// for one.
#[derive(Debug, Clone, Copy, Default)]
pub struct Software;

impl<R, P> Frosting<R, P> for Software
where
    R: Renderer + ImportMem + ExportMem + Bind<P> + 'static,
    R::TextureId: Clone + Texture + Send + 'static,
{
    fn blur(
        &mut self,
        renderer: &mut R,
        (picture, size): (&mut P, Size<i32, Physical>),
        blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool {
        let whole = Rectangle::<i32, Buffer>::from_size((size.w, size.h).into());
        let read = renderer.bind(picture).ok().and_then(|framebuffer| {
            let read = renderer.copy_framebuffer(&framebuffer, whole, Fourcc::Abgr8888);
            read.ok()
        });
        let pixels = read
            .as_ref()
            .and_then(|read| renderer.map_texture(read).ok());
        let blurred = pixels.map(|pixels| {
            let (width, height) = (size.w.unsigned_abs(), size.h.unsigned_abs());
            blur.apply(pixels, (width, height))
        });
        blurred.is_some_and(|blurred| {
            let kept = made
                .as_mut()
                .and_then(|made| made.downcast_mut::<MemoryRenderBuffer>());
            if let Some(buffer) = kept {
                let mut context = buffer.render();
                let drawn = context.draw(|into| {
                    for (to, from) in into.iter_mut().zip(&blurred) {
                        *to = *from;
                    }
                    Ok::<_, std::convert::Infallible>(vec![whole])
                });
                drawn.is_ok()
            } else {
                let buffer = MemoryRenderBuffer::from_slice(
                    &blurred,
                    Fourcc::Abgr8888,
                    (size.w, size.h),
                    1,
                    Transform::Normal,
                    None,
                );
                *made = Some(Box::new(buffer));
                true
            }
        })
    }

    fn element(
        &self,
        renderer: &mut R,
        made: &dyn Any,
        (area, scale, opacity): (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<R>>> {
        let buffer = made.downcast_ref::<MemoryRenderBuffer>();
        let element = buffer.and_then(|buffer| {
            let size = area.size.to_f64().to_logical(scale).to_i32_round();
            let at = area.loc.to_f64();
            let kind = Kind::Unspecified;
            let made = MemoryRenderBufferRenderElement::from_buffer(
                renderer,
                at,
                buffer,
                Some(opacity),
                None,
                Some(size),
                kind,
            );
            made.ok()
        });
        element.map(|element| Rc::new(element) as Rc<dyn RenderElement<R>>)
    }
}

/// What a [`Frosting`] draws, among what the compositor draws.
pub(crate) struct Frosted<R: Renderer>(Rc<dyn RenderElement<R>>);

impl<R: Renderer> smithay::backend::renderer::element::Element for Frosted<R> {
    fn id(&self) -> &Id {
        self.0.id()
    }

    fn current_commit(&self) -> CommitCounter {
        self.0.current_commit()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.0.src()
    }

    fn transform(&self) -> Transform {
        self.0.transform()
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.0.geometry(scale)
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        self.0.damage_since(scale, commit)
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> OpaqueRegions<i32, Physical> {
        self.0.opaque_regions(scale)
    }

    fn alpha(&self) -> f32 {
        self.0.alpha()
    }
}

impl<R: Renderer> RenderElement<R> for Frosted<R> {
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        self.0.draw(frame, src, dst, damage, opaque_regions)
    }
}

/// The most a blur made otherwise may differ from the software's, in any
/// channel of any pixel, and be taken for it: what rounding each pass, and
/// how finely a graphics card weighs the pixels it samples between, make
/// of it.
const ROUNDING: u8 = 4;

/// How large the picture `frosting` is tried on is, and how far it is
/// blurred: twice halved, so that a pass down and one up each draw from
/// what another drew.
const TRIED: (i32, i32) = (48, 32);
const TRIED_BLUR: u32 = 6;

/// Whether `frosting` blurs as [`Software`] does, within rounding: a small
/// picture with sharp edges, of colours and of what can be seen through, is
/// blurred by it, drawn, and read back. If not, why not, for the user.
pub(crate) fn check<R, P>(renderer: &mut R, frosting: &mut dyn Frosting<R, P>) -> Result<(), String>
where
    R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
    R::TextureId: Clone + Texture + Send + 'static,
{
    let (width, height) = (TRIED.0.unsigned_abs(), TRIED.1.unsigned_abs());
    let size = Size::<i32, Physical>::from(TRIED);
    let bytes = tried((width, height));
    let buffer =
        MemoryRenderBuffer::from_slice(&bytes, Fourcc::Abgr8888, TRIED, 1, Transform::Normal, None);
    let shown = MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        Point::default(),
        &buffer,
        None,
        None,
        None,
        Kind::Unspecified,
    );
    let picture = renderer.create_buffer(Fourcc::Abgr8888, (size.w, size.h).into());
    let result = renderer.create_buffer(Fourcc::Abgr8888, (size.w, size.h).into());
    let ready = Blur::new(TRIED_BLUR).zip(shown.ok());
    let ready = ready.zip(picture.ok()).zip(result.ok());
    let tried = ready.and_then(|(((blur, shown), mut picture), mut result)| {
        let mut made = None;
        let blurred = drawn(renderer, shown, (&mut picture, size))
            && frosting.blur(renderer, (&mut picture, size), &blur, &mut made);
        let element = made.as_deref().filter(|_| blurred).and_then(|made| {
            let whole = (Rectangle::from_size(size), 1.0, 1.0);
            frosting.element(renderer, made, whole)
        });
        let shown = element.filter(|element| {
            let element = Frosted(Rc::clone(element));
            drawn(renderer, element, (&mut result, size))
        });
        let read = shown.and_then(|_| read(renderer, (&mut result, size)));
        read.map(|read| (blur.apply(&bytes, (width, height)), read))
    });
    let Some((expected, made)) = tried else {
        return Err("could not blur a picture to be tried".to_owned());
    };
    let most = expected
        .iter()
        .zip(&made)
        .map(|(expected, made)| expected.abs_diff(*made))
        .max()
        .unwrap_or_default();
    if made.len() == expected.len() && most <= ROUNDING {
        Ok(())
    } else {
        Err(format!(
            "blurs otherwise than it should, by as much as {most} in 255"
        ))
    }
}

/// Whether `element` was drawn into `into`, `size` large, by itself, on
/// nothing.
fn drawn<R, P, E>(renderer: &mut R, element: E, (into, size): (&mut P, Size<i32, Physical>)) -> bool
where
    R: Renderer + Bind<P>,
    E: RenderElement<R>,
{
    let mut tracker = OutputDamageTracker::new(size, 1.0, Transform::Normal);
    renderer.bind(into).is_ok_and(|mut framebuffer| {
        let none = Color32F::TRANSPARENT;
        let drawn = tracker.render_output(renderer, &mut framebuffer, 0, &[element], none);
        drawn.is_ok()
    })
}

/// What `from`, `size` large, holds, read back.
fn read<R, P>(renderer: &mut R, (from, size): (&mut P, Size<i32, Physical>)) -> Option<Vec<u8>>
where
    R: Renderer + ExportMem + Bind<P>,
{
    let framebuffer = renderer.bind(from).ok();
    let read = framebuffer.and_then(|framebuffer| {
        let area = Rectangle::from_size((size.w, size.h).into());
        renderer
            .copy_framebuffer(&framebuffer, area, Fourcc::Abgr8888)
            .ok()
    });
    read.and_then(|read| renderer.map_texture(&read).ok().map(<[u8]>::to_vec))
}

/// The picture a frosting is tried on, `width` by `height`, its pixels red,
/// green, blue and alpha, premultiplied: a third red; a third white above
/// and nothing below; and a third blue, half seen through.
fn tried((width, height): (u32, u32)) -> Vec<u8> {
    let mut bytes = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let pixel = match (x * 3 / width, y * 2 / height) {
                (0, _) => [255, 0, 0, 255],
                (1, 0) => [255, 255, 255, 255],
                (1, _) => [0, 0, 0, 0],
                _ => [0, 0, 128, 128],
            };
            bytes.extend(pixel);
        }
    }
    bytes
}

/// A sheet of glass in a scene being laid out.
#[derive(Debug, Clone)]
pub(crate) struct Pane {
    /// Which sheet it is, from one frame to the next.
    key: u64,
    /// Where its glass goes in what is drawn, counted from the top down:
    /// under its part's own pixels.
    pub(crate) at: usize,
    /// Where what is behind it begins: under its part's shadow too, which
    /// is part of it and not behind it.
    pub(crate) under: usize,
    /// Where it is, in pixels of the output.
    area: Rectangle<i32, Physical>,
    /// How far its corners are rounded, in pixels.
    radius: i32,
    /// How far what is behind it is blurred, in pixels.
    blur: u32,
    /// The colour it is solid in where it is not blurred.
    color: Color,
    /// How much of it there is to see as its part comes or goes, from 0 to
    /// 1.
    opacity: f32,
}

impl Pane {
    /// The glass under a part drawn as `style`, at `area` in the space, if
    /// the theme makes it glass: blurred, and with a background that can be
    /// seen through. `key` tells it from others; `at` and `under` are as a
    /// pane's are.
    pub(crate) fn of(
        style: &Style,
        (area, view): (Rectangle<i32, Logical>, View),
        (key, at, under): (u64, usize, usize),
        opacity: f32,
    ) -> Option<Self> {
        let glass = style.blur > 0 && style.background.alpha < u8::MAX;
        let from = view.pixel(area.loc.to_f64());
        let to = view.pixel((area.loc + area.size.to_point()).to_f64());
        let pixels = |units: u32| view.scale.pixels(units.cast_signed()).unsigned_abs();
        glass.then(|| Self {
            key,
            at,
            under,
            area: Rectangle::new(from, (to - from).to_size()),
            radius: view.scale.pixels(style.radius.cast_signed()),
            blur: pixels(style.blur),
            color: style.background,
            opacity,
        })
    }

    /// The same, with what is behind it beginning at `under`.
    pub(crate) const fn under(self, under: usize) -> Self {
        Self { under, ..self }
    }
}

/// What tells a sheet of glass from another, from one frame to the next,
/// made from what it is the glass of.
pub(crate) fn key(of: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    of.hash(&mut hasher);
    hasher.finish()
}

/// A scene laid out: what is drawn, from the top down; the sheets of glass
/// in it; and where the desktop begins in it, which is what is behind
/// everything else.
pub(crate) struct Scene<R>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Texture + Send + 'static,
{
    pub(crate) elements: Vec<Element<R>>,
    pub(crate) panes: Vec<Pane>,
    pub(crate) desktop: usize,
}

/// The screen a scene is drawn on: its name, how large it is in pixels,
/// and how many pixels it has to a unit of layout.
pub(crate) type Screen<'a> = (&'a str, Size<i32, Physical>, f64);

/// Glass, and what it keeps between frames.
#[derive(Debug, Default)]
pub(crate) struct Glass {
    /// What each sheet keeps, by the screen it is on and its key.
    kept: HashMap<(String, u64), Kept>,
    /// The desktop of each screen, blurred, by the screen's name.
    desktops: HashMap<String, Option<Picture>>,
    /// How many blurs have been made.
    blurs: u64,
}

/// What a sheet keeps from one frame to the next.
#[derive(Debug, Default)]
struct Kept {
    /// What is behind it, blurred, if it is not taken from the desktop's.
    behind: Option<Picture>,
    /// It, solid.
    solid: SolidColorBuffer,
    /// The names its strips go by.
    names: Names,
}

/// A picture of what is behind glass, blurred: what it was drawn into, the
/// tracker that says when what it is of changes, the blur, and what the
/// frosting made of it, blurred, once it has.
#[derive(Debug)]
struct Picture {
    /// What of the screen it is of, in pixels.
    area: Rectangle<i32, Physical>,
    blur: Blur,
    target: Box<dyn Held>,
    tracker: OutputDamageTracker,
    made: Option<Box<dyn Any>>,
    /// How many blurs it has made that have not been counted yet.
    blurs: u64,
}

impl Glass {
    /// How many blurs have been made since the compositor started.
    pub(crate) const fn blurs(&self) -> u64 {
        self.blurs
    }

    /// What `scene` draws on `screen`, with the glass of each of its sheets
    /// put under its part: blurred by `frosting`, or solid if there is none,
    /// if `solid`, or if it cannot be blurred. `behind` is what is drawn
    /// where nothing else is.
    pub(crate) fn frost<R, P>(
        &mut self,
        renderer: &mut R,
        screen: Screen<'_>,
        scene: Scene<R>,
        (behind, solid): (Color32F, bool),
        frosting: Option<&mut (dyn Frosting<R, P> + '_)>,
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let Scene {
            mut elements,
            mut panes,
            mut desktop,
        } = scene;
        let (name, ..) = screen;
        self.kept
            .retain(|(on, key), _| on != name || panes.iter().any(|pane| pane.key == *key));
        // The deepest first, so that what is put in leaves what is above it
        // where it was.
        panes.sort_by_key(|pane| std::cmp::Reverse(pane.at));
        let mut frosting = frosting.filter(|_| !solid);
        for index in 0..panes.len() {
            let pane = panes[index].clone();
            let glass = self.glass::<R, P>(
                renderer,
                screen,
                (&pane, &elements, desktop),
                (behind, frosting.as_deref_mut()),
            );
            let added = glass.len();
            let at = pane.at.min(elements.len());
            elements.splice(at..at, glass);
            for above in &mut panes[index + 1..] {
                above.under += added * usize::from(above.under >= at);
            }
            desktop += added * usize::from(desktop >= at);
        }
        elements
    }

    /// The glass of `pane`, in `elements` with the desktop from `desktop`
    /// on, cut to its corners: what is behind it blurred by `frosting` if
    /// there is one and it can, else solid.
    fn glass<R, P>(
        &mut self,
        renderer: &mut R,
        (name, size, scale): Screen<'_>,
        (pane, elements, desktop): (&Pane, &[Element<R>], usize),
        (behind, frosting): (Color32F, Option<&mut (dyn Frosting<R, P> + '_)>),
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let key = (name.to_owned(), pane.key);
        let mut kept = self.kept.remove(&key).unwrap_or_default();
        let blur = frosting.zip(Blur::new(pane.blur));
        let blurred = blur.and_then(|(frosting, blur)| {
            let reach = i32::try_from(blur.reach()).unwrap_or(i32::MAX);
            // Within the screen: beyond it, the edge is taken, as it is of the
            // desktop blurred.
            let around = Rectangle::new(
                pane.area.loc - Point::from((reach, reach)),
                pane.area.size + Size::from((reach * 2, reach * 2)),
            );
            let around = around
                .intersection(Rectangle::from_size(size))
                .unwrap_or_default();
            // Anything but the desktop behind it, as far round as the blur
            // reaches, and it is blurred by itself; else it takes its part
            // of the desktop blurred.
            let under = elements.get(pane.under..).unwrap_or_default();
            let between = desktop.saturating_sub(pane.under).min(under.len());
            let covered = under[..between]
                .iter()
                .any(|element| element.geometry(scale.into()).overlaps(around));
            let (slot, area, under) = if covered {
                (&mut kept.behind, around, under)
            } else {
                let slot = self.desktops.entry(name.to_owned()).or_default();
                let desktop = elements.get(desktop..).unwrap_or_default();
                (slot, Rectangle::from_size(size), desktop)
            };
            let had = slot.take().filter(|had| had.is_for(area.size, blur));
            let picture = had.or_else(|| Picture::new::<R, P>(renderer, (area, blur), scale));
            *slot = picture.map(|mut picture| {
                picture.area = area;
                let blurred = picture.draw::<R, P>(renderer, (under, behind), &mut *frosting);
                picture.blurs += u64::from(blurred);
                picture
            });
            slot.as_mut().and_then(|picture| {
                self.blurs += std::mem::take(&mut picture.blurs);
                let at = (picture.area, scale, pane.opacity);
                let made = picture.made.as_deref();
                made.and_then(|made| frosting.element(renderer, made, at))
            })
        });
        let solid = Color {
            alpha: u8::MAX,
            ..pane.color
        };
        let logical = pane.area.size.to_f64().to_logical(scale).to_i32_round();
        kept.solid.update(logical, color(solid));
        let mut glass = Vec::new();
        for (index, strip) in corners::rounded(pane.area, pane.radius)
            .into_iter()
            .enumerate()
        {
            let frosted = blurred.as_ref().and_then(|blurred| {
                let whole = Frosted(Rc::clone(blurred));
                let cut = Strip::of(whole, scale, strip, index, &kept.names);
                cut.map(Element::Frosted)
            });
            let element = frosted.or_else(|| {
                let kind = Kind::Unspecified;
                let at = pane.area.loc;
                let whole = SolidColorRenderElement::from_buffer(
                    &kept.solid,
                    at,
                    scale,
                    pane.opacity,
                    kind,
                );
                Strip::of(whole, scale, strip, index, &kept.names).map(Element::Tinted)
            });
            glass.extend(element);
        }
        self.kept.insert(key, kept);
        glass
    }
}

impl Picture {
    /// A picture to draw what is behind glass into, of `area` of the screen,
    /// to be blurred by `blur`; none if the renderer cannot make one.
    fn new<R, P>(
        renderer: &mut R,
        (area, blur): (Rectangle<i32, Physical>, Blur),
        scale: f64,
    ) -> Option<Self>
    where
        R: Offscreen<P>,
        P: std::fmt::Debug + 'static,
    {
        let size = Size::<i32, Buffer>::from((area.size.w, area.size.h));
        let target = renderer.create_buffer(Fourcc::Abgr8888, size).ok();
        target.map(|target| Self {
            area,
            blur,
            target: Box::new(target),
            tracker: OutputDamageTracker::new(area.size, scale, Transform::Normal),
            made: None,
            blurs: 0,
        })
    }

    /// Whether it can be drawn again for what is `size` large, blurred by
    /// `blur`.
    fn is_for(&self, size: Size<i32, Physical>, blur: Blur) -> bool {
        self.area.size == size && self.blur == blur
    }

    /// Draws `under` into it, over `behind`, and has `frosting` blur it
    /// again if that changed anything; whether it did.
    fn draw<R, P>(
        &mut self,
        renderer: &mut R,
        (under, behind): (&[Element<R>], Color32F),
        frosting: &mut dyn Frosting<R, P>,
    ) -> bool
    where
        R: Renderer + ImportAll + ImportMem + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let corner = Point::from((-self.area.loc.x, -self.area.loc.y));
        let moved: Vec<_> = under
            .iter()
            .map(|element| RelocateRenderElement::from_element(element, corner, Relocate::Relative))
            .collect();
        let size = self.area.size;
        let tracker = &mut self.tracker;
        let unseen = self.made.is_none();
        let (blur, made) = (&self.blur, &mut self.made);
        let target = (&mut *self.target as &mut dyn Any).downcast_mut::<P>();
        target.is_some_and(|target| {
            let changed = renderer.bind(&mut *target).ok().map(|mut framebuffer| {
                let drawn = tracker.render_output(renderer, &mut framebuffer, 1, &moved, behind);
                drawn.is_ok_and(|drawn| drawn.damage.is_some())
            });
            changed.is_some_and(|changed| {
                (changed || unseen) && frosting.blur(renderer, (target, size), blur, made)
            })
        })
    }
}
