//! Recognising key bindings.

/// What a key press meant, according to [`KeyChains::press`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome<'a, A> {
    /// The press is not part of any binding. Deliver it to the focused
    /// window as usual.
    Unbound,
    /// The press continues a chain that is not complete yet. Do not deliver
    /// it; more keys are expected.
    Pending,
    /// The press does not continue the chain that was in progress, which is
    /// now abandoned. Do not deliver it.
    Cancelled,
    /// The press completed a binding. Do not deliver it; perform the action.
    Triggered(&'a A),
}

/// A set of key bindings and the progress through any chain being typed.
///
/// A binding is one or more chords pressed in sequence. The chord type `C`
/// and action type `A` are chosen by the caller; a chord is typically a set
/// of modifiers plus a key.
///
/// Only presses of ordinary keys should be passed in. Pressing a modifier on
/// its own, on the way to the next chord, must not be reported, or it would
/// cancel the chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyChains<C, A> {
    bindings: Vec<(Vec<C>, A)>,
    pressed: Vec<C>,
}

impl<C: PartialEq, A> KeyChains<C, A> {
    /// Creates the bindings. Bindings with no chords are dropped.
    ///
    /// If one binding's chords begin another's, the shorter one triggers and
    /// the longer can never complete.
    #[must_use]
    pub fn new(bindings: impl IntoIterator<Item = (Vec<C>, A)>) -> Self {
        let bindings = bindings
            .into_iter()
            .filter(|(chords, _)| !chords.is_empty())
            .collect();
        Self {
            bindings,
            pressed: Vec::new(),
        }
    }

    /// Records a key press and reports what it meant.
    pub fn press(&mut self, chord: C) -> KeyOutcome<'_, A> {
        self.pressed.push(chord);
        let pressed = &self.pressed;
        if let Some(index) = self
            .bindings
            .iter()
            .position(|(chords, _)| chords == pressed)
        {
            self.pressed.clear();
            return KeyOutcome::Triggered(&self.bindings[index].1);
        }
        if self
            .bindings
            .iter()
            .any(|(chords, _)| chords.starts_with(pressed))
        {
            return KeyOutcome::Pending;
        }
        let was_in_chain = self.pressed.len() > 1;
        self.pressed.clear();
        if was_in_chain {
            KeyOutcome::Cancelled
        } else {
            KeyOutcome::Unbound
        }
    }

    /// Returns `true` while a chain has been started but not completed.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        !self.pressed.is_empty()
    }

    /// Abandons any chain in progress, for example when focus is lost.
    pub fn cancel(&mut self) {
        self.pressed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Action {
        Terminal,
        Close,
        Quit,
    }

    fn chains() -> KeyChains<&'static str, Action> {
        KeyChains::new([
            (vec!["Super+Return"], Action::Terminal),
            (vec!["Super+x", "c"], Action::Close),
            (vec!["Super+x", "Super+x", "q"], Action::Quit),
            (vec![], Action::Quit),
        ])
    }

    #[test]
    fn a_single_chord_binding_triggers_immediately() {
        let mut keys = chains();
        assert_eq!(
            keys.press("Super+Return"),
            KeyOutcome::Triggered(&Action::Terminal)
        );
        assert!(!keys.is_pending());
    }

    #[test]
    fn an_unbound_key_is_left_for_the_application() {
        let mut keys = chains();
        assert_eq!(keys.press("a"), KeyOutcome::Unbound);
        assert!(!keys.is_pending());
    }

    #[test]
    fn a_chain_is_pending_until_its_last_chord() {
        let mut keys = chains();
        assert_eq!(keys.press("Super+x"), KeyOutcome::Pending);
        assert!(keys.is_pending());
        assert_eq!(keys.press("c"), KeyOutcome::Triggered(&Action::Close));
        assert!(!keys.is_pending());
    }

    #[test]
    fn chains_sharing_a_prefix_are_told_apart_by_later_chords() {
        let mut keys = chains();
        assert_eq!(keys.press("Super+x"), KeyOutcome::Pending);
        assert_eq!(keys.press("Super+x"), KeyOutcome::Pending);
        assert_eq!(keys.press("q"), KeyOutcome::Triggered(&Action::Quit));
    }

    #[test]
    fn a_wrong_key_cancels_the_chain_and_is_swallowed() {
        let mut keys = chains();
        assert_eq!(keys.press("Super+x"), KeyOutcome::Pending);
        assert_eq!(keys.press("z"), KeyOutcome::Cancelled);
        assert!(!keys.is_pending());
        assert_eq!(
            keys.press("c"),
            KeyOutcome::Unbound,
            "the chain started over"
        );
    }

    #[test]
    fn a_bound_key_pressed_mid_chain_cancels_rather_than_triggers() {
        let mut keys = chains();
        assert_eq!(keys.press("Super+x"), KeyOutcome::Pending);
        assert_eq!(keys.press("Super+Return"), KeyOutcome::Cancelled);
    }

    #[test]
    fn cancel_abandons_a_chain_in_progress() {
        let mut keys = chains();
        assert_eq!(keys.press("Super+x"), KeyOutcome::Pending);
        keys.cancel();
        assert!(!keys.is_pending());
        assert_eq!(keys.press("c"), KeyOutcome::Unbound);
    }

    #[test]
    fn bindings_without_chords_never_trigger() {
        let mut keys: KeyChains<&str, Action> = KeyChains::new([(vec![], Action::Quit)]);
        assert_eq!(keys.press("q"), KeyOutcome::Unbound);
    }

    #[test]
    fn a_shorter_binding_shadows_a_longer_one_with_the_same_start() {
        let mut keys = KeyChains::new([(vec!["a", "b"], 1), (vec!["a"], 2)]);
        assert_eq!(keys.press("a"), KeyOutcome::Triggered(&2));
        assert_eq!(keys.press("b"), KeyOutcome::Unbound);
    }
}
