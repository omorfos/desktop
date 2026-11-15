//! The most files a program of the session may have open at once.
//!
//! A login allows 1024 descriptors at first, and many times that once
//! asked: `ulimit -Hn` is commonly 524288. The compositor keeps a
//! descriptor for each pool of memory a program shares with it, and the
//! relay one for each beside it, for every program at once; 1024 is soon
//! reached by one program that makes many, or by a long session of many
//! windows. So the compositor, the relay and the holder each ask for as
//! many as they may, as they start (ADR 93).

use rustix::process::{Resource, getrlimit, setrlimit};

/// Raises the limit on the descriptors this process may have open to the
/// most it may be raised to. What this process starts has it too.
pub fn raise() {
    let mut limit = getrlimit(Resource::Nofile);
    limit.current = limit.maximum;
    // One that cannot be raised is left as it was.
    let _ = setrlimit(Resource::Nofile, limit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_limit_on_descriptors_is_raised_as_far_as_it_may_be() {
        let mut lowered = getrlimit(Resource::Nofile);
        let most = lowered.maximum;
        lowered.current = Some(64);
        setrlimit(Resource::Nofile, lowered).unwrap();
        raise();
        assert_eq!(getrlimit(Resource::Nofile).current, most);
    }
}
