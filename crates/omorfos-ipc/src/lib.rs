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

mod client;
pub mod hold;
mod message;

pub use client::{Client, socket_path, socket_path_from_env};
pub use message::{
    Awake, MAX_MESSAGE, PROTOCOL, Placed, Radio, Reply, Request, Screen, Standing, State, Window,
    Workspace, decode, encode,
};
