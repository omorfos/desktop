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
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use smithay::backend::input::{
    AbsolutePositionEvent, Axis, ButtonState, Event, InputBackend, InputEvent, KeyState,
    KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::{GlesRenderbuffer, GlesRenderer};
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::output::Output;
use smithay::reexports::wayland_server::ListeningSocket;
use smithay::utils::Transform;

use crate::Roles;
use crate::{Compositor, Input, Scale, Session, Themes};

/// The distance scrolled by one notch of a wheel, in pixels.
const WHEEL_NOTCH: f64 = 15.0;

/// The window the compositor is drawn in.
struct Screen {
    backend: WinitGraphicsBackend<GlesRenderer>,
    tracker: OutputDamageTracker,
    /// The window, as an output of the compositor.
    output: Output,
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
    let mut session = Session::new(config_directory)?;
    let compositor = session.compositor_mut();
    let output = compositor.add_output("nested", (size.w, size.h), Transform::Flipped180);

    // The host desktop knows how dense its screen is.
    let density = backend.window().scale_factor();
    compositor.suggest_scale(&output, Scale::new(density));
    listen(&mut session)?;
    // The pointer is drawn by omorfos, so that it looks as applications
    // inside ask, and the host's is hidden over the window.
    backend.window().set_cursor_visible(false);
    let compositor = session.compositor_mut();
    compositor.set_cursor_themes(cursor_themes());
    compositor.set_data_directories(data_directories());
    compositor.show_pointer();
    compositor.set_panel(panel());
    compositor.set_wallpaper(wallpaper());
    compositor.set_launcher(launcher());
    compositor.set_notifier(notifier());
    compositor.set_keeper(keeper());
    compositor.set_recorder(recorder());
    compositor.start_xwayland(None);
    compositor.autostart();

    let redraw = Rc::new(Cell::new(true));
    let wanted = Rc::clone(&redraw);
    let shown = output.clone();
    session
        .handle()
        .insert_source(winit, move |event, (), compositor| match event {
            WinitEvent::Resized { size, scale_factor } => {
                compositor.resize_output(&shown, (size.w, size.h));
                compositor.suggest_scale(&shown, Scale::new(scale_factor));
                wanted.set(true);
            }
            WinitEvent::Input(event) => on_input(compositor, Some(&shown), event),
            WinitEvent::Redraw => wanted.set(true),
            WinitEvent::CloseRequested => compositor.perform(&omorfos_config::Action::Quit),
            WinitEvent::Focus(_) => {}
        })?;

    let mut screen = Screen {
        tracker: OutputDamageTracker::from_output(&output),
        output,
        backend,
        started: Instant::now(),
        redraw,
        reported: Vec::new(),
    };
    session.run(|compositor| screen.turn(compositor))
}

/// Where cursor themes are installed, going by the environment.
/// Where the program that shows notifications for this compositor is.
pub(super) fn notifier() -> PathBuf {
    super::notifier_program(env::current_exe().ok().as_deref())
}

/// Where the program that keeps what is copied, for this compositor, is.
pub(super) fn keeper() -> PathBuf {
    super::keeper_program(env::current_exe().ok().as_deref())
}

/// Where the program that watches what records, for this compositor, is.
pub(super) fn recorder() -> PathBuf {
    super::recorder_program(env::current_exe().ok().as_deref())
}

/// Where the launcher that goes with this compositor is.
pub(super) fn launcher() -> PathBuf {
    super::launcher_program(env::current_exe().ok().as_deref())
}

/// Where the panel that goes with this compositor is.
pub(super) fn panel() -> PathBuf {
    super::panel_program(env::current_exe().ok().as_deref())
}

/// Where the wallpaper, for this compositor, is.
pub(super) fn wallpaper() -> PathBuf {
    super::wallpaper_program(env::current_exe().ok().as_deref())
}

pub(super) fn cursor_themes() -> Themes {
    Themes::conventional(
        env::var_os("XCURSOR_PATH").as_deref(),
        env::var_os("HOME").as_deref(),
    )
}

/// Where programs install what they share, as the environment says.
pub(super) fn data_directories() -> Vec<PathBuf> {
    omorfos_icons::data_directories(
        env::var_os("XDG_DATA_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
        env::var_os("XDG_DATA_DIRS").as_deref(),
    )
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
    session.serve(socket, Roles::NONE)
}

/// Hands an input event to the compositor, whichever backend it is from.
pub(super) fn on_input<B: InputBackend>(
    compositor: &mut Compositor,
    output: Option<&Output>,
    event: InputEvent<B>,
) {
    // A device that reports positions, not movement, covers this output.
    let area = output.map(|output| compositor.area_of(output));
    let area = area.unwrap_or_default();
    let size = area.size;
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
            let x = f64::from(area.loc.x) + event.x_transformed(size.w);
            let y = f64::from(area.loc.y) + event.y_transformed(size.h);
            (Input::Motion { x, y }, event.time_msec())
        }
        InputEvent::PointerMotion { event } => {
            let moved = Input::MotionBy {
                dx: event.delta_x(),
                dy: event.delta_y(),
                raw_dx: event.delta_x_unaccel(),
                raw_dy: event.delta_y_unaccel(),
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
            let (damage, feedback) = {
                let (renderer, mut framebuffer) = self.backend.bind()?;
                // A window by itself is drawn in a buffer of the card's
                // that can be read back from.
                let result = compositor.render::<_, GlesRenderbuffer>(
                    &self.output,
                    renderer,
                    &mut framebuffer,
                    &mut self.tracker,
                    age,
                )?;
                (result.damage, result.feedback)
            };
            self.backend.submit(damage.as_deref())?;
            // The host compositor does not say when it showed what was
            // submitted, so it is taken to be now.
            feedback.shown(None, None);
            compositor.send_frames(&self.output, self.started.elapsed());
            // Draw continuously; the host compositor paces us.
            self.backend.window().request_redraw();
        }
        Ok(())
    }

    /// Tells the user about problems, once each time they change, and
    /// what the log is to be told, once.
    fn report_problems(&mut self, compositor: &mut Compositor) {
        if compositor.problems() != self.reported {
            self.reported = compositor.problems().to_vec();
            for problem in &self.reported {
                let _ = writeln!(stderr(), "omorfos: {problem}");
            }
        }
        for logged in compositor.take_logged() {
            let _ = writeln!(stderr(), "omorfos: {logged}");
        }
    }
}
