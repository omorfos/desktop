//! Writes pixels into memory a Wayland client shares with the compositor.
//!
//! This is the one place in omorfos that uses `unsafe`, and it is a crate
//! of its own so that every other crate can go on forbidding it. Smithay
//! hands a client's shared memory over as a pointer, and Rust has no way
//! to write through a pointer that is not `unsafe`. Why that was accepted,
//! and what would let it be taken out again, is in
//! `docs/adr/0040-one-unsafe-block.md`.
//!
//! What is offered is safe to call with anything: every row is checked to
//! lie inside the memory before it is written.

use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm::Format;
use smithay::wayland::shm::{BufferData, with_buffer_contents_mut};

/// The bytes one pixel takes in the formats this writes.
const PIXEL: usize = 4;

/// Where the rows of a picture go in memory that is `len` bytes long: the
/// start of the first, and how far each is from the one before. Nothing,
/// unless the buffer is `width` by `height` pixels of `format` and every
/// row of it lies inside the memory.
fn place(data: &BufferData, len: usize, format: Format, size: (usize, usize)) -> Option<Rows> {
    let (width, height) = size;
    let row = width.saturating_mul(PIXEL);
    let fits = data.format == format
        && usize::try_from(data.width).ok()? == width
        && usize::try_from(data.height).ok()? == height
        && height > 0;
    let start = usize::try_from(data.offset).ok()?;
    let stride = usize::try_from(data.stride).ok()?;
    // The last row ends where the picture does. A sum too large to hold
    // stops at the largest number there is, which no memory is as long
    // as: nothing mapped is longer than `isize::MAX`.
    let end = stride
        .saturating_mul(height.saturating_sub(1))
        .saturating_add(start)
        .saturating_add(row);
    let len = len.min(isize::MAX.unsigned_abs());
    (fits && stride >= row && end <= len).then_some(Rows { start, stride, row })
}

/// Where rows go: see [`place`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Rows {
    start: usize,
    stride: usize,
    row: usize,
}

/// Writes a picture into `buffer`, a row at a time from the top.
///
/// `pixels` holds the rows one after another with nothing between them,
/// four bytes to a pixel. Returns whether it was written: it is not if
/// the buffer is not shared memory, is not `width` by `height` pixels of
/// `format`, does not lie inside the memory it says it is in, or if
/// `pixels` is not exactly a picture of that size.
#[must_use]
pub fn write(buffer: &WlBuffer, format: Format, size: (usize, usize), pixels: &[u8]) -> bool {
    write_part(buffer, format, size, (0, 0, size.0, size.1), pixels)
}

/// Writes part of a picture into `buffer`: the rectangle `part`, as `(x,
/// y, width, height)` in pixels from the top left corner of a picture of
/// `size`, whose pixels are `pixels`, a row at a time from the top.
///
/// What else is in the buffer is left as it is. Returns whether it was
/// written: as [`write()`], and not if the part does not lie inside the
/// picture or `pixels` is not exactly that part.
#[must_use]
pub fn write_part(
    buffer: &WlBuffer,
    format: Format,
    size: (usize, usize),
    part: (usize, usize, usize, usize),
    pixels: &[u8],
) -> bool {
    let written = with_buffer_contents_mut(buffer, |memory, len, data| {
        let rows = place(&data, len, format, size);
        let rows = rows.and_then(|rows| inside(rows, size, part));
        let rows = rows.filter(|rows| rows.row.checked_mul(part.3) == Some(pixels.len()));
        rows.is_some_and(|rows| {
            for (index, row) in pixels.chunks_exact(rows.row).enumerate() {
                let at = rows.start + index * rows.stride;
                // TODO: have Smithay do this copy, and forbid `unsafe` here
                // as everywhere else. See docs/adr/0040-one-unsafe-block.md.
                #[expect(unsafe_code, reason = "see docs/adr/0040-one-unsafe-block.md")]
                // SAFETY: `memory` is valid for `len` bytes while this
                // closure runs, which Smithay promises, and guards against
                // the client making it shorter meanwhile. `place` has
                // checked that the picture's last row ends at or before
                // `len`, and `inside` that the part lies within the
                // picture, so `at + row.len()` is inside it for every row.
                // `row` is ours and cannot overlap memory mapped from the
                // client's file. The client may write at the same time: no
                // reference to its memory is made, only bytes copied, so
                // the worst is a wrong picture in a buffer that is the
                // client's own.
                unsafe {
                    std::ptr::copy_nonoverlapping(row.as_ptr(), memory.wrapping_add(at), row.len());
                }
            }
            true
        })
    });
    written.unwrap_or(false)
}

/// Where the rows of a part of a picture go, given where the picture's
/// go: the start of its first, as far apart, and as long as it is wide.
/// Nothing, unless the part lies inside the picture and is not empty.
fn inside(
    rows: Rows,
    (width, height): (usize, usize),
    part: (usize, usize, usize, usize),
) -> Option<Rows> {
    let (x, y, across, down) = part;
    let fits = x.checked_add(across).is_some_and(|right| right <= width)
        && y.checked_add(down).is_some_and(|bottom| bottom <= height)
        && across > 0
        && down > 0;
    // Inside the picture, which lies inside the memory, none of this can
    // overflow.
    fits.then(|| Rows {
        start: rows.start + y * rows.stride + x * PIXEL,
        stride: rows.stride,
        row: across * PIXEL,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const XRGB: Format = Format::Xrgb8888;

    fn buffer(offset: i32, (width, height): (i32, i32), stride: i32) -> BufferData {
        BufferData {
            offset,
            width,
            height,
            stride,
            format: XRGB,
        }
    }

    #[test]
    fn rows_go_where_the_buffer_says_when_all_of_it_is_inside_the_memory() {
        let rows = place(&buffer(16, (2, 3), 12), 16 + 12 * 2 + 8, XRGB, (2, 3));
        let expected = Rows {
            start: 16,
            stride: 12,
            row: 8,
        };
        assert_eq!(rows, Some(expected));
    }

    #[test]
    fn a_buffer_that_reaches_past_the_memory_is_not_written_to() {
        // One byte short for the last row.
        assert_eq!(
            place(&buffer(16, (2, 3), 12), 16 + 12 * 2 + 7, XRGB, (2, 3)),
            None
        );
        assert_eq!(place(&buffer(0, (2, 3), 8), 0, XRGB, (2, 3)), None);
        // A size too large to hold is refused, not wrapped round, even
        // by memory that says it is as long as can be.
        let wide = (usize::MAX, 3);
        assert_eq!(place(&buffer(0, (2, 3), 8), usize::MAX, XRGB, wide), None);
        let far = buffer(i32::MAX, (2, 3), i32::MAX);
        assert_eq!(place(&far, 24, XRGB, (2, 3)), None);
    }

    #[test]
    fn a_part_goes_where_it_is_in_the_picture_if_it_lies_inside_it() {
        let rows = Rows {
            start: 16,
            stride: 40,
            row: 32,
        };
        let part = inside(rows, (8, 4), (2, 1, 3, 2));
        let expected = Rows {
            start: 16 + 40 + 8,
            stride: 40,
            row: 12,
        };
        assert_eq!(part, Some(expected));
        assert_eq!(inside(rows, (8, 4), (0, 0, 8, 4)), Some(rows), "all of it");
        assert_eq!(inside(rows, (8, 4), (6, 0, 3, 1)), None, "past the right");
        assert_eq!(inside(rows, (8, 4), (0, 3, 1, 2)), None, "past the bottom");
        assert_eq!(inside(rows, (8, 4), (usize::MAX, 0, 2, 1)), None, "too far");
        assert_eq!(
            inside(rows, (8, 4), (0, usize::MAX, 1, 2)),
            None,
            "too far down"
        );
        assert_eq!(inside(rows, (8, 4), (1, 1, 0, 1)), None, "nothing across");
        assert_eq!(inside(rows, (8, 4), (1, 1, 1, 0)), None, "nothing down");
    }

    #[test]
    fn a_buffer_of_another_shape_is_not_written_to() {
        let plain = buffer(0, (2, 3), 8);
        assert!(place(&plain, 24, XRGB, (2, 3)).is_some());
        assert_eq!(place(&plain, 24, Format::Argb8888, (2, 3)), None, "format");
        assert_eq!(place(&plain, 24, XRGB, (3, 2)), None, "size");
        assert_eq!(place(&plain, 24, XRGB, (2, 2)), None, "height");
        assert_eq!(
            place(&buffer(0, (2, 3), 7), 24, XRGB, (2, 3)),
            None,
            "rows overlap"
        );
        assert_eq!(
            place(&buffer(-4, (2, 3), 8), 24, XRGB, (2, 3)),
            None,
            "offset"
        );
        assert_eq!(
            place(&buffer(0, (2, 3), -8), 24, XRGB, (2, 3)),
            None,
            "stride"
        );
        assert_eq!(
            place(&buffer(0, (-2, 3), 8), 24, XRGB, (2, 3)),
            None,
            "width"
        );
        assert_eq!(
            place(&buffer(0, (2, -3), 8), 24, XRGB, (2, 3)),
            None,
            "height"
        );
        assert_eq!(
            place(&buffer(0, (0, 0), 0), 24, XRGB, (0, 0)),
            None,
            "nothing"
        );
    }
}
