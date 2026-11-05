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
//! The blur is made as the renderer can: in software, read back from it,
//! or not at all, when glass is drawn solid in its part's colour.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use omorfos_config::{Color, Style};
use omorfos_ui::Blur;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::utils::{Relocate, RelocateRenderElement};
use smithay::backend::renderer::element::{Element as _, Kind};
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, ImportAll, ImportMem, Offscreen, Renderer, Texture,
};
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Size, Transform};

use crate::capture::Held;
use crate::compositor::{Element, View, color};
use crate::corners::{self, Names, Strip};

/// How glass is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Frost {
    /// Solid in its part's colour, as where blur cannot be drawn: nothing
    /// behind it is looked at.
    #[default]
    Solid,
    /// Blurred on the CPU from a picture of what is behind it, read back
    /// from the renderer: the blur's specification, which the tests draw.
    /// Reading a picture back from a graphics card makes it wait, so this
    /// is not for one.
    Software,
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
    frost: Frost,
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
/// tracker that says when what it is of changes, and the blur.
#[derive(Debug)]
struct Picture {
    /// What of the screen it is of, in pixels.
    area: Rectangle<i32, Physical>,
    blur: Blur,
    target: Box<dyn Held>,
    tracker: OutputDamageTracker,
    blurred: Option<MemoryRenderBuffer>,
    /// How many blurs it has made that have not been counted yet.
    blurs: u64,
}

impl Glass {
    /// Draws glass as `frost` says from now on.
    pub(crate) fn set(&mut self, frost: Frost) {
        self.frost = frost;
    }

    /// How many blurs have been made since the compositor started.
    pub(crate) const fn blurs(&self) -> u64 {
        self.blurs
    }

    /// What `scene` draws on `screen`, with the glass of each of its sheets
    /// put under its part: blurred, or solid if `solid` or if it cannot be
    /// blurred. `behind` is what is drawn where nothing else is.
    pub(crate) fn frost<R, P>(
        &mut self,
        renderer: &mut R,
        screen: Screen<'_>,
        scene: Scene<R>,
        (behind, solid): (Color32F, bool),
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
        let blurs = self.frost == Frost::Software && !solid;
        for index in 0..panes.len() {
            let pane = panes[index].clone();
            let glass = self.glass::<R, P>(
                renderer,
                screen,
                (&pane, &elements, desktop),
                (behind, blurs),
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
    /// on, cut to its corners: what is behind it blurred if `blurs` and if
    /// it can be, else solid.
    fn glass<R, P>(
        &mut self,
        renderer: &mut R,
        (name, size, scale): Screen<'_>,
        (pane, elements, desktop): (&Pane, &[Element<R>], usize),
        (behind, blurs): (Color32F, bool),
    ) -> Vec<Element<R>>
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Offscreen<P> + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let key = (name.to_owned(), pane.key);
        let mut kept = self.kept.remove(&key).unwrap_or_default();
        let blur = Blur::new(pane.blur).filter(|_| blurs);
        let blurred = blur.and_then(|blur| {
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
                picture.blurs += u64::from(picture.draw::<R, P>(renderer, under, behind));
                picture
            });
            slot.as_mut().and_then(|picture| {
                self.blurs += std::mem::take(&mut picture.blurs);
                let area = picture.area;
                picture.blurred.clone().map(|blurred| (blurred, area))
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
            let frosted = blurred.as_ref().and_then(|(blurred, area)| {
                let size = area.size.to_f64().to_logical(scale).to_i32_round();
                let (at, opacity) = (area.loc.to_f64(), Some(pane.opacity));
                let kind = Kind::Unspecified;
                let whole = MemoryRenderBufferRenderElement::from_buffer(
                    renderer,
                    at,
                    blurred,
                    opacity,
                    None,
                    Some(size),
                    kind,
                );
                let whole = whole.ok();
                let cut =
                    whole.and_then(|whole| Strip::of(whole, scale, strip, index, &kept.names));
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
            blurred: None,
            blurs: 0,
        })
    }

    /// Whether it can be drawn again for what is `size` large, blurred by
    /// `blur`.
    fn is_for(&self, size: Size<i32, Physical>, blur: Blur) -> bool {
        self.area.size == size && self.blur == blur
    }

    /// Draws `under` into it, over `behind`, and blurs it again if that
    /// changed anything; whether it did.
    fn draw<R, P>(&mut self, renderer: &mut R, under: &[Element<R>], behind: Color32F) -> bool
    where
        R: Renderer + ImportAll + ImportMem + ExportMem + Bind<P>,
        R::TextureId: Clone + Texture + Send + 'static,
        P: std::fmt::Debug + 'static,
    {
        let corner = Point::from((-self.area.loc.x, -self.area.loc.y));
        let moved: Vec<_> = under
            .iter()
            .map(|element| RelocateRenderElement::from_element(element, corner, Relocate::Relative))
            .collect();
        let size = self.area.size;
        let whole = Rectangle::<i32, Buffer>::from_size((size.w, size.h).into());
        let tracker = &mut self.tracker;
        let unseen = self.blurred.is_none();
        let target = (&mut *self.target as &mut dyn std::any::Any).downcast_mut::<P>();
        let framebuffer = target.and_then(|target| renderer.bind(target).ok());
        let read = framebuffer.and_then(|mut framebuffer| {
            let drawn = tracker.render_output(renderer, &mut framebuffer, 1, &moved, behind);
            let changed = drawn.is_ok_and(|drawn| drawn.damage.is_some()) || unseen;
            let read =
                changed.then(|| renderer.copy_framebuffer(&framebuffer, whole, Fourcc::Abgr8888));
            read.and_then(Result::ok)
        });
        let pixels = read
            .as_ref()
            .and_then(|read| renderer.map_texture(read).ok());
        let blurred = pixels.map(|pixels| {
            let (width, height) = (size.w.unsigned_abs(), size.h.unsigned_abs());
            self.blur.apply(pixels, (width, height))
        });
        let Some(blurred) = blurred else {
            return false;
        };
        if let Some(buffer) = &mut self.blurred {
            let mut context = buffer.render();
            let drawn = context.draw(|into| {
                for (to, from) in into.iter_mut().zip(&blurred) {
                    *to = *from;
                }
                Ok::<_, std::convert::Infallible>(vec![whole])
            });
            drawn.is_ok()
        } else {
            let pixels = (size.w, size.h);
            let made = MemoryRenderBuffer::from_slice(
                &blurred,
                Fourcc::Abgr8888,
                pixels,
                1,
                Transform::Normal,
                None,
            );
            self.blurred = Some(made);
            true
        }
    }
}
