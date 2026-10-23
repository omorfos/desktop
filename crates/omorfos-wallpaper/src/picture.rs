//! Reading a picture from a file: a PNG or a JPEG, turned the way the
//! camera says it was held, and made no larger than a screen needs it.
//!
//! A file is the user's choice, but it may have come from anywhere, so it
//! is read as untrusted: no more of it than a picture may be, and not
//! decoded at all if it says it is larger than a picture may be. The lock
//! screen reads the user's face with it too.

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use omorfos_config::Fit;
use omorfos_ui::Image;
use zune_jpeg::JpegDecoder;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;

/// The largest file that is read, in bytes.
const LARGEST_FILE: u64 = 256 * 1024 * 1024;

/// The most pixels a picture may have along a side: as many as a picture
/// of 16K has, and a little more.
const LARGEST_SIDE: u32 = 16 * 1024;

/// The most pixels a picture may have in memory at once, which is a
/// photograph of 67 million: four bytes of memory for each. A PNG is read
/// a row at a time, and only as many are kept as the screen needs.
const MOST_PIXELS: u64 = 1 << 26;

/// What is said of a picture that decoded to fewer pixels than its size.
const TORN: &str = "its pixels are not all there";

/// Reads the picture in the file at `path`, as much of it as is needed to
/// lay it on a screen `room` pixels large as `fit` says, or says why it
/// cannot.
///
/// # Errors
///
/// Returns why if the file cannot be read, is too large, or is not a PNG
/// or a JPEG picture that can be decoded.
pub fn read(path: &Path, fit: Fit, room: (u32, u32)) -> Result<Image, String> {
    let mut bytes = Vec::new();
    let opened = File::open(path).and_then(|file| file.metadata().map(|about| (file, about)));
    let opened = opened.map_err(|error| error.to_string());
    let read = opened.and_then(|(file, about)| {
        if about.len() > LARGEST_FILE {
            return Err(format!("it is larger than {} MiB", LARGEST_FILE >> 20));
        }
        // One that grows meanwhile is read no further, and is cut short.
        let mut most = file.take(LARGEST_FILE);
        most.read_to_end(&mut bytes)
            .map_err(|error| error.to_string())
    });
    read.and_then(|_| decode(&bytes, fit, room))
}

/// Decodes a picture from memory, by what its first bytes say it is, as
/// much of it as a screen `room` pixels large needs for `fit`.
///
/// # Errors
///
/// Returns why if it is not a PNG or a JPEG, or cannot be decoded.
pub(crate) fn decode(bytes: &[u8], fit: Fit, room: (u32, u32)) -> Result<Image, String> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(bytes, fit, room)
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        jpeg(bytes, fit, room)
    } else {
        Err("it is not a PNG or a JPEG picture".to_owned())
    }
}

/// Whether a picture this many pixels along each side may be read.
fn sides(width: u32, height: u32) -> Result<(), String> {
    if width <= LARGEST_SIDE && height <= LARGEST_SIDE {
        Ok(())
    } else {
        Err(format!(
            "it is {width} by {height} pixels, more than the {LARGEST_SIDE} along a side that \
             a picture may have"
        ))
    }
}

/// Whether a picture this large may be held in memory.
fn held(width: u32, height: u32) -> Result<(), String> {
    if u64::from(width) * u64::from(height) <= MOST_PIXELS {
        Ok(())
    } else {
        Err(format!(
            "{width} by {height} of its pixels would be held at once, more than the \
             {MOST_PIXELS} that may be"
        ))
    }
}

fn png(bytes: &[u8], fit: Fit, room: (u32, u32)) -> Result<Image, String> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    // Every picture comes out as eight bits a sample, whatever it was
    // stored as: grey or colour, with or without an opacity.
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().map_err(|error| error.to_string())?;
    let (width, height) = reader.info().size();
    sides(width, height)?;
    let mut shrunk = Shrunk::new((width, height), fit, room);
    held(shrunk.size.0, shrunk.size.1)?;
    let each = reader.output_color_type().0.samples();
    if reader.info().interlaced {
        // The rows of an interlaced picture come a few of each at a time,
        // so the whole of it is read before any row is whole.
        held(width, height)?;
        let mut samples = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let frame = reader.next_frame(&mut samples);
        let frame = frame.map_err(|error| error.to_string())?;
        let rows = samples.chunks_exact(frame.line_size.max(1));
        rows.take(frame.buffer_size() / frame.line_size.max(1))
            .for_each(|row| shrunk.row(row, each));
    } else {
        while let Some(row) = reader.next_row().map_err(|error| error.to_string())? {
            shrunk.row(row.data(), each);
        }
    }
    shrunk.image()
}

fn jpeg(bytes: &[u8], fit: Fit, room: (u32, u32)) -> Result<Image, String> {
    let side = usize::try_from(LARGEST_SIDE).unwrap_or(usize::MAX);
    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::RGBA)
        .set_max_width(side)
        .set_max_height(side);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|error| error.to_string())?;
    let (width, height) = decoder.dimensions().unwrap_or_default();
    let (width, height) = (
        u32::try_from(width).unwrap_or(u32::MAX),
        u32::try_from(height).unwrap_or(u32::MAX),
    );
    // The decoder refuses one larger along a side than may be itself, and
    // decodes the whole of it, so what is kept of it is no more than this.
    held(width, height)?;
    let turn = decoder.exif().map_or(1, |exif| orientation(exif));
    let pixels = decoder.decode().map_err(|error| error.to_string())?;
    let (width, height, pixels) = oriented((width, height), &pixels, turn);
    let mut shrunk = Shrunk::new((width, height), fit, room);
    let line = usize::try_from(width)
        .unwrap_or(usize::MAX)
        .saturating_mul(4);
    pixels
        .chunks_exact(line.max(1))
        .for_each(|row| shrunk.row(row, 4));
    shrunk.image()
}

/// A picture made smaller as it is read, a row at a time: only what a
/// screen shows of it, and only as many pixels of that as the screen has,
/// or between that and twice as many along each side.
///
/// What `center` and `tile` show is cut out of the picture, a pixel of it
/// to a pixel of the screen. For `fill`, `fit` and `stretch`, each square
/// of `every` by `every` of its pixels is made one, their average. What is
/// left is made the size of the screen when it is laid on it.
struct Shrunk {
    /// Where what is kept starts, and how many of the picture's pixels it
    /// is along each side.
    left: usize,
    top: u32,
    wide: usize,
    tall: u32,
    every: u32,
    /// How large the picture kept is.
    size: (u32, u32),
    /// The rows of the picture read so far.
    seen: u32,
    /// For each pixel of the row being made, its colours each weighted by
    /// its opacity, and the opacity, summed over the pixels it is made of;
    /// and of how many rows of them.
    sums: Vec<[u64; 4]>,
    rows: u32,
    pixels: Vec<u8>,
}

impl Shrunk {
    /// What is to be kept of a picture `width` by `height`, laid as `fit`
    /// says on a screen `room` pixels large.
    fn new((width, height): (u32, u32), fit: Fit, room: (u32, u32)) -> Self {
        let (across, down) = (room.0.max(1), room.1.max(1));
        let (wide, tall) = (width.min(across), height.min(down));
        let (left, top, wide, tall, every) = match fit {
            // The middle, or the top left, of a picture larger than the
            // screen, as `lay::paint` puts it there.
            Fit::Center => ((width - wide) / 2, (height - tall) / 2, wide, tall, 1),
            Fit::Tile => (0, 0, wide, tall, 1),
            // Made smaller only by as much as still covers the screen, or
            // fits on it.
            Fit::Fill | Fit::Stretch => {
                let every = (width / across).min(height / down);
                (0, 0, width, height, every)
            }
            Fit::Fit => {
                let every = (width / across).max(height / down);
                (0, 0, width, height, every)
            }
        };
        let every = every.max(1);
        let size = (wide.div_ceil(every), tall.div_ceil(every));
        let length = |side: u32| usize::try_from(side).unwrap_or(usize::MAX);
        Self {
            left: length(left),
            top,
            wide: length(wide),
            tall,
            every,
            size,
            seen: 0,
            sums: vec![[0; 4]; length(size.0)],
            rows: 0,
            // It grows as it is made: until it is known that it may be kept,
            // no memory is taken for it.
            pixels: Vec::new(),
        }
    }

    /// Takes the next row of the picture, `each` bytes for each pixel: grey,
    /// grey and its opacity, red, green and blue, or those and the opacity.
    fn row(&mut self, row: &[u8], each: usize) {
        // Made for each, so that the pixels are summed without a question
        // asked of each of them.
        match each {
            1 => self.add::<1>(row),
            2 => self.add::<2>(row),
            3 => self.add::<3>(row),
            _ => self.add::<4>(row),
        }
    }

    fn add<const N: usize>(&mut self, row: &[u8]) {
        let y = self.seen;
        self.seen += 1;
        if y < self.top || y - self.top >= self.tall {
            return;
        }
        let (pixels, _) = row.as_chunks::<N>();
        let kept = pixels.get(self.left..).unwrap_or_default();
        let kept = kept.get(..self.wide).unwrap_or(kept);
        let every = usize::try_from(self.every).unwrap_or(usize::MAX);
        for (sum, block) in self.sums.iter_mut().zip(kept.chunks(every)) {
            for pixel in block {
                // Grey is the red, the green and the blue; the opacity is
                // the last of an even number of samples, and without one
                // a pixel is opaque.
                let red = pixel[0];
                let green = pixel.get(1).filter(|_| N >= 3).copied().unwrap_or(red);
                let blue = pixel.get(2).filter(|_| N >= 3).copied().unwrap_or(red);
                let alpha = pixel.get(N - 1).filter(|_| N.is_multiple_of(2)).copied();
                let [red, green, blue, alpha] =
                    [red, green, blue, alpha.unwrap_or(u8::MAX)].map(u64::from);
                sum[0] += red * alpha;
                sum[1] += green * alpha;
                sum[2] += blue * alpha;
                sum[3] += alpha;
            }
        }
        self.rows += 1;
        let last = y - self.top + 1 == self.tall;
        if self.rows == self.every || last {
            self.flush(every);
        }
    }

    /// Makes a row of the picture kept from the rows summed.
    fn flush(&mut self, every: usize) {
        let mut columns = self.wide;
        for sum in &mut self.sums {
            // The last column may be of fewer pixels than the rest.
            let count = u64::try_from(columns.min(every)).unwrap_or(1) * u64::from(self.rows);
            columns = columns.saturating_sub(every);
            let [red, green, blue, alpha] = *sum;
            let colour = |weighted: u64| {
                let colour = (weighted + alpha / 2).checked_div(alpha).unwrap_or(0);
                u8::try_from(colour).unwrap_or(u8::MAX)
            };
            let opacity = (alpha + count / 2) / count.max(1);
            let opacity = u8::try_from(opacity).unwrap_or(u8::MAX);
            self.pixels
                .extend([colour(red), colour(green), colour(blue), opacity]);
            *sum = [0; 4];
        }
        self.rows = 0;
    }

    /// The picture kept, if all of it was read.
    fn image(self) -> Result<Image, String> {
        let image = Image::new(self.size.0, self.size.1, self.pixels);
        image.ok_or(TORN).map_err(str::to_owned)
    }
}

/// Which way the camera says it was held, from the EXIF data of a
/// photograph, as the TIFF standard numbers the ways: 1 as it is, the
/// rest turned or mirrored. 1 if it says nothing that makes sense.
fn orientation(exif: &[u8]) -> u16 {
    let big = match exif.get(..4) {
        Some(b"MM\0*") => true,
        Some(b"II*\0") => false,
        _ => return 1,
    };
    let short = |offset: usize| {
        let bytes = bytes::<2>(exif, offset);
        bytes.map(|bytes| {
            if big {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_le_bytes(bytes)
            }
        })
    };
    let long = |offset: usize| {
        let bytes = bytes::<4>(exif, offset);
        let long = bytes.map(|bytes| {
            if big {
                u32::from_be_bytes(bytes)
            } else {
                u32::from_le_bytes(bytes)
            }
        });
        long.and_then(|long| usize::try_from(long).ok())
    };
    // The first directory of entries, each twelve bytes: a tag, a type, a
    // count and the value. The orientation is tag 0x112.
    let first = long(4);
    let entries = first.and_then(|first| Some((first + 2, short(first)?)));
    let found = entries.and_then(|(start, count)| {
        let places = (0..usize::from(count)).map(|entry| start + entry * 12);
        let mut tags = places.filter_map(|place| Some((short(place)?, place)));
        tags.find(|(tag, _)| *tag == 0x112)
    });
    let turn = found.and_then(|(_, place)| short(place + 8));
    turn.filter(|turn| (1..=8).contains(turn)).unwrap_or(1)
}

/// The `N` bytes at `offset`, if there are that many there.
fn bytes<const N: usize>(all: &[u8], offset: usize) -> Option<[u8; N]> {
    let rest = all.get(offset..);
    rest.and_then(<[u8]>::first_chunk::<N>).copied()
}

/// The pixels of a picture `width` by `height`, turned or mirrored the
/// way `turn` says, with the size it then has.
fn oriented((width, height): (u32, u32), pixels: &[u8], turn: u16) -> (u32, u32, Vec<u8>) {
    let (w, h) = (u64::from(width), u64::from(height));
    // Five to eight turn it a quarter, so that it stands the other way.
    let sideways = turn >= 5;
    let (across, down) = if sideways { (h, w) } else { (w, h) };
    // Where on the picture as stored each pixel of it as shown is.
    let from = |x: u64, y: u64| match turn {
        2 => (w - 1 - x, y),
        3 => (w - 1 - x, h - 1 - y),
        4 => (x, h - 1 - y),
        5 => (y, x),
        6 => (y, h - 1 - x),
        7 => (w - 1 - y, h - 1 - x),
        8 => (w - 1 - y, x),
        _ => (x, y),
    };
    let places = (0..down).flat_map(|y| (0..across).map(move |x| (x, y)));
    let turned = places.flat_map(|(x, y)| {
        let (x, y) = from(x, y);
        let offset = usize::try_from((y * w + x) * 4).unwrap_or(usize::MAX);
        let pixel = pixels.get(offset..offset.saturating_add(4));
        <[u8; 4]>::try_from(pixel.unwrap_or_default()).unwrap_or_default()
    });
    let size = |side: u64| u32::try_from(side).unwrap_or(u32::MAX);
    (size(across), size(down), turned.collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether a picture has its red half on one side.
    type Held = fn(&Image) -> bool;

    /// Sixteen by eight, the left half red and the right half blue, as GIMP
    /// saves it: progressive.
    const RED_BLUE: &[u8] = include_bytes!("../testdata/red-blue.jpg");

    /// Four by two, the left half red and the right half blue, interlaced.
    /// It and the two below were written by hand, the pixels in the order
    /// of Adam7's seven passes.
    const INTERLACED: &[u8] = include_bytes!("../testdata/red-blue-interlaced.png");

    /// PNG pictures that say they are 16384 pixels along each side, grey,
    /// and are not: their pixels are few. One of them is interlaced.
    const LARGE: &[u8] = include_bytes!("../testdata/large.png");
    const LARGE_INTERLACED: &[u8] = include_bytes!("../testdata/large-interlaced.png");

    /// Room for all of any picture, as it is.
    const ALL: (u32, u32) = (u32::MAX, u32::MAX);

    /// The whole of a picture, as it is.
    fn whole(bytes: &[u8]) -> Result<Image, String> {
        decode(bytes, Fit::Center, ALL)
    }

    /// Each pixel of a picture, as it is drawn on nothing.
    fn drawn(image: &Image) -> Vec<u32> {
        let (width, height) = image.size();
        let mut canvas = omorfos_ui::Canvas::new(width, height);
        canvas.draw_unscaled((0, 0), image);
        let canvas = &canvas;
        let row = |y| (0..width).map(move |x| canvas.at(x, y));
        (0..height).flat_map(row).collect()
    }

    /// Whether the pixel at `(x, y)` is mostly red, or mostly blue.
    fn reddish(image: &Image, (x, y): (u32, u32)) -> bool {
        let mut canvas = omorfos_ui::Canvas::new(image.size().0, image.size().1);
        canvas.draw_unscaled((0, 0), image);
        let pixel = canvas.at(x, y);
        let (red, blue) = ((pixel >> 16) & 0xff, pixel & 0xff);
        assert!(red.abs_diff(blue) > 200, "{pixel:08x} is neither");
        red > blue
    }

    /// A PNG of one row of pixels, as `color` and `depth` say they are
    /// stored.
    fn png_of(width: u32, color: png::ColorType, depth: png::BitDepth, row: &[u8]) -> Vec<u8> {
        rows_of(width, 1, color, depth, row)
    }

    /// A PNG of `height` rows of pixels, as `color` and `depth` say they
    /// are stored.
    fn rows_of(
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        rows: &[u8],
    ) -> Vec<u8> {
        let row = rows;
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(row).unwrap();
        writer.finish().unwrap();
        bytes
    }

    /// The JPEG with EXIF data saying it was held `turn`, written in the
    /// byte order `order` names.
    fn held(turn: u16, order: [u8; 4]) -> Vec<u8> {
        let big = &order == b"MM\0*";
        let short = |value: u16| {
            if big {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        let long = |value: u32| {
            if big {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        // A directory of two entries, the orientation second.
        let mut tiff = order.to_vec();
        tiff.extend(long(8));
        tiff.extend(short(2));
        tiff.extend([short(0x100), short(3)].concat());
        tiff.extend(long(1));
        tiff.extend([short(16), short(0)].concat());
        tiff.extend([short(0x112), short(3)].concat());
        tiff.extend(long(1));
        tiff.extend([short(turn), short(0)].concat());
        tiff.extend(long(0));
        let length = u16::try_from(2 + 6 + tiff.len()).unwrap();
        let mut segment = vec![0xff, 0xe1];
        segment.extend(length.to_be_bytes());
        segment.extend(b"Exif\0\0");
        segment.extend(tiff);
        let mut bytes = RED_BLUE[..2].to_vec();
        bytes.extend(segment);
        bytes.extend(&RED_BLUE[2..]);
        bytes
    }

    #[test]
    fn a_jpeg_is_read_as_it_was_drawn() {
        let image = whole(RED_BLUE).unwrap();
        assert_eq!(image.size(), (16, 8));
        assert!(reddish(&image, (1, 4)));
        assert!(!reddish(&image, (14, 4)));
    }

    #[test]
    fn a_photograph_is_turned_the_way_the_camera_was_held() {
        // Where the red half ends up: on the left, the right, the top or
        // the bottom.
        let left = |image: &Image| reddish(image, (0, 0)) && !reddish(image, (15, 7));
        let right = |image: &Image| !reddish(image, (0, 0)) && reddish(image, (15, 7));
        let top = |image: &Image| reddish(image, (0, 0)) && !reddish(image, (7, 15));
        let bottom = |image: &Image| !reddish(image, (0, 0)) && reddish(image, (7, 15));
        let ways: [(u16, (u32, u32), Held); 8] = [
            (1, (16, 8), left),
            (2, (16, 8), right),
            (3, (16, 8), right),
            (4, (16, 8), left),
            (5, (8, 16), top),
            (6, (8, 16), top),
            (7, (8, 16), bottom),
            (8, (8, 16), bottom),
        ];
        for order in [*b"II*\0", *b"MM\0*"] {
            for (turn, size, side) in ways {
                let image = whole(&held(turn, order)).unwrap();
                assert_eq!(image.size(), size, "{turn}");
                assert!(side(&image), "{turn}");
            }
        }
    }

    #[test]
    fn what_the_camera_says_is_heeded_only_where_it_makes_sense() {
        // A way that is not one, and EXIF data that is not TIFF.
        assert_eq!(whole(&held(9, *b"II*\0")).unwrap().size(), (16, 8));
        assert_eq!(whole(&held(0, *b"II*\0")).unwrap().size(), (16, 8));
        assert_eq!(orientation(b"XX*\0\x08\0\0\0"), 1);
        assert_eq!(orientation(b""), 1);
        assert_eq!(orientation(b"II*\0"), 1, "no directory");
        // A first entry of something else, and a second missing.
        let mut cut = b"II*\0\x08\0\0\0\x02\0".to_vec();
        cut.extend([0x00, 0x01, 3, 0, 1, 0, 0, 0, 16, 0, 0, 0]);
        assert_eq!(orientation(&cut), 1);
        // A directory past the end, or entries that run off it.
        assert_eq!(orientation(b"II*\0\xff\0\0\0"), 1);
        assert_eq!(orientation(b"II*\0\x08\0\0\0\x05\0\x12\x01"), 1);
        assert_eq!(
            orientation(b"MM\0*\0\0\0\x08\0\x01\x01\x12\0\x03\0\0\0\x01\0"),
            1
        );
    }

    #[test]
    fn a_png_is_read_whatever_it_is_stored_as() {
        let grey = png_of(
            2,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[0, 255],
        );
        let rgb = png_of(
            1,
            png::ColorType::Rgb,
            png::BitDepth::Sixteen,
            &[255, 0, 0, 0, 0, 0],
        );
        let clear = png_of(
            1,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            &[0, 0, 255, 0],
        );
        // Grey, with an opacity: white, half there.
        let faint = png_of(
            1,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            &[255, 128],
        );
        assert_eq!(drawn(&whole(&faint).unwrap()), [0x8080_8080]);
        let mut canvas = omorfos_ui::Canvas::new(2, 1);
        canvas.draw_unscaled((0, 0), &whole(&grey).unwrap());
        assert_eq!(
            (canvas.at(0, 0), canvas.at(1, 0)),
            (0xff00_0000, 0xffff_ffff)
        );
        let mut canvas = omorfos_ui::Canvas::new(1, 1);
        canvas.draw_unscaled((0, 0), &whole(&rgb).unwrap());
        assert_eq!(canvas.at(0, 0), 0xffff_0000);
        let mut canvas = omorfos_ui::Canvas::new(1, 1);
        canvas.draw_unscaled((0, 0), &whole(&clear).unwrap());
        assert_eq!(canvas.at(0, 0), 0, "as clear as it was");
    }

    #[test]
    fn a_picture_larger_than_a_picture_may_be_is_not_decoded() {
        // A PNG too wide.
        let row = vec![0; 20_000];
        let wide = png_of(
            20_000,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &row,
        );
        let error = whole(&wide).unwrap_err();
        assert!(error.contains("20000 by 1 pixels"), "{error}");
        // A JPEG that says it is as large along each side as may be, which
        // is more in all than may be; and one that says it is larger still,
        // which the decoder refuses itself. Nothing checks the size it
        // says it is but these.
        let sized = |width: u16, height: u16| {
            let mut bytes = RED_BLUE.to_vec();
            // The picture is progressive, as photographs on the web often
            // are, and its frame says so.
            let frame = bytes
                .windows(2)
                .position(|pair| pair == [0xff, 0xc2])
                .unwrap();
            bytes[frame + 5..frame + 7].copy_from_slice(&height.to_be_bytes());
            bytes[frame + 7..frame + 9].copy_from_slice(&width.to_be_bytes());
            bytes
        };
        let error = whole(&sized(16_000, 16_000)).unwrap_err();
        assert!(error.contains("16000 by 16000"), "{error}");
        assert!(whole(&sized(20_000, 8)).is_err());
    }

    #[test]
    fn what_is_not_a_picture_says_so() {
        let error = whole(b"GIF89a").unwrap_err();
        assert_eq!(error, "it is not a PNG or a JPEG picture");
        assert!(whole(&RED_BLUE[..40]).is_err(), "a JPEG cut short");
        // Cut short after all it says of itself, which is read, and in the
        // first scan of its pixels, which cannot be. Cut in a later one, a
        // progressive picture is still there, if less sharp.
        let error = whole(&RED_BLUE[..250]).unwrap_err();
        assert!(!error.contains("pixels, more than"), "{error}");
        let one = png_of(1, png::ColorType::Rgba, png::BitDepth::Eight, &[0; 4]);
        assert!(whole(&one[..20]).is_err(), "a PNG cut short");
        assert!(
            whole(&one[..one.len() - 20]).is_err(),
            "its pixels cut short"
        );
    }

    #[test]
    fn a_picture_is_read_from_its_file_unless_it_is_too_large_or_not_there() {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().join("red-blue.jpg");
        std::fs::write(&path, RED_BLUE).unwrap();
        assert_eq!(read(&path, Fit::Center, ALL).unwrap().size(), (16, 8));
        assert!(read(&directory.path().join("none.jpg"), Fit::Center, ALL).is_err());
        // A file that is too large is not read at all.
        let large = directory.path().join("large.jpg");
        let file = File::create(&large).unwrap();
        file.set_len(LARGEST_FILE + 1).unwrap();
        let error = read(&large, Fit::Center, ALL).unwrap_err();
        assert!(error.contains("larger than 256 MiB"), "{error}");
        // Nor is a directory, which cannot be read at all.
        assert!(read(directory.path(), Fit::Center, ALL).is_err());
    }

    const RED: u32 = 0xffff_0000;
    const BLUE: u32 = 0xff00_00ff;
    const GREEN: u32 = 0xff00_ff00;

    /// Eight by four, red on the left half and blue on the right, but for
    /// its top left pixel, which is green.
    fn halves() -> Vec<u8> {
        let mut rows = Vec::new();
        for y in 0..4 {
            for x in 0..8 {
                let pixel = match (x, y) {
                    (0, 0) => [0, 255, 0],
                    (0..4, _) => [255, 0, 0],
                    _ => [0, 0, 255],
                };
                rows.extend(pixel);
            }
        }
        rows_of(8, 4, png::ColorType::Rgb, png::BitDepth::Eight, &rows)
    }

    #[test]
    fn a_picture_is_kept_only_as_large_as_the_screen_needs_it() {
        let halves = halves();
        // Each four by four is made one: still enough to cover, or fit on,
        // a screen of two by one.
        for fit in [Fit::Fill, Fit::Fit, Fit::Stretch] {
            let image = decode(&halves, fit, (2, 1)).unwrap();
            assert_eq!(image.size(), (2, 1), "{fit:?}");
        }
        // To cover a screen three by two it is made half as large, and so
        // it is to fit on one three by three, but no smaller: it is laid on
        // that as three wide.
        assert_eq!(decode(&halves, Fit::Fill, (3, 2)).unwrap().size(), (4, 2));
        assert_eq!(decode(&halves, Fit::Fill, (3, 3)).unwrap().size(), (8, 4));
        assert_eq!(decode(&halves, Fit::Fit, (3, 3)).unwrap().size(), (4, 2));
        assert_eq!(decode(&halves, Fit::Fit, (1, 1)).unwrap().size(), (1, 1));
        // On a screen larger than it, it is as it is.
        let image = decode(&halves, Fit::Stretch, (100, 100)).unwrap();
        assert_eq!(image.size(), (8, 4));
        assert_eq!(drawn(&image)[..2], [GREEN, RED]);
    }

    #[test]
    fn a_picture_made_smaller_is_the_average_of_the_pixels_it_is_made_of() {
        let halves = halves();
        let image = decode(&halves, Fit::Fill, (4, 2)).unwrap();
        assert_eq!(image.size(), (4, 2));
        let pixels = drawn(&image);
        // A quarter of the first green, and the rest red.
        assert_eq!(pixels[0], 0xffbf_4000);
        assert_eq!(pixels[1..], [RED, BLUE, BLUE, RED, RED, BLUE, BLUE]);
        // Five wide, the last column is made of fewer than the rest.
        let rows = [
            [255, 0, 0],
            [255, 0, 0],
            [0, 0, 255],
            [0, 0, 255],
            [0, 255, 0],
        ];
        let rows = [rows, rows].concat().concat();
        let five = rows_of(5, 2, png::ColorType::Rgb, png::BitDepth::Eight, &rows);
        let image = decode(&five, Fit::Fill, (2, 1)).unwrap();
        assert_eq!(drawn(&image), [RED, BLUE, GREEN]);
    }

    #[test]
    fn a_clear_pixel_adds_nothing_to_the_colour_of_one_it_is_made_into() {
        let pixels = [
            [255, 0, 0, 255],
            [0, 0, 255, 0],
            [0, 255, 0, 0],
            [0, 0, 255, 0],
        ];
        let rows = [pixels, pixels].concat();
        let rgba = rows_of(
            4,
            2,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            rows.as_flattened(),
        );
        let image = decode(&rgba, Fit::Fill, (2, 1)).unwrap();
        // Red, half as opaque, and nothing at all.
        assert_eq!(drawn(&image), [0x8080_0000, 0]);
    }

    #[test]
    fn a_centred_or_tiled_picture_is_kept_only_where_the_screen_shows_it() {
        let halves = halves();
        // The middle two by two: red and blue.
        let middle = decode(&halves, Fit::Center, (2, 2)).unwrap();
        assert_eq!(drawn(&middle), [RED, BLUE, RED, BLUE]);
        // The top left: green and red.
        let corner = decode(&halves, Fit::Tile, (2, 2)).unwrap();
        assert_eq!(drawn(&corner), [GREEN, RED, RED, RED]);
        // Of a screen wider than it, as tall as the screen is.
        let band = decode(&halves, Fit::Center, (20, 2)).unwrap();
        assert_eq!(band.size(), (8, 2));
    }

    #[test]
    fn an_interlaced_png_is_read_whole() {
        let image = whole(INTERLACED).unwrap();
        assert_eq!(drawn(&image), [RED, RED, BLUE, BLUE, RED, RED, BLUE, BLUE]);
        let image = decode(INTERLACED, Fit::Fill, (2, 1)).unwrap();
        assert_eq!(drawn(&image), [RED, BLUE]);
        assert!(whole(&INTERLACED[..INTERLACED.len() - 20]).is_err());
    }

    #[test]
    fn a_picture_of_16k_is_read_if_no_more_of_it_is_held_than_may_be() {
        // All of it is needed to fill a screen as large, which is more
        // than may be held.
        let error = decode(LARGE, Fit::Fill, (16384, 16384)).unwrap_err();
        assert!(error.contains("16384 by 16384 of its pixels"), "{error}");
        // For a smaller screen it is read, and found not to be all there.
        let error = decode(LARGE, Fit::Fill, (1920, 1080)).unwrap_err();
        assert!(!error.contains("held"), "{error}");
        // Nor is it read if it is interlaced, which is read whole.
        let error = decode(LARGE_INTERLACED, Fit::Fill, (1920, 1080)).unwrap_err();
        assert!(error.contains("16384 by 16384 of its pixels"), "{error}");
    }
}
