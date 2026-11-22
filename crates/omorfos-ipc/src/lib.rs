//! The IPC protocol of the omorfos compositor, and a client for it.
//!
//! Panels, launchers, scripts and `omorfosctl` talk to the compositor over a
//! Unix socket. Each message is one line of JSON: a client sends a
//! [`Request`] and reads a [`Reply`]. After [`Request::Subscribe`] the
//! compositor keeps the connection open and sends a [`Reply::State`] every
//! time the state changes.
//!
//! The socket belongs to the user's session. The compositor creates it with
//! owner-only permissions and refuses peers running as anyone else.
//!
//! ```no_run
//! use omorfos_ipc::{Client, Reply, Request};
//!
//! let path = omorfos_ipc::socket_path_from_env().expect("is omorfos running?");
//! let mut client = Client::connect(&path)?;
//! if let Reply::State(state) = client.request(&Request::State)? {
//!     for window in state.windows {
//!         println!("{} {}", window.id, window.title);
//!     }
//! }
//! # Ok::<(), std::io::Error>(())
//! ```

pub mod actions;
mod client;
pub mod copied;
pub mod handing;
pub mod help;
pub mod hold;
pub mod limits;
pub mod manual;
mod message;
pub mod protocols;
pub mod relay;
pub mod seat;
pub mod session_log;
mod steady;
pub mod threads;

pub use client::{Client, socket_path, socket_path_from_env};
pub use message::{
    Awake, Drawing, LONGEST_COPY, MAX_MESSAGE, PROTOCOL, Placed, Radio, Reply, Request, Screen,
    Standing, State, Window, Workspace, decode, encode,
};
pub use steady::{AGAIN, Steady};

/// Waits, five seconds at most, for the other end of `ours` to be gone. A
/// connection's end is closed only as the last copy of its descriptor is,
/// and a test beside the one waiting that starts a program holds a copy
/// of every descriptor for a moment, as `docs/TESTING.md` says.
#[cfg(test)]
fn gone(ours: impl std::os::fd::AsFd) {
    use rustix::event::{PollFd, PollFlags, Timespec, poll};
    let mut ended = [PollFd::new(&ours, PollFlags::IN)];
    let soon = Timespec::try_from(std::time::Duration::from_secs(5)).ok();
    rustix::io::retry_on_intr(|| poll(&mut ended, soon.as_ref())).unwrap();
}
