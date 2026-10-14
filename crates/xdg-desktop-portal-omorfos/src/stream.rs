//! Streams of pictures in PipeWire, which an application that shares the
//! screen reads.
//!
//! Each screen being shared is a stream of its own, which drives itself: a
//! buffer is taken from it, the compositor puts a picture in it, and it is
//! let go of to whatever reads it. PipeWire makes the memory, shared with
//! the reader, which the compositor writes into directly: nothing is copied
//! on the way.

use std::cell::Cell;
use std::fs::File;
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

use omorfos_pipewire::{Held, Made, Pointer, Removals, pointer_room};
use pw::spa::param::video::VideoFlags;
use rustix::fs::{MemfdFlags, memfd_create};

use crate::Failure;
use crate::card::Card;

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
    /// desktop is shared, with room beside each to say where the pointer
    /// is and what it looks like, in a picture of it `pointer` pixels
    /// large, if it is sent beside them. The pictures are in memory of
    /// `card`, in one of its `layouts`, if what reads the stream can take
    /// that, and in shared memory if not.
    ///
    /// # Errors
    ///
    /// Returns an error if PipeWire will not have it.
    pub(crate) fn share(
        &self,
        size: (u32, u32),
        pointer: Option<(u32, u32)>,
        (card, layouts): (Card, Vec<u64>),
    ) -> Result<Share, Failure> {
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
            let layout = Rc::new(Cell::new(None));
            let (named, read, removed) = (Rc::clone(&node), Rc::clone(&reading), removals.clone());
            let (chosen, agreeing) = (Rc::clone(&layout), Rc::clone(&layout));
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
                        // What reads the stream took memory of the card in
                        // a layout, or shared memory.
                        let card = agreed.flags().contains(VideoFlags::MODIFIER);
                        let layout = card.then_some(agreed.modifier());
                        agreeing.set(layout);
                        let (format, at) = (agreed.format(), agreed.size());
                        let memory = memory(layout);
                        note(&format!(
                            "agreed with what reads the stream ({read:?}): {format:?}, {} by {}, \
                             in {memory}",
                            at.width, at.height
                        ));
                        let room = [Some(buffers(size, card)), pointer.map(beside)];
                        let room = room.iter().flatten();
                        let room = room.filter_map(|room| Pod::from_bytes(room));
                        let _ = stream.update_params(&mut room.collect::<Vec<_>>());
                    }
                });
            // The memory of each buffer is made here, so that the
            // compositor can be given it: on the card, in the layout agreed,
            // or shared.
            let make = move || card.make(size, chosen.get(), || shared(size));
            let listener = removed
                .furnish(listener, make)
                .register()
                .map_err(Failure::from);
            listener.and_then(|listener| {
                let offers = offers(size, &layouts);
                let offer = offers.iter().filter_map(|offer| Pod::from_bytes(offer));
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
                    layout,
                    size,
                    taken: Cell::new(0),
                    sent: Cell::new(0),
                    alone: Cell::new(0),
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
    /// The layout of memory of the graphics card agreed with what reads
    /// the stream, or nothing, for shared memory.
    layout: Rc<Cell<Option<u64>>>,
    size: (u32, u32),
    /// How many buffers have been taken, and how many have gone to what
    /// reads the stream with a picture in them, and with only where the
    /// pointer is.
    taken: Cell<u32>,
    sent: Cell<u32>,
    alone: Cell<u32>,
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

    /// How many buffers have been taken, and how many have gone to what
    /// reads the stream with a picture, and with the pointer alone.
    #[must_use]
    pub fn counted(&self) -> (u32, u32, u32) {
        (self.taken.get(), self.sent.get(), self.alone.get())
    }

    /// The layout of memory of the graphics card agreed with what reads
    /// the stream, or nothing, for shared memory.
    #[must_use]
    pub fn layout(&self) -> Option<u64> {
        self.layout.get()
    }

    /// A buffer to put the next picture in, if one is free: one that what
    /// reads the stream has finished with.
    #[must_use]
    pub fn take(&self) -> Option<Held> {
        let held = Held::take(&self.stream, &self.removals);
        self.taken.set(self.taken.get() + u32::from(held.is_some()));
        held
    }

    /// Gives a buffer back to the stream, for what reads it: with a picture,
    /// if one was put in it, which fills all of it; and saying where the
    /// pointer is, if it is sent beside the pictures. Without either, it
    /// is nothing new.
    pub fn give(&self, mut held: Held, filled: bool, pointer: Option<&Pointer<'_>>) {
        let (_, height) = self.size;
        let stride = held.stride();
        let size = stride * height;
        let filled = filled && held.filled(size, i32::try_from(stride).unwrap_or(i32::MAX));
        let told = pointer.is_some_and(|pointer| held.point(pointer));
        self.alone
            .set(self.alone.get() + u32::from(told && !filled));
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

/// What memory was agreed, in words for the log: of the graphics card laid
/// out as `layout`, or shared.
fn memory(layout: Option<u64>) -> String {
    let card = layout.map(|layout| format!("memory of the graphics card, laid out as {layout:#x}"));
    card.unwrap_or_else(|| "shared memory".to_owned())
}

/// Shared memory for a picture `size` large, a file of its own.
fn shared((width, height): (u32, u32)) -> Option<Made> {
    let size = width * height * PIXEL;
    let file = memfd_create(
        "omorfos-screen",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    );
    let file = file.ok().map(File::from);
    let file = file.filter(|file| file.set_len(u64::from(size)).is_ok());
    file.map(|file| Made {
        file: file.into(),
        offset: 0,
        stride: width * PIXEL,
        size,
        card: false,
        mappable: true,
    })
}

/// The formats a stream offers, the first that what reads it can take
/// being the one agreed: memory of the graphics card in each of the
/// `layouts` it can be made in, and shared memory.
fn offers(size: (u32, u32), layouts: &[u64]) -> Vec<Vec<u8>> {
    let card = layouts.iter().map(|layout| format(size, Some(*layout)));
    card.chain(std::iter::once(format(size, None))).collect()
}

/// A format a stream offers: raw video of one size, blue first, as often
/// as there is something new, in memory of the graphics card in `layout`
/// if there is one, which what reads it has to take as it is.
fn format((width, height): (u32, u32), layout: Option<u64>) -> Vec<u8> {
    use spa::param::format::{FormatProperties, MediaSubtype, MediaType};
    let mut offer = spa::pod::object!(
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
    offer.properties.extend(layout.map(|layout| Property {
        key: FormatProperties::VideoModifier.as_raw(),
        flags: PropertyFlags::MANDATORY,
        value: Value::Long(layout.cast_signed()),
    }));
    serialize(&Value::Object(offer))
}

/// How much room a stream wants for its pictures, and in what memory: of
/// the graphics card, or shared through a file, which the compositor can
/// be given either way.
fn buffers((width, height): (u32, u32), card: bool) -> Vec<u8> {
    let int = |key: u32, value: u32| Property {
        key,
        flags: PropertyFlags::empty(),
        value: Value::Int(i32::try_from(value).unwrap_or(i32::MAX)),
    };
    let kind = if card {
        spa::sys::SPA_DATA_DmaBuf
    } else {
        spa::sys::SPA_DATA_MemFd
    };
    let memory = 1 << kind;
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

/// How much room the stream wants beside each picture to say where the
/// pointer is and what it looks like, in a picture of it `size` pixels
/// large.
fn beside(size: (u32, u32)) -> Vec<u8> {
    let room = Object {
        type_: spa::utils::SpaTypes::ObjectParamMeta.as_raw(),
        id: spa::param::ParamType::Meta.as_raw(),
        properties: vec![
            Property {
                key: spa::sys::SPA_PARAM_META_type,
                flags: PropertyFlags::empty(),
                value: Value::Id(spa::utils::Id(spa::sys::SPA_META_Cursor)),
            },
            Property {
                key: spa::sys::SPA_PARAM_META_size,
                flags: PropertyFlags::empty(),
                value: Value::Int(i32::try_from(pointer_room(size)).unwrap_or(i32::MAX)),
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use pw::spa::pod::PodPropFlags;
    use pw::spa::utils::Id;

    /// What a description says of `key`, as PipeWire reads it, and how.
    fn property(bytes: &[u8], key: u32, read: fn(&Pod) -> i64) -> Option<(i64, PodPropFlags)> {
        let object = Pod::from_bytes(bytes).unwrap().as_object().unwrap();
        let found = object.find_prop(Id(key));
        found.map(|found| (read(found.value()), found.flags()))
    }

    #[test]
    fn memory_of_the_card_is_offered_in_each_layout_it_can_be_made_in_and_shared_memory_last() {
        use spa::param::format::FormatProperties;
        let key = FormatProperties::VideoModifier.as_raw();
        let long = |value: &Pod| value.get_long().unwrap();
        let offers = offers((640, 480), &[0, 7]);
        let layouts: Vec<_> = offers
            .iter()
            .map(|offer| property(offer, key, long))
            .collect();
        let mandatory = PodPropFlags::MANDATORY;
        assert_eq!(layouts, [Some((0, mandatory)), Some((7, mandatory)), None]);
    }

    #[test]
    fn the_log_says_which_memory_was_agreed() {
        assert_eq!(memory(None), "shared memory");
        let card = "memory of the graphics card, laid out as 0x100000000000001";
        assert_eq!(memory(Some(0x0100_0000_0000_0001)), card);
    }

    #[test]
    fn buffers_are_said_to_be_in_the_memory_that_was_agreed() {
        let int = |value: &Pod| i64::from(value.get_int().unwrap());
        let kind = |card| {
            let room = buffers((640, 480), card);
            let kind = property(&room, spa::sys::SPA_PARAM_BUFFERS_dataType, int);
            kind.map(|(kind, _)| kind)
        };
        let card = 1 << spa::sys::SPA_DATA_DmaBuf;
        let shared = 1 << spa::sys::SPA_DATA_MemFd;
        assert_eq!(kind(true), Some(card));
        assert_eq!(kind(false), Some(shared));
    }
}
