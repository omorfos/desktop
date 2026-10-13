//! Asking the compositor for pictures, as a Wayland client.

use std::error::Error;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use std::path::Path;

use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_output, wl_registry, wl_seat, wl_shm, wl_shm_pool,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self as device, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::ExtDataControlOfferV1,
    ext_data_control_source_v1::{self as source, ExtDataControlSourceV1},
};
use wayland_protocols::ext::foreign_toplevel_list::v1::client::{
    ext_foreign_toplevel_handle_v1::{self as toplevel, ExtForeignToplevelHandleV1},
    ext_foreign_toplevel_list_v1::{self as toplevel_list, ExtForeignToplevelListV1},
};
use wayland_protocols::ext::image_capture_source::v1::client::{
    ext_foreign_toplevel_image_capture_source_manager_v1::ExtForeignToplevelImageCaptureSourceManagerV1,
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::ExtOutputImageCaptureSourceManagerV1,
};
use wayland_protocols::ext::image_copy_capture::v1::client::{
    ext_image_copy_capture_frame_v1::{self as frame, ExtImageCopyCaptureFrameV1},
    ext_image_copy_capture_manager_v1::{ExtImageCopyCaptureManagerV1, Options},
    ext_image_copy_capture_session_v1::{self as session, ExtImageCopyCaptureSessionV1},
};

/// Why no picture was taken or saved, in words for the user.
pub type Failure = Box<dyn Error + Send + Sync>;

/// A picture of one screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    /// What the screen is called, such as `DP-1`.
    pub screen: String,
    /// How many pixels across it is.
    pub width: u32,
    /// How many pixels down it is.
    pub height: u32,
    /// Its pixels from the top left, three bytes to each: red, green, blue.
    pub pixels: Vec<u8>,
}

/// How far the picture of one screen has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// It has been asked for, and nothing has come of it yet.
    Asked,
    /// The compositor gives this program no pictures.
    Refused,
    /// It has been taken.
    Taken,
    /// It could not be taken.
    Failed,
}

/// One thing a picture is taken of, a screen or a window, and the
/// picture of it.
#[derive(Debug)]
struct Screen {
    name: Option<String>,
    stage: Stage,
    /// The size the compositor said the picture is.
    size: (u32, u32),
    /// The memory the picture is taken in.
    memory: Option<File>,
}

/// What the compositor has said, and what has been asked of it.
#[derive(Debug, Default)]
struct State {
    shm: Option<wl_shm::WlShm>,
    sources: Option<ExtOutputImageCaptureSourceManagerV1>,
    capturers: Option<ExtImageCopyCaptureManagerV1>,
    outputs: Vec<wl_output::WlOutput>,
    screens: Vec<Screen>,
    /// What one window is asked for through, and the windows there are:
    /// each as a taskbar is told of it, and what it is called.
    windows: Option<ExtForeignToplevelImageCaptureSourceManagerV1>,
    listed: Vec<(ExtForeignToplevelHandleV1, String)>,
    /// The window a picture is wanted of, by what it is called, in place
    /// of the screens; and whether it was among the windows there are.
    wanted: Option<String>,
    found: bool,
    /// How many times the compositor has said it has caught up. After the
    /// first it has said what there is, and after the second what each
    /// screen is called, which is when pictures are asked for.
    caught_up: u32,
    /// Whether the compositor offers pictures of the screen at all.
    offered: bool,
    /// What the clipboard is set through, if this program may.
    clipboards: Option<ExtDataControlManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    /// The picture held for whatever asks to paste it.
    held: Vec<u8>,
    /// Whether something else has been copied since.
    replaced: bool,
}

/// The connection pictures were taken over, kept to hold one of them on
/// the clipboard through.
#[derive(Debug)]
pub(crate) struct Camera {
    queue: EventQueue<State>,
    state: State,
}

impl State {
    /// Whether there is nothing more to wait for.
    fn finished(&self) -> bool {
        let mut screens = self.screens.iter();
        self.caught_up >= 2 && (!self.offered || screens.all(|screen| screen.stage != Stage::Asked))
    }

    /// The screen something is about, if it is still one of them.
    fn screen(&mut self, index: usize) -> impl Iterator<Item = &mut Screen> {
        self.screens.iter_mut().skip(index).take(1)
    }
}

/// Memory of `length` bytes to share with the compositor.
fn memory(length: u64) -> io::Result<File> {
    let made = memfd_create("omorfos-screenshot", MemfdFlags::CLOEXEC);
    let made = made.map(File::from).map_err(io::Error::from);
    made.and_then(|file| file.set_len(length).map(|()| file))
}

/// Takes a picture of every screen of the compositor at `socket`, or of
/// the one that handed this program a connection; or of one `window`, by
/// what it is called in the list of windows, in place of the screens.
///
/// # Errors
///
/// Returns why not: there is no compositor there, it gives this program
/// no pictures, or a picture could not be taken.
pub(crate) fn shoot(socket: &Path, window: Option<String>) -> Result<(Vec<Shot>, Camera), Failure> {
    let stream = omorfos_shell::connect(socket);
    let connection =
        stream.and_then(|stream| Connection::from_socket(stream).map_err(io::Error::other));
    let connection = connection.map_err(|error| format!("{}: {error}", socket.display()))?;
    let mut queue = connection.new_event_queue();
    let handle = queue.handle();
    let mut state = State {
        wanted: window,
        ..State::default()
    };
    connection.display().get_registry(&handle, ());
    connection.display().sync(&handle, ());

    // Everything is asked for as the compositor says what there is, so
    // there is one thing to wait for: that nothing more is to come.
    let mut alive = true;
    while alive && !state.finished() {
        alive = queue.blocking_dispatch(&mut state).is_ok();
    }
    if !alive {
        return Err("the session ended before a picture was taken".into());
    }
    if !state.offered {
        return Err("the compositor does not offer pictures of the screen to this program".into());
    }
    if state.wanted.is_some() && !state.found {
        let said = "the window with focus is not among the windows omorfos lists to this \
                    program. It lists them to a program it starts with `may: [capture, panel]`";
        return Err(said.into());
    }
    if state
        .screens
        .iter()
        .any(|screen| screen.stage == Stage::Refused)
    {
        let said = "omorfos did not let this program take pictures of the screen. It lets a \
                    program it starts with `may: [capture]`, from a key in keys.yaml or from \
                    `autostart` in config.yaml";
        return Err(said.into());
    }
    let shots = state.screens.iter().enumerate().map(Screen::shot);
    let shots: Option<Vec<Shot>> = shots.collect();
    let shots = shots.ok_or("a picture of the screen could not be taken")?;
    Ok((shots, Camera { queue, state }))
}

impl Camera {
    /// Puts a picture, as a PNG, on the clipboard, and holds it there
    /// until something else is copied or the session ends: what is on
    /// the clipboard is given by the program that put it there, each
    /// time it is pasted.
    ///
    /// # Errors
    ///
    /// Returns an error if this program may not set the clipboard.
    pub(crate) fn hold(mut self, png: Vec<u8>) -> Result<(), Failure> {
        let may = self.state.clipboards.as_ref().zip(self.state.seat.as_ref());
        let Some((clipboards, seat)) = may else {
            let said = "omorfos did not let this program set the clipboard. It lets a program \
                        it starts with `may: [capture, clipboard]`";
            return Err(said.into());
        };
        let handle = self.queue.handle();
        let device = clipboards.get_data_device(seat, &handle, ());
        let source = clipboards.create_data_source(&handle, ());
        source.offer("image/png".to_owned());
        device.set_selection(Some(&source));
        self.state.held = png;
        let mut alive = true;
        while alive && !self.state.replaced {
            alive = self.queue.blocking_dispatch(&mut self.state).is_ok();
        }
        Ok(())
    }
}

impl Screen {
    /// Something a picture has been asked for of, or is about to be.
    const fn asked() -> Self {
        Self {
            name: None,
            stage: Stage::Asked,
            size: (0, 0),
            memory: None,
        }
    }

    /// The picture of this screen, the `index`th, if it was taken.
    fn shot((index, screen): (usize, &Self)) -> Option<Shot> {
        let (width, height) = screen.size;
        let taken = screen.stage == Stage::Taken;
        let memory = screen.memory.as_ref().filter(|_| taken);
        let length = usize::try_from(u64::from(width) * u64::from(height) * 4).ok();
        let bytes = memory.zip(length).and_then(|(memory, length)| {
            let mut bytes = vec![0; length];
            memory.read_exact_at(&mut bytes, 0).ok().map(|()| bytes)
        });
        bytes.map(|bytes| {
            // Blue is first of the four bytes of a pixel, and red third.
            let (pixels, _) = bytes.as_chunks::<4>();
            let pixels = pixels.iter();
            let pixels = pixels.flat_map(|[blue, green, red, _]| [*red, *green, *blue]);
            let numbered = format!("screen-{}", index + 1);
            Shot {
                screen: screen.name.clone().unwrap_or(numbered),
                width,
                height,
                pixels: pixels.collect(),
            }
        })
    }
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        // A registry also says when something goes, and of what comes
        // later. Pictures are taken of the screens there were at first.
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_shm" => state.shm = Some(registry.bind(name, 1, handle, ())),
                "ext_output_image_capture_source_manager_v1" => {
                    state.sources = Some(registry.bind(name, 1, handle, ()));
                }
                "ext_image_copy_capture_manager_v1" => {
                    state.capturers = Some(registry.bind(name, 1, handle, ()));
                }
                "ext_data_control_manager_v1" => {
                    state.clipboards = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_seat" => state.seat = Some(registry.bind(name, 1, handle, ())),
                "ext_foreign_toplevel_image_capture_source_manager_v1" => {
                    state.windows = Some(registry.bind(name, 1, handle, ()));
                }
                "ext_foreign_toplevel_list_v1" => {
                    let _: ExtForeignToplevelListV1 = registry.bind(name, 1, handle, ());
                }
                "wl_output" if state.caught_up == 0 => {
                    // The fourth version is the first to say what a
                    // screen is called.
                    let index = state.screens.len();
                    let output = registry.bind(name, version.min(4), handle, index);
                    state.outputs.push(output);
                    state.screens.push(Screen::asked());
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_callback::WlCallback, ()> for State {
    fn event(
        state: &mut Self,
        _: &wl_callback::WlCallback,
        _: wl_callback::Event,
        (): &(),
        connection: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        state.caught_up += 1;
        if state.caught_up == 1 {
            // What there is has been said. Once more, to be told what
            // each screen is called.
            connection.display().sync(handle, ());
            return;
        }
        let offered = state.sources.clone().zip(state.capturers.clone());
        state.offered = offered.is_some() && state.shm.is_some();
        let Some((sources, capturers)) = offered else {
            return;
        };
        let Some(wanted) = state.wanted.clone() else {
            for (index, output) in state.outputs.iter().enumerate() {
                let source = sources.create_source(output, handle, ());
                capturers.create_session(&source, Options::empty(), handle, index);
                source.destroy();
            }
            return;
        };
        // One picture, of the window, in place of those of the screens.
        let mut listed = state.listed.iter();
        let window = listed.find(|(_, called)| *called == wanted);
        let source = window.zip(state.windows.as_ref());
        let source = source.map(|((window, _), windows)| windows.create_source(window, handle, ()));
        state.screens.clear();
        if let Some(source) = source {
            state.found = true;
            state.screens.push(Screen::asked());
            capturers.create_session(&source, Options::empty(), handle, 0);
            source.destroy();
        }
    }
}

impl Dispatch<wl_output::WlOutput, usize> for State {
    fn event(
        state: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            for screen in state.screen(*index) {
                screen.name = Some(name.clone());
            }
        }
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, usize> for State {
    fn event(
        state: &mut Self,
        taking: &ExtImageCopyCaptureSessionV1,
        event: session::Event,
        index: &usize,
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let shm = state.shm.clone();
        for screen in state.screen(*index) {
            match event {
                session::Event::BufferSize { width, height } => screen.size = (width, height),
                session::Event::Stopped => screen.stage = Stage::Refused,
                session::Event::Done => {
                    // The size is known: the picture is asked for, in
                    // memory made to that size.
                    let (width, height) = screen.size;
                    let length = u64::from(width) * u64::from(height) * 4;
                    let sizes = (
                        i32::try_from(length),
                        i32::try_from(width),
                        i32::try_from(height),
                    );
                    let sizes = sizes.0.ok().zip(sizes.1.ok()).zip(sizes.2.ok());
                    let made = memory(length).ok().zip(sizes).zip(shm.as_ref());
                    screen.memory = made.map(|((memory, ((length, width), height)), shm)| {
                        let pool = shm.create_pool(memory.as_fd(), length, handle, ());
                        let format = wl_shm::Format::Xrgb8888;
                        let buffer =
                            pool.create_buffer(0, width, height, width * 4, format, handle, ());
                        pool.destroy();
                        let frame = taking.create_frame(handle, *index);
                        frame.attach_buffer(&buffer);
                        frame.damage_buffer(0, 0, width, height);
                        frame.capture();
                        memory
                    });
                    // Without memory to take it in there is no picture.
                    let failed = screen.memory.is_none().then_some(Stage::Failed);
                    screen.stage = failed.unwrap_or(screen.stage);
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, usize> for State {
    fn event(
        state: &mut Self,
        _: &ExtImageCopyCaptureFrameV1,
        event: frame::Event,
        index: &usize,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        for screen in state.screen(*index) {
            match event {
                frame::Event::Ready => screen.stage = Stage::Taken,
                frame::Event::Failed { .. } => screen.stage = Stage::Failed,
                _ => {}
            }
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtDataControlSourceV1,
        event: source::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // It is asked for the picture, or told that something else has
        // been copied: there is nothing else it is told.
        if let source::Event::Send { fd, .. } = event {
            // A program that asked and then went away is no trouble of
            // ours.
            drop(File::from(fd).write_all(&state.held));
        } else {
            state.replaced = true;
        }
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtDataControlDeviceV1,
        _: device::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // What is on the clipboard is of no interest: only putting
        // something there is.
    }

    wayland_client::event_created_child!(State, ExtDataControlDeviceV1, [
        device::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelListV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ExtForeignToplevelListV1,
        _: <ExtForeignToplevelListV1 as wayland_client::Proxy>::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // A window is noted when it says what it is called.
    }

    wayland_client::event_created_child!(State, ExtForeignToplevelListV1, [
        toplevel_list::EVT_TOPLEVEL_OPCODE => (ExtForeignToplevelHandleV1, ()),
    ]);
}

impl Dispatch<ExtForeignToplevelHandleV1, ()> for State {
    fn event(
        state: &mut Self,
        window: &ExtForeignToplevelHandleV1,
        event: toplevel::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let toplevel::Event::Identifier { identifier } = event {
            state.listed.push((window.clone(), identifier));
        }
    }
}

delegate_noop!(State: ExtForeignToplevelImageCaptureSourceManagerV1);
delegate_noop!(State: ExtDataControlManagerV1);
delegate_noop!(State: ignore ExtDataControlOfferV1);
delegate_noop!(State: ignore wl_seat::WlSeat);
delegate_noop!(State: ignore wl_shm::WlShm);
delegate_noop!(State: wl_shm_pool::WlShmPool);
delegate_noop!(State: ignore wl_buffer::WlBuffer);
delegate_noop!(State: ExtOutputImageCaptureSourceManagerV1);
delegate_noop!(State: ExtImageCaptureSourceV1);
delegate_noop!(State: ExtImageCopyCaptureManagerV1);
