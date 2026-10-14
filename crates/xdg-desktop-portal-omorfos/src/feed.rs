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
    output: Option<wl_output::WlOutput>,
    seat: Option<wl_seat::WlSeat>,
    followed: Followed,
    /// How large the pictures are, once said.
    size: Option<(u32, u32)>,
    /// Whether no more will be given.
    stopped: bool,
    /// Whether the picture asked for has been taken, or could not be, as
    /// while the session is locked.
    ready: bool,
    lost: bool,
    /// How many sessions have been begun: what is said of a picture asked
    /// for in one before the last is not heard.
    begun: u64,
}

/// Pictures of the first screen, taken over a connection to the compositor
/// that is shared with the rest of the program.
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
    /// The compositor's names for the memory pictures have been put in, by
    /// the name of that memory, kept so as not to be made for each picture.
    buffers: Vec<((u64, usize), wl_buffer::WlBuffer)>,
    size: (u32, u32),
    /// The pointer, if it is followed beside the pictures.
    following: Option<Following>,
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
    /// Begins taking pictures of the first screen, with the pointer drawn
    /// in, followed beside them, or neither.
    ///
    /// # Errors
    ///
    /// Returns an error if the compositor gives this program no pictures,
    /// or the memory to take them in cannot be had.
    pub fn new(connection: &Connection, pointer: Pointer) -> Result<Self, Failure> {
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        let mut watching = Watching::default();
        connection.display().get_registry(&handle, ());
        let listed = queue.roundtrip(&mut watching).map_err(Failure::from);
        let session = listed.and_then(|_| {
            let offered = watching.sources.clone().zip(watching.capturers.clone());
            let offered = offered.zip(watching.output.clone());
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
                size,
                following,
            })
        });
        ready.inspect(Self::point)
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

    /// Asks for the next picture, to be put in `memory`. The compositor
    /// takes it once something on the screen has changed, and says it
    /// cannot if the memory is too small, which ends the feed.
    ///
    /// Only one is asked for at a time: the one before has been put where
    /// it was to go.
    pub fn ask(&mut self, memory: &Memory) {
        // Memory handed out before buffers were last taken away is gone,
        // and nothing is being put in it. What reads the stream may well
        // have changed, and is shown the screen at once, though nothing on
        // it changes: the first picture of a session is taken at once.
        let handed = self.buffers.first().map(|((handed, _), _)| *handed);
        if handed.is_some_and(|handed| handed != memory.place.0) {
            let (capturers, source, options) = &self.taking;
            self.session.destroy();
            self.watching.begun += 1;
            let handle = self.queue.handle();
            let begun = Of::Screen(self.watching.begun);
            self.session = capturers.create_session(source, *options, &handle, begun);
        }
        self.buffers.retain(|((handed, _), buffer)| {
            let kept = *handed == memory.place.0;
            if !kept {
                buffer.destroy();
            }
            kept
        });
        let (width, height) = self.size;
        let (across, down) = (width.cast_signed(), height.cast_signed());
        let handle = self.queue.handle();
        let known = self
            .buffers
            .iter()
            .find(|(known, _)| *known == memory.place);
        let known = known.map(|(_, buffer)| buffer.clone());
        let buffer = known.unwrap_or_else(|| self.name(memory));
        let frame = self
            .session
            .create_frame(&handle, Of::Screen(self.watching.begun));
        frame.attach_buffer(&buffer);
        frame.damage_buffer(0, 0, across, down);
        frame.capture();
    }

    /// Has the compositor know memory by a name, to put pictures in, and
    /// keeps the name for the next time it is given.
    fn name(&mut self, memory: &Memory) -> wl_buffer::WlBuffer {
        let (width, height) = self.size;
        let (across, down) = (width.cast_signed(), height.cast_signed());
        let handle = self.queue.handle();
        let end = u64::from(memory.offset) + u64::from(memory.size);
        let end = i32::try_from(end).unwrap_or(i32::MAX);
        let pool = self.shm.create_pool(memory.file.as_fd(), end, &handle, ());
        let offset = memory.offset.cast_signed();
        let format = wl_shm::Format::Xrgb8888;
        let buffer = pool.create_buffer(offset, across, down, across * 4, format, &handle, ());
        pool.destroy();
        self.buffers.push((memory.place, buffer.clone()));
        buffer
    }

    /// Takes in what the compositor has said, without waiting, and returns
    /// whether the picture asked for has been put where it was to go, or
    /// could not be; or nothing, if it is still to come.
    ///
    /// # Errors
    ///
    /// Returns an error once no more pictures will be given: the screen
    /// has gone or changed, or the connection has.
    pub fn turn(&mut self) -> Result<Option<bool>, Failure> {
        let handled = self.queue.dispatch_pending(&mut self.watching);
        let handled = handled.map_err(Failure::from);
        let over = self.watching.stopped;
        let going = handled.and_then(|_| {
            let said = "the screen being shared has gone or changed";
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
            put.or_else(|| std::mem::take(lost).then_some(false))
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
        for (_, buffer) in &self.buffers {
            buffer.destroy();
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
            name, interface, ..
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
                "wl_output" if watching.output.is_none() => {
                    watching.output = Some(registry.bind(name, 1, handle, ()));
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
                // changed: what reads the stream was told the size it is.
                let said = Some((width, height));
                watching.stopped |= watching.size.is_some_and(|size| Some(size) != said);
                watching.size = watching.size.or(said);
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
delegate_noop!(Watching: ignore wl_pointer::WlPointer);
delegate_noop!(Watching: wl_shm_pool::WlShmPool);
delegate_noop!(Watching: ignore wl_buffer::WlBuffer);
delegate_noop!(Watching: ignore wl_output::WlOutput);
delegate_noop!(Watching: ExtOutputImageCaptureSourceManagerV1);
delegate_noop!(Watching: ExtImageCaptureSourceV1);
delegate_noop!(Watching: ExtImageCopyCaptureManagerV1);
