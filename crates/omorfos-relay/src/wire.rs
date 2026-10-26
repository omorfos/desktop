//! How a Wayland message is laid out, read against the description of the
//! message it is: what it makes, and how many descriptors come with it.
//!
//! Every word is in the byte order of the machine, as Wayland has it. A
//! message is its header, the object it is to and a word holding its size
//! and which message of that object's it is, then its arguments, each a
//! word but for strings and arrays, which are a length and then as many
//! bytes, made up to a whole number of words. Descriptors do not come in
//! the bytes but beside them.

use wayland_backend::protocol::{AllowNull, ArgumentType, MessageDesc};

/// The longest a message may be, its header among it, as libwayland
/// bounds it.
pub const LONGEST: usize = 4096;

/// How long a message's header is.
pub const HEADER: usize = 8;

/// The header of a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    /// The object the message is to.
    pub object: u32,
    /// Which of the object's messages it is.
    pub opcode: u16,
    /// How long the message is, its header among it.
    pub size: usize,
}

/// Why a message cannot be passed on: the program that sent it is
/// dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Broken(pub &'static str);

const SHORT: Broken = Broken("a message shorter than what it carries");
const LONG: Broken = Broken("a message longer than what it carries");

/// What a message carries that the relay keeps track of.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Carried<'a> {
    /// The number of the object it makes, if it makes one.
    pub made: Option<u32>,
    /// The name of the interface the object made is of, and its version,
    /// where the message says them, as a registry's `bind` does, rather
    /// than its description.
    pub named: Option<(&'a [u8], u32)>,
    /// How many descriptors come with it.
    pub fds: usize,
}

/// The header of the message `bytes` start with, once the whole of the
/// message is there.
///
/// # Errors
///
/// Returns an error if the header says a size no message can have.
pub fn header(bytes: &[u8]) -> Result<Option<Header>, Broken> {
    let (Some(object), Some(sized)) = (word(bytes, 0), word(bytes, 4)) else {
        return Ok(None);
    };
    let size = usize::from(u16::try_from(sized >> 16).unwrap_or(u16::MAX));
    let opcode = u16::try_from(sized & 0xffff).unwrap_or(u16::MAX);
    if !(HEADER..=LONGEST).contains(&size) || size % 4 != 0 {
        return Err(Broken("a message of a size no message has"));
    }
    let header = Header {
        object,
        opcode,
        size,
    };
    Ok((bytes.len() >= size).then_some(header))
}

/// What the arguments `body` of the message `message` carry.
///
/// # Errors
///
/// Returns an error if the arguments are not what the description says:
/// too few, too many, a string without its end or a nothing where one is
/// not let be, or no object where one is made.
pub fn read<'a>(body: &'a [u8], message: &MessageDesc) -> Result<Carried<'a>, Broken> {
    let mut carried = Carried::default();
    let mut at = 0;
    // An object made of an interface the message does not describe follows
    // the interface's name and version.
    let mut string = None;
    let mut number = None;
    for argument in message.signature {
        match argument {
            ArgumentType::Fd => carried.fds += 1,
            ArgumentType::Str(null) => {
                let (text, next) = sized(body, at)?;
                at = next;
                string = Some(self::string(text, *null)?);
            }
            ArgumentType::Array => at = sized(body, at)?.1,
            ArgumentType::NewId => {
                let made = word(body, at).ok_or(SHORT)?;
                at += 4;
                if made == 0 {
                    return Err(Broken("an object made with no number"));
                }
                carried.made = Some(made);
                carried.named = string.zip(number);
            }
            ArgumentType::Uint => {
                number = Some(word(body, at).ok_or(SHORT)?);
                at += 4;
            }
            ArgumentType::Int | ArgumentType::Fixed | ArgumentType::Object(_) => {
                word(body, at).ok_or(SHORT)?;
                at += 4;
            }
        }
    }
    if at == body.len() {
        Ok(carried)
    } else {
        Err(LONG)
    }
}

/// The word at `at` in `bytes`, if there is one.
pub fn word(bytes: &[u8], at: usize) -> Option<u32> {
    let bytes = bytes.get(at..at.checked_add(4)?)?;
    bytes.try_into().ok().map(u32::from_ne_bytes)
}

/// The bytes of the string or array at `at`, and where the next argument
/// starts.
fn sized(body: &[u8], at: usize) -> Result<(&[u8], usize), Broken> {
    let length = word(body, at).ok_or(SHORT)?;
    // No more can be there than the message holds, which is short.
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    let start = at + 4;
    let rest = body.get(start..).unwrap_or_default();
    let bytes = rest.get(..length).ok_or(SHORT)?;
    // Made up to whole words; what is there is short enough not to overflow.
    let next = start + length.next_multiple_of(4);
    if next <= body.len() {
        Ok((bytes, next))
    } else {
        Err(SHORT)
    }
}

/// A string's bytes, without the nothing it ends with, or nothing if it is
/// nothing and may be.
fn string(text: &[u8], null: AllowNull) -> Result<&[u8], Broken> {
    match text.split_last() {
        Some((0, text)) => Ok(text),
        Some(_) => Err(Broken("a string that does not end")),
        None if null == AllowNull::Yes => Ok(text),
        None => Err(Broken("no string where one has to be")),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use wayland_backend::protocol::Interface;

    use super::*;
    use crate::known;

    /// A message to `object`, the `opcode`th of its, with `arguments`.
    pub(crate) fn message(object: u32, opcode: u16, arguments: &[u8]) -> Vec<u8> {
        let size = u32::try_from(HEADER + arguments.len()).unwrap();
        let mut bytes = object.to_ne_bytes().to_vec();
        bytes.extend((size << 16 | u32::from(opcode)).to_ne_bytes());
        bytes.extend(arguments);
        bytes
    }

    /// A string as it is sent: its length with its end, then its bytes,
    /// made up to a whole number of words.
    pub(crate) fn text(text: &str) -> Vec<u8> {
        let mut bytes = u32::try_from(text.len() + 1)
            .unwrap()
            .to_ne_bytes()
            .to_vec();
        bytes.extend(text.as_bytes());
        bytes.push(0);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        bytes
    }

    /// The words given, one after another.
    pub(crate) fn words(words: &[u32]) -> Vec<u8> {
        words.iter().flat_map(|word| word.to_ne_bytes()).collect()
    }

    /// The request named `name` of `interface`.
    pub(crate) fn request(interface: &'static Interface, name: &str) -> &'static MessageDesc {
        interface
            .requests
            .iter()
            .find(|message| message.name == name)
            .unwrap()
    }

    fn registry() -> &'static Interface {
        request(known::display(), "get_registry")
            .child_interface
            .unwrap()
    }

    #[test]
    fn a_header_is_read_once_the_whole_message_is_there() {
        let bytes = message(3, 2, &words(&[7]));
        let read = header(&bytes);
        assert_eq!(
            read,
            Ok(Some(Header {
                object: 3,
                opcode: 2,
                size: 12
            }))
        );
        assert_eq!(header(&bytes[..11]), Ok(None), "a word still to come");
        assert_eq!(header(&bytes[..7]), Ok(None), "not all of the header");
    }

    #[test]
    fn a_header_saying_a_size_no_message_has_is_broken() {
        let mut bytes = message(3, 0, &[]);
        for size in [4_u32, 10, 4100] {
            bytes[4..8].copy_from_slice(&(size << 16).to_ne_bytes());
            assert!(header(&bytes).is_err(), "{size}");
        }
    }

    #[test]
    fn an_object_bound_by_name_carries_its_name_and_version() {
        let bind = request(registry(), "bind");
        let mut body = words(&[12]);
        body.extend(text("wl_compositor"));
        body.extend(words(&[6, 4]));
        let carried = read(&body, bind).unwrap();
        assert_eq!(carried.made, Some(4));
        assert_eq!(carried.named, Some((&b"wl_compositor"[..], 6)));
    }

    #[test]
    fn an_object_made_of_an_interface_described_carries_no_name() {
        let get_registry = request(known::display(), "get_registry");
        let body = words(&[2]);
        let carried = read(&body, get_registry).unwrap();
        assert_eq!((carried.made, carried.named), (Some(2), None));
    }

    #[test]
    fn descriptors_are_counted_beside_the_bytes() {
        let shm = known::global("wl_shm").unwrap();
        let create_pool = request(shm, "create_pool");
        let body = words(&[5, 4096]);
        let carried = read(&body, create_pool).unwrap();
        assert_eq!((carried.made, carried.fds), (Some(5), 1));
    }

    #[test]
    fn an_array_is_passed_over_by_its_length() {
        let base = known::global("xdg_wm_base").unwrap();
        let xdg_surface = request(base, "get_xdg_surface").child_interface.unwrap();
        let toplevel = request(xdg_surface, "get_toplevel")
            .child_interface
            .unwrap();
        let configure = toplevel
            .events
            .iter()
            .find(|event| event.name == "configure")
            .unwrap();
        let mut body = words(&[100, 200, 5]);
        body.extend([1, 2, 3, 4, 5, 0, 0, 0]);
        assert_eq!(read(&body, configure).map(|carried| carried.made), Ok(None));
        body.truncate(18);
        assert_eq!(read(&body, configure), Err(SHORT), "an array cut short");
    }

    #[test]
    fn arguments_that_are_not_as_described_are_broken() {
        let get_registry = request(known::display(), "get_registry");
        assert_eq!(read(&[], get_registry), Err(SHORT), "no object");
        assert_eq!(
            read(&words(&[2, 3]), get_registry),
            Err(LONG),
            "a word too many"
        );
        assert!(
            read(&words(&[0]), get_registry).is_err(),
            "an object of no number"
        );

        let destination = request(known::global("wp_viewporter").unwrap(), "get_viewport");
        let viewport = destination.child_interface.unwrap();
        let set_destination = request(viewport, "set_destination");
        assert_eq!(
            read(&words(&[1]), set_destination),
            Err(SHORT),
            "a number short"
        );
    }

    #[test]
    fn a_string_has_to_end_and_be_there_unless_it_may_be_nothing() {
        let bind = request(registry(), "bind");
        let mut unended = words(&[12, 4]);
        unended.extend(b"wl_c");
        unended.extend(words(&[6, 4]));
        assert!(read(&unended, bind).is_err(), "no end");
        let nothing = words(&[12, 0, 6, 4]);
        assert!(
            read(&nothing, bind).is_err(),
            "nothing where a name has to be"
        );
        let long = words(&[12, 9]);
        assert_eq!(read(&long, bind), Err(SHORT), "longer than what is left");
        let endless = words(&[12, u32::MAX]);
        assert_eq!(read(&endless, bind), Err(SHORT), "as long as can be");
        assert_eq!(read(&words(&[12]), bind), Err(SHORT), "no length");

        let activation = known::global("xdg_activation_v1").unwrap();
        let token = request(activation, "get_activation_token")
            .child_interface
            .unwrap();
        let set_app_id = request(token, "set_app_id");
        assert!(
            read(&words(&[0]), set_app_id).is_err(),
            "an app id has to be"
        );
        let title = request(
            request(
                request(known::global("xdg_wm_base").unwrap(), "get_xdg_surface")
                    .child_interface
                    .unwrap(),
                "get_toplevel",
            )
            .child_interface
            .unwrap(),
            "set_title",
        );
        assert!(read(&text("a"), title).is_ok());
    }

    #[test]
    fn a_string_that_may_be_nothing_can_be() {
        let manager = known::global("wl_data_device_manager").unwrap();
        let device = request(manager, "get_data_device").child_interface.unwrap();
        let offered = device
            .events
            .iter()
            .find(|event| event.name == "data_offer");
        let offer = offered.and_then(|event| event.child_interface).unwrap();
        let accept = request(offer, "accept");
        assert!(read(&words(&[5, 0]), accept).is_ok(), "accepting no type");
    }

    #[test]
    fn a_word_past_the_end_is_not_there() {
        assert_eq!(
            word(&[1, 0, 0, 0], 0),
            Some(u32::from_ne_bytes([1, 0, 0, 0]))
        );
        assert_eq!(word(&[1, 0, 0], 0), None);
        assert_eq!(word(&[], usize::MAX), None);
    }
}
