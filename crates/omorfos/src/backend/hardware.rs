//! The hardware backend: the compositor drives the screens, keyboard and
//! mouse itself, as the session a user logs in to.
//!
//! This file is glue between the kernel's display and input interfaces, as
//! Smithay wraps them, and [`Compositor`]. It needs a seat, a graphics card
//! and a free virtual terminal, so it cannot run in CI and is excluded from
//! the coverage gate (see `docs/adr/0002-coverage-policy.md`). Keep
//! decisions out of it: anything worth testing belongs in `Compositor`.
//!
//! Every screen connected to the seat's graphics card is driven, at the mode
//! it prefers, and screens are taken on and let go as they are plugged in
//! and unplugged. See `docs/adr/0010-hardware-backend.md` and
//! `docs/adr/0015-several-outputs.md`.

use std::cell::RefCell;
use std::env;
use std::error::Error;
use std::io::{Write, stderr};
use std::rc::Rc;
use std::time::{Duration, Instant};

use omorfos_config::{Action, Command, Mouse, Touchpad};
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
use smithay::backend::udev::{UdevBackend, UdevEvent, all_gpus, primary_gpu};
use smithay::output::Output;
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::drm::control::{
    Device as ControlDevice, Mode, ModeTypeFlags, connector, crtc,
};
use smithay::reexports::input::{Device, DeviceCapability, Libinput};
use smithay::reexports::rustix::fs::OFlags;
use smithay::utils::{DeviceFd, Transform};

use super::winit::{cursor_themes, launcher, listen, notifier, on_input, panel};
use crate::{Compositor, Scale, Session};

/// The graphics cards in use. There is one, but Smithay's manager for
/// several is also its only way to a GPU renderer without `unsafe`.
type Gpus = GpuManager<GbmGlesBackend<GlesRenderer, DrmDeviceFd>>;

/// The machine: its graphics card, the screens plugged into it, and its
/// input devices.
struct Hardware {
    seat: LibSeatSession,
    card: DrmDevice,
    gbm: GbmDevice<DrmDeviceFd>,
    gpus: Gpus,
    /// The device that draws, which the card's buffers are bound to.
    renderer: DrmNode,
    /// The screens being driven, in the order they were taken on.
    screens: Vec<Screen>,
    started: Instant,
    /// Whether the seat is ours. It is not while the user is on another
    /// virtual terminal.
    active: bool,
    /// Whether the screens are on. They are turned off when the user has
    /// been away for as long as the configuration says.
    lit: bool,
    /// The touchpads plugged in, and the settings they were last given.
    touchpads: Vec<Device>,
    /// The other devices that move a pointer, which are mice.
    mice: Vec<Device>,
    configured: Option<(Touchpad, Mouse)>,
    reported: Vec<String>,
}

/// One screen being driven.
struct Screen {
    /// The socket it is plugged into.
    connector: connector::Handle,
    /// The part of the card that drives it.
    crtc: crtc::Handle,
    /// The screen, as an output of the compositor.
    output: Output,
    surface: GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>,
    tracker: OutputDamageTracker,
    /// How long it shows one frame.
    frame: Duration,
    /// What it is showing, as far as drawing is concerned.
    picture: Picture,
    /// Whether applications are already due to be told to draw again.
    ticking: bool,
    /// Whether the last attempt to draw on it failed, and that was said.
    failing: bool,
}

/// What a screen is showing.
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
/// Returns an error if the seat cannot be taken, if no graphics card is
/// found, or if drawing fails.
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
    let (card, card_events, gbm, gpus, renderer) = open_card(&mut seat)?;

    let mut session = Session::new(config_directory)?;
    listen(&mut session)?;
    let hardware = Rc::new(RefCell::new(Hardware {
        seat: seat.clone(),
        card,
        gbm,
        gpus,
        renderer,
        screens: Vec::new(),
        started: Instant::now(),
        active: true,
        lit: true,
        touchpads: Vec::new(),
        mice: Vec::new(),
        configured: None,
        reported: Vec::new(),
    }));
    let compositor = session.compositor_mut();
    hardware.borrow_mut().scan(compositor);
    compositor.set_cursor_themes(cursor_themes());
    compositor.show_pointer();
    announce_session(compositor);
    compositor.set_panel(panel());
    compositor.set_launcher(launcher());
    compositor.set_notifier(notifier());
    compositor.start_xwayland(None);
    compositor.autostart();

    // The keyboard, mice and touchpads.
    let mut devices =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(seat.clone().into());
    devices
        .udev_assign_seat(&seat_name)
        .map_err(|()| "cannot find the keyboard and mouse")?;
    watch_devices(&session, &hardware, devices.clone())?;

    // Leaving for another virtual terminal, and coming back.
    let switching = Rc::clone(&hardware);
    session
        .handle()
        .insert_source(seat_events, move |event, (), compositor| {
            let mut hardware = switching.borrow_mut();
            match event {
                SeatEvent::PauseSession => {
                    devices.suspend();
                    hardware.card.pause();
                    hardware.active = false;
                }
                SeatEvent::ActivateSession => {
                    let _ = devices.resume();
                    let _ = hardware.card.activate(false);
                    hardware.active = true;
                    // Whatever was on the screens is gone, and nothing of
                    // ours need have changed meanwhile, so the next frame
                    // is drawn in full and shown regardless.
                    for screen in &mut hardware.screens {
                        screen.surface.reset_buffers();
                        screen.picture = Picture::Stale;
                    }
                    // A screen may have come or gone while we were away.
                    hardware.scan(compositor);
                }
            }
        })?;

    // A frame reaching a screen.
    let shown = Rc::clone(&hardware);
    session
        .handle()
        .insert_source(card_events, move |event, _, compositor| match event {
            DrmEvent::VBlank(crtc) => shown.borrow_mut().frame_shown(crtc, compositor),
            DrmEvent::Error(error) => {
                let _ = writeln!(stderr(), "omorfos: {error}");
            }
        })?;

    // A screen being plugged in or unplugged.
    let plugged = Rc::clone(&hardware);
    session.handle().insert_source(
        UdevBackend::new(&seat_name)?,
        move |event, (), compositor| {
            if let UdevEvent::Changed { .. } = event {
                plugged.borrow_mut().scan(compositor);
            }
        },
    )?;

    let handle = session.handle();
    session.run(move |compositor| turn(&hardware, &handle, compositor))
}

/// Reads the keyboard, mice and touchpads, and keeps track of which
/// touchpads and mice there are so that they can be given their settings.
fn watch_devices(
    session: &Session,
    hardware: &Rc<RefCell<Hardware>>,
    devices: Libinput,
) -> Result<(), Box<dyn Error>> {
    let plugged = Rc::clone(hardware);
    session.handle().insert_source(
        LibinputInputBackend::new(devices),
        move |event, (), compositor| {
            let mut hardware = plugged.borrow_mut();
            match &event {
                // A device that can count fingers tapping is a touchpad.
                InputEvent::DeviceAdded { device } if device.config_tap_finger_count() > 0 => {
                    hardware.touchpads.push(device.clone());
                    // The new one has yet to be given the settings.
                    hardware.configured = None;
                }
                // Anything else that moves a pointer is a mouse.
                InputEvent::DeviceAdded { device }
                    if device.has_capability(DeviceCapability::Pointer) =>
                {
                    hardware.mice.push(device.clone());
                    hardware.configured = None;
                }
                InputEvent::DeviceRemoved { device } => {
                    hardware.touchpads.retain(|touchpad| touchpad != device);
                    hardware.mice.retain(|mouse| mouse != device);
                }
                _ => {}
            }
            // A device that reports positions covers the first screen.
            let first = hardware.screens.first().map(|screen| screen.output.clone());
            drop(hardware);
            on_input(compositor, first.as_ref(), event);
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

/// What the machine does each turn of the loop: report, switch virtual
/// terminal if asked, and draw on each screen that can be drawn on.
fn turn(
    hardware: &Rc<RefCell<Hardware>>,
    handle: &LoopHandle<'static, Compositor>,
    compositor: &mut Compositor,
) -> Result<(), Box<dyn Error>> {
    let mut now = hardware.borrow_mut();
    now.report_problems(compositor);
    if let Some(terminal) = compositor.take_terminal_switch() {
        let _ = now.seat.change_vt(terminal);
    }
    let general = &compositor.config().general;
    now.configure_pointers(general.touchpad, general.mouse);
    if !now.active {
        return Ok(());
    }
    // Turn the screens off when the user has been away, and on again when
    // they are back. A screen turned back on shows nothing of ours yet.
    let wanted = compositor.screens_on();
    if wanted != now.lit {
        now.lit = wanted;
        for screen in &mut now.screens {
            if wanted {
                screen.surface.reset_buffers();
            } else {
                let _ = screen.surface.surface().clear();
            }
            screen.picture = Picture::Stale;
        }
    }
    if !now.lit {
        return Ok(());
    }
    let Hardware {
        gpus,
        renderer,
        screens,
        started,
        ..
    } = &mut *now;
    for screen in screens {
        if screen.picture == Picture::Queued {
            continue;
        }
        // A screen that cannot be drawn on just now is no reason to end
        // the session. The commonest cause is another virtual terminal
        // having taken the screen a moment before we are told so, and it
        // comes back. It is said once, and tried again the next turn.
        let drawn = match screen.draw(gpus, renderer, compositor) {
            Ok(drawn) => {
                screen.failing = false;
                drawn
            }
            Err(error) => {
                if !std::mem::replace(&mut screen.failing, true) {
                    let name = screen.output.name();
                    let _ = writeln!(stderr(), "omorfos: cannot draw on {name}: {error}");
                }
                screen.picture = Picture::Stale;
                // As though something had been drawn: nothing is to be
                // told to draw again on the strength of this.
                true
            }
        };
        if drawn || screen.ticking {
            continue;
        }
        // Nothing changed, so no frame will reach the screen to pace the
        // applications that are waiting to draw. Tell them a frame later.
        screen.ticking = true;
        let later = Rc::clone(hardware);
        let (connector, started) = (screen.connector, *started);
        let tick = Timer::from_duration(screen.frame);
        handle.insert_source(tick, move |_, (), compositor| {
            let mut hardware = later.borrow_mut();
            let waiting = hardware.screens.iter_mut();
            if let Some(screen) = waiting.into_iter().find(|s| s.connector == connector) {
                screen.ticking = false;
                compositor.send_frames(&screen.output, started.elapsed());
            }
            TimeoutAction::Drop
        })?;
    }
    Ok(())
}

/// The seat's graphics card, opened and ready to draw with.
type Card = (
    DrmDevice,
    DrmDeviceNotifier,
    GbmDevice<DrmDeviceFd>,
    Gpus,
    DrmNode,
);

/// Opens the seat's graphics card.
fn open_card(seat: &mut LibSeatSession) -> Result<Card, Box<dyn Error>> {
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
    let (card, card_events) = DrmDevice::new(fd.clone(), true)?;
    let gbm = GbmDevice::new(fd)?;
    let renderer = node
        .node_with_type(NodeType::Render)
        .and_then(Result::ok)
        .unwrap_or(node);
    let mut gpus: Gpus = GpuManager::new(GbmGlesBackend::default())?;
    gpus.as_mut().add_node(renderer, gbm.clone())?;
    Ok((card, card_events, gbm, gpus, renderer))
}

/// A screen found plugged in: its socket, the name of that socket, the mode
/// it prefers and how wide it says it is in millimetres.
type Found = (connector::Handle, String, Mode, u32);

impl Hardware {
    /// Looks at what is plugged in, takes on the screens that are new and
    /// lets go of those that have gone.
    fn scan(&mut self, compositor: &mut Compositor) {
        let found = self.connected();
        let gone = |screen: &Screen| !found.iter().any(|(socket, ..)| *socket == screen.connector);
        for screen in self.screens.extract_if(.., |screen| gone(screen)) {
            compositor.remove_output(&screen.output);
        }
        for (connector, name, mode, millimetres) in found {
            if self.screens.iter().any(|s| s.connector == connector) {
                continue;
            }
            // A screen that cannot be set up is reported and left dark; the
            // others, and the session, carry on.
            match self.take_on(compositor, connector, &name, mode, millimetres) {
                Ok(screen) => self.screens.push(screen),
                Err(error) => {
                    let _ = writeln!(
                        stderr(),
                        "omorfos: cannot use the screen on {name}: {error}"
                    );
                }
            }
        }
    }

    /// The screens plugged in, a laptop's own panel first.
    fn connected(&self) -> Vec<Found> {
        let Ok(resources) = self.card.resource_handles() else {
            return Vec::new();
        };
        let mut found: Vec<(bool, Found)> = resources
            .connectors()
            .iter()
            .filter_map(|handle| {
                let socket = self.card.get_connector(*handle, true).ok()?;
                if socket.state() != connector::State::Connected {
                    return None;
                }
                let preferred = |mode: &&Mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED);
                let mode = socket.modes().iter().find(preferred);
                let mode = *mode.or_else(|| socket.modes().first())?;
                let kind = socket.interface();
                let name = format!("{}-{}", kind.as_str(), socket.interface_id());
                let built_in = matches!(
                    kind,
                    connector::Interface::EmbeddedDisplayPort
                        | connector::Interface::LVDS
                        | connector::Interface::DSI
                );
                let (millimetres, _) = socket.size().unwrap_or_default();
                Some((!built_in, (*handle, name, mode, millimetres)))
            })
            .collect();
        found.sort_by_key(|(external, _)| *external);
        found.into_iter().map(|(_, found)| found).collect()
    }

    /// Starts driving a screen and tells the compositor about it.
    fn take_on(
        &mut self,
        compositor: &mut Compositor,
        connector: connector::Handle,
        name: &str,
        mode: Mode,
        millimetres: u32,
    ) -> Result<Screen, Box<dyn Error>> {
        let crtc = self
            .free_crtc(connector)
            .ok_or("the graphics card cannot drive another")?;
        let scanout = self.card.create_surface(crtc, mode, &[connector])?;
        let flags = GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT;
        let allocator = GbmAllocator::new(self.gbm.clone(), flags);
        let formats = self.gpus.single_renderer(&self.renderer)?.dmabuf_formats();
        let surface = GbmBufferedSurface::new(
            scanout,
            allocator,
            &[Fourcc::Argb8888, Fourcc::Xrgb8888],
            formats,
        )?;
        let (width, height) = mode.size();
        let size = (i32::from(width), i32::from(height));
        let output = compositor.add_output(name, size, Transform::Normal);
        compositor.suggest_scale(&output, Scale::for_screen(u32::from(width), millimetres));
        Ok(Screen {
            connector,
            crtc,
            tracker: OutputDamageTracker::from_output(&output),
            output,
            surface,
            frame: Duration::from_secs(1) / mode.vrefresh().max(1),
            picture: Picture::Stale,
            ticking: false,
            failing: false,
        })
    }

    /// A part of the card that can drive `connector` and is not driving
    /// another screen.
    fn free_crtc(&self, connector: connector::Handle) -> Option<crtc::Handle> {
        let resources = self.card.resource_handles().ok()?;
        let socket = self.card.get_connector(connector, false).ok()?;
        let in_use = |crtc: &crtc::Handle| self.screens.iter().any(|screen| screen.crtc == *crtc);
        socket
            .encoders()
            .iter()
            .filter_map(|encoder| self.card.get_encoder(*encoder).ok())
            .flat_map(|encoder| resources.filter_crtcs(encoder.possible_crtcs()))
            .find(|crtc| !in_use(crtc))
    }

    /// Notes that a frame reached the screen driven by `crtc`, and tells
    /// the applications on it to draw the next.
    fn frame_shown(&mut self, crtc: crtc::Handle, compositor: &mut Compositor) {
        let elapsed = self.started.elapsed();
        if let Some(screen) = self.screens.iter_mut().find(|screen| screen.crtc == crtc) {
            let _ = screen.surface.frame_submitted();
            // A frame from before the screen was taken away says nothing
            // about what is on it now.
            if screen.picture == Picture::Queued {
                screen.picture = Picture::Ours;
            }
            compositor.send_frames(&screen.output, elapsed);
        }
    }

    /// Gives the touchpads and the mice the configured settings, if they
    /// do not have them yet: at startup, when one is plugged in, and after
    /// a reload. A device that lacks a setting keeps what it has.
    fn configure_pointers(&mut self, touchpad: Touchpad, mouse: Mouse) {
        if self.configured == Some((touchpad, mouse)) {
            return;
        }
        for device in &mut self.touchpads {
            let _ = device.config_tap_set_enabled(touchpad.tap_to_click);
            let _ = device.config_scroll_set_natural_scroll_enabled(touchpad.natural_scroll);
        }
        for device in &mut self.mice {
            let _ = device.config_scroll_set_natural_scroll_enabled(mouse.natural_scroll);
        }
        self.configured = Some((touchpad, mouse));
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

impl Screen {
    /// Draws what the screen shows and hands it over. Returns whether
    /// there was anything new to show.
    fn draw(
        &mut self,
        gpus: &mut Gpus,
        renderer: &DrmNode,
        compositor: &mut Compositor,
    ) -> Result<bool, Box<dyn Error>> {
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
        let mut renderer = gpus.single_renderer(renderer)?;
        let drawn = {
            let mut framebuffer = renderer.bind(&mut buffer)?;
            let result = compositor.render(
                &self.output,
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
}
