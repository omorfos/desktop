//! Laying a picture on a screen, as the configuration says to.

use omorfos_config::{Color, Fit};
use omorfos_ui::{Canvas, Image, Scale};

/// The wallpaper of a screen `size` units of layout, `scale` pixels to a
/// unit: `color`, with `picture` on it as `fit` says.
#[must_use]
pub(crate) fn paint(
    picture: &Image,
    fit: Fit,
    color: Color,
    size: (u32, u32),
    scale: Scale,
) -> Canvas {
    let mut canvas = Canvas::scaled(size.0, size.1, scale);
    canvas.fill((0, 0, size.0, size.1), color);
    // Laid down in pixels: the picture's own pixels are the screen's,
    // whatever the screen's scale.
    let (across, down) = (canvas.width(), canvas.height());
    let (wide, tall) = picture.size();
    match fit {
        Fit::Stretch => canvas.draw_stretched((0, 0, across, down), picture),
        Fit::Fill | Fit::Fit => {
            // Whether the screen is wider for its height than the picture.
            let wider = u64::from(across) * u64::from(tall) > u64::from(down) * u64::from(wide);
            // To fill a wider screen the picture is made as wide as it,
            // and to fit on one as tall.
            let (along, other) = if wider == (fit == Fit::Fill) {
                (across, sized(tall, across, wide))
            } else {
                (sized(wide, down, tall), down)
            };
            let area = (middle(across, along), middle(down, other), along, other);
            canvas.draw_stretched(area, picture);
        }
        Fit::Center => {
            let corner = (middle(across, wide), middle(down, tall));
            canvas.draw_unscaled(corner, picture);
        }
        Fit::Tile => {
            let step = |side: u32| usize::try_from(side).unwrap_or(usize::MAX);
            for y in (0..down).step_by(step(tall)) {
                for x in (0..across).step_by(step(wide)) {
                    canvas.draw_unscaled((x.cast_signed(), y.cast_signed()), picture);
                }
            }
        }
    }
    canvas
}

/// `side` made larger or smaller by `to` over `from`, to the nearest pixel.
fn sized(side: u32, to: u32, from: u32) -> u32 {
    let sized = (u64::from(side) * u64::from(to) + u64::from(from) / 2) / u64::from(from);
    u32::try_from(sized).unwrap_or(u32::MAX)
}

/// Where something `length` long starts for its middle to be at that of
/// `room`: before it starts, if it is longer.
fn middle(room: u32, length: u32) -> i32 {
    let start = (i64::from(room) - i64::from(length)) / 2;
    i32::try_from(start).unwrap_or(i32::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const BLUE: Color = Color::rgb(0, 0, 255);
    const GREEN: Color = Color::rgb(0, 255, 0);

    fn argb(color: Color) -> u32 {
        u32::from_be_bytes([color.alpha, color.red, color.green, color.blue])
    }

    /// A picture `width` by `height` of `color`, but for its top left
    /// pixel, which is `corner`.
    fn picture(width: u32, height: u32, color: Color, corner: Color) -> Image {
        let count = usize::try_from(width * height).unwrap();
        let mut pixels = [color.red, color.green, color.blue, 255].repeat(count);
        pixels[..4].copy_from_slice(&[corner.red, corner.green, corner.blue, 255]);
        Image::new(width, height, pixels).unwrap()
    }

    /// Each row of the canvas, as `#` where it is `color` and `.` where it
    /// is not.
    fn rows(canvas: &Canvas, color: Color) -> Vec<String> {
        let row = |y| (0..canvas.width()).map(move |x| canvas.at(x, y));
        let marked = |y| row(y).map(|pixel| if pixel == argb(color) { '#' } else { '.' });
        (0..canvas.height()).map(|y| marked(y).collect()).collect()
    }

    #[test]
    fn a_stretched_picture_is_the_shape_of_the_screen() {
        let half = Image::new(2, 1, [[255, 0, 0, 255], [0, 0, 255, 255]].concat()).unwrap();
        let canvas = paint(&half, Fit::Stretch, GREEN, (8, 2), Scale::ONE);
        // Blended between the middles of its two pixels, and each alone
        // nearer the edge than that.
        assert_eq!(rows(&canvas, RED), ["##......", "##......"]);
        assert_eq!(rows(&canvas, BLUE), ["......##", "......##"]);
    }

    #[test]
    fn a_picture_filling_the_screen_covers_it_and_one_fitting_on_it_does_not() {
        let square = picture(2, 2, RED, RED);
        // On a wide screen and on a tall one.
        for size in [(6, 2), (2, 6)] {
            let filled = paint(&square, Fit::Fill, BLUE, size, Scale::ONE);
            let whole = rows(&filled, RED);
            assert!(whole.iter().all(|row| !row.contains('.')), "{whole:?}");
        }
        let wide = paint(&square, Fit::Fit, BLUE, (6, 2), Scale::ONE);
        assert_eq!(rows(&wide, RED), ["..##..", "..##.."]);
        assert_eq!(rows(&wide, BLUE), ["##..##", "##..##"]);
        let tall = paint(&square, Fit::Fit, BLUE, (2, 6), Scale::ONE);
        assert_eq!(rows(&tall, RED), ["..", "..", "##", "##", "..", ".."]);
    }

    #[test]
    fn a_picture_filling_the_screen_is_cut_off_equally_on_two_sides() {
        // Green in the top left corner of a wide picture, filling a square
        // screen: as tall as it, and its sides cut off, the green with it.
        let wide = picture(8, 2, RED, GREEN);
        let filled = paint(&wide, Fit::Fill, BLUE, (2, 2), Scale::ONE);
        assert_eq!(rows(&filled, RED), ["##", "##"]);
        let fitted = paint(&wide, Fit::Fit, BLUE, (8, 8), Scale::ONE);
        assert_eq!(rows(&fitted, GREEN)[3], "#.......");
    }

    #[test]
    fn a_centred_picture_is_as_it_is_whatever_the_scale() {
        let small = picture(2, 2, RED, GREEN);
        // Four by four units at a scale of two are eight by eight pixels.
        let canvas = paint(&small, Fit::Center, BLUE, (4, 4), Scale::new(2.0));
        assert_eq!(rows(&canvas, BLUE)[0], "########");
        assert_eq!(rows(&canvas, GREEN)[3], "...#....");
        assert_eq!(rows(&canvas, RED)[4], "...##...");
        // Larger than the screen, its middle is in the middle.
        let large = picture(6, 6, RED, GREEN);
        let canvas = paint(&large, Fit::Center, BLUE, (4, 4), Scale::ONE);
        assert_eq!(rows(&canvas, RED), ["####"; 4]);
    }

    #[test]
    fn a_tiled_picture_is_laid_from_the_top_left_again_and_again() {
        let tile = picture(2, 2, RED, GREEN);
        let canvas = paint(&tile, Fit::Tile, BLUE, (5, 3), Scale::ONE);
        assert_eq!(rows(&canvas, GREEN), ["#.#.#", ".....", "#.#.#"]);
        assert_eq!(rows(&canvas, BLUE), [".....", ".....", "....."]);
    }

    #[test]
    fn what_a_picture_does_not_cover_is_the_colour() {
        let clear = Image::new(1, 1, vec![0, 255, 0, 0]).unwrap();
        let canvas = paint(&clear, Fit::Stretch, BLUE, (2, 2), Scale::ONE);
        assert_eq!(rows(&canvas, BLUE), ["##", "##"]);
    }
}
