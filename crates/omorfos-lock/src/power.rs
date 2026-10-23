//! Sleep, Restart, Shut Down and Log Out from the lock screen, without
//! unlocking: asked of `omorfos-session`, which asks logind, or the
//! compositor, and does only what needs nobody's password (ADR 53).

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

/// The longest a line said by another program is shown, in characters.
const LONGEST: usize = 100;

/// What may be done from the lock screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Act {
    Sleep,
    Restart,
    ShutDown,
    LogOut,
}

impl Act {
    /// Each, in the order they are shown.
    const ALL: [Self; 4] = [Self::Sleep, Self::Restart, Self::ShutDown, Self::LogOut];

    /// The word `omorfos-session` knows it by.
    const fn word(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Restart => "restart",
            Self::ShutDown => "shut-down",
            Self::LogOut => "log-out",
        }
    }

    /// What its button says.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Sleep => "Sleep",
            Self::Restart => "Restart",
            Self::ShutDown => "Shut Down",
            Self::LogOut => "Log Out",
        }
    }

    /// What is asked before it is done, if it loses anything: sleep keeps
    /// the session as it is, locked, and is done at once.
    pub(crate) const fn question(self) -> Option<&'static str> {
        match self {
            Self::Sleep => None,
            Self::Restart => Some("Restart? What is not saved in open programs is lost."),
            Self::ShutDown => Some("Shut down? What is not saved in open programs is lost."),
            Self::LogOut => Some("Log out? What is not saved in open programs is lost."),
        }
    }
}

/// What may be done, and what programs keep the machine from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Offered {
    pub(crate) acts: Vec<Act>,
    /// Who keeps the machine from sleeping or turning off, and why, as a
    /// line to show.
    pub(crate) held: Vec<String>,
}

impl Offered {
    /// What `omorfos-session offered` said. Anything else is passed over.
    fn read(said: &str) -> Self {
        let mut offered = Self::default();
        for line in said.lines() {
            if let Some(word) = line.strip_prefix("can ") {
                let act = Act::ALL.into_iter().find(|act| act.word() == word);
                offered.acts.extend(act);
            } else if let Some(held) = line.strip_prefix("held ") {
                let (from, who) = held.split_once(' ').unwrap_or((held, ""));
                let from = if from == "sleep" {
                    "sleeping"
                } else {
                    "turning off"
                };
                offered
                    .held
                    .push(plain(&format!("Kept from {from} by {who}")));
            }
        }
        offered
    }
}

/// What another program said, on one line, no longer than is shown.
fn plain(said: &str) -> String {
    let plain = said.chars().map(|character| {
        if character.is_control() {
            ' '
        } else {
            character
        }
    });
    plain.take(LONGEST).collect()
}

/// What was heard back from `omorfos-session`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Told {
    /// What may be done now.
    Offered(Offered),
    /// That it was done, or why not.
    Done(Act, Result<(), String>),
}

/// The power buttons: what is offered, and what is being asked.
#[derive(Debug)]
pub(crate) struct Power {
    /// The program asked, `omorfos-session`, and the words it is always
    /// asked with first.
    program: (PathBuf, Vec<OsString>),
    pub(crate) offered: Offered,
    /// What is waiting for the user to say yes to.
    pub(crate) confirming: Option<Act>,
    /// What is being done, until it has been.
    doing: Option<Act>,
    /// Why what was asked last was not done, until something else is.
    pub(crate) failed: String,
    /// Whether what is offered is to be asked again.
    due: bool,
}

impl Power {
    /// Buttons that ask `program`.
    pub(crate) const fn new(program: (PathBuf, Vec<OsString>)) -> Self {
        Self {
            program,
            offered: Offered {
                acts: Vec::new(),
                held: Vec::new(),
            },
            confirming: None,
            doing: None,
            failed: String::new(),
            due: true,
        }
    }

    /// Has what is offered asked again, at the next turn.
    pub(crate) const fn refresh(&mut self) {
        self.due = true;
    }

    /// Asks what is offered, if that is due.
    pub(crate) fn ask(&mut self, told_by: &(Sender<Told>, impl Fn() + Clone + Send + 'static)) {
        if std::mem::take(&mut self.due) {
            let command = self.command(&["offered"]);
            let (sender, wake) = told_by.clone();
            std::thread::spawn(move || {
                let said = run(command).unwrap_or_default();
                let _ = sender.send(Told::Offered(Offered::read(&said)));
                wake();
            });
        }
    }

    /// Takes in a button pressed: the act itself, which is asked about first
    /// if it loses anything; or, being asked about, yes or no to it.
    pub(crate) fn pressed(
        &mut self,
        button: Button,
        told_by: &(Sender<Told>, impl Fn() + Clone + Send + 'static),
    ) {
        if self.doing.is_some() {
            return;
        }
        self.failed.clear();
        let act = match button {
            Button::Act(act) if act.question().is_some() => {
                self.confirming = Some(act);
                self.due = true;
                return;
            }
            Button::Act(act) | Button::Yes(act) => act,
            Button::No => {
                self.confirming = None;
                return;
            }
        };
        self.confirming = None;
        self.doing = Some(act);
        // Asked as from a locked screen, so that nobody is asked for a
        // password that could not be shown; logging out asks for none.
        let words = [act.word(), "--locked"];
        let words = if act == Act::LogOut {
            &words[..1]
        } else {
            &words[..]
        };
        let command = self.command(words);
        let (sender, wake) = told_by.clone();
        std::thread::spawn(move || {
            let done = run(command).map(drop);
            let _ = sender.send(Told::Done(act, done));
            wake();
        });
    }

    /// Takes in what was heard back.
    pub(crate) fn told(&mut self, told: Told) {
        match told {
            Told::Offered(offered) => self.offered = offered,
            Told::Done(act, done) => {
                self.doing = None;
                if let Err(why) = done {
                    self.failed = plain(&format!("{} did not happen: {why}", act.label()));
                }
            }
        }
    }

    /// Forgets what was being asked about, and why something failed: as
    /// the screen shows only the time again.
    pub(crate) fn forget(&mut self) {
        self.confirming = None;
        self.failed.clear();
    }

    /// The buttons shown, in order.
    pub(crate) fn buttons(&self) -> Vec<Button> {
        match self.confirming {
            Some(act) => vec![Button::Yes(act), Button::No],
            None => self.offered.acts.iter().copied().map(Button::Act).collect(),
        }
    }

    /// The program asked for `words`.
    fn command(&self, words: &[&str]) -> Command {
        let (program, first) = &self.program;
        let mut command = Command::new(program);
        command.args(first).args(words);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

/// A button the lock screen shows of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Button {
    /// One that does what it says, or asks first.
    Act(Act),
    /// Yes to what is asked about.
    Yes(Act),
    /// No to it.
    No,
}

impl Button {
    /// What it says.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Act(act) | Self::Yes(act) => act.label(),
            Self::No => "Cancel",
        }
    }
}

/// Runs `command`, and returns what it said; or, if it failed, why, as it
/// said or else as it could not be started.
fn run(mut command: Command) -> Result<String, String> {
    let output = command.output().map_err(|failure| failure.to_string())?;
    let said = String::from_utf8_lossy(&output.stdout).into_owned();
    if output.status.success() {
        return Ok(said);
    }
    let why = String::from_utf8_lossy(&output.stderr);
    let why = why.lines().next().unwrap_or_default();
    Err(why
        .strip_prefix("omorfos-session: ")
        .unwrap_or(why)
        .to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_offered_is_read_and_anything_else_passed_over() {
        let said = "can sleep\ncan log-out\ncan fly\nheld shut-down Firefox: Down\u{7}load\n\
                    held sleep Player: Playing\nheld nothing\nsomething else\n";
        let offered = Offered::read(said);
        assert_eq!(offered.acts, [Act::Sleep, Act::LogOut]);
        assert_eq!(
            offered.held,
            [
                "Kept from turning off by Firefox: Down load",
                "Kept from sleeping by Player: Playing",
                "Kept from turning off by ",
            ]
        );
        let long = Offered::read(&format!("held sleep {}\n", "x".repeat(500)));
        assert_eq!(long.held[0].chars().count(), LONGEST);
    }

    #[test]
    fn what_loses_nothing_is_not_asked_about_first() {
        assert_eq!(Act::Sleep.question(), None);
        for act in [Act::Restart, Act::ShutDown, Act::LogOut] {
            assert!(act.question().is_some_and(|asked| asked.contains("lost")));
        }
        assert_eq!(Button::No.label(), "Cancel");
        assert_eq!(Button::Yes(Act::ShutDown).label(), "Shut Down");
    }
}
