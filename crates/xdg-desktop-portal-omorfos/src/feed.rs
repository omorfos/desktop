//! Pictures of a screen, taken one after another for as long as it is
//! shared, each as soon as something on it has changed.
//!
//! The compositor puts each picture straight into memory that is shared
//! with what reads it, which is given here a buffer at a time.
//!
//! The pointer may be followed beside them: where it is, and pictures of
//! it by itself, each taken when it looks different.

use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;

use omorfos_pipewire::Memory;
use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::protocol::{
    wl_buffer, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm, wl_shm_pool,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_cursor_session_v1::{
        self as cursor, ExtImageCopyCaptureCursorSessionV1,
    },
    ext_image_copy_capture_frame_v1::{self as frame, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self as session, ExtImageCopyCaptureSessionV1},
};

use wayland_protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
};

use crate::card;
use crate::{Failure, Pointer};

/// What a session or a picture is of: the screen, or the pointer by itself.
/// A picture of the screen is known by the session it was asked in, counted
/// from the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Of {
    Screen(u64),
    Pointer,
}

/// What the compositor has said about the pointer being followed.
#[derive(Debug, Default)]
struct Followed {
    /// Where it is on the screen, in its pixels, while it is over it.
    at: Option<(i32, i32)>,
    /// Where its position is in its picture.
    tip: (i32, i32),
    /// How large its pictures are, once said.
    size: Option<(u32, u32)>,
    /// Whether the picture of it asked for has been taken.
    ready: bool,
}

/// What the compositor has said about the pictures being taken.
#[derive(Debug, Default)]
struct Watching {
    shm: Option<wl_shm::WlShm>,
    sources: Option<ExtOutputImageCaptureSourceManagerV1>,
    capturers: Option<ExtImageCopyCaptureManagerV1>,
    /// The screens, by what they are called once they have said.
    outputs: Vec<(wl_output::WlOutput, String)>,
    seat: Option<wl_seat::WlSeat>,
    /// What memory of the graphics card is named to the compositor by.
    cards: Option<ZwpLinuxDmabufV1>,
    /// The card the compositor can put pictures in the memory of, once it
    /// has said, and the layouts it can draw into.
    card: Option<u64>,
    layouts: Vec<u64>,
    followed: Followed,
    /// How large the pictures are, once said.
    size: Option<(u32, u32)>,
    /// How large they are now, if the screen has changed size since.
    resized: Option<(u32, u32)>,
    /// Whether no more will be given.
    stopped: bool,
    /// Whether the picture asked for has been taken, or could not be, as
    /// while the session is locked.
    ready: bool,
    lost: bool,
    /// How many sessions have been begun: what is said of a picture asked
    /// for in one before the last is not heard.
    begun: u64,
    /// What the compositor says has changed in the picture being taken.
    changed: Vec<Part>,
}

/// Pictures of a screen, taken over a connection to the compositor that is
/// shared with the rest of the program.
#[derive(Debug)]
pub struct Feed {
    queue: EventQueue<Watching>,
    watching: Watching,
    shm: wl_shm::WlShm,
    /// What pictures are taken of, how, and the session they are taken in.
    taking: (
        ExtImageCopyCaptureManagerV1,
        ExtImageCaptureSourceV1,
        Options,
    ),
    session: ExtImageCopyCaptureSessionV1,
    /// The compositor's names for the memory pictures have been put in, kept
    /// so as not to be made for each picture.
    buffers: Vec<Named>,
    /// Which memory the picture asked for is being put in.
    asked: Option<(u64, usize)>,
    size: (u32, u32),
    /// The pointer, if it is followed beside the pictures.
    following: Option<Following>,
}

/// A part of a picture: where its top left corner is, and how wide and high
/// it is, in pixels.
type Part = (i32, i32, i32, i32);

/// The most parts of a picture remembered as out of date in a buffer; more
/// are taken as the one part that holds them all.
const PARTS: usize = 8;

/// Memory the compositor knows by a name, what is known by, and what in it
/// is out of date: what has changed on the screen since a picture was
/// last put in it. The compositor writes only that, and what changes.
#[derive(Debug)]
struct Named {
    place: (u64, usize),
    buffer: wl_buffer::WlBuffer,
    stale: Vec<Part>,
}

/// What is out of date in memory once more of the screen has changed:
/// what was, and what has; as one part, once there would be many.
fn gather(stale: &mut Vec<Part>, changed: &[Part]) {
    stale.extend_from_slice(changed);
    if stale.len() > PARTS {
        let corners = stale
            .iter()
            .map(|&(x, y, wide, high)| (x, y, x + wide, y + high));
        let all = corners.reduce(|(left, top, right, bottom), (x, y, across, down)| {
            (left.min(x), top.min(y), right.max(across), bottom.max(down))
        });
        let all = all.map(|(left, top, right, bottom)| (left, top, right - left, bottom - top));
        stale.clear();
        stale.extend(all);
    }
}

/// The pointer being followed: what it is followed through, and the last
/// picture of it.
#[derive(Debug)]
struct Following {
    cursor: ExtImageCopyCaptureCursorSessionV1,
    session: ExtImageCopyCaptureSessionV1,
    /// Where pictures of it are put, which is memory of this program's own.
    memory: File,
    buffer: wl_buffer::WlBuffer,
    size: (u32, u32),
    picture: Vec<u8>,
    /// How many pictures of it there have been.
    taken: u64,
}

/// The pointer as last seen: where it is on the screen, in its pixels, if
/// it is over it; where its position is in its picture; how large that
/// is; the picture itself, four bytes to a pixel as blue, green, red and
/// alpha, premultiplied; and how many pictures of it there have been, so
/// that a new one is known.
#[derive(Debug, Clone, Copy)]
pub struct Seen<'a> {
    /// Where it is, if it is over the screen.
    pub at: Option<(i32, i32)>,
    /// Where its position is in its picture.
    pub tip: (i32, i32),
    /// How large its picture is.
    pub size: (u32, u32),
    /// Its picture.
    pub picture: &'a [u8],
    /// How many pictures of it there have been.
    pub taken: u64,
}

impl Feed {
    /// Begins taking pictures of the screen called `screen`, or of the
    /// first if none is named, with the pointer drawn in, followed beside
    /// them, or neither.
    ///
    /// # Errors
    ///
    /// Returns an error if the compositor gives this program no pictures,
    /// the screen is not there, or the memory to take them in cannot be
    /// had.
    pub fn new(
        connection: &Connection,
        screen: Option<&str>,
        pointer: Pointer,
    ) -> Result<Self, Failure> {
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        let mut watching = Watching::default();
        connection.display().get_registry(&handle, ());
        // Once to be told what there is, and again for what the screens
        // are called, which each says once it is bound.
        let listed = queue.roundtrip(&mut watching).map_err(Failure::from);
        let named = listed.and_then(|_| queue.roundtrip(&mut watching).map_err(Failure::from));
        let session = named.and_then(|_| {
            let offered = watching.sources.clone().zip(watching.capturers.clone());
            let mut outputs = watching.outputs.iter();
            let output = outputs.find(|(_, called)| screen.is_none_or(|screen| screen == called));
            let offered = offered.zip(output.map(|(output, _)| output.clone()));
            let options = if pointer == Pointer::Drawn {
                Options::PaintCursors
            } else {
                Options::empty()
            };
            let seat = watching.seat.clone().filter(|_| pointer == Pointer::Beside);
            let session = offered.map(|((sources, capturers), output)| {
                let source = sources.create_source(&output, &handle, ());
                let session = capturers.create_session(&source, options, &handle, Of::Screen(0));
                // The pointer is followed over the same screen, by itself.
                let followed = seat.map(|seat| {
                    let pointer = seat.get_pointer(&handle, ());
                    let cursor =
                        capturers.create_pointer_cursor_session(&source, &pointer, &handle, ());
                    let session = cursor.get_capture_session(&handle, Of::Pointer);
                    (cursor, session)
                });
                (session, (capturers, source, options), followed)
            });
            let told = queue.roundtrip(&mut watching).map_err(Failure::from);
            told.map(|_| session)
        });
        let ready = session.and_then(|session| {
            let given = session.zip(watching.size).zip(watching.shm.clone());
            let given = given.filter(|_| !watching.stopped);
            given.ok_or_else(|| "omorfos gives this program no pictures of the screen".into())
        });
        let ready = ready.and_then(|(((session, taking, followed), size), shm)| {
            let following = followed.map(|followed| follow(&queue, &watching, &shm, followed));
            following.transpose().map(|following| Self {
                queue,
                watching,
                shm,
                taking,
                session,
                buffers: Vec::new(),
                asked: None,
                size,
                following,
            })
        });
        ready.inspect(Self::point)
    }

    /// The graphics card the compositor can put pictures in the memory of,
    /// if it has said and they can be named to it, and the layouts it can
    /// draw into.
    #[must_use]
    pub fn card(&self) -> (Option<u64>, Vec<u64>) {
        let watching = &self.watching;
        let card = watching.card.filter(|_| watching.cards.is_some());
        (card, watching.layouts.clone())
    }

    /// The pointer as last seen, if it is followed.
    #[must_use]
    pub fn pointer(&self) -> Option<Seen<'_>> {
        let followed = &self.watching.followed;
        self.following.as_ref().map(|following| Seen {
            at: followed.at,
            tip: followed.tip,
            size: following.size,
            picture: &following.picture,
            taken: following.taken,
        })
    }

    /// Asks for the next picture of the pointer, if it is followed, which
    /// the compositor takes once it looks different.
    fn point(&self) {
        let handle = self.queue.handle();
        if let Some(following) = &self.following {
            let frame = following.session.create_frame(&handle, Of::Pointer);
            let (width, height) = following.size;
            frame.attach_buffer(&following.buffer);
            frame.damage_buffer(0, 0, width.cast_signed(), height.cast_signed());
            frame.capture();
        }
    }

    /// How large the pictures are, in pixels.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        self.size
    }

    /// How large the pictures are now, if the screen has changed size
    /// since this was last asked. The memory named before is let go of:
    /// what is put in from now on is made to the new size.
    pub fn resized(&mut self) -> Option<(u32, u32)> {
        let resized = self.watching.resized.take();
        let _ = resized.inspect(|size| {
            self.size = *size;
            for named in self.buffers.drain(..) {
                named.buffer.destroy();
            }
        });
        resized
    }

    /// Asks for the next picture, to be put in `memory`. The compositor
    /// takes it once something on the screen has changed, and says it
    /// cannot if the memory is too small, which ends the feed.
    ///
    /// Only one is asked for at a time: the one before has been put where
    /// it was to go.
    pub fn ask(&mut self, memory: &Memory, layout: Option<u64>) {
        // Memory handed out before buffers were last taken away is gone,
        // and nothing is being put in it. What reads the stream may well
        // have changed, and is shown the screen at once, though nothing on
        // it changes: the first picture of a session is taken at once.
        let handed = self.buffers.first().map(|named| named.place.0);
        if handed.is_some_and(|handed| handed != memory.place.0) {
            let (capturers, source, options) = &self.taking;
            self.session.destroy();
            self.watching.begun += 1;
            let handle = self.queue.handle();
            let begun = Of::Screen(self.watching.begun);
            self.session = capturers.create_session(source, *options, &handle, begun);
        }
        self.buffers.retain(|named| {
            let kept = named.place.0 == memory.place.0;
            if !kept {
                named.buffer.destroy();
            }
            kept
        });
        let handle = self.queue.handle();
        let known = self
            .buffers
            .iter()
            .position(|named| named.place == memory.place);
        let at = known.unwrap_or_else(|| self.name(memory, layout));
        let frame = self
            .session
            .create_frame(&handle, Of::Screen(self.watching.begun));
        // The compositor is told what is out of date in the memory, and
        // writes that and what has changed: memory new to it, all of it.
        let named = &self.buffers[at];
        frame.attach_buffer(&named.buffer);
        for &(x, y, wide, high) in &named.stale {
            frame.damage_buffer(x, y, wide, high);
        }
        frame.capture();
        self.watching.changed.clear();
        self.asked = Some(memory.place);
    }

    /// Has the compositor know memory by a name, to put pictures in: memory
    /// of the graphics card in `layout`, if one was agreed, or shared
    /// memory. Keeps the name for the next time it is given, with all of it
    /// out of date. Returns where it is kept.
    fn name(&mut self, memory: &Memory, layout: Option<u64>) -> usize {
        let (width, height) = self.size;
        let (across, down) = (width.cast_signed(), height.cast_signed());
        let handle = self.queue.handle();
        let shm = &self.shm;
        let cards = (self.watching.cards.as_ref(), &handle);
        let buffer = card::name(cards, memory, (self.size, layout), || {
            let end = u64::from(memory.offset) + u64::from(memory.size);
            let end = i32::try_from(end).unwrap_or(i32::MAX);
            let pool = shm.create_pool(memory.file.as_fd(), end, &handle, ());
            let offset = memory.offset.cast_signed();
            let format = wl_shm::Format::Xrgb8888;
            let buffer = pool.create_buffer(offset, across, down, across * 4, format, &handle, ());
            pool.destroy();
            buffer
        });
        self.buffers.push(Named {
            place: memory.place,
            buffer,
            stale: vec![(0, 0, across, down)],
        });
        self.buffers.len() - 1
    }

    /// Takes in what the compositor has said, without waiting, and returns
    /// whether the picture asked for has been put where it was to go, or
    /// could not be; or nothing, if it is still to come.
    ///
    /// # Errors
    ///
    /// Returns an error once no more pictures will be given: the screen
    /// has gone, or the connection has.
    pub fn turn(&mut self) -> Result<Option<bool>, Failure> {
        let handled = self.queue.dispatch_pending(&mut self.watching);
        let handled = handled.map_err(Failure::from);
        let over = self.watching.stopped;
        let going = handled.and_then(|_| {
            let said = "the screen being shared has gone";
            (!over).then_some(()).ok_or_else(|| Failure::from(said))
        });
        going.map(|()| {
            // A new picture of the pointer is read, and the next asked for.
            let followed = std::mem::take(&mut self.watching.followed.ready);
            let following = self.following.as_mut().filter(|_| followed);
            if let Some(following) = following {
                // What cannot be read is shown as it was.
                let _ = following.memory.read_exact_at(&mut following.picture, 0);
                following.taken += 1;
            }
            if followed {
                self.point();
            }
            let (ready, lost) = (&mut self.watching.ready, &mut self.watching.lost);
            let put = std::mem::take(ready).then_some(true);
            let over = put.or_else(|| std::mem::take(lost).then_some(false));
            // A picture put in memory leaves nothing in it out of date, and
            // what changed for it is out of date in all the rest.
            let asked = self.asked.take_if(|_| over.is_some());
            let filled = asked.filter(|_| over == Some(true));
            let changed = &self.watching.changed;
            if let Some(filled) = filled {
                for named in &mut self.buffers {
                    if named.place == filled {
                        named.stale.clear();
                    } else {
                        gather(&mut named.stale, changed);
                    }
                }
            }
            over
        })
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.session.destroy();
        self.taking.1.destroy();
        if let Some(following) = &self.following {
            following.session.destroy();
            following.cursor.destroy();
            following.buffer.destroy();
        }
        for named in &self.buffers {
            named.buffer.destroy();
        }
    }
}

impl Dispatch<wl_output::WlOutput, ()> for Watching {
    fn event(
        watching: &mut Self,
        output: &wl_output::WlOutput,
        event: wl_output::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            let known = watching.outputs.iter_mut();
            for (_, called) in known.filter(|(known, _)| known == output) {
                called.clone_from(&name);
            }
        }
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for Watching {
    fn event(
        watching: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        // A registry also says when something goes, which ends the session
        // of a screen that went, as the session itself says.
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_shm" => watching.shm = Some(registry.bind(name, 1, handle, ())),
                "ext_output_image_capture_source_manager_v1" => {
                    watching.sources = Some(registry.bind(name, 1, handle, ()));
                }
                "ext_image_copy_capture_manager_v1" => {
                    watching.capturers = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_output" => {
                    let output = registry.bind(name, version.min(4), handle, ());
                    watching.outputs.push((output, String::new()));
                }
                "zwp_linux_dmabuf_v1" => {
                    watching.cards = Some(registry.bind(name, 3, handle, ()));
                }
                "wl_seat" if watching.seat.is_none() => {
                    watching.seat = Some(registry.bind(name, 1, handle, ()));
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, Of> for Watching {
    fn event(
        watching: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: session::Event,
        of: &Of,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match (event, of) {
            // Pictures of the pointer are as large as any pointer, and stay
            // so; it is followed for as long as the screen is.
            (session::Event::BufferSize { width, height }, Of::Pointer) => {
                watching.followed.size = Some((width, height));
            }
            (session::Event::BufferSize { width, height }, Of::Screen(_)) => {
                // A size said again, and not as it was, is a screen that has
                // changed size, which the pictures follow.
                let said = (width, height);
                let changed = watching.size.is_some_and(|size| size != said);
                watching.resized = changed.then_some(said).or(watching.resized);
                watching.size = Some(said);
            }
            (session::Event::DmabufDevice { device }, Of::Screen(_)) => {
                let number = device.try_into().ok().map(u64::from_ne_bytes);
                watching.card = number;
            }
            (session::Event::DmabufFormat { modifiers, .. }, Of::Screen(_)) => {
                let layouts = modifiers.as_chunks::<8>().0.iter();
                watching.layouts = layouts.map(|layout| u64::from_ne_bytes(*layout)).collect();
            }
            (session::Event::Stopped, _) => watching.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureCursorSessionV1, ()> for Watching {
    fn event(
        watching: &mut Self,
        _: &ExtImageCopyCaptureCursorSessionV1,
        event: cursor::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let followed = &mut watching.followed;
        match event {
            cursor::Event::Leave => followed.at = None,
            cursor::Event::Position { x, y } => followed.at = Some((x, y)),
            cursor::Event::Hotspot { x, y } => followed.tip = (x, y),
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, Of> for Watching {
    fn event(
        watching: &mut Self,
        frame: &ExtImageCopyCaptureFrameV1,
        event: frame::Event,
        of: &Of,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let current = *of == Of::Screen(watching.begun);
        match event {
            frame::Event::Damage {
                x,
                y,
                width,
                height,
            } if current => watching.changed.push((x, y, width, height)),
            frame::Event::Ready => {
                watching.ready |= current;
                watching.followed.ready |= *of == Of::Pointer;
                frame.destroy();
            }
            // A picture that could not be taken because what it is of has
            // gone comes with the session saying it has stopped.
            frame::Event::Failed { .. } => {
                watching.lost |= current;
                frame.destroy();
            }
            _ => {}
        }
    }
}

/// Readies what pictures of the pointer are put in, once the compositor
/// has said how large they are: memory of this program's own.
fn follow(
    queue: &EventQueue<Watching>,
    watching: &Watching,
    shm: &wl_shm::WlShm,
    (cursor, session): (
        ExtImageCopyCaptureCursorSessionV1,
        ExtImageCopyCaptureSessionV1,
    ),
) -> Result<Following, Failure> {
    let size = watching.followed.size.unwrap_or_default();
    let (width, height) = size;
    let length = u64::from(width) * u64::from(height) * 4;
    let made = memfd_create("omorfos-pointer", MemfdFlags::CLOEXEC);
    let made = made.map(File::from).map_err(io::Error::from);
    let memory = made.and_then(|file| file.set_len(length).map(|()| file));
    memory.map_err(Failure::from).map(|memory| {
        let handle = queue.handle();
        let bytes = i32::try_from(length).unwrap_or(i32::MAX);
        let pool = shm.create_pool(memory.as_fd(), bytes, &handle, ());
        let (across, down) = (width.cast_signed(), height.cast_signed());
        let format = wl_shm::Format::Argb8888;
        let buffer = pool.create_buffer(0, across, down, across * 4, format, &handle, ());
        pool.destroy();
        Following {
            cursor,
            session,
            memory,
            buffer,
            size,
            picture: vec![0; usize::try_from(length).unwrap_or_default()],
            taken: 0,
        }
    })
}

delegate_noop!(Watching: ignore wl_shm::WlShm);
delegate_noop!(Watching: ignore wl_seat::WlSeat);
delegate_noop!(Watching: ignore ZwpLinuxDmabufV1);
delegate_noop!(Watching: ignore ZwpLinuxBufferParamsV1);
delegate_noop!(Watching: ignore wl_pointer::WlPointer);
delegate_noop!(Watching: wl_shm_pool::WlShmPool);
delegate_noop!(Watching: ignore wl_buffer::WlBuffer);
delegate_noop!(Watching: ExtOutputImageCaptureSourceManagerV1);
delegate_noop!(Watching: ExtImageCaptureSourceV1);
delegate_noop!(Watching: ExtImageCopyCaptureManagerV1);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_out_of_date_in_memory_is_gathered_into_one_part_once_there_are_many() {
        let mut stale = vec![(0, 0, 2, 2)];
        gather(&mut stale, &[(10, 10, 4, 4)]);
        assert_eq!(stale, [(0, 0, 2, 2), (10, 10, 4, 4)]);
        let many: Vec<Part> = (0..8).map(|at| (at * 10, 5, 1, 1)).collect();
        gather(&mut stale, &many);
        assert_eq!(stale, [(0, 0, 71, 14)]);
    }
}
