//! The Wayland protocols the compositor offers, each with how a program's
//! state in it is made again when the compositor is started anew under the
//! relay (ADR 60).
//!
//! The relay knows these and no others. A protocol is offered by being added
//! here, at the version offered, with its note; a test of the compositor's
//! checks that what a program is offered is this list and nothing else, so
//! a new version of Smithay that offers a protocol at another version fails
//! until the list, and the relay, have caught up with it.
//!
//! What holds for every protocol is said once, here. Each object a program
//! made is made again by the request that made it, with what it was made
//! with, and the last of each request that sets its state is sent again.
//! A descriptor a program handed over, a buffer's or a keymap's, is held by
//! the relay and handed over again. The program is told only what changed:
//! what the new compositor says that it had been told already is kept from
//! it, and the serials and handles it has keep their meaning, the relay
//! putting the new compositor's in their place. What cannot be carried,
//! something in flight at that moment, is ended as the protocol lets it
//! end, so that the program has seen nothing it could not have seen.

/// A protocol the compositor offers: a global a program may bind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Protocol {
    /// The name of its global, as the registry says it.
    pub interface: &'static str,
    /// The version of it offered.
    pub version: u32,
    /// How a program's state in it is made again, beyond what holds for
    /// every protocol, or that there is none.
    pub replayed: &'static str,
}

/// The protocol offered to the X server alone, which no other program is
/// shown.
pub const X_SERVER: &str = "xwayland_shell_v1";

/// Every protocol the compositor offers, by the name of its global.
pub const OFFERED: &[Protocol] = &[
    Protocol {
        interface: "ext_data_control_manager_v1",
        version: 1,
        replayed: "As the clipboard's: each device made again for its seat, and what it set as \
                   the selection set again from its source; what it is offered is passed on only \
                   if it is not what it was offered last.",
    },
    Protocol {
        interface: "ext_foreign_toplevel_image_capture_source_manager_v1",
        version: 1,
        replayed: "Each source made again for the same window, by its handle in the list of \
                   windows; one whose window has gone stops the sessions on it, as when the \
                   window closes.",
    },
    Protocol {
        interface: "ext_foreign_toplevel_list_v1",
        version: 1,
        replayed: "The list made again, each window in it matched to the one the relay made \
                   again and kept under the handle and identifier the program knows; a window \
                   new meanwhile is said as new, one gone as closed.",
    },
    Protocol {
        interface: "ext_idle_notifier_v1",
        version: 2,
        replayed: "Each notification made again with its timeout, which counts again from then. \
                   A program told the user is idle is not told they came back by a notification \
                   made anew, only when they do.",
    },
    Protocol {
        interface: "ext_image_copy_capture_manager_v1",
        version: 1,
        replayed: "Each session made again on its source, its buffer constraints passed on only \
                   if they changed. A frame being captured at that moment cannot be carried: it \
                   is failed, as a frame may fail, and the program asks for another.",
    },
    Protocol {
        interface: "ext_output_image_capture_source_manager_v1",
        version: 1,
        replayed: "Each source made again for the same screen; one whose screen has gone stops \
                   the sessions on it, as when it is unplugged.",
    },
    Protocol {
        interface: "ext_session_lock_manager_v1",
        version: 1,
        replayed: "The lock made again, with a lock surface on each screen, which a compositor \
                   started locked takes as its locker coming back. Whether the session is locked \
                   is the holder's word, not the relay's: a locker that does not come back leaves \
                   it locked, with a new locker started.",
    },
    Protocol {
        interface: "wl_compositor",
        version: 6,
        replayed: "Each surface and region made again. A surface's last commit is committed \
                   again: its buffer, from the descriptor held, the whole of it as damaged, its \
                   scale, transform, and opaque and input regions. A frame asked for and not yet \
                   done is asked for again of the new compositor.",
    },
    Protocol {
        interface: "wl_data_device_manager",
        version: 3,
        replayed: "Each data device made again for its seat, and a selection the program set \
                   set again from its source; what it is offered is passed on only if it is not \
                   what it was offered last. A drag in flight cannot be carried: it is \
                   cancelled, as a drop on nothing is.",
    },
    Protocol {
        interface: "wl_output",
        version: 4,
        replayed: "Bound once for each screen. Each made again for the same screen, by its name; \
                   its mode, geometry and scale are passed on only if they changed, and a screen \
                   gone meanwhile is taken away, as when it is unplugged.",
    },
    Protocol {
        interface: "wl_seat",
        version: 9,
        replayed: "The pointer, keyboard and touch made again. The keymap and how keys repeat \
                   are passed on only if they changed. The new compositor's enter on the \
                   surfaces that had the pointer and the keyboard is kept from the program, its \
                   serials standing for the ones the program was given. A key or button held, \
                   or a touch, at that moment cannot be carried: it is let go, as when focus \
                   leaves.",
    },
    Protocol {
        interface: "wl_shm",
        version: 2,
        replayed: "Each pool made again from the descriptor held, at its last size, and each \
                   buffer in it at its offset, size, stride and format. A buffer the old \
                   compositor had not released is released when the new one is done with it.",
    },
    Protocol {
        interface: "wl_subcompositor",
        version: 1,
        replayed: "Each subsurface made again on its parent, at its last position, in its place \
                   among its siblings, and kept in step with its parent or not, as it last was.",
    },
    Protocol {
        interface: "wp_alpha_modifier_v1",
        version: 1,
        replayed: "Each surface's modifier made again, with the last multiplier set.",
    },
    Protocol {
        interface: "wp_cursor_shape_manager_v1",
        version: 2,
        replayed: "Each device made again for its pointer. Told the pointer has left as the \
                   compositor goes and entered again, the program names the shape again, with \
                   the serial the new compositor gave, as it would to any compositor.",
    },
    Protocol {
        interface: "wp_fractional_scale_manager_v1",
        version: 1,
        replayed: "Each surface's fractional scale made again; the scale is passed on only if it \
                   changed.",
    },
    Protocol {
        interface: "wp_presentation",
        version: 2,
        replayed: "Nothing lasting: the clock is said as it is bound, and both compositors use \
                   the monotonic clock. Feedback asked for a frame not yet shown is discarded, \
                   as for a frame that never is.",
    },
    Protocol {
        interface: "wp_security_context_manager_v1",
        version: 1,
        replayed: "The socket a sandbox handed over is the relay's to listen on, and each program \
                   that connects through it is connected to the compositor as one inside that \
                   sandbox, with its engine, app id and instance, so that it is shown no more \
                   than before.",
    },
    Protocol {
        interface: "wp_single_pixel_buffer_manager_v1",
        version: 1,
        replayed: "Each buffer made again with its colour.",
    },
    Protocol {
        interface: "wp_viewporter",
        version: 1,
        replayed: "Each viewport made again, with the source and destination last set.",
    },
    Protocol {
        interface: "xdg_activation_v1",
        version: 1,
        replayed: "A token is the old compositor's and cannot be carried: one used after the new \
                   compositor is up is refused, as a stale one is, and the window it was for is \
                   not brought to the front.",
    },
    Protocol {
        interface: "xdg_toplevel_icon_manager_v1",
        version: 1,
        replayed: "Each window's icon made again from its name and the buffers held, and set \
                   again; the sizes asked for are passed on only if they changed.",
    },
    Protocol {
        interface: "xdg_wm_base",
        version: 6,
        replayed: "Each window made again with its last title, app id, parent and least and \
                   greatest size; the new compositor's first configure is acked by the relay and \
                   passed on only if it differs from the one last acked. A popup cannot be \
                   carried, as its grab was given for input that is past: it is dismissed, as \
                   when the user clicks elsewhere. The compositor's pings are answered by the \
                   program, through the relay, as before.",
    },
    Protocol {
        interface: "xwayland_shell_v1",
        version: 1,
        replayed: "Offered to the X server alone, which ends with the compositor until Smithay \
                   can take up one running (ADR 60), so nothing in it is carried.",
    },
    Protocol {
        interface: "zwlr_layer_shell_v1",
        version: 4,
        replayed: "Each layer surface made again on the same screen, layer and namespace, with \
                   its last size, anchor, margin, exclusive zone and keyboard interactivity; its \
                   configure is passed on only if it differs. One whose screen has gone is \
                   closed, as when it is unplugged, and its popups are as xdg_wm_base's.",
    },
    Protocol {
        interface: "zwp_idle_inhibit_manager_v1",
        version: 1,
        replayed: "Each inhibitor made again on its surface, so that the screen stays on \
                   meanwhile.",
    },
    Protocol {
        interface: "zwp_input_method_manager_v2",
        version: 1,
        replayed: "The input method made again for its seat, with its keyboard grab and popup \
                   surfaces, and told what the text field it was in has, as when one is \
                   entered. Text it was composing at that moment cannot be carried: it is let \
                   go, as when the field loses the keyboard.",
    },
    Protocol {
        interface: "zwp_linux_dmabuf_v1",
        version: 5,
        replayed: "Offered once the backend says which card can be drawn from. Each buffer made \
                   again from the descriptors held, with its planes, size, format and modifier; \
                   the feedback is passed on only if it changed. A buffer the new compositor \
                   cannot take, in a format it no longer offers, cannot be carried: its surface \
                   shows nothing until the program draws again.",
    },
    Protocol {
        interface: "zwp_pointer_constraints_v1",
        version: 1,
        replayed: "Each lock or confinement made again with its region and lifetime. One active \
                   at that moment is said to be inactive and active again, as when focus leaves \
                   and comes back; one that was for once only ends then, as it would.",
    },
    Protocol {
        interface: "zwp_pointer_gestures_v1",
        version: 3,
        replayed: "Each gesture made again for its pointer. One in progress cannot be carried: it \
                   ends as cancelled, as a gesture may.",
    },
    Protocol {
        interface: "zwp_primary_selection_device_manager_v1",
        version: 1,
        replayed: "As the clipboard's: each device made again for its seat, and a selection the \
                   program set set again from its source; what it is offered is passed on only \
                   if it is not what it was offered last.",
    },
    Protocol {
        interface: "zwp_relative_pointer_manager_v1",
        version: 1,
        replayed: "Each relative pointer made again for its pointer; nothing else lasts.",
    },
    Protocol {
        interface: "zwp_text_input_manager_v3",
        version: 1,
        replayed: "Each text input made again for its seat. Told it has been left as the \
                   compositor goes and entered again, the program enables it again with what it \
                   holds, as it would on any enter. The count of commits that the program and \
                   the compositor keep in step is the relay's to keep in step.",
    },
    Protocol {
        interface: "zwp_virtual_keyboard_manager_v1",
        version: 1,
        replayed: "Each virtual keyboard made again with its keymap, from the descriptor held. A \
                   key it held down at that moment cannot be carried: it is let go.",
    },
    Protocol {
        interface: "zxdg_decoration_manager_v1",
        version: 1,
        replayed: "Each window's decoration made again, with the mode it last asked for; the mode \
                   is passed on only if the new compositor chooses another.",
    },
    Protocol {
        interface: "zxdg_exporter_v2",
        version: 1,
        replayed: "Each window exported made again. The handle is the new compositor's: the relay \
                   puts the one the program was given in its place, here and where another \
                   program imports it, so that a handle passed on keeps working.",
    },
    Protocol {
        interface: "zxdg_importer_v2",
        version: 1,
        replayed: "Each window imported made again by its handle, as the exporter's note says, \
                   and its child set again; one whose window has gone is destroyed, as when the \
                   window closes.",
    },
    Protocol {
        interface: "zxdg_output_manager_v1",
        version: 3,
        replayed: "Each screen's description made again; its position, size, name and \
                   description are passed on only if they changed.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_protocol_is_listed_once_in_order_of_its_name() {
        let names: Vec<&str> = OFFERED.iter().map(|protocol| protocol.interface).collect();
        let mut ordered = names.clone();
        ordered.sort_unstable();
        ordered.dedup();
        assert_eq!(names, ordered);
    }

    #[test]
    fn the_x_servers_protocol_is_listed() {
        assert!(
            OFFERED
                .iter()
                .any(|protocol| protocol.interface == X_SERVER)
        );
    }
}
