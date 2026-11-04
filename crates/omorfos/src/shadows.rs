//! The shadows the compositor casts under what it draws round (ADR 33): a
//! picture of each shadow made once on the CPU and kept while it is in
//! use, and drawn in nine pieces under each window it falls from.

use std::rc::Rc;

use omorfos_config::{Shadow, Style};
use omorfos_ui::{Area, Cut, Piece, Shade};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::utils::{Logical, Point, Size};

use crate::Scale;
use crate::compositor::{Element, Painted, View};

/// What a shadow's picture is made for: the shadow, how round the corners
/// of what casts it are, and the scale it is drawn at.
type Kind = (Shadow, u32, Scale);

/// The pictures of the shadows in use, each made once.
#[derive(Debug, Default)]
pub(crate) struct Shades {
    /// Those in use as the scene was last laid out.
    kept: Vec<(Kind, Rc<Shade>)>,
    /// Those used in laying it out now.
    used: Vec<(Kind, Rc<Shade>)>,
}

impl Shades {
    /// Begins laying out the scene: a picture not used in it is let go of
    /// as the next is begun.
    pub(crate) fn begun(&mut self) -> &mut Self {
        self.kept = std::mem::take(&mut self.used);
        self
    }

    /// The picture of `shadow`, cast by what has its corners rounded by
    /// `radius`, at `scale`: one already made if there is one, else made
    /// now; nothing for a shadow not to be seen.
    pub(crate) fn of(&mut self, shadow: Shadow, radius: u32, scale: Scale) -> Option<Rc<Shade>> {
        let kind = (shadow, radius, scale);
        let found = |kept: &Vec<(Kind, Rc<Shade>)>| {
            let mut kept = kept.iter();
            kept.find(|(made, _)| *made == kind)
                .map(|(_, shade)| Rc::clone(shade))
        };
        if let Some(shade) = found(&self.used) {
            return Some(shade);
        }
        let shade = found(&self.kept).or_else(|| Shade::new(&shadow, radius, scale).map(Rc::new));
        self.used.extend(shade.clone().map(|shade| (kind, shade)));
        shade
    }

    /// The shadow what is drawn in `style` casts at `scale`, placed under
    /// its box at `area`: `had`, if that is of the same picture, so that
    /// it is not drawn anew, or else one cut now; nothing for a style that
    /// casts none.
    pub(crate) fn cast(
        &mut self,
        style: &Style,
        scale: Scale,
        area: Area,
        had: Option<Shadowing>,
    ) -> Option<Shadowing> {
        let shade = self.of(style.shadow, style.radius, scale)?;
        let mut shadow = match had {
            Some(had) if had.is_of(&shade) => had,
            _ => Shadowing::new(shade),
        };
        shadow.place(Some(area));
        Some(shadow)
    }
}

/// The shadow under one thing: its picture cut into nine pieces, each its
/// own texture, as one texture drawn nine times would be taken for one
/// thing where what is drawn again is worked out; and where they go. Each
/// is drawn from within the border it is cut with, so that stretched it
/// is blended at its edge with its own pixels.
#[derive(Debug)]
pub(crate) struct Shadowing {
    shade: Rc<Shade>,
    pieces: Vec<(Cut, Painted)>,
    /// Where each piece goes, as it was last placed.
    placed: Vec<Piece>,
}

impl Shadowing {
    /// The shadow of `shade`, cut ready to be drawn, and placed nowhere.
    pub(crate) fn new(shade: Rc<Shade>) -> Self {
        let pieces = shade.cut().into_iter().map(|(from, bytes)| {
            let pixels = (from.2.cast_signed(), from.3.cast_signed());
            let bordered = (pixels.0 + 2, pixels.1 + 2);
            let painted = Painted::from_bytes(&bytes, Fourcc::Argb8888, bordered, pixels.into());
            (from, painted.within(1))
        });
        Self {
            pieces: pieces.collect(),
            shade,
            placed: Vec::new(),
        }
    }

    /// Whether it is the shadow of `shade`.
    pub(crate) fn is_of(&self, shade: &Rc<Shade>) -> bool {
        Rc::ptr_eq(&self.shade, shade)
    }

    /// Places it under what has its box at `area`, or, with none, where it
    /// is not seen.
    pub(crate) fn place(&mut self, area: Option<Area>) {
        self.placed = area.map_or_else(Vec::new, |area| self.shade.pieces(area));
    }

    /// Its pieces drawn where they are placed, as see-through as `opacity`
    /// says.
    pub(crate) fn elements<R>(
        &self,
        renderer: &mut R,
        opacity: f32,
        view: View,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        for piece in &self.placed {
            let mut cut = self.pieces.iter();
            let painted = cut.find(|(from, _)| *from == piece.from);
            let (x, y, width, height) = piece.to;
            let at = Point::<i32, Logical>::from((x, y)).to_f64();
            let size = Size::from((width.cast_signed(), height.cast_signed()));
            let drawn = painted
                .and_then(|(_, painted)| painted.stretched(renderer, (at, size), view, opacity));
            elements.extend(drawn);
        }
    }
}
