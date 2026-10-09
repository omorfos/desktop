//! The nested backend: the compositor runs in a window on an existing
//! Wayland or X11 session. Meant for development and for trying things out.
//!
//! This file is glue between Smithay's winit backend, the event loop and
//! [`Compositor`]. It needs a display and a GPU, so it cannot run in CI and
//! is excluded from the coverage gate (see `docs/adr/0002-coverage-policy.md`).
//! Keep decisions out of it: anything worth testing belongs in `Compositor`.

use std::cell::Cell;
use std::env;
use std::error::Error;
use std::io::{Write, stderr};
use std::rc::Rc;
use std::time::Instant;

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, Event, InputBackend, InputEvent, KeyState,
    KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::reexports::wayland_server::ListeningSocket;
use smithay::utils::Transform;

use crate::{Compositor, Input, Scale, Session};

/// The distance scrolled by one notch of a wheel, in pixels.
const WHEEL_NOTCH: f64 = 15.0;

/// The window the compositor is drawn in.
struct Screen {
    backend: WinitGraphicsBackend<GlesRenderer>,
    tracker: OutputDamageTracker,
    started: Instant,
    /// Set when the window wants drawing again.
    redraw: Rc<Cell<bool>>,
    reported: Vec<String>,
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

    // The usual reason is that there is no session to open a window on,
    // which the error itself does not say.
    let (backend, winit) = winit::init::<GlesRenderer>().map_err(|error| {
        format!(
            "cannot open a window: {error}. omorfos runs inside a Wayland or X11 session for \
             now: start it from one, where WAYLAND_DISPLAY or DISPLAY is set"
        )
    })?;
    let size = backend.window_size();
    // OpenGL's origin is the bottom-left corner, so the picture is flipped.
    let mut session = Session::new(config_directory, (size.w, size.h), Transform::Flipped180)?;

    // The host desktop knows how dense its screen is.
    let density = backend.window().scale_factor();
    session.compositor_mut().suggest_scale(Scale::new(density));
    listen(&mut session)?;

    let redraw = Rc::new(Cell::new(true));
    let wanted = Rc::clone(&redraw);
    session
        .handle()
        .insert_source(winit, move |event, (), compositor| match event {
            WinitEvent::Resized { size, scale_factor } => {
                compositor.resize_output((size.w, size.h));
                compositor.suggest_scale(Scale::new(scale_factor));
                wanted.set(true);
            }
            WinitEvent::Input(event) => on_input(compositor, event),
            WinitEvent::Redraw => wanted.set(true),
            WinitEvent::CloseRequested => compositor.perform(&omorfos_config::Action::Quit),
            WinitEvent::Focus(_) => {}
        })?;

    let mut screen = Screen {
        tracker: OutputDamageTracker::from_output(session.compositor().output()),
        backend,
        started: Instant::now(),
        redraw,
        reported: Vec::new(),
    };
    session.run(|compositor| screen.turn(compositor))
}

/// Opens the Wayland socket for clients and the IPC socket beside it.
pub(super) fn listen(session: &mut Session) -> Result<(), Box<dyn Error>> {
    let socket = ListeningSocket::bind_auto("wayland", 1..33)?;
    if let Some(name) = socket.socket_name() {
        let compositor = session.compositor_mut();
        compositor.set_socket_name(name);
        let _ = writeln!(stderr(), "omorfos: listening on {}", name.display());
        let runtime_directory = env::var_os("XDG_RUNTIME_DIR");
        if let Some(path) = omorfos_ipc::socket_path(None, runtime_directory.as_deref(), Some(name))
        {
            compositor.listen_ipc(path)?;
        }
    }
    session.serve(socket)
}

/// Hands an input event to the compositor, whichever backend it is from.
pub(super) fn on_input<B: InputBackend>(compositor: &mut Compositor, event: InputEvent<B>) {
    let size = compositor.logical_size();
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
        InputEvent::PointerMotion { event } => {
            let moved = Input::MotionBy {
                dx: event.delta_x(),
                dy: event.delta_y(),
            };
            (moved, event.time_msec())
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
    compositor.input(input, time);
}

impl Screen {
    /// What the window does each turn of the loop: report, then draw.
    fn turn(&mut self, compositor: &mut Compositor) -> Result<(), Box<dyn Error>> {
        self.report_problems(compositor);
        if self.redraw.replace(false) {
            let age = self.backend.buffer_age().unwrap_or(0);
            let damage = {
                let (renderer, mut framebuffer) = self.backend.bind()?;
                let result =
                    compositor.render(renderer, &mut framebuffer, &mut self.tracker, age)?;
                result.damage.cloned()
            };
            self.backend.submit(damage.as_deref())?;
            compositor.send_frames(self.started.elapsed());
            // Draw continuously; the host compositor paces us.
            self.backend.window().request_redraw();
        }
        Ok(())
    }

    /// Tells the user about problems, once each time they change.
    fn report_problems(&mut self, compositor: &Compositor) {
        if compositor.problems() != self.reported {
            self.reported = compositor.problems().to_vec();
            for problem in &self.reported {
                let _ = writeln!(stderr(), "omorfos: {problem}");
            }
        }
    }
}
