//! Pictures handed over in the memory of the graphics card: a program that
//! draws with the card hands its pictures over as they are, once the
//! backend says which card and what it can draw.
//!
//! What needs a graphics card is tested where there is one, and passes
//! without being tried where there is none, as in CI. To try it, run the
//! tests on a machine whose user may use `/dev/dri/renderD128`.

use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use smithay::backend::allocator::{Format, Fourcc, Modifier};
use smithay::reexports::gbm;

use crate::common::{Event, Session};
use crate::windows::GREEN;

/// What the software renderer the tests draw with can draw from the memory
/// of a card: pictures laid out a row at a time.
fn drawable() -> [Format; 1] {
    [Format {
        code: Fourcc::Xrgb8888,
        modifier: Modifier::Linear,
    }]
}

/// The graphics card to try with, and its device number, if this machine
/// has one this user may use.
fn card() -> Option<(File, u64)> {
    let path = Path::new("/dev/dri/renderD128");
    let file = File::options().read(true).write(true).open(path).ok();
    let number = std::fs::metadata(path).ok().map(|told| told.rdev());
    file.zip(number)
}

#[test]
fn a_program_may_hand_over_buffers_of_the_graphics_card_once_the_backend_offers_them() {
    let mut session = Session::new();
    let before = session.connect();
    assert!(!before.offers_gpu_buffers(), "not until the backend says");

    session
        .compositor_mut()
        .offer_gpu_buffers(0, drawable())
        .unwrap();
    let mut client = session.connect();
    assert!(client.offers_gpu_buffers());
    // A buffer of a format that is offered is taken.
    let memory = tempfile::tempfile().unwrap();
    memory.set_len(16 * 16 * 4).unwrap();
    let format = (Fourcc::Xrgb8888 as u32, u64::from(Modifier::Linear));
    let _buffer = client.gpu_buffer(memory.as_fd(), ((16, 16), 64), format);
    session.roundtrip(&mut client);
    assert!(!client.take_events().contains(&Event::GpuBufferFailed));
    assert_eq!(client.error(), None);
}

#[test]
fn a_window_drawn_in_the_memory_of_the_graphics_card_is_shown_as_drawn() {
    let Some((card, number)) = card() else {
        let said = "not tried: there is no graphics card this user may use";
        let _ = writeln!(std::io::stderr(), "{said}");
        return;
    };
    let mut session = Session::new();
    session
        .compositor_mut()
        .offer_gpu_buffers(number, drawable())
        .unwrap();
    let mut client = session.connect();
    let window = client.toplevel("drawn", "drawn on the card");
    session.roundtrip(&mut client);

    // A picture drawn into the card's memory, laid out a row at a time.
    let device = gbm::Device::new(card).unwrap();
    let flags = gbm::BufferObjectFlags::LINEAR | gbm::BufferObjectFlags::RENDERING;
    let mut picture = device
        .create_buffer_object::<()>(64, 32, gbm::Format::Xrgb8888, flags)
        .unwrap();
    picture
        .map_mut(0, 0, 64, 32, |mapped| {
            let row = mapped.stride() as usize;
            for line in mapped.buffer_mut().chunks_mut(row) {
                for pixel in line[..64 * 4].as_chunks_mut::<4>().0 {
                    *pixel = GREEN.to_le_bytes();
                }
            }
        })
        .unwrap();
    let file = picture.fd().unwrap();
    let format = (Fourcc::Xrgb8888 as u32, u64::from(picture.modifier()));
    let buffer = client.gpu_buffer(file.as_fd(), ((64, 32), picture.stride()), format);
    window.surface.attach(Some(&buffer), 0, 0);
    window.surface.damage_buffer(0, 0, 64, 32);
    window.surface.commit();
    session.roundtrip(&mut client);
    session.roundtrip(&mut client);
    assert!(!client.take_events().contains(&Event::GpuBufferFailed));
    assert_eq!(
        session.draw().at(10, 10),
        GREEN,
        "what the card holds is drawn"
    );
}
