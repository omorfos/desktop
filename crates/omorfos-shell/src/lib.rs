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
pub mod said;
pub mod shake;
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
/// Returns an error if the memory cannot be had or written to.
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
/// [`Canvas::bytes`] has them.
///
/// # Errors
///
/// Returns an error if the memory cannot be had or written to.
pub fn pixels<S>(
    shm: &wl_shm::WlShm,
    (width, height): (u32, u32),
    bytes: &[u8],
    handle: &QueueHandle<S>,
) -> io::Result<wl_buffer::WlBuffer>
where
    S: Dispatch<wl_shm_pool::WlShmPool, ()> + Dispatch<wl_buffer::WlBuffer, ()> + 'static,
{
    let memory = memfd_create("omorfos-picture", MemfdFlags::CLOEXEC);
    let memory = memory.map(File::from).map_err(io::Error::from);
    let written = memory.and_then(|mut memory| memory.write_all(bytes).map(|()| memory));
    written.map(|memory| {
        let length = i32::try_from(bytes.len()).unwrap_or(i32::MAX);
        let (width, height) = (width.cast_signed(), height.cast_signed());
        let pool = shm.create_pool(memory.as_fd(), length, handle, ());
        let format = wl_shm::Format::Argb8888;
        let buffer = pool.create_buffer(0, width, height, width * 4, format, handle, ());
        pool.destroy();
        buffer
    })
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
    fn a_position_falls_in_the_unit_it_is_within() {
        assert_eq!(whole(0.0), 0);
        assert_eq!(whole(3.99), 3);
        assert_eq!(whole(-0.5), -1);
        assert_eq!(whole(1e300), i32::MAX);
    }
}
