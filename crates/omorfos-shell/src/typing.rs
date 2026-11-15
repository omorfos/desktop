//! What the keys the compositor reports type, for a field of the shell's:
//! the prompt that asks for a secret, the lock screen, and the launcher.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::time::{Duration, Instant};

use omorfos_ui::field::{Reach, Way};
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
    /// Backspace: takes out the character before the cursor; with Ctrl or
    /// Alt, the word.
    Back(Reach),
    /// Delete: takes out the character after the cursor, or with Ctrl or
    /// Alt the word.
    Delete(Reach),
    /// The arrows left and right, and Home and End: the cursor moved, by a
    /// word with Ctrl or Alt, Home and End to an end, choosing
    /// what it goes over with Shift.
    Move {
        /// Which way.
        way: Way,
        /// How far.
        reach: Reach,
        /// Whether Shift is held, choosing what it goes over.
        choosing: bool,
    },
    /// Ctrl and Z, or Super and Z: the last change undone.
    Undo,
    /// Ctrl, Shift and Z, or Ctrl and Y: the last change undone done again.
    Redo,
    /// Ctrl and C, or Super and C: what is chosen copied.
    Copy,
    /// Ctrl and X, or Super and X: what is chosen copied and taken out.
    Cut,
    /// Ctrl and V, or Super and V: what is on the clipboard put in place of
    /// what is chosen.
    Paste,
    /// Tab: on to what comes next, as a button.
    Next,
    /// Shift and Tab: back to what came before.
    Previous,
    /// The arrow up: what is above, in a list.
    Up,
    /// The arrow down: what is below, in a list.
    Down,
    /// Ctrl and A, or Super and A as on a Mac: all that is typed chosen,
    /// for what is typed next to take its place.
    All,
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

    /// Whether the key the compositor numbers `code` repeats while it is
    /// held down, as the layout says: a letter does, Shift does not.
    #[must_use]
    pub fn repeats(&self, code: u32) -> bool {
        let code = xkb::Keycode::new(code.saturating_add(8));
        let keymap = self.state.as_ref().map(xkb::State::get_keymap);
        keymap.is_some_and(|keymap| keymap.key_repeats(code))
    }

    /// Whether the keyboard layout is known, so that keys type something.
    #[must_use]
    pub const fn known(&self) -> bool {
        self.state.is_some()
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
    let (ctrl, alt, logo) = (
        held(xkb::MOD_NAME_CTRL),
        held(xkb::MOD_NAME_ALT),
        held(xkb::MOD_NAME_LOGO),
    );
    let command = ctrl || logo;
    let chord = command || alt;
    let sym = state.key_get_one_sym(code);
    // A shortcut is the key's, whatever letter the layout typed with puts
    // on it, as programs take it.
    let shortcut = command.then(|| latin(state, code)).flatten().unwrap_or(sym);
    let letter = (command && !alt).then(|| u8::try_from(shortcut.raw()).ok());
    let letter = letter.flatten().map(|letter| letter.to_ascii_lowercase());
    let shift = held(xkb::MOD_NAME_SHIFT);
    let found = letter.and_then(|letter| match (letter, shift) {
        (b'a', _) => Some(Key::All),
        (b'z', false) => Some(Key::Undo),
        (b'z', true) | (b'y', false) => Some(Key::Redo),
        (b'c', false) => Some(Key::Copy),
        (b'x', false) => Some(Key::Cut),
        (b'v', false) => Some(Key::Paste),
        _ => None,
    });
    if let Some(found) = found {
        return found;
    }
    // By a word with Ctrl or Alt; and with Super, as Ctrl, which is how
    // the compositor hands Super on to the shell's fields (ADR 78), so
    // that it is the same whichever reaches a field.
    let reach = if command || alt {
        Reach::Word
    } else {
        Reach::Character
    };
    let moved = |way, reach| Key::Move {
        way,
        reach,
        choosing: shift,
    };
    match sym {
        xkb::Keysym::Return | xkb::Keysym::KP_Enter => Key::Enter,
        xkb::Keysym::Escape => Key::Escape,
        xkb::Keysym::BackSpace => Key::Back(reach),
        xkb::Keysym::Delete | xkb::Keysym::KP_Delete => Key::Delete(reach),
        xkb::Keysym::Left | xkb::Keysym::KP_Left => moved(Way::Back, reach),
        xkb::Keysym::Right | xkb::Keysym::KP_Right => moved(Way::On, reach),
        xkb::Keysym::Home | xkb::Keysym::KP_Home => moved(Way::Back, Reach::Line),
        xkb::Keysym::End | xkb::Keysym::KP_End => moved(Way::On, Reach::Line),
        xkb::Keysym::Tab => Key::Next,
        xkb::Keysym::ISO_Left_Tab => Key::Previous,
        xkb::Keysym::Up => Key::Up,
        xkb::Keysym::Down => Key::Down,
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

/// What the key numbered `code` types in the first of the keyboard's
/// layouts where it types a Latin letter, if one does: in Greek or Russian,
/// with a Latin layout beside it, the key that is A in Latin is A to a
/// shortcut, as programs commonly take it.
fn latin(state: &xkb::State, code: xkb::Keycode) -> Option<xkb::Keysym> {
    let keymap = state.get_keymap();
    let layouts = 0..keymap.num_layouts_for_key(code);
    layouts.into_iter().find_map(|layout| {
        let syms = keymap.key_get_syms_by_level(code, layout, 0);
        let first = syms.first().copied();
        first.filter(|sym| u8::try_from(sym.raw()).is_ok_and(|byte| byte.is_ascii_alphabetic()))
    })
}

/// A key held down, done again and again as the compositor says keys
/// repeat: once it has been held for a while, so many times a second.
#[derive(Debug, Default)]
pub struct Repeat {
    /// How long before a key held is done again, and how long between each
    /// time after; nothing until the compositor has said, or if it says
    /// that keys do not repeat.
    pace: Option<(Duration, Duration)>,
    /// The key held, when it is next to be done again, and how long after
    /// that the next time is.
    held: Option<(u32, Instant, Duration)>,
}

impl Repeat {
    /// Takes in how keys repeat, as the compositor says: `rate` times a
    /// second, the first `delay` milliseconds after the key went down. A
    /// rate of 0 is that they do not.
    pub fn learn(&mut self, (rate, delay): (i32, i32)) {
        let every = u32::try_from(rate).ok().filter(|rate| *rate > 0);
        let every = every.map(|rate| Duration::from_secs(1) / rate);
        let delay = Duration::from_millis(u64::try_from(delay).unwrap_or(0));
        self.pace = every.map(|every| (delay, every));
        if self.pace.is_none() {
            self.held = None;
        }
    }

    /// Notes the key `code` going down `now`: held, it is to be done again,
    /// if it `repeats`. Any other key held stops being done again.
    pub fn pressed(&mut self, code: u32, repeats: bool, now: Instant) {
        let pace = self.pace.filter(|_| repeats);
        self.held = pace.map(|(delay, every)| (code, now + delay, every));
    }

    /// Notes the key `code` going up.
    pub fn released(&mut self, code: u32) {
        self.held = self.held.filter(|(held, ..)| *held != code);
    }

    /// Stops the key held being done again, as when the keyboard has gone
    /// elsewhere.
    pub const fn stop(&mut self) {
        self.held = None;
    }

    /// The key held, if it is due to be done again by `now`: once, however
    /// long it has been, so that a program that was kept waiting does not
    /// then do it many times at once.
    pub fn due(&mut self, now: Instant) -> Option<u32> {
        let (code, _, every) = self.held.filter(|(_, next, _)| *next <= now)?;
        self.held = Some((code, now + every, every));
        Some(code)
    }

    /// When the key held is next to be done again, if one is.
    #[must_use]
    pub fn deadline(&self) -> Option<Instant> {
        self.held.map(|(_, next, _)| next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key that is A on a Latin keyboard, as the compositor numbers it.
    const A: u32 = 30;

    /// A keyboard with `layouts`, typing with the one at `group`, and
    /// Ctrl held or not.
    fn keyboard(layouts: &str, group: u32, ctrl: bool) -> xkb::State {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let flags = xkb::KEYMAP_COMPILE_NO_FLAGS;
        let keymap = xkb::Keymap::new_from_names(&context, "", "", layouts, "", None, flags);
        let mut state = xkb::State::new(&keymap.unwrap());
        let ctrl_mask = 1 << state.get_keymap().mod_get_index(xkb::MOD_NAME_CTRL);
        let depressed = if ctrl { ctrl_mask } else { 0 };
        state.update_mask(depressed, 0, 0, 0, 0, group);
        state
    }

    #[test]
    fn ctrl_and_the_key_that_is_a_chooses_all_whatever_the_layout_types_on_it() {
        assert_eq!(typed(&keyboard("us", 0, true), A), Key::All);
        assert_eq!(typed(&keyboard("us,gr", 1, true), A), Key::All, "in Greek");
        assert_eq!(
            typed(&keyboard("us,ru", 1, true), A),
            Key::All,
            "in Russian"
        );
        assert_eq!(
            typed(&keyboard("us,gr", 1, false), A),
            Key::Typed("α".to_owned())
        );
    }

    #[test]
    fn the_arrows_go_up_and_down_a_list() {
        // As the compositor numbers them.
        let (up, down) = (103, 108);
        assert_eq!(typed(&keyboard("us", 0, false), up), Key::Up);
        assert_eq!(typed(&keyboard("us", 0, false), down), Key::Down);
    }

    /// A US keyboard with the modifiers named in `held` held down.
    fn holding(held: &[&str]) -> xkb::State {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let flags = xkb::KEYMAP_COMPILE_NO_FLAGS;
        let keymap = xkb::Keymap::new_from_names(&context, "", "", "us", "", None, flags);
        let mut state = xkb::State::new(&keymap.unwrap());
        let keymap = state.get_keymap();
        let mask = held.iter().map(|name| 1 << keymap.mod_get_index(name));
        state.update_mask(mask.sum(), 0, 0, 0, 0, 0);
        state
    }

    #[test]
    fn the_keys_that_edit_a_line_go_by_a_character_a_word_or_to_an_end() {
        // As the compositor numbers them.
        let (left, right, home, end, delete, backspace) = (105, 106, 102, 107, 111, 14);
        let moved = |way, reach, choosing| Key::Move {
            way,
            reach,
            choosing,
        };
        let plain = holding(&[]);
        assert_eq!(
            typed(&plain, left),
            moved(Way::Back, Reach::Character, false)
        );
        assert_eq!(
            typed(&plain, right),
            moved(Way::On, Reach::Character, false)
        );
        assert_eq!(typed(&plain, home), moved(Way::Back, Reach::Line, false));
        assert_eq!(typed(&plain, end), moved(Way::On, Reach::Line, false));
        assert_eq!(typed(&plain, backspace), Key::Back(Reach::Character));
        assert_eq!(typed(&plain, delete), Key::Delete(Reach::Character));
        // By a word with Ctrl, as GTK has it, or Alt, as a Mac has it.
        // And with Super, as Ctrl, which is how the compositor hands it on.
        for word in [xkb::MOD_NAME_CTRL, xkb::MOD_NAME_ALT, xkb::MOD_NAME_LOGO] {
            let held = holding(&[word]);
            assert_eq!(typed(&held, left), moved(Way::Back, Reach::Word, false));
            assert_eq!(typed(&held, backspace), Key::Back(Reach::Word));
            assert_eq!(typed(&held, delete), Key::Delete(Reach::Word));
        }
        // Shift chooses what the cursor goes over.
        let shift = holding(&[xkb::MOD_NAME_SHIFT, xkb::MOD_NAME_CTRL]);
        assert_eq!(typed(&shift, right), moved(Way::On, Reach::Word, true));
        assert_eq!(
            typed(&holding(&[xkb::MOD_NAME_SHIFT]), home),
            moved(Way::Back, Reach::Line, true)
        );
    }

    #[test]
    fn ctrl_or_super_with_z_y_c_x_or_v_undoes_does_again_copies_cuts_or_pastes() {
        let (key_z, key_y, key_c, key_x, key_b, key_v) = (44, 21, 46, 45, 48, 47);
        for command in [xkb::MOD_NAME_CTRL, xkb::MOD_NAME_LOGO] {
            let held = holding(&[command]);
            assert_eq!(typed(&held, key_z), Key::Undo);
            assert_eq!(typed(&held, key_y), Key::Redo);
            assert_eq!(typed(&held, key_c), Key::Copy);
            assert_eq!(typed(&held, key_x), Key::Cut);
            assert_eq!(typed(&held, key_v), Key::Paste);
            assert_eq!(typed(&held, key_b), Key::Nothing, "no shortcut");
            let shifted = holding(&[command, xkb::MOD_NAME_SHIFT]);
            assert_eq!(typed(&shifted, key_z), Key::Redo);
            assert_eq!(typed(&shifted, key_c), Key::Nothing, "not with Shift");
        }
        // With Alt as well, none of them: that is another shortcut.
        let both = holding(&[xkb::MOD_NAME_CTRL, xkb::MOD_NAME_ALT]);
        assert_eq!(typed(&both, key_z), Key::Nothing);
        // Alone, they type.
        assert_eq!(typed(&holding(&[]), key_z), Key::Typed("z".to_owned()));
    }

    #[test]
    fn with_no_latin_layout_beside_it_there_is_no_a_to_choose_all_by() {
        assert_eq!(typed(&keyboard("gr", 0, true), A), Key::Nothing);
    }

    #[test]
    fn a_key_held_is_done_again_after_a_while_and_then_so_often() {
        let start = Instant::now();
        let at = |millis| start + Duration::from_millis(millis);
        let mut repeat = Repeat::default();
        // Until the compositor says, no key repeats.
        repeat.pressed(14, true, start);
        assert_eq!(repeat.deadline(), None);
        repeat.learn((25, 400));
        repeat.pressed(14, true, start);
        assert_eq!(repeat.due(at(399)), None);
        assert_eq!(repeat.due(at(400)), Some(14));
        assert_eq!(repeat.deadline(), Some(at(440)));
        // Late, it is done once, and the next counted from then.
        assert_eq!(repeat.due(at(1000)), Some(14));
        assert_eq!(repeat.due(at(1000)), None);
        assert_eq!(repeat.deadline(), Some(at(1040)));
        // Another key going up leaves it; its own stops it.
        repeat.released(30);
        assert!(repeat.deadline().is_some());
        repeat.released(14);
        assert_eq!(repeat.deadline(), None);
    }

    #[test]
    fn what_does_not_repeat_is_done_once() {
        let now = Instant::now();
        let mut repeat = Repeat::default();
        repeat.learn((25, 400));
        repeat.pressed(42, false, now);
        assert_eq!(repeat.deadline(), None, "a key that does not repeat");
        repeat.pressed(14, true, now);
        repeat.stop();
        assert_eq!(repeat.deadline(), None, "stopped");
        // A rate of 0 is that keys do not repeat, and stops one held.
        repeat.pressed(14, true, now);
        repeat.learn((0, 400));
        assert_eq!(repeat.deadline(), None);
        repeat.learn((-1, -1));
        repeat.pressed(14, true, now);
        assert_eq!(repeat.due(now + Duration::from_secs(9)), None);
        assert!(format!("{repeat:?}").contains("held: None"));
    }

    #[test]
    fn before_the_layout_is_known_no_key_types_anything() {
        let typing = Typing::default();
        assert!(!typing.known());
        assert_eq!(typing.pressed(30), Key::Nothing);
        assert!(!typing.repeats(30));
        assert!(format!("{typing:?}").contains("known: false"));
    }
}
