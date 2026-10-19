//! What a drag over a screen comes to: the region drawn, or the screen
//! clicked; and where the dim and the border round it are drawn.

/// A rectangle of a screen, in units of layout: its left and top, and how
/// far across and down it goes.
pub(crate) type Rect = (i32, i32, u32, u32);

/// Less far than this a drag is a click, in units of layout either way.
const CLICK: u32 = 4;

/// A rectangle of a screen's picture, in its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    /// The column of its left edge.
    pub x: u32,
    /// The row of its top edge.
    pub y: u32,
    /// How many pixels across it is.
    pub width: u32,
    /// How many pixels down it is.
    pub height: u32,
}

/// What the user picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picked {
    /// Every screen.
    Everything,
    /// One screen, by its place among those it was given pictures of.
    Screen(usize),
    /// A region of one screen, in that screen's pixels.
    Region {
        /// The screen, by its place among those it was given pictures of.
        screen: usize,
        /// Where the region is in its picture.
        area: Area,
    },
}

/// The rectangle between two corners, as far as they are on a screen
/// `shown` units large.
pub(crate) fn between(from: (i32, i32), to: (i32, i32), shown: (u32, u32)) -> Rect {
    let along = |one: i32, other: i32, room: u32| {
        let room = room.cast_signed();
        let (one, other) = (one.clamp(0, room), other.clamp(0, room));
        (one.min(other), one.abs_diff(other))
    };
    let (left, across) = along(from.0, to.0, shown.0);
    let (top, down) = along(from.1, to.1, shown.1);
    (left, top, across, down)
}

/// What a drag from `from` to `to` over screen `screen` comes to, the
/// screen shown `shown` units large from a picture `pixels` large: that
/// screen, for one that hardly moved, as a click; otherwise the region
/// between the two, in the picture's pixels, a pixel at least each way.
pub(crate) fn picked(
    screen: usize,
    (from, to): ((i32, i32), (i32, i32)),
    shown: (u32, u32),
    pixels: (u32, u32),
) -> Picked {
    let (left, top, across, down) = between(from, to, shown);
    if across < CLICK && down < CLICK {
        return Picked::Screen(screen);
    }
    let scaled = |at: u32, room: u32, pixels: u32| {
        let at = u64::from(at) * u64::from(pixels) / u64::from(room.max(1));
        u32::try_from(at).unwrap_or(pixels)
    };
    let (x, y) = (left.unsigned_abs(), top.unsigned_abs());
    let (x, right) = (
        scaled(x, shown.0, pixels.0),
        scaled(x + across, shown.0, pixels.0),
    );
    let (y, bottom) = (
        scaled(y, shown.1, pixels.1),
        scaled(y + down, shown.1, pixels.1),
    );
    let area = Area {
        x,
        y,
        width: right.saturating_sub(x).max(1),
        height: bottom.saturating_sub(y).max(1),
    };
    Picked::Region { screen, area }
}

/// Where the dim goes on a screen `shown` units large: all over it while
/// nothing is drawn, and otherwise in four bands round `drawn`, above,
/// below, to the left and to the right of it.
pub(crate) fn dimmed(drawn: Option<Rect>, (width, height): (u32, u32)) -> [Rect; 4] {
    let Some((left, top, across, down)) = drawn else {
        return [(0, 0, width, height), NOTHING, NOTHING, NOTHING];
    };
    let (right, bottom) = (left + across.cast_signed(), top + down.cast_signed());
    [
        (0, 0, width, top.unsigned_abs()),
        (
            0,
            bottom,
            width,
            height.saturating_sub(bottom.unsigned_abs()),
        ),
        (0, top, left.unsigned_abs(), down),
        (right, top, width.saturating_sub(right.unsigned_abs()), down),
    ]
}

/// Where the border goes round `drawn`, `thick` wide, just inside it:
/// along its top, its bottom, its left and its right. Nothing while
/// nothing is drawn.
pub(crate) fn bordered(drawn: Option<Rect>, thick: u32) -> [Rect; 4] {
    let Some((left, top, across, down)) = drawn else {
        return [NOTHING; 4];
    };
    let thick = thick.min(across).min(down);
    let (right, bottom) = (left + across.cast_signed(), top + down.cast_signed());
    let inside = thick.cast_signed();
    [
        (left, top, across, thick),
        (left, bottom - inside, across, thick),
        (left, top, thick, down),
        (right - inside, top, thick, down),
    ]
}

/// A rectangle with nothing in it, which is not drawn.
pub(crate) const NOTHING: Rect = (0, 0, 0, 0);

#[cfg(test)]
mod tests {
    use super::*;

    const SHOWN: (u32, u32) = (640, 480);

    #[test]
    fn a_drag_is_the_region_between_its_ends_whichever_way_it_goes() {
        assert_eq!(between((10, 20), (110, 70), SHOWN), (10, 20, 100, 50));
        assert_eq!(between((110, 70), (10, 20), SHOWN), (10, 20, 100, 50));
        // As far as the screen goes, and no farther.
        assert_eq!(between((600, 400), (700, -5), SHOWN), (600, 0, 40, 400));
    }

    #[test]
    fn a_drag_that_hardly_moves_is_a_click_on_the_screen() {
        let pixels = (1280, 960);
        assert_eq!(
            picked(1, ((10, 10), (12, 13)), SHOWN, pixels),
            Picked::Screen(1)
        );
        let area = Area {
            x: 20,
            y: 40,
            width: 200,
            height: 100,
        };
        let region = Picked::Region { screen: 1, area };
        assert_eq!(picked(1, ((10, 20), (110, 70)), SHOWN, pixels), region);
        // Moved only one way, it is a region still, a pixel at least the
        // other way.
        let thin = Area {
            x: 10,
            y: 20,
            width: 1,
            height: 100,
        };
        let picked = picked(0, ((10, 20), (10, 120)), SHOWN, SHOWN);
        assert_eq!(
            picked,
            Picked::Region {
                screen: 0,
                area: thin
            }
        );
    }

    #[test]
    fn the_dim_is_all_over_until_a_region_is_drawn_and_then_round_it() {
        assert_eq!(
            dimmed(None, SHOWN),
            [(0, 0, 640, 480), NOTHING, NOTHING, NOTHING]
        );
        let bands = dimmed(Some((100, 50, 200, 100)), SHOWN);
        assert_eq!(
            bands,
            [
                (0, 0, 640, 50),
                (0, 150, 640, 330),
                (0, 50, 100, 100),
                (300, 50, 340, 100),
            ]
        );
    }

    #[test]
    fn the_border_is_inside_the_region_and_no_wider_than_it() {
        assert_eq!(bordered(None, 2), [NOTHING; 4]);
        assert_eq!(
            bordered(Some((100, 50, 200, 100)), 2),
            [
                (100, 50, 200, 2),
                (100, 148, 200, 2),
                (100, 50, 2, 100),
                (298, 50, 2, 100),
            ]
        );
        assert_eq!(bordered(Some((0, 0, 1, 5),), 2)[2], (0, 0, 1, 5));
    }
}
