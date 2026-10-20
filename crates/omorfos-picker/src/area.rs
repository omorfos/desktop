//! What a drag over a screen comes to: the region drawn, or the window or
//! the screen clicked; and where the dim and the border round it are
//! drawn.

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
    /// A window, as a whole.
    Window {
        /// The screen it was picked on, by its place among those it was
        /// given pictures of.
        screen: usize,
        /// The window, by its place among that screen's.
        window: usize,
    },
    /// A region of one screen, in that screen's pixels.
    Region {
        /// The screen, by its place among those it was given pictures of.
        screen: usize,
        /// Where the region is in its picture.
        area: Area,
    },
}

impl Picked {
    /// The screen it is of, or on, by its place among those it was given
    /// pictures of. None for every screen.
    #[must_use]
    pub const fn screen(self) -> Option<usize> {
        match self {
            Self::Everything => None,
            Self::Screen(screen) | Self::Window { screen, .. } | Self::Region { screen, .. } => {
                Some(screen)
            }
        }
    }
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

/// Whether a drag that has gone from `from` to `to` is a click still.
pub(crate) fn click(from: (i32, i32), to: (i32, i32)) -> bool {
    from.0.abs_diff(to.0) < CLICK && from.1.abs_diff(to.1) < CLICK
}

/// The window `at` is on, of `windows`, those on a screen shown `shown`
/// units large from a picture `pixels` large, the one on top first: which
/// it is, and where it is drawn in units. `behind` goes that many further
/// down through those it is on, and round again from the top, so that a
/// window covered by another can be had.
pub(crate) fn under(
    at: (i32, i32),
    windows: &[Area],
    shown: (u32, u32),
    pixels: (u32, u32),
    behind: usize,
) -> Option<(usize, Rect)> {
    let units = |along: u32, room: u32, pixels: u32| {
        let along = u64::from(along) * u64::from(room) / u64::from(pixels.max(1));
        i32::try_from(along).unwrap_or(i32::MAX)
    };
    let drawn = windows.iter().map(|area| {
        let (left, top) = (
            units(area.x, shown.0, pixels.0),
            units(area.y, shown.1, pixels.1),
        );
        let right = units(area.x.saturating_add(area.width), shown.0, pixels.0);
        let bottom = units(area.y.saturating_add(area.height), shown.1, pixels.1);
        (left, top, right, bottom)
    });
    let on = |(_, (left, top, right, bottom)): &(usize, (i32, i32, i32, i32))| {
        (*left..*right).contains(&at.0) && (*top..*bottom).contains(&at.1)
    };
    let drawn = drawn.enumerate();
    let there = drawn.clone().filter(on).count();
    let found = drawn.filter(on).nth(behind % there.max(1));
    found.map(|(window, (left, top, right, bottom))| {
        let size = (right - left).unsigned_abs();
        (window, (left, top, size, (bottom - top).unsigned_abs()))
    })
}

/// What a drag from `from` to `to` over screen `screen` comes to, the
/// screen shown `shown` units large from a picture `pixels` large with
/// `windows` on it: the window or else the screen it began on, for one
/// that hardly moved, as a click, `behind` as many as `under` says;
/// otherwise the region between the two, in the picture's pixels, a pixel
/// at least each way.
pub(crate) fn picked(
    screen: usize,
    (from, to): ((i32, i32), (i32, i32)),
    (windows, behind): (&[Area], usize),
    shown: (u32, u32),
    pixels: (u32, u32),
) -> Picked {
    if click(from, to) {
        let window = under(from, windows, shown, pixels, behind);
        let window = window.map(|(window, _)| Picked::Window { screen, window });
        return window.unwrap_or(Picked::Screen(screen));
    }
    let (left, top, across, down) = between(from, to, shown);
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
            picked(1, ((10, 10), (12, 13)), (&[], 0), SHOWN, pixels),
            Picked::Screen(1)
        );
        let area = Area {
            x: 20,
            y: 40,
            width: 200,
            height: 100,
        };
        let region = Picked::Region { screen: 1, area };
        assert_eq!(
            picked(1, ((10, 20), (110, 70)), (&[], 0), SHOWN, pixels),
            region
        );
        // Moved only one way, it is a region still, a pixel at least the
        // other way.
        let thin = Area {
            x: 10,
            y: 20,
            width: 1,
            height: 100,
        };
        let picked = picked(0, ((10, 20), (10, 120)), (&[], 0), SHOWN, SHOWN);
        assert_eq!(
            picked,
            Picked::Region {
                screen: 0,
                area: thin
            }
        );
    }

    /// Two windows on a picture twice the size it is shown at, the first
    /// over the second where they meet.
    const WINDOWS: [Area; 2] = [
        Area {
            x: 200,
            y: 200,
            width: 400,
            height: 200,
        },
        Area {
            x: 0,
            y: 0,
            width: 300,
            height: 300,
        },
    ];

    #[test]
    fn a_click_on_a_window_picks_the_one_on_top_and_elsewhere_the_screen() {
        let pixels = (1280, 960);
        let click =
            |at: (i32, i32)| picked(1, (at, (at.0 + 1, at.1 + 1)), (&WINDOWS, 0), SHOWN, pixels);
        assert_eq!(
            click((120, 120)),
            Picked::Window {
                screen: 1,
                window: 0
            }
        );
        assert_eq!(
            click((20, 20)),
            Picked::Window {
                screen: 1,
                window: 1
            }
        );
        assert_eq!(click((400, 400)), Picked::Screen(1));
        // A drag that began on a window draws a region all the same.
        let drag = picked(1, ((120, 120), (220, 220)), (&WINDOWS, 0), SHOWN, pixels);
        let area = Area {
            x: 240,
            y: 240,
            width: 200,
            height: 200,
        };
        assert_eq!(drag, Picked::Region { screen: 1, area });
    }

    #[test]
    fn the_window_under_the_pointer_is_drawn_where_it_is_shown() {
        let pixels = (1280, 960);
        assert_eq!(
            under((120, 120), &WINDOWS, SHOWN, pixels, 0),
            Some((0, (100, 100, 200, 100)))
        );
        // Its right and bottom edges are the next window's, or none's.
        assert_eq!(under((300, 150), &WINDOWS, SHOWN, pixels, 0), None);
        assert_eq!(
            under((149, 99), &WINDOWS, SHOWN, pixels, 0).map(|(window, _)| window),
            Some(1)
        );
        assert_eq!(under((0, 0), &[], SHOWN, pixels, 0), None);
    }

    #[test]
    fn a_window_covered_by_another_is_had_by_going_behind_and_round_again() {
        let pixels = (1280, 960);
        let behind = |behind| under((120, 120), &WINDOWS, SHOWN, pixels, behind);
        assert_eq!(behind(1), Some((1, (0, 0, 150, 150))));
        assert_eq!(behind(2), behind(0));
        // Where only one is, going behind it is that one still.
        let alone = under((20, 20), &WINDOWS, SHOWN, pixels, 3);
        assert_eq!(alone.map(|(window, _)| window), Some(1));
        let click = picked(1, ((120, 120), (121, 121)), (&WINDOWS, 1), SHOWN, pixels);
        assert_eq!(
            click,
            Picked::Window {
                screen: 1,
                window: 1
            }
        );
    }

    #[test]
    fn what_is_picked_says_which_screen_it_is_on() {
        let area = WINDOWS[0];
        assert_eq!(Picked::Everything.screen(), None);
        assert_eq!(Picked::Screen(2).screen(), Some(2));
        assert_eq!(
            Picked::Window {
                screen: 1,
                window: 0
            }
            .screen(),
            Some(1)
        );
        assert_eq!(Picked::Region { screen: 3, area }.screen(), Some(3));
    }

    #[test]
    fn a_drag_is_a_click_until_it_has_gone_far_enough_either_way() {
        assert!(click((10, 10), (13, 7)));
        assert!(!click((10, 10), (14, 10)));
        assert!(!click((10, 10), (10, 6)));
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
