//! Programs asking for the session to be kept awake over D-Bus, rather
//! than by Wayland's `idle-inhibit`: by `org.freedesktop.ScreenSaver`, as
//! Chromium, programs built on Electron, VLC and programs written for X11
//! ask while they play; and by the portal's `Inhibit`, as programs in a
//! sandbox ask.
//!
//! What is held is told to the compositor, which keeps the session from
//! going idle or the machine from sleeping while it is (see `docs/IPC.md`,
//! `hold`). A hold lasts until the program that asked lets it go, or
//! leaves the bus, as one that ends or crashes does: nothing is held for a
//! program that is not there. A program can hold only so much, so that one
//! that asks without end cannot make this keep more and more.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::message::Message;
use zbus::zvariant::{OwnedObjectPath, OwnedValue};

/// The interface the screensaver of a desktop is spoken to by.
pub const SCREENSAVER: &str = "org.freedesktop.ScreenSaver";

/// The interface the portal passes what keeps the session awake by.
pub const INHIBIT: &str = "org.freedesktop.impl.portal.Inhibit";

/// The interface by which the portal says it no longer wants what it
/// asked for.
const REQUEST: &str = "org.freedesktop.impl.portal.Request";

/// What keeps the machine from sleeping and the session from going idle,
/// of the portal's numbers for what may be held off.
const SUSPEND: u32 = 4;
const IDLE: u32 = 8;

/// The most that one program may hold at once, and that all may.
const EACH: usize = 64;
const ALL: usize = 1024;

/// Says what is held whenever that changes: whether the session is kept
/// from going idle, and whether the machine from sleeping.
pub type Keep = Box<dyn FnMut(bool, bool) + Send>;

/// How a hold is known to the one that asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum By {
    /// A number given back, as the screensaver's interface does.
    Cookie(u32),
    /// The object the portal named for the request.
    Request(OwnedObjectPath),
}

/// One program's hold.
#[derive(Debug)]
struct Hold {
    /// The program on the bus that asked, by its own name there.
    who: String,
    by: By,
    idle: bool,
    sleep: bool,
}

/// What is held, and who is told.
pub(crate) struct Holds {
    held: Vec<Hold>,
    /// The number given to the next hold of the screensaver's.
    next: u32,
    /// What was last told.
    told: (bool, bool),
    keep: Keep,
}

impl Holds {
    /// Nothing held, and `keep` told whenever that changes.
    pub(crate) fn new(keep: Keep) -> Self {
        Self {
            held: Vec::new(),
            next: 1,
            told: (false, false),
            keep,
        }
    }

    /// Takes a hold, if the one that asks may have another.
    fn take(&mut self, hold: Hold) -> bool {
        let theirs = self.held.iter().filter(|held| held.who == hold.who).count();
        let room = theirs < EACH && self.held.len() < ALL;
        if room {
            self.held.push(hold);
            self.tell();
        }
        room
    }

    /// Lets go of what `who` holds and knows `by`, if it holds it.
    fn release(&mut self, who: &str, by: &By) -> bool {
        let before = self.held.len();
        self.held
            .retain(|held| !(held.who == who && held.by == *by));
        self.tell();
        self.held.len() < before
    }

    /// Lets go of everything `who` held, now that it has left the bus.
    pub(crate) fn gone(&mut self, who: &str) {
        self.held.retain(|held| held.who != who);
        self.tell();
    }

    /// Tells what is held, if it has changed since it was last told.
    fn tell(&mut self) {
        let idle = self.held.iter().any(|held| held.idle);
        let sleep = self.held.iter().any(|held| held.sleep);
        if (idle, sleep) != self.told {
            self.told = (idle, sleep);
            (self.keep)(idle, sleep);
        }
    }
}

/// What the portal sends to keep the session awake: the object the
/// request is known by, the application, the window it asks from, what is
/// to be held off, and how.
type Asked = (
    OwnedObjectPath,
    String,
    String,
    u32,
    HashMap<String, OwnedValue>,
);

/// Answers a call about keeping the session awake, if it is one, and says
/// how sending the answer went; a call that is not one is left alone.
pub(crate) fn answer(
    connection: &Connection,
    message: &Message,
    called: (&str, Option<&str>),
    holds: &mut Holds,
) -> Option<zbus::Result<()>> {
    let header = message.header();
    let body = message.body();
    let who = header.sender().map(ToString::to_string).unwrap_or_default();
    let refuse = |name: &str, why: &str| connection.reply_error(&header, name, &why);
    let malformed = || {
        let name = "org.freedesktop.DBus.Error.InvalidArgs";
        refuse(name, "the arguments are not those of this method")
    };
    let full = || {
        let name = "org.freedesktop.DBus.Error.LimitsExceeded";
        refuse(name, "too much is held already")
    };
    let sent = match called {
        (SCREENSAVER, Some("Inhibit")) => match body.deserialize::<(String, String)>() {
            Ok(_) => {
                let cookie = holds.next;
                holds.next = holds.next.wrapping_add(1).max(1);
                let hold = Hold {
                    who,
                    by: By::Cookie(cookie),
                    idle: true,
                    sleep: false,
                };
                if holds.take(hold) {
                    connection.reply(&header, &cookie)
                } else {
                    full()
                }
            }
            Err(_) => malformed(),
        },
        (SCREENSAVER, Some("UnInhibit")) => match body.deserialize::<u32>() {
            Ok(cookie) => {
                holds.release(&who, &By::Cookie(cookie));
                connection.reply(&header, &())
            }
            Err(_) => malformed(),
        },
        // Nothing is shown in place of the session, so it is never active.
        (SCREENSAVER, Some("GetActive")) => connection.reply(&header, &false),
        (INHIBIT, Some("Inhibit")) => match body.deserialize::<Asked>() {
            Ok((request, _, _, flags, _)) => {
                let hold = Hold {
                    who,
                    by: By::Request(request),
                    idle: flags & IDLE != 0,
                    sleep: flags & SUSPEND != 0,
                };
                if holds.take(hold) {
                    connection.reply(&header, &())
                } else {
                    full()
                }
            }
            Err(_) => malformed(),
        },
        // The portal ends a hold by closing the request it was asked by,
        // as it does any other: one that is not a hold is left alone here.
        (REQUEST, Some("Close")) => {
            let path = header
                .path()
                .map(|path| OwnedObjectPath::from(path.to_owned()));
            let by = path.map(By::Request);
            let released = by.is_some_and(|by| holds.release(&who, &by));
            if !released {
                return None;
            }
            connection.reply(&header, &())
        }
        _ => return None,
    };
    Some(sent)
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc::Receiver;
    use std::time::Duration;

    use zbus::blocking::Connection;
    use zbus::zvariant::{ObjectPath, OwnedValue};

    use super::*;
    use crate::bus::tests::{Daemon, keeper};
    use crate::{Appearance, NAME, PATH, Portal};

    /// The service on a bus of its own, and what it tells of what is held.
    fn service(daemon: &Daemon) -> (Portal, Receiver<(bool, bool)>) {
        let (keep, told) = keeper();
        let look = Appearance {
            dark: true,
            accent: (0.0, 0.0, 0.0),
        };
        let portal = Portal::open(Some(&daemon.address), Box::new(move || look), keep);
        (portal.unwrap(), told)
    }

    /// The next thing told of what is held.
    fn next(told: &Receiver<(bool, bool)>) -> (bool, bool) {
        told.recv_timeout(Duration::from_secs(5)).unwrap()
    }

    /// Asks the screensaver to be held off, as a player does, and gives the
    /// number it is held by.
    fn screensaver_inhibit(peer: &Connection) -> zbus::Result<u32> {
        let asked = ("vlc", "Playing a film");
        let reply = peer.call_method(
            Some(SCREENSAVER),
            "/org/freedesktop/ScreenSaver",
            Some(SCREENSAVER),
            "Inhibit",
            &asked,
        );
        reply.and_then(|reply| reply.body().deserialize())
    }

    fn uninhibit(peer: &Connection, cookie: u32) {
        let path = "/ScreenSaver";
        let asked = (cookie,);
        peer.call_method(
            Some(SCREENSAVER),
            path,
            Some(SCREENSAVER),
            "UnInhibit",
            &asked,
        )
        .unwrap();
    }

    /// Asks, as the portal does for a program in a sandbox, for what
    /// `flags` names to be held off, by a request at `handle`.
    fn portal_inhibit(peer: &Connection, handle: &str, flags: u32) -> zbus::Result<()> {
        let options: HashMap<&str, OwnedValue> = HashMap::new();
        let handle = ObjectPath::try_from(handle).unwrap();
        let asked = (handle, "org.example.Player", "", flags, options);
        let reply = peer.call_method(Some(NAME), PATH, Some(INHIBIT), "Inhibit", &asked);
        reply.map(|_| ())
    }

    #[test]
    fn a_player_keeps_the_session_from_going_idle_until_it_lets_go() {
        let daemon = Daemon::start();
        let (_portal, told) = service(&daemon);
        let player = daemon.peer();

        let cookie = screensaver_inhibit(&player).unwrap();
        assert_eq!(next(&told), (true, false));
        let again = screensaver_inhibit(&player).unwrap();
        assert_ne!(cookie, again, "each is held by its own number");

        uninhibit(&player, cookie);
        uninhibit(&player, again);
        assert_eq!(next(&told), (false, false), "said once all are let go");
    }

    #[test]
    fn what_a_program_held_is_let_go_when_it_leaves_the_bus() {
        let daemon = Daemon::start();
        let (_portal, told) = service(&daemon);
        let player = daemon.peer();
        screensaver_inhibit(&player).unwrap();
        assert_eq!(next(&told), (true, false));

        drop(player);
        assert_eq!(next(&told), (false, false));
    }

    #[test]
    fn a_program_cannot_let_go_of_what_another_holds() {
        let daemon = Daemon::start();
        let (_portal, told) = service(&daemon);
        let (player, other) = (daemon.peer(), daemon.peer());
        let cookie = screensaver_inhibit(&player).unwrap();
        assert_eq!(next(&told), (true, false));

        uninhibit(&other, cookie);
        uninhibit(&player, cookie);
        assert_eq!(next(&told), (false, false), "only once its own lets go");
    }

    #[test]
    fn a_program_in_a_sandbox_keeps_the_machine_from_sleeping_or_the_session_from_idling() {
        let daemon = Daemon::start();
        let (_portal, told) = service(&daemon);
        let portal = daemon.peer();
        let music = "/org/freedesktop/portal/desktop/request/1_2/music";
        let film = "/org/freedesktop/portal/desktop/request/1_2/film";

        portal_inhibit(&portal, music, SUSPEND).unwrap();
        assert_eq!(next(&told), (false, true), "music: only sleep");
        portal_inhibit(&portal, film, IDLE | SUSPEND | 1).unwrap();
        assert_eq!(next(&told), (true, true));

        // The portal ends one by closing its request.
        let close = |path: &str| {
            portal
                .call_method(Some(NAME), path, Some(REQUEST), "Close", &())
                .unwrap();
        };
        close(film);
        assert_eq!(next(&told), (false, true));
        close(music);
        assert_eq!(next(&told), (false, false));
    }

    #[test]
    fn the_screensaver_says_it_is_not_showing_and_refuses_what_it_does_not_understand() {
        let daemon = Daemon::start();
        let (_portal, _told) = service(&daemon);
        let peer = daemon.peer();
        let path = "/org/freedesktop/ScreenSaver";
        let active = peer.call_method(Some(SCREENSAVER), path, Some(SCREENSAVER), "GetActive", &());
        let active: bool = active.unwrap().body().deserialize().unwrap();
        assert!(!active);

        let wrong = (1_u32,);
        let refused = |member: &str| {
            let reply =
                peer.call_method(Some(SCREENSAVER), path, Some(SCREENSAVER), member, &wrong);
            matches!(reply, Err(zbus::Error::MethodError(name, ..))
                if name.as_str() == "org.freedesktop.DBus.Error.InvalidArgs")
        };
        assert!(refused("Inhibit"));
        let wrong = ("just a word",);
        let reply = peer.call_method(
            Some(SCREENSAVER),
            path,
            Some(SCREENSAVER),
            "UnInhibit",
            &wrong,
        );
        assert!(reply.is_err());
        let reply = peer.call_method(Some(NAME), PATH, Some(INHIBIT), "Inhibit", &wrong);
        assert!(reply.is_err());
    }

    #[test]
    fn a_program_can_hold_only_so_much() {
        let daemon = Daemon::start();
        let (_portal, _told) = service(&daemon);
        let greedy = daemon.peer();
        for _ in 0..EACH {
            screensaver_inhibit(&greedy).unwrap();
        }
        let refused = screensaver_inhibit(&greedy);
        assert!(
            matches!(&refused, Err(zbus::Error::MethodError(name, ..))
                if name.as_str() == "org.freedesktop.DBus.Error.LimitsExceeded"),
            "{refused:?}"
        );
        let handle = "/org/freedesktop/portal/desktop/request/1_2/more";
        assert!(portal_inhibit(&greedy, handle, IDLE).is_err());
        // Another may still.
        screensaver_inhibit(&daemon.peer()).unwrap();
    }
}
