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
use omorfos_config::{Part, States};
use omorfos_core::WindowId;
use smithay::wayland::foreign_toplevel_list::ForeignToplevelHandle;

use crate::compositor::{Compositor, Painted, color};
use crate::paint;
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
        subject: Subject,
        pointer: bool,
        (output, size, scale): Standing,
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
        taking.stand((taking.output.clone(), size, scale));
        taking
    }

    /// Notes where what is taken stands now. If its size is not what it
    /// was, the program is told what a buffer has to be to take a picture
    /// in, and the next picture is all new.
    fn stand(&mut self, (output, size, scale): Standing) {
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
        self.session.shm_format(Format::Xrgb8888);
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
    stage: Stage,
}

/// A picture that is to be taken now, and what has changed since the one
/// before it.
#[derive(Debug)]
pub(crate) struct Due {
    shot: usize,
    damage: Damage,
    /// How much of the drawing it is: all of a screen, or as much from
    /// its top left corner as a window takes.
    size: Size<i32, Physical>,
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
            due.extend(damage.map(|damage| Due { shot, damage, size }));
        }
        due
    }

    /// Gives programs the pictures that were due. `pixels` is the drawing
    /// they are taken from, as wide and as high as `drawn`, four bytes to
    /// a pixel as red, green, blue and one to spare; or nothing, if it
    /// could not be read.
    pub(crate) fn hand(
        &mut self,
        due: Vec<Due>,
        pixels: Option<&[u8]>,
        drawn: Size<i32, Physical>,
        clock: Duration,
    ) {
        let across = |pixels: i32| usize::try_from(pixels).unwrap_or_default();
        // A row of the drawing, which is never of no length to cut by.
        let row = (across(drawn.w) * 4).max(4);
        for Due { shot, damage, size } in due {
            let size = (across(size.w), across(size.h));
            self.scratch.clear();
            // As much of each row, from the top, as the picture is wide.
            for row in pixels.unwrap_or_default().chunks(row).take(size.1) {
                let (row, _) = row.get(..size.0 * 4).unwrap_or_default().as_chunks::<4>();
                // A program is given blue first and red third.
                let turned = row.iter();
                let turned =
                    turned.flat_map(|[red, green, blue, _]| [*blue, *green, *red, u8::MAX]);
                self.scratch.extend(turned);
            }
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

    /// Where what pictures are taken of stands: a screen where it is, and
    /// a window on the screen that shows it, or on the first if none
    /// does, as large as it is and no larger than that screen. Nothing,
    /// if it has gone.
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
                    let size = mapped.window.geometry().size;
                    let size: Size<i32, Physical> = size.to_f64().to_physical(scale).to_i32_round();
                    let most = head.pixels();
                    let size = Size::from((size.w.clamp(1, most.w), size.h.clamp(1, most.h)));
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
            taking.stand(stands.unwrap_or((taking.output.clone(), taking.size, taking.scale)));
            taking.blank.update(taking.size.to_logical(1), veiled);
            gone
        });
        for taking in gone.collect::<Vec<_>>() {
            taking.session.stopped();
            capture.ended.push(taking.output);
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
        let subject = source.data::<Option<Subject>>().cloned().flatten();
        let subject = subject.filter(|_| may);
        let stands = subject.as_ref().and_then(|subject| self.stands(subject));
        let Some((subject, stands)) = subject.zip(stands) else {
            session.stopped();
            return;
        };
        let taking = Taking::of(session, subject, pointer, stands);
        self.capture.takings.push(taking);
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
