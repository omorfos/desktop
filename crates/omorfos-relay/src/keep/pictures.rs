//! Pictures of a window kept through a change of compositor (ADR 61, step
//! 3e).
//!
//! What takes pictures of a window names it by its handle in a list of
//! windows, which a program such as the portal lets go of as soon as it has
//! made the source of the pictures; and the next compositor lists the window
//! under a handle and an identifier of its own. As the compositor goes, the
//! source, and what was made of it, is held back from being made again with
//! the rest: what the program asks of it meanwhile is kept, as the journal
//! keeps it, and not sent. Once the next compositor lists the window, the
//! relay makes a list of windows of its own on the program's connection,
//! while the program waits, finds the window in it by the identifier it is
//! listed by, makes the source of it under the program's number, and what
//! was made of it, asks again for each picture asked for, and lets its list
//! go. What takes pictures of a window that is not kept, or does not come
//! back, is told it has stopped, as when the window closes.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::{CAPTURE, GONE, Keeping, Phase, SHOT_FAILED, STOPPED, Shot};
use crate::journal;
use crate::objects::Objects;

/// The list of windows (`ext-foreign-toplevel-list`), as a registry names it.
const LIST: &[u8] = b"ext_foreign_toplevel_list_v1";
/// The event of a list that lists a window, and the requests that destroy a
/// list and a window in it.
const TOPLEVEL: u16 = 0;
const LIST_DESTROY: u16 = 1;
const HANDLE_DESTROY: u16 = 0;
/// The event of a window in a list that says its identifier.
const IDENTIFIER: u16 = 4;

/// A window pictures are taken of, kept, while the next compositor does not
/// have the source of them.
#[derive(Debug)]
pub(super) struct Pictured {
    /// The window, by its program's number and its own for its toplevel.
    window: (u32, u32),
    /// The identifier the next compositor lists it by, once it has.
    identifier: Option<Vec<u8>>,
}

/// The relay's own list of windows, on the program's connection, while it
/// is looked through.
#[derive(Debug)]
pub(super) struct Finding {
    list: u32,
    /// The relay's sync whose answer says the list is whole, and then that
    /// what was found is made again and the list let go of.
    sync: u32,
    /// The windows in it, by the compositor's number for each, with the
    /// identifier of each.
    handles: HashMap<u32, Vec<u8>>,
    /// Whether what was found is made again, and the list let go of.
    ending: bool,
}

impl Keeping {
    /// Takes in that the program made `source` of the window it has by
    /// `handle` in a list of windows, which is known by the identifier the
    /// compositor lists it by.
    pub(super) fn noted(&mut self, source: u32, handle: u32) {
        let listing = self.listings.get(&handle);
        let identifier = listing.map(|listing| listing.identifier.clone());
        self.sources.insert(source, identifier.unwrap_or_default());
    }

    /// What pictures of the window `source` is of are taken through: the
    /// source, and what was made of it.
    fn made_of(&self, source: u32) -> HashSet<u32> {
        let made = self.of_windows.iter();
        let made = made.filter(|(_, (_, of))| *of == source);
        made.map(|(number, _)| *number).collect()
    }

    /// Holds back what pictures of windows are taken through, as the
    /// compositor goes, from being made again with the rest: no compositor
    /// has what the program holds of it until the window is back.
    pub(super) fn hold_pictures(&mut self, objects: &Objects) {
        let held = self.of_windows.keys().copied();
        self.journal.hold_back(held);
        let living = self.of_windows.keys();
        let living = living.filter(|number| objects.living(**number).is_some());
        self.orphans.extend(living);
        self.finding = None;
        self.pictured.clear();
    }

    /// Takes in what the program made of what pictures of a window are
    /// taken through, `made`, of `source`, while that is held back: it is
    /// held back with it.
    pub(super) fn held_with(&mut self, made: u32, source: u32) {
        if self.pictured.contains_key(&source) {
            self.journal.hold_back([made]);
            self.orphans.insert(made);
        }
    }

    /// Takes in which window the compositor that has gone listed by each
    /// identifier, `listed`, and which are kept, `kept`: pictures of a window
    /// that is kept are awaited from the next compositor; of the rest they
    /// stop.
    pub(super) fn await_pictures(
        &mut self,
        objects: &Objects,
        listed: &BTreeMap<String, Option<(u32, u32)>>,
        kept: &HashSet<(u32, u32)>,
    ) {
        let mut sources: Vec<(u32, Vec<u8>)> = self.sources.drain().collect();
        sources.sort_unstable();
        for (source, identifier) in sources {
            let identifier = std::str::from_utf8(&identifier).ok();
            let window = identifier.and_then(|identifier| listed.get(identifier));
            let window = window.copied().flatten();
            match window.filter(|window| kept.contains(window)) {
                Some(window) => {
                    let pictured = Pictured {
                        window,
                        identifier: None,
                    };
                    self.pictured.insert(source, pictured);
                }
                None => self.unpicture(objects, source),
            }
        }
    }

    /// Takes in that the next compositor lists `window` by `identifier`: the
    /// pictures of it awaited are to be taken again.
    pub(super) fn picture_listed(&mut self, identifier: &[u8], window: Option<(u32, u32)>) {
        for pictured in self.pictured.values_mut() {
            if Some(pictured.window) == window {
                pictured.identifier = Some(identifier.to_vec());
            }
        }
    }

    /// Takes in that the windows there are now are `present`: the pictures of
    /// one awaited that has gone, as its program has, or the window, stop.
    pub fn windows_gone(&mut self, objects: &Objects, present: &HashSet<(u32, u32)>) {
        let gone = self.pictured.iter();
        let gone = gone.filter(|(_, pictured)| {
            pictured.identifier.is_none() && !present.contains(&pictured.window)
        });
        let gone: Vec<u32> = gone.map(|(source, _)| *source).collect();
        for source in gone {
            self.unpicture(objects, source);
        }
    }

    /// Whether pictures of a window are awaited.
    #[must_use]
    pub fn awaits_pictures(&self) -> bool {
        !self.pictured.is_empty()
    }

    /// Tells what takes pictures of the window `source` is of that it has
    /// stopped, and a picture of it asked for that it failed, as when the
    /// window closes; none of it is made again, and the relay answers for
    /// it.
    fn unpicture(&mut self, objects: &Objects, source: u32) {
        let made = self.made_of(source);
        let mut living: Vec<u32> = made.iter().copied().collect();
        living.retain(|number| objects.living(*number).is_some());
        living.sort_unstable();
        for number in living {
            let session = self.of_windows.get(&number);
            if session
                .is_some_and(|(interface, _)| *interface == "ext_image_copy_capture_session_v1")
            {
                self.to_program.push(journal::message(number, STOPPED, &[]));
            }
            if self.shots.get(&number) == Some(&Shot::Asked) {
                self.shots.remove(&number);
                self.to_program
                    .push(journal::message(number, SHOT_FAILED, &[GONE]));
            }
        }
        self.of_windows.retain(|number, _| !made.contains(number));
        self.journal.forget(&made);
        self.pictured.remove(&source);
        self.sources.remove(&source);
    }

    /// Makes the relay's own list of windows on the program's connection,
    /// while the program waits, if the next compositor has listed a window
    /// pictures are taken of again and the relay is caught up. Its number is
    /// past `highest`, the highest the program has taken, as the
    /// compositor takes only the next number or one freed.
    pub fn find(&mut self, objects: &Objects) {
        let listed = self.pictured.values();
        let listed = listed.filter(|pictured| pictured.identifier.is_some());
        if self.phase != Phase::Live || self.finding.is_some() || listed.count() == 0 {
            return;
        }
        let global = self.globals.iter().find(|global| global.interface == LIST);
        let global = global.map(|global| (self.names.theirs(global.name), global.version));
        let mut registries = objects.each_living();
        let registry = registries.find(|(_, interface)| interface.name == "wl_registry");
        let Some(((name, version), (registry, _))) = global.zip(registry) else {
            let sources: Vec<u32> = self.pictured.keys().copied().collect();
            for source in sources {
                self.unpicture(objects, source);
            }
            return;
        };
        let list = self.numbering.top.max(objects.highest()) + 1;
        self.numbering.top = list;
        self.private.insert(list);
        let bind = journal::said_between(registry, 0, (&[name], LIST, &[version.min(1), list]));
        self.to_compositor.push(bind);
        let sync = self.numbering.take_next();
        self.private.insert(sync);
        self.to_compositor.push(journal::message(1, 0, &[sync]));
        self.finding = Some(Finding {
            list,
            sync,
            handles: HashMap::new(),
            ending: false,
        });
        self.phase = Phase::Picturing;
    }

    /// Takes in the event `opcode`, with `body`, to the relay's own `object`,
    /// as far as it is of its list of windows: a window listed, its
    /// identifier, or the relay's sync answered. Returns whether it was.
    pub(super) fn finding(
        &mut self,
        objects: &Objects,
        object: u32,
        opcode: u16,
        body: &[u8],
    ) -> bool {
        let Some(mut finding) = self.finding.take() else {
            return false;
        };
        if object == finding.sync {
            self.found(objects, finding);
            return true;
        }
        let said = finding.handles.get_mut(&object);
        match (object == finding.list && opcode == TOPLEVEL, said) {
            (true, _) => {
                let handle = crate::wire::word(body, 0).unwrap_or_default();
                finding.handles.insert(handle, Vec::new());
                self.private.insert(handle);
            }
            (false, Some(said)) if opcode == IDENTIFIER => {
                crate::wire::text(body, 0).clone_into(said);
            }
            _ => {
                self.finding = Some(finding);
                return false;
            }
        }
        self.finding = Some(finding);
        true
    }

    /// Takes in that the relay's sync of `finding` is answered: with its
    /// list whole, each window found in it has what pictures of it are taken
    /// through made again, and the list is let go of; with that done, the
    /// program goes on.
    fn found(&mut self, objects: &Objects, finding: Finding) {
        if finding.ending {
            self.private
                .retain(|number| !finding.handles.contains_key(number));
            self.phase = Phase::Live;
            self.find(objects);
            return;
        }
        let listed = self.pictured.iter();
        let listed = listed.filter_map(|(source, pictured)| {
            let identifier = pictured.identifier.clone();
            identifier.map(|identifier| (*source, identifier))
        });
        let listed: Vec<(u32, Vec<u8>)> = listed.collect();
        for (source, identifier) in listed {
            let handles = finding.handles.iter();
            let mut handles = handles.filter(|(_, said)| **said == identifier);
            match handles.next() {
                Some((handle, _)) => self.remake_pictures(source, *handle, identifier),
                // Listed, and gone again since.
                None => self.unpicture(objects, source),
            }
        }
        let mut handles: Vec<u32> = finding.handles.keys().copied().collect();
        handles.sort_unstable();
        for handle in handles {
            self.to_compositor
                .push(journal::message(handle, HANDLE_DESTROY, &[]));
        }
        self.to_compositor
            .push(journal::message(finding.list, LIST_DESTROY, &[]));
        let sync = self.numbering.take_next();
        self.private.insert(sync);
        self.to_compositor.push(journal::message(1, 0, &[sync]));
        self.finding = Some(Finding {
            sync,
            ending: true,
            ..finding
        });
    }

    /// Makes again what pictures of the window `source` is of are taken
    /// through, the source made of it by `handle`, the next compositor's,
    /// which lists it by `identifier`; and asks again for each picture asked
    /// for meanwhile.
    fn remake_pictures(&mut self, source: u32, handle: u32, identifier: Vec<u8>) {
        let made = self.made_of(source);
        let sending = self.journal.remake(&made, (source, handle));
        self.to_compositor.extend(sending);
        let mut asked: Vec<u32> = made.iter().copied().collect();
        asked.retain(|number| self.shots.get(number) == Some(&Shot::Asked));
        asked.sort_unstable();
        for frame in asked {
            self.to_compositor
                .push(journal::message(frame, CAPTURE, &[]));
        }
        self.orphans.retain(|number| !made.contains(number));
        self.pictured.remove(&source);
        self.sources.insert(source, identifier);
    }
}
