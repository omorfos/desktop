//! Streams of pictures in PipeWire, which an application that shares the
//! screen reads.
//!
//! Each screen being shared is a stream of its own, which drives itself: a
//! buffer is taken from it, the compositor puts a picture in it, and it is
//! let go of to whatever reads it. PipeWire makes the memory, shared with
//! the reader, which the compositor writes into directly: nothing is copied
//! on the way.

use std::cell::Cell;
use std::io::Cursor;
use std::os::fd::{AsFd, BorrowedFd};
use std::rc::Rc;
use std::time::Duration;

use pipewire as pw;
use pw::properties::properties;
use pw::spa;
use pw::spa::pod::serialize::PodSerializer;
use pw::spa::pod::{Object, Pod, Property, PropertyFlags, Value};
use pw::stream::{StreamFlags, StreamListener, StreamRc, StreamState};

use omorfos_pipewire::{Held, Removals};

use crate::Failure;

/// The bytes a pixel takes: blue, green, red and one spare, which is how
/// the compositor gives pictures.
const PIXEL: u32 = 4;

/// A connection to PipeWire, made when the first screen is shared.
pub struct Pipes {
    main: pw::main_loop::MainLoopRc,
    _context: pw::context::ContextRc,
    core: pw::core::CoreRc,
}

impl Pipes {
    /// Connects to PipeWire: at `remote`, a socket by name or by path, or
    /// where the environment says.
    ///
    /// # Errors
    ///
    /// Returns an error if PipeWire is not running there.
    pub fn connect(remote: Option<&str>) -> Result<Self, Failure> {
        pw::init();
        let main = pw::main_loop::MainLoopRc::new(None).map_err(Failure::from);
        main.and_then(|main| {
            let context = pw::context::ContextRc::new(&main, None).map_err(Failure::from);
            context.and_then(|context| {
                let at = remote.map(|remote| properties! { *pw::keys::REMOTE_NAME => remote });
                let core = context.connect_rc(at).map_err(Failure::from);
                core.map(|core| Self {
                    main,
                    _context: context,
                    core,
                })
            })
        })
    }

    /// Does whatever PipeWire has for it to do, without waiting.
    pub fn turn(&self) {
        self.main
            .loop_()
            .iterate(pw::loop_::Timeout::Finite(Duration::ZERO));
    }

    /// Begins a stream of pictures `size` pixels large, as a screen of the
    /// desktop is shared.
    ///
    /// # Errors
    ///
    /// Returns an error if PipeWire will not have it.
    pub fn share(&self, size: (u32, u32)) -> Result<Share, Failure> {
        let kind = properties! {
            *pw::keys::MEDIA_CLASS => "Video/Source",
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        };
        let stream =
            StreamRc::new(self.core.clone(), "omorfos-screen", kind).map_err(Failure::from);
        stream.and_then(|stream| {
            let node = Rc::new(Cell::new(None));
            let reading = Rc::new(Cell::new(false));
            let removals = Removals::default();
            let (named, read, removed) = (Rc::clone(&node), Rc::clone(&reading), removals.clone());
            let listener = stream
                .add_local_listener::<()>()
                .state_changed(move |stream, (), was, now| {
                    note(&format!("the stream went from {was:?} to {now:?}"));
                    // Once there is a node, it can be told of.
                    let known = matches!(now, StreamState::Paused | StreamState::Streaming);
                    named.set(known.then(|| stream.node_id()).or(named.get()));
                    // Pictures are taken only while something reads them.
                    read.set(now == StreamState::Streaming);
                    // The stream is told to go once as it begins, empty, so
                    // that PipeWire knows it is driven so and no other way:
                    // a picture let go of before that is sent in a way that
                    // gets in the way of the next.
                    if now == StreamState::Streaming {
                        let _ = stream.trigger_process();
                    }
                })
                .param_changed(move |stream, (), id, param| {
                    // Once the format is agreed, how large the pictures are
                    // is said, so that PipeWire makes room for them.
                    let agreed = id == spa::param::ParamType::Format.as_raw() && param.is_some();
                    if agreed {
                        let mut agreed = spa::param::video::VideoInfoRaw::new();
                        let read = param.map(|param| agreed.parse(param).is_ok());
                        let (format, at) = (agreed.format(), agreed.size());
                        note(&format!(
                            "agreed with what reads the stream ({read:?}): {format:?}, {} by {}",
                            at.width, at.height
                        ));
                        let room = buffers(size);
                        let room = Pod::from_bytes(&room).into_iter();
                        let _ = stream.update_params(&mut room.collect::<Vec<_>>());
                    }
                });
            // The memory of each buffer is made here, so that the
            // compositor can be given it.
            let listener = removed
                .furnish(listener, size.0 * size.1 * PIXEL)
                .register()
                .map_err(Failure::from);
            listener.and_then(|listener| {
                let offer = format(size);
                let offer = Pod::from_bytes(&offer).into_iter();
                let flags = StreamFlags::DRIVER | StreamFlags::ALLOC_BUFFERS;
                let direction = spa::utils::Direction::Output;
                let connected =
                    stream.connect(direction, None, flags, &mut offer.collect::<Vec<_>>());
                connected.map_err(Failure::from).map(|()| Share {
                    stream,
                    _listener: listener,
                    node,
                    reading,
                    removals,
                    size,
                    taken: Cell::new(0),
                    sent: Cell::new(0),
                })
            })
        })
    }
}

impl AsFd for Pipes {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.main.loop_().fd()
    }
}

/// Says in the session's log what became of a stream, so that what went
/// wrong can be found out.
fn note(said: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stderr(), "xdg-desktop-portal-omorfos: {said}");
}

/// A screen being shared.
pub struct Share {
    stream: StreamRc,
    _listener: StreamListener<()>,
    /// The node the stream is in PipeWire, once it has one.
    node: Rc<Cell<Option<u32>>>,
    /// Whether something is reading the stream.
    reading: Rc<Cell<bool>>,
    removals: Removals,
    size: (u32, u32),
    /// How many buffers have been taken to put pictures in, and how many
    /// have gone to what reads the stream.
    taken: Cell<u32>,
    sent: Cell<u32>,
}

impl Share {
    /// What an application is told to read, once PipeWire has said.
    #[must_use]
    pub fn node(&self) -> Option<u32> {
        self.node.get()
    }

    /// Whether something is reading the stream, which is when pictures are
    /// worth taking.
    #[must_use]
    pub fn reading(&self) -> bool {
        self.reading.get()
    }

    /// How many pictures have been begun, and how many have gone to what
    /// reads the stream.
    #[must_use]
    pub fn counted(&self) -> (u32, u32) {
        (self.taken.get(), self.sent.get())
    }

    /// A buffer to put the next picture in, if one is free: one that what
    /// reads the stream has finished with.
    #[must_use]
    pub fn take(&self) -> Option<Held> {
        let held = Held::take(&self.stream, &self.removals);
        self.taken.set(self.taken.get() + u32::from(held.is_some()));
        held
    }

    /// Gives a buffer back to the stream: to what reads it, if a picture
    /// was put in it, which fills all of it; or to be filled again, if not.
    pub fn give(&self, mut held: Held, filled: bool) {
        let (width, height) = self.size;
        let stride = i32::try_from(width * PIXEL).unwrap_or(i32::MAX);
        let filled = filled && held.filled(width * height * PIXEL, stride);
        drop(held);
        let sent = self.sent.get();
        if sent == 0 && filled {
            note("the first picture has gone to what reads the stream");
        }
        self.sent.set(sent + u32::from(filled));
        // The stream drives itself: it is told that there is something.
        let _ = self.stream.trigger_process();
    }
}

/// The format a stream offers: raw video of one size, blue first, as
/// often as there is something new.
fn format((width, height): (u32, u32)) -> Vec<u8> {
    use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
    let offer = spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        spa::pod::property!(FormatProperties::MediaType, Id, MediaType::Video),
        spa::pod::property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
        spa::pod::property!(
            FormatProperties::VideoFormat,
            Id,
            spa::param::video::VideoFormat::BGRx
        ),
        spa::pod::property!(
            FormatProperties::VideoSize,
            Rectangle,
            spa::utils::Rectangle { width, height }
        ),
        spa::pod::property!(
            FormatProperties::VideoFramerate,
            Fraction,
            spa::utils::Fraction { num: 0, denom: 1 }
        ),
    );
    serialize(&Value::Object(offer))
}

/// How much room a stream wants for its pictures, and in what memory.
fn buffers((width, height): (u32, u32)) -> Vec<u8> {
    let int = |key: u32, value: u32| Property {
        key,
        flags: PropertyFlags::empty(),
        value: Value::Int(i32::try_from(value).unwrap_or(i32::MAX)),
    };
    // Memory shared through a file, which the compositor can be given.
    let memory = 1 << spa::sys::SPA_DATA_MemFd;
    let room = Object {
        type_: spa::utils::SpaTypes::ObjectParamBuffers.as_raw(),
        id: spa::param::ParamType::Buffers.as_raw(),
        properties: vec![
            int(spa::sys::SPA_PARAM_BUFFERS_buffers, 4),
            int(spa::sys::SPA_PARAM_BUFFERS_blocks, 1),
            int(spa::sys::SPA_PARAM_BUFFERS_size, width * height * PIXEL),
            int(spa::sys::SPA_PARAM_BUFFERS_stride, width * PIXEL),
            int(spa::sys::SPA_PARAM_BUFFERS_dataType, memory),
        ],
    };
    serialize(&Value::Object(room))
}

/// A description PipeWire reads, as bytes. Writing to memory does not fail.
fn serialize(value: &Value) -> Vec<u8> {
    let written = PodSerializer::serialize(Cursor::new(Vec::new()), value);
    written
        .map(|(bytes, _)| bytes.into_inner())
        .unwrap_or_default()
}
