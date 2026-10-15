//! End-to-end tests: a real Wayland client talks to the compositor over a
//! socket, and the compositor draws with the software renderer.
//!
//! Everything runs on one thread. The tests step the client and the
//! compositor by hand, so each exchange is deterministic.

mod actions;
mod activation;
mod capture;
mod clipboard;
mod common;
mod config;
mod corners;
mod cursors;
mod decorations;
mod dragging;
mod executable;
mod gpu;
mod idle;
mod input;
mod ipc;
mod keeper;
mod launcher;
mod layers;
mod lock;
mod login;
mod menu;
mod notice;
mod notifications;
mod outputs;
mod palette;
mod panel;
mod popups;
mod portal;
mod recording;
mod reveal;
mod roles;
mod scaling;
mod screenshot;
mod session;
mod sharing;
mod taskbar;
mod tiles;
mod windows;
mod x11;
