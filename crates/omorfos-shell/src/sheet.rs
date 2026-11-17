//! A sheet: a surface a picture is put on, which covers as much of the
//! screen as it is to whatever its pixels, as each component's surfaces
//! are, drawn at the scale of the screen and shown at the size of layout.
//!
//! A component makes the surface and its viewport, gives them the role it
//! wants, and puts each picture it paints with [`Sheet::put`]. The picture
//! before is let go of when the compositor says it has finished with it,
//! as the component's handling of its buffers' events does.

use std::io;

use omorfos_ui::Canvas;
use wayland_client::protocol::{wl_buffer, wl_shm, wl_shm_pool, wl_surface};
use wayland_client::{Dispatch, QueueHandle};
use wayland_protocols::wp::viewporter::client::wp_viewport;

use crate::picture;

/// A surface, its viewport, and the picture on it.
#[derive(Debug)]
pub struct Sheet {
    surface: wl_surface::WlSurface,
    viewport: wp_viewport::WpViewport,
    /// The picture on it, which the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
}

impl Sheet {
    /// A sheet of `surface`, sized by `viewport`, with nothing on it yet.
    #[must_use]
    pub const fn new(surface: wl_surface::WlSurface, viewport: wp_viewport::WpViewport) -> Self {
        Self {
            surface,
            viewport,
            buffer: None,
        }
    }

    /// The surface, to give a role, to ask a frame of, or to commit.
    #[must_use]
    pub const fn surface(&self) -> &wl_surface::WlSurface {
        &self.surface
    }

    /// Whether nothing has been put on it yet.
    #[must_use]
    pub const fn blank(&self) -> bool {
        self.buffer.is_none()
    }

    /// Puts `canvas` on the sheet, to cover `size` in units of layout
    /// however many pixels it has, and commits it.
    ///
    /// # Errors
    ///
    /// Returns an error if the picture cannot be handed to the compositor;
    /// what is on the sheet stays.
    pub fn put<S>(
        &mut self,
        shm: &wl_shm::WlShm,
        canvas: &Canvas,
        (width, height): (u32, u32),
        handle: &QueueHandle<S>,
    ) -> io::Result<()>
    where
        S: Dispatch<wl_buffer::WlBuffer, ()> + Dispatch<wl_shm_pool::WlShmPool, ()> + 'static,
    {
        picture(shm, canvas, handle).map(|buffer| self.show(buffer, (width, height)))
    }

    /// Puts `buffer`, a picture made otherwise than from a canvas, on the
    /// sheet, to cover `size` in units of layout, and commits it.
    pub fn show(&mut self, buffer: wl_buffer::WlBuffer, (width, height): (u32, u32)) {
        self.surface.attach(Some(&buffer), 0, 0);
        self.viewport
            .set_destination(width.cast_signed(), height.cast_signed());
        self.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        self.surface.commit();
        self.buffer = Some(buffer);
    }

    /// Destroys the picture on it, its viewport and the surface, which is
    /// to have had its role destroyed first.
    pub fn destroy(self) {
        self.buffer.iter().for_each(wl_buffer::WlBuffer::destroy);
        self.viewport.destroy();
        self.surface.destroy();
    }
}
