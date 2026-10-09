//! End-to-end tests: a real Wayland client talks to the compositor over a
//! socket, and the compositor draws with the software renderer.
//!
//! Everything runs on one thread. The tests step the client and the
//! compositor by hand, so each exchange is deterministic.

mod actions;
mod activation;
mod clipboard;
mod common;
mod config;
mod cursors;
mod decorations;
mod dragging;
mod executable;
mod idle;
mod input;
mod ipc;
mod layers;
mod lock;
mod login;
mod menu;
mod notice;
mod popups;
mod scaling;
mod session;
mod taskbar;
mod windows;
