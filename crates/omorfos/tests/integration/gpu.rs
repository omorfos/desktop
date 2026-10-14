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

use smithay::backend::allocator::{Format, Fourcc, Modifier};
use smithay::reexports::gbm;

use omorfos::Roles;
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm};
use wayland_protocols::ext::image_copy_capture::v1::client::ext_image_copy_capture_frame_v1::FailureReason;

use crate::common::{Client, Event, HEIGHT, Session, WIDTH};
use crate::windows::{BACKGROUND, GREEN, RED};

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
    crate::common::graphics_card()
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

/// A picture of the first screen asked for in `buffer`, and what came of
/// it once the screen has been drawn.
fn shoot(session: &mut Session, camera: &mut Client, buffer: &WlBuffer) -> Vec<Event> {
    let taking = camera.capture(0, false);
    session.roundtrip(camera);
    camera.take_events();
    let frame = camera.shot(&taking);
    frame.attach_buffer(buffer);
    frame.damage_buffer(0, 0, WIDTH, HEIGHT);
    frame.capture();
    session.roundtrip(camera);
    session.draw();
    session.roundtrip(camera);
    camera.take_events()
}

#[test]
fn a_program_taking_pictures_is_told_it_may_take_them_in_the_memory_of_the_card() {
    let mut session = Session::new();
    session
        .compositor_mut()
        .offer_gpu_buffers(7, drawable())
        .unwrap();
    let mut camera = session.connect_as(Roles::CAPTURE);
    let _taking = camera.capture(0, false);
    session.roundtrip(&mut camera);
    let told = [
        Event::CaptureSize(WIDTH.unsigned_abs(), HEIGHT.unsigned_abs()),
        Event::CaptureFormat(wl_shm::Format::Xrgb8888),
        Event::CaptureCard(7),
        Event::CaptureCardFormat(Fourcc::Xrgb8888 as u32, vec![u64::from(Modifier::Linear)]),
        Event::CaptureKnown,
    ];
    assert_eq!(camera.take_events(), told);

    // What says it is in the card's memory and is not cannot be drawn into.
    let memory = tempfile::tempfile().unwrap();
    memory
        .set_len(u64::try_from(WIDTH * HEIGHT * 4).unwrap())
        .unwrap();
    let format = (Fourcc::Xrgb8888 as u32, u64::from(Modifier::Linear));
    let stride = (WIDTH * 4).unsigned_abs();
    let buffer = camera.gpu_buffer(memory.as_fd(), ((WIDTH, HEIGHT), stride), format);
    let failed = Event::ShotFailed(FailureReason::BufferConstraints);
    assert_eq!(shoot(&mut session, &mut camera, &buffer), [failed]);
}

#[test]
fn a_picture_of_the_screen_is_drawn_straight_into_the_memory_of_the_card() {
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
    let mut app = session.connect();
    session.open(&mut app, "app", (200, 100), RED);
    let mut camera = session.connect_as(Roles::CAPTURE);

    let device = gbm::Device::new(card).unwrap();
    let flags = gbm::BufferObjectFlags::LINEAR | gbm::BufferObjectFlags::RENDERING;
    let (across, down) = (WIDTH.unsigned_abs(), HEIGHT.unsigned_abs());
    let mut picture = device
        .create_buffer_object::<()>(across, down, gbm::Format::Xrgb8888, flags)
        .unwrap();
    let file = picture.fd().unwrap();
    let format = (Fourcc::Xrgb8888 as u32, u64::from(picture.modifier()));
    let buffer = camera.gpu_buffer(file.as_fd(), ((WIDTH, HEIGHT), picture.stride()), format);
    let events = shoot(&mut session, &mut camera, &buffer);
    assert_eq!(events.last(), Some(&Event::ShotReady), "{events:?}");

    // What the card holds is the screen.
    let at = |mapped: &gbm::MappedBufferObject<'_, ()>, (x, y): (usize, usize)| {
        let start = y * mapped.stride() as usize + x * 4;
        let pixel: [u8; 4] = mapped.buffer()[start..start + 4].try_into().unwrap();
        u32::from_le_bytes(pixel) | 0xff00_0000
    };
    let seen = picture
        .map_mut(0, 0, across, down, |mapped| {
            (at(mapped, (100, 80)), at(mapped, (400, 400)))
        })
        .unwrap();
    assert_eq!(seen, (RED, BACKGROUND));
}
