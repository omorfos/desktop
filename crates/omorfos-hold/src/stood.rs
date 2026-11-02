//! How long the session stood still between one compositor and the next,
//! and where that went (ADR 62).

use std::time::Instant;

/// The moments of a change of compositor, as the holder sees them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Stood {
    /// When the compositor before said it is stepping down, or ended.
    since: Instant,
    /// When it ended, closing its connection to the holder.
    ended: Option<Instant>,
    /// When what was opened for it was closed.
    closed: Option<Instant>,
    /// When its process was gone.
    gone: Option<Instant>,
    /// When the next was started, the last time it was.
    started: Option<Instant>,
    /// When the next said it is up, serving the socket.
    up: Option<Instant>,
}

impl Stood {
    /// A change that began `at`.
    pub(crate) const fn since(at: Instant) -> Self {
        Self {
            since: at,
            ended: None,
            closed: None,
            gone: None,
            started: None,
            up: None,
        }
    }

    /// Takes in that the compositor before ended `at`, if that is not known
    /// already.
    pub(crate) fn ended(&mut self, at: Instant) {
        self.ended = self.ended.or(Some(at));
    }

    /// Takes in that what was opened for the compositor before was closed
    /// `at`.
    pub(crate) const fn closed(&mut self, at: Instant) {
        self.closed = Some(at);
    }

    /// Takes in that the process of the compositor before was gone `at`.
    pub(crate) const fn gone(&mut self, at: Instant) {
        self.gone = Some(at);
    }

    /// Takes in that the next was started `at`.
    pub(crate) const fn started(&mut self, at: Instant) {
        self.started = Some(at);
    }

    /// Takes in that the next said it is up `at`, if it has not before.
    pub(crate) fn up(&mut self, at: Instant) {
        self.up = self.up.or(Some(at));
    }

    /// What the log is told, as the next shows the session `at`: how long
    /// the session stood still, and how much of that went to each part of
    /// the change. A part not seen is taken to have taken no time.
    pub(crate) fn said(&self, at: Instant) -> String {
        let ended = self.ended.unwrap_or(self.since);
        let closed = self.closed.unwrap_or(ended).max(ended);
        let gone = self.gone.unwrap_or(closed).max(closed);
        let started = self.started.unwrap_or(gone).max(gone);
        let up = self.up.unwrap_or(started).max(started);
        let ms = |from: Instant, to: Instant| to.saturating_duration_since(from).as_millis();
        format!(
            "omorfos-hold: the session stood still for {} ms between one compositor and the \
             next: {} ms for the one before to end, {} ms to close what it had, {} ms for its \
             process to be gone, {} ms to start the next, {} ms for it to come up, and {} ms \
             for it to show the session",
            ms(self.since, at),
            ms(self.since, ended),
            ms(ended, closed),
            ms(closed, gone),
            ms(gone, started),
            ms(started, up),
            ms(up, at)
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn where_the_time_a_change_stood_still_went_is_said_part_by_part() {
        let at = Instant::now();
        let after = |ms| at + Duration::from_millis(ms);
        let mut stood = Stood::since(at);
        stood.ended(after(300));
        stood.ended(after(310));
        stood.closed(after(340));
        stood.gone(after(390));
        stood.started(after(385));
        stood.started(after(400));
        stood.up(after(500));
        stood.up(after(510));
        assert_eq!(
            stood.said(after(700)),
            "omorfos-hold: the session stood still for 700 ms between one compositor and the \
             next: 300 ms for the one before to end, 40 ms to close what it had, 50 ms for its \
             process to be gone, 10 ms to start the next, 100 ms for it to come up, and 200 ms \
             for it to show the session"
        );
    }

    #[test]
    fn a_part_not_seen_took_no_time() {
        let at = Instant::now();
        let stood = Stood::since(at);
        assert_eq!(
            stood.said(at + Duration::from_millis(40)),
            "omorfos-hold: the session stood still for 40 ms between one compositor and the \
             next: 0 ms for the one before to end, 0 ms to close what it had, 0 ms for its \
             process to be gone, 0 ms to start the next, 0 ms for it to come up, and 40 ms for \
             it to show the session"
        );
    }
}
