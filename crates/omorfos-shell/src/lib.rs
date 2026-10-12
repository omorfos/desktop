//! What the shell components share as Wayland clients.
//!
//! The panel, the launcher and the notifications are each a program of
//! their own, and each draws on a canvas and hands the picture to the
//! compositor. The handing over is the same for all of them and is here.
//! It is exercised wherever a component is tested against a compositor.

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsFd;

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
/// Returns an error if the memory cannot be had or written to.
pub fn picture<S>(
    shm: &wl_shm::WlShm,
    canvas: &Canvas,
    handle: &QueueHandle<S>,
) -> io::Result<wl_buffer::WlBuffer>
where
    S: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
{
    let bytes = canvas.bytes();
    let memory = memfd_create("omorfos-picture", MemfdFlags::CLOEXEC);
    let memory = memory.map(File::from).map_err(io::Error::from);
    let written = memory.and_then(|mut memory| memory.write_all(bytes).map(|()| memory));
    written.map(|memory| {
        let length = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
        let (width, height) = (canvas.width().cast_signed(), canvas.height().cast_signed());
        let pool = shm.create_pool(memory.as_fd(), length, handle, ());
        let format = wl_shm::Format::Argb8888;
        let buffer = pool.create_buffer(0, width, height, width * 4, format, handle, ());
        pool.destroy();
        buffer
    })
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
    fn a_position_falls_in_the_unit_it_is_within() {
        assert_eq!(whole(0.0), 0);
        assert_eq!(whole(3.99), 3);
        assert_eq!(whole(-0.5), -1);
        assert_eq!(whole(1e300), i32::MAX);
    }
}
