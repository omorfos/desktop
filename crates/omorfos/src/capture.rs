//! Pictures of the screen, for a program that may take them.
//!
//! A program asks with `ext-image-copy-capture-v1` and is given what a
//! screen shows, copied into memory it shares. Only a program with the
//! `capture` role is given anything, and for as long as one is taking
//! pictures, and a moment after, the compositor shows a mark on that
//! screen that no program can remove and that is left out of the pictures.
//! See `docs/adr/0037-who-may-do-what.md`.

use std::time::{Duration, Instant};

use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::Element;
use smithay::output::Output;
use smithay::reexports::wayland_protocols::ext::image_capture_source::v1::server::{
    ext_image_capture_source_v1::ExtImageCaptureSourceV1,
    ext_output_image_capture_source_manager_v1::{
        ExtOutputImageCaptureSourceManagerV1, Request as SourceRequest,
    },
};
use smithay::reexports::wayland_protocols::ext::image_copy_capture::v1::server::{
    ext_image_copy_capture_cursor_session_v1::{
        ExtImageCopyCaptureCursorSessionV1, Request as CursorRequest,
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
use smithay::utils::{Physical, Rectangle, Size};

use crate::Scale;
use crate::compositor::{Compositor, Painted};
use crate::paint;
use crate::roles::Roles;

/// How long the mark stays after the last picture of a screen was asked
/// for, so that one picture cannot be taken unseen.
pub(crate) const LINGER: Duration = Duration::from_millis(1500);

/// How far the mark is from the top and the right of its screen.
pub(crate) const MARGIN: i32 = 8;

/// What has changed on a screen, in its pixels.
type Damage = Vec<Rectangle<i32, Physical>>;

/// A program taking pictures of a screen.
#[derive(Debug)]
struct Taking {
    session: ExtImageCopyCaptureSessionV1,
    output: Output,
    /// The size of the pictures, which is that of the screen.
    size: Size<i32, Physical>,
    /// Whether the pointer is to be in the pictures.
    pointer: bool,
    /// What has changed since the last picture.
    tracker: OutputDamageTracker,
    /// Whether a picture has been taken. The first is taken at once, and
    /// those after it when something has changed.
    taken: bool,
}

impl Taking {
    /// Tells the program what a buffer has to be to take a picture in.
    fn announce(&self) {
        let width = self.size.w.unsigned_abs();
        self.session.buffer_size(width, self.size.h.unsigned_abs());
        self.session.shm_format(Format::Xrgb8888);
        self.session.done();
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
    stage: Stage,
}

/// A picture that is to be taken now, and what has changed since the one
/// before it.
#[derive(Debug)]
pub(crate) struct Due {
    shot: usize,
    damage: Damage,
}

/// Everything to do with pictures of the screen being taken.
#[derive(Debug, Default)]
pub(crate) struct Capture {
    takings: Vec<Taking>,
    shots: Vec<Shot>,
    /// Screens a program has stopped taking pictures of since the mark
    /// was last seen to.
    ended: Vec<Output>,
    /// Screens the mark stays on for a while yet, and until when.
    fading: Vec<(Output, Instant)>,
    /// The mark, painted for each scale it is shown at.
    marks: Vec<(Scale, Painted)>,
    /// A picture being turned into the order of colours a program is
    /// given it in, kept so as not to be made anew for every picture.
    scratch: Vec<u8>,
}

impl Capture {
    /// Offers pictures of the screen to programs. Whether one is given
    /// any is decided when it asks.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        display.create_global::<Compositor, ExtOutputImageCaptureSourceManagerV1, ()>(1, ());
        display.create_global::<Compositor, ExtImageCopyCaptureManagerV1, ()>(1, ());
        Self::default()
    }

    /// Whether the mark is shown on a screen: a program is taking
    /// pictures of it, or was a moment ago.
    pub(crate) fn marks(&self, output: &Output) -> bool {
        let taking = self.takings.iter().any(|taking| taking.output == *output);
        let fading = self.fading.iter().any(|(fading, _)| fading == output);
        taking || fading || self.ended.contains(output)
    }

    /// The mark as painted for a scale, if it has been.
    pub(crate) fn mark(&self, scale: Scale) -> Option<&Painted> {
        let painted = self.marks.iter().find(|(painted, _)| *painted == scale);
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

    /// Whether a picture of a screen has been asked for, with the pointer
    /// in it or without.
    pub(crate) fn wants(&self, output: &Output, pointer: bool) -> bool {
        let mut asked = self.shots.iter().filter(|shot| shot.stage == Stage::Asked);
        asked.any(|shot| self.sits(&shot.session, output, pointer))
    }

    /// Whether a program's pictures are of this screen and of this kind.
    fn sits(&self, session: &ExtImageCopyCaptureSessionV1, output: &Output, pointer: bool) -> bool {
        let taking = self
            .takings
            .iter()
            .find(|taking| taking.session == *session);
        taking.is_some_and(|taking| taking.output == *output && taking.pointer == pointer)
    }

    /// The pictures of a screen to take now, given what is on it: those
    /// that are a program's first, and those for which something has
    /// changed since its last.
    pub(crate) fn due<E: Element>(
        &mut self,
        output: &Output,
        pointer: bool,
        elements: &[E],
    ) -> Vec<Due> {
        let mut due = Vec::new();
        for taking in &mut self.takings {
            let asked = self
                .shots
                .iter()
                .position(|shot| shot.stage == Stage::Asked && shot.session == taking.session);
            let here = taking.output == *output && taking.pointer == pointer;
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
            due.extend(damage.map(|damage| Due { shot, damage }));
        }
        due
    }

    /// Gives programs the pictures that were due. `pixels` is what the
    /// screen shows, four bytes to a pixel as red, green, blue and one to
    /// spare, or nothing if it could not be read.
    pub(crate) fn hand(
        &mut self,
        due: Vec<Due>,
        pixels: Option<&[u8]>,
        size: Size<i32, Physical>,
        clock: Duration,
    ) {
        self.scratch.clear();
        let (pixels, _) = pixels.unwrap_or_default().as_chunks::<4>();
        // A program is given blue first and red third.
        self.scratch.extend(
            pixels
                .iter()
                .flat_map(|[red, green, blue, _]| [*blue, *green, *red, u8::MAX]),
        );
        let size = (
            usize::try_from(size.w).unwrap_or_default(),
            usize::try_from(size.h).unwrap_or_default(),
        );
        for Due { shot, damage } in due {
            let shot = &mut self.shots[shot];
            shot.stage = Stage::Over;
            let buffer = shot.buffer.as_ref();
            let written = buffer.is_some_and(|buffer| {
                omorfos_shm::write(buffer, Format::Xrgb8888, size, &self.scratch)
            });
            if !written {
                shot.frame.failed(FailureReason::BufferConstraints);
                continue;
            }
            let taking = self.takings.iter_mut();
            for taking in taking.filter(|taking| taking.session == shot.session) {
                taking.taken = true;
            }
            shot.frame.transform(Transform::Normal);
            for part in damage {
                shot.frame
                    .damage(part.loc.x, part.loc.y, part.size.w, part.size.h);
            }
            let seconds = clock.as_secs();
            let high = u32::try_from(seconds >> 32).unwrap_or(u32::MAX);
            let low = u32::try_from(seconds & u64::from(u32::MAX)).unwrap_or(u32::MAX);
            shot.frame
                .presentation_time(high, low, clock.subsec_nanos());
            shot.frame.ready();
        }
    }

    /// Has a program take no more pictures through this.
    fn end(&mut self, session: &ExtImageCopyCaptureSessionV1) {
        let ended = self
            .takings
            .extract_if(.., |taking| taking.session == *session);
        self.ended.extend(ended.map(|taking| taking.output));
    }
}

impl Compositor {
    /// Whether a program is taking pictures of a screen, or was a moment
    /// ago: whether the mark that says so is on it.
    #[must_use]
    pub fn captured(&self, output: &Output) -> bool {
        self.capture.marks(output)
    }

    /// Keeps pictures of the screen in step with everything else: a
    /// program taking pictures of a screen that has gone is told it has
    /// stopped, one whose screen has changed size is told the new size,
    /// and nothing is given while the session is locked. Sees to the mark.
    pub(crate) fn settle_capture(&mut self, now: Instant) {
        let sizes = self.heads.iter();
        let sizes: Vec<_> = sizes
            .map(|head| (head.output.clone(), head.pixels()))
            .collect();
        let locked = self.is_locked();
        let capture = &mut self.capture;
        let gone = capture.takings.extract_if(.., |taking| {
            !sizes.iter().any(|(output, _)| *output == taking.output)
        });
        for taking in gone.collect::<Vec<_>>() {
            taking.session.stopped();
            capture.ended.push(taking.output);
        }
        for taking in &mut capture.takings {
            let now = sizes.iter().find(|(output, _)| *output == taking.output);
            let size = now.map_or(taking.size, |(_, size)| *size);
            if size != taking.size {
                taking.size = size;
                taking.tracker = OutputDamageTracker::from_output(&taking.output);
                taking.taken = false;
                taking.announce();
            }
        }
        // A picture asked for that will not be taken is said to have failed.
        for shot in &mut capture.shots {
            let live = capture
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

        let unpainted = |head: &&crate::outputs::Head| {
            self.capture.marks(&head.output) && self.capture.mark(head.scale).is_none()
        };
        let scales: Vec<Scale> = self
            .heads
            .iter()
            .filter(unpainted)
            .map(|head| head.scale)
            .collect();
        for scale in scales {
            let mark = Painted::new(&paint::capture_mark(&self.config().theme, scale));
            self.capture.marks.push((scale, mark));
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
        let output = source.data::<Option<Output>>().cloned().flatten();
        let head = output.and_then(|output| self.head(&output).filter(|_| may));
        let Some((output, size)) = head.map(|head| (head.output.clone(), head.pixels())) else {
            session.stopped();
            return;
        };
        let taking = Taking {
            session,
            tracker: OutputDamageTracker::from_output(&output),
            output,
            size,
            pointer,
            taken: false,
        };
        taking.announce();
        self.capture.takings.push(taking);
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
            data_init.init(source, Output::from_resource(&output));
        }
    }
}

impl Dispatch<ExtImageCaptureSourceV1, Option<Output>> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ExtImageCaptureSourceV1,
        _: <ExtImageCaptureSourceV1 as Resource>::Request,
        _: &Option<Output>,
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
            ManagerRequest::CreatePointerCursorSession { session, .. } => {
                data_init.init(session, ());
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtImageCopyCaptureCursorSessionV1, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &ExtImageCopyCaptureCursorSessionV1,
        request: CursorRequest,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        // The pointer by itself is not given as pictures: a program asks
        // for it to be in those of the screen. One that asks is told at
        // once that there will be none.
        if let CursorRequest::GetCaptureSession { session } = request {
            data_init.init(session, ()).stopped();
        }
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
                // All of the picture is written each time, so what the
                // program says is stale in its buffer needs no keeping.
                if x < 0 || y < 0 || width <= 0 || height <= 0 {
                    let said = "what is stale in a buffer cannot lie outside it or be nothing";
                    frame.post_error(FrameError::InvalidBufferDamage, said);
                }
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
