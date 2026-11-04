//! The shadow a part casts (`box-shadow`, ADR 33): a picture, made on the
//! CPU, of the part's box made larger by the shadow's spread, rounded as
//! the part is, its edge softened by the shadow's blur, in the shadow's
//! colour.
//!
//! The picture is made no larger than its corners need, with one row and
//! one column between them, and cut into nine pieces: the corners drawn as
//! they are, the edges and the middle stretched. So one picture serves a
//! part of any size, and one being resized needs no other.

use omorfos_config::Shadow;

use crate::canvas::{Area, Shape, span};
use crate::scale::Scale;

/// What of a shadow's picture a piece is, in pixels: across, down, wide
/// and high.
pub type Cut = (u32, u32, u32, u32);

/// A shadow's picture, and how it is cut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shade {
    /// Its pixels, row by row, as ARGB with premultiplied alpha in the byte
    /// order renderers call `Argb8888`.
    pixels: Vec<u8>,
    /// How many pixels it is across and down: two corners and one between.
    side: u32,
    /// How many pixels across and down each corner is, and how many units
    /// of layout it stands for.
    corner: (u32, u32),
    shadow: Shadow,
}

/// A piece of a shadow's picture, and where it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Piece {
    /// What of the picture it is.
    pub from: Cut,
    /// Where it is drawn, in units of layout.
    pub to: Area,
}

impl Shade {
    /// The picture of `shadow` cast by a part whose corners are rounded by
    /// `radius`, drawn at `scale`; nothing for a shadow not to be seen.
    #[must_use]
    pub fn new(shadow: &Shadow, radius: u32, scale: Scale) -> Option<Self> {
        if shadow.is_none() {
            return None;
        }
        let pixels = |units: u32| span(scale, 0, units).1;
        let rounded = radius.saturating_add_signed(shadow.spread);
        let corner = rounded.saturating_add(shadow.blur.saturating_mul(2)).max(1);
        let (corner_pixels, blur) = (pixels(corner).max(1), pixels(shadow.blur));
        let side = corner_pixels * 2 + 1;
        // The box, as large as the shadow is before it is softened, as far
        // in from each side as the softening reaches out.
        let inside = side - 2 * blur.min(corner_pixels);
        let shape = Shape::plain(inside, pixels(rounded).min(inside / 2));
        let length = usize::try_from(side).unwrap_or(0);
        let mut cover = vec![0.0_f32; length * length];
        let margin = usize::try_from((side - inside) / 2).unwrap_or(0);
        for y in 0..inside {
            for x in 0..inside {
                let at = (y as usize + margin) * length + x as usize + margin;
                cover[at] = f32::from(shape.covered((x, y))) / 255.0;
            }
        }
        let cover = softened(&cover, length, blur);
        let color = shadow.color;
        let pixels = cover.iter().flat_map(|share| {
            let alpha = f32::from(color.alpha) * share;
            let channel = |value: u8| byte(f32::from(value) * alpha / 255.0);
            [
                channel(color.blue),
                channel(color.green),
                channel(color.red),
                byte(alpha),
            ]
        });
        Some(Self {
            pixels: pixels.collect(),
            side,
            corner: (corner_pixels, corner),
            shadow: *shadow,
        })
    }

    /// Its pixels, row by row, as ARGB with premultiplied alpha in the byte
    /// order renderers call `Argb8888`.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.pixels
    }

    /// How many pixels it is across and down.
    #[must_use]
    pub const fn side(&self) -> u32 {
        self.side
    }

    /// Its nine pieces, the corners, the edges between them and the
    /// middle, row by row: each as what of the picture it is, in pixels
    /// across, down, wide and high, and its pixels.
    #[must_use]
    pub fn cut(&self) -> Vec<(Cut, Vec<u8>)> {
        let corner = self.corner.0;
        let bands = [(0, corner), (corner, 1), (corner + 1, corner)];
        let pieces = bands
            .iter()
            .flat_map(|&(y, high)| bands.iter().map(move |&(x, wide)| (x, y, wide, high)));
        pieces.map(|from| (from, self.slice(from))).collect()
    }

    /// The pixels of `from`, a piece of the picture: across, down, wide and
    /// high.
    fn slice(&self, (x, y, wide, high): Cut) -> Vec<u8> {
        let row = |row: u32| {
            let start = usize::try_from((row * self.side + x) * 4).unwrap_or(0);
            let end = start + usize::try_from(wide * 4).unwrap_or(0);
            self.pixels.get(start..end).unwrap_or_default()
        };
        (y..y + high).flat_map(row).copied().collect()
    }

    /// Where its pieces are drawn to cast the shadow of a part whose box is
    /// `area`, in units of layout: what of the picture each is, and where
    /// it goes. A part too small for whole corners has them squeezed.
    #[must_use]
    pub fn pieces(&self, area: Area) -> Vec<Piece> {
        let (left, top, wide, high) = bounds(&self.shadow, area);
        let (corner_pixels, corner) = (self.corner.0, i64::from(self.corner.1));
        // Along one side: where each of the three bands is drawn, and what
        // of the picture each is.
        let bands = |start: i64, length: i64| {
            let ends = corner.min(length / 2);
            let middle = length - 2 * ends;
            let drawn = [
                (start, ends),
                (start + ends, middle),
                (start + ends + middle, ends),
            ];
            let taken = [
                (0, corner_pixels),
                (corner_pixels, 1),
                (corner_pixels + 1, corner_pixels),
            ];
            drawn.into_iter().zip(taken)
        };
        let unit = |value: i64| i32::try_from(value).unwrap_or(i32::MAX);
        let size = |value: i64| u32::try_from(value).unwrap_or(0);
        let mut pieces = Vec::new();
        let across = (i64::from(left), i64::from(wide));
        for ((top, high), (from_y, rows)) in bands(i64::from(top), i64::from(high)) {
            for ((left, wide), (from_x, columns)) in bands(across.0, across.1) {
                if wide > 0 && high > 0 {
                    pieces.push(Piece {
                        from: (from_x, from_y, columns, rows),
                        to: (unit(left), unit(top), size(wide), size(high)),
                    });
                }
            }
        }
        pieces
    }
}

/// Where all of `shadow` falls, cast by a part whose box is `area`, in
/// units of layout; nothing for a shadow not to be seen.
#[must_use]
pub fn falls(shadow: &Shadow, area: Area) -> Option<Area> {
    (!shadow.is_none()).then(|| bounds(shadow, area))
}

/// Where all of `shadow` falls, cast by a part whose box is `area`: moved
/// by its offset, and out by its spread and its blur, or in by as much as
/// it is drawn in, to nothing at most.
fn bounds(shadow: &Shadow, (x, y, width, height): Area) -> Area {
    let reach = i64::from(shadow.spread) + i64::from(shadow.blur);
    let start = |from: i32, by: i32| i64::from(from) + i64::from(by) - reach;
    let length = |of: u32| (i64::from(of) + 2 * reach).max(0);
    let (left, top) = (start(x, shadow.x), start(y, shadow.y));
    let (wide, high) = (length(width), length(height));
    let unit = |value: i64| i32::try_from(value).unwrap_or(i32::MAX);
    let size = |value: i64| u32::try_from(value).unwrap_or(u32::MAX);
    (unit(left), unit(top), size(wide), size(high))
}

/// `cover`, a square `side` pixels across, softened over `blur` pixels as a
/// shadow's edge is: by a Gaussian whose deviation is half that, as CSS has
/// it, across and then down, with nothing beyond the square.
fn softened(cover: &[f32], side: usize, blur: u32) -> Vec<f32> {
    if blur == 0 {
        return cover.to_vec();
    }
    let reach = i64::from(blur);
    let deviation = f64::from(blur) / 2.0;
    let weights: Vec<f32> = (-reach..=reach)
        .map(|at| {
            #[expect(
                clippy::cast_precision_loss,
                reason = "a shadow's blur is a few dozen pixels at most"
            )]
            let at = at as f64;
            #[expect(clippy::cast_possible_truncation, reason = "a weight is from 0 to 1")]
            let weight = (-at * at / (2.0 * deviation * deviation)).exp() as f32;
            weight
        })
        .collect();
    let total: f32 = weights.iter().sum();
    let pass = |from: &[f32], step: (usize, usize)| {
        let mut to = vec![0.0_f32; from.len()];
        for row in 0..side {
            for column in 0..side {
                let mut sum = 0.0;
                for (offset, weight) in (-reach..=reach).zip(&weights) {
                    let along = i64::try_from(column).unwrap_or(0) + offset;
                    let Ok(along) = usize::try_from(along) else {
                        continue;
                    };
                    if along < side {
                        sum += weight * from[row * step.0 + along * step.1];
                    }
                }
                to[row * step.0 + column * step.1] = sum / total;
            }
        }
        to
    };
    let across = pass(cover, (side, 1));
    pass(&across, (1, side))
}

/// The nearest byte to a value from 0 to 255.
fn byte(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is from 0 to 255"
    )]
    let byte = value.round().clamp(0.0, 255.0) as u8;
    byte
}

#[cfg(test)]
mod tests {
    use omorfos_config::Color;

    use super::*;

    /// A window's shadow, as the design language has it.
    const SOFT: Shadow = Shadow {
        x: 0,
        y: 8,
        blur: 24,
        spread: 0,
        color: Color::rgba(0, 0, 0, 72),
    };

    /// How opaque the picture is at a pixel.
    fn alpha(shade: &Shade, (x, y): (u32, u32)) -> u8 {
        let at = (y * shade.side() + x) as usize * 4;
        shade.bytes()[at + 3]
    }

    #[test]
    fn no_shadow_has_no_picture() {
        assert_eq!(Shade::new(&Shadow::NONE, 8, Scale::ONE), None);
    }

    #[test]
    fn a_shadow_is_as_opaque_as_its_colour_inside_and_fades_to_nothing_outside() {
        let shade = Shade::new(&SOFT, 8, Scale::ONE).unwrap();
        // Corners of 8 and twice 24, and one between.
        assert_eq!(shade.side(), 2 * (8 + 48) + 1);
        let middle = shade.side() / 2;
        assert!(alpha(&shade, (middle, middle)).abs_diff(72) <= 1, "inside");
        assert_eq!(alpha(&shade, (0, middle)), 0, "outside");
        // Ever fainter from inside out, along the middle row.
        let row: Vec<u8> = (0..=middle)
            .rev()
            .map(|x| alpha(&shade, (x, middle)))
            .collect();
        assert!(row.windows(2).all(|pair| pair[1] <= pair[0]), "{row:?}");
        assert!(row.iter().any(|alpha| (1..72).contains(alpha)), "soft");
        // Rounder at the corner than along an edge as far in.
        let depth = 30;
        assert!(alpha(&shade, (depth, depth)) < alpha(&shade, (depth, middle)));
        // Black, so its colours are none, as premultiplied.
        assert_eq!(shade.bytes()[..3], [0, 0, 0]);
    }

    #[test]
    fn a_shadow_not_softened_has_a_sharp_edge_in_its_colour() {
        let red = Shadow {
            x: 2,
            y: 2,
            color: Color::rgb(255, 0, 0),
            ..Shadow::NONE
        };
        let shade = Shade::new(&red, 0, Scale::ONE).unwrap();
        assert_eq!(shade.side(), 3, "a corner of one, and one between");
        let pixels: Vec<&[u8]> = shade.bytes().chunks(4).collect();
        assert!(pixels.iter().all(|pixel| *pixel == [0, 0, 255, 255]));
        // Spread inward more than its corners are round, it is no rounder.
        let inward = Shadow { spread: -20, ..red };
        assert_eq!(Shade::new(&inward, 4, Scale::ONE).unwrap().side(), 3);
    }

    #[test]
    fn a_shadow_drawn_at_a_larger_scale_has_more_pixels_and_the_same_place() {
        let shade = Shade::new(&SOFT, 8, Scale::ONE).unwrap();
        let sharper = Shade::new(&SOFT, 8, Scale::new(2.0)).unwrap();
        assert_eq!(sharper.side(), 2 * (2 * 56) + 1);
        let area = (100, 100, 400, 300);
        let places = |shade: &Shade| -> Vec<Area> {
            shade.pieces(area).iter().map(|piece| piece.to).collect()
        };
        assert_eq!(places(&shade), places(&sharper));
    }

    #[test]
    fn nine_pieces_cover_where_the_shadow_falls_and_nothing_else() {
        let shade = Shade::new(&SOFT, 8, Scale::ONE).unwrap();
        let pieces = shade.pieces((100, 100, 400, 300));
        assert_eq!(pieces.len(), 9);
        // Out by the blur, and down by 8.
        let (left, top) = (100 - 24, 100 + 8 - 24);
        let (wide, high) = (400 + 48, 300 + 48);
        let corner = 56;
        assert_eq!(pieces[0].to, (left, top, corner, corner));
        assert_eq!(pieces[0].from, (0, 0, corner, corner));
        assert_eq!(pieces[4].to, (left + 56, top + 56, wide - 112, high - 112));
        assert_eq!(pieces[4].from, (56, 56, 1, 1));
        assert_eq!(
            pieces[8].to,
            (
                left + wide.cast_signed() - 56,
                top + high.cast_signed() - 56,
                56,
                56
            )
        );
        assert_eq!(pieces[8].from, (57, 57, 56, 56));
        let covered: u64 = pieces
            .iter()
            .map(|piece| u64::from(piece.to.2) * u64::from(piece.to.3))
            .sum();
        assert_eq!(covered, u64::from(wide) * u64::from(high));
        // Spread out, it is larger by that on each side.
        let spread = Shadow { spread: 4, ..SOFT };
        let spread = Shade::new(&spread, 8, Scale::ONE).unwrap();
        assert_eq!(spread.pieces((100, 100, 400, 300))[0].to.0, left - 4);
    }

    #[test]
    fn a_shadow_falls_where_its_pieces_go_and_none_falls_nowhere() {
        let area = (100, 100, 400, 300);
        let pieces = Shade::new(&SOFT, 8, Scale::ONE).unwrap().pieces(area);
        let (first, last) = (pieces[0].to, pieces[8].to);
        let wide = last.0 + last.2.cast_signed() - first.0;
        let high = last.1 + last.3.cast_signed() - first.1;
        let whole = (first.0, first.1, wide.cast_unsigned(), high.cast_unsigned());
        assert_eq!(falls(&SOFT, area), Some(whole));
        assert_eq!(whole, (76, 84, 448, 348));
        // Drawn in, it falls within the part, and no shadow at all where
        // it is drawn in further than the part is large.
        let within = Shadow {
            spread: -30,
            blur: 0,
            ..SOFT
        };
        assert_eq!(falls(&within, area), Some((130, 138, 340, 240)));
        let gone = Shadow {
            spread: -200,
            blur: 0,
            ..SOFT
        };
        assert_eq!(falls(&gone, area), Some((300, 308, 0, 0)));
        assert_eq!(falls(&Shadow::NONE, area), None);
    }

    #[test]
    fn the_picture_is_cut_into_its_corners_edges_and_middle() {
        let shade = Shade::new(&SOFT, 8, Scale::ONE).unwrap();
        let cut = shade.cut();
        let from: Vec<Cut> = cut.iter().map(|(from, _)| *from).collect();
        assert_eq!(from[0], (0, 0, 56, 56));
        assert_eq!(from[1], (56, 0, 1, 56));
        assert_eq!(from[4], (56, 56, 1, 1));
        assert_eq!(from[8], (57, 57, 56, 56));
        let bytes: usize = cut.iter().map(|(_, bytes)| bytes.len()).sum();
        assert_eq!(bytes, shade.bytes().len(), "all of it, once");
        // The middle is the middle pixel.
        let middle = usize::try_from((56 * shade.side() + 56) * 4).unwrap();
        assert_eq!(cut[4].1, shade.bytes()[middle..middle + 4]);
        // Each piece is the size it says.
        for ((_, _, wide, high), bytes) in &cut {
            assert_eq!(bytes.len(), usize::try_from(wide * high * 4).unwrap());
        }
    }

    #[test]
    fn a_part_too_small_for_whole_corners_has_them_squeezed() {
        let shade = Shade::new(&SOFT, 8, Scale::ONE).unwrap();
        let pieces = shade.pieces((0, 0, 10, 10));
        // 58 across: two corners of 29, and nothing between.
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces[0].to.2, 29);
        assert_eq!(pieces[0].from.2, 56, "all of the corner");
        // Spread inward past nothing, it is not there.
        let gone = Shadow {
            spread: -32,
            blur: 0,
            ..SOFT
        };
        let gone = Shade::new(&gone, 0, Scale::ONE).unwrap();
        assert_eq!(gone.pieces((0, 0, 10, 10)), []);
    }
}
