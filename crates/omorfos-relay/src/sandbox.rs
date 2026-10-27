//! Sandboxes made through the relay (ADR 61, step 3d).
//!
//! A program such as Flatpak makes a sandbox by handing over a socket it
//! has made and listens on, with a descriptor whose closing ends the
//! sandbox (`wp_security_context_v1`): whatever connects at that socket is
//! inside the sandbox. Handed to the compositor, the socket would be the
//! compositor's to serve, and every program that connected there would end
//! with it. So the relay answers for the sandbox being made, and the
//! compositor is never told of it: the relay listens on the socket itself,
//! and each program that connects there is relayed like any other, kept
//! through a change of compositor, and handed to each compositor as one
//! inside a sandbox. A program inside one is not offered the way to make
//! one, so it cannot make itself another.
//!
//! What a sandbox is said to be, its engine, its program and its instance,
//! is not kept: the compositor takes nothing from it, and a program inside
//! is held to the same whatever it is.

use std::collections::HashMap;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::rc::Rc;

use rustix::io::ioctl_fionbio;
use rustix::net::{SocketFlags, accept_with};
use wayland_backend::protocol::MessageDesc;

use crate::objects::Read;

/// What makes a sandbox, and the sandbox being made.
const MANAGER: &str = "wp_security_context_manager_v1";
const CONTEXT: &str = "wp_security_context_v1";

/// A sandbox: the socket listened on, and the descriptor whose closing
/// ends it.
#[derive(Debug)]
pub struct Sandbox {
    listening: Rc<OwnedFd>,
    closing: Rc<OwnedFd>,
}

impl Sandbox {
    /// The socket programs inside it connect to.
    #[must_use]
    pub fn listening(&self) -> BorrowedFd<'_> {
        self.listening.as_fd()
    }

    /// The descriptor that is readable, or hung up, once the sandbox has
    /// ended.
    #[must_use]
    pub fn closing(&self) -> BorrowedFd<'_> {
        self.closing.as_fd()
    }

    /// Each program waiting to connect inside it, without waiting for one.
    pub fn waiting(&self) -> impl Iterator<Item = UnixStream> + '_ {
        let flags = SocketFlags::CLOEXEC | SocketFlags::NONBLOCK;
        std::iter::from_fn(move || {
            accept_with(&*self.listening, flags)
                .ok()
                .map(UnixStream::from)
        })
    }
}

/// The sandboxes a program is making, and those it has made since they were
/// last taken.
#[derive(Debug, Default)]
pub struct Sandboxes {
    /// By the program's number for the sandbox being made.
    making: HashMap<u32, Sandbox>,
    made: Vec<Sandbox>,
}

impl Sandboxes {
    /// Takes in `read`, the program's request `described` to `object`,
    /// with `fds`, if it is one of making a sandbox. Returns whether it
    /// was: one is the relay's to answer, and not passed on.
    pub fn request(
        &mut self,
        object: u32,
        read: &Read,
        described: &MessageDesc,
        fds: &[Rc<OwnedFd>],
    ) -> bool {
        match (read.interface.name, described.name) {
            (MANAGER, "create_listener") => {
                let made = read.made.map(|(made, ..)| made);
                let given = fds.first().zip(fds.get(1));
                let making = made.zip(given).map(|(made, (listening, closing))| {
                    let sandbox = Sandbox {
                        listening: Rc::clone(listening),
                        closing: Rc::clone(closing),
                    };
                    self.making.insert(made, sandbox);
                });
                let _ = making;
                true
            }
            (CONTEXT, "commit") => {
                // Made once; what is asked of it after that is passed over.
                let made = self.making.remove(&object);
                // Listened on without waiting, as the compositor would. One
                // that will not be is found out by a program never let in.
                if let Some(made) = &made {
                    let _ = ioctl_fionbio(&*made.listening, true);
                }
                self.made.extend(made);
                true
            }
            (CONTEXT, _) => {
                if read.ends {
                    self.making.remove(&object);
                }
                true
            }
            _ => false,
        }
    }

    /// The sandboxes made since they were last taken.
    pub fn made(&mut self) -> Vec<Sandbox> {
        std::mem::take(&mut self.made)
    }
}
