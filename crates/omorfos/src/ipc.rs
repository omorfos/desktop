//! The server side of the IPC socket.
//!
//! Peers are other programs of the same user: panels, scripts, `omorfosctl`.
//! They are still treated as untrusted in one respect that matters to a
//! compositor: none of them may make it wait or grow without bound. All
//! socket I/O is non-blocking, messages and buffers are capped, and a peer
//! that breaks a rule is simply dropped.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omorfos_config::{Action, OfferedMode};
use omorfos_core::WindowId;
use omorfos_ipc::{
    MAX_MESSAGE, PROTOCOL, Placed, Reply, Request, State, Window, Workspace, decode, encode,
};
use serde::Deserialize;
use smithay::reexports::calloop::RegistrationToken;
use smithay::reexports::rustix::net::sockopt::socket_peercred;
use smithay::utils::Rectangle;

use crate::{Compositor, bridge};

/// The most peers served at once. Further connections are refused.
const MAX_PEERS: usize = 32;

/// The most unsent output held for one peer, in bytes. A peer that falls
/// further behind than this is not reading, and is dropped.
const MAX_BACKLOG: usize = 1024 * 1024;

/// How long to wait before trying again to send a peer what its socket
/// would not take.
const RETRY: Duration = Duration::from_millis(20);

/// The listening socket and the peers connected to it.
#[derive(Debug)]
pub(crate) struct Server {
    listener: UnixListener,
    path: PathBuf,
    /// The user who may connect: the owner of the socket file.
    owner: u32,
    peers: Vec<Peer>,
    /// The state last sent to subscribers.
    published: Option<State>,
}

/// What a peer that is sent the state whenever it changes wants it for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Follows {
    /// To know it.
    State,
    /// To show the user what is wrong, so that the compositor need not.
    Problems,
}

#[derive(Debug)]
struct Peer {
    stream: UnixStream,
    /// Bytes received that do not yet end in a newline.
    inbox: Vec<u8>,
    /// Bytes still to be sent.
    outbox: Vec<u8>,
    /// Whether it is sent the state whenever that changes, and what for.
    follows: Option<Follows>,
    /// Subscribed, but not yet sent the state for the first time.
    waiting: bool,
    alive: bool,
    /// What wakes the event loop when the peer writes, to be removed with
    /// the peer.
    wake: Option<RegistrationToken>,
}

impl Server {
    /// Starts listening at `path`, replacing a socket left behind by an
    /// earlier run. The socket is readable and writable by its owner only.
    fn bind(path: PathBuf) -> io::Result<Self> {
        // A leftover file would make the bind fail. If there is none, the
        // removal fails instead, harmlessly.
        let _ = fs::remove_file(&path);
        let owner_only = fs::Permissions::from_mode(0o600);
        UnixListener::bind(&path)
            .and_then(|listener| fs::set_permissions(&path, owner_only).map(|()| listener))
            .and_then(|listener| listener.set_nonblocking(true).map(|()| listener))
            .and_then(|listener| fs::metadata(&path).map(|socket| (listener, socket.uid())))
            .map(|(listener, owner)| Self {
                listener,
                path,
                owner,
                peers: Vec::new(),
                published: None,
            })
    }

    /// Takes on the connections waiting, up to the limit, turning away any
    /// that are not from the socket's owner. `wake` arranges for the event
    /// loop to be woken when a peer that was taken on writes.
    fn accept(&mut self, wake: impl Fn(&UnixStream) -> Option<RegistrationToken>) {
        while let Ok((stream, _)) = self.listener.accept() {
            let trusted = same_user(
                socket_peercred(&stream).ok().map(|peer| peer.uid.as_raw()),
                self.owner,
            );
            let room = self.peers.len() < MAX_PEERS;
            let ready = stream.set_nonblocking(true).is_ok();
            self.peers.extend((trusted && room && ready).then(|| Peer {
                wake: wake(&stream),
                stream,
                inbox: Vec::new(),
                outbox: Vec::new(),
                follows: None,
                waiting: false,
                alive: true,
            }));
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Whether a peer may use the socket: only if its user is known and is the
/// owner.
fn same_user(peer: Option<u32>, owner: u32) -> bool {
    peer == Some(owner)
}

impl Peer {
    /// Reads whatever has arrived and returns the complete lines.
    ///
    /// Lines are taken out as they arrive, so that no more than one message
    /// and one read is ever held, however fast the peer writes.
    fn read_lines(&mut self) -> Vec<Vec<u8>> {
        let mut chunk = [0; 4096];
        let mut lines = Vec::new();
        while self.alive {
            match self.stream.read(&mut chunk) {
                Ok(length) if length > 0 => {
                    self.inbox.extend_from_slice(&chunk[..length]);
                    self.take_lines(&mut lines);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                // The peer closed the connection, or it failed.
                _ => self.alive = false,
            }
        }
        lines
    }

    /// Moves the complete lines out of the inbox. A line longer than a
    /// message may be ends the connection, whether or not it is finished.
    fn take_lines(&mut self, lines: &mut Vec<Vec<u8>>) {
        while let Some(end) = self.inbox.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.inbox.drain(..=end).collect();
            self.alive &= line.len() <= MAX_MESSAGE;
            lines.extend(self.alive.then_some(line));
        }
        // What is left is the start of a line, with its newline still to
        // come. One this long is not a message, and waiting for its end
        // would mean buffering it.
        self.alive &= self.inbox.len() < MAX_MESSAGE;
    }

    fn queue(&mut self, reply: &Reply) {
        self.outbox.extend_from_slice(encode(reply).as_bytes());
    }

    /// Sends as much queued output as the socket will take right now.
    fn flush(&mut self) {
        match self.stream.write(&self.outbox) {
            Ok(sent) => drop(self.outbox.drain(..sent)),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
            Err(_) => self.alive = false,
        }
        self.alive &= self.outbox.len() <= MAX_BACKLOG;
    }
}

impl Compositor {
    /// Starts serving the IPC protocol on a socket at `path`.
    ///
    /// Programs started by the compositor are told where it is through the
    /// `OMORFOS_SOCKET` environment variable.
    ///
    /// # Errors
    ///
    /// Returns an error if the socket cannot be created.
    pub fn listen_ipc(&mut self, path: impl Into<PathBuf>) -> io::Result<()> {
        Server::bind(path.into()).map(|server| {
            // Lasts as long as the session: the socket is never replaced.
            self.wake_on(&server.listener);
            self.ipc = Some(server);
        })
    }

    /// Whether a program is connected that shows the user what is wrong.
    pub(crate) fn problems_shown_elsewhere(&self) -> bool {
        let peers = self.ipc.iter().flat_map(|server| &server.peers);
        let mut showing = peers.filter(|peer| peer.follows == Some(Follows::Problems));
        showing.any(|peer| peer.alive)
    }

    /// Where the IPC socket is, if one is being served.
    pub(crate) fn ipc_path(&self) -> Option<&Path> {
        self.ipc.as_ref().map(|server| server.path.as_path())
    }

    /// Accepts new peers and answers the requests that have arrived.
    pub(crate) fn serve_ipc(&mut self) {
        // The server is moved out while requests are answered, because
        // answering needs the whole compositor.
        let Some(mut server) = self.ipc.take() else {
            return;
        };
        server.accept(|stream| self.wake_on(stream));
        for peer in &mut server.peers {
            for line in peer.read_lines() {
                let request = String::from_utf8(line).map_err(|error| error.to_string());
                match request.and_then(|line| decode::<Request>(&line)) {
                    // Answered when the state is next published, moments
                    // from now, so that it is sent once and not twice.
                    Ok(Request::Subscribe) => {
                        peer.follows = peer.follows.or(Some(Follows::State));
                        peer.waiting = true;
                    }
                    Ok(Request::ShowProblems) => {
                        peer.follows = Some(Follows::Problems);
                        peer.waiting = true;
                    }
                    Ok(request) => peer.queue(&self.answer(request)),
                    Err(message) => peer.queue(&Reply::Error { message }),
                }
            }
        }
        self.ipc = Some(server);
    }

    /// Sends subscribers the state if it changed, and sends everyone what
    /// is queued for them. Peers that have gone or misbehaved are dropped.
    ///
    /// A peer whose socket would not take everything is tried again a
    /// little after `now`.
    pub(crate) fn publish_ipc(&mut self, now: Instant) {
        let Some(mut server) = self.ipc.take() else {
            return;
        };
        let state = self.ipc_state();
        let changed = server.published.as_ref() != Some(&state);
        let update = Reply::State(state.clone());
        for peer in server
            .peers
            .iter_mut()
            .filter(|peer| peer.follows.is_some())
        {
            if changed || peer.waiting {
                peer.queue(&update);
                peer.waiting = false;
            }
        }
        server.published = Some(state);
        for peer in &mut server.peers {
            peer.flush();
        }
        let gone = server.peers.iter().filter(|peer| !peer.alive);
        gone.filter_map(|peer| peer.wake)
            .for_each(|wake| self.event_loop.remove(wake));
        server.peers.retain(|peer| peer.alive);
        let behind = server.peers.iter().any(|peer| !peer.outbox.is_empty());
        self.ipc_retry = behind.then(|| now + RETRY);
        self.ipc = Some(server);
    }

    fn answer(&mut self, request: Request) -> Reply {
        match request {
            Request::Version => Reply::Version {
                protocol: PROTOCOL,
                compositor: env!("CARGO_PKG_VERSION").to_owned(),
            },
            // A subscription is answered in `publish_ipc`; should one get
            // here it is treated as asking for the state once.
            Request::State | Request::Subscribe | Request::ShowProblems => {
                Reply::State(self.ipc_state())
            }
            Request::Placed => Reply::Placed(self.placed()),
            Request::Reload => {
                self.reload_config();
                Reply::Done
            }
            Request::Perform { action } => match Action::deserialize(action) {
                Ok(action) => match self.objection(&action) {
                    None => {
                        self.perform(&action);
                        Reply::Done
                    }
                    Some(message) => Reply::Error { message },
                },
                Err(error) => Reply::Error {
                    message: format!("not an action: {error}"),
                },
            },
            Request::Activate { window } => {
                let id = WindowId::from_raw(window);
                if self.desktop.window(id).is_some() {
                    self.desktop.activate(id);
                    Reply::Done
                } else {
                    Reply::Error {
                        message: format!("there is no window {window}"),
                    }
                }
            }
        }
    }

    /// Why an action cannot be carried out, if it names something that does
    /// not exist. A key binding doing the same is quietly ignored; a peer
    /// is told, because it has no screen to see that nothing happened.
    fn objection(&self, action: &Action) -> Option<String> {
        match action {
            // One past the last is made when asked for.
            Action::Workspace(number) | Action::MoveToWorkspace(number) => {
                (bridge::workspace_index(*number) > self.desktop.workspace_count())
                    .then(|| format!("there is no workspace {}", number.get()))
            }
            Action::JoinWorkspace(id) => {
                let mut places = self.desktop.workspaces();
                let roomy = places.any(|place| place.id.get() == *id && place.room);
                (!roomy).then(|| format!("no workspace {id} has room for a window"))
            }
            _ => None,
        }
    }

    /// Where each window that can be seen is on each screen it is on, the
    /// one on top first: those over the panels, then the rest.
    fn placed(&self) -> Vec<Placed> {
        let mut top_first: Vec<_> = self.scene.iter().rev().collect();
        top_first.sort_by_key(|shown| !shown.over);
        // Nothing of a window rolled up into its titlebar is to be seen.
        let seen = top_first.into_iter().filter(|shown| !shown.shaded);
        let pixels = |units: i32| u32::try_from(units).unwrap_or_default();
        let mut placed = Vec::new();
        for shown in seen {
            let area = Rectangle::new(shown.origin, shown.window.geometry().size);
            let on = self.heads.iter().filter_map(|head| {
                let screen = head.area();
                let part = area.intersection(screen)?;
                let from = part.loc - screen.loc;
                let scale = head.scale;
                Some(Placed {
                    id: shown.id.raw(),
                    handle: self.handle_of(shown.id),
                    output: head.output.name(),
                    x: pixels(scale.pixels(from.x)),
                    y: pixels(scale.pixels(from.y)),
                    width: pixels(scale.pixels(part.size.w)),
                    height: pixels(scale.pixels(part.size.h)),
                })
            });
            placed.extend(on);
        }
        placed
    }

    /// The screens, in the order they stand, and the ways each can be
    /// driven, written as the configuration writes a mode.
    fn ipc_screens(&self) -> Vec<omorfos_ipc::Screen> {
        let written =
            |mode: Option<&OfferedMode>| mode.map(ToString::to_string).unwrap_or_default();
        let screens = self.arranged().into_iter().map(|index| {
            let head = &self.heads[index];
            let area = head.area();
            let mut modes = head.modes.clone();
            let largest = |mode: &OfferedMode| {
                let pixels = u32::from(mode.width) * u32::from(mode.height);
                std::cmp::Reverse((pixels, mode.width, mode.refresh))
            };
            modes.sort_by_key(largest);
            modes.dedup_by_key(|mode| (mode.width, mode.height, mode.refresh));
            omorfos_ipc::Screen {
                name: head.output.name(),
                x: area.loc.x,
                y: area.loc.y,
                width: area.size.w.unsigned_abs(),
                height: area.size.h.unsigned_abs(),
                mode: written(head.mode.as_ref()),
                preferred: written(head.modes.iter().find(|mode| mode.preferred)),
                modes: modes.iter().map(ToString::to_string).collect(),
            }
        });
        screens.collect()
    }

    fn ipc_state(&self) -> State {
        let number = |index: usize| u32::try_from(index + 1).unwrap_or(u32::MAX);
        // The desktop counts outputs in the order they were added, which is
        // the order they are kept in here.
        let output = |index: usize| {
            let head = self.heads.get(index);
            head.map(|head| head.output.name()).unwrap_or_default()
        };
        // What a key would count to: desktops have numbers, and a
        // workspace made for a fullscreen window has none.
        let places = self.desktop.workspaces();
        let numbers: HashMap<(usize, usize), u32> = places
            .filter_map(|place| Some(((place.output, place.index), number(place.desktop?))))
            .collect();
        State {
            workspaces: self
                .desktop
                .workspaces()
                .map(|place| Workspace {
                    id: place.id.get(),
                    output: output(place.output),
                    number: place.desktop.map_or(0, number),
                    name: self.workspace_name(&place),
                    active: place.shown,
                    kept: place.kept,
                    windows: place.tiles.iter().map(|id| id.raw()).collect(),
                    room: place.room,
                })
                .collect(),
            windows: self
                .desktop
                .windows()
                .map(|listed| Window {
                    id: listed.window.id.raw(),
                    app_id: listed.window.app_id.clone(),
                    title: listed.window.title.clone(),
                    output: output(listed.output),
                    workspace: numbers
                        .get(&(listed.output, listed.workspace))
                        .copied()
                        .unwrap_or_default(),
                    focused: listed.focused,
                    shown: listed.shown,
                    minimized: listed.minimized,
                    standing: omorfos_ipc::Standing {
                        shaded: listed.standing.shaded,
                        above: listed.standing.above,
                        sticky: listed.standing.sticky,
                    },
                    handle: self.handle_of(listed.window.id),
                })
                .collect(),
            problems: self.problems().to_vec(),
            events: self.events().map(str::to_owned).collect(),
            screens: self.ipc_screens(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_owner_of_the_socket_may_use_it() {
        assert!(same_user(Some(1000), 1000));
        assert!(!same_user(Some(1001), 1000), "another user");
        assert!(
            !same_user(Some(0), 1000),
            "not even root is assumed to be welcome"
        );
        assert!(!same_user(None, 1000), "an unidentified peer");
    }
}
