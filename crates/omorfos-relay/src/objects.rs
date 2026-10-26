//! The objects a program has made, and the compositor has made for it,
//! each with what it is, so that every message to one can be read.
//!
//! A program numbers the objects it makes from one up, and the compositor
//! those it makes from `0xff000000`. A number a program's object had is not
//! taken again until the compositor says it may be (`wl_display.delete_id`);
//! until then the object is gone but still known, as the compositor may
//! still send it what it sent before it heard. The compositor's numbers are
//! its own to take again whenever it likes.

use std::collections::HashMap;

use wayland_backend::protocol::{Interface, MessageDesc};

use crate::known;
use crate::wire::{self, Broken, Carried, HEADER, Header};

/// The most objects a program may have, gone or not. A program with this
/// many is making them faster than it ends them.
pub const MOST: usize = 1 << 18;

/// The first number the compositor gives the objects it makes.
pub const SERVERS: u32 = 0xff00_0000;

/// The number of the display, which every connection starts with.
const DISPLAY: u32 = 1;

/// Which of the display's events says a number may be taken again.
const DELETE_ID: u16 = 1;

/// An object, and what it is.
#[derive(Debug, Clone, Copy)]
struct Object {
    interface: &'static Interface,
    version: u32,
    /// It has been destroyed, but messages to it may still come.
    gone: bool,
}

/// What a message was, once read.
#[derive(Debug, Clone, Copy)]
pub struct Read {
    /// How many descriptors come with it.
    pub fds: usize,
    /// What the object it is to is.
    pub interface: &'static Interface,
    /// Which of that object's messages it is.
    pub opcode: u16,
    /// The object it makes, if it makes one, what that is, and at which
    /// version.
    pub made: Option<(u32, &'static Interface, u32)>,
    /// Whether it destroys the object it is to.
    pub ends: bool,
}

/// A program's objects.
#[derive(Debug)]
pub struct Objects {
    known: HashMap<u32, Object>,
}

impl Default for Objects {
    fn default() -> Self {
        let display = Object {
            interface: known::display(),
            version: 1,
            gone: false,
        };
        Self {
            known: HashMap::from([(DISPLAY, display)]),
        }
    }
}

impl Objects {
    /// Reads `message`, a request the program sends, and keeps what it
    /// makes and destroys. Returns what it was.
    ///
    /// # Errors
    ///
    /// Returns an error if the program could not have sent it: to an
    /// object it has not, a message the object has not or is too old for,
    /// not as described, or making an object it may not.
    pub fn request(&mut self, message: &[u8], header: Header) -> Result<Read, Broken> {
        let object = self.known.get(&header.object).filter(|object| !object.gone);
        let object = object.copied().ok_or(Broken("a request to no object"))?;
        let described = object.interface.requests.get(usize::from(header.opcode));
        let described = described.ok_or(Broken("a request its object has not"))?;
        if described.since > object.version {
            return Err(Broken("a request newer than its object"));
        }
        let carried = wire::read(body(message, header), described)?;
        let mut made_of = None;
        if let Some(made) = carried.made {
            if made >= SERVERS {
                return Err(Broken(
                    "an object made with one of the compositor's numbers",
                ));
            }
            let (interface, version) = Self::what(described, carried, object.version)?;
            self.make(made, interface, version)?;
            made_of = Some((made, interface, version));
        }
        if described.is_destructor {
            self.destroy(header.object);
        }
        Ok(Read {
            fds: carried.fds,
            interface: object.interface,
            opcode: header.opcode,
            made: made_of,
            ends: described.is_destructor,
        })
    }

    /// Reads `message`, an event the compositor sends the program, and
    /// keeps what it makes and destroys. Returns what it was.
    ///
    /// # Errors
    ///
    /// Returns an error if the compositor could not have sent it, as for
    /// [`Objects::request`].
    pub fn event(&mut self, message: &[u8], header: Header) -> Result<Read, Broken> {
        let object = self.known.get(&header.object).copied();
        let object = object.ok_or(Broken("an event to no object"))?;
        let described = object.interface.events.get(usize::from(header.opcode));
        let described = described.ok_or(Broken("an event its object has not"))?;
        let body = body(message, header);
        let carried = wire::read(body, described)?;
        let mut made_of = None;
        if let Some(made) = carried.made {
            if made < SERVERS {
                return Err(Broken("an object made with one of the program's numbers"));
            }
            let interface = described.child_interface;
            let interface = interface.ok_or(Broken("an object made of no interface"))?;
            self.make(made, interface, object.version)?;
            made_of = Some((made, interface, object.version));
        }
        if header.object == DISPLAY && header.opcode == DELETE_ID {
            // Only a program's own numbers are freed so, and never the
            // display's.
            let deleted = wire::word(body, 0);
            let deleted = deleted.filter(|deleted| (DISPLAY + 1..SERVERS).contains(deleted));
            if let Some(deleted) = deleted {
                self.known.remove(&deleted);
            }
        }
        if described.is_destructor {
            self.destroy(header.object);
        }
        Ok(Read {
            fds: carried.fds,
            interface: object.interface,
            opcode: header.opcode,
            made: made_of,
            ends: described.is_destructor,
        })
    }

    /// What the object `number` is, if the program has it and it is not
    /// gone.
    #[must_use]
    pub fn living(&self, number: u32) -> Option<&'static Interface> {
        let object = self.known.get(&number).filter(|object| !object.gone);
        object.map(|object| object.interface)
    }

    /// The description of the event `opcode` to the object `number`, gone
    /// or not, if the program has it and it has that event.
    #[must_use]
    pub fn event_described(&self, number: u32, opcode: u16) -> Option<&'static MessageDesc> {
        let object = self.known.get(&number)?;
        object.interface.events.get(usize::from(opcode))
    }

    /// Whether `number` is free for the compositor to make an object with,
    /// as the program sees it.
    #[must_use]
    pub fn free(&self, number: u32) -> bool {
        self.known.get(&number).is_none_or(|object| object.gone)
    }

    /// What each object living is, in no order.
    pub fn each_living(&self) -> impl Iterator<Item = (u32, &'static Interface)> + '_ {
        let living = self.known.iter().filter(|(_, object)| !object.gone);
        living.map(|(number, object)| (*number, object.interface))
    }

    /// How many objects the program has, gone or not.
    #[must_use]
    pub fn len(&self) -> usize {
        self.known.len()
    }

    /// Whether the program has no objects, which it never has: it has the
    /// display.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.known.is_empty()
    }

    /// What an object `described` makes is, and its version, given the
    /// version of the object it is made by.
    fn what(
        described: &MessageDesc,
        carried: Carried<'_>,
        version: u32,
    ) -> Result<(&'static Interface, u32), Broken> {
        if let Some(interface) = described.child_interface {
            return Ok((interface, version));
        }
        let (name, version) = carried
            .named
            .ok_or(Broken("an object made of no interface"))?;
        let name = std::str::from_utf8(name).ok();
        let interface = name.and_then(known::global);
        let interface = interface.ok_or(Broken("an interface the compositor does not offer"))?;
        if (1..=interface.version).contains(&version) {
            Ok((interface, version))
        } else {
            Err(Broken("a version the compositor does not offer"))
        }
    }

    /// Keeps that the object `number` is of `interface` at `version`, if
    /// the number is free to be taken.
    fn make(
        &mut self,
        number: u32,
        interface: &'static Interface,
        version: u32,
    ) -> Result<(), Broken> {
        let free = match self.known.get(&number) {
            None => self.known.len() < MOST,
            // The compositor may take one of its own numbers again as soon
            // as the object is gone; a program has to wait to be told.
            Some(object) => object.gone && number >= SERVERS,
        };
        if !free {
            return Err(Broken("an object made with a number that is not free"));
        }
        let object = Object {
            interface,
            version,
            gone: false,
        };
        self.known.insert(number, object);
        Ok(())
    }

    /// Keeps that the object `number` has been destroyed.
    fn destroy(&mut self, number: u32) {
        self.known
            .entry(number)
            .and_modify(|object| object.gone = true);
    }
}

/// The arguments of `message`.
fn body(message: &[u8], header: Header) -> &[u8] {
    message.get(HEADER..header.size).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::tests::{message, request, text, words};

    /// Reads `bytes` as a request.
    fn requested(objects: &mut Objects, bytes: &[u8]) -> Result<usize, Broken> {
        let header = wire::header(bytes).unwrap().unwrap();
        objects.request(bytes, header).map(|read| read.fds)
    }

    /// Reads `bytes` as an event.
    fn sent(objects: &mut Objects, bytes: &[u8]) -> Result<usize, Broken> {
        let header = wire::header(bytes).unwrap().unwrap();
        objects.event(bytes, header).map(|read| read.fds)
    }

    /// Binds `name` at `version` as the object `number`, through the
    /// registry, which is the object 2.
    fn bind(name: &str, version: u32, number: u32) -> Vec<u8> {
        let mut body = words(&[1]);
        body.extend(text(name));
        body.extend(words(&[version, number]));
        message(2, 0, &body)
    }

    /// A program that has its registry.
    fn registered() -> Objects {
        let mut objects = Objects::default();
        requested(&mut objects, &message(1, 1, &words(&[2]))).unwrap();
        objects
    }

    #[test]
    fn a_program_starts_with_the_display_alone() {
        let objects = Objects::default();
        assert_eq!(objects.len(), 1);
        assert!(!objects.is_empty());
    }

    #[test]
    fn what_a_program_binds_and_makes_is_known_by_what_it_is() {
        let mut objects = registered();
        requested(&mut objects, &bind("wl_compositor", 6, 3)).unwrap();
        // wl_compositor.create_surface, then wl_surface.attach with no
        // buffer, which only a surface has.
        requested(&mut objects, &message(3, 0, &words(&[4]))).unwrap();
        assert_eq!(
            requested(&mut objects, &message(4, 1, &words(&[0, 0, 0]))),
            Ok(0)
        );
        assert_eq!(objects.len(), 4);
    }

    #[test]
    fn descriptors_a_request_carries_are_counted() {
        let mut objects = registered();
        requested(&mut objects, &bind("wl_shm", 2, 3)).unwrap();
        assert_eq!(
            requested(&mut objects, &message(3, 0, &words(&[4, 4096]))),
            Ok(1)
        );
    }

    #[test]
    fn a_request_to_no_object_or_one_destroyed_is_broken() {
        let mut objects = registered();
        assert!(requested(&mut objects, &message(9, 0, &[])).is_err());
        // wl_display.sync makes a callback, which the program cannot
        // destroy; the compositor does, and then the number is gone.
        requested(&mut objects, &message(1, 0, &words(&[3]))).unwrap();
        sent(&mut objects, &message(3, 0, &words(&[0]))).unwrap();
        assert!(requested(&mut objects, &message(3, 0, &[])).is_err());
    }

    #[test]
    fn a_request_its_object_has_not_or_is_too_old_for_is_broken() {
        let mut objects = registered();
        assert!(
            requested(&mut objects, &message(2, 7, &[])).is_err(),
            "no such"
        );
        let undescribed = message(1, 1, &[]);
        assert!(
            requested(&mut objects, &undescribed).is_err(),
            "not as described"
        );
        requested(&mut objects, &bind("wl_compositor", 1, 3)).unwrap();
        requested(&mut objects, &message(3, 0, &words(&[4]))).unwrap();
        // wl_surface.set_buffer_scale came with version 3.
        let scale = message(4, 8, &words(&[2]));
        assert_eq!(
            requested(&mut objects, &scale),
            Err(Broken("a request newer than its object"))
        );
    }

    #[test]
    fn what_is_not_offered_cannot_be_bound() {
        let mut objects = registered();
        assert!(
            requested(&mut objects, &bind("wl_drm", 2, 3)).is_err(),
            "not offered"
        );
        assert!(
            requested(&mut objects, &bind("wl_seat", 0, 3)).is_err(),
            "no version"
        );
        assert!(
            requested(&mut objects, &bind("wl_seat", 99, 3)).is_err(),
            "too new"
        );
        let mut unnamed = bind("wl_seat", 1, 3);
        unnamed[16] = 0xff;
        assert!(requested(&mut objects, &unnamed).is_err(), "not a name");
        assert_eq!(objects.len(), 2);
    }

    #[test]
    fn a_program_may_not_take_a_number_in_use_or_the_compositors() {
        let mut objects = registered();
        requested(&mut objects, &bind("wl_compositor", 6, 3)).unwrap();
        assert!(
            requested(&mut objects, &bind("wl_shm", 1, 3)).is_err(),
            "in use"
        );
        assert!(
            requested(&mut objects, &bind("wl_shm", 1, SERVERS)).is_err(),
            "not its own"
        );
    }

    #[test]
    fn a_programs_number_is_free_again_only_once_the_compositor_says() {
        let mut objects = registered();
        requested(&mut objects, &bind("wl_compositor", 6, 3)).unwrap();
        requested(&mut objects, &message(3, 0, &words(&[4]))).unwrap();
        // wl_surface.destroy.
        requested(&mut objects, &message(4, 0, &[])).unwrap();
        let again = message(3, 0, &words(&[4]));
        assert!(requested(&mut objects, &again).is_err(), "not yet");
        // An event still on its way to the surface is read.
        assert_eq!(sent(&mut objects, &message(4, 0, &words(&[7]))), Ok(0));
        sent(&mut objects, &message(1, 1, &words(&[4]))).unwrap();
        assert_eq!(requested(&mut objects, &again), Ok(0));
    }

    #[test]
    fn being_told_a_number_is_free_frees_no_other() {
        let mut objects = registered();
        sent(&mut objects, &message(1, 1, &words(&[1]))).unwrap();
        sent(&mut objects, &message(1, 1, &words(&[SERVERS]))).unwrap();
        assert!(
            sent(&mut objects, &message(1, 1, &[])).is_err(),
            "says no number"
        );
        assert_eq!(objects.len(), 2, "the display and the registry");
    }

    #[test]
    fn what_the_compositor_makes_is_known_and_its_numbers_taken_again() {
        let mut objects = registered();
        requested(&mut objects, &bind("wl_data_device_manager", 3, 3)).unwrap();
        requested(&mut objects, &bind("wl_seat", 9, 4)).unwrap();
        // wl_data_device_manager.get_data_device, then its data_offer.
        requested(&mut objects, &message(3, 1, &words(&[5, 4]))).unwrap();
        let offer = message(5, 0, &words(&[SERVERS]));
        sent(&mut objects, &offer).unwrap();
        assert!(sent(&mut objects, &offer).is_err(), "while it is there");
        // wl_data_offer.destroy, and the compositor may take the number.
        requested(&mut objects, &message(SERVERS, 2, &[])).unwrap();
        assert_eq!(sent(&mut objects, &offer), Ok(0));
    }

    #[test]
    fn the_compositor_may_not_make_objects_with_a_programs_numbers() {
        let mut objects = registered();
        requested(&mut objects, &bind("wl_data_device_manager", 3, 3)).unwrap();
        requested(&mut objects, &bind("wl_seat", 9, 4)).unwrap();
        requested(&mut objects, &message(3, 1, &words(&[5, 4]))).unwrap();
        assert!(sent(&mut objects, &message(5, 0, &words(&[9]))).is_err());
    }

    #[test]
    fn an_event_to_no_object_or_that_its_object_has_not_is_broken() {
        let mut objects = registered();
        assert!(sent(&mut objects, &message(9, 0, &[])).is_err());
        assert!(sent(&mut objects, &message(1, 7, &[])).is_err());
    }

    #[test]
    fn a_program_may_have_only_so_many_objects() {
        let mut objects = registered();
        let compositor = known::global("wl_compositor").unwrap();
        for number in 3..=u32::try_from(MOST).unwrap() {
            objects.make(number, compositor, 1).unwrap();
        }
        assert_eq!(objects.len(), MOST);
        assert!(requested(&mut objects, &bind("wl_shm", 1, 1 << 20)).is_err());
    }

    #[test]
    fn an_object_made_of_an_interface_neither_described_nor_named_is_broken() {
        let get_registry = request(known::display(), "get_registry");
        let unnamed = MessageDesc {
            child_interface: None,
            ..*get_registry
        };
        let carried = Carried {
            made: Some(2),
            ..Carried::default()
        };
        assert!(Objects::what(&unnamed, carried, 1).is_err());

        let mut objects = registered();
        let unknown: &'static Interface = Box::leak(Box::new(Interface {
            events: Box::leak(Box::new([unnamed])),
            ..*known::display()
        }));
        objects.make(3, unknown, 1).unwrap();
        assert!(sent(&mut objects, &message(3, 0, &words(&[SERVERS]))).is_err());
    }
}
