//! The hardware backend: the compositor drives the screen, keyboard and
//! mouse itself, as the session a user logs in to.
//!
//! This file is glue between the kernel's display and input interfaces, as
//! Smithay wraps them, and [`Compositor`]. It needs a seat, a graphics card
//! and a free virtual terminal, so it cannot run in CI and is excluded from
//! the coverage gate (see `docs/adr/0002-coverage-policy.md`). Keep
//! decisions out of it: anything worth testing belongs in `Compositor`.
//!
//! One screen is driven for now: the first one found connected, at the
//! mode it prefers. See `docs/adr/0010-hardware-backend.md`.

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::io::{Write, stderr};
use std::rc::Rc;
use std::time::{Duration, Instant};

use omorfos_config::{Action, Command, Touchpad};
use smithay::backend::allocator::Fourcc;
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBufferFlags, GbmDevice};
use smithay::backend::drm::{
    DrmDevice, DrmDeviceFd, DrmDeviceNotifier, DrmEvent, DrmNode, GbmBufferedSurface, NodeType,
};
use smithay::backend::input::InputEvent;
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::multigpu::GpuManager;
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::{Bind, ImportDma};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::{Event as SeatEvent, Session as Seat};
use smithay::backend::udev::{all_gpus, primary_gpu};
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::drm::control::{
    Device as ControlDevice, Mode, ModeTypeFlags, connector, crtc,
};
use smithay::reexports::input::{Device, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::{DeviceFd, Transform};

use super::winit::{cursor_themes, listen, on_input};
use crate::{Compositor, Scale, Session};

/// The graphics cards in use. There is one, but Smithay's manager for
/// several is also its only way to a GPU renderer without `unsafe`.
type Gpus = GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>;

/// The screen being driven.
struct Screen {
    seat: LibSeatSession,
    card: DrmDevice,
    surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    gpus: Gpus,
    /// The device that draws, which the card's buffers are bound to.
    renderer: DrmNode,
    tracker: OutputDamageTracker,
    /// How long the screen shows one frame.
    frame: Duration,
    started: Instant,
    /// Whether the seat is ours. It is not while the user is on another
    /// virtual terminal.
    active: bool,
    /// What the screen is showing, as far as drawing is concerned.
    picture: Picture,
    /// Whether applications are already due to be told to draw again.
    ticking: bool,
    /// The touchpads plugged in, and the settings they were last given.
    touchpads: Vec<Device>,
    configured: Option<Touchpad>,
    reported: Vec<String>,
}

/// What the screen is showing.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Picture {
    /// Not ours: someone else's, or nothing yet. The next frame has to be
    /// drawn in full, whatever has or has not changed.
    Stale,
    /// Our last frame.
    Ours,
    /// Our last frame, with the next one handed over and not yet shown.
    /// Nothing more can be drawn until it is.
    Queued,
}

/// Runs the compositor on the hardware until the session ends.
///
/// # Errors
///
/// Returns an error if the seat cannot be taken, if no graphics card or
/// connected screen is found, or if drawing fails.
pub fn run() -> Result<(), Box<dyn Error>> {
    let config_directory = omorfos_config::config_dir(
        env::var_os("XDG_CONFIG_HOME").as_deref(),
        env::var_os("HOME").as_deref(),
    )
    .ok_or("cannot find the configuration directory: set HOME or XDG_CONFIG_HOME")?;

    let (mut seat, seat_events) = LibSeatSession::new().map_err(|error| {
        format!(
            "cannot take control of the screen and keyboard: {error}. Start omorfos from a \
             text console (a virtual terminal such as Ctrl+Alt+F3), logged in directly and \
             not over SSH, or from inside another desktop to run it in a window"
        )
    })?;
    let seat_name = seat.seat();

    let (card, card_events, surface, gpus, renderer, mode, millimetres) = open_screen(&mut seat)?;
    let (width, height) = mode.size();
    let size = (i32::from(width), i32::from(height));

    let mut session = Session::new(config_directory, size, Transform::Normal)?;
    listen(&mut session)?;
    let compositor = session.compositor_mut();
    compositor.suggest_scale(Scale::for_screen(u32::from(width), millimetres));
    compositor.set_cursor_themes(cursor_themes());
    compositor.show_pointer();
    announce_session(compositor);

    let screen = Rc::new(RefCell::new(Screen {
        seat: seat.clone(),
        card,
        surface,
        gpus,
        renderer,
        tracker: OutputDamageTracker::from_output(session.compositor().output()),
        frame: Duration::from_secs(1) / mode.vrefresh().max(1),
        started: Instant::now(),
        active: true,
        picture: Picture::Stale,
        ticking: false,
        touchpads: Vec::new(),
        configured: None,
        reported: Vec::new(),
    }));

    // The keyboard, mice and touchpads.
    let mut devices =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(seat.clone().into());
    devices
        .udev_assign_seat(&seat_name)
        .map_err(|()| "cannot find the keyboard and mouse")?;
    watch_devices(&session, &screen, devices.clone())?;

    // Leaving for another virtual terminal, and coming back.
    let switching = Rc::clone(&screen);
    session
        .handle()
        .insert_source(seat_events, move |event, (), _| {
            let mut screen = switching.borrow_mut();
            match event {
                SeatEvent::PauseSession => {
                    devices.suspend();
                    screen.card.pause();
                    screen.active = false;
                }
                SeatEvent::ActivateSession => {
                    let _ = devices.resume();
                    let _ = screen.card.activate(false);
                    // Whatever was on screen is gone, and nothing of ours
                    // need have changed meanwhile, so the next frame is
                    // drawn in full and shown regardless.
                    screen.surface.reset_buffers();
                    screen.picture = Picture::Stale;
                    screen.active = true;
                }
            }
        })?;

    // A frame reaching the screen.
    let shown = Rc::clone(&screen);
    session
        .handle()
        .insert_source(card_events, move |event, _, compositor| {
            let mut screen = shown.borrow_mut();
            match event {
                DrmEvent::VBlank(_) => {
                    let _ = screen.surface.frame_submitted();
                    // A frame from before the screen was taken away says
                    // nothing about what is on it now.
                    if screen.picture == Picture::Queued {
                        screen.picture = Picture::Ours;
                    }
                    compositor.send_frames(screen.started.elapsed());
                }
                DrmEvent::Error(error) => {
                    let _ = writeln!(stderr(), "omorfos: {error}");
                }
            }
        })?;

    let handle = session.handle();
    session.run(move |compositor| turn(&screen, &handle, compositor))
}

/// Reads the keyboard, mice and touchpads, and keeps track of which
/// touchpads there are so that they can be given their settings.
fn watch_devices(
    session: &Session,
    screen: &Rc<RefCell<Screen>>,
    devices: Libinput,
) -> Result<(), Box<dyn Error>> {
    let plugged = Rc::clone(screen);
    session.handle().insert_source(
        LibinputInputBackend::new(devices),
        move |event, (), compositor| {
            match &event {
                // A device that can count fingers tapping is a touchpad.
                InputEvent::DeviceAdded { device } if device.config_tap_finger_count() > 0 => {
                    let mut screen = plugged.borrow_mut();
                    screen.touchpads.push(device.clone());
                    // The new one has yet to be given the settings.
                    screen.configured = None;
                }
                InputEvent::DeviceRemoved { device } => {
                    let mut screen = plugged.borrow_mut();
                    screen.touchpads.retain(|touchpad| touchpad != device);
                }
                _ => {}
            }
            on_input(compositor, event);
        },
    )?;
    Ok(())
}

/// Tells whatever else starts programs for the user about this session.
///
/// Programs are also started by D-Bus and by systemd, which know nothing of
/// the session unless told. This backend is the user's whole session, so
/// they are told; the nested one must not, or it would redirect the
/// programs of the desktop it runs inside.
fn announce_session(compositor: &mut Compositor) {
    let told = compositor.session_environment().into_iter();
    let told = told.map(|(name, value)| format!("{name}={}", value.to_string_lossy()));
    let arguments = std::iter::once("--systemd".to_owned()).chain(told);
    if let Some(tell) = Command::new("dbus-update-activation-environment", arguments) {
        compositor.perform(&Action::Spawn(tell));
    }
}

/// What the screen does each turn of the loop: report, switch virtual
/// terminal if asked, and draw if it can.
fn turn(
    screen: &Rc<RefCell<Screen>>,
    handle: &LoopHandle<'static, Compositor>,
    compositor: &mut Compositor,
) -> Result<(), Box<dyn Error>> {
    let mut now = screen.borrow_mut();
    now.report_problems(compositor);
    if let Some(terminal) = compositor.take_terminal_switch() {
        let _ = now.seat.change_vt(terminal);
    }
    now.configure_touchpads(compositor.config().general.touchpad);
    if !now.active || now.picture == Picture::Queued {
        return Ok(());
    }
    if now.draw(compositor)? || now.ticking {
        return Ok(());
    }
    // Nothing changed, so no frame will reach the screen to pace the
    // applications that are waiting to draw. Tell them a frame later.
    now.ticking = true;
    let later = Rc::clone(screen);
    let tick = Timer::from_duration(now.frame);
    handle.insert_source(tick, move |_, (), compositor| {
        let mut screen = later.borrow_mut();
        screen.ticking = false;
        compositor.send_frames(screen.started.elapsed());
        TimeoutAction::Drop
    })?;
    Ok(())
}

/// The graphics card and the screen it drives, ready to be drawn on.
type Opened = (
    DrmDevice,
    DrmDeviceNotifier,
    GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    Gpus,
    DrmNode,
    Mode,
    // The width of the screen in millimetres, or 0 if it does not say.
    u32,
);

/// Opens the seat's graphics card and sets up its first connected screen.
fn open_screen(seat: &mut LibSeatSession) -> Result<Opened, Box<dyn Error>> {
    let seat_name = seat.seat();
    let path = primary_gpu(&seat_name)
        .ok()
        .flatten()
        .or_else(|| all_gpus(&seat_name).ok()?.into_iter().next())
        .ok_or("no graphics card was found")?;
    let node = DrmNode::from_path(&path)?;
    let flags = OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK;
    let fd = seat
        .open(&path, flags)
        .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (mut card, card_events) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;
    let renderer = node
        .node_with_type(NodeType::Render)
        .and_then(Result::ok)
        .unwrap_or(node);
    let mut gpus: Gpus = GpuManager::new(GbmGlesBackend::default())?;
    gpus.as_mut().add_node(renderer, gbm.clone())?;

    let (connector, mode, crtc, millimetres) = connected_screen(&card)?;
    let scanout = card.create_surface(crtc, mode, &[connector])?;
    let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT);
    let formats = gpus.single_renderer(&renderer)?.dmabuf_formats();
    let surface = GbmBufferedSurface::new(
        scanout,
        allocator,
        &[Fourcc::Argb8888, Fourcc::Xrgb8888],
        formats,
    )?;
    Ok((
        card,
        card_events,
        surface,
        gpus,
        renderer,
        mode,
        millimetres,
    ))
}

/// Finds the first connected screen, the mode it prefers, something to
/// drive it with, and how wide it says it is in millimetres.
fn connected_screen(
    card: &DrmDevice,
) -> Result<(connector::Handle, Mode, crtc::Handle, u32), Box<dyn Error>> {
    let resources = card.resource_handles()?;
    for handle in resources.connectors() {
        let screen = card.get_connector(*handle, true)?;
        if screen.state() != connector::State::Connected {
            continue;
        }
        let preferred = |mode: &&Mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED);
        let mode = screen.modes().iter().find(preferred);
        let mode = mode.or_else(|| screen.modes().first());
        let driver = screen
            .encoders()
            .iter()
            .filter_map(|encoder| card.get_encoder(*encoder).ok())
            .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
            .next();
        if let (Some(mode), Some(driver)) = (mode, driver) {
            let (millimetres, _) = screen.size().unwrap_or_default();
            return Ok((*handle, *mode, driver, millimetres));
        }
    }
    Err("no screen is connected".into())
}

impl Screen {
    /// Draws the output and hands it to the screen. Returns whether there
    /// was anything new to show.
    fn draw(&mut self, compositor: &mut Compositor) -> Result<bool, Box<dyn Error>> {
        let (mut buffer, age) = self.surface.next_buffer()?;
        // A buffer that was taken and never shown, because nothing had
        // changed, is handed out again with the age it had. After coming
        // back from another virtual terminal that age means nothing: the
        // screen holds someone else's picture. An age of zero draws it all.
        let age = if self.picture == Picture::Stale {
            0
        } else {
            age
        };
        let mut renderer = self.gpus.single_renderer(&self.renderer)?;
        let drawn = {
            let mut framebuffer = renderer.bind(&mut buffer)?;
            let result = compositor.render(
                &mut renderer,
                &mut framebuffer,
                &mut self.tracker,
                usize::from(age),
            )?;
            result.damage.cloned().map(|damage| (damage, result.sync))
        };
        match drawn {
            Some((damage, sync)) => {
                self.surface.queue_buffer(Some(sync), Some(damage), ())?;
                self.picture = Picture::Queued;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    /// Gives the touchpads the configured settings, if they do not have
    /// them yet: at startup, when one is plugged in, and after a reload. A
    /// touchpad that lacks a setting keeps what it has.
    fn configure_touchpads(&mut self, wanted: Touchpad) {
        if self.configured == Some(wanted) {
            return;
        }
        for touchpad in &mut self.touchpads {
            let _ = touchpad.config_tap_set_enabled(wanted.tap_to_click);
            let _ = touchpad.config_scroll_set_natural_scroll_enabled(wanted.natural_scroll);
        }
        self.configured = Some(wanted);
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
