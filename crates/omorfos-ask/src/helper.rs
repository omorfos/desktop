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

use omorfos_shell::said::{Said, heard};

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
