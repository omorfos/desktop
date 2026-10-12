//! What has gone from the screen, for as long as it takes to fade out.
//!
//! A window that closes takes its picture with it, and the compositor is
//! only lent something to draw with while a frame is being drawn. So what
//! is on the screen is noted as it is drawn: each piece of it, where it
//! was and the texture it was drawn from. When the thing goes, those
//! pieces are drawn in its place, fainter each frame.
//!
//! See `docs/adr/0026-fading-out.md`.

use std::any::Any;

use smithay::backend::renderer::element::surface::WaylandSurfaceTexture;
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Color32F, Frame, Renderer};
use smithay::utils::{Buffer, Logical, Physical, Point, Rectangle, Scale as Factor, Transform};

/// What a piece of something is drawn from.
#[derive(Debug, Clone)]
pub(crate) enum Paint<T> {
    /// A picture the renderer holds.
    Texture(T),
    /// One colour all over.
    Solid(Color32F),
}

impl<T: Clone> Paint<T> {
    /// What a surface was drawn from.
    pub(crate) fn of<R>(texture: &WaylandSurfaceTexture<R>) -> Self
    where
        R: Renderer<TextureId = T>,
    {
        match texture {
            WaylandSurfaceTexture::Texture(texture) => Self::Texture(texture.clone()),
            WaylandSurfaceTexture::SolidColor(color) => Self::Solid(*color),
        }
    }
}

/// One piece of something as it was last drawn.
#[derive(Debug)]
struct Piece<T> {
    paint: Paint<T>,
    /// The part of what it is drawn from that was drawn.
    src: Rectangle<f64, Buffer>,
    transform: Transform,
    /// Where it was, in the space windows live in.
    area: Rectangle<f64, Logical>,
}

/// The pieces of one thing, noted while it is drawn. Nothing is noted, and
/// nothing is spent on it, when the thing would not fade out anyway.
#[derive(Debug)]
pub(crate) struct Noting<T> {
    pieces: Option<Vec<Piece<T>>>,
    /// Where in the space the output being drawn on is, and how many
    /// pixels it has to a unit of layout.
    origin: Point<f64, Logical>,
    scale: f64,
}

impl<T: Clone + 'static> Noting<T> {
    /// Begins noting something drawn on an output whose corner is at
    /// `origin` and which has `scale` pixels to a unit, if it is `wanted`.
    pub(crate) fn new(wanted: bool, origin: Point<i32, Logical>, scale: f64) -> Self {
        Self {
            pieces: wanted.then(Vec::new),
            origin: origin.to_f64(),
            scale,
        }
    }

    /// What a surface is drawn from, if it is being noted.
    pub(crate) fn paint<R>(&self, texture: &WaylandSurfaceTexture<R>) -> Option<Paint<T>>
    where
        R: Renderer<TextureId = T>,
    {
        self.pieces.as_ref().map(|_| Paint::of(texture))
    }

    /// Notes a piece: what is about to be drawn, and what from.
    pub(crate) fn add(&mut self, drawn: &impl Element, paint: Option<Paint<T>>) {
        let scale = Factor::from(self.scale);
        let pixels = drawn.geometry(scale).to_f64();
        let mut area = pixels.to_logical(scale);
        area.loc += self.origin;
        let piece = paint.map(|paint| Piece {
            paint,
            src: drawn.src(),
            transform: drawn.transform(),
            area,
        });
        if let Some((pieces, piece)) = self.pieces.as_mut().zip(piece) {
            pieces.push(piece);
        }
    }

    /// What was noted, to be kept until the next frame is drawn.
    pub(crate) fn done(self) -> Option<Noted> {
        self.pieces.map(|pieces| Noted {
            count: pieces.len(),
            pieces: Box::new(pieces),
            names: Vec::new(),
        })
    }
}

/// The pieces of one thing as it was last drawn, whatever it was drawn
/// with.
#[derive(Debug)]
pub(crate) struct Noted {
    pieces: Box<dyn Any>,
    count: usize,
    /// What tells each piece from every other, from frame to frame, once
    /// the thing has gone and its pieces are drawn for themselves.
    names: Vec<Id>,
}

impl Noted {
    /// The pieces, now that the thing has gone and they are all that is
    /// left of it.
    pub(crate) fn left(mut self) -> Self {
        self.names = (0..self.count).map(|_| Id::new()).collect();
        self
    }

    /// The pieces as they are to be drawn, `opacity` of the way to being
    /// all there, on an output whose corner is at `origin` and which has
    /// `scale` pixels to a unit. There are none if they were noted while
    /// drawing with something else.
    pub(crate) fn elements<R>(
        &self,
        origin: Point<i32, Logical>,
        scale: f64,
        opacity: f32,
    ) -> impl Iterator<Item = Ghost<R>>
    where
        R: Renderer,
        R::TextureId: Clone + 'static,
    {
        let pieces = self.pieces.downcast_ref::<Vec<Piece<R::TextureId>>>();
        let factor = Factor::from(scale);
        let named = pieces.into_iter().flatten().zip(&self.names);
        named.map(move |(piece, name)| {
            let mut area = piece.area;
            area.loc -= origin.to_f64();
            Ghost {
                id: name.clone(),
                paint: piece.paint.clone(),
                src: piece.src,
                transform: piece.transform,
                geometry: area.to_physical_precise_round(factor),
                opacity,
            }
        })
    }
}

/// A piece of something that has gone, as it is drawn.
#[derive(Debug)]
pub(crate) struct Ghost<R: Renderer> {
    id: Id,
    paint: Paint<R::TextureId>,
    src: Rectangle<f64, Buffer>,
    transform: Transform,
    geometry: Rectangle<i32, Physical>,
    opacity: f32,
}

impl<R: Renderer> Element for Ghost<R> {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        // What it shows never changes; only how much of it there is.
        CommitCounter::default()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.src
    }

    fn transform(&self) -> Transform {
        self.transform
    }

    fn geometry(&self, _: Factor<f64>) -> Rectangle<i32, Physical> {
        self.geometry
    }

    fn alpha(&self) -> f32 {
        self.opacity
    }
}

impl<R: Renderer> RenderElement<R> for Ghost<R> {
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        // A picture that can no longer be drawn, because the program it
        // came from took back the memory it was in, is left out. That is
        // no reason to lose the frame.
        let _ = match &self.paint {
            Paint::Texture(texture) => frame.render_texture_from_to(
                texture,
                src,
                dst,
                damage,
                opaque_regions,
                self.transform,
                self.opacity,
            ),
            Paint::Solid(color) => frame.draw_solid(dst, damage, *color * self.opacity),
        };
        Ok(())
    }
}
