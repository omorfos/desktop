//! Keeps what is copied in the omorfos desktop after the program it was
//! copied from closes.
//!
//! What is on the clipboard is the copying program's to give, and once it
//! closes there is nothing to paste. This program reads whatever is copied,
//! in every form it is offered in, as soon as it is copied, and offers it in
//! the copying program's place, which is then free to close. The compositor
//! lets it, being started with `may: [clipboard]`; started any other way it
//! is refused and says so.
//!
//! What a password manager copies, which says so with
//! `x-kde-passwordManagerHint`, is not kept: it goes when the password
//! manager takes it away, as it means to. Nor is what is only selected,
//! pasted with the middle button. What is too large to keep, or not handed
//! over soon enough, is left with the program that copied it.
//!
//! What is kept may be forgotten after a while, as `clipboard.forget` says,
//! for a password copied from where nothing says it is secret: the
//! clipboard is emptied then, unless something else was copied meanwhile.

use std::error::Error;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::AsFd;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::{wl_registry, wl_seat};
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1::{self as device, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_offer_v1::{self as offer, ExtDataControlOfferV1},
    ext_data_control_source_v1::{self as source, ExtDataControlSourceV1},
};

/// Why the keeper could not go on.
pub type Failure = Box<dyn Error + Send + Sync>;

/// The form offered beside the others by what this program keeps, by
/// which it knows its own.
const MARK: &str = "application/x-omorfos-kept";

/// The form a password manager offers what it copies in besides, to say it
/// is not to be kept.
const SECRET: &str = "x-kde-passwordManagerHint";

/// What a program written for X11 offers that is not data but asks
/// something of it, as the conventions for its clipboard have them.
const ASKED: [&str; 5] = ["TARGETS", "MULTIPLE", "TIMESTAMP", "SAVE_TARGETS", "DELETE"];

/// How much is kept, and how long it is waited for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most that is kept of one copy, in all its forms together, in
    /// bytes. Anything larger is left with the program that copied it.
    pub most: usize,
    /// How long a program that copied has to hand over all of it.
    pub wait: Duration,
    /// How long what is kept stays on the clipboard before it is emptied,
    /// if nothing else is copied meanwhile; for ever without.
    pub forget: Option<Duration>,
}

impl Default for Limits {
    /// A picture of a large screen, in a few forms, kept for two seconds'
    /// wait.
    fn default() -> Self {
        Self {
            most: 64 << 20,
            wait: Duration::from_secs(2),
            forget: None,
        }
    }
}

/// What has been copied, in each form it is offered in.
type Kept = Vec<(String, Arc<[u8]>)>;

/// Keeps what is copied for as long as the session lasts, and returns why
/// it could not go on.
///
/// `variable` looks up an environment variable. The compositor is found
/// by `XDG_RUNTIME_DIR` and `WAYLAND_DISPLAY`, unless it handed over a
/// connection; and how long what is copied is kept, by the configuration
/// found from `XDG_CONFIG_HOME` and `HOME`, as it is when this starts.
#[must_use]
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Failure {
    let config = omorfos_config::config_dir(
        variable("XDG_CONFIG_HOME").as_deref(),
        variable("HOME").as_deref(),
    );
    let forget = config.map_or(0, |config| {
        let store = omorfos_config::Store::open(&config);
        store.current().general.clipboard.forget.get()
    });
    let forget = (forget > 0).then(|| Duration::from_secs(u64::from(forget)));
    start_with(
        variable,
        Limits {
            forget,
            ..Limits::default()
        },
    )
}

/// The same, keeping no more and waiting no longer than `limits` say.
#[must_use]
pub fn start_with(variable: &dyn Fn(&str) -> Option<OsString>, limits: Limits) -> Failure {
    let get = |name: &str| variable(name).filter(|value| !value.is_empty());
    let wayland = get("XDG_RUNTIME_DIR").zip(get("WAYLAND_DISPLAY"));
    let wayland = wayland.map(|(directory, display)| Path::new(&directory).join(display));
    let Some(socket) = wayland else {
        return "this is not an omorfos session: XDG_RUNTIME_DIR or WAYLAND_DISPLAY is not set"
            .into();
    };
    let stream = omorfos_shell::connect(&socket);
    let connection =
        stream.and_then(|stream| Connection::from_socket(stream).map_err(std::io::Error::other));
    match connection {
        Ok(connection) => keep(&connection, limits),
        Err(error) => format!("{}: {error}", socket.display()).into(),
    }
}

/// Keeps what is copied, over `connection`, until it ends.
#[must_use]
pub fn keep(connection: &Connection, limits: Limits) -> Failure {
    let ready = registry_queue_init::<State>(connection).map_err(Failure::from);
    let found = ready.and_then(|(globals, queue)| {
        let handle = queue.handle();
        let manager = globals.bind::<ExtDataControlManagerV1, _, _>(&handle, 1..=1, ());
        let seat = globals.bind::<wl_seat::WlSeat, _, _>(&handle, 1..=1, ());
        let found = manager.ok().zip(seat.ok());
        found.map(|found| (found, queue)).ok_or_else(|| {
            "the compositor does not let this program keep the clipboard: it is to be started \
             with `may: [clipboard]`"
                .into()
        })
    });
    let ((manager, seat), mut queue) = match found {
        Ok(found) => found,
        Err(failure) => return failure,
    };
    let handle = queue.handle();
    let mut state = State::default();
    let device = manager.get_data_device(&seat, &handle, ());
    // Told each time something is kept, to forget it when it is due; and
    // gone, and so ended, when this is.
    let forgetting = limits.forget.map(|after| {
        let (tell, told) = mpsc::channel();
        let (device, connection) = (device.clone(), connection.clone());
        let (kept, held) = (Arc::clone(&state.kept), Arc::clone(&state.held));
        let thread = thread::spawn(move || forget(&told, (&device, &connection), (&kept, &held)));
        Forgetting {
            tell: Some(tell),
            after,
            thread: Some(thread),
        }
    });
    loop {
        // What was copied while the last was being read is seen to first.
        let waiting = state.copied.is_none();
        if let Some(error) = waiting
            .then(|| queue.blocking_dispatch(&mut state).err())
            .flatten()
        {
            // As it is, so that the session ending is known for what it is.
            return error.into();
        }
        let Some((offer, kinds)) = state.copied.take() else {
            continue;
        };
        // What this program offers itself is kept already, and what a
        // password manager copies is not to be. What is on the clipboard as
        // it starts is taken even from a keeper: it is the one it takes the
        // place of, as a new version does, handing over what it keeps.
        let found = std::mem::take(&mut state.found);
        let ours = kinds.iter().any(|kind| kind == MARK) && !found;
        let mine = ours || kinds.iter().any(|kind| kind == SECRET);
        state.changed = false;
        let read = (!mine).then(|| read(connection, &offer, &kinds, limits));
        offer.destroy();
        // Something else copied while it was read, or the clipboard
        // emptied, as it is when the program that copied goes, leaves it
        // out of date; and nothing at all is not worth keeping.
        let outdated = queue.roundtrip(&mut state).is_err() || state.changed;
        let something = |kept: &Kept| kept.iter().any(|(_, data)| !data.is_empty());
        if let Some(kept) = read.flatten().filter(|_| !outdated).filter(something) {
            let source = manager.create_data_source(&handle, kept.clone());
            for (kind, _) in &kept {
                source.offer(kind.clone());
            }
            source.offer(MARK.to_owned());
            // Counted as it is put on the clipboard, so that what forgets
            // knows it from anything kept after it.
            let held = state.held.lock();
            let kept = state.kept.fetch_add(1, Ordering::SeqCst) + 1;
            device.set_selection(Some(&source));
            drop(held);
            if let Some(forgetting) = &forgetting {
                forgetting.kept(kept);
            }
        }
    }
}

/// What forgets what was kept when it is due: told each time something is
/// kept, and ended, and waited for, as the keeper ends.
struct Forgetting {
    tell: Option<mpsc::Sender<(u64, Instant)>>,
    /// How long after it is kept something is forgotten.
    after: Duration,
    thread: Option<thread::JoinHandle<()>>,
}

impl Forgetting {
    /// Says that what is counted `kept` has just been kept.
    fn kept(&self, kept: u64) {
        let due = (kept, Instant::now() + self.after);
        let _ = self.tell.as_ref().map(|tell| tell.send(due));
    }
}

impl Drop for Forgetting {
    fn drop(&mut self) {
        // Told no more, it ends.
        drop(self.tell.take());
        let _ = self.thread.take().map(thread::JoinHandle::join);
    }
}

/// Empties the clipboard when what was last kept is due to be forgotten,
/// as `told` says each time something is kept, unless something else has
/// been kept or copied since, which `kept` counts; until `told` is gone.
fn forget(
    told: &mpsc::Receiver<(u64, Instant)>,
    (device, connection): (&ExtDataControlDeviceV1, &Connection),
    (kept, held): (&AtomicU64, &Mutex<()>),
) {
    let mut due: Option<(u64, Instant)> = None;
    loop {
        let next = due.map_or_else(
            || told.recv().map_err(RecvTimeoutError::from),
            |(_, at)| told.recv_timeout(at.saturating_duration_since(Instant::now())),
        );
        match (next, due) {
            (Ok(next), _) => due = Some(next),
            (Err(RecvTimeoutError::Timeout), Some((which, _))) => {
                let _held = held.lock();
                if kept.load(Ordering::SeqCst) == which {
                    device.set_selection(None);
                    // A failure is seen by the keeper's own wait.
                    let _ = connection.flush();
                }
                due = None;
            }
            // The keeper has gone.
            _ => return,
        }
    }
}

/// Reads what was copied, in each of `kinds` there is data in, all at
/// once: the forms that come within `limits`, unless together they are
/// more than it allows.
fn read(
    connection: &Connection,
    offer: &ExtDataControlOfferV1,
    kinds: &[String],
    limits: Limits,
) -> Option<Kept> {
    let most = u64::try_from(limits.most).unwrap_or(u64::MAX);
    // A program written for X11 offers some things that are not data but
    // what it can be asked to do, and some forms more than once. The rest
    // are kept in the order they were offered, which says which is best.
    let new = |(at, kind): &(usize, &String)| !kinds[..*at].contains(kind);
    // Nor is the mark of a keeper data: it is offered again as this one's.
    let data = |(_, kind): &(usize, &String)| !ASKED.contains(&kind.as_str()) && *kind != MARK;
    let kinds: Vec<&String> = kinds
        .iter()
        .enumerate()
        .filter(new)
        .filter(data)
        .map(|(_, kind)| kind)
        .collect();
    let asked: Vec<_> = kinds.iter().map(|kind| ask(offer, kind, most)).collect();
    // A failure to send is seen by the next wait for events.
    let _ = connection.flush();
    let until = Instant::now() + limits.wait;
    let got = asked.iter().map(|answer| {
        let left = until.saturating_duration_since(Instant::now());
        answer.recv_timeout(left).ok().flatten()
    });
    // What does not come in time, or cannot be read, is gone without.
    let kept = kinds
        .into_iter()
        .zip(got)
        .filter_map(|(kind, data)| data.map(|data| (kind.clone(), Arc::from(data))));
    let kept: Kept = kept.collect();
    let small = kept.iter().map(|(_, data)| data.len()).sum::<usize>() <= limits.most;
    small.then_some(kept)
}

/// Asks for what was copied in the form `kind`, which arrives on what is
/// returned: all of it, or nothing if it was more than `most` bytes or
/// could not be read. A program that never hands it over holds a thread
/// until it closes.
fn ask(offer: &ExtDataControlOfferV1, kind: &str, most: u64) -> mpsc::Receiver<Option<Vec<u8>>> {
    let (sender, receiver) = mpsc::channel();
    let pipe = std::io::pipe();
    let reader = pipe.map(|(reader, writer)| {
        offer.receive(kind.to_owned(), writer.as_fd());
        reader
    });
    let _ = reader.map(|reader| {
        thread::spawn(move || {
            let mut got = Vec::new();
            let read = reader.take(most.saturating_add(1)).read_to_end(&mut got);
            let fits = |length: &usize| u64::try_from(*length).is_ok_and(|length| length <= most);
            let _ = sender.send(read.ok().filter(fits).map(|_| got));
        })
    });
    receiver
}

/// What the compositor has said.
#[derive(Debug, Default)]
struct State {
    /// What is offered and has not been said to be on the clipboard yet,
    /// with the forms it is offered in.
    offers: Vec<(ExtDataControlOfferV1, Vec<String>)>,
    /// What was last copied, with its forms, until it is read.
    copied: Option<(ExtDataControlOfferV1, Vec<String>)>,
    /// Whether the clipboard has changed since it was last read.
    changed: bool,
    /// Whether what was last copied is what was on the clipboard when the
    /// keeper started.
    found: bool,
    /// Whether the clipboard has been heard of at all yet.
    heard: bool,
    /// How many times something has been kept or let go of, so that what
    /// is forgotten is only ever what was kept last; and held while the
    /// clipboard is set, for the same.
    kept: Arc<AtomicU64>,
    held: Arc<Mutex<()>>,
}

impl State {
    /// Lets go of what is offered for being selected, which is not kept.
    fn forget(&mut self, event: &device::Event) {
        if let device::Event::PrimarySelection { id: Some(offer) } = event {
            self.offers.retain(|(waiting, _)| waiting != offer);
            offer.destroy();
        }
    }

    /// Takes what is offered as `offer` out of what is waiting.
    fn take(
        &mut self,
        offer: &ExtDataControlOfferV1,
    ) -> Option<(ExtDataControlOfferV1, Vec<String>)> {
        let at = self.offers.iter().position(|(waiting, _)| waiting == offer);
        at.map(|at| self.offers.swap_remove(at))
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // Only what there is at first is wanted, and that is bound by then.
    }
}

impl Dispatch<ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtDataControlDeviceV1,
        event: device::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            device::Event::DataOffer { id } => state.offers.push((id, Vec::new())),
            device::Event::Selection { id } => {
                state.changed = true;
                state.found = !std::mem::replace(&mut state.heard, true);
                state.copied = id.and_then(|offer| state.take(&offer));
            }
            // What is only selected is not kept, and nothing else is
            // said that matters.
            other => state.forget(&other),
        }
    }

    wayland_client::event_created_child!(State, ExtDataControlDeviceV1, [
        device::EVT_DATA_OFFER_OPCODE => (ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ExtDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        offer: &ExtDataControlOfferV1,
        event: offer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        // A form with no name cannot usefully be asked for.
        if let offer::Event::Offer { mime_type } = event
            && !mime_type.is_empty()
        {
            let waiting = state.offers.iter_mut();
            let waiting = waiting.filter(|(waiting, _)| waiting == offer);
            waiting.for_each(|(_, kinds)| kinds.push(mime_type.clone()));
        }
    }
}

impl Dispatch<ExtDataControlSourceV1, Kept> for State {
    fn event(
        state: &mut Self,
        source: &ExtDataControlSourceV1,
        event: source::Event,
        kept: &Kept,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let source::Event::Send { mime_type, fd } = event {
            // Handed over on a thread of its own, since a program that asks
            // may take its time reading, or never read at all.
            let found = kept.iter().find(|(kind, _)| *kind == mime_type);
            let data = found.map(|(_, data)| Arc::clone(data));
            let _ = data.map(|data| thread::spawn(move || drop(File::from(fd).write_all(&data))));
        } else {
            // Something else has been copied, and what this held is let go,
            // and nothing is to be forgotten of it now. It only ever keeps
            // something once another has copied, which lets go of what it
            // held before: what it lets go of is what was on the clipboard.
            state.kept.fetch_add(1, Ordering::SeqCst);
            source.destroy();
        }
    }
}

delegate_noop!(State: ExtDataControlManagerV1);
delegate_noop!(State: ignore wl_seat::WlSeat);
