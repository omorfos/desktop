//! Pictures of the screen, for a program that may take them.
//!
//! A program asks with `ext-image-copy-capture-v1` and is given what a
//! screen shows, copied into memory it shares. Only a program with the
//! `capture` role is given anything, and for as long as one is taking
//! pictures, and a moment after, the compositor shows a mark on that
//! screen that no program can remove and that is left out of the pictures.
//! See `docs/adr/0037-who-may-do-what.md`.

use std::any::Any;
use std::time::{Duration, Instant};

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::{Format as DrmFormat, Fourcc};
use smithay::backend::renderer::Offscreen;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::Element;
use smithay::backend::renderer::element::solid::SolidColorBuffer;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::ext::image_capture_source::v1::server::{
    ext_foreign_toplevel_image_capture_source_manager_v1::{
        ExtForeignToplevelImageCaptureSourceManagerV1, Request as WindowSourceRequest,
    },
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::{
        ExtOutputImageCaptureSourceManagerV1, Request as SourceRequest,
    },
};
use smithay::reexports::wayland_protocols::ext::image_copy_capture::v1::server::{
    ext_image_copy_capture_cursor_session_v1::{
        Error as CursorError, ExtImageCopyCaptureCursorSessionV1, Request as CursorRequest,
    },
    ext_image_copy_capture_frame_v1::{
        Error as FrameError, ExtImageCopyCaptureFrameV1, FailureReason, Request as FrameRequest,
    },
    ext_image_copy_capture_manager_v1::{
        ExtImageCopyCaptureManagerV1, Options, Request as ManagerRequest,
    },
    ext_image_copy_capture_session_v1::{
        Error as SessionError, ExtImageCopyCaptureSessionV1, Request as SessionRequest,
    },
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::Transform;
use smithay::reexports::wayland_server::protocol::wl_shm::Format;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::utils::{Buffer, Physical, Point, Rectangle, Size};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::shm::with_buffer_contents;

use crate::Scale;
use omorfos_config::{Part, States};
use omorfos_core::WindowId;
use smithay::wayland::foreign_toplevel_list::ForeignToplevelHandle;

use crate::compositor::{Compositor, Painted, color};
use crate::paint;
use crate::paint::Signs;
use crate::recording::{Recorded, Recording};
use crate::roles::Roles;

/// How long the mark stays after the last picture of a screen was asked
/// for, so that one picture cannot be taken unseen.
pub(crate) const LINGER: Duration = Duration::from_millis(1500);

/// How far the mark is from the top and the right of its screen.
pub(crate) const MARGIN: i32 = 8;

/// What has changed on a screen, in its pixels.
type Damage = Vec<Rectangle<i32, Physical>>;

/// What pictures are taken of.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Subject {
    /// All that a screen shows.
    Screen(Output),
    /// One window, by itself.
    Window(WindowId),
}

/// One drawing of something to take pictures from. Programs that want the
/// same are given their pictures from the same drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Pass {
    /// A screen, with the pointer or without.
    Screen {
        /// Whether the pointer is drawn.
        pointer: bool,
    },
    /// A window by itself.
    Window(WindowId),
}

/// The graphics card pictures can be taken in the memory of: its device
/// number, and the layouts of a picture blue first, a row at a time or
/// otherwise, it can draw into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Gpu {
    pub(crate) device: u64,
    pub(crate) layouts: Vec<u64>,
}

impl Gpu {
    /// The card whose device number is `device`, which can draw `formats`.
    pub(crate) fn of(device: u64, formats: &[DrmFormat]) -> Self {
        let blue_first = formats
            .iter()
            .filter(|format| format.code == Fourcc::Xrgb8888);
        Self {
            device,
            layouts: blue_first
                .map(|format| u64::from(format.modifier))
                .collect(),
        }
    }
}

/// The most pixels across or down a picture of a window has: a window can
/// say it is larger than any memory there is to draw it in.
const LARGEST: i32 = 16384;

/// How large a picture of the pointer is, across and down, in pixels: as
/// large as any pointer is drawn, with the pointer at its top left corner.
pub(crate) const POINTER: u32 = 256;

/// A program following the pointer over a screen: told whether it is over
/// it and where, and given pictures of it once it asks.
#[derive(Debug)]
struct Following {
    cursor: ExtImageCopyCaptureCursorSessionV1,
    output: Output,
    /// What pictures of the pointer are asked for through, once they are.
    session: Option<ExtImageCopyCaptureSessionV1>,
    /// Where it was last told the pointer is on the screen, in its pixels,
    /// if it is over it.
    at: Option<Point<i32, Physical>>,
    /// Where in the pointer's picture it was last told its position is,
    /// if it is shown.
    tip: Option<(u32, u32)>,
    /// Which picture of the pointer it was last given, by its serial: 0
    /// while it is hidden. Nothing, before the first.
    given: Option<u64>,
}

impl Following {
    /// Tells the program what has changed of the pointer: whether it is
    /// over the screen, where on it, and where in its picture its position
    /// is, if it is shown.
    fn tell(&mut self, at: Option<Point<i32, Physical>>, tip: Option<(u32, u32)>) {
        let (was, tipped) = (self.at, self.tip);
        if was.is_none() && at.is_some() {
            self.cursor.enter();
        }
        if was.is_some() && at.is_none() {
            self.cursor.leave();
        }
        if tip != tipped {
            let (x, y) = tip.unwrap_or_default();
            self.cursor.hotspot(x.cast_signed(), y.cast_signed());
        }
        let moved = at.filter(|_| at != was);
        if let Some(at) = moved {
            self.cursor.position(at.x, at.y);
        }
        (self.at, self.tip) = (at, tip);
    }
}

/// A picture of the pointer as a program is given it: `POINTER` pixels
/// square, the pointer's own at its top left corner, as much of it as
/// fits, and nothing anywhere else. Nothing at all, while it is hidden.
fn boxed(picture: Option<(&[u8], (u32, u32))>) -> Vec<u8> {
    let row = POINTER as usize * 4;
    let mut boxed = vec![0; row * POINTER as usize];
    let (pixels, (width, _)) = picture.unwrap_or_default();
    let across = (width as usize * 4).max(4);
    let rows = pixels.chunks_exact(across).zip(boxed.chunks_exact_mut(row));
    for (from, to) in rows {
        let fits = from.len().min(row);
        to[..fits].copy_from_slice(&from[..fits]);
    }
    boxed
}

/// Adds pixels read red first to `to`, turned to the order a program is
/// given them in: blue first and red third, with nothing in the fourth.
///
/// A whole pixel is turned at a time, which the compiler makes a few
/// instructions for many pixels at once: this runs over every pixel of
/// every picture of a screen being shared.
/// What is see-through is kept so, for a picture taken `see_through`;
/// otherwise every pixel is made solid.
pub(crate) fn turn(from: &[u8], to: &mut Vec<u8>, see_through: bool) {
    let solid = if see_through { 0 } else { 0xff00_0000 };
    let (from, _) = from.as_chunks::<4>();
    let start = to.len();
    to.resize(start + from.len() * 4, 0);
    let (into, _) = to[start..].as_chunks_mut::<4>();
    for (into, from) in into.iter_mut().zip(from) {
        let pixel = u32::from_le_bytes(*from);
        let turned = (pixel & 0x0000_ff00) | ((pixel & 0xff) << 16) | ((pixel >> 16) & 0xff);
        *into = (turned | (pixel & 0xff00_0000) | solid).to_le_bytes();
    }
}

/// What a picture of a window by itself is drawn in, of whatever kind the
/// renderer draws in, kept from one picture to the next.
pub(crate) trait Held: Any + std::fmt::Debug {}

impl<T: Any + std::fmt::Debug> Held for T {}

/// Says a picture has been taken, and when.
fn ready(frame: &ExtImageCopyCaptureFrameV1, clock: Duration) {
    let seconds = clock.as_secs();
    let high = u32::try_from(seconds >> 32).unwrap_or(u32::MAX);
    let low = u32::try_from(seconds & u64::from(u32::MAX)).unwrap_or(u32::MAX);
    frame.presentation_time(high, low, clock.subsec_nanos());
    frame.ready();
}

/// Where what pictures are taken of stands just now: the screen it is on,
/// how many pixels of that screen it takes, and how many of them there
/// are to a unit of layout.
type Standing = (Output, Size<i32, Physical>, f64);

/// A program taking pictures.
#[derive(Debug)]
struct Taking {
    session: ExtImageCopyCaptureSessionV1,
    subject: Subject,
    /// The screen the pictures are drawn on to be taken, which is the one
    /// that shows the mark: the screen itself, or the one a window is on.
    output: Output,
    /// The size of the pictures: that of the screen, or of the window.
    size: Size<i32, Physical>,
    scale: f64,
    /// Whether the pointer is to be in pictures of a screen.
    pointer: bool,
    /// What has changed since the last picture.
    tracker: OutputDamageTracker,
    /// Whether a picture has been taken. The first is taken at once, and
    /// those after it when something has changed.
    taken: bool,
    /// What a window is in its pictures while a rule says it is private.
    blank: SolidColorBuffer,
}

impl Taking {
    /// A program beginning to take pictures of something that stands
    /// somewhere.
    fn of(
        session: ExtImageCopyCaptureSessionV1,
        (subject, pointer): (Subject, bool),
        (output, size, scale): Standing,
        gpu: Option<&Gpu>,
    ) -> Self {
        let mut taking = Self {
            session,
            tracker: OutputDamageTracker::from_output(&output),
            subject,
            output,
            size: Size::default(),
            scale,
            pointer,
            taken: false,
            blank: SolidColorBuffer::default(),
        };
        taking.stand((taking.output.clone(), size, scale), gpu);
        taking
    }

    /// Notes where what is taken stands now. If its size is not what it
    /// was, the program is told what a buffer has to be to take a picture
    /// in, shared memory or, if there is one, memory of the graphics card,
    /// and the next picture is all new.
    fn stand(&mut self, (output, size, scale): Standing, gpu: Option<&Gpu>) {
        let same = size == self.size && scale.to_bits() == self.scale.to_bits();
        self.output = output;
        if same {
            return;
        }
        (self.size, self.scale) = (size, scale);
        self.taken = false;
        self.tracker = match self.subject {
            Subject::Screen(_) => OutputDamageTracker::from_output(&self.output),
            Subject::Window(_) => OutputDamageTracker::new(size, scale, Transform::Normal.into()),
        };
        self.session
            .buffer_size(size.w.unsigned_abs(), size.h.unsigned_abs());
        // A window may be see-through, as a screen is not.
        if let Subject::Window(_) = self.subject {
            self.session.shm_format(Format::Argb8888);
        }
        self.session.shm_format(Format::Xrgb8888);
        if let Some(gpu) = gpu {
            self.session
                .dmabuf_device(gpu.device.to_ne_bytes().to_vec());
            let layouts = gpu.layouts.iter().flat_map(|layout| layout.to_ne_bytes());
            self.session
                .dmabuf_format(Fourcc::Xrgb8888 as u32, layouts.collect());
        }
        self.session.done();
    }

    /// The drawing its pictures are taken from.
    fn pass(&self) -> Pass {
        match self.subject {
            Subject::Screen(_) => Pass::Screen {
                pointer: self.pointer,
            },
            Subject::Window(window) => Pass::Window(window),
        }
    }
}

/// How far a picture has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// The program is still saying what to take it in.
    Open,
    /// Asked for, and not taken yet.
    Asked,
    /// Taken or failed: nothing more happens to it.
    Over,
}

/// One picture a program asked for.
#[derive(Debug)]
struct Shot {
    frame: ExtImageCopyCaptureFrameV1,
    session: ExtImageCopyCaptureSessionV1,
    buffer: Option<WlBuffer>,
    /// What the program says is out of date in its buffer, which is
    /// written with what has changed.
    stale: Damage,
    stage: Stage,
}

/// A picture that is to be taken now, and what has changed since the one
/// before it.
#[derive(Debug)]
pub(crate) struct Due {
    shot: usize,
    damage: Damage,
    /// What is written: what has changed, and what the program said was
    /// out of date in its buffer, within the picture.
    write: Damage,
    /// How much of the drawing it is: all of a screen, or as much from
    /// its top left corner as a window takes.
    size: Size<i32, Physical>,
}

impl Due {
    /// How large the picture is.
    pub(crate) const fn size(&self) -> Size<i32, Physical> {
        self.size
    }
}

/// Everything to do with pictures of the screen being taken.
#[derive(Debug, Default)]
pub(crate) struct Capture {
    /// The graphics card pictures can be taken in the memory of, once the
    /// backend has said which.
    pub(crate) gpu: Option<Gpu>,
    takings: Vec<Taking>,
    following: Vec<Following>,
    shots: Vec<Shot>,
    /// Screens a program has stopped taking pictures of since the mark
    /// was last seen to.
    ended: Vec<Output>,
    /// Screens the mark stays on for a while yet, and until when.
    fading: Vec<(Output, Instant)>,
    /// The mark, painted for each scale it is shown at.
    marks: Vec<((Scale, Signs), Painted)>,
    /// A picture being turned into the order of colours a program is
    /// given it in, kept so as not to be made anew for every picture.
    scratch: Vec<u8>,
    /// What a picture of a window by itself was last drawn in, and its
    /// size, kept so as not to be made anew for every picture.
    alone: Option<(Size<i32, Buffer>, Box<dyn Held>)>,
    /// What records from a microphone or a camera, which the mark shows
    /// as well.
    pub(crate) recording: Recording,
}

impl Capture {
    /// Offers pictures of the screen to programs. Whether one is given
    /// any is decided when it asks.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        display.create_global::<Compositor, ExtOutputImageCaptureSourceManagerV1, ()>(1, ());
        display.create_global::<Compositor, ExtImageCopyCaptureManagerV1, ()>(1, ());
        display
            .create_global::<Compositor, ExtForeignToplevelImageCaptureSourceManagerV1, ()>(1, ());
        Self::default()
    }

    /// Whether the mark is shown on a screen: a program is taking
    /// pictures of it, or was a moment ago.
    pub(crate) fn marks(&self, output: &Output) -> bool {
        let taking = self.takings.iter().any(|taking| taking.output == *output);
        let fading = self.fading.iter().any(|(fading, _)| fading == output);
        taking || fading || self.ended.contains(output)
    }

    /// The programs taking pictures on a screen, each once for what it
    /// takes: whether that is a window on it, or the screen.
    pub(crate) fn takers(&self, output: &Output) -> Vec<(Client, bool)> {
        let mut takers: Vec<(Client, bool)> = Vec::new();
        let here = self
            .takings
            .iter()
            .filter(|taking| taking.output == *output);
        for taking in here {
            let window = matches!(taking.subject, Subject::Window(_));
            let taker = taking.session.client().map(|client| (client, window));
            let new = taker.filter(|taker| !takers.contains(taker));
            takers.extend(new);
        }
        takers
    }

    /// The mark as painted for a scale, if it has been.
    pub(crate) fn mark(&self, scale: Scale, signs: Signs) -> Option<&Painted> {
        let painted = self
            .marks
            .iter()
            .find(|(painted, _)| *painted == (scale, signs));
        painted.map(|(_, mark)| mark)
    }

    /// Forgets how the mark was painted, for a theme that has changed.
    pub(crate) fn repaint(&mut self) {
        self.marks.clear();
    }

    /// When the mark next goes from a screen.
    pub(crate) fn deadline(&self) -> Option<Instant> {
        self.fading.iter().map(|(_, until)| *until).min()
    }

    /// Whether a program following the pointer takes pictures of it.
    pub(crate) fn pointer_wanted(&self) -> bool {
        self.following
            .iter()
            .any(|following| following.session.is_some())
    }

    /// The drawings of a screen from which pictures have been asked for.
    pub(crate) fn wanted(&self, output: &Output) -> Vec<Pass> {
        let mut passes = Vec::new();
        let asked = self.shots.iter().filter(|shot| shot.stage == Stage::Asked);
        for shot in asked {
            let taking = self.takings.iter();
            let mut taking = taking.filter(|taking| taking.session == shot.session);
            let pass = taking
                .find(|taking| taking.output == *output)
                .map(Taking::pass);
            passes.extend(pass.filter(|pass| !passes.contains(pass)));
        }
        passes
    }

    /// What a window is in pictures of it while it is private, as large
    /// as it is.
    pub(crate) fn blank(&self, window: WindowId) -> Option<&SolidColorBuffer> {
        let of = Subject::Window(window);
        let taking = self.takings.iter().find(|taking| taking.subject == of);
        taking.map(|taking| &taking.blank)
    }

    /// The pictures to take now from a drawing of a screen, given what is
    /// in it: those that are a program's first, and those for which
    /// something has changed since its last.
    pub(crate) fn due<E: Element>(
        &mut self,
        output: &Output,
        pass: Pass,
        elements: &[E],
    ) -> Vec<Due> {
        let mut due = Vec::new();
        for taking in &mut self.takings {
            let asked = self
                .shots
                .iter()
                .position(|shot| shot.stage == Stage::Asked && shot.session == taking.session);
            let here = taking.output == *output && taking.pass() == pass;
            let Some(shot) = asked.filter(|_| here) else {
                continue;
            };
            let changed = taking.tracker.damage_output(1, elements).ok();
            let changed = changed.and_then(|(damage, _)| damage.cloned());
            let damage = if taking.taken {
                changed
            } else {
                Some(vec![Rectangle::from_size(taking.size)])
            };
            let size = taking.size;
            let whole = Rectangle::from_size(size);
            let stale = self.shots[shot].stale.iter();
            due.extend(damage.map(|damage| {
                let write = damage.iter().chain(stale);
                let write = write.filter_map(|part| part.intersection(whole)).collect();
                Due {
                    shot,
                    damage,
                    write,
                    size,
                }
            }));
        }
        due
    }

    /// What of the drawing has to be read to give the pictures due: all
    /// that any of them writes, as one rectangle. Nothing, if none writes
    /// anything.
    pub(crate) fn reading(due: &[Due]) -> Option<Rectangle<i32, Physical>> {
        let parts = due.iter().flat_map(|due| due.write.iter().copied());
        parts.reduce(Rectangle::merge)
    }

    /// Gives programs the pictures that were due, writing into each only
    /// what it needs. `pixels` is the part of the drawing `read` says, four
    /// bytes to a pixel as red, green, blue and one to spare; or nothing,
    /// if it could not be read.
    pub(crate) fn hand(
        &mut self,
        due: Vec<Due>,
        pixels: Option<&[u8]>,
        read: Rectangle<i32, Physical>,
        clock: Duration,
    ) {
        let across = |pixels: i32| usize::try_from(pixels).unwrap_or_default();
        // A row of what was read.
        let row = across(read.size.w) * 4;
        let pixels = pixels.unwrap_or_default();
        for Due {
            shot: index,
            damage,
            write,
            size,
        } in due
        {
            let size = (across(size.w), across(size.h));
            let shot = &mut self.shots[index];
            shot.stage = Stage::Over;
            let buffer = shot.buffer.as_ref();
            // Written in the form the program chose of those it was offered.
            let format = buffer
                .and_then(|buffer| with_buffer_contents(buffer, |_, _, data| data.format).ok());
            let format =
                format.filter(|format| [Format::Argb8888, Format::Xrgb8888].contains(format));
            let see_through = format == Some(Format::Argb8888);
            let written = write.iter().all(|part| {
                let (x, y) = (across(part.loc.x), across(part.loc.y));
                let (wide, high) = (across(part.size.w), across(part.size.h));
                // Where the part is in what was read, which holds it.
                let (left, top) = (
                    across(part.loc.x - read.loc.x),
                    across(part.loc.y - read.loc.y),
                );
                self.scratch.clear();
                for line in top..top + high {
                    let start = line * row + left * 4;
                    let from = pixels.get(start..start + wide * 4).unwrap_or_default();
                    turn(from, &mut self.scratch, see_through);
                }
                let part = (x, y, wide, high);
                buffer.zip(format).is_some_and(|(buffer, format)| {
                    omorfos_shm::write_part(buffer, format, size, part, &self.scratch)
                })
            });
            self.finish(index, written.then_some(damage), clock);
        }
    }

    /// What a picture of a window by itself `size` pixels large is drawn
    /// in: the one the last was drawn in, if it is as large, or a new one.
    /// Nothing, if none can be made.
    pub(crate) fn alone<R, P>(
        &mut self,
        renderer: &mut R,
        size: Size<i32, Physical>,
    ) -> Option<&mut P>
    where
        R: Offscreen<P>,
        P: Held,
    {
        let size = Size::<i32, Buffer>::from((size.w, size.h));
        let kept = self.alone.as_ref().is_some_and(|(made, _)| *made == size);
        if !kept {
            let made = renderer.create_buffer(Fourcc::Abgr8888, size).ok();
            self.alone = made.map(|made| (size, Box::new(made) as Box<dyn Held>));
        }
        let held = self
            .alone
            .as_mut()
            .map(|(_, held)| &mut **held as &mut dyn Any);
        held.and_then(<dyn Any>::downcast_mut)
    }

    /// The pictures due that are to be drawn into memory of the graphics
    /// card, with what each is drawn into, and those to be written into
    /// shared memory.
    pub(crate) fn split(&self, due: Vec<Due>) -> (Vec<(Due, Dmabuf)>, Vec<Due>) {
        let mut on_card = Vec::new();
        let mut in_memory = Vec::new();
        for due in due {
            let buffer = self.shots[due.shot].buffer.as_ref();
            let card = buffer.and_then(|buffer| get_dmabuf(buffer).ok()).cloned();
            match card {
                Some(card) => on_card.push((due, card)),
                None => in_memory.push(due),
            }
        }
        (on_card, in_memory)
    }

    /// Gives a program a picture drawn into the memory of the graphics
    /// card, if it could be.
    pub(crate) fn give(&mut self, due: Due, drawn: bool, clock: Duration) {
        self.shots[due.shot].stage = Stage::Over;
        self.finish(due.shot, drawn.then_some(due.damage), clock);
    }

    /// Tells a program its picture has been taken, and what changed in it:
    /// or, without that, that it could not be put in what it was given.
    fn finish(&mut self, index: usize, damage: Option<Damage>, clock: Duration) {
        let shot = &self.shots[index];
        let Some(damage) = damage else {
            shot.frame.failed(FailureReason::BufferConstraints);
            return;
        };
        let taking = self.takings.iter_mut();
        for taking in taking.filter(|taking| taking.session == shot.session) {
            taking.taken = true;
        }
        shot.frame.transform(Transform::Normal);
        for part in damage {
            shot.frame
                .damage(part.loc.x, part.loc.y, part.size.w, part.size.h);
        }
        ready(&shot.frame, clock);
    }

    /// Has a program take no more pictures through this.
    fn end(&mut self, session: &ExtImageCopyCaptureSessionV1) {
        let following = self.following.iter_mut();
        for following in following.filter(|following| following.session.as_ref() == Some(session)) {
            following.session = None;
        }
        let ended = self
            .takings
            .extract_if(.., |taking| taking.session == *session);
        self.ended.extend(ended.map(|taking| taking.output));
    }
}

impl Compositor {
    /// What a window is called in the list of windows a taskbar is given,
    /// which is how a program says which window it wants a picture of.
    /// Empty for one that is not in it.
    pub(crate) fn handle_of(&self, window: WindowId) -> String {
        let mapped = self.windows.iter().find(|mapped| mapped.id == window);
        mapped
            .map(|mapped| mapped.listed.identifier())
            .unwrap_or_default()
    }

    /// Whether a program is taking pictures of a screen, or was a moment
    /// ago: whether the mark that says so is on it.
    #[must_use]
    pub fn captured(&self, output: &Output) -> bool {
        self.capture.marks(output)
    }

    /// Where the pointer's position is on a screen, in its pixels from its
    /// top left corner, if it is over it.
    fn pointed(&self, output: &Output) -> Option<Point<i32, Physical>> {
        let head = self.head(output);
        let over = head.filter(|head| head.area().to_f64().contains(self.pointer_location));
        over.map(|head| {
            let from = self.pointer_location - head.area().loc.to_f64();
            from.to_physical(head.scale.factor()).to_i32_round()
        })
    }

    /// Where what pictures are taken of stands: a screen where it is, and
    /// a window on the screen that shows it, or on the first if none
    /// does, as large as it is, however large that screen, up to `LARGEST`.
    /// Nothing, if it has gone.
    fn stands(&self, subject: &Subject) -> Option<Standing> {
        match subject {
            Subject::Screen(output) => {
                let head = self.head(output);
                head.map(|head| (head.output.clone(), head.pixels(), head.scale.factor()))
            }
            Subject::Window(window) => {
                let mapped = self.windows.iter().find(|mapped| mapped.id == *window);
                let shown = self.scene.iter().find(|shown| shown.id == *window);
                let on = shown.and_then(|shown| self.head_at(shown.origin.to_f64()));
                let head = on.or(self.heads.first());
                mapped.zip(head).map(|(mapped, head)| {
                    let scale = head.scale.factor();
                    // All it draws, its shadow included.
                    let size = mapped.window.bbox().size;
                    let size: Size<i32, Physical> = size.to_f64().to_physical(scale).to_i32_round();
                    let size = Size::from((size.w.clamp(1, LARGEST), size.h.clamp(1, LARGEST)));
                    (head.output.clone(), size, scale)
                })
            }
        }
    }

    /// Keeps pictures in step with everything else: a program taking
    /// pictures of what has gone is told it has stopped, one whose screen
    /// or window has changed size is told the new size, and nothing is
    /// given while the session is locked. Sees to the mark.
    pub(crate) fn settle_capture(&mut self, now: Instant) {
        let subjects = self.capture.takings.iter();
        let standing: Vec<_> = subjects
            .map(|taking| self.stands(&taking.subject))
            .collect();
        self.settle_following();
        let veiled = color(
            self.config()
                .theme
                .style(Part::Capture, States::NONE)
                .background,
        );
        let locked = self.is_locked();
        let capture = &mut self.capture;
        let mut standing = standing.into_iter();
        let gone = capture.takings.extract_if(.., |taking| {
            let stands = standing.next().flatten();
            let gone = stands.is_none();
            // What has gone is left as it stood, to be told it has stopped.
            let stands = stands.unwrap_or((taking.output.clone(), taking.size, taking.scale));
            taking.stand(stands, capture.gpu.as_ref());
            taking.blank.update(taking.size.to_logical(1), veiled);
            gone
        });
        for taking in gone.collect::<Vec<_>>() {
            taking.session.stopped();
            capture.ended.push(taking.output);
        }
        // A picture asked for that will not be taken is said to have failed.
        for shot in &mut capture.shots {
            let following = capture.following.iter();
            let followed = following
                .filter_map(|following| following.session.as_ref())
                .any(|session| *session == shot.session);
            let live = followed
                || capture
                    .takings
                    .iter()
                    .any(|taking| taking.session == shot.session);
            if shot.stage == Stage::Asked && (locked || !live) {
                shot.stage = Stage::Over;
                let stopped = FailureReason::Stopped;
                shot.frame.failed(if live {
                    FailureReason::Unknown
                } else {
                    stopped
                });
            }
        }
        let ended = capture.ended.drain(..);
        capture
            .fading
            .extend(ended.map(|output| (output, now + LINGER)));
        capture.fading.retain(|(_, until)| *until > now);

        let wanted = self
            .heads
            .iter()
            .map(|head| (head.scale, self.signs(&head.output)));
        let wanted = wanted
            .filter(|(scale, signs)| signs.any() && self.capture.mark(*scale, *signs).is_none());
        let wanted: Vec<(Scale, Signs)> = wanted.collect();
        for (scale, signs) in wanted {
            let theme = &self.config().theme;
            let mark = Painted::new(&paint::capture_mark(theme, scale, signs));
            self.capture.marks.push(((scale, signs), mark));
        }
    }

    /// What the mark on a screen shows a sign of: pictures taken of it, and
    /// a microphone or a camera recorded from, which shows on every screen.
    pub(crate) fn signs(&self, output: &Output) -> Signs {
        let recording = &self.capture.recording;
        Signs {
            screen: self.capture.marks(output),
            sound: recording.any(&[Recorded::Microphone, Recorded::Sound]),
            camera: recording.any(&[Recorded::Camera]),
        }
    }

    /// Keeps programs following the pointer in step: one over a screen that
    /// has gone is told there will be no more pictures; the rest are told
    /// where the pointer is, and given a picture of it that is new, unless
    /// the session is locked.
    pub(crate) fn settle_following(&mut self) {
        let following = self.capture.following.iter();
        let there: Vec<_> = following
            .map(|following| self.head(&following.output).is_some())
            .collect();
        // One following the pointer over a screen that has gone is told
        // there will be no more pictures of it.
        let mut there = there.into_iter();
        let gone = self
            .capture
            .following
            .extract_if(.., |_| !there.next().unwrap_or(false));
        for following in gone.collect::<Vec<_>>() {
            if following.at.is_some() {
                following.cursor.leave();
            }
            following
                .session
                .iter()
                .for_each(ExtImageCopyCaptureSessionV1::stopped);
        }
        let following = self.capture.following.iter();
        let pointed: Vec<_> = following
            .map(|following| self.pointed(&following.output))
            .collect();
        let locked = self.is_locked();
        // A program following the pointer is told where it is, and given
        // a picture of it that is new.
        let clock = self.clock();
        let (picture, settled, capture) = self.pointer_picture();
        let ours = picture.map(|picture| (&picture.bytes[..], picture.pixels));
        for (following, at) in capture.following.iter_mut().zip(pointed) {
            following.tell(at, picture.map(|picture| picture.tip));
        }
        let serial = picture.map_or(0, |picture| picture.serial);
        for shot in &mut capture.shots {
            let mut following = capture.following.iter_mut();
            let of = following.find(|following| following.session.as_ref() == Some(&shot.session));
            let new = of.filter(|following| following.given != Some(serial));
            let asked = shot.stage == Stage::Asked && !locked && settled;
            let Some(following) = new.filter(|_| asked) else {
                continue;
            };
            shot.stage = Stage::Over;
            let boxed = boxed(ours);
            let size = (POINTER as usize, POINTER as usize);
            let buffer = shot.buffer.as_ref();
            let written = buffer
                .is_some_and(|buffer| omorfos_shm::write(buffer, Format::Argb8888, size, &boxed));
            if !written {
                shot.frame.failed(FailureReason::BufferConstraints);
                continue;
            }
            following.given = Some(serial);
            let whole = POINTER.cast_signed();
            shot.frame.transform(Transform::Normal);
            shot.frame.damage(0, 0, whole, whole);
            ready(&shot.frame, clock);
        }
    }

    /// Starts a program taking pictures of what `source` stands for, if
    /// it may and the screen is still there.
    fn take(
        &mut self,
        client: &Client,
        session: ExtImageCopyCaptureSessionV1,
        source: &ExtImageCaptureSourceV1,
        pointer: bool,
    ) {
        let may = Self::roles(Some(client)).has(Roles::CAPTURE);
        if !may {
            self.refuse(Some(client), "take pictures of the screen", "capture");
        }
        let subject = source.data::<Option<Subject>>().cloned().flatten();
        let subject = subject.filter(|_| may);
        let stands = subject.as_ref().and_then(|subject| self.stands(subject));
        let Some((subject, stands)) = subject.zip(stands) else {
            session.stopped();
            return;
        };
        let taking = Taking::of(
            session,
            (subject, pointer),
            stands,
            self.capture.gpu.as_ref(),
        );
        self.capture.takings.push(taking);
    }

    /// Starts a program following the pointer over the screen `source`
    /// stands for, if it may. Over a window, or over a screen that has
    /// gone, it is not followed.
    fn follow(
        &mut self,
        client: &Client,
        cursor: ExtImageCopyCaptureCursorSessionV1,
        source: &ExtImageCaptureSourceV1,
    ) {
        let may = Self::roles(Some(client)).has(Roles::CAPTURE);
        if !may {
            self.refuse(Some(client), "follow the pointer", "capture");
        }
        let subject = source.data::<Option<Subject>>().cloned().flatten();
        let output = match subject.filter(|_| may) {
            Some(Subject::Screen(output)) => self.head(&output).map(|_| output),
            _ => None,
        };
        self.capture
            .following
            .extend(output.map(|output| Following {
                cursor,
                output,
                session: None,
                at: None,
                tip: None,
                given: None,
            }));
    }
}

impl GlobalDispatch<ExtForeignToplevelImageCaptureSourceManagerV1, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        manager: New<ExtForeignToplevelImageCaptureSourceManagerV1>,
        (): &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(manager, ());
    }

    fn can_view(client: Client, (): &()) -> bool {
        Self::outside(&client)
    }
}

impl Dispatch<ExtForeignToplevelImageCaptureSourceManagerV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &ExtForeignToplevelImageCaptureSourceManagerV1,
        request: WindowSourceRequest,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let WindowSourceRequest::CreateSource {
            source,
            toplevel_handle,
        } = request
        {
            // A window is named by its entry in the list of windows, which
            // only a program that is told what windows there are has.
            let listed = ForeignToplevelHandle::from_resource(&toplevel_handle);
            let listed = listed.map(|listed| listed.identifier());
            let mut windows = state.windows.iter();
            let window = windows.find(|mapped| Some(mapped.listed.identifier()) == listed);
            data_init.init(source, window.map(|mapped| Subject::Window(mapped.id)));
        }
    }
}

impl GlobalDispatch<ExtOutputImageCaptureSourceManagerV1, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        manager: New<ExtOutputImageCaptureSourceManagerV1>,
        (): &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(manager, ());
    }

    fn can_view(client: Client, (): &()) -> bool {
        Self::outside(&client)
    }
}

impl Dispatch<ExtOutputImageCaptureSourceManagerV1, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ExtOutputImageCaptureSourceManagerV1,
        request: SourceRequest,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let SourceRequest::CreateSource { source, output } = request {
            let screen = Output::from_resource(&output).map(Subject::Screen);
            data_init.init(source, screen);
        }
    }
}

impl Dispatch<ExtImageCaptureSourceV1, Option<Subject>> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ExtImageCaptureSourceV1,
        _: <ExtImageCaptureSourceV1 as Resource>::Request,
        _: &Option<Subject>,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        // All that can be done with one is to let go of it.
    }
}

impl GlobalDispatch<ExtImageCopyCaptureManagerV1, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        manager: New<ExtImageCopyCaptureManagerV1>,
        (): &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(manager, ());
    }

    fn can_view(client: Client, (): &()) -> bool {
        Self::outside(&client)
    }
}

impl Dispatch<ExtImageCopyCaptureManagerV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        client: &Client,
        _: &ExtImageCopyCaptureManagerV1,
        request: ManagerRequest,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            ManagerRequest::CreateSession {
                session,
                source,
                options,
            } => {
                let session = data_init.init(session, ());
                let options = options.into_result().unwrap_or(Options::empty());
                let pointer = options.contains(Options::PaintCursors);
                state.take(client, session, &source, pointer);
            }
            ManagerRequest::CreatePointerCursorSession {
                session, source, ..
            } => {
                let cursor = data_init.init(session, ());
                state.follow(client, cursor, &source);
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureCursorSessionV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        cursor: &ExtImageCopyCaptureCursorSessionV1,
        request: CursorRequest,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let CursorRequest::GetCaptureSession { session } = request {
            let session = data_init.init(session, ());
            let following = state.capture.following.iter_mut();
            let mut following = following.filter(|following| following.cursor == *cursor);
            match following.next() {
                Some(following) if following.session.is_some() => {
                    let said = "pictures of the pointer are asked for already";
                    cursor.post_error(CursorError::DuplicateSession, said);
                }
                // Pictures of the pointer are as large as any pointer, and
                // see through where it is not.
                Some(following) => {
                    session.buffer_size(POINTER, POINTER);
                    session.shm_format(Format::Argb8888);
                    session.done();
                    following.session = Some(session);
                }
                // A program that may not follow the pointer, or not over
                // what it named, is told there will be no pictures.
                None => session.stopped(),
            }
        }
    }

    fn destroyed(
        state: &mut Self,
        _: ClientId,
        cursor: &ExtImageCopyCaptureCursorSessionV1,
        (): &(),
    ) {
        let following = &mut state.capture.following;
        following.retain(|following| following.cursor != *cursor);
    }
}

impl Dispatch<ExtImageCopyCaptureSessionV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        session: &ExtImageCopyCaptureSessionV1,
        request: SessionRequest,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let SessionRequest::CreateFrame { frame } = request {
            let frame = data_init.init(frame, ());
            let shots = &mut state.capture.shots;
            if shots.iter().any(|shot| shot.session == *session) {
                let said = "the picture before this one has not been let go of";
                session.post_error(SessionError::DuplicateFrame, said);
            }
            shots.push(Shot {
                frame,
                session: session.clone(),
                buffer: None,
                stale: Vec::new(),
                stage: Stage::Open,
            });
        }
    }

    fn destroyed(state: &mut Self, _: ClientId, session: &ExtImageCopyCaptureSessionV1, (): &()) {
        state.capture.end(session);
    }
}

impl Dispatch<ExtImageCopyCaptureFrameV1, ()> for Compositor {
    fn request(
        state: &mut Self,
        _: &Client,
        frame: &ExtImageCopyCaptureFrameV1,
        request: FrameRequest,
        (): &(),
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let shots = &mut state.capture.shots;
        let open = shots.iter_mut().find(|shot| shot.frame == *frame);
        let Some(shot) = open.filter(|shot| shot.stage == Stage::Open) else {
            if !matches!(request, FrameRequest::Destroy) {
                let said = "the picture has been asked for already";
                frame.post_error(FrameError::AlreadyCaptured, said);
            }
            return;
        };
        match request {
            FrameRequest::AttachBuffer { buffer } => shot.buffer = Some(buffer),
            FrameRequest::DamageBuffer {
                x,
                y,
                width,
                height,
            } => {
                // What the program says is out of date in its buffer is
                // written with the picture, as well as what has changed.
                if x < 0 || y < 0 || width <= 0 || height <= 0 {
                    let said = "what is stale in a buffer cannot lie outside it or be nothing";
                    frame.post_error(FrameError::InvalidBufferDamage, said);
                }
                let part = Rectangle::new((x, y).into(), (width, height).into());
                shot.stale.push(part);
            }
            FrameRequest::Capture if shot.buffer.is_none() => {
                frame.post_error(FrameError::NoBuffer, "there is nothing to take it in");
            }
            FrameRequest::Capture => shot.stage = Stage::Asked,
            _ => {}
        }
    }

    fn destroyed(state: &mut Self, _: ClientId, frame: &ExtImageCopyCaptureFrameV1, (): &()) {
        state.capture.shots.retain(|shot| shot.frame != *frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_read_red_first_are_given_blue_first_and_solid_unless_see_through() {
        let mut turned = vec![9];
        turn(&[1, 2, 3, 4, 10, 20, 30, 40, 99], &mut turned, false);
        assert_eq!(turned, [9, 3, 2, 1, 255, 30, 20, 10, 255]);
        let mut turned = Vec::new();
        turn(&[1, 2, 3, 4, 10, 20, 30, 40], &mut turned, true);
        assert_eq!(turned, [3, 2, 1, 4, 30, 20, 10, 40]);
    }
}
