//! The seat itself, through libseat (ADR 63).
//!
//! Asking a seat for a device needs one, which CI has not, so this file is
//! excluded from the coverage gate (see `docs/adr/0002-coverage-policy.md`).
//! It decides nothing: what is opened, for whom, and when it is closed is
//! worked out in `seating`, which is tested with a seat of its own.

use std::os::fd::OwnedFd;
use std::path::Path;

use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{AsErrno, Event, Session};
use smithay::reexports::calloop::{LoopHandle, RegistrationToken};
use smithay::reexports::rustix::fs::OFlags;
use smithay::reexports::rustix::io::Errno;

use crate::Hold;
use crate::seating::{Seat, Seating};

impl Seat for LibSeatSession {
    fn name(&self) -> String {
        Session::seat(self)
    }

    fn open(&mut self, path: &Path) -> Result<OwnedFd, i32> {
        let flags = OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK;
        Session::open(self, path, flags)
            .map_err(|error| error.as_errno().unwrap_or(Errno::IO.raw_os_error()))
    }

    fn close(&mut self, device: OwnedFd) {
        let _ = Session::close(self, device);
    }

    fn switch(&mut self, terminal: i32) {
        let _ = Session::change_vt(self, terminal);
    }
}

impl Hold {
    /// Takes the seat, to keep while compositors come and go; or nothing,
    /// where there is none to take, and each compositor takes its own.
    #[must_use]
    pub fn seated(mut self) -> Self {
        if let Ok((session, notifier)) = LibSeatSession::new() {
            let active = session.is_active();
            self.seating = Some(Seating::new(Box::new(session), active));
            self.notifier = Some(notifier);
        }
        self
    }
}

/// Has `hold` told, through `handle`, whenever the seat it keeps is paused
/// or active again, once there is a loop to tell it on.
pub(crate) fn watch(hold: &mut Hold, handle: &LoopHandle<'static, Hold>) {
    if let Some(notifier) = hold.notifier.take() {
        let watched = handle.insert_source(notifier, |event, (), hold| {
            hold.seat_changed(matches!(event, Event::ActivateSession));
        });
        hold.watching = watched.ok();
    }
}

/// Stops watching the seat, which closes it.
pub(crate) fn unwatch(watching: Option<RegistrationToken>, handle: &LoopHandle<'static, Hold>) {
    if let Some(token) = watching {
        handle.remove(token);
    }
}
