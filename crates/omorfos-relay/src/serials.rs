//! The serials a program holds, kept to their meaning through a change of
//! compositor (ADR 61, step 3b).
//!
//! A compositor numbers what happens, an enter, a key, a configure, and a
//! program names one of those numbers back when it asks something of it: a
//! cursor set as the pointer entered, a configure acked. Each compositor
//! counts from its own start, so the next's serials would go back under a
//! program, and mean nothing to it of the one before's. Until the first
//! change of compositor, each passes as it is. From then on the next
//! compositor's are told past every serial the program has, and taken back
//! as they are asked with; one of a compositor before is asked with as the
//! next compositor's that stands for it.
//!
//! Which argument is a serial is written here by hand, as a message's
//! description says only that it is a number. Some arguments called a
//! serial are counts of the program's own, and are not here: what
//! `zwp_text_input_v3.done` and `action` carry, which count the field's
//! commits, and `zwp_input_method_v2.commit`'s, which counts what the
//! compositor said. The X server's `xwayland_surface_v1.set_serial` is not
//! here either, as the X server is not relayed.

/// The requests that carry a compositor's serial: the interface, the
/// request, and which argument it is, each before it a word.
pub const REQUESTS: [(&str, &str, usize); 16] = [
    ("ext_session_lock_surface_v1", "ack_configure", 0),
    ("wl_data_device", "set_selection", 1),
    ("wl_data_device", "start_drag", 3),
    ("wl_data_offer", "accept", 0),
    ("wl_pointer", "set_cursor", 0),
    ("wp_cursor_shape_device_v1", "set_shape", 0),
    ("xdg_activation_token_v1", "set_serial", 0),
    ("xdg_popup", "grab", 1),
    ("xdg_positioner", "set_parent_configure", 0),
    ("xdg_surface", "ack_configure", 0),
    ("xdg_toplevel", "move", 1),
    ("xdg_toplevel", "resize", 1),
    ("xdg_toplevel", "show_window_menu", 1),
    ("xdg_wm_base", "pong", 0),
    ("zwlr_layer_surface_v1", "ack_configure", 0),
    ("zwp_primary_selection_device_v1", "set_selection", 1),
];

/// The events that carry a compositor's serial, as [`REQUESTS`] says them.
pub const EVENTS: [(&str, &str, usize); 22] = [
    ("ext_session_lock_surface_v1", "configure", 0),
    ("wl_data_device", "enter", 0),
    ("wl_keyboard", "enter", 0),
    ("wl_keyboard", "key", 0),
    ("wl_keyboard", "leave", 0),
    ("wl_keyboard", "modifiers", 0),
    ("wl_pointer", "button", 0),
    ("wl_pointer", "enter", 0),
    ("wl_pointer", "leave", 0),
    ("wl_touch", "down", 0),
    ("wl_touch", "up", 0),
    ("xdg_surface", "configure", 0),
    ("xdg_wm_base", "ping", 0),
    ("zwlr_layer_surface_v1", "configure", 0),
    ("zwp_input_method_keyboard_grab_v2", "key", 0),
    ("zwp_input_method_keyboard_grab_v2", "modifiers", 0),
    ("zwp_pointer_gesture_hold_v1", "begin", 0),
    ("zwp_pointer_gesture_hold_v1", "end", 0),
    ("zwp_pointer_gesture_pinch_v1", "begin", 0),
    ("zwp_pointer_gesture_pinch_v1", "end", 0),
    ("zwp_pointer_gesture_swipe_v1", "begin", 0),
    ("zwp_pointer_gesture_swipe_v1", "end", 0),
];

/// Where in its body the message `name` of `interface` carries a serial,
/// if it does, as `carrying` lists them.
#[must_use]
pub fn carried(carrying: &[(&str, &str, usize)], interface: &str, name: &str) -> Option<usize> {
    carrying
        .iter()
        .find(|(of, called, _)| *of == interface && *called == name)
        .map(|(_, _, argument)| argument * 4)
}

/// Half the serials there are: one this far past another is taken to be
/// before it, as serials go round.
const HALF: u32 = 1 << 31;

/// A program's serials and the compositor's, as they stand.
#[derive(Debug, Default)]
pub struct Serials {
    /// What the program is told is the compositor's, and this more.
    offset: u32,
    /// The first the program may be told of this compositor's, once it is
    /// not the first: one before is a compositor before's.
    since: Option<u32>,
    /// The last the program has been told, as it has it.
    last: u32,
    /// The compositor's last, as it has it.
    latest: u32,
    /// The compositor's of the pointer's last enter, as it has it.
    entered: Option<u32>,
}

impl Serials {
    /// The program's for the compositor's `theirs`, which it is told.
    pub fn told(&mut self, theirs: u32) -> u32 {
        self.latest = theirs;
        self.last = theirs.wrapping_add(self.offset);
        self.last
    }

    /// The compositor's for the program's `ours`, which it asks with; one of
    /// a compositor before is the next's last, or for the pointer's cursor,
    /// `pointer`, of its last enter, as it asks for that.
    #[must_use]
    pub fn asked(&self, ours: u32, pointer: bool) -> u32 {
        let Some(since) = self.since else {
            return ours;
        };
        if ours.wrapping_sub(since) < HALF {
            return ours.wrapping_sub(self.offset);
        }
        let entered = self.entered.filter(|_| pointer);
        entered.unwrap_or(self.latest)
    }

    /// Takes in that the compositor's last serial was of the pointer
    /// entering.
    pub fn entered(&mut self) {
        self.entered = Some(self.latest);
    }

    /// The compositor's last serial, as it has it.
    #[must_use]
    pub fn latest(&self) -> u32 {
        self.latest
    }

    /// The last serial the program has been told, as it has it.
    #[must_use]
    pub fn last(&self) -> u32 {
        self.last
    }

    /// Takes in that the program is joined to another compositor, whose
    /// serials are told past every one it has been.
    pub fn joined(&mut self) {
        self.offset = self.last.wrapping_add(1);
        self.since = Some(self.offset);
        self.entered = None;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use wayland_backend::protocol::{ArgumentType, Interface};

    use super::*;
    use crate::known;

    /// Every interface a program may come to hold an object of, by name.
    fn interfaces() -> HashMap<&'static str, &'static Interface> {
        let mut found = HashMap::new();
        let mut waiting: Vec<&'static Interface> = known::globals().collect();
        waiting.push(known::display());
        while let Some(interface) = waiting.pop() {
            if found.insert(interface.name, interface).is_some() {
                continue;
            }
            for message in interface.requests.iter().chain(interface.events) {
                waiting.extend(message.child_interface);
                waiting.extend(message.arg_interfaces.iter().copied());
            }
        }
        found
    }

    #[test]
    fn each_serial_listed_is_a_number_after_words_alone() {
        let interfaces = interfaces();
        for (carrying, events) in [(&REQUESTS[..], false), (&EVENTS[..], true)] {
            for (interface, name, argument) in carrying {
                let of = interfaces[interface];
                let messages = if events { of.events } else { of.requests };
                let message = messages.iter().find(|message| message.name == *name);
                let message = message.unwrap();
                // Before it, only what takes a word: no string or array.
                let before = format!("{:?}", &message.signature[..*argument]);
                let words = !before.contains("Str") && !before.contains("Array");
                assert!(words, "{interface}.{name}: {before}");
                let serial = message.signature.get(*argument);
                assert_eq!(serial, Some(&ArgumentType::Uint), "{interface}.{name}");
            }
        }
    }

    #[test]
    fn a_serial_is_found_where_it_is_carried_and_only_there() {
        assert_eq!(carried(&REQUESTS, "xdg_toplevel", "move"), Some(4));
        assert_eq!(carried(&EVENTS, "wl_pointer", "enter"), Some(0));
        assert_eq!(carried(&EVENTS, "wl_pointer", "motion"), None);
        assert_eq!(
            carried(&EVENTS, "zwp_text_input_v3", "done"),
            None,
            "a count"
        );
    }

    #[test]
    fn until_the_compositor_changes_a_serial_passes_as_it_is() {
        let mut serials = Serials::default();
        assert_eq!(serials.told(500), 500);
        assert_eq!(serials.asked(500, false), 500);
        assert_eq!(serials.asked(u32::MAX, true), u32::MAX);
    }

    #[test]
    fn the_next_compositors_serials_are_told_past_every_one_before_and_asked_as_its_own() {
        let mut serials = Serials::default();
        serials.told(500);
        serials.joined();
        let told = serials.told(3);
        assert_eq!(told, 504);
        assert_eq!(serials.asked(told, false), 3);

        // One of the compositor before's is its last, or for the cursor its
        // last enter's, once there is one.
        assert_eq!(serials.asked(500, true), 3, "no enter yet");
        serials.told(7);
        serials.entered();
        serials.told(9);
        assert_eq!(serials.asked(500, false), 9);
        assert_eq!(serials.asked(500, true), 7);
        assert_eq!(serials.last(), 510);

        // And again, past those.
        serials.joined();
        assert_eq!(serials.told(1), 512);
        assert_eq!(serials.asked(510, true), 1, "the one before's last");
    }

    #[test]
    fn serials_that_go_round_are_still_told_in_order() {
        let mut serials = Serials::default();
        serials.told(u32::MAX - 1);
        serials.joined();
        let told = serials.told(5);
        assert_eq!(told, 4, "round past the top");
        assert_eq!(serials.asked(told, false), 5);
        assert_eq!(serials.asked(u32::MAX - 1, false), 5, "before");
    }
}
