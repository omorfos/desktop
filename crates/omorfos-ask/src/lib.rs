//! What asks the user for a secret: for now, polkit's agent for the
//! session, which asks for the administrator's password when a program
//! wants to do what needs it, such as installing software.
//!
//! polkit asks the agent to ask; the agent chooses whose password to ask
//! for, the user's own if they may say yes, and runs polkit's helper,
//! which checks it. The question is put to the user in the sealed prompt:
//! the compositor dims the session behind it and gives it the keyboard
//! alone, which it does for this program only, as it was started with the
//! `ask` role. See `docs/adr/0048-asking-for-a-secret.md`.

mod agent;
pub mod assuan;
mod door;
mod helper;
mod people;
pub mod pinentry;
mod prompt;

use std::collections::VecDeque;
use std::error::Error;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Instant;

pub use agent::{AGENT, Agent, Asked, Begin, PATH, POLKIT};
pub use door::NAME as DOOR;
use door::{Door, Passing};
pub use helper::Helper;
use omorfos_config::Store;
pub use omorfos_shell::said::{Said, heard};
pub use omorfos_shell::shake::{SHAKE, SHAKING, shaken};
pub use people::{Identity, Person, chosen, named, users};
use pinentry::{Step, Wanted};
pub use prompt::{
    Answer, Asking, Button, CANCEL, Desk, EYE, FIELD, GO, SIZE, SURFACE, button_at, eye_says,
    label, paint, shown,
};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use rustix::pipe::{PipeFlags, pipe_with};
use zbus::message::Message;

/// Why the program cannot go on, in words for the user.
pub type Failure = Box<dyn Error + Send + Sync>;

/// Where polkit's helper is installed, by the distributions that put it
/// in different places.
const HELPERS: [&str; 3] = [
    "/usr/lib/polkit-1/polkit-agent-helper-1",
    "/usr/libexec/polkit-agent-helper-1",
    "/usr/lib/policykit-1/polkit-agent-helper-1",
];

/// Where the system bus is when the environment does not say.
const SYSTEM_BUS: &str = "unix:path=/run/dbus/system_bus_socket";

/// What is said when what was typed was not right.
pub const WRONG: &str = "That did not work. Try again.";

/// Where what is spoken to is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The system's bus, which polkit is on.
    pub system: String,
    /// The session polkit knows this one as.
    pub session: Option<String>,
    /// The language things are to be said in.
    pub locale: String,
    /// polkit's helper, which checks a secret.
    pub helper: PathBuf,
    /// The system's list of users.
    pub users: PathBuf,
    /// The system's list of groups.
    pub groups: PathBuf,
    /// The number of the user at the keyboard.
    pub me: u32,
    /// Where the configuration is, for the look of the prompt.
    pub config: Option<PathBuf>,
    /// Where the compositor is, unless it handed over a connection.
    pub wayland: Option<PathBuf>,
    /// Where `omorfos-pinentry` asks for a passphrase to be asked, in the
    /// runtime directory.
    pub door: Option<PathBuf>,
    /// The pinentry that may ask there: the one installed beside this.
    pub pinentry: PathBuf,
}

impl Places {
    /// Where they are, as the environment says: looked up by `variable`,
    /// which is passed in so that this can be tested without touching the
    /// environment. The helper is wherever the system installed it.
    #[must_use]
    pub fn from(variable: &dyn Fn(&str) -> Option<OsString>) -> Self {
        let text = |name| {
            variable(name)
                .map(OsString::into_string)
                .and_then(Result::ok)
        };
        let locale = text("LC_ALL").or_else(|| text("LANG"));
        let helper = HELPERS
            .iter()
            .map(PathBuf::from)
            .find(|helper| helper.exists());
        let (home, config) = (variable("HOME"), variable("XDG_CONFIG_HOME"));
        let (runtime, display) = (variable("XDG_RUNTIME_DIR"), variable("WAYLAND_DISPLAY"));
        Self {
            system: text("DBUS_SYSTEM_BUS_ADDRESS").unwrap_or_else(|| SYSTEM_BUS.to_owned()),
            session: text("XDG_SESSION_ID"),
            locale: locale.unwrap_or_else(|| "C".to_owned()),
            helper: helper.unwrap_or(PathBuf::from(HELPERS[0])),
            users: PathBuf::from("/etc/passwd"),
            groups: PathBuf::from("/etc/group"),
            me: rustix::process::getuid().as_raw(),
            config: omorfos_config::config_dir(config.as_deref(), home.as_deref()),
            door: runtime
                .as_ref()
                .map(|runtime| Path::new(runtime).join(door::NAME)),
            wayland: runtime.zip(display).map(socket),
            pinentry: beside("omorfos-pinentry"),
        }
    }
}

/// Where the program called `name` is installed beside this one.
fn beside(name: &str) -> PathBuf {
    let own = std::env::current_exe().ok();
    let folder = own
        .as_deref()
        .and_then(Path::parent)
        .unwrap_or(Path::new("/usr/bin"));
    folder.join(name)
}

/// Where the compositor's socket is: the display's, in the runtime
/// directory.
fn socket((runtime, display): (OsString, OsString)) -> PathBuf {
    Path::new(&runtime).join(display)
}

/// Asks for the session the environment describes, until it cannot go on,
/// and returns why.
#[must_use]
pub fn start(variable: &dyn Fn(&str) -> Option<OsString>) -> Failure {
    let places = Places::from(variable);
    let Some(wayland) = places.wayland.clone() else {
        return "this is not an omorfos session: XDG_RUNTIME_DIR or WAYLAND_DISPLAY is not set"
            .into();
    };
    match omorfos_shell::connect(&wayland) {
        Ok(stream) => run(&places, stream),
        Err(error) => format!("{}: {error}", wayland.display()).into(),
    }
}

/// Asks over the connection to the compositor `stream`, with what is
/// found where `places` says, until it cannot go on, and returns why.
#[must_use]
pub fn run(places: &Places, stream: UnixStream) -> Failure {
    let Some(session) = places.session.as_deref() else {
        return "polkit is told which session this is by XDG_SESSION_ID, which is not set".into();
    };
    let config = places.config.clone().unwrap_or_default();
    let desk = Desk::new(stream, Store::open(&config));
    let agent = desk.and_then(|desk| {
        let agent = Agent::open(&places.system, session, &places.locale);
        agent.map(|agent| (agent, desk))
    });
    let waking = pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK).map_err(Failure::from);
    let ready = agent.and_then(|ready| waking.map(|waking| (ready, waking)));
    match ready {
        Ok(((agent, desk), waking)) => {
            // Without the door, polkit is still asked for; GnuPG is not.
            let door = places.door.as_deref().and_then(|path| {
                let opened = Door::open(path, places.pinentry.clone());
                opened
                    .inspect_err(|error| note(&format!("{}: {error}", path.display())))
                    .ok()
            });
            serve(places, (&agent, door.as_ref()), desk, waking)
        }
        Err(failure) => failure,
    }
}

/// Says in the session's log what became of something asked.
fn note(said: &str) {
    // If even that cannot be said, there is nobody to say it to.
    let _ = writeln!(std::io::stderr(), "omorfos-ask: {said}");
}

/// What waits to be asked: polkit's request, or a passphrase GnuPG asks
/// for.
enum Waiting {
    Polkit(Begin),
    Key(Passing),
}

/// What is being asked.
enum Current {
    Polkit(Request),
    Key(Passing),
}

/// What wakes the loop when the helper has said something.
type Wake = std::sync::Arc<OwnedFd>;

/// A request being asked.
struct Request {
    call: Message,
    cookie: String,
    /// Whose secret is asked for, by name.
    user: String,
    asking: Asking,
    /// The helper checking this try, and what it says. A helper given up
    /// on has nobody left to tell.
    helper: Option<Helper>,
    said: Receiver<Said>,
}

/// Asks what polkit asks, and what GnuPG asks through `door`, one request
/// after another, until the compositor or polkit can no longer be heard.
fn serve(
    places: &Places,
    (agent, door): (&Agent, Option<&Door>),
    mut desk: Desk,
    (woken, wake): (OwnedFd, OwnedFd),
) -> Failure {
    let wake = std::sync::Arc::new(wake);
    let mut waiting: VecDeque<Waiting> = VecDeque::new();
    let mut asked: Option<Current> = None;
    loop {
        let heard = drain(polkit(asked.as_ref()), &woken);
        if let Some(Current::Polkit(request)) =
            asked.take_if(|current| matches!(current, Current::Polkit(_)))
        {
            let mut told = |request, said| told(places, &mut desk, agent, request, said, &wake);
            asked = heard
                .into_iter()
                .try_fold(request, &mut told)
                .map(Current::Polkit);
        }
        // What is given up on goes first, the request being asked among it.
        for heard in agent.asked() {
            match heard {
                Asked::Begin(begin) => waiting.push_back(Waiting::Polkit(begin)),
                Asked::Cancel(cookie) => {
                    asked = withdrawn(&mut desk, agent, asked, &mut waiting, &cookie);
                }
            }
        }
        waiting.extend(
            door.iter()
                .flat_map(|door| door.knocked())
                .map(Waiting::Key),
        );
        // A pinentry gpg-agent has given up on has gone, and is asked for no
        // longer.
        waiting.retain(still_wanted);
        if let Some(Current::Key(passing)) = &asked
            && passing.gone()
        {
            desk.dismiss();
            asked = None;
        }
        // One at a time: the next once this is done.
        while asked.is_none()
            && let Some(next) = waiting.pop_front()
        {
            asked = match next {
                Waiting::Polkit(begin) => {
                    begun(places, &mut desk, agent, begin, &wake).map(Current::Polkit)
                }
                Waiting::Key(passing) => {
                    desk.ask(of_key(&passing.wanted), passing.wanted.again);
                    Some(Current::Key(passing))
                }
            };
        }
        // What changed is shown, and what the user did taken in; and what
        // that changes is shown in turn, before anything is waited for.
        let answer = match desk.turn(Instant::now()) {
            Ok(answer) => answer,
            // As it is, so that the session ending is known for what it is.
            Err(failure) => return failure,
        };
        if let Some(answer) = answer {
            asked = asked.and_then(|current| match current {
                Current::Polkit(request) => {
                    answered(&mut desk, agent, request, answer).map(Current::Polkit)
                }
                Current::Key(passing) => {
                    let typed = match answer {
                        Answer::Typed(typed) => Some(typed),
                        Answer::Cancel => None,
                    };
                    passing.give(typed);
                    desk.dismiss();
                    None
                }
            });
            continue;
        }
        let asking = agent.asking();
        let mut sources = vec![
            PollFd::new(agent, PollFlags::IN),
            PollFd::new(&asking, PollFlags::IN),
            PollFd::new(&desk, PollFlags::IN),
            PollFd::new(&woken, PollFlags::IN),
        ];
        sources.extend(door.map(|door| PollFd::new(door, PollFlags::IN)));
        let pinentries = waiting.iter().filter_map(|waiting| match waiting {
            Waiting::Key(passing) => Some(passing),
            Waiting::Polkit(_) => None,
        });
        let current = match &asked {
            Some(Current::Key(passing)) => Some(passing),
            _ => None,
        };
        sources.extend(
            pinentries
                .chain(current)
                .map(|passing| PollFd::new(passing, PollFlags::IN)),
        );
        // Until something is heard, or a key held is due to be done again.
        let longest = desk
            .deadline()
            .map(|due| due.saturating_duration_since(Instant::now()));
        let longest = longest.and_then(|longest| Timespec::try_from(longest).ok());
        let _ = poll(&mut sources, longest.as_ref());
        if !sources[0].revents().is_empty() {
            return "polkit can no longer be heard".into();
        }
    }
}

/// Whether what waits is still wanted: a pinentry `gpg-agent` has given up
/// on has gone, and is asked for no longer.
fn still_wanted(waiting: &Waiting) -> bool {
    !matches!(waiting, Waiting::Key(passing) if passing.gone())
}

/// The request polkit asks that is being asked, if one is.
const fn polkit(asked: Option<&Current>) -> Option<&Request> {
    match asked {
        Some(Current::Polkit(request)) => Some(request),
        _ => None,
    }
}

/// What the prompt says when GnuPG asks for a passphrase, in words of ours
/// alone (ADR 58): which passphrase, of which key, as GnuPG lists it.
fn of_key(wanted: &Wanted) -> Asking {
    let (heading, go) = match wanted.step {
        Step::Passphrase => ("GnuPG asks for the passphrase of a key", "Unlock"),
        Step::New => ("GnuPG asks for a new passphrase for a key", "Go on"),
        Step::NewAgain => ("Type the new passphrase again", "Set"),
    };
    let key = wanted.key.as_ref();
    let named = key.map_or_else(|| UNNAMED.to_owned(), |key| key.name.clone());
    let fingerprint = key.map_or_else(String::new, |key| {
        format!("Fingerprint {}", key.fingerprint)
    });
    Asking {
        heading: heading.to_owned(),
        go: go.to_owned(),
        message: named,
        user: String::new(),
        question: fingerprint,
        open: false,
        said: if wanted.again {
            NOT_RIGHT.to_owned()
        } else {
            String::new()
        },
        checking: false,
    }
}

/// What a key GnuPG does not name is called.
pub const UNNAMED: &str = "A key GnuPG does not name";

/// What is said when the passphrase GnuPG was given was not right.
pub const NOT_RIGHT: &str = "That passphrase was not right.";

/// What the helper of the request being asked has said since this was
/// last asked.
fn drain(asked: Option<&Request>, woken: &OwnedFd) -> Vec<Said> {
    let mut drained = [0; 64];
    while rustix::io::read(woken, &mut drained).is_ok_and(|read| read > 0) {}
    asked
        .map(|request| request.said.try_iter().collect())
        .unwrap_or_default()
}

/// Starts checking a try of the request's secret.
fn check(places: &Places, request: &mut Request, wake: &Wake) -> bool {
    let (tell, said) = mpsc::channel();
    request.said = said;
    let wake = std::sync::Arc::clone(wake);
    let told = move |said| {
        let _ = tell.send(said);
        let _ = rustix::io::write(&*wake, &[1]);
    };
    let helper = Helper::start(&places.helper, (&request.user, &request.cookie), told);
    let started = helper.is_ok();
    if let Err(error) = &helper {
        let _ = writeln!(
            std::io::stderr(),
            "omorfos-ask: {}: {error}",
            places.helper.display()
        );
    }
    request.helper = helper.ok();
    started
}

/// Takes up a request polkit asks: whose secret to ask for, and the first
/// try of it. One that cannot be asked is said to have been given up.
fn begun(
    places: &Places,
    desk: &mut Desk,
    agent: &Agent,
    begin: Begin,
    wake: &Wake,
) -> Option<Request> {
    let Begin {
        call,
        cookie,
        message,
        identities,
    } = begin;
    let users = fs::read_to_string(&places.users).unwrap_or_default();
    let groups = fs::read_to_string(&places.groups).unwrap_or_default();
    let people = named(&identities, &people::users(&users), &groups);
    let Some(person) = chosen(&people, places.me) else {
        agent.cancelled(&call);
        return None;
    };
    let asking = Asking {
        heading: "Authentication is required".to_owned(),
        go: "Authenticate".to_owned(),
        message,
        user: person.name.clone(),
        question: String::new(),
        open: false,
        said: String::new(),
        checking: true,
    };
    let mut request = Request {
        call,
        cookie,
        user: person.name.clone(),
        asking,
        helper: None,
        said: mpsc::channel().1,
    };
    if check(places, &mut request, wake) {
        Some(request)
    } else {
        agent.cancelled(&request.call);
        desk.dismiss();
        None
    }
}

/// Takes in what the helper said of the try being checked.
fn told(
    places: &Places,
    desk: &mut Desk,
    agent: &Agent,
    mut request: Request,
    said: Said,
    wake: &Wake,
) -> Option<Request> {
    match said {
        Said::Secret(question) => {
            request.asking.open = false;
            request.asking.question = question;
            request.asking.checking = false;
        }
        Said::Open(question) => {
            request.asking.open = true;
            request.asking.question = question;
            request.asking.checking = false;
        }
        Said::Error(what) | Said::Info(what) => request.asking.said = what,
        Said::Right => {
            agent.done(&request.call);
            desk.dismiss();
            return None;
        }
        Said::Wrong | Said::Unavailable => {
            // Another try, afresh, until the user gives up; and the box
            // shakes, as a head does.
            desk.shake();
            WRONG.clone_into(&mut request.asking.said);
            request.asking.checking = true;
            if !check(places, &mut request, wake) {
                agent.cancelled(&request.call);
                desk.dismiss();
                return None;
            }
            desk.ask(request.asking.clone(), true);
            return Some(request);
        }
    }
    desk.ask(request.asking.clone(), false);
    Some(request)
}

/// Takes in what the user did.
fn answered(
    desk: &mut Desk,
    agent: &Agent,
    mut request: Request,
    answer: Answer,
) -> Option<Request> {
    match answer {
        Answer::Typed(typed) => {
            request
                .helper
                .iter_mut()
                .for_each(|helper| helper.answer(&typed));
            let mut typed = typed.into_bytes();
            typed.fill(0);
            request.asking.checking = true;
            desk.ask(request.asking.clone(), false);
            Some(request)
        }
        Answer::Cancel => {
            request.helper.take().into_iter().for_each(Helper::stop);
            agent.cancelled(&request.call);
            desk.dismiss();
            None
        }
    }
}

/// Gives up on the request known by `cookie`, as polkit says to.
fn withdrawn(
    desk: &mut Desk,
    agent: &Agent,
    asked: Option<Current>,
    waiting: &mut VecDeque<Waiting>,
    cookie: &str,
) -> Option<Current> {
    waiting.retain(|waiting| {
        let Waiting::Polkit(begin) = waiting else {
            return true;
        };
        let keep = begin.cookie != cookie;
        if !keep {
            agent.cancelled(&begin.call);
        }
        keep
    });
    match asked {
        Some(Current::Polkit(mut request)) if request.cookie == cookie => {
            request.helper.take().into_iter().for_each(Helper::stop);
            agent.cancelled(&request.call);
            desk.dismiss();
            None
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_are_where_the_environment_says() {
        let none = Places::from(&|_| None);
        assert_eq!(none.system, SYSTEM_BUS);
        assert_eq!((none.session, none.locale.as_str()), (None, "C"));
        assert_eq!(none.users, PathBuf::from("/etc/passwd"));
        assert_eq!(none.groups, PathBuf::from("/etc/group"));
        assert_eq!(none.wayland, None);
        assert!(
            HELPERS
                .iter()
                .any(|helper| none.helper == Path::new(helper))
        );
        let all = Places::from(&|name| Some(OsString::from(format!("/{name}"))));
        assert_eq!(all.system, "/DBUS_SYSTEM_BUS_ADDRESS");
        assert_eq!(all.session.as_deref(), Some("/XDG_SESSION_ID"));
        assert_eq!(all.locale, "/LC_ALL");
        assert_eq!(
            all.wayland,
            Some(PathBuf::from("/WAYLAND_DISPLAY")),
            "where it says"
        );
        let lang = Places::from(&|name| (name == "LANG").then(|| OsString::from("pl_PL.UTF-8")));
        assert_eq!(lang.locale, "pl_PL.UTF-8");
    }

    #[test]
    fn outside_a_session_or_without_its_number_nothing_is_asked() {
        let failure = start(&|_| None).to_string();
        assert!(
            failure.starts_with("this is not an omorfos session"),
            "{failure}"
        );
        let nowhere = |name: &str| match name {
            "XDG_RUNTIME_DIR" => Some(OsString::from("/nowhere")),
            "WAYLAND_DISPLAY" => Some(OsString::from("wayland-0")),
            _ => None,
        };
        assert!(
            start(&nowhere)
                .to_string()
                .starts_with("/nowhere/wayland-0: ")
        );
        // Where a compositor would be, and is reached, but it is not known
        // which session this is.
        let directory = tempfile::TempDir::new().unwrap();
        let _listening =
            std::os::unix::net::UnixListener::bind(directory.path().join("w")).unwrap();
        let runtime = directory.path().as_os_str().to_owned();
        let reached = move |name: &str| match name {
            "XDG_RUNTIME_DIR" => Some(runtime.clone()),
            "WAYLAND_DISPLAY" => Some(OsString::from("w")),
            _ => None,
        };
        assert!(start(&reached).to_string().contains("XDG_SESSION_ID"));
        let (ours, _theirs) = UnixStream::pair().unwrap();
        let places = Places::from(&|_| None);
        let failure = run(&places, ours).to_string();
        assert!(failure.contains("XDG_SESSION_ID"), "{failure}");
        // A compositor that does not answer is no session.
        let (ours, theirs) = UnixStream::pair().unwrap();
        drop(theirs);
        let places = Places {
            session: Some("7".to_owned()),
            ..places
        };
        assert_ne!(run(&places, ours).to_string(), "");
    }
}
