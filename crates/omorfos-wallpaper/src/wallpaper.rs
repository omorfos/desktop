//! The wallpaper as a program: a surface behind everything on each screen,
//! drawn again when what it is to show changes.

use std::error::Error;
use std::io;
use std::mem;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use omorfos_config::{Color, Easing, Fit, Part, States, Store, Transition, Watcher};
use omorfos_shell::{picture, pixels};
use omorfos_ui::{Canvas, Image, Scale};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use wayland_client::backend::ReadEventsGuard;
use wayland_client::protocol::{
    wl_buffer, wl_callback, wl_compositor, wl_output, wl_registry, wl_shm, wl_shm_pool,
    wl_subcompositor, wl_subsurface, wl_surface,
};
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle, delegate_noop};
use wayland_protocols::wp::alpha_modifier::v1::client::{
    wp_alpha_modifier_surface_v1, wp_alpha_modifier_v1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1, wp_fractional_scale_v1,
};
use wayland_protocols::wp::single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1;
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::cache::{Cache, Kept};
use crate::{Slides, lay};

/// How long a screen's picture is shown before another one drawn for it is
/// kept in its place, while a directory's pictures are shown in turn, so
/// that the disk is not written a screenful at each.
const KEEP_EVERY: Duration = Duration::from_mins(30);

/// Why the wallpaper could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The wallpaper: a connection to the compositor as a Wayland client, and
/// a surface on each screen that has something to show.
///
/// It does nothing by itself. Whoever has it calls [`Wallpaper::turn`]
/// when there may be something to do, or leaves it to
/// [`Wallpaper::serve`].
pub struct Wallpaper {
    connection: Connection,
    queue: EventQueue<Shell>,
    shell: Shell,
    /// Tells when the configuration may have changed. Absent when the
    /// configuration directory cannot be watched, such as when there is
    /// none.
    watcher: Option<Watcher>,
}

impl Wallpaper {
    /// A wallpaper on the Wayland connection `wayland`, as the
    /// configuration in `config` has it, with pictures named from `home`.
    /// The pictures of a directory are shuffled from `seed`.
    ///
    /// What each screen was given is kept in `cache`, if there is one, to
    /// be given again at once the next time it would be drawn the same; and
    /// a directory starts from the picture it was kept at.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection cannot be used.
    pub fn new(
        wayland: UnixStream,
        config: &Path,
        cache: Option<PathBuf>,
        home: Option<PathBuf>,
        seed: u64,
    ) -> Result<Self, Failure> {
        let connection = Connection::from_socket(wayland).map_err(Failure::from);
        connection.map(|connection| {
            let queue = connection.new_event_queue();
            connection.display().get_registry(&queue.handle(), ());
            Self {
                connection,
                queue,
                shell: Shell::new(Store::open(config), Cache::new(cache), home, seed),
                watcher: Watcher::new(config).ok(),
            }
        })
    }

    /// Does whatever there is to do, without waiting: hears what the
    /// compositor has said, takes in the configuration if it changed, moves
    /// on to the next picture if it is time, and draws the screens whose
    /// wallpaper changed.
    ///
    /// `now` is how long the wallpaper has been running, which is what the
    /// pictures of a directory change by.
    ///
    /// # Errors
    ///
    /// Returns an error once the compositor has gone or stopped making
    /// sense. The wallpaper is of no use after that.
    pub fn turn(&mut self, now: Duration) -> Result<(), Failure> {
        self.shell.now = now;
        self.hear()
            .and_then(|()| self.learn().map_err(Failure::from))
            .and_then(|()| {
                let shown = self.shell.show(&self.queue.handle());
                shown.map_err(Failure::from)
            })
            .and_then(|()| self.connection.flush().map_err(Failure::from))
    }

    /// What could not be shown since this was last asked, each said once.
    pub fn problems(&mut self) -> Vec<String> {
        mem::take(&mut self.shell.problems)
    }

    /// Runs the wallpaper, asleep until there is something to do, until it
    /// cannot go on. Returns why. What could not be shown is said on the
    /// standard error as it happens.
    pub fn serve(&mut self) -> Failure {
        use std::io::Write;

        let started = Instant::now();
        loop {
            let now = started.elapsed();
            if let Err(failure) = self.turn(now) {
                return failure;
            }
            for problem in self.problems() {
                // If even that cannot be said, there is nobody to say it to.
                let _ = writeln!(io::stderr(), "omorfos-wallpaper: {problem}");
            }
            let longest = self.deadline().map(|due| due.saturating_sub(now));
            self.wait(longest);
        }
    }

    /// When there will next be something to do without anything having
    /// happened: when a directory's next picture is to be made ready, or is
    /// due.
    #[must_use]
    pub fn deadline(&self) -> Option<Duration> {
        let slides = self.shell.slides.iter();
        let slides = slides.filter_map(|slides| slides.choosing().or(slides.due()));
        // A fade is carried along by the compositor saying it has shown
        // each step, and finished by then even if it stops saying so, as
        // it may with the screen off.
        let lasts = self.shell.fade().lasts();
        let faces = self
            .shell
            .screens
            .iter()
            .filter_map(|screen| screen.face.as_ref());
        let fading = faces.filter_map(|face| face.fade.as_ref());
        let ends = fading.map(|fade| fade.since + lasts);
        slides.chain(ends).min()
    }

    /// Sleeps until the connection has something to read, the
    /// configuration changes, or `longest` has passed.
    fn wait(&self, longest: Option<Duration>) {
        let readable = PollFlags::IN;
        let mut sources = vec![PollFd::new(&self.connection, readable)];
        let watcher = self.watcher.iter();
        sources.extend(watcher.map(|watcher| PollFd::new(watcher, readable)));
        // A wait that is interrupted is a wake-up like any other, and
        // anything worse shows in the turn that follows.
        let longest = longest.and_then(|longest| Timespec::try_from(longest).ok());
        let _ = poll(&mut sources, longest.as_ref());
    }

    /// Reads what the compositor sent and acts on it.
    fn hear(&mut self) -> Result<(), Failure> {
        // Whether reading with nothing to read counts as a failure depends
        // on how the library was built, so the outcome is not looked at
        // here. A real failure is kept by the connection and comes back
        // from everything after it, down to the flush that ends the turn.
        let _ = self.queue.prepare_read().map(ReadEventsGuard::read);
        let handled = self.queue.dispatch_pending(&mut self.shell);
        handled.map(drop).map_err(Failure::from)
    }

    /// Takes in the configuration if it changed.
    fn learn(&mut self) -> io::Result<()> {
        let watcher = self.watcher.as_mut();
        let changed = watcher.map_or(Ok(false), Watcher::drain);
        changed.map(|changed| {
            if changed && self.shell.store.reload() {
                self.shell.stale = true;
                // What is drawn from it now is kept at once.
                let faces = self
                    .shell
                    .screens
                    .iter_mut()
                    .filter_map(|screen| screen.face.as_mut());
                faces.for_each(|face| face.kept = None);
            }
        })
    }
}

/// Runs a wallpaper in the session whose Wayland socket is at `wayland`,
/// as the configuration in `config` has it, keeping what it draws in
/// `cache`, until it cannot go on. Returns why.
pub(crate) fn run(
    wayland: &Path,
    config: &Path,
    cache: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Failure {
    // Chance enough for a shuffle: what the clock says, which is never the
    // same twice for a person to see.
    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    let seed = seed.map_or(0, |since| since.as_nanos());
    #[expect(
        clippy::cast_possible_truncation,
        reason = "only the part that changes most is wanted"
    )]
    let seed = seed as u64;
    let wallpaper = omorfos_shell::connect(wayland)
        .map_err(Failure::from)
        .and_then(|wayland| Wallpaper::new(wayland, config, cache, home, seed));
    match wallpaper {
        Ok(mut wallpaper) => wallpaper.serve(),
        Err(failure) => failure,
    }
}

/// What the Wayland connection's events act on: the screens and their
/// surfaces, and what is needed to draw them.
struct Shell {
    compositor: Option<wl_compositor::WlCompositor>,
    shm: Option<wl_shm::WlShm>,
    layers: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    viewporter: Option<wp_viewporter::WpViewporter>,
    scales: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    pixels: Option<wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1>,
    subsurfaces: Option<wl_subcompositor::WlSubcompositor>,
    alphas: Option<wp_alpha_modifier_v1::WpAlphaModifierV1>,
    screens: Vec<Screen>,
    store: Store,
    /// What each screen was last given.
    cache: Cache,
    home: Option<PathBuf>,
    /// What screens are to be given when a directory's next picture is
    /// shown, made ready before it is due.
    ready: Vec<(View, Drawing)>,
    /// The pictures shown, of each file or directory the configuration
    /// names.
    slides: Vec<Slides>,
    /// What the next pictures shown are shuffled from.
    seed: u64,
    /// How long the wallpaper has been running, as of this turn.
    now: Duration,
    /// What could not be shown, yet to be said.
    problems: Vec<String>,
    /// Whether anything happened that may change what a screen shows.
    stale: bool,
}

/// A screen, by the number the compositor announced it under.
struct Screen {
    global: u32,
    /// What the compositor calls it, once it has said.
    name: String,
    output: wl_output::WlOutput,
    /// Its surface, while it has something to show.
    face: Option<Face>,
}

/// What a screen shows: the picture, if there is one, laid as `fit` says
/// on `color`.
#[derive(Debug, Clone, PartialEq)]
struct Wanted {
    picture: Option<PathBuf>,
    fit: Fit,
    color: Color,
}

/// A screen's surface.
struct Face {
    surface: wl_surface::WlSurface,
    layer: zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
    viewport: wp_viewport::WpViewport,
    fraction: wp_fractional_scale_v1::WpFractionalScaleV1,
    /// How large the compositor made it, in units of layout. Nothing until
    /// it has said.
    size: (u32, u32),
    /// How many pixels the screen has to a unit.
    scale: Scale,
    /// What is on it now, drawn at that size and scale.
    shown: Option<View>,
    /// The picture of that, which the compositor is reading from.
    buffer: Option<wl_buffer::WlBuffer>,
    /// When what it was given was last kept, since the configuration last
    /// changed.
    kept: Option<Duration>,
    /// What is fading in over what it showed, while it does.
    fade: Option<Fade>,
}

/// What a screen is to show, fading in over what it showed on a surface of
/// its own above it, fainter at first and then not at all. Once it has,
/// it is shown on the screen's surface, and this one goes.
struct Fade {
    surface: wl_surface::WlSurface,
    subsurface: wl_subsurface::WlSubsurface,
    viewport: wp_viewport::WpViewport,
    alpha: wp_alpha_modifier_surface_v1::WpAlphaModifierSurfaceV1,
    /// What it shows, and the same again for the screen's own surface,
    /// since a buffer is each surface's own to give back.
    buffer: wl_buffer::WlBuffer,
    after: wl_buffer::WlBuffer,
    /// Since when.
    since: Duration,
}

/// What a screen shows, at a size and a scale.
type View = (Wanted, (u32, u32), Scale);

/// What a screen is given to show.
enum Drawing {
    /// Its colour alone.
    Plain,
    /// A picture laid on it.
    Drawn(Canvas),
    /// What it was given before, as it was kept.
    Kept(Kept),
}

/// A picture as read for a screen: the file, and how it is laid on a
/// screen of so many pixels.
type Reading = (PathBuf, Fit, (u32, u32));

/// What it takes to put a surface on a screen.
struct Tools<'a> {
    compositor: &'a wl_compositor::WlCompositor,
    shm: &'a wl_shm::WlShm,
    layers: &'a zwlr_layer_shell_v1::ZwlrLayerShellV1,
    viewporter: &'a wp_viewporter::WpViewporter,
    scales: &'a wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    pixels: &'a wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1,
    subsurfaces: &'a wl_subcompositor::WlSubcompositor,
    alphas: &'a wp_alpha_modifier_v1::WpAlphaModifierV1,
    handle: &'a QueueHandle<Shell>,
}

impl Shell {
    fn new(store: Store, cache: Cache, home: Option<PathBuf>, seed: u64) -> Self {
        Self {
            compositor: None,
            shm: None,
            layers: None,
            viewporter: None,
            scales: None,
            pixels: None,
            subsurfaces: None,
            alphas: None,
            screens: Vec::new(),
            store,
            cache,
            home,
            ready: Vec::new(),
            slides: Vec::new(),
            seed,
            now: Duration::ZERO,
            problems: Vec::new(),
            // Nothing is drawn yet.
            stale: true,
        }
    }

    /// How what a screen shows fades into what it shows next.
    fn fade(&self) -> Transition {
        let fade = self.store.current().general.wallpaper.fade.get();
        Transition {
            duration: fade,
            easing: Easing::EaseInOut,
        }
    }

    /// Lets go of a screen that is no longer there.
    fn forget(&mut self, global: u32) {
        let gone = self
            .screens
            .extract_if(.., |screen| screen.global == global);
        gone.filter_map(|screen| screen.face).for_each(Face::close);
    }

    /// The surface on a screen, if it has one.
    fn faces(&mut self, global: u32) -> impl Iterator<Item = &mut Face> {
        let screens = self.screens.iter_mut();
        let screens = screens.filter(move |screen| screen.global == global);
        screens.filter_map(|screen| screen.face.as_mut())
    }

    /// Notes how many pixels a screen has to a unit. Returns whether that
    /// is news.
    fn scaled(&mut self, global: u32, scale: Scale) -> bool {
        self.faces(global).fold(false, |changed, face| {
            let before = mem::replace(&mut face.scale, scale);
            changed || before != scale
        })
    }

    /// The pictures each screen is to show, from what the configuration
    /// names: those already being shown kept as they are, and the next of
    /// each that is due. Returns whether any is another picture.
    fn turn_slides(&mut self) -> bool {
        let wallpaper = &self.store.current().general.wallpaper;
        let every = Duration::from_secs(u64::from(wallpaper.every.get()));
        // A screen is not shown anything until it is known by its name.
        let named = self.screens.iter().filter(|screen| !screen.name.is_empty());
        let sources: Vec<(&str, PathBuf)> = named
            .filter_map(|screen| {
                let picture = wallpaper.on(&screen.name).picture?;
                Some((screen.name.as_str(), picture.path(self.home.as_deref())?))
            })
            .collect();
        let mut wanted: Vec<PathBuf> = sources.iter().map(|(_, source)| source.clone()).collect();
        wanted.sort();
        wanted.dedup();
        let mut kept: Vec<Slides> = Vec::new();
        let mut changed = false;
        for source in wanted {
            let order = wallpaper.order;
            let old = self
                .slides
                .iter()
                .position(|slides| slides.are(&source, order, every));
            let slides = if let Some(at) = old {
                let mut slides = self.slides.swap_remove(at);
                changed |= slides.turn(self.now);
                slides
            } else {
                changed = true;
                self.seed = self.seed.wrapping_add(1);
                // Where a screen showing it was left, as what it was last
                // given says.
                let showing = sources.iter().filter(|(_, shows)| *shows == source);
                let mut left = showing.filter_map(|(name, _)| self.cache.picture(name));
                let from = left.find(|picture| picture.parent() == Some(source.as_path()));
                Slides::new(source, order, every, self.now, self.seed, from.as_deref())
            };
            kept.push(slides);
        }
        self.slides = kept;
        changed
    }

    /// What the screen called `name` is to show: now, or once the pictures
    /// chosen to come next are shown, if `coming`.
    fn wanted(&self, name: &str, coming: bool) -> Option<Wanted> {
        let config = self.store.current();
        let shown = config.general.wallpaper.on(name);
        let source = shown
            .picture
            .and_then(|picture| picture.path(self.home.as_deref()));
        let file = source.and_then(|source| {
            let mut slides = self.slides.iter();
            let slides = slides.find(|slides| slides.shows(&source));
            let which = if coming {
                Slides::coming
            } else {
                Slides::shown
            };
            slides.and_then(which).map(Path::to_owned)
        });
        // With neither a picture nor a colour, the compositor's desktop is
        // shown, and there is nothing to draw.
        let desktop = config.theme.style(Part::Desktop, States::NONE).background;
        let named = !name.is_empty() && (shown.picture.is_some() || shown.color.is_some());
        named.then(|| Wanted {
            picture: file,
            fit: shown.fit,
            color: shown.color.unwrap_or(desktop),
        })
    }

    /// Puts a surface on every screen that has something to show and none
    /// yet, takes it from one that has nothing, and draws those that no
    /// longer show what they should. Then makes ready what screens are to
    /// show next, if the next pictures of a directory were chosen.
    fn show(&mut self, handle: &QueueHandle<Self>) -> io::Result<()> {
        let moved = self.turn_slides();
        let now = self.now;
        let transition = self.fade();
        let surfaces = self
            .screens
            .iter_mut()
            .filter_map(|screen| screen.face.as_mut());
        surfaces.for_each(|surface| surface.step(now, transition, handle));
        let mut chosen = false;
        for slides in &mut self.slides {
            chosen |= slides.choose(now);
        }
        if !mem::take(&mut self.stale) && !moved && !chosen {
            return Ok(());
        }
        let globals = self.compositor.as_ref().zip(self.shm.as_ref());
        let globals = globals.zip(self.layers.as_ref());
        let globals = globals.zip(self.viewporter.as_ref().zip(self.scales.as_ref()));
        let globals = globals.zip(self.pixels.as_ref());
        let globals = globals.zip(self.subsurfaces.as_ref().zip(self.alphas.as_ref()));
        // Until the compositor has offered everything, there is nothing to
        // draw with. Each offer makes this worth trying again.
        let Some((
            ((((compositor, shm), layers), (viewporter, scales)), pixels),
            (subsurfaces, alphas),
        )) = globals
        else {
            return Ok(());
        };
        let tools = Tools {
            compositor,
            shm,
            layers,
            viewporter,
            scales,
            pixels,
            subsurfaces,
            alphas,
            handle,
        };
        let wanted: Vec<Option<Wanted>> = self
            .screens
            .iter()
            .map(|screen| self.wanted(&screen.name, false))
            .collect();
        let mut painter = Painter::default();
        let mut ready = mem::take(&mut self.ready);
        let mut screens = self.screens.iter_mut().zip(wanted);
        let drawn: io::Result<()> = screens.try_for_each(|(screen, wanted)| {
            let Some(wanted) = wanted else {
                screen.face.take().into_iter().for_each(Face::close);
                return Ok(());
            };
            let face = screen.face.take();
            let face = face.unwrap_or_else(|| Face::open(&tools, screen));
            let face = screen.face.insert(face);
            let Some(view) = face.due(wanted) else {
                return Ok(());
            };
            let made = ready.iter().position(|(made, _)| *made == view);
            let made = made.map(|at| ready.swap_remove(at).1);
            let about = view.0.picture.as_deref().zip(description(&view));
            let recalled = || {
                let (picture, drawn) = about.as_ref()?;
                self.cache.recall(&screen.name, picture, drawn)
            };
            let drawing = made.or_else(|| recalled().map(Drawing::Kept));
            let drawing = drawing.unwrap_or_else(|| painter.paint(&view, &mut self.problems));
            face.draw(&tools, &view, &drawing, (now, transition))
                .map(|()| {
                    // Kept for the next session, but not so often that a
                    // directory shown in turn writes a screenful each time.
                    let due = face
                        .kept
                        .is_none_or(|kept| now.saturating_sub(kept) >= KEEP_EVERY);
                    if let (Drawing::Drawn(canvas), Some((picture, drawn)), true) =
                        (&drawing, &about, due)
                    {
                        face.kept = Some(now);
                        let pixels = ((canvas.width(), canvas.height()), canvas.bytes());
                        let kept = self.cache.keep(&screen.name, picture, drawn, pixels);
                        if let Err(why) = kept {
                            let problem =
                                format!("cannot keep what `{}` shows: {why}", screen.name);
                            self.problems.push(problem);
                        }
                    }
                })
        });
        drawn.map(|()| {
            // What was made ready for the pictures before has been shown,
            // or is no longer wanted.
            if !moved {
                self.ready = ready;
            }
            if chosen {
                self.make_ready(&mut painter);
            }
        })
    }

    /// Draws ahead of time what each screen is to show once the pictures
    /// chosen to come next are shown, at the size and scale it has now.
    fn make_ready(&mut self, painter: &mut Painter) {
        for screen in &self.screens {
            let coming = self.wanted(&screen.name, true);
            let face = screen
                .face
                .as_ref()
                .filter(|face| face.size.0 > 0 && face.size.1 > 0);
            let view = coming
                .zip(face)
                .map(|(coming, face)| (coming, face.size, face.scale));
            let view = view.filter(|(coming, ..)| coming.picture.is_some());
            let fresh = view.filter(|view| self.ready.iter().all(|(made, _)| made != view));
            if let Some(view) = fresh {
                let drawing = painter.paint(&view, &mut self.problems);
                self.ready.push((view, drawing));
            }
        }
    }
}

/// What a screen is shown described, as far as it is not in the picture's
/// name: how large the picture is and when it was last changed, and how it
/// is laid on a screen of what size. Nothing if it is not there to say.
fn description((wanted, size, scale): &View) -> Option<String> {
    let file = wanted.picture.as_deref()?;
    let about = std::fs::metadata(file).ok()?;
    let changed = about.modified().ok();
    Some(format!(
        "{} {changed:?} {:?} {:?} {size:?} {scale:?}",
        about.len(),
        wanted.fit,
        wanted.color
    ))
}

/// Draws pictures on screens, reading each as large as a screen needs it
/// once, however many screens it is drawn on in a turn.
#[derive(Default)]
struct Painter {
    read: Vec<(Reading, Option<Image>)>,
}

impl Painter {
    /// What a screen is given for `view`: its picture laid on it, or its
    /// colour alone if the picture cannot be read, which is said in
    /// `problems`, once.
    fn paint(&mut self, view: &View, problems: &mut Vec<String>) -> Drawing {
        let (wanted, size, scale) = view;
        // As many pixels as the canvas it is laid on.
        let pixels = |units: u32| scale.pixels(units.cast_signed()).unsigned_abs();
        let room = (pixels(size.0), pixels(size.1));
        let file = wanted.picture.as_deref();
        // Nor is one that could not be read tried again, and said again,
        // for another screen.
        let failed = |file: &&Path| {
            let mut tried = self.read.iter();
            tried.any(|((path, ..), image)| path == file && image.is_none())
        };
        let image = file.filter(|file| !failed(file)).and_then(|file| {
            let reading = (file.to_owned(), wanted.fit, room);
            let known = self.read.iter().position(|(read, _)| *read == reading);
            let at = known.unwrap_or_else(|| {
                let image = crate::picture::read(file, wanted.fit, room);
                let image = image.map_err(|why| {
                    problems.push(format!("cannot show `{}`: {why}", file.display()));
                });
                self.read.push((reading, image.ok()));
                self.read.len() - 1
            });
            self.read.get(at).and_then(|(_, image)| image.as_ref())
        });
        image.map_or(Drawing::Plain, |image| {
            Drawing::Drawn(lay::paint(image, wanted.fit, wanted.color, *size, *scale))
        })
    }
}

impl Face {
    /// Asks for a surface behind everything on a screen, as large as it.
    fn open(tools: &Tools<'_>, screen: &Screen) -> Self {
        let handle = tools.handle;
        let surface = tools.compositor.create_surface(handle, ());
        let layer = tools.layers.get_layer_surface(
            &surface,
            Some(&screen.output),
            zwlr_layer_shell_v1::Layer::Background,
            "wallpaper".to_owned(),
            handle,
            screen.global,
        );
        let edges = zwlr_layer_surface_v1::Anchor::all();
        layer.set_anchor(edges);
        layer.set_size(0, 0);
        // Under the panel too: what keeps windows clear of it does not
        // keep the wallpaper clear.
        layer.set_exclusive_zone(-1);
        let viewport = tools.viewporter.get_viewport(&surface, handle, ());
        let scales = tools.scales;
        let fraction = scales.get_fractional_scale(&surface, handle, screen.global);
        // The compositor answers this with how large it is to be.
        surface.commit();
        Self {
            surface,
            layer,
            viewport,
            fraction,
            size: (0, 0),
            scale: Scale::ONE,
            shown: None,
            buffer: None,
            kept: None,
            fade: None,
        }
    }

    /// Gives the surface back.
    fn close(mut self) {
        self.fade.take().into_iter().for_each(Fade::close);
        self.fraction.destroy();
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
        self.buffer.iter().for_each(wl_buffer::WlBuffer::destroy);
    }

    /// What is to be drawn for `wanted` to be shown, at the size and scale
    /// the surface has: nothing if it already shows just that, or if the
    /// compositor has yet to say how large it is.
    fn due(&self, wanted: Wanted) -> Option<View> {
        let view = (wanted, self.size, self.scale);
        let sized = self.size.0 > 0 && self.size.1 > 0;
        (sized && self.shown.as_ref() != Some(&view)).then_some(view)
    }

    /// Shows `drawing`, as what `view` is: faded in over what it showed
    /// before, as `fade` says from `now`, if it showed something at the same
    /// size and scale; at once otherwise.
    fn draw(
        &mut self,
        tools: &Tools<'_>,
        view: &View,
        drawing: &Drawing,
        (now, fade): (Duration, Transition),
    ) -> io::Result<()> {
        let (wanted, size, scale) = view;
        let buffer = || match drawing {
            // A screen of one colour is a single pixel of it, made as large
            // as the screen: there is nothing to gain from drawing millions
            // of the same.
            Drawing::Plain => Ok(plain(tools, wanted.color)),
            Drawing::Drawn(canvas) => picture(tools.shm, canvas, tools.handle),
            Drawing::Kept(kept) => pixels(tools.shm, kept.size, &kept.pixels, tools.handle),
        };
        let placed = self.shown.as_ref().map(|(_, size, scale)| (*size, *scale));
        let fades = placed == Some((*size, *scale)) && fade.lasts() > Duration::ZERO;
        // One fading in already is done with at once, for this to fade in
        // over it.
        self.step(Duration::MAX, fade, tools.handle);
        let units = (size.0.cast_signed(), size.1.cast_signed());
        // Fading in, it is given twice: once for the surface it fades in
        // on and once for the screen's own, which shows it after.
        let given = buffer().and_then(|first| {
            let second = fades.then(buffer).transpose();
            second.map(|second| (first, second))
        });
        given.map(|(buffer, after)| {
            match after {
                Some(after) => {
                    self.fade = Some(Fade::open(tools, &self.surface, units, buffer, after, now));
                }
                None => self.put(buffer, units),
            }
            self.shown = Some(view.clone());
        })
    }

    /// Shows `buffer` on the screen's own surface, `size` units of layout.
    fn put(&mut self, buffer: wl_buffer::WlBuffer, (width, height): (i32, i32)) {
        self.surface.attach(Some(&buffer), 0, 0);
        // However many pixels the picture has, it covers the screen.
        self.viewport.set_destination(width, height);
        self.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        self.surface.commit();
        // The picture before this one is let go of when the compositor
        // says it has finished with it.
        self.buffer = Some(buffer);
    }

    /// Takes what is fading in a step further, as far as `fade` says it is
    /// by `now`, and asks to hear once the compositor has shown it; or, if
    /// it has faded in all the way, shows it on the screen's own surface.
    fn step(&mut self, now: Duration, fade: Transition, handle: &QueueHandle<Shell>) {
        let Some(fading) = self.fade.take() else {
            return;
        };
        let progress = fade.progress(now.saturating_sub(fading.since));
        if progress < 1.0 {
            fading.show(progress, handle);
            self.fade = Some(fading);
            return;
        }
        let (width, height) = self
            .shown
            .as_ref()
            .map(|(_, size, _)| *size)
            .unwrap_or_default();
        self.put(
            fading.after.clone(),
            (width.cast_signed(), height.cast_signed()),
        );
        fading.close();
    }
}

impl Fade {
    /// Puts `buffer` on a surface of its own over `under`, as large as
    /// `units`, not yet to be seen; and keeps `after` for `under`.
    fn open(
        tools: &Tools<'_>,
        under: &wl_surface::WlSurface,
        (width, height): (i32, i32),
        buffer: wl_buffer::WlBuffer,
        after: wl_buffer::WlBuffer,
        since: Duration,
    ) -> Self {
        let handle = tools.handle;
        let surface = tools.compositor.create_surface(handle, ());
        let subsurface = tools
            .subsurfaces
            .get_subsurface(&surface, under, handle, ());
        // It changes as it fades, without the screen's surface.
        subsurface.set_desync();
        let viewport = tools.viewporter.get_viewport(&surface, handle, ());
        viewport.set_destination(width, height);
        let alpha = tools.alphas.get_surface(&surface, handle, ());
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
        let fade = Self {
            surface,
            subsurface,
            viewport,
            alpha,
            buffer,
            after,
            since,
        };
        fade.show(0.0, handle);
        // Where it is on the screen's surface comes with that one's commit.
        under.commit();
        fade
    }

    /// Shows it `progress` of the way from not at all to wholly, and asks
    /// to hear once the compositor has.
    fn show(&self, progress: f32, handle: &QueueHandle<Shell>) {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a share, between none and all, of the largest there is"
        )]
        let multiplier = (f64::from(progress) * f64::from(u32::MAX)) as u32;
        self.alpha.set_multiplier(multiplier);
        self.surface.frame(handle, ());
        self.surface.commit();
    }

    /// Gives its surface back, and what it showed.
    fn close(self) {
        self.alpha.destroy();
        self.viewport.destroy();
        self.subsurface.destroy();
        self.surface.destroy();
        self.buffer.destroy();
    }
}

/// A buffer of a single pixel of `color`, which the compositor holds and
/// no memory need be shared for.
fn plain(tools: &Tools<'_>, color: Color) -> wl_buffer::WlBuffer {
    // Each channel is a share of all thirty-two bits, with the colours
    // multiplied by the opacity.
    let share = |channel: u8| {
        let multiplied = u32::from(channel) * u32::from(color.alpha) / 255;
        multiplied * 0x0101_0101
    };
    let [red, green, blue] = [color.red, color.green, color.blue].map(share);
    let alpha = u32::from(color.alpha) * 0x0101_0101;
    let pixels = tools.pixels;
    pixels.create_u32_rgba_buffer(red, green, blue, alpha, tools.handle, ())
}

impl Dispatch<wl_registry::WlRegistry, ()> for Shell {
    fn event(
        shell: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::GlobalRemove { name } = event {
            shell.forget(name);
        }
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            shell.stale = true;
            match interface.as_str() {
                "wl_compositor" => {
                    shell.compositor = Some(registry.bind(name, version.min(4), handle, ()));
                }
                "wl_shm" => shell.shm = Some(registry.bind(name, 1, handle, ())),
                "zwlr_layer_shell_v1" => {
                    shell.layers = Some(registry.bind(name, 1, handle, ()));
                }
                "wp_viewporter" => shell.viewporter = Some(registry.bind(name, 1, handle, ())),
                "wp_fractional_scale_manager_v1" => {
                    shell.scales = Some(registry.bind(name, 1, handle, ()));
                }
                "wp_single_pixel_buffer_manager_v1" => {
                    shell.pixels = Some(registry.bind(name, 1, handle, ()));
                }
                "wl_subcompositor" => shell.subsurfaces = Some(registry.bind(name, 1, handle, ())),
                "wp_alpha_modifier_v1" => shell.alphas = Some(registry.bind(name, 1, handle, ())),
                "wl_output" => shell.screens.push(Screen {
                    global: name,
                    name: String::new(),
                    // From its fourth version an output says what it is
                    // called.
                    output: registry.bind(name, version.min(4), handle, name),
                    face: None,
                }),
                _ => {}
            }
        }
    }
}

impl Dispatch<wl_output::WlOutput, u32> for Shell {
    fn event(
        shell: &mut Self,
        _: &wl_output::WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            let screens = shell.screens.iter_mut();
            let named = screens.filter(|screen| screen.global == *global);
            named.for_each(|screen| screen.name.clone_from(&name));
            shell.stale = true;
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, u32> for Shell {
    fn event(
        shell: &mut Self,
        layer: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_layer_surface_v1::Event::Configure {
            serial,
            width,
            height,
        } = event
        {
            layer.ack_configure(serial);
            shell
                .faces(*global)
                .for_each(|face| face.size = (width, height));
            shell.stale = true;
            return;
        }
        // The only other thing it says is that the compositor closed it,
        // which it does when the screen goes.
        shell.forget(*global);
    }
}

impl Dispatch<wp_fractional_scale_v1::WpFractionalScaleV1, u32> for Shell {
    fn event(
        shell: &mut Self,
        _: &wp_fractional_scale_v1::WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // The scale comes in parts of 120 to one. Told the one it already
        // has, a screen has nothing to do over.
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event
            && shell.scaled(*global, Scale::new(f64::from(scale) / 120.0))
        {
            shell.stale = true;
        }
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for Shell {
    fn event(
        _: &mut Self,
        buffer: &wl_buffer::WlBuffer,
        _: wl_buffer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // All a buffer ever says is that the compositor has finished with
        // it, which it does once a newer picture has taken its place.
        buffer.destroy();
    }
}

delegate_noop!(Shell: wl_compositor::WlCompositor);
delegate_noop!(Shell: ignore wl_shm::WlShm);
delegate_noop!(Shell: wl_shm_pool::WlShmPool);
delegate_noop!(Shell: ignore wl_surface::WlSurface);
delegate_noop!(Shell: zwlr_layer_shell_v1::ZwlrLayerShellV1);
delegate_noop!(Shell: wp_viewporter::WpViewporter);
delegate_noop!(Shell: wp_viewport::WpViewport);
delegate_noop!(Shell: wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1);
delegate_noop!(Shell: wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1);
delegate_noop!(Shell: wl_subcompositor::WlSubcompositor);
delegate_noop!(Shell: wl_subsurface::WlSubsurface);
delegate_noop!(Shell: wp_alpha_modifier_v1::WpAlphaModifierV1);
delegate_noop!(Shell: wp_alpha_modifier_surface_v1::WpAlphaModifierSurfaceV1);
// That a step of a fade has been shown only wakes the wallpaper, which
// takes the next.
delegate_noop!(Shell: ignore wl_callback::WlCallback);
