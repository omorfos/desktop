//! Writes pixels into memory a Wayland client shares with the compositor.
//!
//! Smithay hands a client's shared memory over only as a pointer, and Rust
//! has no way to write through a pointer that is not `unsafe`. So nothing
//! is written through it: the pixels go into the file the memory is mapped
//! from, which is the same memory. The file is found by where Smithay has
//! it mapped, and opened again through the descriptor Smithay keeps of it.
//! Why, and what was given up for it, is in
//! `docs/adr/0061-written-through-the-file.md`.
//!
//! What is offered is safe to call with anything: every row is checked to
//! lie inside the memory, and the file to be as long, before it is written.

use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::Path;

use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_shm::Format;
use smithay::reexports::wayland_server::{Resource, Weak};
use smithay::wayland::shm::{BufferData, with_buffer_contents};

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

/// The files behind the buffers pictures have been written into, kept so
/// as not to be looked for again for every picture.
///
/// A buffer is always in the same pool, and a pool in the same file, even
/// when it is made larger, so the file found for a buffer is right for as
/// long as the buffer lasts. They are known by the buffer, never by where
/// its memory is: once a pool is let go of, another, perhaps of another
/// program, can be put at the same place, and a picture of the screen
/// would go to a program that did not ask for it.
#[derive(Debug, Default)]
pub struct Files {
    known: Vec<(Weak<WlBuffer>, File)>,
}

impl Files {
    /// Writes a picture into `buffer`, a row at a time from the top.
    ///
    /// `pixels` holds the rows one after another with nothing between
    /// them, four bytes to a pixel. Returns whether it was written: it is
    /// not if the buffer is not shared memory, is not `width` by `height`
    /// pixels of `format`, does not lie inside the memory it says it is
    /// in, or if `pixels` is not exactly a picture of that size; nor if
    /// the file behind it cannot be written, or has been made shorter
    /// than the picture.
    #[must_use]
    pub fn write(
        &mut self,
        buffer: &WlBuffer,
        format: Format,
        size: (usize, usize),
        pixels: &[u8],
    ) -> bool {
        self.write_part(buffer, format, size, (0, 0, size.0, size.1), pixels)
    }

    /// Writes part of a picture into `buffer`: the rectangle `part`, as
    /// `(x, y, width, height)` in pixels from the top left corner of a
    /// picture of `size`, whose pixels are `pixels`, a row at a time from
    /// the top.
    ///
    /// What else is in the buffer is left as it is. Returns whether it was
    /// written: as [`Files::write`], and not if the part does not lie
    /// inside the picture or `pixels` is not exactly that part.
    #[must_use]
    pub fn write_part(
        &mut self,
        buffer: &WlBuffer,
        format: Format,
        size: (usize, usize),
        part: (usize, usize, usize, usize),
        pixels: &[u8],
    ) -> bool {
        // Only where the memory is, as a number: it is never read or
        // written through.
        let mapped = with_buffer_contents(buffer, |memory, len, data| (memory.addr(), len, data));
        let placed = mapped.ok().and_then(|(base, len, data)| {
            let rows = place(&data, len, format, size);
            let rows = rows.and_then(|rows| inside(rows, size, part));
            let rows = rows.filter(|rows| rows.row.checked_mul(part.3) == Some(pixels.len()));
            rows.map(|rows| (base, len, rows))
        });
        placed.is_some_and(|(base, len, rows)| {
            let file = self.file(buffer, base, len);
            file.is_some_and(|file| written(file, rows, pixels))
        })
    }

    /// The file behind `buffer`, whose memory is `len` bytes at `base`:
    /// the one found before, or else the one mapped there now.
    fn file(&mut self, buffer: &WlBuffer, base: usize, len: usize) -> Option<&File> {
        // Files of buffers let go of are closed, so that what a program
        // has let go of is not kept for it.
        self.known.retain(|(known, _)| known.is_alive());
        let id = buffer.id();
        let at = self.known.iter().position(|(known, _)| known.id() == id);
        let at = at.or_else(|| {
            file_mapped_at(base, len).map(|file| {
                self.known.push((buffer.downgrade(), file));
                self.known.len() - 1
            })
        });
        at.and_then(|at| self.known.get(at)).map(|(_, file)| file)
    }
}

/// Writes `pixels` into `file` at `rows`. Nothing, and false, if the file
/// is shorter than the last of them: the program can have made it shorter
/// than its memory, and writing would make it longer again.
fn written(file: &File, rows: Rows, pixels: &[u8]) -> bool {
    let count = pixels.len() / rows.row;
    // Inside the memory, as `place` and `inside` have checked, none of
    // this can overflow.
    let end = rows.start + rows.stride * (count - 1) + rows.row;
    let long_enough = file
        .metadata()
        .is_ok_and(|meta| u64::try_from(end).is_ok_and(|end| meta.len() >= end));
    // Rows with nothing between them in the file go in at once.
    let step = if rows.stride == rows.row {
        pixels.len()
    } else {
        rows.row
    };
    long_enough
        && pixels
            .chunks_exact(step)
            .enumerate()
            .all(|(index, rows_of)| {
                let at = rows.start + index * rows.stride;
                u64::try_from(at).is_ok_and(|at| file.write_all_at(rows_of, at).is_ok())
            })
}

/// The file mapped, whole and shared, at `base` for at least `len` bytes,
/// opened again for writing through the descriptor of it that Smithay
/// keeps open to make its pool larger.
fn file_mapped_at(base: usize, len: usize) -> Option<File> {
    let maps = File::open("/proc/self/maps").ok();
    let lines = maps
        .into_iter()
        .flat_map(|maps| BufReader::new(maps).lines().map_while(Result::ok));
    // The lines are in the order of where they are.
    let mut before = lines.take_while(|line| start(line).is_none_or(|start| start <= base));
    let mapped = before.find_map(|line| mapping(&line, base, len));
    let fds = mapped.and_then(|_| fs::read_dir("/proc/self/fd").ok());
    mapped.zip(fds).and_then(|(file, fds)| {
        fds.map_while(Result::ok)
            .find_map(|fd| opened(&fd.path(), file))
    })
}

/// `path`, a descriptor in `/proc/self/fd`, opened again for writing if
/// it is of `file`, a device and an inode.
fn opened(path: &Path, file: ((u64, u64), u64)) -> Option<File> {
    let of = |meta: fs::Metadata| ((major(meta.dev()), minor(meta.dev())), meta.ino()) == file;
    // Looked at before it is opened, as opening some files does
    // something; and again after, as the number may have been closed and
    // given to another file in between, by another thread.
    let same = fs::metadata(path).is_ok_and(of);
    let opened = same.then(|| File::options().write(true).open(path).ok());
    opened
        .flatten()
        .filter(|opened| opened.metadata().is_ok_and(of))
}

/// Where the memory a line of `/proc/self/maps` is about starts.
fn start(line: &str) -> Option<usize> {
    let (start, _) = line.split_once('-')?;
    usize::from_str_radix(start, 16).ok()
}

/// The device and inode of the file a line of `/proc/self/maps` says is
/// mapped shared and writable from its start, at `base` for at least
/// `len` bytes.
fn mapping(line: &str, base: usize, len: usize) -> Option<((u64, u64), u64)> {
    let mut fields = line.split_ascii_whitespace();
    let (start, end) = fields.next()?.split_once('-')?;
    let start = usize::from_str_radix(start, 16).ok()?;
    let end = usize::from_str_radix(end, 16).ok()?;
    let mode = fields.next()?;
    let shared = mode.starts_with("rw") && mode.ends_with('s');
    let offset = u64::from_str_radix(fields.next()?, 16).ok()?;
    let (major, minor) = fields.next()?.split_once(':')?;
    let device = (
        u64::from_str_radix(major, 16).ok()?,
        u64::from_str_radix(minor, 16).ok()?,
    );
    let inode = fields.next()?.parse().ok()?;
    let whole = start == base && end.checked_sub(start).is_some_and(|long| long >= len);
    (whole && shared && offset == 0 && inode != 0).then_some((device, inode))
}

/// The major number of a device, taken apart as the kernel puts it
/// together for `stat`.
fn major(dev: u64) -> u64 {
    ((dev & 0x0000_0000_000f_ff00) >> 8) | ((dev & 0xffff_f000_0000_0000) >> 32)
}

/// The minor number of a device, taken apart the same way.
fn minor(dev: u64) -> u64 {
    (dev & 0x0000_0000_0000_00ff) | ((dev & 0x0000_0fff_fff0_0000) >> 12)
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

    /// A line of `/proc/self/maps` of 64 KiB shared from a file on device
    /// `00:01` with inode 2051, at `0x7f00_0000_0000`.
    const SHARED: &str = "7f0000000000-7f0000010000 rw-s 00000000 00:01 2051 /memfd:shm (deleted)";
    const BASE: usize = 0x7f00_0000_0000;

    #[test]
    fn the_file_shared_whole_at_a_place_is_known_by_its_device_and_inode() {
        assert_eq!(mapping(SHARED, BASE, 0x1_0000), Some(((0, 1), 2051)));
        assert_eq!(mapping(SHARED, BASE, 16), Some(((0, 1), 2051)), "less");
        let device = "7f0000000000-7f0000010000 rw-s 00000000 fd:1a 7 /home/a";
        assert_eq!(mapping(device, BASE, 16), Some(((0xfd, 0x1a), 7)));
        assert_eq!(start(SHARED), Some(BASE));
    }

    #[test]
    fn memory_that_is_not_all_of_a_file_shared_and_writable_where_asked_is_not_it() {
        let not = |line: &str| assert_eq!(mapping(line, BASE, 16), None, "{line}");
        assert_eq!(mapping(SHARED, BASE + 4096, 16), None, "elsewhere");
        assert_eq!(mapping(SHARED, BASE, 0x1_0001), None, "too short");
        not("7f0000000000-7f0000010000 rw-p 00000000 00:01 2051 /a");
        not("7f0000000000-7f0000010000 r--s 00000000 00:01 2051 /a");
        not("7f0000000000-7f0000010000 rw-s 00001000 00:01 2051 /a");
        not("7f0000000000-7f0000010000 rw-s 00000000 00:00 0");
        not("7f0000000000-7f0000000000 rw-s 00000000 00:01 2051 /a");
        not("7f0000010000-7f0000000000 rw-s 00000000 00:01 2051 /a");
    }

    #[test]
    fn a_line_that_is_not_as_the_kernel_writes_it_is_of_no_file() {
        let not = |line: &str| assert_eq!(mapping(line, BASE, 16), None, "{line}");
        not("");
        not("7f0000000000");
        not("x-7f0000010000 rw-s 00000000 00:01 2051");
        not("7f0000000000-x rw-s 00000000 00:01 2051");
        not("7f0000000000-7f0000010000");
        not("7f0000000000-7f0000010000 rw-s");
        not("7f0000000000-7f0000010000 rw-s x 00:01 2051");
        not("7f0000000000-7f0000010000 rw-s 00000000");
        not("7f0000000000-7f0000010000 rw-s 00000000 0001 2051");
        not("7f0000000000-7f0000010000 rw-s 00000000 x:01 2051");
        not("7f0000000000-7f0000010000 rw-s 00000000 00:x 2051");
        not("7f0000000000-7f0000010000 rw-s 00000000 00:01");
        not("7f0000000000-7f0000010000 rw-s 00000000 00:01 x");
        assert_eq!(start("7f0000000000"), None);
        assert_eq!(start("x-7f0000010000"), None);
    }

    #[test]
    fn a_device_is_taken_apart_as_the_kernel_puts_it_together() {
        // `makedev(0xfd, 0x1a)`, and `makedev(0x1234, 0x56789)`, whose
        // numbers do not fit in the old twelve and eight bits.
        assert_eq!((major(0xfd1a), minor(0xfd1a)), (0xfd, 0x1a));
        let large = 0x1000_5672_3489;
        assert_eq!((major(large), minor(large)), (0x1234, 0x56789));
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
