//! What applications ask the desktop through the portals, answered for
//! omorfos.
//!
//! An application that wants to know something only the desktop can say,
//! or to do something only the desktop can let it, asks
//! `xdg-desktop-portal`, which every desktop shares. That passes the
//! question to a program of the desktop's own, and this is omorfos's.
//!
//! So far it answers one thing: how the desktop looks, light or dark and
//! in what colour, so that applications can look the same. It reads that
//! from the theme, and tells applications when the theme changes.

mod appearance;
mod bus;
mod card;
mod feed;
mod prompt;
mod screencast;
mod stream;

use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

pub use appearance::Appearance;
pub use bus::{Asked, LOOKS, Look, NAME, PATH, Portal, REQUEST, Response, SCREENSHOT, SETTINGS};
use card::Card;
pub use feed::{Feed, Seen};
use omorfos_config::{Store, Watcher};
use omorfos_pipewire::{Held, Picture};
pub use prompt::{Answer, Desk, Question, Still, asking, button_at, paint};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
pub use screencast::{Pointer, SCREENCAST, SESSION, streams};
pub use stream::{Pipes, Share};
use zbus::message::Message;
use zbus::zvariant::OwnedObjectPath;

/// Why the program cannot go on, in words for the user.
pub type Failure = Box<dyn Error + Send + Sync>;

/// How the desktop whose configuration is in a folder looks.
fn look(config: &Path) -> Appearance {
    Appearance::of(&Store::open(config).current().theme)
}

/// Answers the portals for the session the environment describes, until
/// it cannot go on, and returns why.
///
/// `variable` looks a variable of the environment up. The compositor is
/// found by `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`, unless it handed over
/// a connection; the bus is at `DBUS_SESSION_BUS_ADDRESS`; and the
/// configuration is found as the compositor finds it, by `XDG_CONFIG_HOME`
/// and `HOME`.
///
/// It goes on for as long as the compositor does. The bus outlives a
/// session, and without that a copy would be left answering for a session
/// that has ended.
#[must_use]
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Failure {
    let (home, config) = (variable("HOME"), variable("XDG_CONFIG_HOME"));
    let config = omorfos_config::config_dir(config.as_deref(), home.as_deref());
    let (runtime, display) = (variable("XDG_RUNTIME_DIR"), variable("WAYLAND_DISPLAY"));
    let wayland = runtime.clone().zip(display.clone());
    let wayland = wayland.map(|(runtime, display)| Path::new(&runtime).join(display));
    let remote = variable("PIPEWIRE_REMOTE").and_then(|remote| remote.into_string().ok());
    let address = variable("DBUS_SESSION_BUS_ADDRESS");
    let address = address.as_deref().and_then(|address| address.to_str());
    // Where applications' desktop entries are, to name one that asks by.
    let (data_home, data_dirs) = (variable("XDG_DATA_HOME"), variable("XDG_DATA_DIRS"));
    let entries = omorfos_icons::data_directories(
        data_home.as_deref(),
        home.as_deref(),
        data_dirs.as_deref(),
    );
    // Where the compositor says where windows are, to pick one by.
    let socket = variable("OMORFOS_SOCKET");
    let socket =
        omorfos_ipc::socket_path(socket.as_deref(), runtime.as_deref(), display.as_deref());
    let found = config.zip(wayland).zip(home.map(PathBuf::from));
    let Some(((config, wayland), home)) = found else {
        return "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is \
                not set"
            .into();
    };
    // Pictures go where the screenshot tool puts them.
    let named = config.parent().map(|above| above.join("user-dirs.dirs"));
    let named = named.and_then(|file| fs::read_to_string(file).ok());
    let folder = omorfos_screenshot::pictures(named.as_deref(), &home).join("Screenshots");
    let session = omorfos_shell::connect(&wayland);
    let session = session.map_err(|error| Failure::from(format!("{}: {error}", wayland.display())));
    let asked = config.clone();
    let portal = session.and_then(|session| {
        let portal = Portal::open(address, Box::new(move || look(&asked)));
        portal.map(|portal| (portal, session))
    });
    let portal = portal.and_then(|(portal, session)| {
        let desk = Desk::new(session, Store::open(&config));
        desk.map(|desk| (portal, desk))
    });
    match portal {
        Ok((portal, desk)) => serve(&portal, &config, desk, (&folder, remote, entries, socket)),
        Err(failure) => failure,
    }
}

/// An address for a file, as one is given to an application: anything in
/// its path other than letters, numbers, `/` and `-._~` written as a byte
/// in hexadecimal.
#[must_use]
pub fn address(path: &Path) -> String {
    let bytes = path.as_os_str().as_encoded_bytes().iter();
    let written = bytes.map(|byte| {
        let plain = byte.is_ascii_alphanumeric() || b"/-._~".contains(byte);
        if plain {
            char::from(*byte).to_string()
        } else {
            format!("%{byte:02X}")
        }
    });
    format!("file://{}", written.collect::<String>())
}

/// Saves what is left of `shots` in `folder`, as one picture, and returns
/// its address. Nothing, if there is nothing or it could not be saved.
fn kept(shots: &[omorfos_screenshot::Shot], folder: &Path) -> Option<String> {
    let shot = (!shots.is_empty()).then(|| omorfos_screenshot::together(shots));
    let saved = shot.and_then(|shot| {
        let saved = fs::create_dir_all(folder);
        let stamp = omorfos_screenshot::stamp();
        saved
            .and_then(|()| omorfos_screenshot::save(folder, &stamp, None, &shot))
            .ok()
    });
    saved.map(|path| address(&path))
}

/// What a picture is of, once picked: the pictures, nothing if nothing
/// was picked, or why there are none.
type Picked = Result<Option<Vec<omorfos_screenshot::Shot>>, omorfos_screenshot::Failure>;

/// What came of picking: what a picture is of, or why there is none; or
/// which screen to share, by what it is called. Nothing if nothing was
/// picked.
enum Chose {
    Picture(Picked),
    Screen(Option<String>),
}

/// What is being picked, on a thread of its own, for what was asked for:
/// the call, the request it may be withdrawn by, and where it says what
/// was picked. For a screen to share, the session it is shared in and how
/// the pointer is shown.
struct Picking {
    call: Message,
    request: OwnedObjectPath,
    done: Receiver<Chose>,
    share: Option<(OwnedObjectPath, Pointer)>,
    /// Whether the portal no longer wants it, and has been told so.
    withdrawn: bool,
}

/// What the picker says to do, at the top of each screen, when the screen
/// to share is picked.
const SHARE_HINT: &str = "Click the screen you want to share, or press Esc to cancel.";

/// How often what is being picked is looked at, while it is.
const PICKING: Duration = Duration::from_millis(50);

/// A picture of each screen, to pick a colour from. None, if they could
/// not be taken.
fn stills(desk: &Desk) -> Vec<Still> {
    let shots = omorfos_screenshot::shoot(desk.connection(), None).ok();
    let shots = shots.into_iter().flat_map(|(shots, _)| shots);
    let stills = shots.map(|shot| Still {
        screen: shot.screen,
        size: (shot.width, shot.height),
        pixels: shot.pixels,
    });
    stills.collect()
}

/// Says in the session's log what became of something asked, so that
/// what went wrong can be found out. If even that cannot be said, there is
/// nobody to say it to.
fn note(said: &str) {
    let _ = writeln!(io::stderr(), "xdg-desktop-portal-omorfos: {said}");
}

/// What an application is called in the log.
fn called(app: &str) -> &str {
    if app.is_empty() { "a program" } else { app }
}

/// A question that waits on the user: the call to answer, the request it
/// may be withdrawn by, and what it is for.
struct Pending {
    call: Message,
    request: OwnedObjectPath,
    wants: Wants,
}

impl Pending {
    const fn new(call: Message, request: OwnedObjectPath, wants: Wants) -> Self {
        Self {
            call,
            request,
            wants,
        }
    }
}

/// What an application waits on the user for.
enum Wants {
    /// A picture of the screen.
    Picture,
    /// The screen shared, in a session, with the pointer shown so.
    Share(OwnedObjectPath, Pointer),
    /// A colour from the screen.
    Pick,
}

/// The shortest time between two pictures of a screen being shared: a
/// sixtieth of a second, which is as smooth as a call needs, and spares the
/// compositor the work of more on a faster screen.
const BETWEEN: Duration = Duration::from_nanos(1_000_000_000 / 60);

/// A screen being shared, for a session the portal made.
struct Sharing {
    session: OwnedObjectPath,
    feed: Feed,
    out: Share,
    /// The call to start sharing, until the application can be told what
    /// to read, which is once PipeWire has said.
    call: Option<Message>,
    /// The buffer the compositor is putting a picture in, if it is, and
    /// how large the pictures were when it was asked for.
    held: Option<(Held, (u32, u32))>,
    /// When the next picture may be asked for.
    next: Instant,
    /// What what reads the stream was last told of the pointer, if it is
    /// sent beside the pictures, and whether there is more to tell it.
    told: Option<Told>,
    pointing: bool,
}

/// What what reads a stream was last told of the pointer: where it is, if
/// over the screen; where its position is in its picture; and which
/// picture of it that was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Told {
    at: Option<(i32, i32)>,
    tip: (i32, i32),
    taken: u64,
}

/// What to tell what reads a stream of the pointer as it is seen: where it
/// is, and its picture, or that it is not to be seen while it is not over
/// the screen. The picture goes with every buffer, so that what reads the
/// stream is never left without it for having missed one, as it can as it
/// begins.
fn telling(seen: Seen<'_>) -> (omorfos_pipewire::Pointer<'_>, Told) {
    let told = Told {
        at: seen.at,
        tip: seen.tip,
        taken: seen.taken,
    };
    let look = if seen.at.is_some() {
        Picture::Shown(seen.picture)
    } else {
        Picture::Hidden
    };
    let pointer = omorfos_pipewire::Pointer {
        at: seen.at.unwrap_or_default(),
        tip: seen.tip,
        size: seen.size,
        picture: look,
    };
    (pointer, told)
}

impl Sharing {
    /// Tells what reads the stream of the pointer by itself, if it has
    /// moved or looks different, when it is time and a buffer is free:
    /// there was no picture to tell it with.
    fn tell(&mut self, now: Instant) {
        let pointer = self.feed.pointer().map(telling);
        let moved = pointer.filter(|(_, told)| Some(*told) != self.told);
        let moved = moved.filter(|_| self.out.reading());
        self.pointing = moved.is_some();
        let free = moved.filter(|_| self.next <= now);
        let free = free.and_then(|pointer| self.out.take().map(|held| (held, pointer)));
        if let Some((held, (pointer, told))) = free {
            self.out.give(held, None, Some(&pointer));
            (self.told, self.pointing) = (Some(told), false);
            self.next = now + BETWEEN;
        }
    }
}

impl Drop for Sharing {
    fn drop(&mut self) {
        let (taken, sent, alone) = self.out.counted();
        note(&format!(
            "while it was shared, {taken} buffers were taken, and {sent} pictures and {alone} of \
             the pointer alone sent"
        ));
    }
}

/// Everything that waits on the user, or on the screen, or on PipeWire.
struct Backend<'a> {
    portal: &'a Portal,
    desk: Desk,
    /// Where pictures are saved.
    folder: &'a Path,
    /// Where PipeWire is, if not where the environment says.
    remote: Option<String>,
    /// Where programs install what they share, desktop entries among it.
    entries: Vec<PathBuf>,
    /// The compositor's socket for scripts, which says where windows are.
    socket: Option<PathBuf>,
    pending: Option<Pending>,
    /// What is being picked, once the user said yes to a picture.
    picking: Option<Picking>,
    pipes: Option<Pipes>,
    shares: Vec<Sharing>,
}

impl Backend<'_> {
    /// Does what the user said to a question.
    fn answered(&mut self, answer: Answer, pending: Pending) {
        // The question is taken off the screen at once, and before a
        // picture is taken, so that it is not in it.
        let _ = self.desk.turn();
        let Pending {
            call,
            request,
            wants,
        } = pending;
        match (answer, wants) {
            (Answer::Give, Wants::Picture) => {
                // Then they pick what of the screens it is, over pictures
                // of them taken now, on a thread of its own, so that what
                // else there is to do goes on meanwhile.
                let shots = omorfos_screenshot::shoot(self.desk.connection(), None).ok();
                if let Some((shots, _)) = shots {
                    let (told, done) = channel();
                    let (connection, theme) = (self.desk.connection().clone(), self.desk.theme());
                    let socket = self.socket.clone();
                    std::thread::spawn(move || {
                        // Where the windows are is asked as the pictures
                        // were taken, and not on the way that answers the
                        // bus, so that it waits on the compositor alone.
                        let placed = omorfos_screenshot::placed(socket.as_deref());
                        let picked = omorfos_screenshot::pick(&connection, shots, &placed, &theme);
                        let _ = told.send(Chose::Picture(picked));
                    });
                    note("the user said yes, and picks what the picture is of");
                    let picking = Picking {
                        call,
                        request,
                        done,
                        share: None,
                        withdrawn: false,
                    };
                    self.picking = Some(picking);
                } else {
                    note("the user said yes, and the picture could not be taken");
                    self.portal.respond(&call, Response::Failed, None);
                }
            }
            (Answer::Give, Wants::Share(session, pointer)) => {
                // With more than one screen, the user picks which.
                if self.desk.screens() > 1 {
                    self.pick_screen(call, request, (session, pointer));
                } else {
                    self.begin(&call, session, pointer, None);
                }
            }
            // A colour is only picked, and anything else picks none.
            (answer, Wants::Pick) => {
                if let Answer::Picked(colour) = answer {
                    note("a colour was picked");
                    self.portal
                        .respond_colour(&call, Response::Given, Some(colour));
                } else {
                    note("no colour was picked");
                    self.portal.respond_colour(&call, Response::Refused, None);
                }
            }
            // A question is only answered yes or no.
            (_, Wants::Picture | Wants::Share(..)) => {
                note("the user said no");
                self.portal.respond(&call, Response::Refused, None);
            }
        }
    }

    /// Has the user pick which screen to share in `session`, with the
    /// pointer shown as `pointer` says, over pictures of them taken now,
    /// on a thread of its own as a picture is picked.
    fn pick_screen(
        &mut self,
        call: Message,
        request: OwnedObjectPath,
        share: (OwnedObjectPath, Pointer),
    ) {
        let Ok((shots, _)) = omorfos_screenshot::shoot(self.desk.connection(), None) else {
            note("the user said yes, and the screens could not be shown to pick from");
            self.portal.respond(&call, Response::Failed, None);
            return;
        };
        let names: Vec<String> = shots.iter().map(|shot| shot.screen.clone()).collect();
        let stills = shots.into_iter().map(|shot| omorfos_picker::Still {
            screen: shot.screen,
            size: (shot.width, shot.height),
            pixels: shot.pixels,
            windows: Vec::new(),
        });
        let stills = stills.collect();
        let (told, done) = channel();
        let (connection, theme) = (self.desk.connection().clone(), self.desk.theme());
        std::thread::spawn(move || {
            let asking = (omorfos_picker::Pickable::Screen, SHARE_HINT);
            let picked = omorfos_picker::pick(&connection, stills, asking, &theme);
            // What cannot be shown to pick from is taken for a no, as the
            // question it follows could be shown.
            let screen = picked
                .ok()
                .flatten()
                .and_then(omorfos_picker::Picked::screen);
            let screen = screen.and_then(|screen| names.get(screen).cloned());
            let _ = told.send(Chose::Screen(screen));
        });
        note("the user said yes, and picks which screen to share");
        self.picking = Some(Picking {
            call,
            request,
            done,
            share: Some(share),
            withdrawn: false,
        });
    }

    /// Begins sharing `screen`, or the first, in `session`, with the
    /// pointer shown as `pointer` says, and answers `call` once the stream
    /// can be read; or answers it that it cannot be.
    fn begin(
        &mut self,
        call: &Message,
        session: OwnedObjectPath,
        pointer: Pointer,
        screen: Option<&str>,
    ) {
        match self.share(screen, pointer) {
            Ok((feed, out)) => {
                note("the screen is being shared");
                let sharing = Sharing {
                    session,
                    feed,
                    out,
                    call: Some(call.clone()),
                    held: None,
                    next: Instant::now(),
                    told: None,
                    pointing: false,
                };
                self.shares.push(sharing);
            }
            Err(failure) => {
                note(&format!(
                    "the user said yes, and the screen cannot be shared: {failure}"
                ));
                self.portal.respond(call, Response::Failed, None);
            }
        }
    }

    /// Answers what was being picked, once it has been: with a picture of
    /// it saved among the user's pictures, or by sharing the screen picked;
    /// and not at all if the portal no longer wants it, which it has been
    /// told.
    fn picked(&mut self) {
        let done = self
            .picking
            .as_ref()
            .and_then(|picking| picking.done.try_recv().ok());
        let picking = self.picking.take_if(|_| done.is_some());
        let Some((picking, done)) = picking.zip(done) else {
            return;
        };
        let wanted = !picking.withdrawn;
        let done = match done {
            Chose::Picture(done) => done,
            Chose::Screen(screen) => {
                let share = picking.share.filter(|_| wanted);
                let Some((session, pointer)) = share else {
                    return;
                };
                if let Some(screen) = screen {
                    self.begin(&picking.call, session, pointer, Some(&screen));
                } else {
                    note("no screen was picked to share");
                    self.portal.respond(&picking.call, Response::Refused, None);
                }
                return;
            }
        };
        // What is no longer wanted is not kept: no picture is saved that
        // nobody asked for.
        if !wanted {
            note("what was picked is not kept, as it is no longer wanted");
            return;
        }
        let failed = done.as_ref().err().map(ToString::to_string);
        let saved = done
            .ok()
            .map(|picked| picked.map(|shots| kept(&shots, self.folder)));
        let response = match &saved {
            Some(Some(Some(_))) => Response::Given,
            None | Some(Some(None)) => Response::Failed,
            Some(None) => Response::Refused,
        };
        let _ = failed.inspect(|failed| note(&format!("nothing could be picked: {failed}")));
        let saved = saved.flatten().flatten();
        match &saved {
            Some(saved) => note(&format!("the picture was taken and saved at {saved}")),
            None => note("no picture was given"),
        }
        self.portal.respond(&picking.call, response, saved);
    }

    /// Begins sharing `screen`, or the first: pictures of it, and the
    /// stream they go into.
    fn share(&mut self, screen: Option<&str>, pointer: Pointer) -> Result<(Feed, Share), Failure> {
        let pipes = self
            .pipes
            .take()
            .map_or_else(|| Pipes::connect(self.remote.as_deref()), Ok);
        pipes.and_then(|pipes| {
            let begun = Feed::new(self.desk.connection(), screen, pointer);
            let begun = begun.and_then(|feed| {
                let pointer = feed.pointer().map(|seen| seen.size);
                // Memory of the card the compositor draws with, if it can
                // be had, in the layouts both can use.
                let (number, offered) = feed.card();
                let card = Card::open(number);
                let layouts = card.layouts(feed.size(), &offered);
                pipes
                    .share(feed.size(), pointer, (card, layouts))
                    .map(|out| (feed, out))
            });
            self.pipes = Some(pipes);
            begun
        })
    }

    /// Takes in what has been asked.
    fn heard(&mut self, asked: Asked) {
        // A picture being picked is asked about still.
        let pending = self.pending.is_some() || self.picking.is_some();
        match asked {
            // One question at a time: another while one is up fails.
            Asked::Screenshot { call, app, .. }
            | Asked::Share { call, app, .. }
            | Asked::Pick { call, app, .. }
                if pending =>
            {
                let app = called(&app);
                note(&format!(
                    "{app} asked something while another was being asked about"
                ));
                self.portal.respond(&call, Response::Failed, None);
            }
            Asked::Screenshot { call, request, app } => {
                note(&format!(
                    "{} asked for a picture of the screen",
                    called(&app)
                ));
                let pending = Pending::new(call, request, Wants::Picture);
                self.ask(&app, Question::Picture, pending);
            }
            Asked::Pick { call, request, app } => {
                note(&format!(
                    "{} asked for a colour on the screen",
                    called(&app)
                ));
                // The picture is taken before anything is put up, so that
                // nothing of the portal's is in it.
                let stills = stills(&self.desk);
                if !stills.is_empty() && self.desk.pick(stills) {
                    self.pending = Some(Pending::new(call, request, Wants::Pick));
                } else {
                    note("no picture of the screen could be taken, or shown to pick from");
                    self.portal.respond_colour(&call, Response::Failed, None);
                }
            }
            Asked::Share {
                call,
                request,
                session,
                app,
                pointer,
            } => {
                let how = match pointer {
                    Pointer::Hidden => "without the pointer",
                    Pointer::Drawn => "with the pointer drawn in",
                    Pointer::Beside => "with the pointer sent beside the pictures",
                };
                note(&format!(
                    "{} asked to share the screen, {how}",
                    called(&app)
                ));
                let pending = Pending::new(call, request, Wants::Share(session, pointer));
                self.ask(&app, Question::Share, pending);
            }
            Asked::Withdrawn(request) => {
                let withdrawn = self.pending.take_if(|pending| pending.request == request);
                if let Some(pending) = withdrawn {
                    note("the portal no longer wants what it asked for");
                    self.desk.dismiss();
                    self.portal.respond(&pending.call, Response::Refused, None);
                }
                // What is being picked is up until the user is done with it;
                // the application is told now that it has nothing.
                let picking = self.picking.iter_mut();
                let picking =
                    picking.filter(|picking| picking.request == request && !picking.withdrawn);
                for picking in picking {
                    note("the portal no longer wants what is being picked");
                    picking.withdrawn = true;
                    self.portal.respond(&picking.call, Response::Refused, None);
                }
            }
            Asked::Ended(session) => {
                let ended = self
                    .shares
                    .extract_if(.., |sharing| sharing.session == session);
                if ended.count() > 0 {
                    note("the screen is no longer shared");
                }
                // A session that ends while its screen is picked wants it no
                // more; the picker stays until the user is done with it.
                let picking = self.picking.iter_mut().filter(|picking| {
                    let of = picking.share.as_ref().map(|(of, _)| of);
                    of == Some(&session) && !picking.withdrawn
                });
                for picking in picking {
                    note("the session ended while its screen was being picked");
                    picking.withdrawn = true;
                    self.portal.respond(&picking.call, Response::Refused, None);
                }
            }
        }
    }

    /// Puts a question to the user, if they can be asked, naming the
    /// application as its desktop entry does, or failing that by what the
    /// sandbox calls it.
    fn ask(&mut self, app: &str, question: Question, pending: Pending) {
        let shown = omorfos_icons::named(app, &self.entries);
        if self.desk.ask(shown.as_deref().unwrap_or(app), question) {
            self.pending = Some(pending);
        } else {
            note("the user cannot be asked: the compositor does not offer the means");
            self.portal.respond(&pending.call, Response::Failed, None);
        }
    }

    /// Keeps what is shared going: tells an application what to read once
    /// it can be, has the compositor put each new picture in its stream,
    /// and returns how long until the next may be asked for, if that is to
    /// be waited for. One whose screen has gone is ended, and the
    /// application told.
    fn keep(&mut self) -> Option<Duration> {
        if let Some(pipes) = &self.pipes {
            pipes.turn();
        }
        let portal = self.portal;
        let now = Instant::now();
        self.shares.retain_mut(|sharing| {
            let known = sharing.out.node().zip(sharing.call.as_ref());
            if let Some((node, call)) = known {
                let (width, height) = sharing.feed.size();
                let size = (width.cast_signed(), height.cast_signed());
                portal.respond_shared(call, node, (0, 0), size);
                sharing.call = None;
            }
            let turned = sharing.feed.turn();
            // A screen that changes size goes on being shared at the new
            // size. A picture asked for at the old the compositor cannot
            // take, and its buffer goes back empty, as any such does.
            let resized = sharing.feed.resized();
            let _ = resized.inspect(|size| sharing.out.resize(*size));
            if let Err(failure) = &turned {
                note(&format!("the screen is no longer shared: {failure}"));
                portal.close(&sharing.session);
            }
            // A picture being put in memory PipeWire has since taken away
            // is not waited for.
            drop(sharing.held.take_if(|(held, _)| held.gone()));
            // A picture that could not be taken, as while the session is
            // locked, gives its buffer back to be filled again later.
            let over = turned.as_ref().ok().copied().flatten();
            let given = over.and_then(|filled| sharing.held.take().map(|held| (held, filled)));
            if let Some(((held, asked), filled)) = given {
                let pointer = sharing.feed.pointer().map(telling);
                let picture = filled.then_some(asked);
                sharing
                    .out
                    .give(held, picture, pointer.as_ref().map(|(pointer, _)| pointer));
                sharing.told = pointer.map(|(_, told)| told);
                sharing.next = now + BETWEEN;
            }
            sharing.tell(now);
            // The next picture is asked for when something reads them, the
            // one before has been put in its buffer, it is time, and
            // PipeWire has a buffer free: one is handed back when what reads
            // the stream has finished with it.
            let due = sharing.out.reading() && sharing.held.is_none() && sharing.next <= now;
            let free = due.then(|| sharing.out.take()).flatten();
            let free = free.and_then(|held| held.memory().map(|memory| (held, memory)));
            if let Some((held, memory)) = free {
                sharing.feed.ask(&memory, sharing.out.layout());
                sharing.held = Some((held, sharing.feed.size()));
            }
            turned.is_ok()
        });
        let waiting = self.shares.iter().filter(|sharing| {
            let wants = sharing.held.is_none() || sharing.pointing;
            sharing.out.reading() && wants && sharing.next > now
        });
        waiting.map(|sharing| sharing.next - now).min()
    }
}

/// Answers what waits on the user, keeps what is shared going, and tells
/// applications when the look of the desktop changes, until the bus or the
/// session has gone.
fn serve(
    portal: &Portal,
    config: &Path,
    desk: Desk,
    (folder, remote, entries, socket): (&Path, Option<String>, Vec<PathBuf>, Option<PathBuf>),
) -> Failure {
    // A folder that is not there cannot be watched for what is put in it.
    let _ = fs::create_dir_all(config);
    let mut watcher = Watcher::new(config).ok();
    let mut last = look(config);
    let mut backend = Backend {
        portal,
        desk,
        folder,
        remote,
        entries,
        socket,
        pending: None,
        picking: None,
        pipes: None,
        shares: Vec::new(),
    };
    loop {
        let Ok(answer) = backend.desk.turn() else {
            return "the session has ended".into();
        };
        let answered = backend.pending.take_if(|_| answer.is_some());
        if let Some((answer, pending)) = answer.zip(answered) {
            backend.answered(answer, pending);
        }
        backend.picked();
        let wait = backend.keep();
        // What is being picked says when it is done only when it is looked at.
        let picking = backend.picking.as_ref().map(|_| PICKING);
        let wait = [wait, picking].into_iter().flatten().min();
        // What was asked of the compositor meanwhile, the next picture
        // above all, is sent before waiting for its answer.
        let _ = backend.desk.connection().flush();
        let asking = portal.asking();
        let mut sources = vec![
            PollFd::new(portal, PollFlags::IN),
            PollFd::new(&asking, PollFlags::IN),
            PollFd::new(&backend.desk, PollFlags::IN),
        ];
        let pipes = backend
            .pipes
            .iter()
            .map(|pipes| PollFd::new(pipes, PollFlags::IN));
        sources.extend(pipes);
        sources.extend(
            watcher
                .as_ref()
                .map(|watcher| PollFd::new(watcher, PollFlags::IN)),
        );
        // If the wait fails, what there is to do is looked for all the same.
        let wait = wait.and_then(|wait| Timespec::try_from(wait).ok());
        let _ = poll(&mut sources, wait.as_ref());
        let gone = sources.first().is_some_and(|bus| !bus.revents().is_empty());
        drop(sources);
        if gone {
            return "the bus has gone, or another copy has taken over".into();
        }
        // What was picked meanwhile is answered first, so that what is
        // asked next is not taken for asked while it is still picked.
        backend.picked();
        for asked in portal.asked() {
            backend.heard(asked);
        }
        // What is said to have changed is read, so that it is not said
        // again. Whether anything did is seen by looking.
        let changed = watcher.as_mut().map(Watcher::drain);
        if changed.is_some_and(|changed| changed.unwrap_or(false)) {
            backend.desk.reread();
        }
        let now = look(config);
        portal.changed(last, now);
        last = now;
    }
}

#[cfg(test)]
mod tests {
    use std::thread;

    use std::os::unix::net::UnixListener;

    use super::*;
    use crate::bus::tests::Daemon;

    /// A stand-in for a compositor, which hangs up on what connects.
    struct Compositor {
        _folder: tempfile::TempDir,
        runtime: OsString,
        listener: UnixListener,
    }

    impl Compositor {
        fn new() -> Self {
            let folder = tempfile::TempDir::new().unwrap();
            let listener = UnixListener::bind(folder.path().join("wayland-test")).unwrap();
            Self {
                runtime: folder.path().as_os_str().to_owned(),
                _folder: folder,
                listener,
            }
        }
    }

    #[test]
    fn an_application_without_a_name_is_called_a_program() {
        assert_eq!(called("org.example.App"), "org.example.App");
        assert_eq!(called(""), "a program");
    }

    #[test]
    fn an_address_is_given_for_a_file_with_what_would_be_misread_written_out() {
        let path = Path::new("/home/ada/Pictures/Screenshots/Screenshot 2026-10-14 10-00-00.png");
        assert_eq!(
            address(path),
            "file:///home/ada/Pictures/Screenshots/Screenshot%202026-10-14%2010-00-00.png"
        );
        assert_eq!(address(Path::new("/a/ü#~_.x")), "file:///a/%C3%BC%23~_.x");
    }

    #[test]
    fn it_says_why_it_cannot_begin() {
        let said = start(&|_| None).to_string();
        assert!(said.contains("not an omorfos session"), "{said}");

        // There is no compositor where it is said to be.
        let variable = |name: &str| match name {
            "HOME" | "XDG_RUNTIME_DIR" => Some(OsString::from("/nowhere")),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-none")),
            _ => None,
        };
        let said = start(&variable).to_string();
        assert!(said.starts_with("/nowhere/wayland-none: "), "{said}");

        // There is one, and no bus.
        let compositor = Compositor::new();
        let runtime = compositor.runtime.clone();
        let variable = move |name: &str| match name {
            "HOME" => Some(OsString::from("/nowhere")),
            "XDG_RUNTIME_DIR" => Some(runtime.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-test")),
            "DBUS_SESSION_BUS_ADDRESS" => Some(OsString::from("unix:path=/nowhere/bus")),
            _ => None,
        };
        let said = start(&variable).to_string();
        assert!(!said.contains("not an omorfos session"), "{said}");

        // There are both, and what seemed a compositor hangs up.
        let compositor = Compositor::new();
        let hangs_up = compositor.runtime.clone();
        let daemon = Daemon::start();
        let address = daemon.address.clone();
        let variable = move |name: &str| match name {
            "HOME" => Some(OsString::from("/nowhere")),
            "XDG_RUNTIME_DIR" => Some(hangs_up.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-test")),
            "DBUS_SESSION_BUS_ADDRESS" => Some(OsString::from(&address)),
            _ => None,
        };
        let refused = thread::spawn(move || start(&variable).to_string());
        let (stream, _) = compositor.listener.accept().unwrap();
        drop(stream);
        let said = refused.join().unwrap();
        assert!(!said.contains("not an omorfos session"), "{said}");
    }
}
