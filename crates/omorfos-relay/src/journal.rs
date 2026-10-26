//! What a program has made, written down as it is made, so that it can be
//! made again for a compositor started anew (ADR 61, step 2).
//!
//! For each object, the request that made it is kept, with its descriptors
//! held open, and what has been asked of it since that lasts, as [`rule`]
//! says for each request: the last of a request that sets something, every
//! one of one that adds to it, and, for what applies on a commit, what is
//! pending and what the last commit applied. A message's description says
//! what each argument is but not what it means, so these rules are written
//! out for each request.
//!
//! An object destroyed while something kept still names it, a buffer a
//! surface last showed or the pool that buffer came from, is kept as gone,
//! made again with the rest, and destroyed again once the rest has been.
//! Its number is not given back to the program, which the compositor says
//! by `wl_display.delete_id`, until nothing kept names it any more, so that
//! the program cannot take it for something else meanwhile.
//!
//! A surface's parts (`wl_subsurface`) are stacked as they last were, and
//! each surface is committed after its parts, so that what a part shows,
//! which it shows only once the surface it is part of is committed, is
//! there by then.

use std::collections::{HashMap, HashSet};
use std::os::fd::OwnedFd;
use std::rc::Rc;

use wayland_backend::protocol::{Interface, MessageDesc};

use crate::objects::Read;
use crate::wire::{self, Broken, HEADER, Header};

/// A message to be sent, and its descriptors, each held as long as anything
/// that will send it holds it, the record among them.
pub type Sending = (Vec<u8>, Vec<Rc<OwnedFd>>);

/// The display, which every connection starts with.
const DISPLAY: u32 = 1;

/// Which of the display's events says a number may be taken again.
const DELETE_ID: u16 = 1;

/// The numbers of the surface's requests the relay sends of its own
/// accord, and of the one that gives it a buffer, which a window may not
/// have until the compositor has said how it is to be.
const ATTACH: u16 = 1;
const DAMAGE: u16 = 2;
const COMMIT: u16 = 6;
const DAMAGE_BUFFER: u16 = 9;

/// The numbers of a part's requests that put it over, and under, another.
const PLACE_ABOVE: u16 = 2;
const PLACE_BELOW: u16 = 3;

/// What a request does to what is kept of the object it is to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// It sets something that lasts: the last of it is kept.
    Lasting,
    /// It adds to what the object is: each is kept, in order.
    Adding,
    /// It sets something that applies on the surface's next commit.
    Pending,
    /// It commits what is pending.
    Commit,
    /// Nothing of it lasts, or what does is the compositor's to say again.
    Passing,
}

/// What each request does to what is kept, by the name of its interface and
/// its own.
#[must_use]
pub fn rule(interface: &str, request: &str) -> Rule {
    match (interface, request) {
        (
            "wl_surface",
            "attach"
            | "set_opaque_region"
            | "set_input_region"
            | "set_buffer_transform"
            | "set_buffer_scale"
            | "offset",
        ) => Rule::Pending,
        ("wl_surface", "commit") => Rule::Commit,
        ("wl_region", "add" | "subtract")
        | ("wl_data_source" | "zwp_primary_selection_source_v1", "offer") => Rule::Adding,
        ("wl_shm_pool", "resize")
        | ("wl_data_source", "set_actions")
        | ("xdg_surface", "set_window_geometry")
        | (
            "xdg_toplevel",
            "set_parent" | "set_title" | "set_app_id" | "set_max_size" | "set_min_size",
        )
        | ("xdg_positioner", _)
        | ("zxdg_toplevel_decoration_v1", "set_mode" | "unset_mode")
        | ("wp_viewport", "set_source" | "set_destination")
        | ("wl_subsurface", "set_position" | "set_sync" | "set_desync") => Rule::Lasting,
        _ => Rule::Passing,
    }
}

/// The interfaces of the objects, other than the globals bound, that can be
/// made again: a program holding one of another cannot be kept.
const KEPT: [&str; 23] = [
    "wl_display",
    "wl_registry",
    "wl_callback",
    "wl_surface",
    "wl_subsurface",
    "wl_region",
    "wl_shm_pool",
    "wl_buffer",
    "wl_pointer",
    "wl_keyboard",
    "wp_cursor_shape_device_v1",
    "wl_data_device",
    "wl_data_source",
    "zwp_primary_selection_device_v1",
    "zwp_primary_selection_source_v1",
    "xdg_surface",
    "xdg_toplevel",
    "xdg_positioner",
    "zxdg_toplevel_decoration_v1",
    "wp_viewport",
    "wp_fractional_scale_v1",
    "zxdg_output_v1",
    "zwp_text_input_v3",
];

/// Whether an object of `interface` can be made again.
#[must_use]
pub fn keeps(interface: &str) -> bool {
    KEPT.contains(&interface) || crate::known::global(interface).is_some()
}

/// A request kept, its descriptors held open.
#[derive(Debug)]
struct Message {
    /// Its bytes, header and all.
    bytes: Vec<u8>,
    /// Its descriptors.
    fds: Vec<Rc<OwnedFd>>,
    /// The objects it names, the one it is to among them, but not one it
    /// makes.
    names: Vec<u32>,
}

impl Message {
    /// `bytes`, a request `described`, kept with copies of `fds`.
    fn kept(bytes: &[u8], fds: &[Rc<OwnedFd>], described: &MessageDesc) -> Self {
        let fds = fds.to_vec();
        let mut names: Vec<u32> = wire::word(bytes, 0).into_iter().collect();
        names.extend(wire::named(
            bytes.get(HEADER..).unwrap_or_default(),
            described,
        ));
        Self {
            bytes: bytes.to_vec(),
            fds,
            names,
        }
    }

    /// The message, to be sent, its descriptors held with it.
    fn sending(&self) -> Sending {
        (self.bytes.clone(), self.fds.clone())
    }

    /// Which of the object's messages it is.
    fn opcode(&self) -> u16 {
        let sized = wire::word(&self.bytes, 4).unwrap_or_default();
        u16::try_from(sized & 0xffff).unwrap_or(u16::MAX)
    }
}

/// A message to `object`, the `opcode`th of its, of `arguments`.
#[must_use]
pub fn message(object: u32, opcode: u16, arguments: &[u32]) -> Sending {
    let size = u32::try_from(HEADER + 4 * arguments.len()).unwrap_or(u32::MAX);
    let mut bytes = object.to_ne_bytes().to_vec();
    bytes.extend((size << 16 | u32::from(opcode)).to_ne_bytes());
    bytes.extend(arguments.iter().flat_map(|argument| argument.to_ne_bytes()));
    (bytes, Vec::new())
}

/// The numbers a compositor has taken for the program's objects, as they are
/// made again.
///
/// A compositor takes a number for an object only if it is the next, past
/// every number it has taken, or one it has freed. A program's numbers have
/// gaps where it destroyed something; before an object is made again under
/// its number, each below it not yet taken is taken by a sync of the
/// relay's own, which the compositor answers and frees, and whose answer is
/// kept from the program.
#[derive(Debug)]
pub struct Numbering {
    /// The highest number taken.
    pub top: u32,
    /// The numbers taken by the relay's syncs, whose answers are to be kept
    /// from the program.
    pub fillers: Vec<u32>,
}

impl Default for Numbering {
    fn default() -> Self {
        Self {
            top: DISPLAY,
            fillers: Vec::new(),
        }
    }
}

impl Numbering {
    /// Takes, with syncs, each number below `made` not yet taken.
    fn fill(&mut self, sending: &mut Vec<Sending>, made: u32) {
        for filler in self.top.saturating_add(1)..made {
            sending.push(message(DISPLAY, 0, &[filler]));
            self.fillers.push(filler);
        }
        self.top = self.top.max(made);
    }

    /// The next number not taken, taken for a sync of the relay's own.
    pub fn take_next(&mut self) -> u32 {
        self.top += 1;
        self.top
    }
}

/// What is kept of an object.
#[derive(Debug)]
struct Kept {
    /// When it was made, among the program's objects.
    order: u64,
    interface: &'static Interface,
    version: u32,
    /// The request that made it, and whether that bound a global.
    made: Message,
    bound: bool,
    /// The request that destroyed it, if it is gone but still named.
    gone: Option<Message>,
    /// The last of each request that sets something lasting.
    lasting: Vec<Message>,
    /// Each request that added to it, in order.
    adding: Vec<Message>,
    /// What applies on its next commit, and what its last commit applied.
    pending: Vec<Message>,
    committed: Vec<Message>,
    /// How many messages kept, of other objects, name it.
    named: usize,
}

impl Kept {
    /// The names in every message kept of it.
    fn names(&self) -> Vec<Vec<u32>> {
        let state = self.lasting.iter().chain(&self.adding);
        let state = state.chain(&self.pending).chain(&self.committed);
        let all = [&self.made]
            .into_iter()
            .chain(self.gone.as_ref())
            .chain(state);
        all.map(|message| message.names.clone()).collect()
    }
}

/// What a program has made.
#[derive(Debug, Default)]
pub struct Journal {
    kept: HashMap<u32, Kept>,
    made: u64,
    /// Numbers the compositor has said may be taken again, which the
    /// program has not been told, as something kept still names them.
    withheld: HashSet<u32>,
    /// Numbers the program is now to be told may be taken again.
    freed: Vec<u32>,
    /// How the parts of each surface that has had any are stacked, from the
    /// bottom, the surface itself among them. A part destroyed may still be
    /// in it; it is passed over.
    stacks: HashMap<u32, Vec<u32>>,
}

impl Journal {
    /// Keeps what the request `bytes` to `header`'s object, `described`,
    /// which came with `fds` and which `read` says made or did, does that
    /// lasts.
    pub fn request(
        &mut self,
        header: Header,
        bytes: &[u8],
        fds: &[Rc<OwnedFd>],
        read: &Read,
        described: &MessageDesc,
    ) {
        if let Some((made, interface, version)) = read.made {
            let message = Message::kept(bytes, fds, described);
            self.stacked(made, interface.name, &message.names);
            self.name(made, &message.names);
            self.made += 1;
            let kept = Kept {
                order: self.made,
                interface,
                version,
                made: message,
                bound: read.interface.name == "wl_registry",
                gone: None,
                lasting: Vec::new(),
                adding: Vec::new(),
                pending: Vec::new(),
                committed: Vec::new(),
                named: 0,
            };
            self.kept.insert(made, kept);
            return;
        }
        let object = header.object;
        let rule = rule(read.interface.name, described.name);
        if read.interface.name == "wl_subsurface"
            && matches!(read.opcode, PLACE_ABOVE | PLACE_BELOW)
        {
            let sibling = wire::word(bytes, HEADER).unwrap_or_default();
            self.place(object, sibling, read.opcode == PLACE_ABOVE);
        }
        if read.ends {
            let message = Message::kept(bytes, fds, described);
            self.ended(object, message);
        } else if rule != Rule::Passing {
            let message = Message::kept(bytes, fds, described);
            self.keep(object, rule, message);
        }
    }

    /// Takes in that `made`, of `interface`, was made by a request naming
    /// `names`: a surface is in no stack yet, and a part is put on top of
    /// the stack of the surface it is part of.
    fn stacked(&mut self, made: u32, interface: &str, names: &[u32]) {
        let part = (interface == "wl_subsurface")
            .then(|| names.get(1).zip(names.get(2)))
            .flatten();
        if interface == "wl_surface" {
            self.stacks.remove(&made);
        }
        let surface = part.map_or(made, |(surface, _)| *surface);
        if interface == "wl_surface" || part.is_some() {
            for stack in self.stacks.values_mut() {
                stack.retain(|stacked| *stacked != surface);
            }
        }
        if let Some((surface, parent)) = part {
            let stack = self.stacks.entry(*parent).or_insert_with(|| vec![*parent]);
            stack.push(*surface);
        }
    }

    /// Puts the surface of the part `part` over `sibling`, or under it.
    fn place(&mut self, part: u32, sibling: u32, above: bool) {
        let names = self.kept.get(&part).map(|kept| &kept.made.names);
        let found = names.and_then(|names| names.get(1).zip(names.get(2)));
        let found = found.and_then(|(surface, parent)| {
            let surface = *surface;
            self.stacks.get_mut(parent).map(|stack| (surface, stack))
        });
        let placed = found.map(|(surface, stack)| {
            stack.retain(|stacked| *stacked != surface);
            // Put by what is not its sibling, which the compositor ends the
            // program for, it is put on top.
            let at = stack.iter().position(|stacked| *stacked == sibling);
            let at = at.map_or(stack.len(), |at| at + usize::from(above));
            stack.insert(at, surface);
        });
        let _ = placed;
    }

    /// Keeps `message`, to `object`, as `rule` says.
    fn keep(&mut self, object: u32, rule: Rule, message: Message) {
        let names = message.names.clone();
        // A request whose rule keeps something is to an object kept.
        let kept = self.kept.get_mut(&object);
        let dropped: Vec<Message> = kept
            .map(|kept| match rule {
                Rule::Lasting => replace(&mut kept.lasting, message).into_iter().collect(),
                Rule::Adding => {
                    kept.adding.push(message);
                    Vec::new()
                }
                Rule::Pending => replace(&mut kept.pending, message).into_iter().collect(),
                Rule::Commit | Rule::Passing => {
                    let mut dropped = vec![message];
                    for pending in std::mem::take(&mut kept.pending) {
                        dropped.extend(replace(&mut kept.committed, pending));
                    }
                    dropped
                }
            })
            .unwrap_or_default();
        self.name(object, &names);
        for message in dropped {
            self.unname(object, &message.names);
        }
    }

    /// Takes in that `object` was destroyed by `message`: kept as gone if
    /// something kept names it, let go of if not.
    fn ended(&mut self, object: u32, message: Message) {
        let names = message.names.clone();
        let Some(kept) = self.kept.get_mut(&object) else {
            return;
        };
        kept.gone = Some(message);
        self.name(object, &names);
        self.end(object);
    }

    /// Takes in what `read`, an event to `header`'s object with arguments
    /// `body`, says of the program's objects. Returns whether it is to be
    /// passed on: not that a number may be taken again while something kept
    /// still names it.
    pub fn event(&mut self, header: Header, body: &[u8], read: &Read) -> bool {
        if header.object == DISPLAY && header.opcode == DELETE_ID {
            let deleted = wire::word(body, 0).unwrap_or_default();
            if self.kept.contains_key(&deleted) {
                self.withheld.insert(deleted);
                return false;
            }
        }
        if read.ends {
            // Destroyed by the compositor, it is not destroyed again.
            let kept = self.kept.entry(header.object);
            kept.and_modify(|kept| kept.gone = None);
            self.end(header.object);
        }
        true
    }

    /// The numbers the program is now to be told may be taken again, as
    /// nothing kept names them any more.
    pub fn freed(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.freed)
    }

    /// What, of what the program has made, cannot be made again: each
    /// interface once, in the order first made.
    #[must_use]
    pub fn unkept(&self) -> Vec<&'static str> {
        let mut unkept = Vec::new();
        for (_, kept) in self.ordered() {
            let interface = kept.interface.name;
            if !keeps(interface) && !unkept.contains(&interface) {
                unkept.push(interface);
            }
        }
        unkept
    }

    /// The requests that make the program's registries again, in the order
    /// they were made.
    pub fn registries(&self, numbering: &mut Numbering) -> Vec<Sending> {
        let mut sending = Vec::new();
        let ordered = self.ordered().into_iter();
        let registries = ordered.filter(|(_, kept)| kept.interface.name == "wl_registry");
        for (number, kept) in registries {
            numbering.fill(&mut sending, number);
            sending.push(kept.made.sending());
        }
        sending
    }

    /// The requests that make the rest again, in the order it was made,
    /// with what lasts of each; then each surface committed as it last was,
    /// but for a window's, which may not show anything until the compositor
    /// has said how it is to be: those are committed bare, and the windows
    /// listed, by their `xdg_surface`, for [`Journal::shown`]. A global is
    /// bound again under the name `rename` gives the one it was bound by.
    ///
    /// # Errors
    ///
    /// Returns an error if a global bound is no longer offered.
    pub fn replay(
        &self,
        rename: &dyn Fn(u32) -> Option<u32>,
        numbering: &mut Numbering,
    ) -> Result<(Vec<Sending>, Vec<u32>), Broken> {
        let mut sending = Vec::new();
        // What is asked of an `xdg_surface`, where its window's edges are,
        // may be asked only once it has a role, which is made after it.
        let mut roled = Vec::new();
        let ordered = self.ordered();
        for (number, kept) in &ordered {
            if kept.interface.name == "wl_registry" {
                continue;
            }
            numbering.fill(&mut sending, *number);
            let (mut bytes, fds) = kept.made.sending();
            if kept.bound {
                let name = wire::word(&bytes, HEADER).unwrap_or_default();
                let name = rename(name).ok_or(Broken("a global that is no longer offered"))?;
                wire::put(&mut bytes, HEADER, name);
            }
            sending.push((bytes, fds));
            let lasting = kept
                .lasting
                .iter()
                .chain(&kept.adding)
                .map(Message::sending);
            if kept.interface.name == "xdg_surface" {
                roled.extend(lasting);
            } else {
                sending.extend(lasting);
            }
        }
        sending.extend(roled);
        let parts = self.parts();
        sending.extend(self.stacking(&parts));
        let windows = self.windows();
        let mut awaited = Vec::new();
        let mut surfaces: Vec<&(u32, &Kept)> = ordered
            .iter()
            .filter(|(_, kept)| kept.interface.name == "wl_surface" && kept.gone.is_none())
            .collect();
        // Parts first, the deepest first, so that each is committed before
        // what it is part of.
        let depth = |mut surface: u32| {
            let mut depth = 0;
            while let Some((_, parent)) = parts.get(&surface)
                && depth < parts.len()
            {
                depth += 1;
                surface = *parent;
            }
            depth
        };
        surfaces.sort_by_key(|(number, _)| std::cmp::Reverse(depth(*number)));
        for (number, kept) in surfaces {
            match windows.get(number) {
                Some(window) => {
                    let bare = kept
                        .committed
                        .iter()
                        .filter(|message| message.opcode() != ATTACH);
                    for message in bare {
                        sending.push(message.sending());
                    }
                    sending.push(message(*number, COMMIT, &[]));
                    awaited.push(*window);
                }
                None => sending.extend(surface(*number, kept)),
            }
        }
        Ok((sending, awaited))
    }

    /// The requests that show what the window `xdg_surface` last showed,
    /// once the compositor has said how it is to be and been answered.
    #[must_use]
    pub fn shown(&self, xdg_surface: u32) -> Vec<Sending> {
        let number = self
            .kept
            .get(&xdg_surface)
            .and_then(|kept| kept.made.names.get(1));
        let kept = number.and_then(|number| self.kept.get(number).map(|kept| (*number, kept)));
        kept.map_or(Vec::new(), |(number, kept)| surface(number, kept))
    }

    /// The requests that destroy again what was made again only as
    /// something else named it, last made first.
    #[must_use]
    pub fn gone(&self) -> Vec<Sending> {
        let mut gone: Vec<&Kept> = self
            .kept
            .values()
            .filter(|kept| kept.gone.is_some())
            .collect();
        gone.sort_by_key(|kept| std::cmp::Reverse(kept.order));
        let gone = gone.into_iter().filter_map(|kept| kept.gone.as_ref());
        gone.map(Message::sending).collect()
    }

    /// Each surface that is a living part, with the part and the surface it
    /// is part of.
    fn parts(&self) -> HashMap<u32, (u32, u32)> {
        let parts = self
            .kept
            .iter()
            .filter(|(_, kept)| kept.interface.name == "wl_subsurface" && kept.gone.is_none());
        let parts = parts.filter_map(|(number, kept)| {
            let names = &kept.made.names;
            let found = names.get(1).zip(names.get(2));
            found.map(|(surface, parent)| (*surface, (*number, *parent)))
        });
        parts.collect()
    }

    /// The requests that stack the living `parts` of each surface as they
    /// were: outwards from the surface, each over the one below it, and each
    /// under the one above it.
    fn stacking(&self, parts: &HashMap<u32, (u32, u32)>) -> Vec<Sending> {
        let mut sending = Vec::new();
        let mut stacks: Vec<(&u32, &Vec<u32>)> = self.stacks.iter().collect();
        stacks.sort_by_key(|(parent, _)| **parent);
        for (parent, stack) in stacks {
            let (mut below, mut above) = (Vec::new(), Vec::new());
            for surface in stack {
                if surface == parent {
                    std::mem::swap(&mut below, &mut above);
                } else if let Some((part, _)) = parts.get(surface) {
                    above.push((*part, *surface));
                }
            }
            // Before the surface itself was met, its parts went into what
            // is now `below`.
            let mut previous = *parent;
            for (part, surface) in above {
                sending.push(message(part, PLACE_ABOVE, &[previous]));
                previous = surface;
            }
            previous = *parent;
            for (part, surface) in below.into_iter().rev() {
                sending.push(message(part, PLACE_BELOW, &[previous]));
                previous = surface;
            }
        }
        sending
    }

    /// Each surface that is a living window's, and the window's
    /// `xdg_surface`.
    fn windows(&self) -> HashMap<u32, u32> {
        let windows = self
            .kept
            .iter()
            .filter(|(_, kept)| kept.interface.name == "xdg_surface" && kept.gone.is_none());
        let windows = windows
            .filter_map(|(number, kept)| kept.made.names.get(1).map(|surface| (*surface, *number)));
        windows.collect()
    }

    /// What is kept, in the order it was made.
    fn ordered(&self) -> Vec<(u32, &Kept)> {
        let mut ordered: Vec<(u32, &Kept)> = self
            .kept
            .iter()
            .map(|(number, kept)| (*number, kept))
            .collect();
        ordered.sort_by_key(|(_, kept)| kept.order);
        ordered
    }

    /// Counts each object in `names` but `owner` as named once more.
    fn name(&mut self, owner: u32, names: &[u32]) {
        for named in names.iter().filter(|named| **named != owner) {
            if let Some(kept) = self.kept.get_mut(named) {
                kept.named += 1;
            }
        }
    }

    /// Counts each object in `names` but `owner` as named once less, and lets
    /// go of one gone that nothing names any more.
    fn unname(&mut self, owner: u32, names: &[u32]) {
        for named in names.iter().filter(|named| **named != owner) {
            let unnamed = self.kept.get_mut(named).map(|kept| {
                kept.named = kept.named.saturating_sub(1);
                kept.named == 0 && kept.gone.is_some()
            });
            if unnamed == Some(true) {
                self.end(*named);
            }
        }
    }

    /// Lets go of `object`, destroyed, unless something kept still names it.
    fn end(&mut self, object: u32) {
        let named = self.kept.get(&object).map(|kept| kept.named);
        if named != Some(0) {
            return;
        }
        let kept = self.kept.remove(&object);
        for names in kept.iter().flat_map(Kept::names) {
            self.unname(object, &names);
        }
        if self.withheld.remove(&object) {
            self.freed.push(object);
        }
    }
}

/// The surface `number`, committed as it last was, then what is pending of
/// it.
fn surface(number: u32, kept: &Kept) -> Vec<Sending> {
    let mut sending = Vec::new();
    for message in &kept.committed {
        sending.push(message.sending());
    }
    // What was shown is shown whole: the new compositor has none of it.
    let whole = [0, 0, i32::MAX.unsigned_abs(), i32::MAX.unsigned_abs()];
    let damage = if kept.version >= 4 {
        DAMAGE_BUFFER
    } else {
        DAMAGE
    };
    sending.push(message(number, damage, &whole));
    sending.push(message(number, COMMIT, &[]));
    for message in &kept.pending {
        sending.push(message.sending());
    }
    sending
}

/// Puts `message` last in `kept`, in the place of the one of the same
/// request, which it returns: so that requests that undo each other, as one
/// that sets something and one that unsets it, are made again in the order
/// they were last made.
fn replace(kept: &mut Vec<Message>, message: Message) -> Option<Message> {
    let opcode = message.opcode();
    let same = kept.iter().position(|kept| kept.opcode() == opcode);
    let replaced = same.map(|at| kept.remove(at));
    kept.push(message);
    replaced
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;
    use crate::objects::Objects;
    use crate::wire::tests::{message as sent, text, words};

    /// A program, as the relay sees it.
    #[derive(Default)]
    struct Program {
        objects: Objects,
        journal: Journal,
    }

    impl Program {
        /// Sends the request `bytes`, with `fds`.
        fn send(&mut self, bytes: &[u8], fds: &[Rc<OwnedFd>]) {
            let header = wire::header(bytes).unwrap().unwrap();
            let read = self.objects.request(bytes, header).unwrap();
            let described = &read.interface.requests[usize::from(read.opcode)];
            self.journal.request(header, bytes, fds, &read, described);
        }

        /// Is sent the event `bytes`; says whether it is passed on.
        fn told(&mut self, bytes: &[u8]) -> bool {
            let header = wire::header(bytes).unwrap().unwrap();
            let body = &bytes[HEADER..];
            let passed = self.journal.event(header, body, &self.read(bytes, header));
            if passed {
                self.objects.event(bytes, header).unwrap();
            }
            passed
        }

        /// What the event `bytes` is, without taking it in.
        fn read(&self, bytes: &[u8], header: Header) -> Read {
            let interface = self
                .objects
                .living(header.object)
                .unwrap_or(crate::known::display());
            let described = &interface.events[usize::from(header.opcode)];
            Read {
                fds: 0,
                interface,
                opcode: header.opcode,
                made: None,
                ends: described.is_destructor && !bytes.is_empty(),
            }
        }

        /// Gets a registry, as 2, and binds `name` to `interface`, as `as`.
        fn bind(&mut self, name: u32, interface: &str, version: u32, number: u32) {
            if self.objects.living(2).is_none() {
                self.send(&sent(1, 1, &words(&[2])), &[]);
            }
            let mut body = words(&[name]);
            body.extend(text(interface));
            body.extend(words(&[version, number]));
            self.send(&sent(2, 0, &body), &[]);
        }
    }

    /// What the record sends a new compositor, past its registries.
    fn replayed(
        journal: &Journal,
        rename: &dyn Fn(u32) -> Option<u32>,
    ) -> Result<(Vec<Sending>, Vec<u32>), Broken> {
        let mut numbering = Numbering::default();
        journal.registries(&mut numbering);
        journal.replay(rename, &mut numbering)
    }

    /// The opcodes of what is sent, each with the object it is to.
    fn opcodes(sending: &[Sending]) -> Vec<(u32, u32)> {
        let header = |bytes: &[u8]| {
            let header = wire::header(bytes).unwrap().unwrap();
            (header.object, u32::from(header.opcode))
        };
        sending.iter().map(|(bytes, _)| header(bytes)).collect()
    }

    /// A program with a compositor bound as 3 and a surface as 4.
    fn surfaced() -> Program {
        let mut program = Program::default();
        program.bind(1, "wl_compositor", 6, 3);
        program.send(&sent(3, 0, &words(&[4])), &[]);
        program
    }

    /// Binds `wl_shm` as 50, with a pool of it as `pool`.
    fn pooled(program: &mut Program, pool: u32) {
        let memory = Rc::new(OwnedFd::from(File::open("/dev/null").unwrap()));
        if program.objects.living(50).is_none() {
            program.bind(9, "wl_shm", 1, 50);
        }
        program.send(&sent(50, 0, &words(&[pool, 4096])), &[memory]);
    }

    /// Makes `buffer` from `pool`.
    fn buffered(program: &mut Program, pool: u32, buffer: u32) {
        program.send(&sent(pool, 0, &words(&[buffer, 0, 10, 10, 40, 0])), &[]);
    }

    #[test]
    fn what_is_made_is_made_again_in_order_with_globals_bound_by_their_new_names() {
        let mut program = surfaced();
        // A region, added to twice.
        program.send(&sent(3, 1, &words(&[5])), &[]);
        program.send(&sent(5, 1, &words(&[0, 0, 10, 10])), &[]);
        program.send(&sent(5, 1, &words(&[20, 0, 10, 10])), &[]);
        let registries = program.journal.registries(&mut Numbering::default());
        assert_eq!(opcodes(&registries), [(1, 1)]);
        let (sending, awaited) = replayed(&program.journal, &|name| Some(name + 10)).unwrap();
        assert_eq!(
            opcodes(&sending),
            [(2, 0), (3, 0), (3, 1), (5, 1), (5, 1), (4, 9), (4, 6)],
            "bound, a surface, a region added to twice, the surface committed"
        );
        assert_eq!(wire::word(&sending[0].0, HEADER), Some(11), "the new name");
        assert_eq!(awaited, [0_u32; 0]);
    }

    #[test]
    fn a_global_no_longer_offered_cannot_be_bound_again() {
        let program = surfaced();
        assert!(replayed(&program.journal, &|_| None).is_err());
    }

    #[test]
    fn a_surface_is_committed_as_it_last_was_and_left_with_what_is_pending() {
        let mut program = surfaced();
        pooled(&mut program, 6);
        buffered(&mut program, 6, 7);
        buffered(&mut program, 6, 8);
        // Attached 7 and scaled, committed; then 8 attached, not committed.
        program.send(&sent(4, 1, &words(&[7, 0, 0])), &[]);
        program.send(&sent(4, 8, &words(&[2])), &[]);
        program.send(&sent(4, 6, &[]), &[]);
        program.send(&sent(4, 1, &words(&[8, 0, 0])), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let surface: Vec<(u32, u32)> = opcodes(&sending)
            .into_iter()
            .filter(|(object, _)| *object == 4)
            .collect();
        assert_eq!(surface, [(4, 1), (4, 8), (4, 9), (4, 6), (4, 1)]);
        let attached: Vec<u32> = sending
            .iter()
            .filter(|(bytes, _)| {
                wire::header(bytes).unwrap().unwrap()
                    == Header {
                        object: 4,
                        opcode: 1,
                        size: 20,
                    }
            })
            .filter_map(|(bytes, _)| wire::word(bytes, HEADER))
            .collect();
        assert_eq!(attached, [7, 8], "the one committed, then the one pending");
    }

    #[test]
    fn a_surface_too_old_to_say_damage_in_its_buffer_says_it_on_itself() {
        let mut program = Program::default();
        program.bind(1, "wl_compositor", 3, 3);
        program.send(&sent(3, 0, &words(&[4])), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        assert!(opcodes(&sending).contains(&(4, 2)));
    }

    #[test]
    fn the_descriptors_a_request_came_with_are_sent_again() {
        let mut program = surfaced();
        pooled(&mut program, 6);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let pool = sending
            .iter()
            .find(|(bytes, _)| wire::word(bytes, 0) == Some(50));
        assert_eq!(pool.map(|(_, fds)| fds.len()), Some(1));
    }

    #[test]
    fn of_what_sets_something_the_last_is_kept() {
        let mut program = Program::default();
        program.bind(1, "xdg_wm_base", 6, 3);
        program.bind(2, "wl_compositor", 6, 4);
        program.send(&sent(4, 0, &words(&[5])), &[]);
        program.send(&sent(3, 2, &words(&[6, 5])), &[]);
        program.send(&sent(6, 1, &words(&[7])), &[]);
        program.send(&sent(7, 2, &text("first")), &[]);
        program.send(&sent(7, 2, &text("last")), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let titles: Vec<&Vec<u8>> = sending
            .iter()
            .map(|(bytes, _)| bytes)
            .filter(|bytes| wire::word(bytes, 0) == Some(7))
            .collect();
        assert_eq!(titles.len(), 1, "one title");
        assert_eq!(&titles[0][HEADER + 4..HEADER + 8], b"last");
    }

    #[test]
    fn where_a_windows_edges_are_is_said_again_once_it_is_a_window() {
        let mut program = Program::default();
        program.bind(1, "xdg_wm_base", 6, 3);
        program.bind(2, "wl_compositor", 6, 4);
        program.send(&sent(4, 0, &words(&[5])), &[]);
        program.send(&sent(3, 2, &words(&[6, 5])), &[]);
        program.send(&sent(6, 1, &words(&[7])), &[]);
        program.send(&sent(6, 3, &words(&[30, 20, 200, 100])), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let sent = opcodes(&sending);
        let at = |wanted| sent.iter().position(|sent| *sent == wanted).unwrap();
        assert!(at((6, 1)) < at((6, 3)), "the role first: {sent:?}");
    }

    #[test]
    fn what_undoes_what_was_set_is_made_again_in_the_order_last_made() {
        let mut program = Program::default();
        program.bind(1, "xdg_wm_base", 6, 3);
        program.bind(2, "wl_compositor", 6, 4);
        program.bind(3, "zxdg_decoration_manager_v1", 1, 5);
        program.send(&sent(4, 0, &words(&[6])), &[]);
        program.send(&sent(3, 2, &words(&[7, 6])), &[]);
        program.send(&sent(7, 1, &words(&[8])), &[]);
        program.send(&sent(5, 1, &words(&[9, 8])), &[]);
        // Drawn by the compositor, then as the compositor would, then by
        // itself.
        program.send(&sent(9, 1, &words(&[2])), &[]);
        program.send(&sent(9, 2, &[]), &[]);
        program.send(&sent(9, 1, &words(&[1])), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let decoration: Vec<(u32, u32)> = opcodes(&sending)
            .into_iter()
            .filter(|(object, _)| *object == 9)
            .collect();
        assert_eq!(decoration, [(9, 2), (9, 1)], "unset, then set");
        let mode = sending
            .iter()
            .rfind(|(bytes, _)| wire::word(bytes, 0) == Some(9));
        assert_eq!(wire::word(&mode.unwrap().0, HEADER), Some(1), "by itself");
    }

    #[test]
    fn parts_are_stacked_as_they_last_were_and_committed_before_what_they_are_part_of() {
        let mut program = surfaced();
        program.bind(2, "wl_subcompositor", 1, 5);
        for surface in [6, 7, 8, 12] {
            program.send(&sent(3, 0, &words(&[surface])), &[]);
        }
        // 6, 7 and 12 parts of 4, and 8 of 6.
        program.send(&sent(5, 1, &words(&[9, 6, 4])), &[]);
        program.send(&sent(5, 1, &words(&[10, 7, 4])), &[]);
        program.send(&sent(5, 1, &words(&[11, 8, 6])), &[]);
        program.send(&sent(5, 1, &words(&[13, 12, 4])), &[]);
        // 7 put under 4; 6 put by what is not its sibling, so on top; and
        // 12 no longer a part.
        program.send(&sent(10, 3, &words(&[4])), &[]);
        program.send(&sent(9, 2, &words(&[8])), &[]);
        program.send(&sent(13, 0, &[]), &[]);
        for surface in [4, 6, 7, 8, 12] {
            program.send(&sent(surface, 6, &[]), &[]);
        }
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let sent = opcodes(&sending);
        let placed: Vec<&(u32, u32)> = sent
            .iter()
            .filter(|(object, opcode)| [9, 10, 11].contains(object) && *opcode >= 2)
            .collect();
        assert_eq!(placed, [&(9, 2), &(10, 3), &(11, 2)]);
        let under = sending
            .iter()
            .find(|(bytes, _)| wire::word(bytes, 0) == Some(10));
        assert_eq!(wire::word(&under.unwrap().0, HEADER), Some(4), "under 4");
        let committed: Vec<u32> = sent
            .iter()
            .filter(|(_, opcode)| *opcode == 6)
            .map(|(object, _)| *object)
            .collect();
        assert_eq!(committed, [8, 6, 7, 4, 12], "the deepest first");
    }

    #[test]
    fn a_surface_made_again_under_a_number_or_given_another_part_is_stacked_anew() {
        let mut program = surfaced();
        program.bind(2, "wl_subcompositor", 1, 5);
        program.send(&sent(3, 0, &words(&[6])), &[]);
        program.send(&sent(3, 0, &words(&[7])), &[]);
        program.send(&sent(5, 1, &words(&[8, 6, 4])), &[]);
        program.send(&sent(5, 1, &words(&[9, 7, 6])), &[]);
        // 6's part let go and 6 destroyed; the number taken by a surface new
        // and with no parts; 7 then made part of 4 instead.
        program.send(&sent(8, 0, &[]), &[]);
        program.send(&sent(9, 0, &[]), &[]);
        program.send(&sent(6, 0, &[]), &[]);
        for freed in [8, 9, 6] {
            assert!(program.told(&sent(1, 1, &words(&[freed]))), "{freed} freed");
        }
        program.send(&sent(3, 0, &words(&[6])), &[]);
        program.send(&sent(5, 1, &words(&[8, 7, 4])), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        let placed: Vec<(u32, u32)> = opcodes(&sending)
            .into_iter()
            .filter(|(object, opcode)| *object == 8 && *opcode >= 2)
            .collect();
        assert_eq!(placed, [(8, 2)], "7 over 4, and nothing of 6");
        let stacked: Vec<_> = program.journal.stacks.keys().copied().collect();
        assert_eq!(stacked, [4]);
    }

    #[test]
    fn a_window_is_committed_bare_and_shown_once_the_compositor_has_said_how() {
        let mut program = surfaced();
        program.bind(2, "xdg_wm_base", 6, 5);
        program.send(&sent(5, 2, &words(&[6, 4])), &[]);
        program.send(&sent(6, 1, &words(&[7])), &[]);
        pooled(&mut program, 9);
        buffered(&mut program, 9, 10);
        program.send(&sent(4, 1, &words(&[10, 0, 0])), &[]);
        program.send(&sent(4, 8, &words(&[2])), &[]);
        program.send(&sent(4, 6, &[]), &[]);
        let (sending, awaited) = replayed(&program.journal, &Some).unwrap();
        let surface: Vec<(u32, u32)> = opcodes(&sending)
            .into_iter()
            .filter(|(object, _)| *object == 4)
            .collect();
        assert_eq!(
            surface,
            [(4, 8), (4, 6)],
            "committed scaled, with no buffer"
        );
        assert_eq!(awaited, [6]);
        let shown = program.journal.shown(6);
        assert_eq!(opcodes(&shown), [(4, 1), (4, 8), (4, 9), (4, 6)]);
        assert_eq!(opcodes(&program.journal.shown(99)), [(0_u32, 0_u32); 0]);
    }

    #[test]
    fn what_is_destroyed_while_shown_is_made_again_and_its_number_withheld() {
        let mut program = surfaced();
        pooled(&mut program, 6);
        buffered(&mut program, 6, 7);
        program.send(&sent(4, 1, &words(&[7, 0, 0])), &[]);
        program.send(&sent(4, 6, &[]), &[]);
        // The buffer and its pool destroyed, still shown.
        program.send(&sent(7, 0, &[]), &[]);
        program.send(&sent(6, 1, &[]), &[]);
        assert!(
            !program.told(&sent(1, 1, &words(&[7]))),
            "the buffer's number withheld"
        );
        assert!(!program.told(&sent(1, 1, &words(&[6]))), "and its pool's");
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        assert!(opcodes(&sending).contains(&(6, 0)), "the buffer made again");
        assert_eq!(opcodes(&program.journal.gone()), [(7, 0), (6, 1)]);
        assert_eq!(program.journal.freed(), [0_u32; 0]);

        // Once another is shown, both may be taken again.
        pooled(&mut program, 9);
        buffered(&mut program, 9, 8);
        program.send(&sent(4, 1, &words(&[8, 0, 0])), &[]);
        program.send(&sent(4, 6, &[]), &[]);
        let mut freed = program.journal.freed();
        freed.sort_unstable();
        assert_eq!(freed, [6, 7]);
        assert_eq!(opcodes(&program.journal.gone()), [(0_u32, 0_u32); 0]);
    }

    #[test]
    fn what_is_destroyed_unnamed_is_let_go_of() {
        let mut program = surfaced();
        program.send(&sent(3, 1, &words(&[5])), &[]);
        program.send(&sent(5, 0, &[]), &[]);
        assert!(
            program.told(&sent(1, 1, &words(&[5]))),
            "its number given back"
        );
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        assert!(!opcodes(&sending).contains(&(3, 1)), "not made again");
        // Nor what is destroyed that was never kept.
        program.send(&sent(4, 0, &[]), &[]);
    }

    #[test]
    fn what_the_compositor_destroys_is_not_made_again() {
        let mut program = Program::default();
        program.send(&sent(1, 0, &words(&[2])), &[]);
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        assert_eq!(opcodes(&sending), [(1, 0)], "a sync not yet answered");
        assert!(program.told(&sent(2, 0, &words(&[5]))), "answered");
        let (sending, _) = replayed(&program.journal, &Some).unwrap();
        assert_eq!(opcodes(&sending), [(0_u32, 0_u32); 0]);
    }

    #[test]
    fn a_program_holding_what_cannot_be_made_again_cannot_be_kept() {
        let mut program = surfaced();
        program.bind(2, "zwp_linux_dmabuf_v1", 3, 5);
        assert_eq!(program.journal.unkept(), [""; 0], "a global bound");
        program.send(&sent(5, 1, &words(&[6])), &[]);
        program.send(&sent(5, 1, &words(&[7])), &[]);
        program.bind(3, "wp_presentation", 2, 8);
        program.send(&sent(8, 1, &words(&[4, 9])), &[]);
        assert_eq!(
            program.journal.unkept(),
            ["zwp_linux_buffer_params_v1", "wp_presentation_feedback"],
            "each once, in the order made"
        );
        assert!(keeps("wl_seat") && keeps("xdg_toplevel") && !keeps("xdg_popup"));
    }

    #[test]
    fn each_request_does_what_its_rule_says() {
        assert_eq!(rule("wl_surface", "attach"), Rule::Pending);
        assert_eq!(rule("wl_surface", "commit"), Rule::Commit);
        assert_eq!(rule("wl_region", "subtract"), Rule::Adding);
        assert_eq!(
            rule("zwp_primary_selection_source_v1", "offer"),
            Rule::Adding
        );
        assert_eq!(rule("wl_data_source", "set_actions"), Rule::Lasting);
        assert_eq!(rule("xdg_toplevel", "set_title"), Rule::Lasting);
        assert_eq!(rule("xdg_toplevel", "move"), Rule::Passing);
        assert_eq!(rule("wp_viewport", "set_destination"), Rule::Lasting);
        assert_eq!(rule("wl_subsurface", "set_desync"), Rule::Lasting);
        assert_eq!(rule("wl_subsurface", "place_above"), Rule::Passing);
    }

    #[test]
    fn a_message_of_the_relays_own_is_made_as_any_other() {
        let (bytes, fds) = message(4, 6, &[]);
        assert_eq!(bytes, sent(4, 6, &[]));
        assert!(fds.is_empty());
    }
}
