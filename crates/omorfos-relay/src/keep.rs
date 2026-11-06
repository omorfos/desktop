//! Keeping a program through a change of compositor (ADR 61, step 2).
//!
//! When the compositor ends, the program waits, told only that the pointer, the
//! keyboard and any field typed into have left its surfaces, that a gesture
//! under way was cancelled, that the pointer it held is let go of, that a user
//! it was told is away is back, as the next compositor counts from when it
//! starts, that a key the keyboard held is let go of, that an input method
//! typing in a field is no longer, with any key held in its keyboard grab let
//! go of, that a picture it asked for has failed
//! and the pointer it follows has left, and that what lasts only a moment has ended: a popup
//! dismissed, a token given, a picture discarded, which the relay answers for
//! from then on. A pointer held once only that has been held is over too, and
//! so is an input method told another is the one, and a lock told it is over;
//! and a window in its lists of windows that is not kept has closed. When the next is joined, what the
//! program had is made again in two parts, each ended by a sync of the relay's
//! own: its registries, so that the new compositor's globals are known, then
//! the rest, as [`Journal`] kept it. Until the second is answered, what the
//! compositor says is sifted: what the program knows already is kept from it.
//! What a buffer has to be for its pictures is told again only if it is not as
//! it was. A locker is not told again that the session is locked, and a lock
//! surface told the size it was is shown as it was by the relay; one told
//! another is the locker's to draw anew, as what it showed is of another size. A window the next compositor lists that the program has listed is
//! the one it has, once the relay knows which it is. A window of another
//! program's that it imported by the handle the compositor before shared it by
//! is imported again by the one the next shares it by, once that is known: the
//! import made again by the old handle, which the next compositor refuses, is
//! destroyed first, and that it was refused is kept from the program. What it
//! takes pictures of a window through is made again once the next compositor
//! lists the window, as `pictures` says, and stops if it does not.
//!
//! Some numbers mean something only to one compositor, and the relay puts
//! the program's in their place and theirs in the program's: the names of
//! globals, and the numbers of the objects a compositor makes, which a new
//! one takes from where the one before did while the program may still hold
//! some of those. Of serials it changes only those that are counts: a
//! field's, which counts the field's commits, and what an input method
//! commits with, which counts what it has been told is whole; the new
//! compositor counts each from when it made it again. The rest it does not
//! change: a window's configure acked by the relay is not acked again, nor
//! one the compositor before sent, and a program told the pointer has come
//! back names its shape again with the new compositor's serial, as it would
//! to any compositor.

use std::collections::{BTreeMap, HashMap, HashSet};

use wayland_backend::protocol::MessageDesc;

use crate::journal::{self, Journal, Numbering, Sending};
use crate::objects::{Objects, Read, SERVERS};
use crate::serials::{self, Serials};
use crate::wire::{self, Broken, HEADER, Header};

mod pictures;

use pictures::{Finding, Pictured};

/// The display, which every connection starts with.
const DISPLAY: u32 = 1;

/// Which of the display's events says a number may be taken again.
const DELETE_ID: u16 = 1;

/// How many of the relay's syncs follow one another once it has caught up
/// before what is held is taken to have left: the compositor gives a window
/// the keyboard and the pointer in the turn after it is shown again, and the
/// second is answered after that turn.
const SETTLING: u8 = 2;

/// Which of a field's events ends what is typed into it for now, with the
/// number of times it has been committed.
const TEXT_DONE: u16 = 5;

/// The lifetime of a lock or confinement of the pointer that ends once it is
/// let go of.
const ONESHOT: u32 = 1;

/// The events of an input method that say it is typing in a field, that it
/// is not, that what it has been told is whole, and that another is the
/// one; and its keyboard grab's that says a key was pressed or let go of.
const ACTIVATE: u16 = 0;
const DEACTIVATE: u16 = 1;
const TYPING_DONE: u16 = 5;
const UNAVAILABLE: u16 = 6;
const GRABBED_KEY: u16 = 1;

/// The events of a keyboard that say it entered a surface, that it left,
/// and that a key was pressed or let go of.
const ENTER: u16 = 1;
const LEAVE: u16 = 2;
const KEY: u16 = 3;

/// The events of a window in a list of windows: that it has closed, that
/// what has been said of it is whole, what it is called, what program it
/// is of, and its identifier.
const CLOSED: u16 = 0;
const LISTED_DONE: u16 = 1;
const TITLE: u16 = 2;
const APP_ID: u16 = 3;
const IDENTIFIER: u16 = 4;

/// The events of a session of pictures (`ext-image-copy-capture`) that say
/// what a buffer has to be is whole, and that no more will be taken.
const CONSTRAINED: u16 = 4;
const STOPPED: u16 = 5;

/// The events of a picture that say it is taken and that it failed, with
/// why: for no reason given, or as what it was of has gone; and its request
/// that asks for it.
const SHOT_READY: u16 = 3;
const SHOT_FAILED: u16 = 4;
const UNKNOWN: u32 = 0;
const GONE: u32 = 2;
const CAPTURE: u16 = 3;

/// The events of what follows the pointer that say it came over what it
/// follows, and that it left.
const POINTER_IN: u16 = 0;
const POINTER_OUT: u16 = 1;

/// The event of a window shared with other programs (`xdg-foreign`) that
/// says the handle it is shared by, and the importer's request that imports
/// one by its handle.
const HANDLE: u16 = 0;
const IMPORT: u16 = 1;

/// The event of a window imported that says it is gone, and its requests
/// that destroy it and make a window of the program's belong to it.
const IMPORT_GONE: u16 = 0;
const UNIMPORT: u16 = 0;
const PARENT_OF: u16 = 1;

/// The requests that ack a configure: a window's `xdg_surface`'s, a layer
/// surface's, and a lock surface's.
const WINDOW_ACK: u16 = 4;
const LAYER_ACK: u16 = 6;
const LOCK_ACK: u16 = 1;

/// The event of a lock that says the session is locked; its other says the
/// lock was refused or is over.
const LOCKED: u16 = 0;

/// Numbers that mean something only to one compositor, kept as the
/// program's for theirs where the two differ.
#[derive(Debug, Default)]
struct Renumbered {
    /// The compositor's for the program's.
    theirs: HashMap<u32, u32>,
    /// The program's for the compositor's.
    ours: HashMap<u32, u32>,
}

impl Renumbered {
    /// The compositor's number for the program's `number`.
    fn theirs(&self, number: u32) -> u32 {
        self.theirs.get(&number).copied().unwrap_or(number)
    }

    /// The program's number for the compositor's `number`.
    fn ours(&self, number: u32) -> u32 {
        self.ours.get(&number).copied().unwrap_or(number)
    }

    /// The program's number for the compositor's `number`, if it is kept
    /// as theirs: not if the program's number of that name is another's.
    fn known(&self, number: u32) -> Option<u32> {
        let ours = self.ours(number);
        (self.theirs(ours) == number).then_some(ours)
    }

    /// Keeps that the program's `ours` is the compositor's `theirs`.
    fn set(&mut self, ours: u32, theirs: u32) {
        if ours != theirs {
            self.theirs.insert(ours, theirs);
            self.ours.insert(theirs, ours);
        }
    }

    /// Forgets the program's `ours`.
    fn forget(&mut self, ours: u32) {
        if let Some(theirs) = self.theirs.remove(&ours) {
            self.ours.remove(&theirs);
        }
    }
}

/// A global, as a registry says it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Global {
    name: u32,
    interface: Vec<u8>,
    version: u32,
}

/// A window or a layer surface, as far as its configures go.
#[derive(Debug, Default)]
struct Window {
    /// The serials the compositor has sent the program for it, not yet
    /// acked.
    offered: Vec<u32>,
    /// The size a layer surface was last told it is.
    size: Option<(u32, u32)>,
}

/// What has the pointer, the keyboard or a field typed into, as the program
/// was told.
#[derive(Debug, Clone, Copy)]
struct Focus {
    surface: u32,
    /// The serial it was told it with, but for a field's, which has none.
    serial: Option<u32>,
    /// Where on the surface a pointer is, as the program was last told.
    at: Option<(u32, u32)>,
    /// Which event says it has left.
    leave: u16,
    /// Whether a frame ends what a pointer says, as from version 5.
    framed: bool,
}

/// The pointer held in place, or kept within a surface, as the program
/// asked and was told.
#[derive(Debug, Clone, Copy)]
struct Constraint {
    /// Held once only: once let go of, it is over.
    once: bool,
    /// Whether it is held now.
    held: bool,
    /// Whether it has been held.
    was: bool,
}

/// An input method of the program's, as it was told.
#[derive(Debug, Default, Clone, Copy)]
struct Method {
    /// How many times it has been told what it was told is whole: as the
    /// program counts, and as the compositor does, which counts from when
    /// it made it.
    done: (u32, u32),
    /// Whether it is typing in a field.
    active: bool,
}

/// Where a picture of the program's is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shot {
    /// Made, and not yet asked for.
    Made,
    /// Asked for, and neither taken nor failed.
    Asked,
    /// Taken, or failed.
    Told,
}

/// What a buffer has to be for a session's pictures, as the compositor
/// says it, ending with that it is whole.
#[derive(Debug, Default)]
struct Constraints {
    /// What the program was last told.
    told: Vec<Vec<u8>>,
    /// What the compositor has said since.
    coming: Vec<Vec<u8>>,
    /// Whether what comes is held back until it is whole, and passed on
    /// only if it is not as the program was told: the first the next
    /// compositor says.
    sifting: bool,
}

/// A window in a list of windows the program has (`ext-foreign-toplevel-list`).
#[derive(Debug, Default)]
struct Listing {
    /// The list it is in.
    list: u32,
    /// Its identifier, as the compositor gives it now.
    identifier: Vec<u8>,
    /// What the program was last told it is called, and what program it is
    /// of.
    title: Vec<u8>,
    app_id: Vec<u8>,
}

/// A window the next compositor lists that is kept from the program until
/// it is known whether the program has it listed already.
#[derive(Debug)]
struct Holding {
    /// The compositor's number for it.
    theirs: u32,
    /// The list it is in.
    list: u32,
    /// Its identifier, once said.
    identifier: Option<Vec<u8>>,
    /// What the compositor has said of it, from the event that lists it.
    said: Vec<Vec<u8>>,
}

/// A window of another program's that the program imported
/// (`xdg-foreign`).
#[derive(Debug)]
struct Import {
    /// What it was imported through.
    importer: u32,
    /// The handle it was imported by, or the one its window has been shared
    /// by since.
    handle: Vec<u8>,
    /// The window of the program's made to belong to it, if any.
    child: Option<u32>,
    renewal: Renewal,
}

/// Where a window imported is in being imported again under the next
/// compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Renewal {
    /// As the program imported it: its window not kept, or imported again.
    Done,
    /// Its window kept, until the next compositor shares it.
    Awaited,
    /// Shared anew before it was made again, to be imported again once it
    /// is.
    Known,
}

/// Where the relay is with a program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Joined to a compositor, passing everything on.
    Live,
    /// With no compositor.
    Waiting,
    /// Making the program's registries again.
    Registering,
    /// Making the rest again.
    Making,
    /// Caught up, waiting a turn of the compositor's to see what it gives
    /// back, while the program waits to be read: the relay's syncs take
    /// numbers it may take next.
    Settling,
    /// Caught up, looking through a list of windows of the relay's own for
    /// a window pictures are taken of, while the program waits to be read:
    /// the list has a number it may take next.
    Picturing,
}

/// Keeping a program through a change of compositor.
#[derive(Debug)]
pub struct Keeping {
    /// What the program has made.
    pub journal: Journal,
    phase: Phase,
    names: Renumbered,
    servers: Renumbered,
    /// What the compositor before made, which the program still holds and
    /// no compositor has.
    orphans: HashSet<u32>,
    /// The globals the program has been told of, by its names for them.
    globals: Vec<Global>,
    /// The globals the new compositor offers, as it says them.
    offered: Vec<Global>,
    /// The windows, by their `xdg_surface`, and the layer surfaces.
    windows: HashMap<u32, Window>,
    /// What has the pointer, the keyboard or a field, by the pointer,
    /// keyboard or field.
    focus: HashMap<u32, Focus>,
    /// What the pointer or the keyboard had as the compositor went, which
    /// the program is not told it left unless the next does not give it
    /// back.
    held: HashMap<u32, Focus>,
    /// The pointers whose next frame ends an enter kept from the program.
    framing: HashSet<u32>,
    /// How many syncs of the relay's own are still to be answered before
    /// what is held and not given back is taken to have left.
    settling: u8,
    /// The program's serials and the compositor's.
    serials: Serials,
    /// What each of the program's devices for the clipboard or the
    /// primary selection last set as it, and the compositor has not said
    /// is it no longer: the source, by the device.
    selections: HashMap<u32, u32>,
    /// Of those, the ones to be set again under the next compositor once
    /// it says there is none, as it does to a program given the keyboard.
    selecting: HashMap<u32, u32>,
    /// The buffer attached to each surface and not yet committed, by the
    /// surface: none where one was taken away.
    attached: HashMap<u32, u32>,
    /// The buffers shown and not yet released, which the compositor holds.
    held_buffers: HashSet<u32>,
    /// How many times each field has been committed: as the program counts,
    /// and as the compositor does, which counts from when it made it.
    commits: HashMap<u32, (u32, u32)>,
    /// What the program was last told of how something is, by the object and
    /// the event that said it.
    described: HashMap<(u32, u16), Vec<u8>>,
    /// The gestures under way, each with the event that says it has ended.
    gestures: HashMap<u32, u16>,
    /// The pointer's locks and confinements, by their own number.
    constraints: HashMap<u32, Constraint>,
    /// What watches for the user being away, that has been told they are.
    away: HashSet<u32>,
    /// The tokens to hand focus on that the program has been given, and the
    /// menus it has been told are over.
    given: HashSet<u32>,
    /// The windows in the program's lists of windows, by its number for
    /// each.
    listings: HashMap<u32, Listing>,
    /// Of those, the ones kept through a change of compositor, by their
    /// list and the window they are, the program's number and its own for
    /// the window's toplevel, until the next compositor lists that window.
    awaiting: HashMap<(u32, (u32, u32)), u32>,
    /// Of those, the ones the next compositor has listed, until what it
    /// says of each is whole: whether anything new has been told of it.
    relisted: HashMap<u32, bool>,
    /// While some are awaited, the window the next compositor lists by each
    /// identifier, where it is relayed.
    known: HashMap<Vec<u8>, Option<(u32, u32)>>,
    /// The windows the next compositor lists that are held back meanwhile,
    /// in the order listed.
    holding: Vec<Holding>,
    /// The compositor's numbers for those let through, which are not held
    /// back again as they are read again.
    unheld: HashSet<u32>,
    /// What the compositor said that was held back, to be read again in its
    /// place.
    pub again: Vec<u8>,
    /// The handle each window the program shares is shared by, as it was
    /// told it, by its number for what shares it.
    exports: HashMap<u32, Vec<u8>>,
    /// Each handle a window the program shares was shared by under the
    /// compositor before, with the one the next shares it by, for every
    /// program that imported it to import it again by.
    pub renamed: Vec<(Vec<u8>, Vec<u8>)>,
    /// The windows of other programs' the program imported, by its number
    /// for each.
    imports: HashMap<u32, Import>,
    /// The program's input methods, by its number for each.
    methods: HashMap<u32, Method>,
    /// The keys held, as each keyboard grab of an input method was told.
    grabbed: HashMap<u32, HashSet<u32>>,
    /// The keys held, as each keyboard was told.
    pressed: HashMap<u32, HashSet<u32>>,
    /// The input methods told another is the one, and the locks told they
    /// are over, and what each made, which are to go: not made again.
    refused: HashSet<u32>,
    /// The locks the program has been told have locked the session.
    locked: HashSet<u32>,
    /// The program's pictures, by its number for each.
    shots: HashMap<u32, Shot>,
    /// What a buffer has to be for each of the program's sessions of
    /// pictures, by its number for each.
    taking: HashMap<u32, Constraints>,
    /// Whether the pointer is over what each of the program's that follow
    /// it follows, by its number for each.
    following: HashMap<u32, bool>,
    /// What the program takes pictures of a window through, by its number
    /// for each, with what each is, and the source it is of: the source
    /// itself, and each session, picture and follower of the pointer made
    /// of it.
    of_windows: HashMap<u32, (&'static str, u32)>,
    /// The identifier each source of pictures of a window was made by, as
    /// the compositor lists the window, by the program's number for it.
    sources: HashMap<u32, Vec<u8>>,
    /// The windows pictures are taken of that are kept, by the source of
    /// them, while the next compositor does not have it.
    pictured: BTreeMap<u32, Pictured>,
    /// The relay's own list of windows, while it is looked through.
    finding: Option<Finding>,
    /// The highest of the program's numbers when the compositor went: what
    /// the program may take next may be any up to it.
    highest: u32,
    /// The numbers the new compositor has taken as what the program had is
    /// made again.
    numbering: Numbering,
    /// The relay's own syncs that end a part of that, not yet answered.
    syncs: Vec<u32>,
    /// The numbers of every sync of the relay's own whose answers, and
    /// whose numbers being freed, are still to be kept from the program.
    private: HashSet<u32>,
    /// What the relay is to send the compositor of its own accord.
    pub to_compositor: Vec<Sending>,
    /// What the relay is to tell the program of its own accord.
    pub to_program: Vec<Sending>,
}

impl Default for Keeping {
    fn default() -> Self {
        Self {
            journal: Journal::default(),
            phase: Phase::Live,
            names: Renumbered::default(),
            servers: Renumbered::default(),
            orphans: HashSet::new(),
            globals: Vec::new(),
            offered: Vec::new(),
            windows: HashMap::new(),
            focus: HashMap::new(),
            held: HashMap::new(),
            framing: HashSet::new(),
            settling: 0,
            serials: Serials::default(),
            selections: HashMap::new(),
            selecting: HashMap::new(),
            attached: HashMap::new(),
            held_buffers: HashSet::new(),
            commits: HashMap::new(),
            described: HashMap::new(),
            gestures: HashMap::new(),
            constraints: HashMap::new(),
            away: HashSet::new(),
            given: HashSet::new(),
            listings: HashMap::new(),
            awaiting: HashMap::new(),
            relisted: HashMap::new(),
            known: HashMap::new(),
            holding: Vec::new(),
            unheld: HashSet::new(),
            again: Vec::new(),
            exports: HashMap::new(),
            renamed: Vec::new(),
            imports: HashMap::new(),
            methods: HashMap::new(),
            grabbed: HashMap::new(),
            pressed: HashMap::new(),
            refused: HashSet::new(),
            locked: HashSet::new(),
            shots: HashMap::new(),
            taking: HashMap::new(),
            following: HashMap::new(),
            of_windows: HashMap::new(),
            sources: HashMap::new(),
            pictured: BTreeMap::new(),
            finding: None,
            highest: DISPLAY,
            numbering: Numbering::default(),
            syncs: Vec::new(),
            private: HashSet::new(),
            to_compositor: Vec::new(),
            to_program: Vec::new(),
        }
    }
}

impl Keeping {
    /// Whether what the program sends is passed on now: not while there is
    /// no compositor, or the one there is has still to catch up.
    #[must_use]
    pub fn live(&self) -> bool {
        self.phase == Phase::Live
    }

    /// Whether what the compositor says is taken as it is, as it is once
    /// the relay has caught up with it.
    #[must_use]
    pub fn caught_up(&self) -> bool {
        matches!(self.phase, Phase::Live | Phase::Settling | Phase::Picturing)
    }

    /// Takes in the request `bytes`, `described`, which `read` says the
    /// program sent; changes in it what means something only to the
    /// compositor before. Returns whether it is to be passed on.
    pub fn request(&mut self, read: &Read, bytes: &mut [u8], described: &MessageDesc) -> bool {
        let header = Header {
            object: wire::word(bytes, 0).unwrap_or_default(),
            opcode: read.opcode,
            size: bytes.len(),
        };
        self.made_or_ended(read, header.object, bytes);
        match (read.interface.name, described.name) {
            ("wl_surface", "attach") => {
                let buffer = wire::word(bytes, HEADER).unwrap_or_default();
                self.attached.insert(header.object, buffer);
            }
            ("wl_surface", "commit") => {
                let shown = self.attached.remove(&header.object);
                self.held_buffers
                    .extend(shown.filter(|buffer| *buffer != 0));
            }
            ("wl_buffer", "destroy") => {
                self.held_buffers.remove(&header.object);
            }
            ("wl_data_device" | "zwp_primary_selection_device_v1", "set_selection") => {
                let source = wire::word(bytes, HEADER).unwrap_or_default();
                self.selecting.remove(&header.object);
                self.selections.remove(&header.object);
                if source != 0 {
                    self.selections.insert(header.object, source);
                }
            }
            ("wl_data_source" | "zwp_primary_selection_source_v1", "destroy") => {
                let gone = |_: &u32, source: &mut u32| *source != header.object;
                self.selections.retain(gone);
                self.selecting.retain(gone);
            }
            ("zwp_input_method_v2", "commit") => self.committing(header.object, bytes),
            (interface, request) => self.importing(header.object, interface, request, bytes),
        }
        if read.ends && read.interface.name == "wl_surface" {
            // A surface gone has nothing to be said to have left.
            let gone = |_: &u32, focus: &mut Focus| focus.surface != header.object;
            self.focus.retain(gone);
            self.held.retain(gone);
        }
        let roled = matches!(
            read.interface.name,
            "xdg_surface" | "zwlr_layer_surface_v1" | "ext_session_lock_surface_v1"
        );
        if roled && described.name == "ack_configure" {
            let serial = wire::word(bytes, HEADER).unwrap_or_default();
            let window = self.windows.entry(header.object).or_default();
            if !window.offered.contains(&serial) {
                // Acked by the relay, or sent by the compositor before.
                return false;
            }
            window.offered.retain(|offered| *offered > serial);
        }
        if read.interface.name == "zwp_text_input_v3" && described.name == "commit" {
            let commits = self.commits.entry(header.object).or_default();
            *commits = (commits.0.wrapping_add(1), commits.1.wrapping_add(1));
        }
        if read.interface.name == "wl_registry" && described.name == "bind" {
            let name = wire::word(bytes, HEADER).unwrap_or_default();
            wire::put(bytes, HEADER, self.names.theirs(name));
        }
        let interface = read.interface.name;
        if let Some(at) = serials::carried(&serials::REQUESTS, interface, described.name) {
            // The pointer's cursor is set under its last enter's serial.
            let pointer = matches!(interface, "wl_pointer" | "wp_cursor_shape_device_v1");
            let ours = wire::word(bytes, HEADER + at).unwrap_or_default();
            wire::put(bytes, HEADER + at, self.serials.asked(ours, pointer));
        }
        if self.orphans.contains(&header.object) {
            if read.ends {
                self.orphans.remove(&header.object);
                // One of the program's own numbers no compositor has is the
                // relay's to free.
                if header.object < SERVERS {
                    let freed = journal::message(DISPLAY, DELETE_ID, &[header.object]);
                    self.to_program.push(freed);
                }
            }
            return false;
        }
        if header.object >= SERVERS {
            wire::put(bytes, 0, self.servers.theirs(header.object));
            if read.ends {
                self.servers.forget(header.object);
            }
        }
        let orphans = &self.orphans;
        let servers = &self.servers;
        let body = bytes.get_mut(HEADER..).unwrap_or_default();
        wire::renumber(body, described, &mut |number, _| {
            let ours = number >= SERVERS;
            (!orphans.contains(&number)).then(|| if ours { servers.theirs(number) } else { number })
        })
    }

    /// Takes in the request `bytes` to `object`, which `read` says it is,
    /// as far as it makes or destroys something: what was told of what had
    /// a number before is not of what has it now, and a lock or
    /// confinement of the pointer made is kept, as held once only or for
    /// as long as it lasts.
    fn made_or_ended(&mut self, read: &Read, object: u32, bytes: &[u8]) {
        self.picturing(read, object, bytes);
        if read.ends {
            self.constraints.remove(&object);
            self.away.remove(&object);
            self.listings.remove(&object);
            self.relisted.remove(&object);
            self.awaiting.retain(|_, handle| *handle != object);
            self.exports.remove(&object);
            self.imports.remove(&object);
            self.methods.remove(&object);
            self.grabbed.remove(&object);
            self.pressed.remove(&object);
            // A pointer, keyboard or field gone has nothing to be said to
            // have left.
            self.focus.remove(&object);
            self.held.remove(&object);
            self.framing.remove(&object);
            self.refused.remove(&object);
            self.locked.remove(&object);
            self.shots.remove(&object);
            self.taking.remove(&object);
            self.following.remove(&object);
            // What pictures of a window are taken through is made again
            // while something kept names it, as a source a session is of.
            if !self.journal.has(object) {
                self.of_windows.remove(&object);
            }
            // A window gone belongs to none imported any longer.
            for import in self.imports.values_mut() {
                import.child = import.child.filter(|child| *child != object);
            }
        }
        let Some((made, interface, _)) = read.made else {
            return;
        };
        self.described.retain(|(object, _), _| *object != made);
        self.commits.remove(&made);
        self.gestures.remove(&made);
        self.given.remove(&made);
        self.constraints.remove(&made);
        self.away.remove(&made);
        self.methods.remove(&made);
        self.grabbed.remove(&made);
        self.locked.remove(&made);
        // What an input method or a lock refused makes goes with it.
        if self.refused.contains(&object) {
            self.refused.insert(made);
        }
        if matches!(
            interface.name,
            "zwp_locked_pointer_v1" | "zwp_confined_pointer_v1"
        ) {
            let lifetime = wire::word(bytes, HEADER + 16).unwrap_or_default();
            let constraint = Constraint {
                once: lifetime == ONESHOT,
                held: false,
                was: false,
            };
            self.constraints.insert(made, constraint);
        }
    }

    /// Takes in the request `bytes` to `object`, which `read` says it is, as
    /// far as it makes what pictures are taken through, a picture, or asks
    /// for one: what is made of a window, or of what was, is a window's.
    fn picturing(&mut self, read: &Read, object: u32, bytes: &[u8]) {
        if read.interface.name == "ext_image_copy_capture_frame_v1" && read.opcode == CAPTURE {
            let _ = self.shots.get_mut(&object).map(|shot| *shot = Shot::Asked);
        }
        let Some((made, interface, _)) = read.made else {
            return;
        };
        self.shots.remove(&made);
        self.taking.remove(&made);
        self.following.remove(&made);
        self.of_windows.remove(&made);
        match interface.name {
            "ext_image_copy_capture_frame_v1" => {
                self.shots.insert(made, Shot::Made);
            }
            "ext_image_copy_capture_cursor_session_v1" => {
                self.following.insert(made, false);
            }
            _ => {}
        }
        let named = wire::word(bytes, HEADER + 4).unwrap_or_default();
        let source = match read.interface.name {
            "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                self.noted(made, named);
                Some(made)
            }
            "ext_image_copy_capture_manager_v1" => self.of_windows.get(&named).map(|of| of.1),
            _ => self.of_windows.get(&object).map(|of| of.1),
        };
        if let Some(source) = source {
            self.of_windows.insert(made, (interface.name, source));
            self.held_with(made, source);
        }
    }

    /// Takes in the request `bytes`, `request` of `interface`, to `object`,
    /// as far as it imports a window of another program's or makes one of
    /// the program's belong to it.
    fn importing(&mut self, object: u32, interface: &str, request: &str, bytes: &[u8]) {
        let body = bytes.get(HEADER..).unwrap_or_default();
        match (interface, request) {
            ("zxdg_importer_v2", "import_toplevel") => {
                let import = Import {
                    importer: object,
                    handle: wire::text(body, 4).to_vec(),
                    child: None,
                    renewal: Renewal::Done,
                };
                let imported = wire::word(body, 0).unwrap_or_default();
                self.imports.insert(imported, import);
            }
            ("zxdg_imported_v2", "set_parent_of") => {
                let import = self.imports.get_mut(&object);
                let _ = import.map(|import| import.child = wire::word(body, 0));
            }
            _ => {}
        }
    }

    /// Puts in `bytes`, the input method `method`'s commit, how many times
    /// it has been told what it was told is whole as the compositor counts,
    /// for as the program counts.
    fn committing(&self, method: u32, bytes: &mut [u8]) {
        let done = self.methods.get(&method).map(|method| method.done);
        let (said, heard) = done.unwrap_or_default();
        let serial = wire::word(bytes, HEADER).unwrap_or_default();
        wire::put(bytes, HEADER, serial.wrapping_sub(said.wrapping_sub(heard)));
    }

    /// Takes in the event `bytes`, of `header`, before it is read as the
    /// program's: the relay's own syncs answered, and what means something
    /// only to this compositor changed to what it means to the program.
    /// Returns its
    /// header as the program is to read it, or nothing if it is not to be
    /// passed on.
    ///
    /// # Errors
    ///
    /// Returns an error if what the program had cannot be made again, as a
    /// global it bound is no longer offered.
    pub fn before(
        &mut self,
        objects: &Objects,
        header: Header,
        bytes: &mut [u8],
    ) -> Result<Option<Header>, Broken> {
        let body = bytes.get(HEADER..).unwrap_or_default();
        if self.private.contains(&header.object) {
            // One of the relay's syncs, answered.
            if self.syncs.contains(&header.object) {
                self.answered(objects, header.object)?;
            } else {
                self.finding(objects, header.object, header.opcode, body);
            }
            return Ok(None);
        }
        if header.object == DISPLAY && header.opcode == 1 {
            let deleted = wire::word(body, 0).unwrap_or_default();
            if self.private.remove(&deleted) {
                return Ok(None);
            }
        }
        if self.hold(objects, header, bytes) {
            return Ok(None);
        }
        let object = if header.object >= SERVERS {
            self.servers.ours(header.object)
        } else {
            header.object
        };
        wire::put(bytes, 0, object);
        let interface = objects.living(object).map(|interface| interface.name);
        if interface == Some("wl_registry") {
            return Ok(self.registry(header, bytes));
        }
        if interface == Some("zwp_text_input_v3") && header.opcode == TEXT_DONE {
            // Said as the compositor counts the field's commits, it is told
            // as the program does.
            let (said, heard) = self.commits.get(&object).copied().unwrap_or_default();
            let serial = wire::word(bytes, HEADER).unwrap_or_default();
            wire::put(bytes, HEADER, serial.wrapping_add(said.wrapping_sub(heard)));
        }
        // One to no object, or with no such event, is found broken when it
        // is read as the program's.
        let described = objects.event_described(object, header.opcode);
        let (servers, orphans) = (&mut self.servers, &self.orphans);
        let body = bytes.get_mut(HEADER..).unwrap_or_default();
        let renumber = |described| {
            wire::renumber(body, described, &mut |number, makes| {
                if number < SERVERS {
                    return Some(number);
                }
                if !makes {
                    return Some(servers.ours(number));
                }
                // The program takes a new number only if it is the next past
                // those it has, or one freed: the lowest free is given it.
                let free = |ours: &u32| !orphans.contains(ours) && objects.free(*ours);
                let ours = Some(number).filter(free);
                let ours = ours
                    .or_else(|| (SERVERS..=u32::MAX).find(free))
                    .unwrap_or(number);
                servers.set(ours, number);
                Some(ours)
            })
        };
        let _ = described.map(renumber);
        let name = described
            .map(|described| described.name)
            .unwrap_or_default();
        let at =
            interface.and_then(|interface| serials::carried(&serials::EVENTS, interface, name));
        if let Some(at) = at {
            let theirs = wire::word(bytes, HEADER + at).unwrap_or_default();
            wire::put(bytes, HEADER + at, self.serials.told(theirs));
        }
        Ok(Some(Header { object, ..header }))
    }

    /// Takes in an event to a registry: a global offered or taken away,
    /// named as the program names it, and kept from it while the relay
    /// catches up.
    fn registry(&mut self, header: Header, bytes: &mut [u8]) -> Option<Header> {
        let body = bytes.get(HEADER..).unwrap_or_default();
        let name = wire::word(body, 0).unwrap_or_default();
        if header.opcode == 0 {
            let length =
                usize::try_from(wire::word(body, 4).unwrap_or_default()).unwrap_or_default();
            let interface = body
                .get(8..8 + length.saturating_sub(1))
                .unwrap_or_default()
                .to_vec();
            let version = wire::word(body, 8 + length.next_multiple_of(4)).unwrap_or_default();
            let global = Global {
                name,
                interface,
                version,
            };
            if !self.caught_up() {
                if !self.offered.contains(&global) {
                    self.offered.push(global);
                }
                return None;
            }
            // Told to each registry the program has, as a library it
            // draws with makes one of its own: the same global, by the
            // same name, unless the one it was is gone and its name taken.
            let globals = &self.globals;
            let known = self.names.known(name);
            let same = known.filter(|ours| globals.iter().any(|known| known.name == *ours));
            let ours = same.unwrap_or_else(|| {
                if let Some(gone) = known {
                    self.names.forget(gone);
                }
                let ours = self.name_for(name);
                // Its name may have been another's, taken away.
                self.names.forget(ours);
                self.names.set(ours, name);
                self.globals.push(Global {
                    name: ours,
                    ..global
                });
                ours
            });
            wire::put(bytes, HEADER, ours);
        } else {
            // Each registry is told, so the name is kept until a new
            // global takes it, on either side.
            let ours = self.names.ours(name);
            self.globals.retain(|global| global.name != ours);
            wire::put(bytes, HEADER, ours);
            if !self.caught_up() {
                return None;
            }
        }
        Some(header)
    }

    /// The program's name for the global the compositor names `name`: the
    /// same, unless the program knows another global by it; then one past
    /// the highest it knows, or, with no name past that, the lowest it does
    /// not know. A name is never 0.
    fn name_for(&self, name: u32) -> u32 {
        let taken = |name: u32| self.globals.iter().any(|global| global.name == name);
        if !taken(name) {
            return name;
        }
        let names = self.globals.iter().map(|global| global.name);
        let past = names.max().and_then(|highest| highest.checked_add(1));
        past.unwrap_or_else(|| (1..u32::MAX).find(|name| !taken(*name)).unwrap_or(name))
    }

    /// Takes in the event `bytes`, which `read` says it is, once read as
    /// the program's. Returns whether it is to be passed on, after what the
    /// relay is to tell the program of its own accord.
    pub fn after(&mut self, read: &Read, bytes: &[u8]) -> bool {
        let header = wire::header(bytes).ok().flatten();
        let object = header.map(|header| header.object).unwrap_or_default();
        let body = bytes.get(HEADER..).unwrap_or_default();
        let word = |at| wire::word(body, at).unwrap_or_default();
        let catching = !self.caught_up();
        match (read.interface.name, read.opcode) {
            ("wl_pointer", 0) => {
                self.serials.entered();
                let at = Some((word(8), word(12)));
                return self.entered(object, word(4), Some(word(0)), at, true, read);
            }
            ("wl_pointer", 2) => {
                let moved = self.focus.get_mut(&object);
                let _ = moved.map(|focus| focus.at = Some((word(4), word(8))));
            }
            ("wl_pointer", 5) if self.framing.remove(&object) => return false,
            ("wl_keyboard", opcode) => return self.keyboard(object, opcode, body, read),
            ("zwp_text_input_v3", 0) => self.focused(object, word(0), None, None, read),
            ("wl_pointer" | "zwp_text_input_v3", 1) => {
                self.focus.remove(&object);
            }
            // What the next compositor first says of a window, before it is
            // shown again, is the relay's to answer: a window not yet shown
            // has no size and no focus, which a program takes for focus
            // gone. What it says once the window is shown is told.
            ("xdg_toplevel", 0) if catching => return false,
            ("xdg_surface", 0) => return self.configured(object, word(0), None, WINDOW_ACK),
            ("zwlr_layer_surface_v1", 0) => {
                let size = Some((word(4), word(8)));
                return self.configured(object, word(0), size, LAYER_ACK);
            }
            ("ext_session_lock_surface_v1", 0) => {
                let size = Some((word(4), word(8)));
                return self.configured(object, word(0), size, LOCK_ACK);
            }
            ("ext_session_lock_v1", opcode) => return self.locking(object, opcode),
            ("zwp_pointer_gesture_swipe_v1" | "zwp_pointer_gesture_pinch_v1", 0) => {
                self.gestures.insert(object, 2);
            }
            ("zwp_pointer_gesture_hold_v1", 0) => {
                self.gestures.insert(object, 1);
            }
            ("zwp_pointer_gesture_swipe_v1" | "zwp_pointer_gesture_pinch_v1", 2)
            | ("zwp_pointer_gesture_hold_v1", 1) => {
                self.gestures.remove(&object);
            }
            ("wl_buffer", 0) => {
                self.held_buffers.remove(&object);
            }
            ("ext_foreign_toplevel_list_v1", 0) => {
                let listing = Listing {
                    list: object,
                    ..Listing::default()
                };
                self.listings.insert(word(0), listing);
            }
            ("ext_foreign_toplevel_handle_v1", opcode) => {
                return self.listing(object, opcode, body);
            }
            ("zxdg_exported_v2", HANDLE) => self.shared(object, wire::text(body, 0)),
            ("zxdg_imported_v2", IMPORT_GONE) => {
                // Refused by the handle of the compositor before, it is
                // imported again once its window is shared anew.
                let import = self.imports.get(&object);
                return import.is_none_or(|import| import.renewal != Renewal::Awaited);
            }
            ("zwp_input_method_v2" | "zwp_input_method_keyboard_grab_v2", opcode) => {
                self.typing(object, read.interface.name, opcode, body);
            }
            (interface, opcode) if interface.starts_with("ext_image_copy_capture") => {
                return self.pictured(object, interface, opcode, bytes);
            }
            ("ext_idle_notification_v1", 0) => {
                self.away.insert(object);
            }
            ("ext_idle_notification_v1", _) => {
                self.away.remove(&object);
            }
            ("zwp_locked_pointer_v1" | "zwp_confined_pointer_v1", opcode) => {
                let constraint = self.constraints.get_mut(&object);
                let _ = constraint.map(|constraint| {
                    constraint.held = opcode == 0;
                    constraint.was |= constraint.held;
                });
            }
            ("wl_data_source", 2) | ("zwp_primary_selection_source_v1", 1) => {
                self.selections.retain(|_, source| *source != object);
            }
            ("wl_data_device", 5) | ("zwp_primary_selection_device_v1", 1) => {
                return self.selected(read, object, word(0));
            }
            ("xdg_activation_token_v1", 0) | ("xdg_popup", 1) => {
                self.given.insert(object);
            }
            ("wl_shm", 0) | ("wl_seat" | "wl_output" | "zxdg_output_v1", _) => return !catching,
            (interface, opcode) if let Some(changes) = describes(interface, opcode) => {
                // Told again as it was, while the relay catches up or of
                // what is told only as it changes, it is not told at all.
                let same = self.described.get(&(object, opcode)).map(Vec::as_slice) == Some(body);
                self.described.insert((object, opcode), body.to_vec());
                return !(same && (catching || changes));
            }
            _ => {}
        }
        true
    }

    /// Takes in the event `opcode`, with `body`, to `object`, of `interface`:
    /// an input method's, as far as it says whether it is typing in a field,
    /// that what it has been told is whole, or that another is the one; or
    /// its keyboard grab's, as far as it says which keys are held.
    fn typing(&mut self, object: u32, interface: &str, opcode: u16, body: &[u8]) {
        let word = |at| wire::word(body, at).unwrap_or_default();
        match (interface, opcode) {
            ("zwp_input_method_keyboard_grab_v2", GRABBED_KEY) => {
                let held = self.grabbed.entry(object).or_default();
                let (key, pressed) = (word(8), word(12) != 0);
                if pressed {
                    held.insert(key);
                } else {
                    held.remove(&key);
                }
            }
            ("zwp_input_method_v2", ACTIVATE | DEACTIVATE) => {
                self.methods.entry(object).or_default().active = opcode == ACTIVATE;
            }
            ("zwp_input_method_v2", TYPING_DONE) => {
                let method = self.methods.entry(object).or_default();
                method.done = (method.done.0.wrapping_add(1), method.done.1.wrapping_add(1));
            }
            ("zwp_input_method_v2", UNAVAILABLE) => {
                self.refused.insert(object);
            }
            _ => {}
        }
    }

    /// Takes in the event `bytes`, `opcode` of `interface`, to `object`, as
    /// far as it says a picture is taken or failed, what a buffer has to be
    /// for a session's, or that the pointer came over what is followed or
    /// left it. Returns whether it is passed on.
    fn pictured(&mut self, object: u32, interface: &str, opcode: u16, bytes: &[u8]) -> bool {
        match (interface, opcode) {
            ("ext_image_copy_capture_frame_v1", SHOT_READY | SHOT_FAILED) => {
                let _ = self.shots.get_mut(&object).map(|shot| *shot = Shot::Told);
            }
            ("ext_image_copy_capture_cursor_session_v1", POINTER_IN | POINTER_OUT) => {
                self.following.insert(object, opcode == POINTER_IN);
            }
            ("ext_image_copy_capture_session_v1", opcode) if opcode <= CONSTRAINED => {
                return self.constrained(object, opcode, bytes);
            }
            _ => {}
        }
        true
    }

    /// Takes in the event `bytes`, `opcode`, of the session `session`, that
    /// says what a buffer has to be for its pictures, or that that is whole.
    /// The first the next compositor says is held back until it is whole,
    /// and passed on only if it is not as the program was told, for a
    /// program may take it as the screen changed. Returns whether it is
    /// passed on.
    fn constrained(&mut self, session: u32, opcode: u16, bytes: &[u8]) -> bool {
        let constraints = self.taking.entry(session).or_default();
        if opcode != CONSTRAINED {
            constraints.coming.push(bytes.to_vec());
            return !constraints.sifting;
        }
        let coming = std::mem::take(&mut constraints.coming);
        let sifted = std::mem::take(&mut constraints.sifting);
        let same = coming == constraints.told;
        if sifted && !same {
            let held = coming.iter().map(|said| (said.clone(), Vec::new()));
            self.to_program.extend(held);
        }
        constraints.told = coming;
        !(sifted && same)
    }

    /// Lets go of what the program takes pictures through, as the compositor
    /// goes. A picture asked for is told it failed, as one may, for the
    /// program to ask for another; it and every picture told of go into
    /// `spent`, and the rest is made again: what pictures of a window are
    /// taken through once the window is back. A pointer followed is told it
    /// left. What a buffer has to be is told again only if it changed.
    fn let_go_of_pictures(&mut self, spent: &mut HashSet<u32>) {
        let mut shots: Vec<(u32, Shot)> = self.shots.iter().map(|(n, s)| (*n, *s)).collect();
        shots.sort_unstable_by_key(|(number, _)| *number);
        for (frame, shot) in shots {
            if shot == Shot::Asked {
                self.to_program
                    .push(journal::message(frame, SHOT_FAILED, &[UNKNOWN]));
            }
            if shot != Shot::Made {
                self.shots.remove(&frame);
                self.of_windows.remove(&frame);
                spent.insert(frame);
            }
        }
        let mut following: Vec<(&u32, &mut bool)> = self.following.iter_mut().collect();
        following.sort_unstable_by_key(|(number, _)| **number);
        for (number, over) in following {
            if std::mem::take(over) {
                self.to_program
                    .push(journal::message(*number, POINTER_OUT, &[]));
            }
        }
        for constraints in self.taking.values_mut() {
            constraints.coming.clear();
            constraints.sifting = true;
        }
    }

    /// Takes in the event `opcode` to the lock `object`; returns whether it
    /// is passed on.
    fn locking(&mut self, object: u32, opcode: u16) -> bool {
        // Told once that the session is locked, the locker is not told again
        // as the next compositor locks it with the lock made again.
        if opcode == LOCKED {
            return self.locked.insert(object);
        }
        // Else over, or refused, as a second locker is: the next compositor
        // would refuse it as well, and the program has been told.
        let made = self.journal.made_by(object);
        self.refused.insert(object);
        self.refused.extend(made);
        true
    }

    /// Lets each input method typing in a field go of it, as the compositor
    /// goes, as when the field loses the keyboard: what it was composing
    /// cannot be carried. A key held in its keyboard grab is let go of, as
    /// no compositor will say so. The next compositor counts what it tells
    /// each from none.
    fn let_go_of_typing(&mut self) {
        let mut typing = Vec::new();
        for (number, method) in &mut self.methods {
            if method.active {
                method.active = false;
                method.done.0 = method.done.0.wrapping_add(1);
                typing.push(*number);
            }
            method.done.1 = 0;
        }
        typing.sort_unstable();
        for method in typing {
            self.to_program
                .push(journal::message(method, DEACTIVATE, &[]));
            self.to_program
                .push(journal::message(method, TYPING_DONE, &[]));
        }
        let held = self.grabbed.drain();
        let held = held.flat_map(|(grab, keys)| keys.into_iter().map(move |key| (grab, key)));
        let mut held: Vec<(u32, u32)> = held.collect();
        held.sort_unstable();
        for (grab, key) in held {
            // With no serial and no time, as a gesture cancelled is told.
            let let_go = journal::message(grab, GRABBED_KEY, &[0, 0, key, 0]);
            self.to_program.push(let_go);
        }
    }

    /// Takes in that the compositor said, as `read` says, that `offer` is
    /// what `device` has as the selection: none, where it is 0. What the
    /// program selected before is selected again if nothing else is, and
    /// that there is none is kept from it; if something else is, its source
    /// is told it is the selection no longer. Returns whether it is passed
    /// on.
    fn selected(&mut self, read: &Read, device: u32, offer: u32) -> bool {
        let Some(source) = self.selecting.remove(&device) else {
            return true;
        };
        // The request that sets it and the event that says the source is
        // the selection no longer, of the clipboard's and the primary
        // selection's.
        let (set, cancelled) = match read.interface.name {
            "wl_data_device" => (1, 2),
            _ => (0, 1),
        };
        if offer == 0 {
            // With its serial second, which Smithay does not check: the
            // keyboard is what it asks.
            let latest = self.serials.latest();
            self.to_compositor
                .push(journal::message(device, set, &[source, latest]));
            return false;
        }
        self.selections.remove(&device);
        self.to_program
            .push(journal::message(source, cancelled, &[]));
        true
    }

    /// Takes in the event `opcode`, with `body`, to `handle`, a window in a
    /// list of the program's. Returns whether it is passed on: not what is
    /// said again of one the next compositor has listed, where it has not
    /// changed, nor that it is whole where nothing has.
    fn listing(&mut self, handle: u32, opcode: u16, body: &[u8]) -> bool {
        let Some(listing) = self.listings.get_mut(&handle) else {
            return true;
        };
        let said = match opcode {
            CLOSED => {
                self.listings.remove(&handle);
                self.relisted.remove(&handle);
                return true;
            }
            LISTED_DONE => return self.relisted.remove(&handle).unwrap_or(true),
            TITLE => &mut listing.title,
            APP_ID => &mut listing.app_id,
            _ => {
                listing.identifier = wire::text(body, 0).to_vec();
                return true;
            }
        };
        let same = said.as_slice() == body;
        body.clone_into(said);
        let relisted = self.relisted.get_mut(&handle);
        let Some(told) = relisted else {
            return true;
        };
        *told |= !same;
        !same
    }

    /// Takes in the event `bytes`, of `header`, if it lists a window anew
    /// while a window the program had listed under the compositor before is
    /// still to be listed again, or is of one listed so: holds it back until
    /// the window it is, is known. Returns whether it is held.
    fn hold(&mut self, objects: &Objects, header: Header, bytes: &[u8]) -> bool {
        let mut holding = self.holding.iter_mut();
        if let Some(held) = holding.find(|held| held.theirs == header.object) {
            if header.opcode == IDENTIFIER && held.said.len() == 1 {
                let body = bytes.get(HEADER..).unwrap_or_default();
                held.identifier = Some(wire::text(body, 0).to_vec());
            }
            held.said.push(bytes.to_vec());
            self.resolve();
            return true;
        }
        let list = objects.living(header.object);
        let lists = list.is_some_and(|list| list.name == "ext_foreign_toplevel_list_v1");
        if !lists || header.opcode != 0 || self.awaiting.is_empty() {
            return false;
        }
        let theirs = wire::word(bytes, HEADER).unwrap_or_default();
        if self.unheld.remove(&theirs) {
            return false;
        }
        self.holding.push(Holding {
            theirs,
            list: header.object,
            identifier: None,
            said: vec![bytes.to_vec()],
        });
        true
    }

    /// Takes in that the compositor lists, by `identifier`, the window
    /// `window`, where it is relayed: if the program has that window
    /// listed from before, the window held back as that identifier is the
    /// one it has.
    pub fn listed(&mut self, identifier: &[u8], window: Option<(u32, u32)>) {
        self.picture_listed(identifier, window);
        if !self.awaiting.is_empty() {
            self.known.insert(identifier.to_vec(), window);
            self.resolve();
        }
    }

    /// Takes in that the compositor lists the window it identified by
    /// `identifier` no longer.
    pub fn unlisted(&mut self, identifier: &[u8]) {
        self.known.remove(identifier);
    }

    /// Lets go of each window held back that is known now: one the program
    /// has listed from before is the one it has, and it is told only what
    /// follows of it; another is new, and told as the compositor said it.
    fn resolve(&mut self) {
        let mut holding = std::mem::take(&mut self.holding);
        let known = &self.known;
        // One that says something before its identifier is not known by it.
        let window = |held: &Holding| match &held.identifier {
            Some(identifier) => known.get(identifier).copied(),
            None => (held.said.len() > 1).then_some(None),
        };
        for held in holding.extract_if(.., |held| window(held).is_some()) {
            let window = window(&held).flatten();
            let had = window.and_then(|window| self.awaiting.remove(&(held.list, window)));
            let Some(had) = had else {
                self.unheld.insert(held.theirs);
                self.again.extend(held.said.into_iter().flatten());
                continue;
            };
            // The program's own is the next compositor's: neither the event
            // that lists it nor its identifier is told again.
            self.servers.set(had, held.theirs);
            self.orphans.remove(&had);
            self.relisted.insert(had, false);
            let listing = self.listings.get_mut(&had);
            let identifier = held.identifier.unwrap_or_default();
            let _ = listing.map(|listing| listing.identifier = identifier);
            self.again.extend(held.said.into_iter().skip(2).flatten());
        }
        self.holding = holding;
        if self.awaiting.is_empty() {
            self.known.clear();
        }
    }

    /// Takes in which window the compositor that has gone listed by each
    /// identifier, `listed`, where it is relayed, and which of those are
    /// kept, `kept`: each window in the program's lists that is kept is
    /// awaited from the next compositor, and the program is told the rest
    /// have closed.
    pub fn unlist(
        &mut self,
        objects: &Objects,
        listed: &BTreeMap<String, Option<(u32, u32)>>,
        kept: &HashSet<(u32, u32)>,
    ) {
        self.await_pictures(objects, listed, kept);
        let mut listings: Vec<(&u32, &Listing)> = self.listings.iter().collect();
        listings.sort_unstable_by_key(|(handle, _)| **handle);
        let mut closed = Vec::new();
        for (handle, listing) in listings {
            let identifier = std::str::from_utf8(&listing.identifier).ok();
            let window = identifier.and_then(|identifier| listed.get(identifier));
            let window = window
                .copied()
                .flatten()
                .filter(|window| kept.contains(window));
            match window {
                Some(window) => {
                    self.awaiting.insert((listing.list, window), *handle);
                }
                None => closed.push(*handle),
            }
        }
        for handle in closed {
            self.listings.remove(&handle);
            self.to_program.push(journal::message(handle, CLOSED, &[]));
        }
    }

    /// Takes in that the window the program shares as `exported` is shared
    /// by `handle`: where it was shared by another, under the compositor
    /// before, every program that imported it by that is to import it again
    /// by this.
    fn shared(&mut self, exported: u32, handle: &[u8]) {
        let before = self.exports.insert(exported, handle.to_vec());
        let renamed = before.map(|before| (before, handle.to_vec()));
        self.renamed.extend(renamed);
    }

    /// The handles the windows the program shares are shared by, as it was
    /// told them.
    pub fn exports(&self) -> impl Iterator<Item = &[u8]> {
        self.exports.values().map(Vec::as_slice)
    }

    /// Takes in the handles the compositor that has gone shared the windows
    /// that are kept by, `kept`: each window the program imported by one is
    /// awaited, to be imported again once the next compositor shares it.
    pub fn unimport(&mut self, kept: &HashSet<Vec<u8>>) {
        for import in self.imports.values_mut() {
            import.renewal = if kept.contains(&import.handle) {
                Renewal::Awaited
            } else {
                Renewal::Done
            };
        }
    }

    /// Takes in that the window the compositor that has gone shared by
    /// `before` is shared by `now`: each the program imported by `before`,
    /// and awaits, is imported again by `now` once it is made again.
    pub fn exported(&mut self, before: &[u8], now: &[u8]) {
        for import in self.imports.values_mut() {
            if import.renewal == Renewal::Awaited && import.handle == before {
                now.clone_into(&mut import.handle);
                import.renewal = Renewal::Known;
            }
        }
        if matches!(self.phase, Phase::Making | Phase::Settling | Phase::Live) {
            self.reimport();
        }
    }

    /// Imports again each window shared anew, as the program last made it.
    /// What was imported by the handle before, which the compositor refused,
    /// is destroyed first, and the number that frees is the relay's to keep
    /// from the program, as the program has it still.
    fn reimport(&mut self) {
        let known = self.imports.iter_mut();
        let mut known: Vec<(&u32, &mut Import)> = known
            .filter(|(_, import)| import.renewal == Renewal::Known)
            .collect();
        known.sort_unstable_by_key(|(imported, _)| **imported);
        for (imported, import) in known {
            import.renewal = Renewal::Done;
            self.private.insert(*imported);
            let (importer, handle) = (import.importer, &import.handle);
            let again = journal::said_after(importer, IMPORT, &[*imported], handle);
            self.to_compositor
                .push(journal::message(*imported, UNIMPORT, &[]));
            self.to_compositor.push(again);
            let parent = import
                .child
                .map(|child| journal::message(*imported, PARENT_OF, &[child]));
            self.to_compositor.extend(parent);
        }
    }

    /// Takes in the event `opcode`, with `body`, to `keyboard`, which `read`
    /// says it is, as far as it says which surface the keyboard has and
    /// which keys it holds: those it entered with, each pressed and not let
    /// go of since, and none once it has left. Returns whether it is passed
    /// on.
    fn keyboard(&mut self, keyboard: u32, opcode: u16, body: &[u8], read: &Read) -> bool {
        let word = |at| wire::word(body, at).unwrap_or_default();
        match opcode {
            ENTER => {
                let keys = usize::try_from(word(8)).unwrap_or_default();
                let keys = (12..12 + keys).step_by(4).map(word);
                self.pressed.insert(keyboard, keys.collect());
                // A key held as it enters is the program's to be told of.
                let clear = word(8) == 0;
                return self.entered(keyboard, word(4), Some(word(0)), None, clear, read);
            }
            LEAVE => {
                self.focus.remove(&keyboard);
                self.pressed.remove(&keyboard);
            }
            // Pressed, or repeated, as from version 10; or let go of.
            KEY if word(12) == 0 => {
                self.pressed.entry(keyboard).or_default().remove(&word(8));
            }
            KEY => {
                self.pressed.entry(keyboard).or_default().insert(word(8));
            }
            _ => {}
        }
        true
    }

    /// Lets go of each key a keyboard holds, as the compositor goes, as no
    /// compositor will say so: the program repeats a key held itself, and
    /// would type it on, as the keyboard back where it was is kept from it.
    fn let_go_of_keys(&mut self) {
        let held = self.pressed.drain();
        let held =
            held.flat_map(|(keyboard, keys)| keys.into_iter().map(move |key| (keyboard, key)));
        let mut held: Vec<(u32, u32)> = held.collect();
        held.sort_unstable();
        let last = self.serials.last();
        for (keyboard, key) in held {
            // Under the last serial the program was told, as a leave is,
            // and with no time.
            let let_go = journal::message(keyboard, KEY, &[last, 0, key, 0]);
            self.to_program.push(let_go);
        }
    }

    /// Takes in that `device`, a pointer or keyboard, entered `surface`
    /// with `serial`, at `at` and with nothing held if `clear`, as `read`
    /// says. Where it had that surface as the compositor went, there and
    /// with nothing held, the enter is kept from the program; where it had
    /// another, the program is told it left that first. Returns whether
    /// the enter is passed on.
    fn entered(
        &mut self,
        device: u32,
        surface: u32,
        serial: Option<u32>,
        at: Option<(u32, u32)>,
        clear: bool,
        read: &Read,
    ) -> bool {
        if let Some(held) = self.held.remove(&device) {
            if held.surface == surface && held.at == at && clear {
                // Its frame, which ends the enter, goes with it.
                if held.framed {
                    self.framing.insert(device);
                }
                self.focus.insert(device, held);
                return false;
            }
            self.left(device, held);
        }
        self.focused(device, surface, serial, at, read);
        true
    }

    /// Keeps that `surface` has `device`'s focus, as `read`'s enter says,
    /// with `serial`, and at `at`.
    fn focused(
        &mut self,
        device: u32,
        surface: u32,
        serial: Option<u32>,
        at: Option<(u32, u32)>,
        read: &Read,
    ) {
        let interface = read.interface.name;
        let framed = interface == "wl_pointer" && read.interface.version >= 5;
        let leave = if interface == "wl_keyboard" { 2 } else { 1 };
        let focus = Focus {
            surface,
            serial,
            at,
            leave,
            framed,
        };
        self.focus.insert(device, focus);
    }

    /// Tells the program that `device` has left what `focus` says it had,
    /// under the last serial it was told, if it says one.
    fn left(&mut self, device: u32, focus: Focus) {
        let serial = focus.serial.map(|_| self.serials.last());
        let arguments: Vec<u32> = serial.into_iter().chain([focus.surface]).collect();
        self.to_program
            .push(journal::message(device, focus.leave, &arguments));
        if focus.framed {
            self.to_program.push(journal::message(device, 5, &[]));
        }
    }

    /// Takes in that the compositor has said how the window `role`, its
    /// `xdg_surface`, or the layer surface or lock surface `role` that says
    /// its `size`, is to be, with `serial`, which `ack` acks. Returns whether
    /// it is passed on.
    fn configured(&mut self, role: u32, serial: u32, size: Option<(u32, u32)>, ack: u16) -> bool {
        let caught_up = self.caught_up();
        let window = self.windows.entry(role).or_default();
        let told = window.size;
        window.size = size;
        // What a lock surface showed may be shown only at the size it was
        // drawn for: told another, it is the locker's to ack and draw anew.
        let drawn_anew = ack == LOCK_ACK && size != told;
        if caught_up || drawn_anew {
            window.offered.push(serial);
            return true;
        }
        // Catching up, the relay acks it and shows what was shown.
        let theirs = self.serials.asked(serial, false);
        self.to_compositor
            .push(journal::message(role, ack, &[theirs]));
        self.to_compositor.extend(self.journal.shown(role));
        // A layer surface told another size, as its screen may be, is told
        // so: the relay has acked it already, and the program's ack is kept
        // from the compositor.
        size.is_some() && size != told
    }

    /// Takes in that the relay's sync `sync` is answered: the registries
    /// made again, the rest is; the rest made again, the program goes on.
    fn answered(&mut self, objects: &Objects, sync: u32) -> Result<(), Broken> {
        self.syncs.retain(|waiting| *waiting != sync);
        if self.phase == Phase::Registering {
            self.rename(objects);
            let names = &self.names;
            let globals = &self.globals;
            let rename = |ours: u32| {
                globals
                    .iter()
                    .any(|global| global.name == ours)
                    .then(|| names.theirs(ours))
            };
            let (mut sending, _) = self.journal.replay(&rename, &mut self.numbering)?;
            self.numbering.fill_to(&mut sending, self.highest);
            self.private.extend(self.numbering.fillers.drain(..));
            self.to_compositor.extend(sending);
            self.reimport();
            self.sync();
            self.phase = Phase::Making;
        } else if self.phase == Phase::Making {
            // The rest made again, as the last sync is answered.
            self.to_compositor.extend(self.journal.gone());
            self.phase = Phase::Live;
            if !self.held.is_empty() {
                self.phase = Phase::Settling;
                self.settling = SETTLING;
                self.sync();
            }
        } else {
            self.settle();
        }
        self.find(objects);
        Ok(())
    }

    /// Takes in that one of the relay's syncs after it caught up is
    /// answered: after the last, what the next compositor has not given
    /// back has left.
    fn settle(&mut self) {
        self.settling = self.settling.saturating_sub(1);
        if self.settling > 0 {
            self.sync();
            return;
        }
        let mut held: Vec<(u32, Focus)> = self.held.drain().collect();
        held.sort_by_key(|(device, _)| *device);
        for (device, focus) in held {
            self.left(device, focus);
        }
        self.phase = Phase::Live;
    }

    /// Names the new compositor's globals as the program knows them, tells
    /// the program of those it does not, and takes away those no longer
    /// offered.
    fn rename(&mut self, objects: &Objects) {
        self.names = Renumbered::default();
        let mut known: Vec<Global> = std::mem::take(&mut self.globals);
        let mut new = Vec::new();
        for global in std::mem::take(&mut self.offered) {
            let same = known
                .iter()
                .position(|known| known.interface == global.interface);
            match same {
                Some(at) => {
                    let known = known.remove(at);
                    self.names.set(known.name, global.name);
                    self.globals.push(Global {
                        name: known.name,
                        ..global
                    });
                }
                None => new.push(global),
            }
        }
        let registries: Vec<u32> = objects
            .each_living()
            .filter(|(_, interface)| interface.name == "wl_registry")
            .map(|(number, _)| number)
            .collect();
        for gone in &known {
            for registry in &registries {
                self.to_program
                    .push(journal::message(*registry, 1, &[gone.name]));
            }
        }
        for global in new {
            let ours = self.name_for(global.name);
            self.names.set(ours, global.name);
            for registry in &registries {
                self.to_program.push(offer(*registry, ours, &global));
            }
            self.globals.push(Global {
                name: ours,
                ..global
            });
        }
    }

    /// A sync of the relay's own under the program's number `number`, for
    /// what the relay answers for itself and the compositor is never told
    /// of. A compositor takes for a new object only the next number or one
    /// it has freed, so the number is taken up there all the same, and what
    /// the compositor says of it is kept from the program.
    pub fn stand_in(&mut self, number: u32) -> Sending {
        self.private.insert(number);
        journal::message(DISPLAY, 0, &[number])
    }

    /// Sends a sync of the relay's own, to know when the compositor has
    /// caught up with what was sent before it.
    fn sync(&mut self) {
        let sync = self.numbering.take_next();
        self.private.insert(sync);
        self.syncs.push(sync);
        self.to_compositor
            .push(journal::message(DISPLAY, 0, &[sync]));
    }

    /// Takes in that the compositor has gone: what it made is no one's, the
    /// program is told that the pointer and keyboard have left its
    /// surfaces, and it waits.
    ///
    /// # Errors
    ///
    /// Returns the interfaces of what the program holds that cannot be
    /// made again, when there is any, as it cannot be kept.
    pub fn lost(&mut self, objects: &Objects) -> Result<(), Vec<&'static str>> {
        let unkept = self.journal.unkept();
        if !unkept.is_empty() {
            return Err(unkept);
        }
        self.highest = objects.highest();
        // What lasts only a moment ends with the compositor, as it would have
        // told the program, and the relay answers for it from now on.
        // A pointer held is let go of, as when focus leaves, and held again
        // once the next compositor gives it back, as it would be then; but
        // one held once only is over.
        let mut constraints: Vec<(u32, Constraint)> = self.constraints.drain().collect();
        constraints.sort_unstable_by_key(|(number, _)| *number);
        let mut spent = HashSet::new();
        for (number, constraint) in constraints {
            if constraint.held {
                self.to_program.push(journal::message(number, 1, &[]));
            }
            if constraint.once && constraint.was {
                spent.insert(number);
            } else {
                let constraint = Constraint {
                    held: false,
                    ..constraint
                };
                self.constraints.insert(number, constraint);
            }
        }
        // The next compositor counts how long the user has been away from
        // when it starts, and would not say they are back if they come back
        // before it says they are away.
        let mut away: Vec<u32> = self.away.drain().collect();
        away.sort_unstable();
        for watching in away {
            self.to_program.push(journal::message(watching, 1, &[]));
        }
        self.let_go_of_typing();
        self.let_go_of_keys();
        // An input method told another is the one is to go, and is not
        // made again, nor what it made; the relay answers for each.
        spent.extend(self.refused.drain());
        self.let_go_of_pictures(&mut spent);
        self.hold_pictures(objects);
        let mut menu = false;
        for (number, interface) in self.journal.end_lost(&spent) {
            let untold = !self.given.remove(&number);
            match interface {
                "xdg_popup" if untold => {
                    self.to_program.push(journal::message(number, 1, &[]));
                    menu = true;
                }
                "xdg_activation_token_v1" if untold => {
                    // A token no compositor will take.
                    self.to_program.push(journal::said(number, 0, ""));
                }
                "wp_presentation_feedback" => {
                    self.to_program.push(journal::message(number, 2, &[]));
                    self.to_program
                        .push(journal::message(DISPLAY, DELETE_ID, &[number]));
                    continue;
                }
                _ => {}
            }
            self.orphans.insert(number);
        }
        // What the program selected is selected again once the next
        // compositor gives it the keyboard.
        self.selecting.clone_from(&self.selections);
        // A buffer the compositor held, which the next is not shown, is
        // released, as the one before would have released it.
        let shown = self.journal.buffers_shown();
        let mut released: Vec<u32> = self.held_buffers.difference(&shown).copied().collect();
        released.sort_unstable();
        self.held_buffers.retain(|buffer| shown.contains(buffer));
        for buffer in released {
            self.to_program.push(journal::message(buffer, 0, &[]));
        }
        let mut gestures: Vec<(u32, u16)> = self.gestures.drain().collect();
        gestures.sort_unstable();
        for (gesture, ended) in gestures {
            // Its serial and time, then that it was cancelled.
            let cancelled = journal::message(gesture, ended, &[0, 0, 1]);
            self.to_program.push(cancelled);
        }
        let made = objects
            .each_living()
            .filter(|(number, _)| *number >= SERVERS);
        self.orphans.extend(made.map(|(number, _)| number));
        self.servers = Renumbered::default();
        self.syncs.clear();
        self.offered.clear();
        for window in self.windows.values_mut() {
            window.offered.clear();
        }
        // The pointer and keyboard are held, to be kept from the program
        // if the next compositor gives them back; a field is told it left,
        // to be enabled again as it enters. With a menu ended, they are
        // told they left, as a menu ends as focus moves: a program may take
        // a menu ended alone for one still up, and the click after it for
        // the one that closes it.
        let mut focus: Vec<(u32, Focus)> = self.focus.drain().collect();
        focus.sort_by_key(|(device, _)| *device);
        self.framing.clear();
        for (device, focus) in focus {
            if focus.serial.is_some() && !menu {
                self.held.insert(device, focus);
            } else {
                self.left(device, focus);
            }
        }
        // The next compositor counts each field's commits from none.
        for commits in self.commits.values_mut() {
            commits.1 = 0;
        }
        // What the compositor said and was held back is not to be said.
        self.holding.clear();
        self.unheld.clear();
        self.relisted.clear();
        self.awaiting.clear();
        self.known.clear();
        self.renamed.clear();
        self.phase = Phase::Waiting;
        Ok(())
    }

    /// Joins the program to a new compositor: its registries are made again
    /// first.
    pub fn join(&mut self) {
        self.serials.joined();
        self.numbering = Numbering::default();
        self.private.clear();
        let registries = self.journal.registries(&mut self.numbering);
        self.private.extend(self.numbering.fillers.drain(..));
        self.to_compositor.extend(registries);
        self.sync();
        self.phase = Phase::Registering;
    }
}

/// Whether the event `opcode` of `interface` says how something is, which a
/// program told the same again need not be: a decoration's mode, the scale a
/// surface is best drawn at, how large a window may be and what it may do.
/// If so, says whether the protocol tells it only as it changes, so that
/// the same again is never needed: the scale and the transform, which the
/// new compositor tells once a surface is shown, after the relay has caught
/// up. The rest are told with a window's configure, and only those the new
/// compositor tells while the relay catches up are sifted.
fn describes(interface: &str, opcode: u16) -> Option<bool> {
    match (interface, opcode) {
        ("wp_fractional_scale_v1" | "ext_background_effect_manager_v1", 0)
        | ("wl_surface", 2 | 3) => Some(true),
        ("zxdg_toplevel_decoration_v1", 0) | ("xdg_toplevel", 2 | 3) => Some(false),
        _ => None,
    }
}

/// A registry's event that offers `global` under `name`.
fn offer(registry: u32, name: u32, global: &Global) -> Sending {
    let mut body = name.to_ne_bytes().to_vec();
    let length = u32::try_from(global.interface.len() + 1).unwrap_or(u32::MAX);
    body.extend(length.to_ne_bytes());
    body.extend(&global.interface);
    body.push(0);
    body.resize(body.len().next_multiple_of(4), 0);
    body.extend(global.version.to_ne_bytes());
    let size = u32::try_from(HEADER + body.len()).unwrap_or(u32::MAX);
    let mut bytes = registry.to_ne_bytes().to_vec();
    bytes.extend((size << 16).to_ne_bytes());
    bytes.extend(body);
    (bytes, Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::tests::{message, text, words};

    /// A program as the relay keeps it, with what it has made.
    #[derive(Default)]
    struct Kept {
        objects: Objects,
        keeping: Keeping,
    }

    impl Kept {
        /// Sends the request `bytes`; returns it as passed on, if it is.
        fn send(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
            let header = wire::header(bytes).unwrap().unwrap();
            let read = self.objects.request(bytes, header).unwrap();
            let described = &read.interface.requests[usize::from(read.opcode)];
            self.keeping
                .journal
                .request(header, bytes, &[], &read, described);
            let mut passed = bytes.to_vec();
            self.keeping
                .request(&read, &mut passed, described)
                .then_some(passed)
        }

        /// The compositor sends the event `bytes`; returns it as the program
        /// is told it, if it is.
        fn told(&mut self, bytes: &[u8]) -> Option<Vec<u8>> {
            let mut bytes = bytes.to_vec();
            let header = wire::header(&bytes).unwrap().unwrap();
            let header = self
                .keeping
                .before(&self.objects, header, &mut bytes)
                .unwrap()?;
            let read = self.objects.event(&bytes, header).unwrap();
            let body = bytes[HEADER..].to_vec();
            let kept = self.keeping.journal.event(header, &body, &read);
            (kept && self.keeping.after(&read, &bytes)).then_some(bytes)
        }

        /// A registry as 2, offered `globals` by the compositor, with what
        /// each is called and its version.
        fn registered(globals: &[(u32, &str, u32)]) -> Self {
            let mut kept = Self::default();
            kept.send(&message(1, 1, &words(&[2]))).unwrap();
            for (name, interface, version) in globals {
                kept.told(&global(*name, interface, *version)).unwrap();
            }
            kept
        }

        /// Binds the global named `name` as `number`.
        fn bind(&mut self, name: u32, interface: &str, version: u32, number: u32) -> Vec<u8> {
            let mut body = words(&[name]);
            body.extend(text(interface));
            body.extend(words(&[version, number]));
            self.send(&message(2, 0, &body)).unwrap()
        }

        /// Loses the compositor, and joins the next, which offers `globals`;
        /// answers the relay's syncs as it would.
        fn restarted(&mut self, globals: &[(u32, &str, u32)]) {
            assert_eq!(self.keeping.lost(&self.objects), Ok(()));
            self.keeping.join();
            let sync = *self.keeping.syncs.first().unwrap();
            for (name, interface, version) in globals {
                assert_eq!(self.told(&global(*name, interface, *version)), None);
            }
            self.answer(sync);
        }

        /// Reads again what was held back and is known now, as the link
        /// does; returns each as the program is told it, if it is.
        fn again(&mut self) -> Vec<Option<Vec<u8>>> {
            let mut again = std::mem::take(&mut self.keeping.again);
            let mut told = Vec::new();
            while let Some(header) = wire::header(&again).unwrap() {
                let message: Vec<u8> = again.drain(..header.size).collect();
                told.push(self.told(&message));
            }
            told
        }

        /// Answers the sync `sync`, as the compositor does: done, then its
        /// number freed.
        fn answer(&mut self, sync: u32) {
            assert_eq!(self.told(&message(sync, 0, &words(&[0]))), None);
            assert_eq!(self.told(&message(1, 1, &words(&[sync]))), None);
        }
    }

    /// A registry's event that offers a global.
    fn global(name: u32, interface: &str, version: u32) -> Vec<u8> {
        announced(2, name, interface, version)
    }

    /// The event of `registry` that offers a global.
    fn announced(registry: u32, name: u32, interface: &str, version: u32) -> Vec<u8> {
        let mut body = words(&[name]);
        body.extend(text(interface));
        body.extend(words(&[version]));
        message(registry, 0, &body)
    }

    #[test]
    fn a_global_told_to_another_registry_is_named_there_as_it_is_to_the_first() {
        let globals = [(1, "wl_compositor", 6), (2, "zwp_linux_dmabuf_v1", 5)];
        let mut kept = Kept::registered(&globals);
        // A second registry, as the library a program draws with makes.
        kept.send(&message(1, 1, &words(&[3]))).unwrap();
        for (name, interface, version) in globals {
            let told = announced(3, name, interface, version);
            assert_eq!(kept.told(&told), Some(told.clone()));
        }
        let mut bind = words(&[2]);
        bind.extend(text("zwp_linux_dmabuf_v1"));
        bind.extend(words(&[5, 4]));
        kept.send(&message(3, 0, &bind)).unwrap();

        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert!(kept.keeping.live(), "kept, bound as it was");
    }

    #[test]
    fn a_global_under_a_name_the_compositor_gave_another_is_new_to_the_program() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6)]);
        kept.send(&message(1, 1, &words(&[3]))).unwrap();
        kept.told(&announced(3, 1, "wl_compositor", 6)).unwrap();
        kept.restarted(&[(7, "wl_compositor", 6)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        // The program's 1 is the compositor's 7, so its 1 is another.
        for registry in [2, 3] {
            let told = kept.told(&announced(registry, 1, "wl_seat", 7));
            assert_eq!(told, Some(announced(registry, 2, "wl_seat", 7)));
        }
        // Its 7 taken away, and given to a screen.
        for registry in [2, 3] {
            kept.told(&message(registry, 1, &words(&[7]))).unwrap();
        }
        let told = kept.told(&announced(2, 7, "wl_output", 4));
        assert_eq!(told, Some(announced(2, 7, "wl_output", 4)));
        let removed = kept.told(&message(2, 1, &words(&[7])));
        assert_eq!(removed, Some(message(2, 1, &words(&[7]))));
    }

    #[test]
    fn a_name_the_program_had_for_a_global_taken_away_is_the_compositors_for_the_next() {
        let mut kept = Kept::registered(&[(1, "wl_output", 4)]);
        kept.restarted(&[(7, "wl_output", 4)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        let removed = kept.told(&message(2, 1, &words(&[7])));
        assert_eq!(removed, Some(message(2, 1, &words(&[1]))));
        // The compositor's 1 is new, and the program's 1 is no longer its 7.
        let told = kept.told(&announced(2, 1, "wl_seat", 7));
        assert_eq!(told, Some(announced(2, 1, "wl_seat", 7)));
        let bound = kept.bind(1, "wl_seat", 7, 3);
        assert_eq!(wire::word(&bound, HEADER), Some(1));
    }

    #[test]
    fn a_number_freed_before_the_compositor_went_and_made_again_is_freed_once() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6)]);
        kept.bind(1, "wl_compositor", 6, 3);
        // A surface, given a region as where it takes input, and the region
        // destroyed: its number is the program's again only once nothing
        // kept names it.
        kept.send(&message(3, 0, &words(&[4]))).unwrap();
        kept.send(&message(3, 1, &words(&[5]))).unwrap();
        kept.send(&message(4, 5, &words(&[5]))).unwrap();
        kept.send(&message(5, 0, &[])).unwrap();
        let freed = message(DISPLAY, DELETE_ID, &words(&[5]));
        assert_eq!(kept.told(&freed), None, "withheld");

        kept.restarted(&[(1, "wl_compositor", 6)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert!(kept.keeping.live());
        // The region, made again and destroyed again by the relay, is no
        // longer named once the surface is gone; the next compositor frees
        // its number.
        kept.send(&message(4, 0, &[])).unwrap();
        let told = kept.keeping.journal.freed().len() + usize::from(kept.told(&freed).is_some());
        assert_eq!(told, 1, "the program told its number is free once");
    }

    #[test]
    fn a_menus_surface_shown_again_with_nothing_is_kept_while_its_frame_is_awaited() {
        let globals = [
            (1, "wl_compositor", 6),
            (2, "xdg_wm_base", 6),
            (3, "wp_single_pixel_buffer_manager_v1", 1),
        ];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        // A window, on 6, and a menu on it, on 7, showing a buffer twice
        // and waiting for its next frame.
        for surface in [6, 7] {
            kept.send(&message(3, 0, &words(&[surface]))).unwrap();
        }
        kept.send(&message(4, 2, &words(&[8, 6]))).unwrap();
        kept.send(&message(8, 1, &words(&[9]))).unwrap();
        kept.send(&message(4, 1, &words(&[10]))).unwrap();
        kept.send(&message(4, 2, &words(&[11, 7]))).unwrap();
        kept.send(&message(11, 2, &words(&[12, 8, 10]))).unwrap();
        kept.send(&message(5, 1, &words(&[13, 0, 0, 0, 0])))
            .unwrap();
        for _ in 0..2 {
            kept.send(&message(7, 1, &words(&[13, 0, 0]))).unwrap();
            kept.send(&message(7, 6, &[])).unwrap();
        }
        kept.send(&message(7, 3, &words(&[14]))).unwrap();

        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        // Told the menu is done, the program destroys it, and its surface.
        for menu in [12, 11, 7] {
            kept.send(&message(menu, 0, &[]));
        }
        let freed = message(DISPLAY, DELETE_ID, &words(&[7]));
        assert_eq!(kept.told(&freed), None, "withheld, as the frame names it");
    }

    /// A program holding the selection: a data device, 7, on the seat, 5,
    /// and a source, 8, offering text, set as the selection with serial 9;
    /// and as the primary selection, the same of 9 and 10.
    fn selecting() -> (Kept, [(u32, &'static str, u32); 3]) {
        let globals = [
            (1, "wl_data_device_manager", 3),
            (2, "wl_seat", 7),
            (3, "zwp_primary_selection_device_manager_v1", 1),
        ];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        kept.send(&message(3, 1, &words(&[6, 4]))).unwrap();
        kept.send(&message(3, 0, &words(&[7]))).unwrap();
        kept.send(&message(7, 0, &text("text/plain"))).unwrap();
        kept.send(&message(6, 1, &words(&[7, 9]))).unwrap();
        kept.send(&message(5, 1, &words(&[8, 4]))).unwrap();
        kept.send(&message(5, 0, &words(&[9]))).unwrap();
        kept.send(&message(9, 0, &text("text/plain"))).unwrap();
        kept.send(&message(8, 0, &words(&[9, 9]))).unwrap();
        (kept, globals)
    }

    #[test]
    fn what_a_program_selected_is_selected_again_as_it_has_the_keyboard_and_nothing_else_is() {
        let (mut kept, globals) = selecting();
        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        kept.keeping.to_compositor.clear();
        // As the program has the keyboard, the next compositor says there is
        // no selection: kept from the program, which still holds one, and its
        // own set again.
        assert_eq!(kept.told(&message(6, 5, &words(&[0]))), None);
        assert_eq!(kept.told(&message(8, 1, &words(&[0]))), None);
        let latest = kept.keeping.serials.latest();
        assert_eq!(
            bytes(&kept.keeping.to_compositor),
            [
                message(6, 1, &words(&[7, latest])),
                message(8, 0, &words(&[9, latest])),
            ]
        );
        kept.keeping.to_compositor.clear();
        // Once only.
        assert!(kept.told(&message(6, 5, &words(&[0]))).is_some());
        assert!(kept.keeping.to_compositor.is_empty());
    }

    #[test]
    fn what_a_program_selected_gives_way_to_what_was_selected_meanwhile() {
        let (mut kept, globals) = selecting();
        kept.restarted(&globals);
        // Something else copied meanwhile, under the next compositor.
        kept.told(&message(6, 0, &words(&[SERVERS]))).unwrap();
        let told = kept.told(&message(6, 5, &words(&[SERVERS])));
        assert_eq!(told, Some(message(6, 5, &words(&[SERVERS]))));
        kept.told(&message(8, 0, &words(&[SERVERS + 1]))).unwrap();
        let told = kept.told(&message(8, 1, &words(&[SERVERS + 1])));
        assert_eq!(told, Some(message(8, 1, &words(&[SERVERS + 1]))));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(7, 2, &[]), message(9, 1, &[])],
            "each source told it is the selection no longer"
        );
    }

    #[test]
    fn what_was_selected_no_longer_or_set_again_is_not_selected_again() {
        let (mut kept, globals) = selecting();
        // The clipboard taken by another; what was selected let go of, and
        // the primary selection given up.
        kept.told(&message(7, 2, &[])).unwrap();
        kept.send(&message(9, 1, &[])).unwrap();
        kept.send(&message(8, 0, &words(&[0, 9]))).unwrap();
        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        kept.keeping.to_compositor.clear();
        assert!(kept.told(&message(6, 5, &words(&[0]))).is_some());
        assert!(kept.told(&message(8, 1, &words(&[0]))).is_some());
        assert!(kept.keeping.to_compositor.is_empty());
    }

    #[test]
    fn a_global_taken_away_is_told_to_each_registry_by_the_programs_name_for_it() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6)]);
        kept.send(&message(1, 1, &words(&[3]))).unwrap();
        kept.told(&announced(3, 1, "wl_compositor", 6)).unwrap();
        // The next compositor calls it 7; the program, 1.
        kept.restarted(&[(7, "wl_compositor", 6)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        for registry in [2, 3] {
            let removed = kept.told(&message(registry, 1, &words(&[7])));
            assert_eq!(removed, Some(message(registry, 1, &words(&[1]))));
        }
    }

    #[test]
    fn a_global_offered_live_under_a_name_the_program_knows_is_named_anew() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6)]);
        kept.restarted(&[(7, "wl_compositor", 6)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert!(kept.keeping.live());
        // The new compositor names another global as the program names its
        // compositor.
        let told = kept.told(&global(1, "wl_shm", 1)).unwrap();
        assert_eq!(
            wire::word(&told, HEADER),
            Some(2),
            "named past the names known"
        );
        let bound = kept.bind(2, "wl_shm", 1, 3);
        assert_eq!(
            wire::word(&bound, HEADER),
            Some(1),
            "bound by the compositor's name"
        );
        let compositor = kept.bind(1, "wl_compositor", 6, 4);
        assert_eq!(wire::word(&compositor, HEADER), Some(7));
        // Taken away, it is taken away by the program's name.
        let gone = kept.told(&message(2, 1, &words(&[1]))).unwrap();
        assert_eq!(wire::word(&gone, HEADER), Some(2));
    }

    #[test]
    fn a_global_offered_under_the_highest_name_there_is_is_named_by_one_free() {
        let mut kept = Kept::registered(&[(u32::MAX, "wl_compositor", 6)]);
        kept.restarted(&[(7, "wl_compositor", 6)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        // With no name past the one the program knows, the lowest it does
        // not.
        let told = kept.told(&global(u32::MAX, "wl_shm", 1)).unwrap();
        assert_eq!(wire::word(&told, HEADER), Some(1));
        let bound = kept.bind(1, "wl_shm", 1, 3);
        assert_eq!(wire::word(&bound, HEADER), Some(u32::MAX));
    }

    #[test]
    fn a_global_taken_away_while_the_relay_catches_up_is_kept_from_the_program() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6)]);
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        kept.keeping.join();
        assert_eq!(kept.told(&message(2, 1, &words(&[1]))), None);
    }

    #[test]
    fn what_has_left_the_program_is_not_said_to_leave_again() {
        let mut kept = Kept::registered(&[(1, "wl_seat", 7), (2, "wl_compositor", 6)]);
        kept.bind(1, "wl_seat", 7, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        kept.send(&message(3, 0, &words(&[6]))).unwrap();
        kept.send(&message(3, 1, &words(&[7]))).unwrap();
        // The pointer and keyboard enter the surface, and leave it.
        kept.told(&message(6, 0, &words(&[10, 5, 0, 0]))).unwrap();
        kept.told(&message(7, 1, &words(&[11, 5, 0]))).unwrap();
        kept.told(&message(6, 1, &words(&[12, 5]))).unwrap();
        kept.told(&message(7, 2, &words(&[13, 5]))).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        assert_eq!(kept.keeping.to_program.len(), 0, "nothing had them");
    }

    #[test]
    fn a_layer_surface_is_told_its_size_again_only_where_it_changed() {
        let globals = [(1, "zwlr_layer_shell_v1", 4), (2, "wl_compositor", 6)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "zwlr_layer_shell_v1", 4, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        let mut made = words(&[6, 5, 0, 2]);
        made.extend(text("panel"));
        kept.send(&message(3, 0, &made)).unwrap();
        kept.told(&message(6, 0, &words(&[1, 640, 30]))).unwrap();
        assert!(kept.send(&message(6, 6, &words(&[1]))).is_some(), "acked");

        kept.restarted(&globals);
        let same = message(6, 0, &words(&[3, 640, 30]));
        assert_eq!(kept.told(&same), None, "the same, so not told");
        let acked = |kept: &Kept| {
            kept.keeping.to_compositor.iter().any(|(bytes, _)| {
                let header = wire::header(bytes).unwrap().unwrap();
                (header.object, header.opcode) == (6, 6)
            })
        };
        assert!(acked(&kept), "acked by the relay");

        kept.restarted(&globals);
        let told = kept.told(&message(6, 0, &words(&[4, 800, 30])));
        let told = told.expect("another, so told");
        assert_eq!(wire::word(&told, HEADER + 4), Some(800));
        assert!(acked(&kept), "and acked by the relay");
        let serial = wire::word(&told, HEADER).unwrap();
        assert_eq!(
            kept.send(&message(6, 6, &words(&[serial]))),
            None,
            "the program's ack kept from the compositor"
        );
    }

    #[test]
    fn a_window_configured_as_the_program_last_acked_it_is_not_told_again() {
        let mut kept = Kept::registered(&[(1, "xdg_wm_base", 6), (2, "wl_compositor", 6)]);
        kept.bind(1, "xdg_wm_base", 6, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        kept.send(&message(3, 2, &words(&[6, 5]))).unwrap();
        kept.send(&message(6, 1, &words(&[7]))).unwrap();
        let configure = message(7, 0, &words(&[200, 100, 0]));
        kept.told(&configure).unwrap();
        kept.told(&message(6, 0, &words(&[1]))).unwrap();
        assert!(kept.send(&message(6, 4, &words(&[1]))).is_some(), "acked");
        assert!(
            kept.send(&message(6, 4, &words(&[9]))).is_none(),
            "never sent"
        );

        kept.restarted(&[(1, "xdg_wm_base", 6), (2, "wl_compositor", 6)]);
        assert_eq!(kept.told(&configure), None, "held");
        assert_eq!(
            kept.told(&message(6, 0, &words(&[3]))),
            None,
            "the same, so not told"
        );
        let acked = kept.keeping.to_compositor.iter().any(|(bytes, _)| {
            wire::header(bytes).unwrap().unwrap()
                == Header {
                    object: 6,
                    opcode: 4,
                    size: 12,
                }
        });
        assert!(acked, "acked by the relay");
    }

    #[test]
    fn what_the_compositor_makes_under_a_number_the_program_holds_is_numbered_anew() {
        let mut kept = Kept::registered(&[(1, "wl_data_device_manager", 3), (2, "wl_seat", 7)]);
        kept.bind(1, "wl_data_device_manager", 3, 3);
        kept.bind(2, "wl_seat", 7, 4);
        kept.send(&message(3, 1, &words(&[5, 4]))).unwrap();
        // An offer, which the program holds as the compositor goes.
        kept.told(&message(5, 0, &words(&[SERVERS]))).unwrap();
        kept.restarted(&[(1, "wl_data_device_manager", 3), (2, "wl_seat", 7)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);

        // The next compositor's first offer takes the same number.
        let told = kept.told(&message(5, 0, &words(&[SERVERS]))).unwrap();
        assert_eq!(wire::word(&told, HEADER), Some(SERVERS + 1));
        // Destroyed, it is destroyed under its own.
        let destroy = kept.send(&message(SERVERS + 1, 2, &[])).unwrap();
        assert_eq!(wire::word(&destroy, 0), Some(SERVERS));
        // The old one goes nowhere.
        assert_eq!(kept.send(&message(SERVERS, 2, &[])), None);
    }

    #[test]
    fn the_relays_syncs_that_fill_a_gap_are_kept_from_the_program() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6)]);
        kept.bind(1, "wl_compositor", 6, 3);
        kept.send(&message(3, 1, &words(&[4]))).unwrap();
        kept.send(&message(3, 1, &words(&[5]))).unwrap();
        kept.send(&message(4, 0, &[])).unwrap();
        kept.told(&message(1, 1, &words(&[4]))).unwrap();
        kept.restarted(&[(1, "wl_compositor", 6)]);
        // 4 was filled, before 5 was made again.
        assert!(kept.keeping.private.contains(&4));
        kept.answer(4);
        assert!(!kept.keeping.private.contains(&4));
    }

    /// A program's objects and its keeping, sent `requests` and told
    /// `events` in turn, as `(true, bytes)` for an event.
    fn run(kept: &mut (Objects, Keeping), steps: &[(bool, Vec<u8>)]) -> Vec<Option<Vec<u8>>> {
        let (objects, keeping) = kept;
        steps
            .iter()
            .map(|(event, bytes)| {
                let mut bytes = bytes.clone();
                let header = wire::header(&bytes).unwrap().unwrap();
                if *event {
                    let header = keeping.before(objects, header, &mut bytes).unwrap()?;
                    let read = objects.event(&bytes, header).unwrap();
                    keeping.after(&read, &bytes).then_some(bytes)
                } else {
                    let read = objects.request(&bytes, header).unwrap();
                    let described = &read.interface.requests[usize::from(read.opcode)];
                    keeping
                        .journal
                        .request(header, &bytes, &[], &read, described);
                    keeping
                        .request(&read, &mut bytes, described)
                        .then_some(bytes)
                }
            })
            .collect()
    }

    /// Binds `interface` as `number`, by the name `name`, on the registry 2.
    fn bound(name: u32, interface: &str, version: u32, number: u32) -> (bool, Vec<u8>) {
        let mut body = words(&[name]);
        body.extend(text(interface));
        body.extend(words(&[version, number]));
        (false, message(2, 0, &body))
    }

    /// A registry `registry`'s event that offers a global.
    fn offered(registry: u32, name: u32, interface: &str) -> (bool, Vec<u8>) {
        let mut body = words(&[name]);
        body.extend(text(interface));
        body.extend(words(&[1]));
        (true, message(registry, 0, &body))
    }

    #[test]
    fn an_object_the_compositor_made_is_named_in_a_request_as_it_names_it_or_not_at_all() {
        let mut kept = (Objects::default(), Keeping::default());
        let list = "ext_foreign_toplevel_list_v1";
        let sources = "ext_foreign_toplevel_image_capture_source_manager_v1";
        let passed = run(
            &mut kept,
            &[
                (false, message(1, 1, &words(&[2]))),
                bound(1, list, 1, 3),
                bound(2, sources, 1, 4),
                // A window, as the list says it, and a source made of it.
                (true, message(3, 0, &words(&[SERVERS]))),
                (false, message(4, 0, &words(&[5, SERVERS]))),
                // Destroyed, as a source cannot yet be made again.
                (false, message(5, 0, &[])),
            ],
        );
        assert_eq!(passed[4], Some(message(4, 0, &words(&[5, SERVERS]))));

        let (objects, keeping) = &mut kept;
        assert_eq!(keeping.lost(objects), Ok(()));
        keeping.join();
        let after = run(&mut kept, &[(false, message(4, 0, &words(&[6, SERVERS])))]);
        assert_eq!(
            after,
            [None],
            "a window the compositor before made goes nowhere"
        );
    }

    #[test]
    fn a_global_offered_on_each_registry_while_the_relay_catches_up_is_taken_in_once() {
        let mut kept = (Objects::default(), Keeping::default());
        run(
            &mut kept,
            &[
                (false, message(1, 1, &words(&[2]))),
                (false, message(1, 1, &words(&[3]))),
                offered(2, 1, "wl_shm"),
                offered(3, 1, "wl_shm"),
            ],
        );
        let (objects, keeping) = &mut kept;
        assert_eq!(keeping.lost(objects), Ok(()));
        keeping.join();
        let told = run(
            &mut kept,
            &[offered(2, 1, "wl_shm"), offered(3, 1, "wl_shm")],
        );
        assert_eq!(told, [None, None]);
        assert_eq!(kept.1.offered.len(), 1);
    }

    #[test]
    fn what_the_compositor_says_to_and_of_what_it_made_is_said_as_the_program_numbers_it() {
        let mut kept = Kept::registered(&[(1, "wl_data_device_manager", 3), (2, "wl_seat", 7)]);
        kept.bind(1, "wl_data_device_manager", 3, 3);
        kept.bind(2, "wl_seat", 7, 4);
        kept.send(&message(3, 1, &words(&[5, 4]))).unwrap();
        kept.told(&message(5, 0, &words(&[SERVERS]))).unwrap();
        kept.restarted(&[(1, "wl_data_device_manager", 3), (2, "wl_seat", 7)]);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        // A new offer, under the number the program still holds, what it
        // offers, and it as the selection.
        kept.told(&message(5, 0, &words(&[SERVERS]))).unwrap();
        let offers = kept
            .told(&message(SERVERS, 0, &text("text/plain")))
            .unwrap();
        assert_eq!(wire::word(&offers, 0), Some(SERVERS + 1));
        let selection = kept.told(&message(5, 5, &words(&[SERVERS]))).unwrap();
        assert_eq!(wire::word(&selection, HEADER), Some(SERVERS + 1));
    }

    #[test]
    fn what_the_next_compositor_first_says_of_a_window_is_answered_by_the_relay_alone() {
        let mut kept = Kept::registered(&[(1, "xdg_wm_base", 6), (2, "wl_compositor", 6)]);
        kept.bind(1, "xdg_wm_base", 6, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        kept.send(&message(3, 2, &words(&[6, 5]))).unwrap();
        kept.send(&message(6, 1, &words(&[7]))).unwrap();
        kept.told(&message(7, 0, &words(&[200, 100, 0]))).unwrap();
        kept.told(&message(6, 0, &words(&[1]))).unwrap();
        kept.send(&message(6, 4, &words(&[1]))).unwrap();

        // Of a window not yet shown again, with no size and no focus: the
        // program, which would take it for focus gone, is not told.
        kept.restarted(&[(1, "xdg_wm_base", 6), (2, "wl_compositor", 6)]);
        assert_eq!(kept.told(&message(7, 0, &words(&[0, 0, 0]))), None);
        assert_eq!(kept.told(&message(6, 0, &words(&[3]))), None);
        let acked = message(6, 4, &words(&[3]));
        assert!(
            bytes(&kept.keeping.to_compositor).contains(&acked),
            "as its own"
        );
        assert!(kept.keeping.to_program.is_empty());

        // Shown, what the compositor says of it is told, past the serial the
        // program acked, and its ack taken back.
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        let shown = message(7, 0, &words(&[300, 200, 0]));
        assert_eq!(kept.told(&shown), Some(shown));
        let told = kept.told(&message(6, 0, &words(&[4])));
        assert_eq!(told, Some(message(6, 0, &words(&[6]))));
        let ack = kept.send(&message(6, 4, &words(&[6])));
        assert_eq!(ack, Some(message(6, 4, &words(&[4]))));
    }

    #[test]
    fn globals_new_to_the_program_are_offered_and_those_gone_taken_away() {
        let mut kept = Kept::registered(&[(1, "wl_compositor", 6), (2, "wl_subcompositor", 1)]);
        kept.bind(1, "wl_compositor", 6, 3);
        // The next has no subcompositor, which the program never bound, and
        // offers the shm under the name the program knew it by: taken away
        // first, the name is free for the shm.
        kept.restarted(&[(1, "wl_compositor", 6), (2, "wl_shm", 1)]);
        let told: Vec<Vec<u8>> = kept
            .keeping
            .to_program
            .drain(..)
            .map(|(bytes, _)| bytes)
            .collect();
        assert_eq!(
            told[0],
            message(2, 1, &words(&[2])),
            "the subcompositor gone"
        );
        assert_eq!(told[1], global(2, "wl_shm", 1), "the shm offered");
        assert_eq!(
            kept.bind(2, "wl_shm", 1, 4)[HEADER..HEADER + 4],
            2_u32.to_ne_bytes()
        );
    }

    /// A program holding a pointer, 5, and a keyboard, 6, on the seat, with
    /// both in its surface 7: the pointer entered at (1, 2) and moved to
    /// (3, 4), with serial 10, the keyboard with 11.
    fn focused() -> (Kept, [(u32, &'static str, u32); 2]) {
        let globals = [(1, "wl_compositor", 6), (2, "wl_seat", 7)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "wl_compositor", 6, 3);
        kept.bind(2, "wl_seat", 7, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        kept.send(&message(4, 1, &words(&[6]))).unwrap();
        kept.send(&message(3, 0, &words(&[7]))).unwrap();
        kept.told(&message(5, 0, &words(&[10, 7, 1, 2]))).unwrap();
        kept.told(&message(5, 5, &[])).unwrap();
        kept.told(&message(5, 2, &words(&[0, 3, 4]))).unwrap();
        kept.told(&message(6, 1, &words(&[11, 7, 0]))).unwrap();
        (kept, globals)
    }

    #[test]
    fn the_pointer_and_keyboard_back_where_they_were_are_not_told_they_left() {
        let (mut kept, globals) = focused();
        kept.restarted(&globals);
        assert!(
            kept.keeping.to_program.is_empty(),
            "nothing told as it goes"
        );
        let entered = message(5, 0, &words(&[3, 7, 3, 4]));
        assert_eq!(kept.told(&entered), None);
        assert_eq!(kept.told(&message(5, 5, &[])), None, "nor the frame of it");
        assert_eq!(kept.told(&message(6, 1, &words(&[4, 7, 0]))), None);
        // The cursor, set under the enter the program knows, is set under
        // the next compositor's.
        let cursor = kept.send(&message(5, 0, &words(&[10, 0, 0, 0])));
        assert_eq!(cursor, Some(message(5, 0, &words(&[3, 0, 0, 0]))));
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert!(kept.keeping.live());
        assert!(kept.keeping.to_program.is_empty(), "nothing left");
    }

    #[test]
    fn a_key_held_as_the_compositor_goes_is_let_go_of_and_the_keyboard_kept() {
        let (mut kept, globals) = focused();
        // Return pressed, as what starts the compositor anew is typed, and
        // another pressed and let go of.
        let key = |serial, key, state| message(6, 3, &words(&[serial, 100, key, state]));
        // Shift held and let go of, which says nothing of keys.
        let modifiers = |serial, held| message(6, 4, &words(&[serial, held, 0, 0, 0]));
        let typed = [
            key(12, 28, 1),
            modifiers(13, 1),
            key(14, 30, 1),
            key(15, 30, 0),
            modifiers(16, 0),
        ];
        for told in typed {
            assert!(kept.told(&told).is_some());
        }
        kept.restarted(&globals);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(6, 3, &words(&[16, 0, 28, 0]))],
            "let go of, under the last serial it was told, with no time"
        );
        kept.keeping.to_program.clear();
        let entered = message(6, 1, &words(&[4, 7, 0]));
        assert_eq!(kept.told(&entered), None, "the keyboard back where it was");
    }

    #[test]
    fn the_keys_a_keyboard_enters_holding_are_held_until_it_leaves_or_is_released() {
        let (mut kept, globals) = focused();
        // Back in, holding Return and Shift.
        kept.told(&message(6, 2, &words(&[12, 7]))).unwrap();
        kept.told(&message(6, 1, &words(&[13, 7, 8, 28, 42])))
            .unwrap();
        kept.restarted(&globals);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [
                message(6, 3, &words(&[13, 0, 28, 0])),
                message(6, 3, &words(&[13, 0, 42, 0]))
            ]
        );

        // Holding a key as it leaves: nothing is held once it has.
        let (mut kept, globals) = focused();
        kept.told(&message(6, 3, &words(&[12, 100, 28, 1])))
            .unwrap();
        kept.told(&message(6, 2, &words(&[13, 7]))).unwrap();
        kept.restarted(&globals);
        assert!(kept.keeping.to_program.is_empty(), "nothing to let go of");

        // Released by the program while a key is held: it has no keys.
        let (mut kept, globals) = focused();
        kept.told(&message(6, 3, &words(&[12, 100, 28, 1])))
            .unwrap();
        kept.send(&message(6, 0, &[])).unwrap();
        kept.restarted(&globals);
        assert!(
            kept.keeping.to_program.is_empty(),
            "nothing told a keyboard released"
        );
    }

    #[test]
    fn the_pointer_back_elsewhere_and_a_keyboard_not_back_are_told_they_left() {
        let (mut kept, globals) = focused();
        kept.restarted(&globals);
        // Back over the surface, at another place on it: the program is told
        // it left, under the last serial it was told, and then entered.
        let entered = kept.told(&message(5, 0, &words(&[3, 7, 0, 0])));
        assert_eq!(entered, Some(message(5, 0, &words(&[15, 7, 0, 0]))));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(5, 1, &words(&[15, 7])), message(5, 5, &[])]
        );
        kept.keeping.to_program.clear();
        assert!(kept.told(&message(5, 5, &[])).is_some(), "its own frame");
        // The keyboard is not back by the time the relay has caught up, nor
        // a turn of the compositor's after, while the program waits to be
        // read, as the relay's syncs take numbers it may take next.
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        for _ in 0..SETTLING {
            assert!(kept.keeping.to_program.is_empty(), "not yet");
            assert!(!kept.keeping.live(), "not read meanwhile");
            let sync = *kept.keeping.syncs.first().unwrap();
            kept.answer(sync);
        }
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(6, 2, &words(&[15, 7]))]
        );
        assert!(kept.keeping.live());
    }

    #[test]
    fn a_buffer_the_compositor_held_that_is_not_shown_again_is_released_as_it_goes() {
        let globals = [
            (1, "wl_compositor", 6),
            (2, "wp_single_pixel_buffer_manager_v1", 1),
        ];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "wl_compositor", 6, 3);
        kept.bind(2, "wp_single_pixel_buffer_manager_v1", 1, 4);
        kept.send(&message(3, 0, &words(&[5]))).unwrap();
        // Shown one after another: the first released, the second still held
        // as the third is shown, and the fourth attached but not committed.
        for buffer in [6, 7, 8, 9] {
            kept.send(&message(4, 1, &words(&[buffer, 0, 0, 0, 0])))
                .unwrap();
        }
        for buffer in [6, 7, 8] {
            kept.send(&message(5, 1, &words(&[buffer, 0, 0]))).unwrap();
            kept.send(&message(5, 6, &[])).unwrap();
        }
        kept.send(&message(5, 1, &words(&[9, 0, 0]))).unwrap();
        kept.told(&message(6, 0, &[])).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(7, 0, &[])],
            "the one held, which the next compositor will not have"
        );
    }

    #[test]
    fn a_program_whose_menu_ends_with_the_compositor_is_told_its_focus_left_too() {
        let globals = [
            (1, "wl_compositor", 6),
            (2, "wl_seat", 7),
            (3, "xdg_wm_base", 6),
        ];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        // A window on 6, with the pointer and keyboard, and a menu on it.
        kept.send(&message(3, 0, &words(&[6]))).unwrap();
        kept.send(&message(3, 0, &words(&[7]))).unwrap();
        kept.send(&message(4, 0, &words(&[8]))).unwrap();
        kept.send(&message(4, 1, &words(&[9]))).unwrap();
        kept.send(&message(5, 2, &words(&[10, 6]))).unwrap();
        kept.send(&message(10, 1, &words(&[11]))).unwrap();
        kept.send(&message(5, 1, &words(&[12]))).unwrap();
        kept.send(&message(5, 2, &words(&[13, 7]))).unwrap();
        kept.send(&message(13, 2, &words(&[14, 10, 12]))).unwrap();
        kept.told(&message(8, 0, &words(&[20, 6, 0, 0]))).unwrap();
        kept.told(&message(9, 1, &words(&[21, 6, 0]))).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        // As a menu ends as focus moves: a program may take a menu ended
        // alone for one still up.
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [
                message(14, 1, &[]),
                message(8, 1, &words(&[21, 6])),
                message(8, 5, &[]),
                message(9, 2, &words(&[21, 6])),
            ]
        );
    }

    #[test]
    fn a_surface_destroyed_with_the_pointer_and_keyboard_on_it_is_not_said_to_be_left() {
        let (mut kept, globals) = focused();
        kept.send(&message(7, 0, &[])).unwrap();
        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert!(kept.keeping.live(), "nothing held");
        assert!(kept.keeping.to_program.is_empty(), "nor told");
    }

    #[test]
    fn a_pointer_and_keyboard_released_with_a_surface_entered_are_not_said_to_leave_it() {
        let (mut kept, globals) = focused();
        kept.send(&message(5, 1, &[])).unwrap();
        kept.send(&message(6, 0, &[])).unwrap();
        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert!(kept.keeping.live(), "nothing held");
        assert!(kept.keeping.to_program.is_empty(), "nor told");
    }

    #[test]
    fn a_serial_after_a_restart_is_told_past_those_before_and_asked_as_the_compositors() {
        let globals = [(1, "xdg_wm_base", 6)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "xdg_wm_base", 6, 3);
        let pinged = |serial| message(3, 0, &words(&[serial]));
        let answered = |serial| message(3, 3, &words(&[serial]));
        assert_eq!(
            kept.told(&pinged(500)),
            Some(pinged(500)),
            "as it is, before"
        );
        assert_eq!(kept.send(&answered(500)), Some(answered(500)));
        kept.restarted(&globals);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert_eq!(kept.told(&pinged(3)), Some(pinged(504)));
        assert_eq!(kept.send(&answered(504)), Some(answered(3)));
        assert_eq!(
            kept.send(&answered(500)),
            Some(answered(3)),
            "the one before's"
        );
    }

    #[test]
    fn what_is_asked_of_an_object_of_the_compositors_is_asked_under_its_number_or_not_at_all() {
        let mut kept = Kept::registered(&[(1, "wl_data_device_manager", 3), (2, "wl_seat", 7)]);
        kept.bind(1, "wl_data_device_manager", 3, 3);
        kept.bind(2, "wl_seat", 7, 4);
        kept.send(&message(3, 1, &words(&[5, 4]))).unwrap();
        kept.told(&message(5, 0, &words(&[SERVERS]))).unwrap();
        // wl_data_offer.accept, live: passed on as it is.
        let mut accept = words(&[7]);
        accept.extend(text("text/plain"));
        let accepted = message(SERVERS, 0, &accept);
        assert_eq!(kept.send(&accepted), Some(accepted.clone()));
        kept.restarted(&[(1, "wl_data_device_manager", 3), (2, "wl_seat", 7)]);
        // The compositor before's, asked of, goes nowhere.
        assert_eq!(kept.send(&accepted), None);
    }

    #[test]
    fn what_describes_a_global_again_as_the_relay_catches_up_is_kept_from_the_program() {
        let mut kept = Kept::registered(&[(1, "wl_shm", 1)]);
        kept.bind(1, "wl_shm", 1, 3);
        kept.restarted(&[(1, "wl_shm", 1)]);
        assert_eq!(
            kept.told(&message(3, 0, &words(&[0]))),
            None,
            "a format it knows"
        );
    }

    #[test]
    fn a_scale_is_told_only_as_it_changes_and_anew_to_what_is_made_anew() {
        let mut kept = Kept::registered(&[
            (1, "wl_compositor", 6),
            (2, "wp_fractional_scale_manager_v1", 1),
        ]);
        kept.bind(1, "wl_compositor", 6, 3);
        kept.bind(2, "wp_fractional_scale_manager_v1", 1, 4);
        kept.send(&message(3, 0, &words(&[5]))).unwrap();
        kept.send(&message(4, 1, &words(&[6, 5]))).unwrap();
        let scale = |scale| message(6, 0, &words(&[scale]));
        assert!(kept.told(&scale(120)).is_some());
        assert_eq!(kept.told(&scale(120)), None, "the same again");
        assert!(kept.told(&scale(180)).is_some(), "changed");
        // Destroyed, and another made under its number, which is told.
        kept.send(&message(6, 0, &[])).unwrap();
        kept.told(&message(1, 1, &words(&[6]))).unwrap();
        kept.send(&message(4, 1, &words(&[6, 5]))).unwrap();
        assert!(kept.told(&scale(180)).is_some(), "told the new one");
    }

    #[test]
    fn what_a_program_may_have_behind_it_is_told_only_as_it_changes() {
        let globals = [(1, "ext_background_effect_manager_v1", 1)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "ext_background_effect_manager_v1", 1, 3);
        let capabilities = |flags| message(3, 0, &words(&[flags]));
        assert!(kept.told(&capabilities(1)).is_some());
        kept.restarted(&globals);
        assert_eq!(kept.told(&capabilities(1)), None, "as it was");
        assert!(kept.told(&capabilities(0)).is_some(), "changed");
    }

    #[test]
    fn a_decorations_mode_told_again_is_kept_from_the_program_only_as_the_relay_catches_up() {
        let mut kept = Kept::registered(&[
            (1, "xdg_wm_base", 6),
            (2, "wl_compositor", 6),
            (3, "zxdg_decoration_manager_v1", 1),
        ]);
        kept.bind(1, "xdg_wm_base", 6, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.bind(3, "zxdg_decoration_manager_v1", 1, 5);
        kept.send(&message(4, 0, &words(&[6]))).unwrap();
        kept.send(&message(3, 2, &words(&[7, 6]))).unwrap();
        kept.send(&message(7, 1, &words(&[8]))).unwrap();
        kept.send(&message(5, 1, &words(&[9, 8]))).unwrap();
        let server_side = message(9, 0, &words(&[2]));
        assert!(kept.told(&server_side).is_some());
        assert!(kept.told(&server_side).is_some(), "live, with a configure");
        kept.restarted(&[
            (1, "xdg_wm_base", 6),
            (2, "wl_compositor", 6),
            (3, "zxdg_decoration_manager_v1", 1),
        ]);
        assert_eq!(kept.told(&server_side), None, "as it was");
        let client_side = message(9, 0, &words(&[1]));
        assert!(kept.told(&client_side).is_some(), "as it was not");
    }

    #[test]
    fn a_field_is_told_it_was_left_and_what_is_typed_by_its_own_count_of_commits() {
        let mut kept = Kept::registered(&[
            (1, "wl_seat", 7),
            (2, "wl_compositor", 6),
            (3, "zwp_text_input_manager_v3", 1),
        ]);
        kept.bind(1, "wl_seat", 7, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.bind(3, "zwp_text_input_manager_v3", 1, 5);
        kept.send(&message(4, 0, &words(&[6]))).unwrap();
        kept.send(&message(5, 1, &words(&[7, 3]))).unwrap();
        kept.told(&message(7, 0, &words(&[6]))).unwrap();
        for _ in 0..2 {
            kept.send(&message(7, 7, &[])).unwrap();
        }
        let done = |serial| message(7, 5, &words(&[serial]));
        assert_eq!(kept.told(&done(2)), Some(done(2)), "counted alike");
        kept.restarted(&[
            (1, "wl_seat", 7),
            (2, "wl_compositor", 6),
            (3, "zwp_text_input_manager_v3", 1),
        ]);
        assert_eq!(
            kept.keeping.to_program.first().map(|(bytes, _)| bytes),
            Some(&message(7, 1, &words(&[6]))),
            "left, with no serial"
        );
        kept.send(&message(7, 7, &[])).unwrap();
        assert_eq!(kept.told(&done(1)), Some(done(3)), "as the field counts");

        // Destroyed, and another made under its number, which counts anew.
        kept.send(&message(7, 0, &[])).unwrap();
        kept.told(&message(1, 1, &words(&[7]))).unwrap();
        kept.send(&message(5, 1, &words(&[7, 3]))).unwrap();
        kept.send(&message(7, 7, &[])).unwrap();
        assert_eq!(kept.told(&done(1)), Some(done(1)));
    }

    /// The globals an input method binds: a seat as 3, and what input
    /// methods are asked for by as 4.
    const TYPING: [(u32, &str, u32); 2] =
        [(1, "wl_seat", 7), (2, "zwp_input_method_manager_v2", 1)];

    #[test]
    fn an_input_method_typing_as_the_compositor_goes_is_let_go_of_and_counts_on() {
        let mut kept = Kept::registered(&TYPING);
        kept.bind(1, "wl_seat", 7, 3);
        kept.bind(2, "zwp_input_method_manager_v2", 1, 4);
        kept.send(&message(4, 0, &words(&[3, 5]))).unwrap();
        let done = message(5, 5, &[]);
        let commit = |serial| message(5, 3, &words(&[serial]));
        assert!(
            kept.told(&message(5, 0, &[])).is_some(),
            "typing in a field"
        );
        assert_eq!(kept.told(&done), Some(done.clone()));
        assert_eq!(kept.send(&commit(1)), Some(commit(1)), "counted alike");
        // Its keyboard grabbed, two keys pressed and one let go of.
        kept.send(&message(5, 5, &words(&[6]))).unwrap();
        let key = |key, pressed| message(6, 1, &words(&[9, 0, key, pressed]));
        // Its modifiers, and what kind of text the field takes, said too.
        let modifiers = message(6, 2, &words(&[9, 0, 0, 0, 0]));
        let content = message(5, 4, &words(&[0, 0]));
        for told in [key(30, 1), key(31, 1), key(31, 0), modifiers, content] {
            assert!(kept.told(&told).is_some());
        }

        kept.restarted(&TYPING);
        let told: Vec<Vec<u8>> = kept
            .keeping
            .to_program
            .drain(..)
            .map(|(bytes, _)| bytes)
            .collect();
        assert_eq!(
            told,
            [
                message(5, 1, &[]),
                done.clone(),
                message(6, 1, &words(&[0, 0, 30, 0]))
            ],
            "out of the field, and the key held let go of"
        );
        // Typing in it again under the next compositor, which counts anew.
        kept.told(&message(5, 0, &[])).unwrap();
        assert_eq!(kept.told(&done), Some(done));
        assert_eq!(
            kept.send(&commit(3)),
            Some(commit(1)),
            "as the compositor counts"
        );
        for told in [key(30, 1), key(30, 0)] {
            assert!(kept.told(&told).is_some());
        }

        // Not typing, and nothing held, as the next goes: nothing to say.
        kept.told(&message(5, 1, &[])).unwrap();
        kept.told(&message(5, 5, &[])).unwrap();
        kept.restarted(&TYPING);
        assert!(
            kept.keeping.to_program.is_empty(),
            "{:?}",
            kept.keeping.to_program
        );
    }

    /// The globals a program taking pictures binds: a screen as 3, what
    /// makes sources of screens as 4 and of windows as 5, what takes
    /// pictures as 6, a list of windows as 7, what makes buffers as 8, and
    /// a seat as 9.
    const PICTURING: [(u32, &str, u32); 7] = [
        (1, "wl_output", 4),
        (2, "ext_output_image_capture_source_manager_v1", 1),
        (3, "ext_foreign_toplevel_image_capture_source_manager_v1", 1),
        (4, "ext_image_copy_capture_manager_v1", 1),
        (5, "ext_foreign_toplevel_list_v1", 1),
        (6, "wp_single_pixel_buffer_manager_v1", 1),
        (7, "wl_seat", 7),
    ];

    /// A program that has bound each of [`PICTURING`].
    fn picturing() -> Kept {
        let mut kept = Kept::registered(&PICTURING);
        for (number, (name, interface, version)) in (3..).zip(PICTURING) {
            kept.bind(name, interface, version, number);
        }
        kept
    }

    /// Makes a buffer as `buffer`.
    fn buffer(kept: &mut Kept, buffer: u32) {
        kept.send(&message(8, 1, &words(&[buffer, 0, 0, 0, 0])))
            .unwrap();
    }

    /// Makes `frame` of `session`, to be taken in the buffer `buffer`, and
    /// asks for it if `asked`.
    fn shot(kept: &mut Kept, (session, frame, buffer): (u32, u32, u32), asked: bool) {
        kept.send(&message(session, 0, &words(&[frame]))).unwrap();
        kept.send(&message(frame, 1, &words(&[buffer]))).unwrap();
        kept.send(&message(frame, 2, &words(&[0, 0, 1, 1])))
            .unwrap();
        if asked {
            kept.send(&message(frame, CAPTURE, &[])).unwrap();
        }
    }

    /// What the relay sent the compositor, by object and opcode.
    fn sent(kept: &Kept) -> Vec<(u32, u16)> {
        headers(&kept.keeping.to_compositor)
    }

    #[test]
    fn a_picture_of_a_screen_asked_for_as_the_compositor_goes_fails_and_one_not_is_made_again() {
        let mut kept = picturing();
        // The screen's source, and three sessions on it, each with a
        // picture: asked for, made, and taken.
        kept.send(&message(4, 0, &words(&[10, 3]))).unwrap();
        for session in [11, 12, 13] {
            kept.send(&message(6, 0, &words(&[session, 10, 0])))
                .unwrap();
        }
        // The pointer followed, over the screen, then moving.
        kept.send(&message(9, 0, &words(&[14]))).unwrap();
        kept.send(&message(6, 1, &words(&[15, 10, 14]))).unwrap();
        for told in [
            message(15, POINTER_IN, &[]),
            message(15, 2, &words(&[1, 2])),
        ] {
            assert!(kept.told(&told).is_some());
        }
        buffer(&mut kept, 16);
        shot(&mut kept, (11, 17, 16), true);
        shot(&mut kept, (12, 18, 16), false);
        shot(&mut kept, (13, 19, 16), true);
        for told in [
            message(17, 1, &words(&[0, 0, 1, 1])),
            message(19, SHOT_READY, &[]),
        ] {
            assert!(kept.told(&told).is_some());
        }

        kept.restarted(&PICTURING);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [
                message(17, SHOT_FAILED, &words(&[UNKNOWN])),
                message(15, POINTER_OUT, &[])
            ],
            "the picture asked for failed, and the pointer left"
        );
        kept.keeping.to_program.clear();
        let made = sent(&kept);
        for again in [(6, 0), (6, 1), (12, 0), (18, 1), (18, 2)] {
            assert!(made.contains(&again), "{again:?} in {made:?}");
        }
        assert!(
            !made.contains(&(11, 0)) && !made.contains(&(13, 0)),
            "{made:?}"
        );
        // Those not made again are the relay's to free.
        assert_eq!(kept.send(&message(17, 0, &[])), None);
        assert_eq!(kept.send(&message(19, 0, &[])), None);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(1, 1, &words(&[17])), message(1, 1, &words(&[19]))]
        );
    }

    /// A program taking pictures of the window (7, 20), which it lists as
    /// `SERVERS` by the identifier "w", as the portal does: its source 10,
    /// let go of with the window's handle, a session 11 of it with a picture
    /// 16 asked for, and the pointer followed over it as 13, with a session
    /// 14 of pictures of that, and a picture 17 of it made; and the list let
    /// go of too unless `listing`. The compositor goes, with the window kept
    /// if `window`, and the next, offering `globals`, has the registries
    /// made again.
    fn picturing_a_window(window: bool, listing: bool, globals: &[(u32, &str, u32)]) -> Kept {
        let mut kept = picturing();
        let handle = SERVERS;
        kept.told(&toplevel(7, handle)).unwrap();
        kept.told(&saying(handle, IDENTIFIER, "w")).unwrap();
        kept.send(&message(5, 0, &words(&[10, handle]))).unwrap();
        kept.send(&message(6, 0, &words(&[11, 10, 0]))).unwrap();
        kept.send(&message(9, 0, &words(&[12]))).unwrap();
        kept.send(&message(6, 1, &words(&[13, 10, 12]))).unwrap();
        kept.send(&message(13, 1, &words(&[14]))).unwrap();
        buffer(&mut kept, 15);
        shot(&mut kept, (11, 16, 15), true);
        shot(&mut kept, (14, 17, 15), false);
        kept.send(&message(handle, 0, &[])).unwrap();
        kept.send(&message(10, 0, &[])).unwrap();
        if !listing {
            kept.send(&message(7, 1, &[])).unwrap();
        }

        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        let listed = BTreeMap::from([("w".to_owned(), Some((7, 20)))]);
        let windows = HashSet::from_iter(window.then_some((7, 20)));
        kept.keeping.unlist(&kept.objects, &listed, &windows);
        kept.keeping.join();
        let sync = *kept.keeping.syncs.first().unwrap();
        for (name, interface, version) in globals {
            assert_eq!(kept.told(&global(*name, interface, *version)), None);
        }
        kept.answer(sync);
        kept
    }

    #[test]
    fn what_takes_pictures_of_a_window_not_kept_stops_and_is_not_made_again() {
        let mut kept = picturing_a_window(false, true, &PICTURING);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [
                message(16, SHOT_FAILED, &words(&[UNKNOWN])),
                message(11, STOPPED, &[]),
                message(14, STOPPED, &[]),
            ],
            "as when the window closes"
        );
        kept.keeping.to_program.clear();
        let making = *kept.keeping.syncs.first().unwrap();
        kept.answer(making);
        let made = sent(&kept);
        for unmade in [(5, 0), (6, 0), (6, 1), (13, 1), (11, 0), (14, 0)] {
            assert!(!made.contains(&unmade), "{unmade:?} in {made:?}");
        }
        assert!(!kept.keeping.awaits_pictures());
        // Those not made again are the relay's to free.
        assert_eq!(kept.send(&message(11, 1, &[])), None);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(1, 1, &words(&[11]))]
        );
    }

    #[test]
    fn what_takes_pictures_of_a_window_kept_is_made_again_by_a_list_of_the_relays_own() {
        let mut kept = picturing_a_window(true, true, &PICTURING);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(16, SHOT_FAILED, &words(&[UNKNOWN]))],
            "not stopped"
        );
        // Not made again with the rest, nor looked for until that is.
        let made = sent(&kept);
        assert!(!made.contains(&(5, 0)), "{made:?}");
        kept.keeping.find(&kept.objects);
        let making = *kept.keeping.syncs.first().unwrap();
        kept.keeping.to_compositor.clear();
        kept.answer(making);
        assert!(kept.keeping.live());
        // Another window listed is not the one.
        kept.keeping.listed(b"w2", Some((7, 21)));
        kept.keeping.find(&kept.objects);
        assert!(kept.keeping.live());
        // Asked for meanwhile, a picture is kept, with the rest.
        assert_eq!(kept.send(&message(11, 0, &words(&[18]))), None);
        assert_eq!(kept.send(&message(18, 1, &words(&[15]))), None);
        assert_eq!(kept.send(&message(18, CAPTURE, &[])), None);
        // Found as it is listed, with the program waiting meanwhile.
        kept.keeping.to_compositor.clear();
        kept.keeping.listed(b"w1", Some((7, 20)));
        kept.keeping.find(&kept.objects);
        assert!(!kept.keeping.live());
        let list = kept.objects.highest().max(kept.keeping.numbering.top - 1);
        let bound =
            journal::said_between(2, 0, (&[5], b"ext_foreign_toplevel_list_v1", &[1, list]));
        assert_eq!(kept.keeping.to_compositor[0].0, bound.0);
        let finding = list + 1;
        assert_eq!(
            bytes(&kept.keeping.to_compositor[1..]),
            [message(1, 0, &words(&[finding]))]
        );
        kept.keeping.to_compositor.clear();
        let (other, window) = (SERVERS + 4, SERVERS + 5);
        for (handle, identifier) in [(other, "w2"), (window, "w1")] {
            assert_eq!(kept.told(&toplevel(list, handle)), None);
            assert_eq!(kept.told(&saying(handle, IDENTIFIER, identifier)), None);
            assert_eq!(kept.told(&saying(handle, TITLE, "t")), None);
        }
        // What the list says that is not looked for is kept from the
        // program too.
        assert_eq!(kept.told(&message(list, 1, &[])), None);
        kept.answer(finding);
        let made = sent(&kept);
        // The source, what was made of it, and the picture made, then the
        // one asked for meanwhile, asked for again.
        let remade = [(5, 0), (6, 0), (6, 1), (13, 1), (14, 0), (17, 1), (17, 2)];
        for remade in remade.into_iter().chain([(11, 0), (18, 1), (18, CAPTURE)]) {
            assert!(made.contains(&remade), "{remade:?} in {made:?}");
        }
        let source = kept
            .keeping
            .to_compositor
            .iter()
            .find(|(bytes, _)| wire::header(bytes).unwrap().unwrap().object == 5);
        assert_eq!(wire::word(&source.unwrap().0, HEADER + 4), Some(window));
        let at = |sent: (u32, u16)| made.iter().position(|made| *made == sent).unwrap();
        assert!(at((5, 0)) < at((6, 0)) && at((6, 0)) < at((11, 0)));
        assert!(
            at((11, 0)) < at((10, 0)),
            "the source let go of again after"
        );
        assert!(at((18, 1)) < at((18, CAPTURE)));
        let ending = made.len() - 4;
        assert_eq!(
            made[ending..],
            [(other, 0), (window, 0), (list, 1), (1, 0)],
            "the relay's list let go of"
        );
        let ended = kept.keeping.numbering.top;
        assert!(!kept.keeping.live());
        kept.answer(ended);
        assert!(kept.keeping.live());
        assert!(!kept.keeping.awaits_pictures());
        // The window's numbers are the compositor's again, the relay's list
        // let go of.
        assert_eq!(kept.told(&message(1, 1, &words(&[list]))), None);
        assert!(!kept.keeping.private.contains(&window));
        assert!(
            kept.keeping
                .to_program
                .iter()
                .all(|(bytes, _)| { wire::header(bytes).unwrap().unwrap().opcode != STOPPED })
        );
        // What was made again is passed on as any is.
        assert!(kept.send(&message(18, CAPTURE, &[])).is_some());
    }

    #[test]
    fn pictures_of_a_window_that_is_not_in_the_relays_list_or_has_gone_stop() {
        // Listed, then gone by the time the relay's list is whole.
        let mut kept = picturing_a_window(true, true, &PICTURING);
        let making = *kept.keeping.syncs.first().unwrap();
        kept.answer(making);
        kept.keeping.to_program.clear();
        kept.keeping.listed(b"w1", Some((7, 20)));
        kept.keeping.find(&kept.objects);
        let finding = kept.keeping.numbering.top;
        kept.answer(finding);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [message(11, STOPPED, &[]), message(14, STOPPED, &[])]
        );

        // Not offered a list by the next compositor.
        let unlisting: Vec<(u32, &str, u32)> = PICTURING
            .into_iter()
            .filter(|(_, interface, _)| *interface != "ext_foreign_toplevel_list_v1")
            .collect();
        let mut kept = picturing_a_window(true, false, &unlisting);
        let making = *kept.keeping.syncs.first().unwrap();
        kept.answer(making);
        kept.keeping.to_program.clear();
        kept.keeping.listed(b"w1", Some((7, 20)));
        kept.keeping.find(&kept.objects);
        assert!(kept.keeping.live());
        assert!(!kept.keeping.awaits_pictures());

        // Its program gone first, with a picture asked for meanwhile.
        let mut kept = picturing_a_window(true, true, &PICTURING);
        let making = *kept.keeping.syncs.first().unwrap();
        kept.answer(making);
        kept.send(&message(17, CAPTURE, &[]));
        kept.keeping.to_program.clear();
        kept.keeping
            .windows_gone(&kept.objects, &HashSet::from([(7, 20)]));
        assert!(kept.keeping.awaits_pictures(), "nothing said of it yet");
        kept.keeping.windows_gone(&kept.objects, &HashSet::new());
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [
                message(11, STOPPED, &[]),
                message(14, STOPPED, &[]),
                message(17, SHOT_FAILED, &words(&[GONE])),
            ]
        );
        assert!(!kept.keeping.awaits_pictures());
    }

    #[test]
    fn what_a_buffer_has_to_be_is_told_again_only_if_the_next_compositor_says_otherwise() {
        let mut kept = picturing();
        kept.send(&message(4, 0, &words(&[10, 3]))).unwrap();
        for session in [11, 12] {
            kept.send(&message(6, 0, &words(&[session, 10, 0])))
                .unwrap();
        }
        let batch = |session, width| {
            [
                message(session, 0, &words(&[width, 50])),
                message(session, 1, &words(&[1])),
                message(session, CONSTRAINED, &[]),
            ]
        };
        for told in batch(11, 100).into_iter().chain(batch(12, 100)) {
            assert_eq!(kept.told(&told), Some(told.clone()), "told as said");
        }

        kept.restarted(&PICTURING);
        for told in batch(11, 100) {
            assert_eq!(kept.told(&told), None, "the same, not told again");
        }
        let [size, format, done] = batch(12, 200);
        assert_eq!(kept.told(&size), None, "held until it is whole");
        assert_eq!(kept.told(&format), None);
        assert_eq!(kept.told(&done), Some(done.clone()));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            [size, format],
            "told before the end of it"
        );
        // Said again after that, it is told as it is said.
        kept.keeping.to_program.clear();
        for told in batch(11, 300) {
            assert_eq!(kept.told(&told), Some(told.clone()));
        }
        let stopped = message(11, STOPPED, &[]);
        assert_eq!(kept.told(&stopped), Some(stopped.clone()));
    }

    #[test]
    fn an_input_method_refused_is_not_made_again_nor_what_it_made() {
        let mut kept = Kept::registered(&TYPING);
        kept.bind(1, "wl_seat", 7, 3);
        kept.bind(2, "zwp_input_method_manager_v2", 1, 4);
        kept.send(&message(4, 0, &words(&[3, 5]))).unwrap();
        assert!(
            kept.told(&message(5, 6, &[])).is_some(),
            "told another is the one"
        );
        kept.send(&message(5, 5, &words(&[6]))).unwrap();

        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        kept.keeping.join();
        let made: Vec<(u32, u16)> = kept
            .keeping
            .to_compositor
            .iter()
            .map(|(bytes, _)| wire::header(bytes).unwrap().unwrap())
            .map(|header| (header.object, header.opcode))
            .collect();
        assert!(
            !made.contains(&(4, 0)) && !made.contains(&(5, 5)),
            "{made:?}"
        );
        // Let go of, its numbers are the relay's to free.
        assert_eq!(kept.send(&message(6, 0, &[])), None);
        assert_eq!(kept.send(&message(5, 6, &[])), None);
        let freed: Vec<Vec<u8>> = kept
            .keeping
            .to_program
            .drain(..)
            .map(|(bytes, _)| bytes)
            .collect();
        assert_eq!(
            freed,
            [message(1, 1, &words(&[6])), message(1, 1, &words(&[5]))]
        );
    }

    /// The globals a locker binds: what locks the session as 3, what makes
    /// surfaces as 4, and a screen as 5.
    const LOCKING: [(u32, &str, u32); 3] = [
        (1, "ext_session_lock_manager_v1", 1),
        (2, "wl_compositor", 6),
        (3, "wl_output", 4),
    ];

    /// A locker with the session locked by 7, asked with the surface 6
    /// shown on the screen as the lock surface 8, which is to be 640 by 480,
    /// as was said with the serial 1.
    fn locking() -> Kept {
        let mut kept = Kept::registered(&LOCKING);
        kept.bind(1, "ext_session_lock_manager_v1", 1, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.bind(3, "wl_output", 4, 5);
        kept.send(&message(4, 0, &words(&[6]))).unwrap();
        kept.send(&message(3, 1, &words(&[7]))).unwrap();
        kept.send(&message(7, 1, &words(&[8, 6, 5]))).unwrap();
        kept.told(&message(8, 0, &words(&[1, 640, 480]))).unwrap();
        kept.send(&message(8, 1, &words(&[1]))).unwrap();
        kept.send(&message(6, 6, &[])).unwrap();
        kept
    }

    /// What has been sent to the compositor, by the object each is to and
    /// which request it is.
    fn asked(kept: &Kept) -> Vec<(u32, u16)> {
        let headers = kept.keeping.to_compositor.iter();
        let headers = headers.map(|(bytes, _)| wire::header(bytes).unwrap().unwrap());
        headers
            .map(|header| (header.object, header.opcode))
            .collect()
    }

    #[test]
    fn a_lock_is_made_again_and_its_locker_not_told_again_that_the_session_is_locked() {
        let mut kept = locking();
        assert!(kept.told(&message(7, 0, &[])).is_some(), "locked");

        kept.restarted(&LOCKING);
        let made = asked(&kept);
        assert!(made.contains(&(3, 1)) && made.contains(&(7, 1)), "{made:?}");
        assert!(
            !made.contains(&(6, 6)),
            "not committed before its first configure is acked: {made:?}"
        );
        // Told the size it was, the relay acks it, and shows what was shown.
        let same = message(8, 0, &words(&[3, 640, 480]));
        assert_eq!(kept.told(&same), None, "the same, so not told");
        let made = asked(&kept);
        let acked = made.iter().position(|asked| *asked == (8, 1));
        let shown = made.iter().position(|asked| *asked == (6, 6));
        assert!(acked.is_some() && shown > acked, "{made:?}");
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        assert_eq!(kept.told(&message(7, 0, &[])), None, "locked already");
        assert!(kept.keeping.to_program.is_empty());
        assert!(kept.send(&message(7, 2, &[])).is_some(), "unlocked");
    }

    #[test]
    fn a_lock_not_yet_said_to_be_locked_is_told_so_by_the_next_compositor() {
        let mut kept = locking();
        kept.restarted(&LOCKING);
        assert_eq!(kept.told(&message(8, 0, &words(&[3, 640, 480]))), None);
        let sync = *kept.keeping.syncs.first().unwrap();
        kept.answer(sync);
        let locked = message(7, 0, &[]);
        assert_eq!(kept.told(&locked), Some(locked));
    }

    #[test]
    fn a_lock_surface_told_another_size_is_the_lockers_to_ack_and_draw_again() {
        let mut kept = locking();
        kept.told(&message(7, 0, &[])).unwrap();
        kept.restarted(&LOCKING);
        let told = kept.told(&message(8, 0, &words(&[3, 800, 600])));
        let told = told.expect("another, so told");
        assert_eq!(wire::word(&told, HEADER + 4), Some(800));
        let made = asked(&kept);
        assert!(
            !made.contains(&(8, 1)) && !made.contains(&(6, 6)),
            "what was shown is of another size, so neither acked nor shown by the relay: {made:?}"
        );
        let serial = wire::word(&told, HEADER).unwrap();
        assert!(
            kept.send(&message(8, 1, &words(&[serial]))).is_some(),
            "acked by the locker"
        );
    }

    #[test]
    fn a_lock_refused_is_not_made_again_nor_what_it_made() {
        let mut kept = locking();
        assert!(kept.told(&message(7, 1, &[])).is_some(), "refused");
        // A second surface asked for, before the locker heard.
        kept.send(&message(4, 0, &words(&[9]))).unwrap();
        kept.send(&message(7, 1, &words(&[10, 9, 5]))).unwrap();

        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        kept.keeping.join();
        let made = asked(&kept);
        assert!(
            !made.contains(&(3, 1)) && !made.contains(&(7, 1)),
            "{made:?}"
        );
        // Let go of, their numbers are the relay's to free.
        for gone in [10, 8, 7] {
            assert_eq!(kept.send(&message(gone, 0, &[])), None);
        }
        let freed: Vec<Vec<u8>> = kept
            .keeping
            .to_program
            .drain(..)
            .map(|(bytes, _)| bytes)
            .collect();
        let deleted = |number| message(1, 1, &words(&[number]));
        assert_eq!(freed, [deleted(10), deleted(8), deleted(7)]);
    }

    #[test]
    fn how_large_a_screen_is_told_again_as_the_relay_catches_up_is_kept_from_the_program() {
        let globals = [(1, "wl_output", 4), (2, "zxdg_output_manager_v1", 3)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "wl_output", 4, 3);
        kept.bind(2, "zxdg_output_manager_v1", 3, 4);
        kept.send(&message(4, 1, &words(&[5, 3]))).unwrap();
        let size = message(5, 1, &words(&[1920, 1080]));
        assert!(kept.told(&size).is_some());
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        kept.keeping.join();
        assert_eq!(kept.told(&size), None);
    }

    #[test]
    fn what_says_how_something_is_is_told_the_same_again_only_with_a_configure() {
        assert_eq!(describes("wl_surface", 2), Some(true), "a scale");
        assert_eq!(describes("wl_surface", 3), Some(true), "a transform");
        assert_eq!(describes("xdg_toplevel", 2), Some(false), "bounds");
        assert_eq!(describes("xdg_toplevel", 0), None, "a configure itself");
    }

    #[test]
    fn a_program_bound_to_what_the_next_compositor_no_longer_offers_cannot_be_kept() {
        let mut kept = Kept::registered(&[(1, "wl_shm", 1)]);
        kept.bind(1, "wl_shm", 1, 3);
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        kept.keeping.join();
        let sync = *kept.keeping.syncs.first().unwrap();
        let mut done = message(sync, 0, &words(&[0]));
        let header = wire::header(&done).unwrap().unwrap();
        let answered = kept.keeping.before(&kept.objects, header, &mut done);
        assert_eq!(answered, Err(Broken("a global that is no longer offered")));
    }

    #[test]
    fn a_program_holding_what_cannot_be_made_again_is_not_kept() {
        let mut kept = Kept::registered(&[(1, "wl_seat", 7)]);
        kept.bind(1, "wl_seat", 7, 3);
        // Touch, which no compositor offers yet.
        kept.send(&message(3, 2, &words(&[4]))).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Err(vec!["wl_touch"]));
    }

    /// The bytes of what is sent.
    fn bytes(sending: &[Sending]) -> Vec<Vec<u8>> {
        sending.iter().map(|(bytes, _)| bytes.clone()).collect()
    }

    /// The headers of what is sent, each its object and opcode.
    fn headers(sending: &[Sending]) -> Vec<(u32, u16)> {
        let header = |bytes: &[u8]| wire::header(bytes).unwrap().unwrap();
        let sent = sending.iter().map(|(bytes, _)| header(bytes));
        sent.map(|header| (header.object, header.opcode)).collect()
    }

    #[test]
    fn every_number_a_program_has_taken_is_taken_by_the_next_compositor_freed_or_not() {
        let globals = [(1, "wl_compositor", 6)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "wl_compositor", 6, 3);
        // Surfaces 4 to 7, the last three destroyed and their numbers freed:
        // the program may take any of them next, as libwayland takes the
        // last freed first.
        for surface in 4..=7 {
            kept.send(&message(3, 0, &words(&[surface]))).unwrap();
        }
        for surface in 5..=7 {
            kept.send(&message(surface, 0, &[])).unwrap();
            kept.told(&message(DISPLAY, DELETE_ID, &words(&[surface])))
                .unwrap();
        }

        kept.restarted(&globals);
        let synced = kept.keeping.to_compositor.iter().filter_map(|(bytes, _)| {
            let header = wire::header(bytes).unwrap().unwrap();
            let sync = (header.object, header.opcode) == (DISPLAY, 0);
            sync.then(|| wire::word(bytes, HEADER).unwrap())
        });
        let synced: Vec<u32> = synced.collect();
        for freed in 5..=7 {
            assert!(
                synced.contains(&freed),
                "{freed} taken, and freed, by the next compositor: {synced:?}"
            );
        }
    }

    #[test]
    fn what_lasts_only_a_moment_is_told_it_has_ended_and_is_the_relays_to_answer_for() {
        let globals = [
            (1, "wl_compositor", 6),
            (2, "xdg_wm_base", 6),
            (3, "xdg_activation_v1", 1),
            (4, "wp_presentation", 2),
            (5, "wp_single_pixel_buffer_manager_v1", 1),
        ];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        // A window, 8, and two menus on it, 9 and 10, of which the second
        // has been told it is over.
        for surface in [8, 9, 10] {
            kept.send(&message(3, 0, &words(&[surface]))).unwrap();
        }
        kept.send(&message(4, 2, &words(&[11, 8]))).unwrap();
        kept.send(&message(11, 1, &words(&[12]))).unwrap();
        kept.send(&message(4, 1, &words(&[13]))).unwrap();
        for (xdg_surface, surface, popup) in [(14, 9, 15), (16, 10, 17)] {
            kept.send(&message(4, 2, &words(&[xdg_surface, surface])))
                .unwrap();
            kept.send(&message(xdg_surface, 2, &words(&[popup, 11, 13])))
                .unwrap();
        }
        // The first shows a buffer, and is to show it again.
        kept.send(&message(7, 1, &words(&[18, 0, 0, 0, 0])))
            .unwrap();
        kept.send(&message(9, 1, &words(&[18, 0, 0]))).unwrap();
        kept.send(&message(9, 6, &[])).unwrap();
        kept.send(&message(9, 1, &words(&[18, 0, 0]))).unwrap();
        // Two tokens, of which only the first has been given, and word
        // asked for of the window's next picture.
        kept.send(&message(5, 1, &words(&[19]))).unwrap();
        kept.send(&message(5, 1, &words(&[20]))).unwrap();
        kept.send(&message(6, 1, &words(&[8, 21]))).unwrap();
        kept.told(&message(19, 0, &text("given"))).unwrap();
        kept.told(&message(17, 1, &[])).unwrap();

        kept.restarted(&globals);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            bytes(&[
                journal::message(15, 1, &[]),
                journal::said(20, 0, ""),
                journal::message(21, 2, &[]),
                journal::message(DISPLAY, DELETE_ID, &[21]),
                journal::message(18, 0, &[]),
            ]),
            "the menu not yet over is, the token not yet given is given one \
             no compositor takes, the picture was not shown, and the menu's \
             buffer, shown no more, is released"
        );
        let made = headers(&kept.keeping.to_compositor);
        let ended = [14, 15, 16, 17, 19, 20, 21];
        assert!(
            !made.iter().any(|(object, _)| ended.contains(object)),
            "none of it made again: {made:?}"
        );
        assert!(!made.contains(&(9, 1)), "nor shown in a menu's surface");
        assert!(made.contains(&(8, 6)), "the window's is");
        assert!(kept.keeping.numbering.top >= 21, "and its numbers taken");

        // Asked of, or named, it is not passed on; let go of, the relay
        // frees its number.
        kept.send(&message(4, 2, &words(&[22, 10]))).unwrap();
        assert_eq!(kept.send(&message(22, 2, &words(&[23, 16, 13]))), None);
        assert_eq!(kept.send(&message(15, 0, &[])), None);
        assert_eq!(kept.send(&message(19, 4, &[])), None);
        assert_eq!(
            bytes(&kept.keeping.to_program[5..]),
            bytes(&[
                journal::message(DISPLAY, DELETE_ID, &[15]),
                journal::message(DISPLAY, DELETE_ID, &[19]),
            ])
        );
    }

    #[test]
    fn a_gesture_under_way_as_the_compositor_goes_is_told_it_was_cancelled() {
        let globals = [
            (1, "wl_compositor", 6),
            (2, "wl_seat", 9),
            (3, "zwp_pointer_gestures_v1", 3),
        ];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        kept.send(&message(3, 0, &words(&[6]))).unwrap();
        kept.send(&message(4, 0, &words(&[7]))).unwrap();
        // A swipe, a pinch and fingers held still.
        for (opcode, gesture) in [(0, 8), (1, 9), (3, 10)] {
            kept.send(&message(5, opcode, &words(&[gesture, 7])))
                .unwrap();
        }
        for gesture in [8, 9, 10] {
            kept.told(&message(gesture, 0, &words(&[1, 2, 6, 3])))
                .unwrap();
        }
        kept.told(&message(9, 2, &words(&[3, 4, 0]))).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            bytes(&[
                journal::message(8, 2, &[0, 0, 1]),
                journal::message(10, 1, &[0, 0, 1]),
            ]),
            "the pinch was over"
        );
    }

    #[test]
    fn a_pointer_held_is_let_go_of_as_the_compositor_goes_and_one_held_once_is_over() {
        let globals = [
            (1, "wl_compositor", 6),
            (2, "wl_seat", 9),
            (3, "zwp_pointer_constraints_v1", 1),
        ];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        kept.send(&message(3, 0, &words(&[6]))).unwrap();
        kept.send(&message(4, 0, &words(&[7]))).unwrap();
        // Kept within the surface for as long as it lasts, and held: once
        // and not yet, once and let go of, and once and held now.
        for (opcode, made, lifetime) in [(2, 8, 2), (1, 9, 1), (2, 10, 1), (1, 11, 1)] {
            kept.send(&message(5, opcode, &words(&[made, 6, 7, 0, lifetime])))
                .unwrap();
        }
        for told in [(8, 0), (10, 0), (10, 1), (11, 0)] {
            kept.told(&message(told.0, told.1, &[])).unwrap();
        }
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            bytes(&[journal::message(8, 1, &[]), journal::message(11, 1, &[])]),
            "those held let go of"
        );
        let orphans = &kept.keeping.orphans;
        assert!(orphans.contains(&10) && orphans.contains(&11), "over");
        assert!(!orphans.contains(&8) && !orphans.contains(&9));

        // One over is the relay's to let go of.
        kept.keeping.to_program.clear();
        assert_eq!(kept.send(&message(10, 0, &[])), None);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            bytes(&[journal::message(DISPLAY, DELETE_ID, &[10])])
        );
    }

    #[test]
    fn a_program_told_the_user_is_away_is_told_they_are_back_as_the_compositor_goes() {
        let globals = [(1, "wl_seat", 9), (2, "ext_idle_notifier_v1", 2)];
        let mut kept = Kept::registered(&globals);
        for (name, interface, version) in globals {
            kept.bind(name, interface, version, name + 2);
        }
        // Told away; told away and back; told away and destroyed; and not
        // told yet, of input alone.
        for (opcode, made) in [(1, 5), (1, 6), (1, 7), (2, 8)] {
            kept.send(&message(4, opcode, &words(&[made, 40, 3])))
                .unwrap();
        }
        for told in [(5, 0), (6, 0), (6, 1), (7, 0)] {
            kept.told(&message(told.0, told.1, &[])).unwrap();
        }
        kept.send(&message(7, 0, &[])).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        assert_eq!(
            bytes(&kept.keeping.to_program),
            bytes(&[journal::message(5, 1, &[])])
        );
    }

    /// The event of the list of windows `list` that lists one as `handle`.
    fn toplevel(list: u32, handle: u32) -> Vec<u8> {
        message(list, 0, &words(&[handle]))
    }

    /// The event `opcode` of the window `handle` in a list, saying `said`.
    fn saying(handle: u32, opcode: u16, said: &str) -> Vec<u8> {
        message(handle, opcode, &text(said))
    }

    #[test]
    fn a_window_listed_and_kept_is_the_one_the_program_has_once_the_next_compositor_lists_it() {
        let globals = [(1, "ext_foreign_toplevel_list_v1", 1)];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "ext_foreign_toplevel_list_v1", 1, 3);
        // Kept and said the same, let go of meanwhile, not relayed, kept and
        // said anew, and let go of at once.
        let (same, gone, alone, anew, dropped) =
            (SERVERS, SERVERS + 1, SERVERS + 2, SERVERS + 3, SERVERS + 4);
        let listing = [
            (same, "a"),
            (gone, "b"),
            (alone, "c"),
            (anew, "d"),
            (dropped, "e"),
        ];
        for (handle, identifier) in listing {
            kept.told(&toplevel(3, handle)).unwrap();
            kept.told(&saying(handle, IDENTIFIER, identifier)).unwrap();
            kept.told(&saying(handle, TITLE, "one")).unwrap();
            kept.told(&message(handle, LISTED_DONE, &[])).unwrap();
        }
        // One the program lets go of as the compositor says more of it.
        kept.send(&message(dropped, 0, &[])).unwrap();
        kept.told(&saying(dropped, TITLE, "two")).unwrap();

        // The windows of a, b and d are kept; c's is not relayed.
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        let listed = BTreeMap::from([
            ("a".to_owned(), Some((7, 20))),
            ("b".to_owned(), Some((7, 21))),
            ("c".to_owned(), None),
            ("d".to_owned(), Some((7, 23))),
        ]);
        let windows = HashSet::from([(7, 20), (7, 21), (7, 23)]);
        kept.keeping.unlist(&kept.objects, &listed, &windows);
        assert_eq!(
            bytes(&kept.keeping.to_program),
            bytes(&[journal::message(alone, CLOSED, &[])])
        );
        // b's is let go of while the compositor is away.
        assert_eq!(kept.send(&message(gone, 0, &[])), None);
        kept.keeping.join();
        let sync = *kept.keeping.syncs.first().unwrap();
        assert_eq!(
            kept.told(&global(1, "ext_foreign_toplevel_list_v1", 1)),
            None
        );
        kept.answer(sync);

        // b's window, known before it is listed, is new to the program, as
        // is one that says something before its identifier.
        kept.keeping.listed(b"b2", Some((7, 21)));
        let theirs = SERVERS + 10;
        assert_eq!(kept.told(&toplevel(3, theirs)), None);
        assert_eq!(kept.told(&saying(theirs, IDENTIFIER, "b2")), None);
        let told = kept.again();
        assert!(told.iter().all(Option::is_some), "{told:?}");
        assert_eq!(kept.told(&toplevel(3, theirs + 1)), None);
        assert_eq!(kept.told(&saying(theirs + 1, TITLE, "new")), None);
        let told = kept.again();
        assert!(told.iter().all(Option::is_some), "{told:?}");

        // a's, known only once said, is the one it has, told nothing new.
        assert_eq!(kept.told(&toplevel(3, theirs + 2)), None);
        assert_eq!(kept.told(&saying(theirs + 2, IDENTIFIER, "a2")), None);
        assert_eq!(kept.told(&saying(theirs + 2, TITLE, "one")), None);
        kept.keeping.listed(b"a2", Some((7, 20)));
        kept.keeping.unlisted(b"gone");
        assert_eq!(kept.again(), [None], "the title it has");
        assert_eq!(kept.told(&message(theirs + 2, LISTED_DONE, &[])), None);

        // d's, known as it is said, is told what changed, under its number.
        kept.keeping.listed(b"d2", Some((7, 23)));
        assert_eq!(kept.told(&toplevel(3, theirs + 3)), None);
        assert_eq!(kept.told(&saying(theirs + 3, IDENTIFIER, "d2")), None);
        assert_eq!(kept.again(), []);
        let retitled = kept.told(&saying(theirs + 3, TITLE, "two"));
        assert_eq!(retitled, Some(saying(anew, TITLE, "two")));
        let done = kept.told(&message(theirs + 3, LISTED_DONE, &[]));
        assert_eq!(done, Some(message(anew, LISTED_DONE, &[])));
        assert_eq!(
            kept.send(&message(anew, 0, &[])),
            Some(message(theirs + 3, 0, &[]))
        );

        // None is awaited now, and one listed is told as it is said, and
        // that it has closed.
        assert!(kept.told(&toplevel(3, theirs + 4)).is_some());
        assert!(kept.keeping.known.is_empty());
        assert!(kept.told(&saying(theirs + 4, APP_ID, "x")).is_some());
        assert!(kept.told(&message(theirs + 4, CLOSED, &[])).is_some());
        assert_eq!(kept.keeping.listings.len(), 3, "the same, the new two");
    }

    /// The importer 4's request that imports, as `imported`, the window
    /// shared by `handle`.
    fn import(imported: u32, handle: &str) -> Vec<u8> {
        let mut body = words(&[imported]);
        body.extend(text(handle));
        message(4, IMPORT, &body)
    }

    #[test]
    fn a_window_imported_by_a_handle_of_the_compositor_before_is_imported_again_by_the_next() {
        let globals = [
            (1, "zxdg_exporter_v2", 1),
            (2, "zxdg_importer_v2", 1),
            (3, "wl_compositor", 6),
        ];
        let mut kept = Kept::registered(&globals);
        kept.bind(1, "zxdg_exporter_v2", 1, 3);
        kept.bind(2, "zxdg_importer_v2", 1, 4);
        kept.bind(3, "wl_compositor", 6, 5);
        for surface in [6, 7, 8] {
            kept.send(&message(5, 0, &words(&[surface]))).unwrap();
        }
        // A window of its own shared, which it imports too; another
        // program's kept, whose dialog is let go of; and one not kept.
        kept.send(&message(3, 1, &words(&[9, 6]))).unwrap();
        kept.told(&message(9, HANDLE, &text("h1"))).unwrap();
        for (imported, handle) in [(10, "h1"), (11, "h3"), (12, "x")] {
            kept.send(&import(imported, handle)).unwrap();
        }
        kept.send(&message(10, PARENT_OF, &words(&[7]))).unwrap();
        kept.send(&message(11, PARENT_OF, &words(&[8]))).unwrap();
        kept.send(&message(8, 0, &[])).unwrap();
        assert_eq!(kept.keeping.exports().collect::<Vec<_>>(), [b"h1"]);

        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        kept.keeping
            .unimport(&HashSet::from([b"h1".to_vec(), b"h3".to_vec()]));
        kept.keeping.join();
        let sync = *kept.keeping.syncs.first().unwrap();
        for (name, interface, version) in globals {
            assert_eq!(kept.told(&global(name, interface, version)), None);
        }
        // Its own is shared anew before it is made again, and imported again
        // as soon as it is, with its dialog; a handle not imported is
        // nothing to it.
        kept.keeping.exported(b"h1", b"h2");
        kept.keeping.exported(b"h0", b"h9");
        kept.keeping.to_compositor.clear();
        kept.answer(sync);
        let sent = bytes(&kept.keeping.to_compositor);
        let again = [
            journal::message(10, UNIMPORT, &[]),
            journal::said_after(4, IMPORT, &[10], b"h2"),
            journal::message(10, PARENT_OF, &[7]),
        ];
        assert_eq!(sent[sent.len() - 4..sent.len() - 1], bytes(&again));
        // What the next compositor says of each made again by the old
        // handle is kept from the program, but of the one not kept.
        assert_eq!(kept.told(&message(10, IMPORT_GONE, &[])), None);
        assert_eq!(kept.told(&message(11, IMPORT_GONE, &[])), None);
        assert!(kept.told(&message(12, IMPORT_GONE, &[])).is_some());
        assert!(kept.told(&message(9, HANDLE, &text("h2"))).is_some());
        assert_eq!(kept.keeping.renamed, [(b"h1".to_vec(), b"h2".to_vec())]);

        // The other program's, shared anew once it was made again, is
        // imported again at once, without the dialog it no longer has.
        kept.keeping.to_compositor.clear();
        kept.keeping.exported(b"h3", b"h4");
        let again = [
            journal::message(11, UNIMPORT, &[]),
            journal::said_after(4, IMPORT, &[11], b"h4"),
        ];
        assert_eq!(bytes(&kept.keeping.to_compositor), bytes(&again));
        // The numbers the relay's destroying frees are kept from the program,
        // which frees them itself as it destroys what it has.
        assert_eq!(kept.told(&message(1, DELETE_ID, &words(&[10]))), None);
        assert_eq!(kept.told(&message(1, DELETE_ID, &words(&[11]))), None);
        assert!(kept.send(&message(10, UNIMPORT, &[])).is_some());
        assert!(kept.told(&message(1, DELETE_ID, &words(&[10]))).is_some());
        assert_eq!(kept.keeping.imports.len(), 2);
    }

    #[test]
    fn a_buffer_on_the_card_the_compositor_made_cannot_be_made_again_while_held() {
        let mut kept = Kept::registered(&[(1, "zwp_linux_dmabuf_v1", 3)]);
        kept.bind(1, "zwp_linux_dmabuf_v1", 3, 3);
        kept.send(&message(3, 1, &words(&[4]))).unwrap();
        kept.send(&message(4, 2, &words(&[16, 16, 0x3432_5258, 0])))
            .unwrap();
        kept.told(&message(4, 0, &words(&[SERVERS]))).unwrap();
        assert_eq!(
            kept.keeping.lost(&kept.objects),
            Err(vec![journal::MADE_BY_THE_COMPOSITOR])
        );
        kept.send(&message(SERVERS, 0, &[])).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()), "let go of");
    }
}
