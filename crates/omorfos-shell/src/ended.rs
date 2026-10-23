//! Telling the session ending from something gone wrong. The components
//! run until the session ends, when the compositor closes their
//! connection: that is how they are told, and nothing to say. The
//! compositor sending one away for breaking the protocol is not that, and
//! is said.

use std::error::Error;
use std::io;

use wayland_client::DispatchError;
use wayland_client::backend::WaylandError;

/// Whether `failure`, or what it came of, is the compositor having closed
/// the connection: the session has ended.
#[must_use]
pub fn ended(failure: &(dyn Error + 'static)) -> bool {
    let mut cause = Some(failure);
    while let Some(now) = cause {
        let wayland = |error: &WaylandError| match error {
            WaylandError::Io(error) => Some(closed(error)),
            WaylandError::Protocol(_) => Some(false),
        };
        let dispatched = now
            .downcast_ref::<DispatchError>()
            .and_then(|error| match error {
                DispatchError::Backend(error) => wayland(error),
                DispatchError::BadMessage { .. } => Some(false),
            });
        let said = dispatched
            .or_else(|| now.downcast_ref::<WaylandError>().and_then(wayland))
            .or_else(|| now.downcast_ref::<io::Error>().map(closed));
        if let Some(said) = said {
            return said;
        }
        cause = now.source();
    }
    false
}

/// What the session ending is, of a failure: nothing to say, and an end
/// as it should be; or the failure, to be said.
///
/// # Errors
///
/// Returns `failure` unless it is the session having ended.
pub fn quietly(failure: Failure) -> Result<(), Failure> {
    if ended(&*failure) {
        Ok(())
    } else {
        Err(failure)
    }
}

/// Why a component could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// Whether an error of reading or writing is the other end having gone.
fn closed(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset | io::ErrorKind::UnexpectedEof
    )
}

#[cfg(test)]
mod tests {
    use std::fmt;

    use wayland_client::backend::protocol::ProtocolError;

    use super::*;

    /// A failure that came of another.
    #[derive(Debug)]
    struct Wrapped(io::Error);

    impl fmt::Display for Wrapped {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("wrapped")
        }
    }

    impl Error for Wrapped {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.0)
        }
    }

    fn io(kind: io::ErrorKind) -> io::Error {
        io::Error::from(kind)
    }

    fn protocol() -> WaylandError {
        WaylandError::Protocol(ProtocolError {
            code: 1,
            object_id: 3,
            object_interface: "xdg_wm_base".to_owned(),
            message: "no".to_owned(),
        })
    }

    #[test]
    fn the_compositor_closing_the_connection_is_the_session_ending() {
        for kind in [
            io::ErrorKind::BrokenPipe,
            io::ErrorKind::ConnectionReset,
            io::ErrorKind::UnexpectedEof,
        ] {
            assert!(ended(&io(kind)));
            assert!(ended(&WaylandError::Io(io(kind))));
            assert!(ended(&DispatchError::Backend(WaylandError::Io(io(kind)))));
            assert!(ended(&Wrapped(io(kind))), "whatever it came of");
        }
    }

    #[test]
    fn anything_else_is_something_gone_wrong() {
        assert!(!ended(&io(io::ErrorKind::PermissionDenied)));
        assert!(!ended(&protocol()), "sent away for breaking the protocol");
        assert!(!ended(&DispatchError::Backend(protocol())));
        let unheard = DispatchError::BadMessage {
            sender_id: wayland_client::backend::ObjectId::null(),
            interface: "wl_display",
            opcode: 9,
        };
        assert!(!ended(&unheard));
        let said: Box<dyn Error> = "it went wrong".into();
        assert!(!ended(&*said));
    }

    #[test]
    fn the_session_ending_is_an_end_as_it_should_be_and_anything_else_is_said() {
        let gone: Failure = Box::new(io(io::ErrorKind::BrokenPipe));
        assert!(quietly(gone).is_ok());
        let wrapped: Failure = Box::new(Wrapped(io(io::ErrorKind::PermissionDenied)));
        assert_eq!(quietly(wrapped).unwrap_err().to_string(), "wrapped");
        let wrong: Failure = "it went wrong".into();
        assert_eq!(quietly(wrong).unwrap_err().to_string(), "it went wrong");
    }
}
