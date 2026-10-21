//! The part of the session that runs all session: it locks the screen
//! when logind asks, and before the machine sleeps, and holds the sleep
//! back until the screen is locked, so that the machine never wakes
//! unlocked.
//!
//! logind lets a program delay sleep by holding a file it hands out
//! (`Inhibit`, in `delay` mode), and says when the machine is about to
//! sleep and when it has woken (`PrepareForSleep`). The compositor says
//! when the session is locked: a locker holds it, and nothing of the
//! session is on any screen. What to do when is [`Watch`], a value made
//! from what has been heard; the rest is plumbing.

use std::path::Path;
use std::sync::mpsc::{self, Sender};
use std::time::{Duration, Instant};

use omorfos_ipc::{Client, Reply, Request};
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::Type;
use zbus::names::{InterfaceName, MemberName};
use zbus::zvariant::{ObjectPath, OwnedFd, OwnedObjectPath, OwnedValue};

use crate::{Failure, Places, connect, shown};

/// logind, on the system's bus.
pub(crate) const LOGIND: &str = "org.freedesktop.login1";
/// Where its manager is.
pub(crate) const MANAGER_PATH: &str = "/org/freedesktop/login1";
/// What its manager is.
pub(crate) const MANAGER: &str = "org.freedesktop.login1.Manager";
/// What a session of its is.
const SESSION: &str = "org.freedesktop.login1.Session";

/// How long logind waits for sleep to be let go before it sleeps anyway,
/// where it does not say: its own default.
const DELAY: Duration = Duration::from_secs(5);

/// What is heard, from logind and from the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Heard {
    /// The machine is about to sleep, or has woken.
    Sleeping(bool),
    /// logind asks for the session to be locked.
    LockAsked,
    /// Whether the session is locked, as the compositor says.
    Locked(bool),
}

/// What is to be done about what was heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Do {
    /// Ask the compositor to lock the session.
    Lock,
    /// Let the machine sleep: once the session is locked, or once nothing
    /// will lock it, or once logind would not wait any longer.
    LetSleep {
        /// Whether it is to be said that the machine woke unlocked: it
        /// was to have been locked, and was not in time.
        warn: bool,
    },
    /// Hold the next sleep back until it is let go.
    Hold,
}

/// What is known, and what is being waited for.
#[derive(Debug)]
pub(crate) struct Watch {
    /// How long logind waits for sleep to be let go.
    delay: Duration,
    /// Whether the session is locked.
    locked: bool,
    /// When the compositor was last asked to lock, until it has.
    asked: Option<Instant>,
    /// While the machine waits to sleep, when it is let go whatever
    /// happens.
    sleeping: Option<Instant>,
}

impl Watch {
    /// Nothing heard yet, where logind waits `delay` for sleep to be let
    /// go.
    pub(crate) const fn new(delay: Duration) -> Self {
        Self {
            delay,
            locked: false,
            asked: None,
            sleeping: None,
        }
    }

    /// What to do on hearing `heard` at `now`.
    pub(crate) fn heard(&mut self, heard: Heard, now: Instant) -> Option<Do> {
        match heard {
            Heard::Locked(locked) => {
                self.locked = locked;
                // The locker asked for has come.
                self.asked = self.asked.filter(|_| !locked);
                let waiting = self.sleeping.take_if(|_| locked);
                waiting.map(|_| Do::LetSleep { warn: false })
            }
            Heard::Sleeping(true) if self.locked => Some(Do::LetSleep { warn: false }),
            Heard::Sleeping(true) => {
                self.sleeping = Some(now + self.delay);
                self.lock(now)
            }
            // Woken, whether or not sleep was let go: logind may not have
            // waited. What held it then holds nothing now.
            Heard::Sleeping(false) => {
                self.sleeping = None;
                Some(Do::Hold)
            }
            Heard::LockAsked => self.lock(now),
        }
    }

    /// When sleep is to be let go of if nothing is heard before.
    pub(crate) const fn due(&self) -> Option<Instant> {
        self.sleeping
    }

    /// What to do once [`Self::due`] has come with nothing heard.
    pub(crate) const fn waited(&mut self) -> Option<Do> {
        match self.sleeping.take() {
            Some(_) => Some(Do::LetSleep { warn: true }),
            None => None,
        }
    }

    /// What to do once the compositor has said that nothing will lock the
    /// session, since none is set to: the user is told so by the
    /// compositor, and the machine is not kept awake for it.
    pub(crate) fn refused(&mut self) -> Option<Do> {
        self.asked = None;
        let waiting = self.sleeping.take();
        waiting.map(|_| Do::LetSleep { warn: false })
    }

    /// Asks for the session to be locked, unless it is or has just been
    /// asked to be: a locker started twice is one too many.
    fn lock(&mut self, now: Instant) -> Option<Do> {
        let recently = self.asked.is_some_and(|at| now < at + self.delay);
        if self.locked || recently {
            return None;
        }
        self.asked = Some(now);
        Some(Do::Lock)
    }
}

/// Runs until the compositor ends, locking the session when logind asks
/// and before the machine sleeps. A machine without logind has nothing to
/// lock before and nobody to ask, and this ends at once.
///
/// # Errors
///
/// If the compositor cannot be reached, or logind's bus goes away.
pub fn watch(places: &Places) -> Result<(), Failure> {
    let compositor = places.compositor.as_deref();
    let compositor = compositor.ok_or("there is no compositor to lock the session of")?;
    let news = Client::connect(compositor);
    let news = news.and_then(|mut news| news.send(&Request::Subscribe).map(|()| news))?;
    let Ok(bus) = connect(&places.system) else {
        return Ok(());
    };
    // Listened to before sleep is held back, so that nothing said in
    // between goes unheard while it is.
    let signals = signals(&bus);
    // logind not being there is the one failure that is not one: anything
    // else, a logind too busy to answer in time, is tried again.
    let held = inhibit(&bus);
    if let Err(error) = &held
        && absent(error)
    {
        return Ok(());
    }
    let mut held = Some(held?);
    let mut watch = Watch::new(delay(&bus));
    let (tell, heard) = mpsc::channel();
    let (told, followed) = (tell.clone(), tell.clone());
    std::thread::spawn(move || listen(signals, &told));
    std::thread::spawn(move || follow(news, &followed));
    loop {
        // While nothing is due, as good as for ever: none is waited for
        // longer than an `Instant` can count.
        let due = watch
            .due()
            .map(|at| at.saturating_duration_since(Instant::now()));
        let mut done = match heard.recv_timeout(due.unwrap_or(Duration::MAX)) {
            Ok(Ok(heard)) => watch.heard(heard, Instant::now()),
            Ok(Err(ended)) => return ended,
            // Never cut off, since `tell` is kept here till the end: it
            // has waited.
            Err(_) => watch.waited(),
        };
        while let Some(action) = done.take() {
            done = match action {
                Do::Lock if ask_to_lock(compositor) => None,
                Do::Lock => watch.refused(),
                Do::LetSleep { warn: warned } => {
                    drop(held.take());
                    if warned {
                        warn(places);
                    }
                    None
                }
                Do::Hold => {
                    held = inhibit(&bus).ok();
                    None
                }
            };
        }
    }
}

/// What is told by one of the threads that listen: something heard, or
/// that what it listened to has ended, and how.
type Told = Result<Heard, Result<(), Failure>>;

/// Holds the machine's next sleep back until what is returned is dropped.
fn inhibit(bus: &Connection) -> zbus::Result<OwnedFd> {
    let why = "The screen is locked before the machine sleeps";
    let asked = ("sleep", "omorfos", why, "delay");
    let reply = bus.call_method(Some(LOGIND), MANAGER_PATH, Some(MANAGER), "Inhibit", &asked);
    reply.and_then(|reply| reply.body().deserialize())
}

/// Whether `error` says that logind is not on the bus at all, as on a
/// machine without it.
fn absent(error: &zbus::Error) -> bool {
    let unknown = "org.freedesktop.DBus.Error.ServiceUnknown";
    matches!(error, zbus::Error::MethodError(name, ..) if name.as_str() == unknown)
}

/// How long logind waits for sleep to be let go.
fn delay(bus: &Connection) -> Duration {
    // Asked as any call is, and given up on as one is.
    let asked = (MANAGER, "InhibitDelayMaxUSec");
    let properties = Some("org.freedesktop.DBus.Properties");
    let reply = bus.call_method(Some(LOGIND), MANAGER_PATH, properties, "Get", &asked);
    let value: Option<OwnedValue> = reply.ok().and_then(|reply| reply.body().deserialize().ok());
    let said = value.and_then(|value| u64::try_from(value).ok());
    said.map_or(DELAY, Duration::from_micros)
}

/// What logind says, from now on: that the machine is about to sleep or
/// has woken, and that this session is to be locked.
fn signals(bus: &Connection) -> zbus::Result<(MessageIterator, Option<OwnedObjectPath>)> {
    // Only what logind itself says is heard: the bus delivers a signal
    // from a name only if it comes from whoever has the name.
    let rule = zbus::MatchRule::builder()
        .msg_type(Type::Signal)
        .sender(LOGIND);
    let signals = rule.and_then(|rule| MessageIterator::for_match_rule(rule.build(), bus, None));
    let asked = bus.call_method(
        Some(LOGIND),
        MANAGER_PATH,
        Some(MANAGER),
        "GetSession",
        &("auto",),
    );
    // Outside a session nothing asks it to lock, and sleep still does.
    let session = asked.ok().and_then(|reply| reply.body().deserialize().ok());
    signals.map(|signals| (signals, session))
}

/// Tells what logind says until it can no longer be heard: its bus goes
/// away, or it could not be listened to at all.
fn listen(heard: zbus::Result<(MessageIterator, Option<OwnedObjectPath>)>, tell: &Sender<Told>) {
    // Nothing to tell, if it could not be listened to.
    let heard = heard.into_iter();
    heard.for_each(|(signals, session)| tell_signals(signals, session.as_ref(), tell));
    let _ = tell.send(Err(Err("logind can no longer be heard".into())));
}

/// Tells each thing logind says that is heeded, until it stops.
fn tell_signals(signals: MessageIterator, session: Option<&OwnedObjectPath>, tell: &Sender<Told>) {
    for message in signals.flatten() {
        let header = message.header();
        let said = header.interface().map(InterfaceName::as_str);
        let member = header.member().map(MemberName::as_str);
        let path = header.path().map(ObjectPath::as_str);
        let ours = session.map(|session| session.as_str());
        let heard = match (said, member) {
            (Some(MANAGER), Some("PrepareForSleep")) => message
                .body()
                .deserialize::<bool>()
                .ok()
                .map(Heard::Sleeping),
            // logind's `Unlock` is never heeded: a locked session is
            // opened by its user's secret and nothing else.
            (Some(SESSION), Some("Lock")) => (path == ours).then_some(Heard::LockAsked),
            _ => None,
        };
        if let Some(heard) = heard {
            let _ = tell.send(Ok(heard));
        }
    }
}

/// Tells whether the session is locked each time the compositor says it
/// changed, until the compositor ends.
fn follow(mut news: Client, tell: &Sender<Told>) {
    while let Some(state) = news.receive().ok().and_then(Reply::into_state) {
        let _ = tell.send(Ok(Heard::Locked(state.locked)));
    }
    let _ = tell.send(Err(Ok(())));
}

/// Asks the compositor to lock the session, with the locker its
/// configuration names, and says whether it was asked: whether it did
/// not refuse because none is named. One that could not be asked at all
/// may still lock, and is waited for.
fn ask_to_lock(compositor: &Path) -> bool {
    let lock = Request::Perform {
        action: serde_json::Value::from("lock"),
    };
    let connected = Client::connect(compositor);
    let answer =
        connected.and_then(|mut client| client.send(&lock).and_then(|()| client.receive()));
    !matches!(answer, Ok(Reply::Error { .. }))
}

/// Says that the machine slept before the session was locked, so that it
/// woke unlocked: shown, where it can be, once it has woken.
fn warn(places: &Places) {
    let bus = places.session.as_deref();
    let bus = bus.and_then(|address| connect(address).ok());
    let _ = bus.map(|bus| {
        shown::warn(
            &bus,
            "The screen was not locked before sleep",
            "The machine went to sleep before the screen locker had locked \
             the screen, so it woke unlocked. Check that `locker` in \
             config.yaml names a screen locker, and that it starts quickly.",
        )
    });
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::thread::JoinHandle;

    use super::*;
    use crate::stand_in::{self, Compositor, Daemon, Logind, SESSION};

    const DELAY: Duration = Duration::from_secs(5);

    fn after(start: Instant, seconds: u64) -> Instant {
        start + Duration::from_secs(seconds)
    }

    #[test]
    fn sleep_is_held_back_until_the_session_is_locked() {
        let start = Instant::now();
        let mut watch = Watch::new(DELAY);
        assert_eq!(watch.heard(Heard::Locked(false), start), None);
        assert_eq!(watch.heard(Heard::Sleeping(true), start), Some(Do::Lock));
        assert_eq!(watch.due(), Some(after(start, 5)), "logind waits as long");
        let now = after(start, 1);
        assert_eq!(watch.heard(Heard::Locked(false), now), None, "not yet");
        let locked = Some(Do::LetSleep { warn: false });
        assert_eq!(watch.heard(Heard::Locked(true), now), locked);
        assert_eq!(watch.due(), None);
        assert_eq!(watch.waited(), None, "nothing more is let go");
        assert_eq!(watch.heard(Heard::Sleeping(false), now), Some(Do::Hold));
    }

    #[test]
    fn a_session_locked_already_is_let_sleep_at_once() {
        let start = Instant::now();
        let mut watch = Watch::new(DELAY);
        assert_eq!(watch.heard(Heard::Locked(true), start), None);
        let heard = watch.heard(Heard::Sleeping(true), start);
        assert_eq!(heard, Some(Do::LetSleep { warn: false }));
        assert_eq!(watch.due(), None);
        assert_eq!(
            watch.heard(Heard::LockAsked, start),
            None,
            "nor locked again"
        );
    }

    #[test]
    fn a_session_not_locked_in_time_is_let_sleep_and_said_to_be() {
        let start = Instant::now();
        let mut watch = Watch::new(DELAY);
        assert_eq!(watch.heard(Heard::Sleeping(true), start), Some(Do::Lock));
        assert_eq!(watch.waited(), Some(Do::LetSleep { warn: true }));
        assert_eq!(watch.due(), None);
        // Woken, and locked only now: nothing is waiting to be let go.
        let now = after(start, 30);
        assert_eq!(watch.heard(Heard::Sleeping(false), now), Some(Do::Hold));
        assert_eq!(watch.heard(Heard::Locked(true), now), None);
    }

    #[test]
    fn a_session_with_nothing_to_lock_it_is_let_sleep_at_once() {
        let start = Instant::now();
        let mut watch = Watch::new(DELAY);
        assert_eq!(watch.heard(Heard::Sleeping(true), start), Some(Do::Lock));
        assert_eq!(watch.refused(), Some(Do::LetSleep { warn: false }));
        assert_eq!(watch.due(), None);
        assert_eq!(watch.refused(), None, "nothing is waiting");
        let heard = watch.heard(Heard::LockAsked, after(start, 1));
        assert_eq!(heard, Some(Do::Lock), "a locker may have been named since");
    }

    #[test]
    fn a_machine_woken_before_it_was_let_go_holds_the_next_sleep_back_afresh() {
        let start = Instant::now();
        let mut watch = Watch::new(DELAY);
        watch.heard(Heard::Sleeping(true), start);
        assert_eq!(watch.heard(Heard::Sleeping(false), start), Some(Do::Hold));
        assert_eq!(watch.due(), None);
    }

    #[test]
    fn the_session_is_locked_when_logind_asks_once_however_often_it_asks() {
        let start = Instant::now();
        let mut watch = Watch::new(DELAY);
        assert_eq!(watch.heard(Heard::LockAsked, start), Some(Do::Lock));
        assert_eq!(watch.due(), None, "it is not about to sleep");
        assert_eq!(watch.heard(Heard::LockAsked, after(start, 1)), None);
        let heard = watch.heard(Heard::Sleeping(true), after(start, 2));
        assert_eq!(heard, None, "the locker asked for is still on its way");
        // A locker that never came is asked for again.
        let heard = watch.heard(Heard::LockAsked, after(start, 5));
        assert_eq!(heard, Some(Do::Lock));
        watch.heard(Heard::Locked(true), after(start, 6));
        watch.heard(Heard::Locked(false), after(start, 7));
        let heard = watch.heard(Heard::LockAsked, after(start, 8));
        assert_eq!(heard, Some(Do::Lock), "unlocked, it locks again");
    }

    /// Places with a bus and a compositor of the test's own.
    fn places(daemon: &Daemon, compositor: &Compositor) -> Places {
        Places {
            backlights: PathBuf::from("/nowhere"),
            path: OsString::new(),
            session: Some(daemon.address.clone()),
            system: daemon.address.clone(),
            compositor: Some(compositor.path.clone()),
            autostart: Vec::new(),
            desktops: Vec::new(),
            scoper: None,
        }
    }

    /// Watches over a session on a thread of its own, and waits until it
    /// is holding sleep back and following the state.
    fn watching(places: Places, logind: &Logind, compositor: &Compositor) -> JoinHandle<String> {
        let running = std::thread::spawn(move || match watch(&places) {
            Ok(()) => "ended".to_owned(),
            Err(failure) => failure.to_string(),
        });
        eventually(|| logind.holds() == [true] && compositor.subscribers() == 1);
        running
    }

    fn eventually(mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        while !done() {
            assert!(start.elapsed() < Duration::from_secs(10), "never happened");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// What is asked of the compositor to lock the session.
    const LOCK: &str = r#"{"perform":{"action":"lock"}}"#;

    #[test]
    fn before_the_machine_sleeps_the_session_is_locked_and_then_let_sleep() {
        let daemon = Daemon::start();
        // Long enough that it is never let go for want of patience.
        let logind = stand_in::logind_waiting(&daemon, Duration::from_secs(60));
        let compositor = Compositor::start();
        let running = watching(places(&daemon, &compositor), &logind, &compositor);

        logind.sleeping(true);
        eventually(|| *compositor.asked.lock().unwrap() == [LOCK]);
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(logind.holds(), [true], "held back while it locks");
        compositor.locked(true);
        eventually(|| logind.holds() == [false]);

        // Woken, the next sleep is held back too.
        logind.sleeping(false);
        eventually(|| logind.holds() == [false, true]);
        compositor.end();
        assert_eq!(running.join().unwrap(), "ended", "with the compositor");
    }

    #[test]
    fn a_machine_that_sleeps_unlocked_says_so_when_it_wakes() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let (_service, shown) = stand_in::notifications(&daemon);
        let compositor = Compositor::start();
        let _running = watching(places(&daemon, &compositor), &logind, &compositor);

        logind.sleeping(true);
        // The stand-in waits a fifth of a second.
        eventually(|| logind.holds() == [false]);
        eventually(|| !shown.lock().unwrap().is_empty());
        let (icon, title, level, words) = shown.lock().unwrap()[0].clone();
        assert_eq!((icon.as_str(), level), ("dialog-warning", -1));
        assert_eq!(title, "The screen was not locked before sleep");
        assert!(words.contains("`locker`"), "{words}");
    }

    #[test]
    fn a_compositor_with_nothing_to_lock_with_lets_the_machine_sleep_at_once() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let (_service, shown) = stand_in::notifications(&daemon);
        let compositor = Compositor::start();
        compositor.refuse();
        let _running = watching(places(&daemon, &compositor), &logind, &compositor);
        logind.sleeping(true);
        eventually(|| logind.holds() == [false]);
        // Let go of because it was refused: waiting until logind would
        // wait no longer warns that the machine slept unlocked.
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(*shown.lock().unwrap(), [], "not waited for");
    }

    #[test]
    fn a_warning_with_nowhere_to_be_shown_is_not_shown() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let compositor = Compositor::start();
        let places = Places {
            session: None,
            ..places(&daemon, &compositor)
        };
        let _running = watching(places, &logind, &compositor);
        logind.sleeping(true);
        eventually(|| logind.holds() == [false]);
    }

    #[test]
    fn logind_asking_this_session_to_lock_locks_it_and_unlock_is_not_heeded() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let compositor = Compositor::start();
        let _running = watching(places(&daemon, &compositor), &logind, &compositor);

        logind.tell("/org/freedesktop/login1/session/_2", "Lock");
        logind.tell(SESSION, "Unlock");
        logind.tell(SESSION, "Lock");
        eventually(|| !compositor.asked.lock().unwrap().is_empty());
        std::thread::sleep(Duration::from_millis(50));
        let asked = compositor.asked.lock().unwrap();
        assert_eq!(*asked, [LOCK], "for this session, once");
    }

    #[test]
    fn without_logind_there_is_nothing_to_watch_for() {
        let daemon = Daemon::start();
        let compositor = Compositor::start();
        assert!(
            watch(&places(&daemon, &compositor)).is_ok(),
            "logind not on the bus"
        );
        let unreachable = Places {
            system: "unix:path=/nowhere/bus".to_owned(),
            ..places(&daemon, &compositor)
        };
        assert!(watch(&unreachable).is_ok(), "no bus");
    }

    #[test]
    fn a_logind_that_does_not_answer_is_a_failure_to_be_started_again_after() {
        let daemon = Daemon::start();
        let _mute = stand_in::mute_logind(&daemon);
        let compositor = Compositor::start();
        let started = Instant::now();
        assert!(watch(&places(&daemon, &compositor)).is_err());
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "and is not waited for long"
        );
    }

    #[test]
    fn logind_is_waited_on_as_long_as_it_says_it_waits_or_else_five_seconds() {
        let daemon = Daemon::start();
        let bus = connect(&daemon.address).unwrap();
        assert_eq!(delay(&bus), Duration::from_secs(5), "not there to say");
        let _logind = stand_in::logind(&daemon);
        assert_eq!(delay(&bus), Duration::from_millis(200));
    }

    #[test]
    fn without_a_compositor_there_is_no_session_to_watch() {
        let daemon = Daemon::start();
        let compositor = Compositor::start();
        let none = Places {
            compositor: None,
            ..places(&daemon, &compositor)
        };
        let failure = watch(&none).unwrap_err();
        assert_eq!(
            failure.to_string(),
            "there is no compositor to lock the session of"
        );
        let gone = Places {
            compositor: Some(PathBuf::from("/nowhere/ipc.sock")),
            ..none
        };
        assert!(watch(&gone).is_err());
    }

    #[test]
    fn logind_going_away_is_a_failure_to_be_started_again_after() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let compositor = Compositor::start();
        let running = watching(places(&daemon, &compositor), &logind, &compositor);
        // Listening, once it is heard to.
        logind.sleeping(true);
        eventually(|| !compositor.asked.lock().unwrap().is_empty());
        drop(daemon);
        assert_eq!(running.join().unwrap(), "logind can no longer be heard");
    }
}
