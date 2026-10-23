//! What the keys the compositor reports type, for a field that a secret
//! is typed into: the prompt that asks for one, and the lock screen.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;

use xkbcommon::xkb;

/// The largest keyboard layout read, in bytes. One is some tens of
/// kilobytes.
const LARGEST_KEYMAP: u32 = 1024 * 1024;

/// What a key going down does to a field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// Enter: goes on with what is typed.
    Enter,
    /// Escape: gives up, or clears the field.
    Escape,
    /// Backspace: takes the last character out.
    Back,
    /// Types this.
    Typed(String),
    /// Nothing: a key held with Ctrl, Alt or Super, or one that types
    /// nothing that can be shown, or before the layout is known.
    Nothing,
}

/// The keyboard as it is, to know what its keys type.
pub struct Typing {
    layouts: xkb::Context,
    state: Option<xkb::State>,
}

impl Default for Typing {
    fn default() -> Self {
        Self {
            layouts: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            state: None,
        }
    }
}

impl std::fmt::Debug for Typing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Typing")
            .field("known", &self.state.is_some())
            .finish_non_exhaustive()
    }
}

impl Typing {
    /// Takes in the keyboard layout the compositor sent, `size` bytes of
    /// it in `layout`.
    pub fn learn_layout(&mut self, layout: OwnedFd, size: u32) {
        let length = usize::try_from(size.min(LARGEST_KEYMAP)).unwrap_or(0);
        let mut bytes = vec![0; length];
        let read = File::from(layout).read_exact_at(&mut bytes, 0);
        let text = read.ok().and_then(|()| String::from_utf8(bytes).ok());
        let keymap = text.and_then(|text| {
            let text = text.trim_end_matches('\0').to_owned();
            let (format, flags) = (xkb::KEYMAP_FORMAT_TEXT_V1, xkb::KEYMAP_COMPILE_NO_FLAGS);
            xkb::Keymap::new_from_string(&self.layouts, text, format, flags)
        });
        self.state = keymap.as_ref().map(xkb::State::new);
    }

    /// Takes in which modifiers are held, latched and locked, and which
    /// layout is typed with, as the compositor says.
    pub fn modifiers(&mut self, (depressed, latched, locked, group): (u32, u32, u32, u32)) {
        // The layout is always said first.
        let state = self.state.as_mut();
        let _ = state.map(|state| state.update_mask(depressed, latched, locked, 0, 0, group));
    }

    /// What the key the compositor numbers `code` does, going down.
    #[must_use]
    pub fn pressed(&self, code: u32) -> Key {
        // Before the layout is known, no key types anything.
        self.state
            .as_ref()
            .map_or(Key::Nothing, |state| typed(state, code))
    }
}

/// What the key the compositor numbers `code` does on the keyboard as it is
/// in `state`.
fn typed(state: &xkb::State, code: u32) -> Key {
    // The numbers the layout knows keys by are eight higher.
    let code = xkb::Keycode::new(code.saturating_add(8));
    let held = |name| state.mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE);
    let chord = held(xkb::MOD_NAME_CTRL) || held(xkb::MOD_NAME_ALT) || held(xkb::MOD_NAME_LOGO);
    match state.key_get_one_sym(code) {
        xkb::Keysym::Return | xkb::Keysym::KP_Enter => Key::Enter,
        xkb::Keysym::Escape => Key::Escape,
        xkb::Keysym::BackSpace => Key::Back,
        _ => {
            let typed = state.key_get_utf8(code);
            let printable = typed.chars().all(|character| !character.is_control());
            if chord || !printable || typed.is_empty() {
                Key::Nothing
            } else {
                Key::Typed(typed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_the_layout_is_known_no_key_types_anything() {
        let typing = Typing::default();
        assert_eq!(typing.pressed(30), Key::Nothing);
        assert!(format!("{typing:?}").contains("known: false"));
    }
}
