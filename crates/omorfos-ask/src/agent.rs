//! The authentication agent, as polkit speaks to one: told to ask the user
//! for a request, by its cookie, and answered once the user has; or told
//! to give up asking.
//!
//! Messages are read one at a time and answered here, as the portal's are.
//! Only polkit itself is heard: a call from any other program is refused,
//! so that no other program can put up the prompt.

use std::collections::HashMap;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use rustix::pipe::{PipeFlags, pipe_with};
use zbus::blocking::connection::Builder;
use zbus::blocking::{Connection, MessageIterator};
use zbus::message::{Message, Type};
use zbus::names::{InterfaceName, MemberName, UniqueName};
use zbus::zvariant::{ObjectPath, OwnedValue, Value};

use crate::Failure;
use crate::people::Identity;

/// polkit, on the system's bus.
pub const POLKIT: &str = "org.freedesktop.PolicyKit1";
/// Where it is asked to take an agent.
const AUTHORITY_PATH: &str = "/org/freedesktop/PolicyKit1/Authority";
/// What it is asked by.
const AUTHORITY: &str = "org.freedesktop.PolicyKit1.Authority";
/// What an agent is spoken to by.
pub const AGENT: &str = "org.freedesktop.PolicyKit1.AuthenticationAgent";
/// Where this agent is.
pub const PATH: &str = "/org/omorfos/PolicyKit1/AuthenticationAgent";
/// What polkit is told when the user gives up.
const CANCELLED: &str = "org.freedesktop.PolicyKit1.Error.Cancelled";

/// A request polkit asks the user to be asked for.
#[derive(Debug)]
pub struct Begin {
    /// The call, to be answered once the user has.
    pub call: Message,
    /// What the request is known by.
    pub cookie: String,
    /// What polkit says is asked for, in words for the user.
    pub message: String,
    /// Who may say yes.
    pub identities: Vec<Identity>,
}

/// What polkit asks that waits on the user.
#[derive(Debug)]
pub enum Asked {
    /// Ask the user.
    Begin(Begin),
    /// Give up asking for the request known by this.
    Cancel(String),
}

/// The agent, registered with polkit for the session.
#[derive(Debug)]
pub struct Agent {
    connection: Connection,
    asked: Receiver<Asked>,
    /// What becomes readable when something has been asked.
    asking: OwnedFd,
    /// What becomes readable once polkit can no longer be heard.
    gone: OwnedFd,
}

impl Agent {
    /// Registers with polkit on the bus at `address`, for the session
    /// polkit knows as `session`, saying things in `locale`.
    ///
    /// # Errors
    ///
    /// If the bus cannot be reached, or polkit does not take the agent, as
    /// when another is the session's already.
    pub fn open(address: &str, session: &str, locale: &str) -> Result<Self, Failure> {
        let connection = Builder::address(address).and_then(Builder::build);
        let connection = connection.map_err(Failure::from);
        let pipe = || pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK);
        let opened = connection.and_then(|connection| {
            let pipes = pipe().and_then(|one| pipe().map(|other| (one, other)));
            pipes
                .map(|pipes| (connection, pipes))
                .map_err(Failure::from)
        });
        let opened = opened.and_then(|(connection, pipes)| {
            // Listening before registering, so that nothing asked is
            // missed; and for polkit going, after which it no longer knows
            // this agent.
            let messages = MessageIterator::from(&connection);
            let owner = register(&connection, session, locale).map_err(Failure::from);
            owner.map(|owner| (connection, pipes, messages, owner))
        });
        opened.map(
            |(connection, ((gone, going), (asking, ask)), messages, owner)| {
                let answering = connection.clone();
                let (sender, asked) = mpsc::channel();
                thread::spawn(move || {
                    let messages = messages.map_while(Result::ok);
                    for message in messages.take_while(|message| !lost(message)) {
                        let heard = heard(&answering, &message, &owner);
                        if let Some(heard) = heard {
                            let _ = sender.send(heard);
                            let _ = rustix::io::write(&ask, &[1]);
                        }
                    }
                    let _ = rustix::io::write(&going, &[1]);
                });
                Self {
                    connection,
                    asked,
                    asking,
                    gone,
                }
            },
        )
    }

    /// What has been asked since this was last called, in order.
    #[must_use]
    pub fn asked(&self) -> Vec<Asked> {
        // Emptied first, so that what is asked meanwhile wakes again.
        let mut drained = [0; 64];
        while rustix::io::read(&self.asking, &mut drained).is_ok_and(|read| read > 0) {}
        self.asked.try_iter().collect()
    }

    /// What becomes readable when something has been asked.
    #[must_use]
    pub fn asking(&self) -> BorrowedFd<'_> {
        self.asking.as_fd()
    }

    /// Tells polkit the user was asked: whether the request may now go
    /// ahead is polkit's to know, from its helper.
    pub fn done(&self, call: &Message) {
        let _ = self.connection.reply(&call.header(), &());
    }

    /// Tells polkit the user would not, or could not, be asked.
    pub fn cancelled(&self, call: &Message) {
        let header = call.header();
        let said = "The user did not authenticate";
        let _ = self.connection.reply_error(&header, CANCELLED, &said);
    }
}

impl AsFd for Agent {
    /// What becomes readable once polkit can no longer be heard: the bus
    /// has gone, or polkit has, and with it this agent's registration.
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.gone.as_fd()
    }
}

/// Listens for polkit going, learns who it is on the bus, and registers
/// with it for `session`; returns who it is, which only it may ask.
fn register(connection: &Connection, session: &str, locale: &str) -> zbus::Result<String> {
    let bus = "org.freedesktop.DBus";
    let rule = format!("type='signal',sender='{bus}',member='NameOwnerChanged',arg0='{POLKIT}'");
    let call = |member: &str, body: &(String,)| {
        connection.call_method(Some(bus), "/org/freedesktop/DBus", Some(bus), member, body)
    };
    let watched = call("AddMatch", &(rule,));
    // Who polkit is, asked before registering: should it go meanwhile, the
    // match says so.
    let owner = watched.and_then(|_| call("GetNameOwner", &(POLKIT.to_owned(),)));
    let owner = owner.and_then(|reply| reply.body().deserialize::<String>());
    let subject = (
        "unix-session",
        HashMap::from([("session-id", Value::from(session))]),
    );
    // polkit takes where the agent is as a string, not as an object's path.
    let asked = (subject, locale, PATH);
    let register = "RegisterAuthenticationAgent";
    owner.and_then(|owner| {
        let registered = connection.call_method(
            Some(POLKIT),
            AUTHORITY_PATH,
            Some(AUTHORITY),
            register,
            &asked,
        );
        registered.map(|_| owner)
    })
}

/// Whether `message` says polkit has gone, or come anew, which forgets
/// every agent.
fn lost(message: &Message) -> bool {
    let header = message.header();
    let member = header.member().map(MemberName::as_str);
    let signal = message.message_type() == Type::Signal && member == Some("NameOwnerChanged");
    signal
        && message
            .body()
            .deserialize::<(String, String, String)>()
            .is_ok_and(|(name, _, _)| name == POLKIT)
}

/// What a message asks of the agent, answering at once what does not wait
/// on the user: anything from another program than polkit, whose `owner`
/// it is not, or that is not something an agent is asked.
fn heard(connection: &Connection, message: &Message, owner: &str) -> Option<Asked> {
    let header = message.header();
    if message.message_type() != Type::MethodCall {
        return None;
    }
    let sender = header.sender().map(UniqueName::as_str);
    let interface = header.interface().map(InterfaceName::as_str);
    let member = header.member().map(MemberName::as_str);
    let path = header.path().map(ObjectPath::as_str);
    let refuse = |name: &str, why: &str| {
        let _ = connection.reply_error(&header, name, &why);
        None
    };
    if sender != Some(owner) {
        return refuse(
            "org.freedesktop.DBus.Error.AccessDenied",
            "only polkit may ask",
        );
    }
    if path != Some(PATH) || interface != Some(AGENT) {
        return refuse("org.freedesktop.DBus.Error.UnknownMethod", "not an agent's");
    }
    match member {
        Some("BeginAuthentication") => {
            type Begun = (
                String,
                String,
                String,
                HashMap<String, String>,
                String,
                Vec<(String, HashMap<String, OwnedValue>)>,
            );
            let Ok((_, said, _, _, cookie, identities)) = message.body().deserialize::<Begun>()
            else {
                return refuse(
                    "org.freedesktop.DBus.Error.InvalidArgs",
                    "not as polkit asks",
                );
            };
            Some(Asked::Begin(Begin {
                call: message.clone(),
                cookie,
                message: said,
                identities: identities.into_iter().filter_map(identity).collect(),
            }))
        }
        Some("CancelAuthentication") => {
            let cookie = message.body().deserialize::<(String,)>();
            let _ = connection.reply(&header, &());
            cookie.ok().map(|(cookie,)| Asked::Cancel(cookie))
        }
        _ => refuse("org.freedesktop.DBus.Error.UnknownMethod", "not an agent's"),
    }
}

/// Who an identity polkit sends names, if it is a user or a group.
fn identity((kind, details): (String, HashMap<String, OwnedValue>)) -> Option<Identity> {
    let number = |key: &str| details.get(key).and_then(|value| u32::try_from(value).ok());
    match kind.as_str() {
        "unix-user" => number("uid").map(Identity::User),
        "unix-group" => number("gid").map(Identity::Group),
        _ => None,
    }
}
