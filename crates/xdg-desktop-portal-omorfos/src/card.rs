//! Memory of the graphics card, which a screen being shared is drawn into
//! by the compositor and read from by what shares it, with nothing copied
//! and no pixel touched by either program's processor.
//!
//! Only a real card gets past opening it, and CI runners have none, so the
//! file is left out of the coverage gate and listed in
//! `docs/adr/0002-coverage-policy.md`. It decides nothing: each call says
//! what came of it, or does what it is given to do instead, and what the
//! stream offers and agrees to is decided where it is called. It is tested
//! on a machine with a card.

use std::fs::{self, File};
use std::os::fd::AsFd;
use std::os::unix::fs::MetadataExt;

use omorfos_pipewire::{Made, Memory};
use wayland_client::QueueHandle;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1::Flags, zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
};

/// The layout of memory laid out a row at a time, which anything may map.
const ROWS: u64 = 0;

/// The graphics card the compositor draws with, if this program may use it.
#[derive(Default)]
pub(crate) struct Card {
    device: Option<gbm::Device<File>>,
}

impl std::fmt::Debug for Card {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Card")
            .field("open", &self.device.is_some())
            .finish()
    }
}

impl Card {
    /// The card whose device number is `number`, if there is one and this
    /// program may use it.
    pub(crate) fn open(number: Option<u64>) -> Self {
        let nodes = fs::read_dir("/dev/dri").into_iter().flatten().flatten();
        let mut nodes = nodes.map(|node| node.path());
        let node = nodes.find(|path| {
            let rendering = path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("renderD"));
            let told = fs::metadata(path).ok().map(|told| told.rdev());
            rendering && told.is_some() && told == number
        });
        let file = node.and_then(|node| File::options().read(true).write(true).open(node).ok());
        Self {
            device: file.and_then(|file| gbm::Device::new(file).ok()),
        }
    }

    /// Of `layouts`, those in which memory for a picture `size` large can
    /// be made on the card, in one part: none, without a card.
    pub(crate) fn layouts(&self, size: (u32, u32), layouts: &[u64]) -> Vec<u64> {
        let made = layouts.iter().filter_map(|&layout| {
            let made = self.allocate(size, layout)?;
            (made.plane_count() == 1).then(|| u64::from(made.modifier()))
        });
        made.collect()
    }

    /// Memory for a picture `size` large: on the card, in `layout`, if one
    /// was agreed and it can be had there; or what `otherwise` makes.
    pub(crate) fn make(
        &self,
        size: (u32, u32),
        layout: Option<u64>,
        otherwise: impl FnOnce() -> Option<Made>,
    ) -> Option<Made> {
        let made = layout.and_then(|layout| {
            let made = self.allocate(size, layout)?;
            let file = made.fd_for_plane(0).ok()?;
            let stride = made.stride_for_plane(0);
            Some(Made {
                file,
                offset: made.offset(0),
                stride,
                size: stride * size.1,
                card: true,
                mappable: layout == ROWS,
            })
        });
        match layout {
            Some(_) => made,
            None => otherwise(),
        }
    }

    /// Memory for a picture `size` large on the card, in `layout`.
    fn allocate(&self, (width, height): (u32, u32), layout: u64) -> Option<gbm::BufferObject<()>> {
        let device = self.device.as_ref()?;
        let layouts = std::iter::once(gbm::Modifier::from(layout));
        let usage = gbm::BufferObjectFlags::RENDERING;
        let format = gbm::Format::Xrgb8888;
        device
            .create_buffer_object_with_modifiers2(width, height, format, layouts, usage)
            .ok()
    }
}

/// What the compositor is to know memory by: memory of the card, in the
/// `layout` agreed, through `cards`; or what `otherwise` names it.
pub(crate) fn name<D>(
    (cards, handle): (Option<&ZwpLinuxDmabufV1>, &QueueHandle<D>),
    memory: &Memory,
    ((width, height), layout): ((u32, u32), Option<u64>),
    otherwise: impl FnOnce() -> WlBuffer,
) -> WlBuffer
where
    D: wayland_client::Dispatch<
            wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
            (),
        > + wayland_client::Dispatch<WlBuffer, ()>
        + 'static,
{
    let card = cards.zip(layout).map(|(cards, layout)| {
        let params = cards.create_params(handle, ());
        let high = u32::try_from(layout >> 32).unwrap_or_default();
        let low = u32::try_from(layout & u64::from(u32::MAX)).unwrap_or_default();
        params.add(
            memory.file.as_fd(),
            0,
            memory.offset,
            memory.stride,
            high,
            low,
        );
        let format = gbm::Format::Xrgb8888 as u32;
        let (across, down) = (width.cast_signed(), height.cast_signed());
        let named = params.create_immed(across, down, format, Flags::empty(), handle, ());
        params.destroy();
        named
    });
    match card {
        Some(named) => named,
        None => otherwise(),
    }
}
