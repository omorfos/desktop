//! A blocking client for the socket.

use std::env;
use std::ffi::OsStr;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use crate::message::{MAX_MESSAGE, Reply, Request, decode, encode};

/// Works out where the compositor's socket is.
///
/// `explicit` is the value of `OMORFOS_SOCKET`, which the compositor sets
/// for the programs it starts and which wins if present. Otherwise the
/// socket is `omorfos-<display>.sock` in `runtime_dir` (`XDG_RUNTIME_DIR`),
/// where `<display>` is `wayland_display` (`WAYLAND_DISPLAY`). Returns `None`
/// if there is not enough to go on.
///
/// The variables are passed in so that the rule can be used and tested
/// without touching the environment; see [`socket_path_from_env`].
#[must_use]
pub fn socket_path(
    explicit: Option<&OsStr>,
    runtime_dir: Option<&OsStr>,
    wayland_display: Option<&OsStr>,
) -> Option<PathBuf> {
    let derived = || {
        let mut name = OsStr::new("omorfos-").to_owned();
        name.push(wayland_display?);
        name.push(".sock");
        Some(Path::new(runtime_dir?).join(name))
    };
    explicit.map(PathBuf::from).or_else(derived)
}

/// [`socket_path`] applied to the process's environment.
#[must_use]
pub fn socket_path_from_env() -> Option<PathBuf> {
    socket_path(
        env::var_os("OMORFOS_SOCKET").as_deref(),
        env::var_os("XDG_RUNTIME_DIR").as_deref(),
        env::var_os("WAYLAND_DISPLAY").as_deref(),
    )
}

/// A connection to the compositor.
#[derive(Debug)]
pub struct Client {
    stream: BufReader<UnixStream>,
}

impl Client {
    /// Connects to the socket at `path`.
    ///
    /// # Errors
    ///
    /// Returns an error if nothing is listening there or the connection is
    /// refused.
    pub fn connect(path: &Path) -> io::Result<Self> {
        UnixStream::connect(path).map(Self::from_stream)
    }

    /// Wraps a stream that is already connected.
    #[must_use]
    pub fn from_stream(stream: UnixStream) -> Self {
        Self {
            stream: BufReader::new(stream),
        }
    }

    /// Sends a request without waiting for the answer.
    ///
    /// # Errors
    ///
    /// Returns an error if the request cannot be written.
    pub fn send(&mut self, request: &Request) -> io::Result<()> {
        self.stream.get_mut().write_all(encode(request).as_bytes())
    }

    /// Waits for the next message from the compositor: the answer to a
    /// request, or the next update after [`Request::Subscribe`].
    ///
    /// # Errors
    ///
    /// Returns an error if the connection is closed or fails, or if the
    /// compositor sends something that is not a [`Reply`].
    pub fn receive(&mut self) -> io::Result<Reply> {
        let mut line = String::new();
        let limit = u64::try_from(MAX_MESSAGE).unwrap_or(u64::MAX);
        let read = self.stream.by_ref().take(limit).read_line(&mut line);
        read.and_then(|length| {
            if length == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the compositor closed the connection",
                ));
            }
            decode(&line).map_err(|problem| io::Error::new(io::ErrorKind::InvalidData, problem))
        })
    }

    /// Sends a request and waits for its answer.
    ///
    /// # Errors
    ///
    /// As for [`Client::send`] and [`Client::receive`].
    pub fn request(&mut self, request: &Request) -> io::Result<Reply> {
        self.send(request).and_then(|()| self.receive())
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::thread;

    use super::*;

    fn pair() -> (Client, UnixStream) {
        let (ours, theirs) = UnixStream::pair().unwrap();
        (Client::from_stream(ours), theirs)
    }

    #[test]
    fn the_socket_given_explicitly_wins() {
        let path = socket_path(
            Some(OsStr::new("/tmp/custom.sock")),
            Some(OsStr::new("/run/user/1000")),
            Some(OsStr::new("wayland-1")),
        );
        assert_eq!(path, Some(PathBuf::from("/tmp/custom.sock")));
    }

    #[test]
    fn otherwise_the_socket_is_named_after_the_wayland_display() {
        let path = socket_path(
            None,
            Some(OsStr::new("/run/user/1000")),
            Some(OsStr::new("wayland-1")),
        );
        assert_eq!(
            path,
            Some(PathBuf::from("/run/user/1000/omorfos-wayland-1.sock"))
        );
    }

    #[test]
    fn without_a_runtime_directory_or_display_there_is_no_socket_to_find() {
        assert_eq!(socket_path(None, None, Some(OsStr::new("wayland-1"))), None);
        assert_eq!(
            socket_path(None, Some(OsStr::new("/run/user/1000")), None),
            None
        );
    }

    #[test]
    fn the_environment_is_consulted_for_the_socket() {
        // Whatever the environment holds, this agrees with the pure rule.
        let expected = socket_path(
            env::var_os("OMORFOS_SOCKET").as_deref(),
            env::var_os("XDG_RUNTIME_DIR").as_deref(),
            env::var_os("WAYLAND_DISPLAY").as_deref(),
        );
        assert_eq!(socket_path_from_env(), expected);
    }

    #[test]
    fn a_request_is_sent_as_one_line_and_its_answer_read_back() {
        let (mut client, mut compositor) = pair();
        compositor.write_all(b"\"done\"\n").unwrap();

        assert_eq!(client.request(&Request::Reload).unwrap(), Reply::Done);

        let mut sent = [0; 9];
        compositor.read_exact(&mut sent).unwrap();
        assert_eq!(&sent, b"\"reload\"\n");
    }

    #[test]
    fn updates_are_received_one_at_a_time() {
        let (mut client, mut compositor) = pair();
        compositor
            .write_all(b"\"done\"\n{\"error\":{\"message\":\"no\"}}\n")
            .unwrap();
        assert_eq!(client.receive().unwrap(), Reply::Done);
        assert_eq!(
            client.receive().unwrap(),
            Reply::Error {
                message: "no".to_owned()
            }
        );
    }

    #[test]
    fn a_closed_connection_is_an_error() {
        let (mut client, compositor) = pair();
        drop(compositor);
        assert_eq!(
            client.receive().unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        assert!(client.request(&Request::Version).is_err());
    }

    #[test]
    fn nonsense_from_the_compositor_is_an_error() {
        let (mut client, mut compositor) = pair();
        compositor.write_all(b"definitely not json\n").unwrap();
        assert_eq!(
            client.receive().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn an_endless_line_is_cut_off_rather_than_read_forever() {
        let (mut client, mut compositor) = pair();
        let writer = thread::spawn(move || {
            // More than the limit, and never a newline. The write may fail
            // once the client gives up; that is fine.
            let _ = compositor.write_all(&vec![b'x'; MAX_MESSAGE * 2]);
        });
        assert_eq!(
            client.receive().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
        drop(client);
        writer.join().unwrap();
    }

    #[test]
    fn connecting_reaches_a_listening_socket() {
        let directory = tempfile::TempDir::new().unwrap();
        let path = directory.path().join("omorfos.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 10];
            stream.read_exact(&mut request).unwrap();
            stream
                .write_all(
                    encode(&Reply::Version {
                        protocol: 1,
                        compositor: "t".to_owned(),
                    })
                    .as_bytes(),
                )
                .unwrap();
        });

        let mut client = Client::connect(&path).unwrap();
        let reply = client.request(&Request::Version).unwrap();
        assert_eq!(
            reply,
            Reply::Version {
                protocol: 1,
                compositor: "t".to_owned()
            }
        );
        server.join().unwrap();
    }

    #[test]
    fn connecting_to_nothing_fails() {
        let directory = tempfile::TempDir::new().unwrap();
        assert!(Client::connect(&directory.path().join("absent.sock")).is_err());
    }
}
