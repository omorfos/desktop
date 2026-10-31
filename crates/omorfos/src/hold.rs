//! The compositor under `omorfos-hold`, which holds the session while the
//! compositor is started again (ADR 60).
//!
//! The holder hands the compositor the socket programs connect to, and
//! tells it what the compositor before it left: whether the session was
//! locked, which number its X server had, and whether what starts with the
//! session has been started already. The compositor says back, as each
//! changes, what the next one would need to be told.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Instant;

use omorfos_ipc::hold::{Told, Word};

use crate::compositor::Compositor;

/// What the user is told when the compositor is asked to start again with
/// nothing to start it.
const NOT_HELD: &str = "the compositor cannot be started again here: nothing would start \
     it, as omorfos-hold does where omorfos is the whole session";

/// What the user is told when the version of the compositor running is
/// older than the one installed, which would not come up.
const OLDER: &str = "omorfos was updated, but the new version would not start, so the one \
     before it is running: whatever the update was to fix is not fixed yet";

/// What the user is told by a compositor started for its new version, which
/// the one before took up though the session was in use.
const UPDATED: &str = "omorfos has been updated to its new version, which waited as long as it \
     may for a moment the session was not in use";

/// The holder, as the compositor knows it.
#[derive(Debug)]
pub(crate) struct Holder {
    /// The compositor's end of the connection the holder started it with.
    connection: UnixStream,
    told: Told,
    /// What the holder knows, from having been told it or from what it
    /// told.
    said: Said,
    /// Whether the compositor is ending to be started again, and if so
    /// whether as its new version though the session is in use.
    stepping_down: Option<bool>,
}

/// What the holder knows of the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Said {
    up: bool,
    locked: bool,
    display: Option<u32>,
    stepping_down: Option<bool>,
}

impl Said {
    /// The words that tell one who knows `self` what is now so.
    fn words_for(self, now: Self) -> impl Iterator<Item = Word> {
        let up = (now.up && !self.up).then_some(Word::Up);
        let lock = if now.locked {
            Word::Locked
        } else {
            Word::Unlocked
        };
        let locked = (now.locked != self.locked).then_some(lock);
        let display = now.display.filter(|_| now.display != self.display);
        let stepping = now.stepping_down.filter(|_| self.stepping_down.is_none());
        // Said first, for a holder that does not know it to step down still.
        let updated = stepping
            .filter(|regardless| *regardless)
            .map(|_| Word::Updated);
        let stepping = stepping.map(|_| Word::SteppingDown);
        [up, locked, display.map(Word::Display), updated, stepping]
            .into_iter()
            .flatten()
    }
}

impl Compositor {
    /// Takes up what the holder handed over: `connection` to it, what it
    /// was `told`, and the `socket` programs connect to, which it serves.
    pub(crate) fn take_up_holder(
        &mut self,
        connection: UnixStream,
        told: Told,
        socket: &Path,
    ) -> std::io::Result<()> {
        // A socket handed over has a name, which programs are told, and
        // the IPC socket is beside it, named after it.
        let name = socket.file_name().unwrap_or_default();
        let directory = socket.parent().unwrap_or(socket);
        self.set_socket_name(name);
        let ipc = omorfos_ipc::socket_path(None, Some(directory.as_os_str()), Some(name));
        let listened = ipc.into_iter().try_for_each(|path| self.listen_ipc(path));
        listened.map(|()| {
            let said = Said {
                up: false,
                locked: told.locked,
                display: told.display,
                stepping_down: None,
            };
            self.holder = Some(Holder {
                connection,
                told,
                said,
                stepping_down: None,
            });
            if told.older {
                self.problems.push(OLDER.to_owned());
            }
            // A session that was locked is locked before anything of it
            // could be drawn, and the locker started to unlock it: under a
            // relay, once it has handed over what it kept, unless the
            // locker was among it.
            if told.locked {
                self.come_up_locked();
                if !told.relayed {
                    self.start_locker();
                }
            }
        })
    }

    /// Whether this compositor was started after another in the same
    /// login, which started what starts with the session: it is not to
    /// be started twice.
    #[must_use]
    pub fn started_again(&self) -> bool {
        self.holder.as_ref().is_some_and(|holder| holder.told.again)
    }

    /// Whether the compositor is ending to be started again, rather than
    /// ending the session.
    #[must_use]
    pub fn stepping_down(&self) -> bool {
        let holder = self.holder.as_ref();
        holder.is_some_and(|holder| holder.stepping_down.is_some())
    }

    /// The number the holder says the session's X server had, which the
    /// next is to have.
    pub(crate) fn held_display(&self) -> Option<u32> {
        self.holder.as_ref().and_then(|holder| holder.told.display)
    }

    /// Ends the compositor to be started again by the holder, without
    /// asking the programs in the session to end first: they are the
    /// session's, which goes on. With no holder there is nothing to start
    /// it again, and it is refused.
    pub(crate) fn restart_compositor(&mut self) {
        self.step_down(false);
    }

    /// Ends the compositor to be started again by the holder, as its new
    /// version where one is installed, telling it the next is to say so if
    /// that is `regardless` of the session being in use.
    pub(crate) fn step_down(&mut self, regardless: bool) {
        match self.holder.as_mut() {
            Some(holder) => {
                holder.stepping_down = Some(regardless);
                self.running = false;
            }
            None => self.refused.say(NOT_HELD),
        }
    }

    /// Says, as of `now`, that the compositor before took up this new
    /// version though the session was in use, if the holder said so, once.
    pub(crate) fn say_if_updated(&mut self, now: Instant) {
        let holder = self.holder.as_mut();
        if holder.is_some_and(|holder| std::mem::take(&mut holder.told.updated)) {
            self.happened(UPDATED, now);
        }
    }

    /// Tells the holder what has changed since it was last told.
    pub(crate) fn tell_holder(&mut self) {
        let locked = self.is_locked();
        let display = self.x11_number();
        let Some(holder) = self.holder.as_mut() else {
            return;
        };
        let now = Said {
            up: true,
            locked,
            display,
            stepping_down: holder.stepping_down,
        };
        // What cannot be said is not said: a holder that does not read is
        // one that has gone, and the session with it.
        for word in holder.said.words_for(now) {
            let _ = (&holder.connection).write_all(word.line().as_bytes());
        }
        holder.said = now;
    }
}
