//! Keeping a program through a change of compositor (ADR 61, step 2).
//!
//! When the compositor ends, the program waits, told only that the pointer,
//! the keyboard and any field typed into have left its surfaces. When the next is joined, what
//! the program had is made again in two parts, each ended by a sync of the
//! relay's own: its registries, so that the new compositor's globals are
//! known, then the rest, as [`Journal`] kept it. Until the second is
//! answered, what the compositor says is sifted: what the program knows
//! already is kept from it.
//!
//! Some numbers mean something only to one compositor, and the relay puts
//! the program's in their place and theirs in the program's: the names of
//! globals, and the numbers of the objects a compositor makes, which a new
//! one takes from where the one before did while the program may still hold
//! some of those. Of serials it changes only a field's, which counts the
//! field's commits: the new compositor counts from when it made the field
//! again. The rest it does not change: a window's configure acked by the
//! relay is not acked again, nor one the compositor before sent, and a
//! program told the pointer has come back names its shape again with the
//! new compositor's serial, as it would to any compositor.

use std::collections::{HashMap, HashSet};

use wayland_backend::protocol::MessageDesc;

use crate::journal::{self, Journal, Numbering, Sending};
use crate::objects::{Objects, Read, SERVERS};
use crate::wire::{self, Broken, HEADER, Header};

/// The display, which every connection starts with.
const DISPLAY: u32 = 1;

/// Which of a field's events ends what is typed into it for now, with the
/// number of times it has been committed.
const TEXT_DONE: u16 = 5;

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

/// A window, as far as its configures go.
#[derive(Debug, Default)]
struct Window {
    /// The serials the compositor has sent the program for it, not yet
    /// acked.
    offered: Vec<u32>,
    /// What the compositor last said the window is to be, and what the
    /// program last acked: the arguments of a toplevel's configure.
    told: Option<Vec<u8>>,
    acked: Option<Vec<u8>>,
    /// A toplevel's configure held while the relay catches up.
    held: Option<Sending>,
}

/// What has the pointer, the keyboard or a field typed into, as the program
/// was told.
#[derive(Debug, Clone, Copy)]
struct Focus {
    surface: u32,
    /// The serial it was told it with, but for a field's, which has none.
    serial: Option<u32>,
    /// Which event says it has left.
    leave: u16,
    /// Whether a frame ends what a pointer says, as from version 5.
    framed: bool,
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
    /// The windows, by their `xdg_surface`, and each toplevel's.
    windows: HashMap<u32, Window>,
    toplevels: HashMap<u32, u32>,
    /// What has the pointer, the keyboard or a field, by the pointer,
    /// keyboard or field.
    focus: HashMap<u32, Focus>,
    /// How many times each field has been committed: as the program counts,
    /// and as the compositor does, which counts from when it made it.
    commits: HashMap<u32, (u32, u32)>,
    /// What the program was last told of how something is, by the object and
    /// the event that said it.
    described: HashMap<(u32, u16), Vec<u8>>,
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
            toplevels: HashMap::new(),
            focus: HashMap::new(),
            commits: HashMap::new(),
            described: HashMap::new(),
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

    /// Takes in the request `bytes`, `described`, which `read` says the
    /// program sent; changes in it what means something only to the
    /// compositor before. Returns whether it is to be passed on.
    pub fn request(&mut self, read: &Read, bytes: &mut [u8], described: &MessageDesc) -> bool {
        let header = Header {
            object: wire::word(bytes, 0).unwrap_or_default(),
            opcode: read.opcode,
            size: bytes.len(),
        };
        if let Some((made, interface, _)) = read.made {
            // What was told of what had the number before is not of this.
            self.described.retain(|(object, _), _| *object != made);
            self.commits.remove(&made);
            if interface.name == "xdg_toplevel" {
                self.toplevels.insert(made, header.object);
                self.windows.entry(header.object).or_default();
            }
        }
        if read.interface.name == "xdg_surface" && described.name == "ack_configure" {
            let serial = wire::word(bytes, HEADER).unwrap_or_default();
            let window = self.windows.entry(header.object).or_default();
            if !window.offered.contains(&serial) {
                // Acked by the relay, or sent by the compositor before.
                return false;
            }
            window.offered.retain(|offered| *offered > serial);
            window.acked.clone_from(&window.told);
        }
        if read.interface.name == "zwp_text_input_v3" && described.name == "commit" {
            let commits = self.commits.entry(header.object).or_default();
            *commits = (commits.0.wrapping_add(1), commits.1.wrapping_add(1));
        }
        if read.interface.name == "wl_registry" && described.name == "bind" {
            let name = wire::word(bytes, HEADER).unwrap_or_default();
            wire::put(bytes, HEADER, self.names.theirs(name));
        }
        if self.orphans.contains(&header.object) {
            if read.ends {
                self.orphans.remove(&header.object);
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
            (!(ours && orphans.contains(&number)))
                .then(|| if ours { servers.theirs(number) } else { number })
        })
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
            }
            return Ok(None);
        }
        if header.object == DISPLAY && header.opcode == 1 {
            let deleted = wire::word(body, 0).unwrap_or_default();
            if self.private.remove(&deleted) {
                return Ok(None);
            }
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
            if self.phase != Phase::Live {
                if !self.offered.contains(&global) {
                    self.offered.push(global);
                }
                return None;
            }
            let ours = self.name_for(name);
            self.names.set(ours, name);
            self.globals.push(Global {
                name: ours,
                ..global
            });
            wire::put(bytes, HEADER, ours);
        } else {
            let ours = self.names.ours(name);
            self.globals.retain(|global| global.name != ours);
            self.names.forget(ours);
            wire::put(bytes, HEADER, ours);
            if self.phase != Phase::Live {
                return None;
            }
        }
        Some(header)
    }

    /// The program's name for the global the compositor names `name`: the
    /// same, unless the program knows another global by it.
    fn name_for(&self, name: u32) -> u32 {
        let taken = |name: u32| self.globals.iter().any(|global| global.name == name);
        let mut ours = name;
        while taken(ours) {
            ours = self
                .globals
                .iter()
                .map(|global| global.name)
                .max()
                .unwrap_or(name)
                + 1;
        }
        ours
    }

    /// Takes in the event `bytes`, which `read` says it is, once read as
    /// the program's. Returns whether it is to be passed on, after what the
    /// relay is to tell the program of its own accord.
    pub fn after(&mut self, read: &Read, bytes: &[u8]) -> bool {
        let header = wire::header(bytes).ok().flatten();
        let object = header.map(|header| header.object).unwrap_or_default();
        let body = bytes.get(HEADER..).unwrap_or_default();
        let word = |at| wire::word(body, at).unwrap_or_default();
        let catching = self.phase != Phase::Live;
        match (read.interface.name, read.opcode) {
            ("wl_pointer", 0) => self.focused(object, word(4), Some(word(0)), 1, read),
            ("wl_keyboard", 1) => self.focused(object, word(4), Some(word(0)), 2, read),
            ("zwp_text_input_v3", 0) => self.focused(object, word(0), None, 1, read),
            ("wl_pointer" | "zwp_text_input_v3", 1) | ("wl_keyboard", 2) => {
                self.focus.remove(&object);
            }
            ("xdg_toplevel", 0) => {
                let window = self.toplevels.get(&object).copied().unwrap_or_default();
                let window = self.windows.entry(window).or_default();
                if catching {
                    window.held = Some((bytes.to_vec(), Vec::new()));
                    return false;
                }
                window.told = Some(body.to_vec());
            }
            ("xdg_surface", 0) => return self.configured(object, word(0)),
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

    /// Keeps that `surface` has `device`'s focus, as `read`'s enter says,
    /// with `serial`; `leave` is the event that says it has left.
    fn focused(&mut self, device: u32, surface: u32, serial: Option<u32>, leave: u16, read: &Read) {
        let framed = read.interface.name == "wl_pointer" && read.interface.version >= 5;
        let focus = Focus {
            surface,
            serial,
            leave,
            framed,
        };
        self.focus.insert(device, focus);
    }

    /// Takes in that the compositor has said how the window `xdg_surface`
    /// is to be, with `serial`. Returns whether it is passed on.
    fn configured(&mut self, xdg_surface: u32, serial: u32) -> bool {
        let window = self.windows.entry(xdg_surface).or_default();
        if self.phase == Phase::Live {
            window.offered.push(serial);
            return true;
        }
        // Catching up, the relay acks it and shows what the window showed;
        // the program is told only if the window is to be other than it last
        // acked, and its ack is then kept from the compositor.
        self.to_compositor
            .push(journal::message(xdg_surface, 4, &[serial]));
        self.to_compositor.extend(self.journal.shown(xdg_surface));
        let held = window.held.take();
        let other = held
            .as_ref()
            .map(|(held, _)| held.get(HEADER..).unwrap_or_default().to_vec());
        if other.is_some() && other != window.acked {
            window.told = other;
            self.to_program.extend(held);
            return true;
        }
        false
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
            let (sending, _) = self.journal.replay(&rename, &mut self.numbering)?;
            self.private.extend(self.numbering.fillers.drain(..));
            self.to_compositor.extend(sending);
            self.sync();
            self.phase = Phase::Making;
        } else {
            // The rest made again, as the last sync is answered.
            self.to_compositor.extend(self.journal.gone());
            self.phase = Phase::Live;
        }
        Ok(())
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
        let made = objects
            .each_living()
            .filter(|(number, _)| *number >= SERVERS);
        self.orphans.extend(made.map(|(number, _)| number));
        self.servers = Renumbered::default();
        self.syncs.clear();
        self.offered.clear();
        for window in self.windows.values_mut() {
            window.offered.clear();
            window.held = None;
        }
        let mut focus: Vec<(u32, Focus)> = self.focus.drain().collect();
        focus.sort_by_key(|(device, _)| *device);
        for (device, focus) in focus {
            let arguments: Vec<u32> = focus.serial.into_iter().chain([focus.surface]).collect();
            let leave = journal::message(device, focus.leave, &arguments);
            self.to_program.push(leave);
            if focus.framed {
                self.to_program.push(journal::message(device, 5, &[]));
            }
        }
        // The next compositor counts each field's commits from none.
        for commits in self.commits.values_mut() {
            commits.1 = 0;
        }
        self.phase = Phase::Waiting;
        Ok(())
    }

    /// Joins the program to a new compositor: its registries are made again
    /// first.
    pub fn join(&mut self) {
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
        ("wp_fractional_scale_v1", 0) | ("wl_surface", 2 | 3) => Some(true),
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

        /// Answers the sync `sync`, as the compositor does: done, then its
        /// number freed.
        fn answer(&mut self, sync: u32) {
            assert_eq!(self.told(&message(sync, 0, &words(&[0]))), None);
            assert_eq!(self.told(&message(1, 1, &words(&[sync]))), None);
        }
    }

    /// A registry's event that offers a global.
    fn global(name: u32, interface: &str, version: u32) -> Vec<u8> {
        let mut body = words(&[name]);
        body.extend(text(interface));
        body.extend(words(&[version]));
        message(2, 0, &body)
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
    fn a_window_the_next_compositor_would_have_otherwise_is_told_so() {
        let mut kept = Kept::registered(&[(1, "xdg_wm_base", 6), (2, "wl_compositor", 6)]);
        kept.bind(1, "xdg_wm_base", 6, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        kept.send(&message(3, 2, &words(&[6, 5]))).unwrap();
        kept.send(&message(6, 1, &words(&[7]))).unwrap();
        kept.told(&message(7, 0, &words(&[200, 100, 0]))).unwrap();
        kept.told(&message(6, 0, &words(&[1]))).unwrap();
        kept.send(&message(6, 4, &words(&[1]))).unwrap();

        kept.restarted(&[(1, "xdg_wm_base", 6), (2, "wl_compositor", 6)]);
        let other = message(7, 0, &words(&[300, 200, 0]));
        assert_eq!(kept.told(&other), None, "held");
        let configure = message(6, 0, &words(&[3]));
        assert_eq!(kept.told(&configure), Some(configure.clone()), "told");
        let told: Vec<&Vec<u8>> = kept
            .keeping
            .to_program
            .iter()
            .map(|(bytes, _)| bytes)
            .collect();
        assert_eq!(told, [&other], "with what it is to be, first");
        // Acked by the relay, the program's ack is not sent again.
        assert_eq!(kept.send(&message(6, 4, &words(&[3]))), None);
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

    #[test]
    fn what_has_the_pointer_and_keyboard_is_told_they_left_as_the_compositor_goes() {
        let mut kept = Kept::registered(&[(1, "wl_seat", 7), (2, "wl_compositor", 6)]);
        kept.bind(1, "wl_seat", 7, 3);
        kept.bind(2, "wl_compositor", 6, 4);
        kept.send(&message(4, 0, &words(&[5]))).unwrap();
        kept.send(&message(3, 0, &words(&[6]))).unwrap();
        kept.send(&message(3, 1, &words(&[7]))).unwrap();
        kept.told(&message(6, 0, &words(&[10, 5, 0, 0]))).unwrap();
        kept.told(&message(7, 1, &words(&[11, 5, 0]))).unwrap();
        assert_eq!(kept.keeping.lost(&kept.objects), Ok(()));
        let told: Vec<&Vec<u8>> = kept
            .keeping
            .to_program
            .iter()
            .map(|(bytes, _)| bytes)
            .collect();
        assert_eq!(
            told,
            [
                &message(6, 1, &words(&[10, 5])),
                &message(6, 5, &[]),
                &message(7, 2, &words(&[11, 5])),
            ],
            "the pointer left, its frame, then the keyboard"
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
    fn a_decorations_mode_told_again_is_kept_from_the_program_only_as_the_relay_catches_up() {
        let mut kept = Kept::registered(&[
            (1, "xdg_wm_base", 6),
            (2, "wl_compositor", 6),
            (3, "zxdg_decoration_manager_v1", 1),
        ]);
        kept.bind(1, "xdg_wm_base", 6, 4);
        kept.bind(2, "wl_compositor", 6, 5);
        kept.bind(3, "zxdg_decoration_manager_v1", 1, 6);
        kept.send(&message(5, 0, &words(&[7]))).unwrap();
        kept.send(&message(4, 2, &words(&[8, 7]))).unwrap();
        kept.send(&message(8, 1, &words(&[9]))).unwrap();
        kept.send(&message(6, 1, &words(&[10, 9]))).unwrap();
        let server_side = message(10, 0, &words(&[2]));
        assert!(kept.told(&server_side).is_some());
        assert!(kept.told(&server_side).is_some(), "live, with a configure");
        kept.restarted(&[
            (1, "xdg_wm_base", 6),
            (2, "wl_compositor", 6),
            (3, "zxdg_decoration_manager_v1", 1),
        ]);
        assert_eq!(kept.told(&server_side), None, "as it was");
        let client_side = message(10, 0, &words(&[1]));
        assert!(kept.told(&client_side).is_some(), "as it was not");
    }

    #[test]
    fn a_field_is_told_it_was_left_and_what_is_typed_by_its_own_count_of_commits() {
        let mut kept = Kept::registered(&[
            (1, "wl_seat", 7),
            (2, "wl_compositor", 6),
            (3, "zwp_text_input_manager_v3", 1),
        ]);
        kept.bind(1, "wl_seat", 7, 4);
        kept.bind(2, "wl_compositor", 6, 5);
        kept.bind(3, "zwp_text_input_manager_v3", 1, 6);
        kept.send(&message(5, 0, &words(&[7]))).unwrap();
        kept.send(&message(6, 1, &words(&[8, 4]))).unwrap();
        kept.told(&message(8, 0, &words(&[7]))).unwrap();
        for _ in 0..2 {
            kept.send(&message(8, 7, &[])).unwrap();
        }
        let done = |serial| message(8, 5, &words(&[serial]));
        assert_eq!(kept.told(&done(2)), Some(done(2)), "counted alike");
        kept.restarted(&[
            (1, "wl_seat", 7),
            (2, "wl_compositor", 6),
            (3, "zwp_text_input_manager_v3", 1),
        ]);
        assert_eq!(
            kept.keeping.to_program.first().map(|(bytes, _)| bytes),
            Some(&message(8, 1, &words(&[7]))),
            "left, with no serial"
        );
        kept.send(&message(8, 7, &[])).unwrap();
        assert_eq!(kept.told(&done(1)), Some(done(3)), "as the field counts");

        // Destroyed, and another made under its number, which counts anew.
        kept.send(&message(8, 0, &[])).unwrap();
        kept.told(&message(1, 1, &words(&[8]))).unwrap();
        kept.send(&message(6, 1, &words(&[8, 4]))).unwrap();
        kept.send(&message(8, 7, &[])).unwrap();
        assert_eq!(kept.told(&done(1)), Some(done(1)));
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
        let mut kept = Kept::registered(&[(1, "zwp_linux_dmabuf_v1", 3)]);
        kept.bind(1, "zwp_linux_dmabuf_v1", 3, 3);
        // What a buffer on the graphics card is to be made of.
        kept.send(&message(3, 1, &words(&[4]))).unwrap();
        assert_eq!(
            kept.keeping.lost(&kept.objects),
            Err(vec!["zwp_linux_buffer_params_v1"])
        );
    }
}
