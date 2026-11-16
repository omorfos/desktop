//! Threads started without ending the program when none can be.
//!
//! `std::thread::spawn` panics where the system starts no more threads, as
//! under the user's limit on processes (`RLIMIT_NPROC`), and so ends the
//! program that asked: the compositor, and with it the session. Every
//! thread of omorfos is started here instead. One that cannot be is said
//! on standard error, which goes to the session's log, and the error is
//! returned; whoever asked goes on without what it would have done.
//! `clippy.toml` refuses `std::thread::spawn` everywhere else.

use std::io::{self, Write};
use std::thread::{self, JoinHandle};

/// Starts `work` on a thread of its own, called `name`.
///
/// # Errors
///
/// Returns an error, and says it on standard error, if no thread can be
/// started.
pub fn start<T, F>(name: &str, work: F) -> io::Result<JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let started = thread::Builder::new().name(name.to_owned()).spawn(work);
    started.inspect_err(|error| {
        let _ = writeln!(io::stderr(), "{name}: no thread could be started: {error}");
    })
}

#[cfg(test)]
mod tests {
    use rustix::process::{Resource, getrlimit, setrlimit};

    use super::*;

    /// Set in the test run by itself as a process of its own, whose limit
    /// on processes is its own to lower.
    const STARVED: &str = "OMORFOS_TEST_NO_THREADS";

    #[test]
    fn a_thread_is_started_and_its_work_done() {
        let started = start("work", || 6 * 7).unwrap();
        assert_eq!(started.join().unwrap(), 42);
    }

    #[test]
    fn a_thread_that_cannot_be_started_is_said_not_to_be_and_ends_nothing() {
        let name =
            "threads::tests::a_thread_that_cannot_be_started_is_said_not_to_be_and_ends_nothing";
        if std::env::var_os(STARVED).is_none() {
            let ran = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", name, "--nocapture"])
                .env(STARVED, "1")
                .output()
                .unwrap();
            let said = String::from_utf8_lossy(&ran.stdout);
            let err = String::from_utf8_lossy(&ran.stderr);
            assert!(ran.status.success(), "{said}{err}");
            assert!(said.contains("1 passed"), "{said}");
            assert!(err.contains("work: no thread could be started"), "{err}");
            return;
        }
        // No more processes than there are already, of this user's.
        let mut limit = getrlimit(Resource::Nproc);
        limit.current = Some(0);
        setrlimit(Resource::Nproc, limit).unwrap();
        assert!(start("work", || ()).is_err());
    }
}
