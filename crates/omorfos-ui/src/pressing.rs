//! What the pointer's button went down on, held until it is let go
//! (ADR 113): what was pressed acts as the button is let go over it, is
//! drawn pressed while it is held and the pointer is on it, and does
//! nothing let go anywhere else.
//!
//! It is a model and no more: each place that takes a press feeds it what
//! is under the pointer, by a key of its own, draws `held` as `:active`
//! and acts on what `release` gives.

/// A press held on something of `K`, until its button is let go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pressing<K> {
    /// What the press went down on, and whether the pointer is on it still.
    down: Option<(K, bool)>,
}

impl<K> Default for Pressing<K> {
    fn default() -> Self {
        Self { down: None }
    }
}

impl<K: PartialEq + Copy> Pressing<K> {
    /// Nothing pressed.
    #[must_use]
    pub const fn new() -> Self {
        Self { down: None }
    }

    /// The button went down `on` what is under the pointer, if anything.
    pub fn press(&mut self, on: Option<K>) {
        self.down = on.map(|on| (on, true));
    }

    /// The pointer moved, and is `on` what is under it now: what was
    /// pressed is pressed again if it is that, and not otherwise.
    pub fn moved(&mut self, on: Option<K>) {
        if let Some((down, over)) = self.down.as_mut() {
            *over = on == Some(*down);
        }
    }

    /// The button was let go `on` what is under the pointer: what it went
    /// down on, if it is that, and nothing otherwise.
    pub fn release(&mut self, on: Option<K>) -> Option<K> {
        let (down, _) = self.down.take()?;
        on.filter(|on| *on == down)
    }

    /// What is drawn as pressed: what the button went down on, while it
    /// is held and the pointer is on it.
    #[must_use]
    pub fn held(&self) -> Option<K> {
        self.down.filter(|(_, over)| *over).map(|(down, _)| down)
    }

    /// What was pressed has gone, as a notification whose time ran out:
    /// letting go acts on nothing.
    pub fn forget(&mut self) {
        self.down = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_press_let_go_on_what_it_went_down_on_acts() {
        let mut pressing = Pressing::new();
        pressing.press(Some(3));
        assert_eq!(pressing.held(), Some(3));
        assert_eq!(pressing.release(Some(3)), Some(3));
        assert_eq!(pressing.held(), None, "let go");
    }

    #[test]
    fn let_go_elsewhere_it_does_nothing() {
        let mut pressing = Pressing::new();
        pressing.press(Some(3));
        assert_eq!(pressing.release(Some(4)), None);
        pressing.press(Some(3));
        assert_eq!(pressing.release(None), None);
    }

    #[test]
    fn moved_off_it_is_not_held_and_back_on_it_is_again() {
        let mut pressing = Pressing::new();
        pressing.press(Some(3));
        pressing.moved(Some(4));
        assert_eq!(pressing.held(), None, "off it");
        pressing.moved(None);
        assert_eq!(pressing.held(), None);
        pressing.moved(Some(3));
        assert_eq!(pressing.held(), Some(3), "back on it");
        assert_eq!(pressing.release(Some(3)), Some(3));
    }

    #[test]
    fn a_press_on_nothing_holds_nothing() {
        let mut pressing = Pressing::<u8>::default();
        pressing.press(None);
        pressing.moved(Some(3));
        assert_eq!(pressing.held(), None);
        assert_eq!(pressing.release(Some(3)), None);
    }

    #[test]
    fn a_release_with_no_press_acts_on_nothing() {
        let mut pressing = Pressing::new();
        pressing.moved(Some(3));
        assert_eq!(pressing.release(Some(3)), None);
    }

    #[test]
    fn forgotten_it_acts_on_nothing() {
        let mut pressing = Pressing::new();
        pressing.press(Some(3));
        pressing.forget();
        assert_eq!(pressing.held(), None);
        assert_eq!(pressing.release(Some(3)), None);
    }
}
