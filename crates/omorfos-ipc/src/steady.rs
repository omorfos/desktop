//! A connection to the compositor's socket that outlasts the compositor.

use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::path::PathBuf;
use std::time::Duration;

use crate::client::Client;
use crate::message::{Reply, Request};

/// How long after one try the connection is tried again, while the
/// compositor is away: about as long as one takes to start.
pub const AGAIN: Duration = Duration::from_millis(100);

/// A connection to the compositor's socket that is made again once it has
/// closed, as it does when the compositor is started anew under the relay
/// (ADR 61). It never waits, and each time it is made, what is to be asked
/// first, such as [`Request::Subscribe`], is asked again. While the
/// compositor is away, what would be sent is not, as a compositor that has
/// gone does nothing it is asked.
///
/// One not told where the socket is cannot be made again: its closing is
/// the end of it, and is said to be.
///
/// Times are given by whoever has it, from any start, as a program counts
/// how long it has been running.
#[derive(Debug)]
pub struct Steady {
    socket: Option<PathBuf>,
    first: Option<Request>,
    client: Option<Client>,
    /// When it was last made, or tried.
    tried: Duration,
}

impl Steady {
    /// `client`, made never to wait, with `first` sent first, made again at
    /// `socket` once it closes, if there is one, at `now`.
    ///
    /// # Errors
    ///
    /// Returns an error if `client` cannot be made not to wait, or `first`
    /// cannot be sent.
    pub fn new(
        client: Client,
        first: Option<Request>,
        socket: Option<PathBuf>,
        now: Duration,
    ) -> io::Result<Self> {
        let ready = ready(client, first.as_ref());
        ready.map(|client| Self {
            socket,
            first,
            client: Some(client),
            tried: now,
        })
    }

    /// Hands each message that has come to `heard`, without waiting, at
    /// `now`, once the connection is made again if it is away and it is
    /// time to try.
    ///
    /// # Errors
    ///
    /// Returns an error if a connection that cannot be made again has
    /// closed or failed, or the compositor said what is not a message.
    pub fn hear(&mut self, now: Duration, mut heard: impl FnMut(Reply)) -> io::Result<()> {
        self.again(now);
        let read = self.client.as_mut().map_or(Ok(()), |client| {
            let mut said = std::iter::from_fn(|| client.try_receive().transpose());
            said.try_for_each(|reply| reply.map(&mut heard))
        });
        self.kept(read, now)
    }

    /// Sends `request` at `now`, if the connection is there.
    ///
    /// # Errors
    ///
    /// Returns an error if it cannot be written, and the connection cannot
    /// be made again.
    pub fn send(&mut self, request: &Request, now: Duration) -> io::Result<()> {
        let sent = self
            .client
            .as_mut()
            .map_or(Ok(()), |client| client.send(request));
        self.kept(sent, now)
    }

    /// How long from `now` until it is next tried, while it is away and can
    /// be made again: as long as a program waiting for anything else need
    /// wait at most.
    #[must_use]
    pub fn until(&self, now: Duration) -> Option<Duration> {
        self.deadline().map(|due| due.saturating_sub(now))
    }

    /// When it is next to be tried, while it is away and can be made again.
    fn deadline(&self) -> Option<Duration> {
        let away = self.client.is_none() && self.socket.is_some();
        away.then(|| self.tried + AGAIN)
    }

    /// The connection, to be waited on, while it is there.
    #[must_use]
    pub fn waited(&self) -> Option<BorrowedFd<'_>> {
        self.client.as_ref().map(AsFd::as_fd)
    }

    /// `done`, unless it failed and the connection can be made again: then
    /// it is let go, to be tried again from `now`.
    fn kept(&mut self, done: io::Result<()>, now: Duration) -> io::Result<()> {
        match done {
            Err(_) if self.socket.is_some() => {
                self.client = None;
                self.tried = now;
                Ok(())
            }
            done => done,
        }
    }

    /// Makes the connection again, if it is away and it is time to try, at
    /// `now`.
    fn again(&mut self, now: Duration) {
        let due = self.deadline().is_some_and(|due| due <= now);
        if let Some(socket) = self.socket.as_deref().filter(|_| due) {
            self.tried = now;
            let made =
                Client::connect(socket).and_then(|client| ready(client, self.first.as_ref()));
            self.client = made.ok();
        }
    }
}

/// `client`, made never to wait, with `first` sent.
fn ready(mut client: Client, first: Option<&Request>) -> io::Result<Client> {
    let sent = client
        .set_nonblocking(true)
        .and_then(|()| first.map_or(Ok(()), |first| client.send(first)));
    sent.map(|()| client)
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::{UnixListener, UnixStream};

    use super::*;
    use crate::message::encode;

    const NOW: Duration = Duration::from_secs(1);

    /// What `from` has been sent, as the lines that have come so far.
    fn asked(from: &UnixStream) -> String {
        let mut line = String::new();
        BufReader::new(from).read_line(&mut line).unwrap();
        line
    }

    /// Everything heard on `steady` at `now`.
    fn heard(steady: &mut Steady, now: Duration) -> io::Result<Vec<Reply>> {
        let mut heard = Vec::new();
        steady.hear(now, |reply| heard.push(reply)).map(|()| heard)
    }

    #[test]
    fn what_is_asked_first_is_asked_and_what_comes_is_heard_without_waiting() {
        let (ours, mut theirs) = UnixStream::pair().unwrap();
        let first = Some(Request::Subscribe);
        let mut steady = Steady::new(Client::from_stream(ours), first, None, NOW).unwrap();
        assert_eq!(asked(&theirs), encode(&Request::Subscribe));
        assert_eq!(heard(&mut steady, NOW).unwrap(), []);
        theirs.write_all(encode(&Reply::Done).as_bytes()).unwrap();
        assert_eq!(heard(&mut steady, NOW).unwrap(), [Reply::Done]);
        steady.send(&Request::Version, NOW).unwrap();
        assert_eq!(asked(&theirs), encode(&Request::Version));
        assert!(steady.waited().is_some());
        assert_eq!(steady.until(NOW), None, "nothing to try");
    }

    #[test]
    fn one_that_cannot_be_made_again_ends_when_it_closes() {
        let (ours, theirs) = UnixStream::pair().unwrap();
        let mut steady = Steady::new(Client::from_stream(ours), None, None, NOW).unwrap();
        drop(theirs);
        assert!(heard(&mut steady, NOW).is_err());
        assert!(steady.send(&Request::Version, NOW).is_err());
    }

    #[test]
    fn one_whose_socket_is_known_is_made_again_once_the_compositor_is_back() {
        let directory = tempfile::TempDir::new().unwrap();
        let socket = directory.path().join("omorfos.sock");
        let listening = UnixListener::bind(&socket).unwrap();
        let first = Some(Request::Subscribe);
        let client = Client::connect(&socket).unwrap();
        let mut steady = Steady::new(client, first, Some(socket.clone()), NOW).unwrap();
        let (compositor, _) = listening.accept().unwrap();
        assert_eq!(asked(&compositor), encode(&Request::Subscribe));

        // The compositor goes, and with it its socket.
        drop((compositor, listening));
        std::fs::remove_file(&socket).unwrap();
        assert_eq!(heard(&mut steady, NOW).unwrap(), [], "away, not ended");
        assert!(steady.waited().is_none());
        assert_eq!(
            steady.send(&Request::Version, NOW).ok(),
            Some(()),
            "dropped"
        );
        assert_eq!(steady.until(NOW), Some(AGAIN));
        // Not tried again before its time, and tried in vain while nothing
        // is there.
        let later = NOW + AGAIN;
        assert_eq!(heard(&mut steady, later).unwrap(), []);
        assert_eq!(
            steady.until(later + AGAIN * 2),
            Some(Duration::ZERO),
            "overdue"
        );

        // The next compositor is there: it is asked first what the one
        // before was, and heard.
        let listening = UnixListener::bind(&socket).unwrap();
        let soon = later + AGAIN / 2;
        assert_eq!(heard(&mut steady, soon).unwrap(), [], "not yet");
        let due = later + AGAIN;
        assert_eq!(heard(&mut steady, due).unwrap(), []);
        let (mut compositor, _) = listening.accept().unwrap();
        assert_eq!(asked(&compositor), encode(&Request::Subscribe));
        compositor
            .write_all(encode(&Reply::Done).as_bytes())
            .unwrap();
        assert_eq!(heard(&mut steady, due).unwrap(), [Reply::Done]);
        assert_eq!(steady.until(due), None);

        // What cannot be sent lets it go too.
        drop(compositor);
        let failed = (0..100).any(|_| {
            let _ = steady.send(&Request::Version, due);
            steady.waited().is_none()
        });
        assert!(failed);
    }
}
