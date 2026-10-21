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
mod helper;
mod people;
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

pub use agent::{AGENT, Agent, Asked, Begin, PATH, POLKIT};
pub use helper::{Helper, Said, heard};
use omorfos_config::Store;
pub use people::{Identity, Person, chosen, named, users};
pub use prompt::{
    Answer, Asking, Button, CANCEL, Desk, FIELD, GO, SHOW, SIZE, button_at, label, paint, shown,
};
use rustix::event::{PollFd, PollFlags, poll};
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
            wayland: runtime.zip(display).map(socket),
        }
    }
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
        Ok(((agent, desk), waking)) => serve(places, &agent, desk, waking),
        Err(failure) => failure,
    }
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

/// Asks what polkit asks, one request after another, until the compositor
/// or polkit can no longer be heard.
fn serve(
    places: &Places,
    agent: &Agent,
    mut desk: Desk,
    (woken, wake): (OwnedFd, OwnedFd),
) -> Failure {
    let wake = std::sync::Arc::new(wake);
    let mut waiting: VecDeque<Begin> = VecDeque::new();
    let mut asked: Option<Request> = None;
    loop {
        for said in drain(asked.as_ref(), &woken) {
            asked = asked.and_then(|request| told(places, &mut desk, agent, request, said, &wake));
        }
        // What is given up on goes first, the request being asked among it.
        for heard in agent.asked() {
            match heard {
                Asked::Begin(begin) => waiting.push_back(begin),
                Asked::Cancel(cookie) => {
                    asked = withdrawn(&mut desk, agent, asked, &mut waiting, &cookie);
                }
            }
        }
        // One at a time: the next once this is done.
        while asked.is_none()
            && let Some(begin) = waiting.pop_front()
        {
            asked = begun(places, &mut desk, agent, begin, &wake);
        }
        // What changed is shown, and what the user did taken in; and what
        // that changes is shown in turn, before anything is waited for.
        let Ok(answer) = desk.turn() else {
            return "the session has ended".into();
        };
        if let Some(answer) = answer {
            asked = asked.and_then(|request| answered(&mut desk, agent, request, answer));
            continue;
        }
        let asking = agent.asking();
        let mut sources = [
            PollFd::new(agent, PollFlags::IN),
            PollFd::new(&asking, PollFlags::IN),
            PollFd::new(&desk, PollFlags::IN),
            PollFd::new(&woken, PollFlags::IN),
        ];
        let _ = poll(&mut sources, None);
        if !sources[0].revents().is_empty() {
            return "polkit can no longer be heard".into();
        }
    }
}

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
        Said::Wrong => {
            // Another try, afresh, until the user gives up.
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
    asked: Option<Request>,
    waiting: &mut VecDeque<Begin>,
    cookie: &str,
) -> Option<Request> {
    waiting.retain(|begin| {
        let keep = begin.cookie != cookie;
        if !keep {
            agent.cancelled(&begin.call);
        }
        keep
    });
    let (this, other) = asked
        .into_iter()
        .partition::<Vec<_>, _>(|request| request.cookie == cookie);
    for mut request in this {
        request.helper.take().into_iter().for_each(Helper::stop);
        agent.cancelled(&request.call);
        desk.dismiss();
    }
    other.into_iter().next()
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
