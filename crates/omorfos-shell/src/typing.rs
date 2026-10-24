//! What the keys the compositor reports type, for a field that a secret
//! is typed into: the prompt that asks for one, and the lock screen.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileExt;
use std::time::{Duration, Instant};

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
    /// Tab: on to what comes next, as a button.
    Next,
    /// Shift and Tab: back to what came before.
    Previous,
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
    let command = held(xkb::MOD_NAME_CTRL) || held(xkb::MOD_NAME_LOGO);
    let chord = command || held(xkb::MOD_NAME_ALT);
    match state.key_get_one_sym(code) {
        xkb::Keysym::a | xkb::Keysym::A if command && !held(xkb::MOD_NAME_ALT) => Key::All,
        xkb::Keysym::Return | xkb::Keysym::KP_Enter => Key::Enter,
        xkb::Keysym::Escape => Key::Escape,
        xkb::Keysym::BackSpace => Key::Back,
        xkb::Keysym::Tab => Key::Next,
        xkb::Keysym::ISO_Left_Tab => Key::Previous,
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
        assert_eq!(typing.pressed(30), Key::Nothing);
        assert!(!typing.repeats(30));
        assert!(format!("{typing:?}").contains("known: false"));
    }
}
