//! The ways of unlocking, each asked by a helper of its own at once: the
//! password always, and a finger and a security key where the
//! configuration offers them and PAM is set up for them.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use omorfos_shell::said::{Said, heard};

/// How long a way other than the password waits to be asked again after
/// it failed, so that one that fails at once does not keep the lock screen
/// busy starting it.
const AGAIN: Duration = Duration::from_secs(1);

/// A way of unlocking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    /// The user's password, or whatever the system's login asks.
    Password,
    /// A finger on the reader.
    Fingerprint,
    /// A security key touched.
    Key,
}

impl Way {
    /// The PAM service it is asked through.
    #[must_use]
    pub const fn service(self) -> &'static str {
        match self {
            Self::Password => "omorfos-lock",
            Self::Fingerprint => "omorfos-lock-fingerprint",
            Self::Key => "omorfos-lock-key",
        }
    }
}

/// What a way has come to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Came {
    /// The user is who they say: the session is to be unlocked.
    Right,
    /// The password typed was not right.
    Wrong,
    /// Nothing that ends it yet.
    Going,
}

/// A way being asked.
#[derive(Debug)]
pub(crate) struct Asking {
    pub(crate) way: Way,
    /// What it last said, to be shown.
    pub(crate) said: String,
    /// Whether it is asking for the secret, which may be typed now.
    pub(crate) asks: bool,
    /// Whether it has been found not to be set up here, and is not shown.
    pub(crate) gone: bool,
    helper: Option<Helper>,
    /// When to ask it again, after it failed.
    again: Option<Instant>,
}

/// A helper asking PAM, and where what is typed goes.
#[derive(Debug)]
struct Helper {
    child: Child,
    told: ChildStdin,
}

impl Helper {
    /// Starts `program` to ask `way` about `user`, and sends each thing it
    /// says to `heard`, and [`Said::Wrong`] once it has stopped without
    /// saying how it ended.
    fn start(
        program: &Path,
        (way, user): (Way, &str),
        heard_by: (Sender<(Way, Said)>, impl Fn() + Send + 'static),
    ) -> std::io::Result<Self> {
        let started = Command::new(program)
            .arg("check")
            .arg(way.service())
            .arg(user)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        started.and_then(|mut child| {
            // Asked for when it was started, so both are there.
            let pipes = child.stdin.take().zip(child.stdout.take());
            let pipes = pipes.ok_or(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
            pipes.map(|(told, said)| {
                let (sender, wake) = heard_by;
                std::thread::spawn(move || {
                    let lines = BufReader::new(said).lines().map_while(Result::ok);
                    let mut ended = false;
                    for said in lines.filter_map(|line| heard(&line)) {
                        ended = matches!(said, Said::Right | Said::Wrong | Said::Unavailable);
                        let _ = sender.send((way, said));
                        wake();
                    }
                    if !ended {
                        let _ = sender.send((way, Said::Wrong));
                        wake();
                    }
                });
                Self { child, told }
            })
        })
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The ways being asked, and what starts the helpers.
#[derive(Debug)]
pub(crate) struct Ways {
    pub(crate) asking: Vec<Asking>,
    program: PathBuf,
    user: String,
}

impl Ways {
    /// The `ways` to ask about `user`, by helpers that are `program`.
    pub(crate) fn new(ways: &[Way], program: PathBuf, user: String) -> Self {
        let asking = ways.iter().map(|way| Asking {
            way: *way,
            said: String::new(),
            asks: false,
            gone: false,
            helper: None,
            again: Some(Instant::now()),
        });
        Self {
            asking: asking.collect(),
            program,
            user,
        }
    }

    /// Starts the helpers that are due to be, and returns when the next
    /// is.
    pub(crate) fn start(
        &mut self,
        now: Instant,
        heard_by: &(Sender<(Way, Said)>, impl Fn() + Clone + Send + 'static),
    ) -> Option<Instant> {
        for asking in &mut self.asking {
            if asking.again.is_some_and(|again| again <= now) {
                asking.again = None;
                let (sender, wake) = (heard_by.0.clone(), heard_by.1.clone());
                let started =
                    Helper::start(&self.program, (asking.way, &self.user), (sender, wake));
                match started {
                    Ok(helper) => asking.helper = Some(helper),
                    // One that cannot be started is tried again later.
                    Err(_) => asking.again = Some(now + AGAIN),
                }
            }
        }
        self.asking.iter().filter_map(|asking| asking.again).min()
    }

    /// Takes in what `way` said, at `now`.
    pub(crate) fn heard(&mut self, (way, said): (Way, Said), now: Instant) -> Came {
        let password = way == Way::Password;
        for asking in self.asking.iter_mut().filter(|asking| asking.way == way) {
            match &said {
                Said::Right => return Came::Right,
                // What PAM asks is said in the field; what it said before
                // stays under it until something is typed.
                Said::Secret(_) | Said::Open(_) if password => asking.asks = true,
                // Only the password is typed into; a way that asks for more,
                // as a key may for its PIN, is not asked here.
                Said::Secret(_) | Said::Open(_) => {
                    asking.said =
                        "asks for something to be typed, which only the password is".into();
                    asking.gone = true;
                    asking.helper.take().into_iter().for_each(Helper::stop);
                }
                Said::Error(what) | Said::Info(what) => asking.said.clone_from(what),
                Said::Wrong | Said::Unavailable => {
                    asking.asks = false;
                    asking.helper.take().into_iter().for_each(Helper::stop);
                    // What another way said is of the try that ended, and
                    // is not shown until it says something again: a key
                    // taken out says nothing, and is not to be shown.
                    if !password {
                        asking.said.clear();
                    }
                    let unavailable = said == Said::Unavailable;
                    // The password is never left off, whatever its helper
                    // says, and is asked again at once after a wrong try;
                    // another a moment later, so that one that fails at once
                    // does not keep the lock screen busy.
                    asking.gone = unavailable && !password;
                    let wait = if password && !unavailable {
                        Duration::ZERO
                    } else {
                        AGAIN
                    };
                    asking.again = (!asking.gone).then_some(now + wait);
                    if password && !unavailable {
                        if asking.said.is_empty() {
                            asking.said = "That was not right".into();
                        }
                        return Came::Wrong;
                    }
                }
            }
        }
        Came::Going
    }

    /// Hands what was typed to the password's helper, if it is asking.
    /// Returns whether it was.
    pub(crate) fn answer(&mut self, typed: &str) -> bool {
        let asking = self.asking.iter_mut();
        let mut asking = asking.filter(|asking| asking.way == Way::Password && asking.asks);
        let Some(password) = asking.next() else {
            return false;
        };
        password.asks = false;
        password.said.clear();
        let told = password.helper.as_mut().map(|helper| &mut helper.told);
        // One that has gone says so, and is asked again.
        let _ = told.map(|told| writeln!(told, "{typed}").and_then(|()| told.flush()));
        true
    }

    /// Stops every helper.
    pub(crate) fn stop(&mut self) {
        let helpers = self
            .asking
            .iter_mut()
            .filter_map(|asking| asking.helper.take());
        helpers.for_each(Helper::stop);
    }
}
