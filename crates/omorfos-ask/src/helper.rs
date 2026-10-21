//! What polkit's helper says, and what it is told.
//!
//! polkit does not take a password from an agent. The agent runs polkit's
//! own helper, `polkit-agent-helper-1`, which may read the user's secret,
//! as the user it is to check: it is told the cookie of the request on its
//! first line, asks PAM, and says on its standard output what PAM asks or
//! says, one line each, a word first, until it says whether that was
//! right. What is typed is written back to it, a line for each question.

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};

/// What the helper says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Said {
    /// A question whose answer is not to be shown as it is typed, as a
    /// password is not.
    Secret(String),
    /// A question whose answer may be shown.
    Open(String),
    /// Something gone wrong to tell the user.
    Error(String),
    /// Something to tell the user.
    Info(String),
    /// That was right: the program may.
    Right,
    /// That was not, or it could not be asked.
    Wrong,
}

/// What one line from the helper says, if it says anything this knows.
#[must_use]
pub fn heard(line: &str) -> Option<Said> {
    let line = line.trim_end_matches(['\n', '\r']);
    let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
    let rest = rest.to_owned();
    match word {
        "PAM_PROMPT_ECHO_OFF" => Some(Said::Secret(rest)),
        "PAM_PROMPT_ECHO_ON" => Some(Said::Open(rest)),
        "PAM_ERROR_MSG" => Some(Said::Error(rest)),
        "PAM_TEXT_INFO" => Some(Said::Info(rest)),
        "SUCCESS" => Some(Said::Right),
        "FAILURE" => Some(Said::Wrong),
        _ => None,
    }
}

/// The helper, checking one try of the user's secret.
#[derive(Debug)]
pub struct Helper {
    child: Child,
    told: ChildStdin,
}

impl Helper {
    /// Starts `program` to check the secret of the user called `user` for
    /// the request `cookie`, and calls `told` with each thing it says, and
    /// with [`Said::Wrong`] once it has stopped without saying it was
    /// right.
    ///
    /// # Errors
    ///
    /// If the helper cannot be started or told the request.
    pub fn start(
        program: &Path,
        (user, cookie): (&str, &str),
        told: impl Fn(Said) + Send + 'static,
    ) -> std::io::Result<Self> {
        let started = Command::new(program)
            .arg(user)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        started.and_then(|mut child| {
            // Asked for when it was started, so both are there.
            let pipes = child.stdin.take().zip(child.stdout.take());
            let pipes = pipes.ok_or(std::io::Error::from(std::io::ErrorKind::BrokenPipe));
            let told_cookie = pipes.and_then(|(mut said_to, said)| {
                writeln!(said_to, "{cookie}").map(|()| (said_to, said))
            });
            told_cookie.map(|(said_to, said)| {
                std::thread::spawn(move || {
                    let lines = BufReader::new(said).lines().map_while(Result::ok);
                    let mut ended = false;
                    for said in lines.filter_map(|line| heard(&line)) {
                        ended = matches!(said, Said::Right | Said::Wrong);
                        told(said);
                    }
                    if !ended {
                        told(Said::Wrong);
                    }
                });
                Self {
                    child,
                    told: said_to,
                }
            })
        })
    }

    /// Answers what the helper asked with what was typed.
    pub fn answer(&mut self, typed: &str) {
        let _ = writeln!(self.told, "{typed}");
    }

    /// Stops the helper, as when the user or polkit gives up on the
    /// request.
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        // A helper that said it was done ends of itself; it is waited for so
        // that it is not left a zombie.
        let _ = self.child.try_wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_line_says_what_pam_asks_or_says_or_how_it_ended() {
        let said = |line| heard(line);
        assert_eq!(
            said("PAM_PROMPT_ECHO_OFF Password: \n"),
            Some(Said::Secret("Password: ".to_owned()))
        );
        assert_eq!(
            said("PAM_PROMPT_ECHO_ON Login:"),
            Some(Said::Open("Login:".to_owned()))
        );
        assert_eq!(
            said("PAM_ERROR_MSG Account locked"),
            Some(Said::Error("Account locked".to_owned()))
        );
        assert_eq!(
            said("PAM_TEXT_INFO Touch the key"),
            Some(Said::Info("Touch the key".to_owned()))
        );
        assert_eq!(said("SUCCESS\n"), Some(Said::Right));
        assert_eq!(said("FAILURE"), Some(Said::Wrong));
        assert_eq!(said("something else"), None);
        assert_eq!(said(""), None);
    }
}
