//! What the shell components share as Wayland clients.
//!
//! The panel, the launcher and the notifications are each a program of
//! their own, and each draws on a canvas and hands the picture to the
//! compositor. The handing over is the same for all of them and is here.
//! It is exercised wherever a component is tested against a compositor.
//! So is [`said`], what a helper that asks PAM says.

use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

pub mod clock;
pub mod ended;
pub mod month;
pub mod popover;
pub mod said;
pub mod shake;
pub mod sheet;
pub mod tooltip;
pub mod typing;

use omorfos_ui::Canvas;
use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool};
use wayland_client::{Dispatch, QueueHandle};

/// Hands a canvas to the compositor as a buffer it can read.
///
/// The pixels are written to memory that is shared with the compositor by
/// passing it the descriptor; nothing is mapped, so nothing here is
/// `unsafe`. The buffer is the caller's to destroy, which it does when the
/// compositor says it has finished with it.
///
/// # Errors
///
/// Returns an error if the canvas is empty or too large for Wayland, or if
/// the memory cannot be had or written to.
pub fn picture<S>(
    shm: &wl_shm::WlShm,
    canvas: &Canvas,
    handle: &QueueHandle<S>,
) -> io::Result<wl_buffer::WlBuffer>
where
    S: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
{
    let size = (canvas.width(), canvas.height());
    pixels(shm, size, canvas.bytes(), handle)
}

/// Hands pixels to the compositor as a buffer it can read, as
/// [`picture`] does those of a canvas: `size` of them, four bytes each, as
/// [`Canvas::bytes`] has them, row after row with nothing between, so
/// that there are exactly four for each pixel.
///
/// # Errors
///
/// Returns an error if `size` is empty or too large for Wayland, or the
/// bytes are not exactly four for each pixel, or if the memory cannot be
/// had or written to.
pub fn pixels<S>(
    shm: &wl_shm::WlShm,
    size: (u32, u32),
    bytes: &[u8],
    handle: &QueueHandle<S>,
) -> io::Result<wl_buffer::WlBuffer>
where
    S: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
{
    let Layout {
        width,
        height,
        stride,
        length,
    } = checked_layout(size, bytes.len())?;
    let memory = memfd_create("omorfos-picture", MemfdFlags::CLOEXEC);
    let memory = memory.map(File::from).map_err(io::Error::from);
    let written = memory.and_then(|mut memory| memory.write_all(bytes).map(|()| memory));
    written.map(|memory| {
        let pool = shm.create_pool(memory.as_fd(), length, handle, ());
        let format = wl_shm::Format::Argb8888;
        let buffer = pool.create_buffer(0, width, height, stride, format, handle, ());
        pool.destroy();
        buffer
    })
}

/// How pixels four bytes each, row after row with nothing between, lie in
/// memory shared with the compositor, in the signed numbers Wayland gives
/// a pool and a buffer: how many there are across and down, how far apart
/// the rows are in bytes, and how many bytes there are in all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// How many pixels there are across.
    pub width: i32,
    /// How many pixels there are down.
    pub height: i32,
    /// How far apart the rows are, in bytes.
    pub stride: i32,
    /// How many bytes there are in all.
    pub length: i32,
}

impl Layout {
    /// How pixels `size` large lie in memory.
    ///
    /// # Errors
    ///
    /// Returns an error if `size` is empty, or too large for any of
    /// Wayland's numbers.
    pub fn of((width, height): (u32, u32)) -> io::Result<Self> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "invalid shared pixel buffer");
        let width = i32::try_from(width).ok().filter(|width| *width > 0);
        let height = i32::try_from(height).ok().filter(|height| *height > 0);
        let sides = width.zip(height);
        let stride = sides.and_then(|(width, _)| width.checked_mul(4));
        let length = stride
            .zip(height)
            .and_then(|(stride, height)| stride.checked_mul(height));
        let layout = sides.zip(stride).zip(length);
        layout
            .map(|(((width, height), stride), length)| Self {
                width,
                height,
                stride,
                length,
            })
            .ok_or_else(invalid)
    }
}

/// How pixels `size` large lie in memory, as [`Layout::of`] says, once it
/// is known that there are exactly `byte_length` bytes of them.
///
/// # Errors
///
/// Returns an error if `size` is empty or too large for Wayland, or the
/// bytes are not exactly four for each pixel.
pub fn checked_layout(size: (u32, u32), byte_length: usize) -> io::Result<Layout> {
    let layout = Layout::of(size)?;
    let fits = usize::try_from(layout.length).is_ok_and(|length| length == byte_length);
    fits.then_some(layout)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid shared pixel buffer"))
}

/// Connects to the compositor: over the connection the compositor made
/// and handed over, if it handed one, or failing that at its socket,
/// `path`, as any program does.
///
/// A component the compositor starts is handed a connection that is known
/// to be its own, which is what lets it do what an ordinary program may
/// not. It is handed over as what the program reads from, and
/// `WAYLAND_SOCKET` says so, in the way every Wayland program understands.
/// Taking a copy of what is read from needs nothing `unsafe`, which
/// taking a descriptor by its number would.
///
/// # Errors
///
/// Returns an error if what was handed over cannot be copied, or the
/// socket cannot be connected to.
pub fn connect(path: &Path) -> io::Result<UnixStream> {
    let handed = env::var_os("WAYLAND_SOCKET").is_some_and(|number| number == "0");
    if handed {
        let ours = io::stdin().as_fd().try_clone_to_owned();
        ours.map(UnixStream::from)
    } else {
        UnixStream::connect(path)
    }
}

/// The whole unit of layout a position on a surface falls in.
#[must_use]
pub fn whole(position: f64) -> i32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "a position on a surface is far within range, and a cast saturates"
    )]
    let whole = position.floor() as i32;
    whole
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pixel_buffer_must_have_nonzero_dimensions() {
        assert_eq!(
            checked_layout((0, 1), 0).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            checked_layout((1, 0), 0).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_pixel_buffer_must_have_exactly_four_bytes_per_pixel() {
        assert_eq!(
            checked_layout((2, 2), 15).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_pixel_buffer_must_fit_waylands_signed_dimensions_and_stride() {
        assert_eq!(
            checked_layout((u32::try_from(i32::MAX).unwrap() + 1, 1), 0)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            checked_layout((1, u32::try_from(i32::MAX).unwrap() + 1), 0)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        let too_wide = u32::try_from(i32::MAX / 4 + 1).unwrap();
        assert_eq!(
            checked_layout((too_wide, 1), 0).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            checked_layout((1, 1), usize::try_from(i32::MAX).unwrap() + 1)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_well_formed_pixel_buffer_has_a_checked_wayland_layout() {
        let layout = Layout {
            width: 2,
            height: 3,
            stride: 8,
            length: 24,
        };
        assert_eq!(checked_layout((2, 3), 24).unwrap(), layout);
        assert_eq!(Layout::of((2, 3)).unwrap(), layout);
    }

    #[test]
    fn pixels_whose_bytes_are_too_many_for_wayland_lie_in_no_layout() {
        assert_eq!(
            Layout::of((1 << 15, 1 << 14)).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn a_position_falls_in_the_unit_it_is_within() {
        assert_eq!(whole(0.0), 0);
        assert_eq!(whole(3.99), 3);
        assert_eq!(whole(-0.5), -1);
        assert_eq!(whole(1e300), i32::MAX);
    }
}
