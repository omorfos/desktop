//! End-to-end tests: a real Wayland client talks to the compositor over a
//! socket, and the compositor draws with the software renderer.
//!
//! Everything runs on one thread. The tests step the client and the
//! compositor by hand, so each exchange is deterministic.

mod actions;
mod activation;
mod ask;
mod capture;
mod clipboard;
mod common;
mod config;
mod corners;
mod cursors;
mod decorations;
mod display_key;
mod dragging;
mod executable;
mod gpu;
mod holding;
mod idle;
mod input;
mod ipc;
mod keeper;
mod launcher;
mod layers;
mod lock;
mod lock_screen;
mod login;
mod menu;
mod mirroring;
mod notice;
mod notifications;
mod outputs;
mod palette;
mod panel;
mod popups;
mod portal;
mod presentation;
mod recording;
mod reveal;
mod roles;
mod scaling;
mod screens_off;
mod screenshot;
mod sealed;
mod session;
mod sharing;
mod sleep;
mod switcher;
mod taskbar;
mod tiles;
mod typing;
mod wallpaper;
mod windows;
mod x11;
