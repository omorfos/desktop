//! The overview: every window of a workspace laid out small side by side,
//! under a strip of the workspaces of its screen (ADR 76).
//!
//! The windows are in the order they are read in where they stand, by
//! their middles, top to bottom and then left to right, so that the
//! overview reads as the screen did. They are laid out in rows, as many
//! across as shows the most of them, each as large as fits its share of
//! the room, but no larger than it is. The workspaces are along the top,
//! each a small screen, as many as fit.

use crate::WindowId;
use crate::geometry::{Point, Rect, Size};

/// How the overview is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverviewMetrics {
    /// How tall each workspace in the strip is.
    pub strip: u32,
    /// The room between one window and the next, and between the strip
    /// and the windows.
    pub gap: u32,
    /// The least room kept between it all and the screen's edges.
    pub margin: u32,
}

/// Where the overview shows each window, and each workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverviewLayout {
    /// Each window, where it is shown, in the order it is read and walked.
    pub windows: Vec<(WindowId, Rect)>,
    /// Each workspace, in order, where it is shown in the strip.
    pub workspaces: Vec<Rect>,
}

/// A length worked out in fractions, as the nearest whole one.
#[expect(
    clippy::cast_possible_truncation,
    reason = "within a screen, far inside an i32"
)]
fn whole(length: f64) -> i32 {
    length.round() as i32
}

/// A length worked out in fractions, as the nearest whole one, and none
/// below nothing.
fn length(length: f64) -> u32 {
    whole(length).max(0).cast_unsigned()
}

/// How much `size` is made smaller to fit `room`, and never larger.
fn shrink(size: Size, room: (f64, f64)) -> f64 {
    let across = room.0 / f64::from(size.width.max(1));
    let down = room.1 / f64::from(size.height.max(1));
    across.min(down).clamp(0.0, 1.0)
}

/// The middle of `rect`, twice over, so that it is whole.
fn middle(rect: Rect) -> (i64, i64) {
    let x = 2 * i64::from(rect.origin.x) + i64::from(rect.size.width);
    let y = 2 * i64::from(rect.origin.y) + i64::from(rect.size.height);
    (y, x)
}

/// Lays out the overview on `screen`: `workspaces` in the strip along its
/// top, and `windows`, each where it stands, under it.
#[must_use]
pub fn overview(
    screen: Rect,
    windows: &[(WindowId, Rect)],
    workspaces: usize,
    metrics: OverviewMetrics,
) -> OverviewLayout {
    let OverviewMetrics { strip, gap, margin } = metrics;
    let (gap, margin) = (f64::from(gap), f64::from(margin));
    let (left, top) = (f64::from(screen.origin.x), f64::from(screen.origin.y));
    let (width, height) = (f64::from(screen.size.width), f64::from(screen.size.height));
    // The strip, each workspace the shape of the screen, smaller still if
    // they would not fit across.
    let count = f64::from(u32::try_from(workspaces).unwrap_or(u32::MAX));
    let tall = f64::from(strip);
    let wide = tall * width / height.max(1.0);
    let room = 2.0f64.mul_add(-margin, width);
    let gaps = (count - 1.0).max(0.0) * gap;
    let fit = ((room - gaps) / (count * wide).max(1.0)).clamp(0.0, 1.0);
    let (wide, tall) = (wide * fit, tall * fit);
    let across = count.mul_add(wide, (count - 1.0).max(0.0) * gap);
    let start = left + (width - across) / 2.0;
    let places = (0..workspaces).map(|at| {
        let at = f64::from(u32::try_from(at).unwrap_or(0));
        let x = at.mul_add(wide + gap, start);
        Rect::new(
            Point::new(whole(x), whole(top + margin)),
            Size::new(length(wide), length(tall)),
        )
    });
    let workspaces = places.collect();
    // The windows under it, in the order they are read.
    let mut sorted = windows.to_vec();
    sorted.sort_by_key(|(_, rect)| middle(*rect));
    let room = (
        left + margin,
        top + margin + tall + gap,
        2.0f64.mul_add(-margin, width),
        height - 2.0f64.mul_add(margin, tall + gap),
    );
    OverviewLayout {
        windows: rows(&sorted, room, gap),
        workspaces,
    }
}

/// `windows` laid out in rows in `room`, as many across as shows the most
/// of them, `gap` apart, the rows and each row in the middle.
fn rows(
    windows: &[(WindowId, Rect)],
    room: (f64, f64, f64, f64),
    gap: f64,
) -> Vec<(WindowId, Rect)> {
    let (left, top, width, height) = room;
    let count = windows.len();
    let cell = |across: usize| {
        let down = count.div_ceil(across.max(1));
        let (across, down) = (
            f64::from(u32::try_from(across).unwrap_or(1)),
            f64::from(u32::try_from(down).unwrap_or(1)),
        );
        let cell = (
            (across - 1.0).mul_add(-gap, width) / across,
            (down - 1.0).mul_add(-gap, height) / down,
        );
        (cell.0.max(1.0), cell.1.max(1.0))
    };
    // How much of the windows is seen with so many across.
    let seen = |across: usize| {
        let room = cell(across);
        let shown = windows.iter().map(|(_, rect)| {
            let factor = shrink(rect.size, room);
            f64::from(rect.size.width) * f64::from(rect.size.height) * factor * factor
        });
        shown.sum::<f64>()
    };
    let best = (1..=count.max(1)).fold((1, f64::MIN), |(best, most), across| {
        let shown = seen(across);
        if shown > most {
            (across, shown)
        } else {
            (best, most)
        }
    });
    let across = best.0;
    let room = cell(across);
    let lines: Vec<&[(WindowId, Rect)]> = windows.chunks(across).collect();
    let down = f64::from(u32::try_from(lines.len()).unwrap_or(1));
    let block = down.mul_add(room.1, (down - 1.0).max(0.0) * gap);
    let mut y = top + (height - block) / 2.0;
    let mut laid = Vec::with_capacity(count);
    for line in lines {
        let sizes: Vec<(f64, f64)> = line
            .iter()
            .map(|(_, rect)| {
                let factor = shrink(rect.size, room);
                (
                    f64::from(rect.size.width) * factor,
                    f64::from(rect.size.height) * factor,
                )
            })
            .collect();
        let many = f64::from(u32::try_from(line.len()).unwrap_or(1));
        let wide = sizes.iter().map(|(wide, _)| wide).sum::<f64>() + (many - 1.0) * gap;
        let mut x = left + (width - wide) / 2.0;
        for ((window, _), (wide, tall)) in line.iter().zip(sizes) {
            let at = Point::new(whole(x), whole(y + (room.1 - tall) / 2.0));
            laid.push((
                *window,
                Rect::new(at, Size::new(length(wide), length(tall))),
            ));
            x += wide + gap;
        }
        y += room.1 + gap;
    }
    laid
}

#[cfg(test)]
mod tests {
    use super::*;

    const METRICS: OverviewMetrics = OverviewMetrics {
        strip: 100,
        gap: 20,
        margin: 40,
    };

    const SCREEN: Rect = Rect::new(Point::new(1000, 0), Size::new(1600, 1000));

    fn window(id: u64, (x, y): (i32, i32), (width, height): (u32, u32)) -> (WindowId, Rect) {
        (
            WindowId::from_raw(id),
            Rect::new(Point::new(1000 + x, y), Size::new(width, height)),
        )
    }

    /// The room the windows are laid out in under the strip.
    fn room() -> Rect {
        Rect::new(Point::new(1040, 160), Size::new(1520, 800))
    }

    fn inside(rect: Rect, room: Rect) -> bool {
        rect.origin.x >= room.origin.x
            && rect.origin.y >= room.origin.y
            && rect.right() <= room.right()
            && rect.bottom() <= room.bottom()
    }

    fn overlap(one: Rect, other: Rect) -> bool {
        i64::from(one.origin.x) < other.right()
            && i64::from(other.origin.x) < one.right()
            && i64::from(one.origin.y) < other.bottom()
            && i64::from(other.origin.y) < one.bottom()
    }

    #[test]
    fn a_window_smaller_than_the_room_keeps_its_size_in_the_middle_of_it() {
        let layout = overview(SCREEN, &[window(1, (0, 0), (400, 300))], 1, METRICS);
        let middle = Point::new(1040 + (1520 - 400) / 2, 160 + (800 - 300) / 2);
        assert_eq!(
            layout.windows,
            [(
                WindowId::from_raw(1),
                Rect::new(middle, Size::new(400, 300))
            )]
        );
    }

    #[test]
    fn two_large_windows_stand_side_by_side_made_small_to_fit() {
        let windows = [
            window(1, (800, 0), (1600, 1000)),
            window(2, (0, 0), (1600, 1000)),
        ];
        let layout = overview(SCREEN, &windows, 1, METRICS);
        let (one, two) = (layout.windows[0], layout.windows[1]);
        assert_eq!(one.0, WindowId::from_raw(2), "the one on the left first");
        assert_eq!(one.1.origin.y, two.1.origin.y, "on one row");
        assert!(one.1.right() < i64::from(two.1.origin.x), "{one:?} {two:?}");
        assert_eq!(
            one.1.size,
            Size::new(750, 469),
            "half the room, its shape kept"
        );
    }

    #[test]
    fn many_windows_are_laid_out_in_rows_in_the_order_they_are_read_and_none_overlap() {
        let windows = [
            window(5, (900, 600), (800, 400)),
            window(1, (0, 0), (600, 500)),
            window(4, (100, 550), (700, 400)),
            window(2, (700, 50), (500, 500)),
            window(3, (1300, 100), (300, 900)),
        ];
        let layout = overview(SCREEN, &windows, 1, METRICS);
        let order: Vec<u64> = layout.windows.iter().map(|(id, _)| id.raw()).collect();
        assert_eq!(order, [1, 2, 3, 4, 5]);
        for (at, (_, one)) in layout.windows.iter().enumerate() {
            assert!(inside(*one, room()), "{one:?}");
            for (_, other) in &layout.windows[at + 1..] {
                assert!(!overlap(*one, *other), "{one:?} {other:?}");
            }
        }
        let rows: std::collections::BTreeSet<i32> = layout
            .windows
            .iter()
            .map(|(_, rect)| rect.origin.y + rect.size.height.cast_signed() / 2)
            .collect();
        assert!(rows.len() > 1, "more than one row");
    }

    #[test]
    fn the_workspaces_stand_along_the_top_each_the_shape_of_the_screen() {
        let layout = overview(SCREEN, &[], 3, METRICS);
        assert_eq!(layout.windows, []);
        let wide = 160;
        let across = 3 * wide + 2 * 20;
        let left = 1000 + (1600 - across) / 2;
        let expected: Vec<Rect> = (0..3)
            .map(|at| Rect::new(Point::new(left + at * (wide + 20), 40), Size::new(160, 100)))
            .collect();
        assert_eq!(layout.workspaces, expected);
        // So many that they would not fit are made smaller, and do.
        let layout = overview(SCREEN, &[], 32, METRICS);
        let last = layout.workspaces[31];
        assert!(last.right() <= 1000 + 1600 - 40, "{last:?}");
        assert!(layout.workspaces[0].origin.x >= 1000 + 40);
        assert!(last.size.height < 100);
    }
}
