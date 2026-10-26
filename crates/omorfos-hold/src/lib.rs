//! Holds an omorfos session while its compositor is started again
//! (ADR 60).
//!
//! The login screen's `omorfos` becomes `omorfos-hold`, which binds the
//! socket programs connect to and starts the compositor with it, as
//! `omorfos --held`. When the compositor ends, because it crashed or
//! stepped down for a new version, the holder starts it again under the
//! same name, telling it what the one before left: whether the session was
//! locked, which number its X server had, and that what starts with the
//! session has been started. It decides nothing else.

mod exec;
mod policy;
pub mod renew;
mod socket;

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Instant;

use omorfos_ipc::hold::{self, Heard};

pub use exec::give_back;
pub use policy::{Next, Policy, Which};
pub use socket::Socket;

/// What the holder holds for the session.
#[derive(Debug)]
pub struct Hold {
    socket: Socket,
    /// Where the compositor is installed.
    compositor: PathBuf,
    /// The file of the compositor that last came up, held open so that it
    /// can be started again if a new version installed will not.
    held: Option<File>,
    policy: Policy,
    /// Where the holder itself is installed, where it takes up its own new
    /// versions.
    own: Option<PathBuf>,
}

/// What the compositor running is, as far as the holder is concerned.
struct Running {
    child: Child,
    connection: UnixStream,
    file: File,
}

impl Hold {
    /// Holds `socket` for the compositor installed at `compositor`, as
    /// `policy` says.
    #[must_use]
    pub const fn new(socket: Socket, compositor: PathBuf, policy: Policy) -> Self {
        Self {
            socket,
            compositor,
            held: None,
            policy,
            own: None,
        }
    }

    /// Takes up the holder's own new versions installed at `own`.
    #[must_use]
    pub fn renewed_from(self, own: PathBuf) -> Self {
        Self {
            own: Some(own),
            ..self
        }
    }

    /// Starts the compositor, and again each time it ends as it should be
    /// started again, until the session ends; says why it started one
    /// again on `log`. Returns whether the session was given up.
    pub fn run(&mut self, log: &mut dyn Write) -> bool {
        self.run_from(Which::Installed, log)
    }

    /// Runs as [`Hold::run`] does, starting the compositor from `which`
    /// file first, as another version of the holder said to.
    pub fn run_from(&mut self, mut which: Which, log: &mut dyn Write) -> bool {
        loop {
            match self.once(which, log) {
                Next::Start { which: next, after } => {
                    std::thread::sleep(after);
                    which = next;
                    // Between one compositor and the next, nothing on the
                    // screen is the holder's, and its own new version is
                    // taken up.
                    exec::renew(self, which, log);
                }
                Next::End { failed } => return failed,
            }
        }
    }

    /// Starts the compositor from `which` file, hears it until it ends,
    /// and says what is next.
    fn once(&mut self, which: Which, log: &mut dyn Write) -> Next {
        let told = self.policy.starting(which);
        let failed = match self.start(which, &told.written()) {
            Ok(running) => self.hear(running),
            Err(error) => {
                let _ = writeln!(log, "omorfos-hold: cannot start the compositor: {error}");
                true
            }
        };
        let next = self.policy.ended(failed, Instant::now(), self.other());
        let said = match next {
            Next::Start {
                which: Which::Held, ..
            } if which == Which::Installed => {
                "the version installed keeps failing; starting the one before it"
            }
            Next::Start { .. } if failed => "the compositor failed; starting it again",
            Next::Start { .. } => "starting the compositor again, as it asked",
            Next::End { failed: true } => "the compositor keeps failing; ending the session",
            Next::End { failed: false } => "the session has ended",
        };
        let _ = writeln!(log, "omorfos-hold: {said}");
        next
    }

    /// Starts the compositor from `which` file, handing it the socket and
    /// telling it `told`.
    fn start(&self, which: Which, told: &str) -> io::Result<Running> {
        let file = match which {
            Which::Installed => File::open(&self.compositor),
            Which::Held => {
                let none = Err(io::ErrorKind::NotFound.into());
                self.held.as_ref().map_or(none, File::try_clone)
            }
        }?;
        // Started from the file held, by the holder's own name for it,
        // which stays good however the file installed is replaced.
        let program = format!("/proc/{}/fd/{}", std::process::id(), file.as_raw_fd());
        let pair = UnixStream::pair();
        let started = pair.and_then(|(connection, theirs)| {
            Command::new(program)
                .arg0("omorfos")
                .arg("--held")
                .stdin(OwnedFd::from(theirs))
                // Whatever the holder was started in, the compositor drives
                // the screen, and has no session to connect to.
                .env_remove("WAYLAND_DISPLAY")
                .env_remove("WAYLAND_SOCKET")
                .env_remove("DISPLAY")
                .spawn()
                .map(|child| (connection, child))
        });
        started.map(|(connection, child)| {
            // One that has gone already cannot be handed anything, and is
            // heard to have ended.
            let _ = hold::give(&connection, told, &[self.socket.as_fd()]);
            Running {
                child,
                connection,
                file,
            }
        })
    }

    /// Hears what the compositor running says until it ends, and keeps its
    /// file if it came up. Returns whether it failed.
    fn hear(&mut self, mut running: Running) -> bool {
        let mut heard = Heard::default();
        let mut bytes = [0; 256];
        // The connection closes when the compositor ends: nothing it starts
        // has it.
        while let Ok(read @ 1..) = running.connection.read(&mut bytes) {
            for word in heard.hear(&bytes[..read]) {
                self.policy.heard(word);
            }
        }
        let status = running.child.wait();
        if self.policy.is_up() {
            self.held = Some(running.file);
        }
        status.map_or(true, |status| !status.success())
    }

    /// Whether the file held open is another than the one installed now.
    fn other(&self) -> bool {
        let identity = |metadata: std::fs::Metadata| (metadata.dev(), metadata.ino());
        let held = self
            .held
            .as_ref()
            .map(|held| held.metadata().map(identity).ok());
        let installed = std::fs::metadata(&self.compositor).map(identity).ok();
        held.is_some_and(|held| held != installed)
    }
}

/// Holds the session in `runtime`, the user's runtime directory, for the
/// compositor installed beside the holder, which is installed at `own`,
/// until it ends. Returns whether it was given up.
///
/// # Errors
///
/// Returns an error if no socket can be bound for it.
pub fn hold(runtime: &Path, own: PathBuf, log: &mut dyn Write) -> io::Result<bool> {
    let socket = Socket::bind(runtime)?;
    let compositor = own.with_file_name("omorfos");
    let hold = Hold::new(socket, compositor, Policy::default());
    Ok(hold.renewed_from(own).run(log))
}
