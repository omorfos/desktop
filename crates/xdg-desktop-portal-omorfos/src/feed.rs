//! Pictures of a screen, taken one after another for as long as it is
//! shared, each as soon as something on it has changed.
//!
//! The compositor puts each picture straight into memory that is shared
//! with what reads it, which is given here a buffer at a time.

use std::os::fd::AsFd;

use omorfos_pipewire::Memory;
use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self as frame, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self as session, ExtImageCopyCaptureSessionV1},
};

use crate::Failure;

/// What the compositor has said about the pictures being taken.
#[derive(Debug, Default)]
struct Watching {
    shm: Option<wl_shm::WlShm>,
    sources: Option<ExtOutputImageCaptureSourceManagerV1>,
    capturers: Option<ExtImageCopyCaptureManagerV1>,
    output: Option<wl_output::WlOutput>,
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
}

impl Feed {
    /// Begins taking pictures of the first screen, with the pointer in
    /// them or not.
    ///
    /// # Errors
    ///
    /// Returns an error if the compositor gives this program no pictures,
    /// or the memory to take them in cannot be had.
    pub fn new(connection: &Connection, pointer: bool) -> Result<Self, Failure> {
        let mut queue = connection.new_event_queue();
        let handle = queue.handle();
        let mut watching = Watching::default();
        connection.display().get_registry(&handle, ());
        let listed = queue.roundtrip(&mut watching).map_err(Failure::from);
        let session = listed.and_then(|_| {
            let offered = watching.sources.clone().zip(watching.capturers.clone());
            let offered = offered.zip(watching.output.clone());
            let options = if pointer {
                Options::PaintCursors
            } else {
                Options::empty()
            };
            let session = offered.map(|((sources, capturers), output)| {
                let source = sources.create_source(&output, &handle, ());
                let session = capturers.create_session(&source, options, &handle, ());
                (session, (capturers, source, options))
            });
            let told = queue.roundtrip(&mut watching).map_err(Failure::from);
            told.map(|_| session)
        });
        let ready = session.and_then(|session| {
            let given = session.zip(watching.size).zip(watching.shm.clone());
            let given = given.filter(|_| !watching.stopped);
            given.ok_or_else(|| "omorfos gives this program no pictures of the screen".into())
        });
        ready.map(|(((session, taking), size), shm)| Self {
            queue,
            watching,
            shm,
            taking,
            session,
            buffers: Vec::new(),
            size,
        })
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
            self.session = capturers.create_session(source, *options, &handle, ());
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
        let frame = self.session.create_frame(&handle, self.watching.begun);
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
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for Watching {
    fn event(
        watching: &mut Self,
        _: &ExtImageCopyCaptureSessionV1,
        event: session::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            session::Event::BufferSize { width, height } => {
                // A size said again, and not as it was, is a screen that has
                // changed: what reads the stream was told the size it is.
                let said = Some((width, height));
                watching.stopped |= watching.size.is_some_and(|size| Some(size) != said);
                watching.size = watching.size.or(said);
            }
            session::Event::Stopped => watching.stopped = true,
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, u64> for Watching {
    fn event(
        watching: &mut Self,
        frame: &ExtImageCopyCaptureFrameV1,
        event: frame::Event,
        begun: &u64,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let current = *begun == watching.begun;
        match event {
            frame::Event::Ready => {
                watching.ready |= current;
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

delegate_noop!(Watching: ignore wl_shm::WlShm);
delegate_noop!(Watching: wl_shm_pool::WlShmPool);
delegate_noop!(Watching: ignore wl_buffer::WlBuffer);
delegate_noop!(Watching: ignore wl_output::WlOutput);
delegate_noop!(Watching: ExtOutputImageCaptureSourceManagerV1);
delegate_noop!(Watching: ExtImageCaptureSourceV1);
delegate_noop!(Watching: ExtImageCopyCaptureManagerV1);
