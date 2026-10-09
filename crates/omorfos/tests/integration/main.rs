//! End-to-end tests: a real Wayland client talks to the compositor over a
//! socket, and the compositor draws with the software renderer.
//!
//! Everything runs on one thread. The tests step the client and the
//! compositor by hand, so each exchange is deterministic.

mod actions;
mod clipboard;
mod common;
mod config;
mod decorations;
mod dragging;
mod executable;
mod input;
mod ipc;
mod layers;
mod menu;
mod notice;
mod windows;
