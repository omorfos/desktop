//! The nested backend: the compositor runs in a window on an existing
//! Wayland or X11 session. Meant for development and for trying things out.
//!
//! This file is glue between Smithay's winit backend, the event loop and
//! [`Compositor`]. It needs a display and a GPU, so it cannot run in CI and
//! is excluded from the coverage gate (see `docs/adr/0002-coverage-policy.md`).
//! Keep decisions out of it: anything worth testing belongs in `Compositor`.

use std::env;
use std::error::Error;
use std::io::{Write, stderr};
use std::time::{Duration, Instant};

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent,
    PointerAxisEvent, PointerButtonEvent,
};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend, WinitInput};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{EventLoop, Interest, Mode, PostAction};
use smithay::reexports::wayland_server::{Display, ListeningSocket};
use smithay::utils::Transform;

use crate::{Compositor, Input};

/// How often the loop wakes when nothing else does, so that configuration
/// reloads and child processes are attended to.
const IDLE_INTERVAL: Duration = Duration::from_millis(100);

/// The distance scrolled by one notch of a wheel, in pixels.
const WHEEL_NOTCH: f64 = 15.0;

struct Session {
    compositor: Compositor,
    display: Display<Compositor>,
    socket: ListeningSocket,
    backend: WinitGraphicsBackend<GlesRenderer>,
    tracker: OutputDamageTracker,
    started: Instant,
    redraw: bool,
    reported: Vec<String>,
    failure: Option<Box<dyn Error>>,
}

/// Runs the compositor in a window until the session ends.
///
/// # Errors
///
/// Returns an error if no display or GPU is available, if the Wayland socket
/// cannot be created, or if rendering fails.
pub fn run() -> Result<(), Box<dyn Error>> {
    let config_directory = omorfos_config::config_dir(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
    .ok_or("cannot find the configuration directory: set HOME or XDG_CONFIG_HOME")?;

    let mut event_loop: EventLoop<'_, Session> = EventLoop::try_new()?;
    let mut display: Display<Compositor> = Display::new()?;
    let (backend, winit) = winit::init::<GlesRenderer>()?;
    let size = backend.window_size();
    // OpenGL's origin is the bottom-left corner, so the picture is flipped.
    let mut compositor = Compositor::new(
        display.handle(),
        config_directory,
        (size.w, size.h),
        Transform::Flipped180,
    )?;

    let socket = ListeningSocket::bind_auto("wayland", 1..33)?;
    if let Some(name) = socket.socket_name() {
        compositor.set_socket_name(name);
        let _ = writeln!(stderr(), "omorfos: listening on {}", name.display());
        let runtime_directory = env::var_os("XDG_RUNTIME_DIR");
        if let Some(path) = omorfos_ipc::socket_path(None, runtime_directory.as_deref(), Some(name))
        {
            compositor.listen_ipc(path)?;
        }
    }

    let handle = event_loop.handle();
    handle.insert_source(winit, |event, (), session| session.on_winit(event))?;
    // These two only wake the loop; the work happens in `Session::turn`.
    let wake = |_, _: &mut _, _: &mut Session| Ok(PostAction::Continue);
    let clients = display.backend().poll_fd().try_clone_to_owned()?;
    handle.insert_source(Generic::new(clients, Interest::READ, Mode::Level), wake)?;
    let listener = std::os::fd::AsFd::as_fd(&socket).try_clone_to_owned()?;
    handle.insert_source(Generic::new(listener, Interest::READ, Mode::Level), wake)?;

    let mut session = Session {
        tracker: OutputDamageTracker::from_output(compositor.output()),
        compositor,
        display,
        socket,
        backend,
        started: Instant::now(),
        redraw: true,
        reported: Vec::new(),
        failure: None,
    };
    let signal = event_loop.get_signal();
    event_loop.run(Some(IDLE_INTERVAL), &mut session, |session| {
        if let Err(error) = session.turn() {
            session.failure = Some(error);
        }
        if session.failure.is_some() || !session.compositor.is_running() {
            signal.stop();
        }
    })?;
    session.failure.map_or(Ok(()), Err)
}

impl Session {
    fn on_winit(&mut self, event: WinitEvent) {
        match event {
            WinitEvent::Resized { size, .. } => {
                self.compositor.resize_output((size.w, size.h));
                self.redraw = true;
            }
            WinitEvent::Input(event) => self.on_input(event),
            WinitEvent::Redraw => self.redraw = true,
            WinitEvent::CloseRequested => {
                self.compositor.perform(&omorfos_config::Action::Quit);
            }
            WinitEvent::Focus(_) => {}
        }
    }

    fn on_input(&mut self, event: InputEvent<WinitInput>) {
        let size = self.compositor.output_size();
        let (input, time) = match event {
            InputEvent::Keyboard { event } => {
                let pressed = event.state() == KeyState::Pressed;
                (
                    Input::Key {
                        code: event.key_code(),
                        pressed,
                    },
                    event.time_msec(),
                )
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let (x, y) = (event.x_transformed(size.w), event.y_transformed(size.h));
                (Input::Motion { x, y }, event.time_msec())
            }
            InputEvent::PointerButton { event } => {
                let pressed = event.state() == ButtonState::Pressed;
                (
                    Input::Button {
                        button: event.button_code(),
                        pressed,
                    },
                    event.time_msec(),
                )
            }
            InputEvent::PointerAxis { event } => {
                let amount = |axis| {
                    event
                        .amount(axis)
                        .or_else(|| {
                            event
                                .amount_v120(axis)
                                .map(|v120| v120 / 120.0 * WHEEL_NOTCH)
                        })
                        .unwrap_or(0.0)
                };
                let scroll = Input::Scroll {
                    horizontal: amount(Axis::Horizontal),
                    vertical: amount(Axis::Vertical),
                };
                (scroll, event.time_msec())
            }
            _ => return,
        };
        self.compositor.input(input, time);
    }

    /// One turn of the loop: talk to clients, update, draw.
    fn turn(&mut self) -> Result<(), Box<dyn Error>> {
        while let Some(stream) = self.socket.accept()? {
            self.compositor.insert_client(stream)?;
        }
        self.display.dispatch_clients(&mut self.compositor)?;
        self.compositor.refresh(Instant::now());
        self.display.flush_clients()?;
        self.report_problems();

        if self.redraw {
            self.redraw = false;
            let age = self.backend.buffer_age().unwrap_or(0);
            let damage = {
                let (renderer, mut framebuffer) = self.backend.bind()?;
                let result =
                    self.compositor
                        .render(renderer, &mut framebuffer, &mut self.tracker, age)?;
                result.damage.cloned()
            };
            self.backend.submit(damage.as_deref())?;
            self.compositor.send_frames(self.started.elapsed());
            self.display.flush_clients()?;
            // Draw continuously; the host compositor paces us.
            self.backend.window().request_redraw();
        }
        Ok(())
    }

    /// Tells the user about problems, once each time they change.
    fn report_problems(&mut self) {
        if self.compositor.problems() != self.reported {
            self.reported = self.compositor.problems().to_vec();
            for problem in &self.reported {
                let _ = writeln!(stderr(), "omorfos: {problem}");
            }
        }
    }
}
