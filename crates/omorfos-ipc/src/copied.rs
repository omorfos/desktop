//! How many kinds a program may offer what it copied, selected or dragged
//! in, and how long each may be.
//!
//! The relay holds a program connected through it to these, and the
//! compositor one connected without it, as under the nested backend: both
//! take them from here, so the session does the same with the relay or
//! without it.

/// The most kinds a program may offer what it copied, selected or dragged
/// in: far more than any program offers. Every one is sent to whichever
/// program is given the keyboard, or dragged over, and more than its
/// socket holds would end it, with whatever was not saved in it.
pub const MOST_KINDS: usize = 256;

/// The longest a kind may be, in bytes: as long as a media type may be.
pub const LONGEST_KIND: usize = 255;
