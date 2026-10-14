//! Holds PipeWire's buffers while a picture is put in them.
//!
//! This is the second place in omorfos that uses `unsafe`, after
//! `omorfos-shm`, and it is a crate of its own for the same reason: so
//! that every other crate can go on forbidding it. The PipeWire bindings
//! hand a buffer that is to be kept for a while, or given memory of the
//! caller's own, only as a raw pointer. Why that was accepted, and what
//! would let it be taken out, is in `docs/adr/0041-pipewire-buffers.md`.
//!
//! What is offered is safe to call with anything. The memory of each
//! buffer is made here, as a file of its own that the compositor can be
//! given; PipeWire would otherwise put every buffer in one block it does
//! not name. A buffer is held with the stream it came from, is given back
//! to that stream however it is let go of, and is not touched once
//! PipeWire has taken it away, which it says it does before it frees one.

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::os::fd::{AsRawFd, BorrowedFd, OwnedFd};
use std::ptr::NonNull;
use std::rc::Rc;

use pipewire::spa::sys::{
    SPA_CHUNK_FLAG_NONE, SPA_DATA_FLAG_MAPPABLE, SPA_DATA_FLAG_READWRITE, SPA_DATA_MemFd,
    SPA_META_Cursor, SPA_VIDEO_FORMAT_BGRA, spa_data, spa_meta, spa_meta_bitmap, spa_meta_cursor,
    spa_point, spa_rectangle,
};
use pipewire::stream::{ListenerLocalBuilder, StreamRc};
use pipewire::sys::pw_buffer;
use rustix::fs::{MemfdFlags, memfd_create};

/// The memory of a stream's buffers, made here, and the times PipeWire has
/// taken buffers away, so that a buffer held from before is known to be
/// gone.
#[derive(Debug, Clone, Default)]
pub struct Removals {
    count: Rc<Cell<u64>>,
    /// The file each buffer's memory is, by where the buffer is.
    files: Rc<RefCell<Vec<(usize, OwnedFd)>>>,
}

impl Removals {
    /// Has a stream's buffers be memory made here, `size` bytes each, by
    /// listening for them being made and taken away. The stream is to be
    /// connected with `ALLOC_BUFFERS`, which says that it makes them.
    #[must_use]
    pub fn furnish<'a, D>(
        &self,
        listener: ListenerLocalBuilder<'a, D>,
        size: u32,
    ) -> ListenerLocalBuilder<'a, D> {
        let (made, gone) = (self.clone(), self.clone());
        listener
            .add_buffer(move |_, _, buffer| made.made(buffer, size))
            .remove_buffer(move |_, _, buffer| gone.removed(buffer))
    }

    /// Gives a buffer PipeWire has just made memory of its own, shared
    /// through a file, if some can be had. Without, it has none, and
    /// nothing is put in it.
    fn made(&self, buffer: *mut pw_buffer, size: u32) {
        let file = memfd_create(
            "omorfos-screen",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        );
        let file = file.ok().map(File::from);
        let file = file.filter(|file| file.set_len(u64::from(size)).is_ok());
        let part = NonNull::new(buffer).and_then(first);
        let given = part.zip(file);
        let given = given.map(|(mut part, file)| {
            // TODO: have the PipeWire bindings give a buffer memory safely,
            // and forbid `unsafe` here as everywhere else. See
            // docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: PipeWire made the buffer and hands it over while it
            // is made, on this thread, to be given its memory: nothing else
            // reads or writes it meanwhile.
            let part = unsafe { part.as_mut() };
            // The stream offers no other memory than this, so this is what
            // the buffer may be.
            part.type_ = SPA_DATA_MemFd;
            // What reads it is to map it to read it.
            part.flags = SPA_DATA_FLAG_READWRITE | SPA_DATA_FLAG_MAPPABLE;
            part.fd = i64::from(file.as_raw_fd());
            part.mapoffset = 0;
            part.maxsize = size;
            part.data = std::ptr::null_mut();
            (buffer.addr(), OwnedFd::from(file))
        });
        self.files.borrow_mut().extend(given);
    }

    /// Notes that a buffer has been taken away, and lets go of its memory.
    /// Every buffer held from the stream until now is let go of without
    /// being touched.
    fn removed(&self, buffer: *mut pw_buffer) {
        self.count.set(self.count.get().wrapping_add(1));
        self.files
            .borrow_mut()
            .retain(|(at, _)| *at != buffer.addr());
    }
}

/// How many bytes what is said of the pointer takes, before its picture.
const CURSOR: u32 = 28;
const BITMAP: u32 = 20;
const _: () = assert!(size_of::<spa_meta_cursor>() == CURSOR as usize);
const _: () = assert!(size_of::<spa_meta_bitmap>() == BITMAP as usize);

/// How much room to ask for, beside each picture, to say where the pointer
/// is and what it looks like, in a picture of it `size` pixels large.
#[must_use]
pub const fn pointer_room((width, height): (u32, u32)) -> u32 {
    let pixels = width.saturating_mul(height).saturating_mul(4);
    (CURSOR + BITMAP).saturating_add(pixels)
}

/// Where the pointer is and what it looks like, as what reads a stream is
/// told beside a picture.
#[derive(Debug, Clone, Copy)]
pub struct Pointer<'a> {
    /// Where its position is in the picture, in pixels.
    pub at: (i32, i32),
    /// Where its position is in its own picture.
    pub tip: (i32, i32),
    /// How large its own picture is, in pixels.
    pub size: (u32, u32),
    /// What it looks like.
    pub picture: Picture<'a>,
}

/// What the pointer looks like.
#[derive(Debug, Clone, Copy)]
pub enum Picture<'a> {
    /// As these pixels: four bytes to a pixel as blue, green, red and
    /// alpha, premultiplied, row by row.
    Shown(&'a [u8]),
    /// As nothing: it is not over what is shared, or is hidden.
    Hidden,
}

/// Memory a picture is put in: a file shared with whatever reads the
/// stream, where in it the buffer starts, and how many bytes it has.
#[derive(Debug)]
pub struct Memory {
    /// The file, a copy of the stream's own.
    pub file: OwnedFd,
    /// Where in it the buffer starts.
    pub offset: u32,
    /// How many bytes the buffer has.
    pub size: u32,
    /// What the buffer is known by: how many times buffers had been taken
    /// away when it was handed out, and which of them it is. The same
    /// memory is handed out again and again, until buffers are taken away.
    pub place: (u64, usize),
}

/// A buffer taken from a stream to be filled, which goes back to it when it
/// is let go of: to be read, if something was put in it.
#[derive(Debug)]
pub struct Held {
    stream: StreamRc,
    buffer: NonNull<pw_buffer>,
    removals: Removals,
    /// How many removals there had been when it was taken.
    taken: u64,
}

impl Held {
    /// Takes a buffer of `stream` to fill, if one is free.
    #[must_use]
    pub fn take(stream: &StreamRc, removals: &Removals) -> Option<Self> {
        // TODO: have the PipeWire bindings hold a buffer safely, and forbid
        // `unsafe` here as everywhere else. See docs/adr/0041-pipewire-buffers.md.
        #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
        // SAFETY: the stream is alive, being borrowed, and this is called
        // on the thread its loop runs on, as everything here is. What comes
        // back is a buffer of that stream, or null if none is free, and it
        // is given back to that stream when this is dropped.
        let raw = unsafe { stream.dequeue_raw_buffer() };
        NonNull::new(raw).map(|buffer| {
            let mut held = Self {
                stream: stream.clone(),
                buffer,
                removals: removals.clone(),
                taken: removals.count.get(),
            };
            // Until a picture is put in it, it is said to have none, so that
            // given back unfilled it is read as nothing new: the pointer
            // alone, if it has moved.
            let _ = held.chunk(0, 0, SPA_CHUNK_FLAG_NONE);
            held
        })
    }

    /// Whether PipeWire has taken it away since it was taken, as it does
    /// when what reads the stream changes. Nothing put in it is read.
    #[must_use]
    pub fn gone(&self) -> bool {
        !self.live()
    }

    /// Whether PipeWire has not taken it away since it was taken.
    fn live(&self) -> bool {
        self.removals.count.get() == self.taken
    }

    /// The shared memory its picture goes in, if it has any: what the
    /// stream offers is shared memory, unless it is given something else.
    #[must_use]
    pub fn memory(&self) -> Option<Memory> {
        let place = (self.taken, self.buffer.as_ptr().addr());
        self.first().and_then(|data| {
            let shared = data.type_ == SPA_DATA_MemFd && data.fd >= 0;
            let fd = i32::try_from(data.fd).ok().filter(|_| shared);
            let file = fd.and_then(|fd| {
                // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
                #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
                // SAFETY: the descriptor is the buffer's, which PipeWire
                // keeps open until it takes the buffer away, and it has
                // not: `first` says so. It is copied at once, so nothing
                // outlives this borrow of it.
                let file = unsafe { BorrowedFd::borrow_raw(fd) };
                file.try_clone_to_owned().ok()
            });
            file.map(|file| Memory {
                file,
                offset: data.mapoffset,
                size: data.maxsize,
                place,
            })
        })
    }

    /// Says how much of it the picture put in it fills: `size` bytes, rows
    /// of `stride` bytes. Returns whether it could be said: not once
    /// PipeWire has taken it away, when nothing put in it is read.
    pub fn filled(&mut self, size: u32, stride: i32) -> bool {
        self.chunk(size, stride, SPA_CHUNK_FLAG_NONE)
    }

    /// Says what is in it: `size` bytes, rows of `stride`, and what is
    /// wrong with them, if anything. Returns whether it could be said: not
    /// once PipeWire has taken it away.
    fn chunk(&mut self, size: u32, stride: i32, flags: u32) -> bool {
        let chunk = self.first().map(|data| data.chunk);
        let chunk = chunk.and_then(NonNull::new);
        let said = chunk.map(|mut chunk| {
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: the chunk is the first part's, not null, and the
            // buffer is live; nothing else writes it while it is held.
            let chunk = unsafe { chunk.as_mut() };
            chunk.offset = 0;
            chunk.size = size;
            chunk.stride = stride;
            chunk.flags = i32::try_from(flags).unwrap_or_default();
        });
        said.is_some()
    }

    /// Says beside its picture, if it has one, where the pointer is and
    /// what it looks like. Returns whether it could be said: not if what
    /// reads the stream did not ask to be told, or PipeWire has taken the
    /// buffer away.
    pub fn point(&mut self, pointer: &Pointer<'_>) -> bool {
        let (width, height) = pointer.size;
        let needed = pointer_room(pointer.size);
        let pixels = (needed - CURSOR - BITMAP) as usize;
        let meta = self.meta(SPA_META_Cursor);
        // What is said of the pointer is said whole, or not at all.
        let meta = meta.filter(|meta| meta.size >= needed);
        let meta = meta.and_then(|meta| NonNull::new(meta.data.cast::<u8>()));
        let said = meta.map(|at| {
            let told = spa_meta_cursor {
                id: 1,
                flags: 0,
                position: spa_point {
                    x: pointer.at.0,
                    y: pointer.at.1,
                },
                hotspot: spa_point {
                    x: pointer.tip.0,
                    y: pointer.tip.1,
                },
                bitmap_offset: CURSOR,
            };
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: the metadata is the buffer's, which is live, and has
            // room for this, as was checked; nothing else writes it while
            // the buffer is held. It need not be aligned.
            unsafe {
                at.cast::<spa_meta_cursor>().write_unaligned(told);
            }
            let header = spa_meta_bitmap {
                format: SPA_VIDEO_FORMAT_BGRA,
                size: spa_rectangle { width, height },
                stride: (width * 4).cast_signed(),
                offset: BITMAP,
            };
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: as above: the bitmap comes after what was written
            // and there is room for it and its pixels after it.
            let bitmap = unsafe { at.add(CURSOR as usize) };
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: as above, for the bitmap's header.
            unsafe {
                bitmap.cast::<spa_meta_bitmap>().write_unaligned(header);
            }
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: as above: the pixels are what follows the header.
            let after = unsafe { bitmap.add(BITMAP as usize) };
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: as above: `pixels` bytes of the room that was
            // checked for, which nothing else reads or writes meanwhile.
            let room = unsafe { std::slice::from_raw_parts_mut(after.as_ptr(), pixels) };
            // A picture that is not as large as it is said to be is not
            // shown.
            match pointer.picture {
                Picture::Shown(bytes) if bytes.len() == room.len() => {
                    room.copy_from_slice(bytes);
                }
                _ => room.fill(0),
            }
        });
        said.is_some()
    }

    /// The buffer's metadata of a kind, if it has it and is still the
    /// stream's.
    fn meta(&self, kind: u32) -> Option<spa_meta> {
        let live = self.live().then_some(self.buffer);
        // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
        #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
        // SAFETY: the buffer is live, so PipeWire has not freed it.
        let spa = live.and_then(|buffer| NonNull::new(unsafe { buffer.as_ref() }.buffer));
        spa.and_then(|spa| {
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: as above, for what the buffer points at.
            let spa = unsafe { spa.as_ref() };
            let metas = NonNull::new(spa.metas).filter(|_| spa.n_metas > 0);
            metas.and_then(|metas| {
                // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
                #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
                // SAFETY: as above: the buffer has `n_metas` of them, at
                // `metas`, which nothing else writes while it is held.
                let metas =
                    unsafe { std::slice::from_raw_parts(metas.as_ptr(), spa.n_metas as usize) };
                metas.iter().find(|meta| meta.type_ == kind).copied()
            })
        })
    }

    /// The first part of the buffer, which is where a picture goes, if it
    /// has one and is still the stream's.
    fn first(&self) -> Option<&spa_data> {
        let live = self.live().then_some(self.buffer);
        let part = live.and_then(first);
        // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
        #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
        // SAFETY: the buffer is live, so PipeWire has not freed it, and its
        // parts are read while it is held and nothing else writes them.
        part.map(|part| unsafe { part.as_ref() })
    }
}

/// The first part of a buffer PipeWire made, if it has one.
fn first(buffer: NonNull<pw_buffer>) -> Option<NonNull<spa_data>> {
    // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
    #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
    // SAFETY: this is only called with a buffer PipeWire has not freed: one
    // it is handing over, or one held and live. What it points at is null
    // for a buffer that has none.
    let spa = unsafe { buffer.as_ref() }.buffer;
    NonNull::new(spa).and_then(|spa| {
        // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
        #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
        // SAFETY: as above, for what the buffer points at.
        let spa = unsafe { spa.as_ref() };
        let parts = (spa.n_datas > 0).then_some(spa.datas);
        parts.and_then(NonNull::new)
    })
}

impl Drop for Held {
    fn drop(&mut self) {
        if self.live() {
            // TODO: as above. See docs/adr/0041-pipewire-buffers.md.
            #[expect(unsafe_code, reason = "see docs/adr/0041-pipewire-buffers.md")]
            // SAFETY: the buffer was taken from this stream, which is kept
            // alive by being held here, and PipeWire has not taken it away.
            unsafe {
                self.stream.queue_raw_buffer(self.buffer.as_ptr());
            }
        }
    }
}
