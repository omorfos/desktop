//! What a program may do that an ordinary program may not.
//!
//! A privilege is given to a program the compositor started itself, for a
//! role its configuration names, by starting the program with a connection
//! that has the role. Nothing is given by a name, which a program can
//! choose. See `docs/adr/0037-who-may-do-what.md`.

use std::fs;

use omorfos_config::Role;
use smithay::delegate_security_context;
use smithay::reexports::wayland_server::Client;
use smithay::wayland::security_context::{
    SecurityContext, SecurityContextHandler, SecurityContextListenerSource,
};

use crate::compositor::{ClientState, Compositor};

/// The roles a connection has. Most have none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Roles(u8);

impl Roles {
    /// No role: an ordinary program.
    pub const NONE: Self = Self(0);
    /// Every role there is.
    pub const ALL: Self = Self(u8::MAX);
    /// Put surfaces on the screen outside windows, and list the windows.
    pub const PANEL: Self = Self(1);
    /// Put surfaces on the screen as a panel may, and with one of them
    /// take the keyboard from whatever has it.
    pub const OVERLAY: Self = Self(1 << 1);
    /// Lock the session.
    pub const LOCK: Self = Self(1 << 2);
    /// Take pictures of the screen.
    pub const CAPTURE: Self = Self(1 << 3);
    /// Read the clipboard and set it at any time, not only while one of
    /// its windows has the keyboard.
    pub const CLIPBOARD: Self = Self(1 << 4);

    /// These roles and those as well.
    #[must_use]
    pub const fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether any of `wanted` is among these.
    #[must_use]
    pub const fn has(self, wanted: Self) -> bool {
        self.0 & wanted.0 != 0
    }

    /// The roles a configuration names. Those the compositor has nothing
    /// to give for yet are named for the day it has.
    #[must_use]
    pub fn named(roles: &[Role]) -> Self {
        let one = |role: &Role| match role {
            Role::Panel => Self::PANEL,
            Role::Overlay => Self::OVERLAY,
            Role::Lock => Self::LOCK,
            Role::Capture => Self::CAPTURE,
            Role::Clipboard => Self::CLIPBOARD,
            Role::Ask | Role::Input | Role::Screens => Self::NONE,
        };
        roles.iter().map(one).fold(Self::NONE, Self::with)
    }
}

/// What has been refused, so that the user is told of each thing once.
#[derive(Debug, Default)]
pub(crate) struct Refused {
    /// What the user has been told already.
    told: Vec<String>,
    /// What is to be said when the compositor next knows what time it is.
    pending: Vec<String>,
}

impl Refused {
    /// Notes something to tell the user, once however often it happens.
    pub(crate) fn say(&mut self, said: &str) {
        if !self.told.iter().any(|told| told == said) {
            self.told.push(said.to_owned());
            self.pending.push(said.to_owned());
        }
    }

    /// What has been refused since this was last asked.
    pub(crate) fn take(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }
}

impl Compositor {
    /// The roles a client's connection has.
    pub(crate) fn roles(client: Option<&Client>) -> Roles {
        let state = client.and_then(|client| client.get_data::<ClientState>());
        state.map_or(Roles::NONE, |state| state.roles)
    }

    /// Whether a client is told what windows there are, which is for
    /// what shows them: their titles say what the user is doing.
    pub(crate) fn lists(client: &Client) -> bool {
        Self::roles(Some(client)).has(Roles::PANEL)
    }

    /// Whether a client is given the clipboard at any time. That is
    /// what is copied wherever, passwords among it, so it is not offered
    /// to a program that has not the role: offering it is giving it.
    pub(crate) fn copies(client: &Client) -> bool {
        Self::roles(Some(client)).has(Roles::CLIPBOARD)
    }

    /// Whether a client is outside any sandbox. One inside is not shown
    /// what only a role allows, since it can be given none, nor the way a
    /// sandbox is made, by which it could say it is another program.
    pub(crate) fn outside(client: &Client) -> bool {
        let state = client.get_data::<ClientState>();
        !state.is_some_and(|state| state.sandboxed)
    }

    /// Notes that a client was refused something it has no role for, so
    /// that the user is told once what asked, for what, and how it could
    /// be let.
    pub(crate) fn refuse(&mut self, client: Option<&Client>, asked: &str, role: &str) {
        // What a program calls itself is only to tell the user which it
        // was. Nothing is decided by it.
        let process = client.and_then(|client| client.get_credentials(&self.display).ok());
        let called = process
            .and_then(|process| fs::read_to_string(format!("/proc/{}/comm", process.pid)).ok());
        let name = called_by(called.as_deref());
        let said = format!(
            "`{name}` asked to {asked} and was not let; to let it, have omorfos start it with \
             `may: [{role}]`, from `autostart` in config.yaml or a key in keys.yaml"
        );
        self.refused.say(&said);
    }
}

impl SecurityContextHandler for Compositor {
    fn context_created(&mut self, source: SecurityContextListenerSource, _: SecurityContext) {
        // Whatever connects where the sandbox listens is inside it. If the
        // sandbox cannot be listened to, its programs cannot connect at
        // all, which holds them to less still.
        let accepted = self
            .event_loop
            .insert_source(source, |stream, (), compositor| {
                let _ = compositor.insert_sandboxed(stream);
            });
        let _ = accepted;
    }
}

delegate_security_context!(Compositor);

/// What to call a program, given what it calls itself if that could be
/// found out.
fn called_by(itself: Option<&str>) -> &str {
    let name = itself.map(str::trim).filter(|name| !name.is_empty());
    name.unwrap_or("a program")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_roles_a_configuration_names_are_those_there_is_something_to_give_for() {
        let named = Roles::named(&[Role::Panel, Role::Lock]);
        assert!(named.has(Roles::PANEL) && named.has(Roles::LOCK));
        assert!(!named.has(Roles::OVERLAY));
        assert_eq!(Roles::named(&[Role::Overlay]), Roles::OVERLAY);
        assert_eq!(Roles::named(&[Role::Capture]), Roles::CAPTURE);
        assert_eq!(Roles::named(&[Role::Clipboard]), Roles::CLIPBOARD);
        let later = [Role::Ask, Role::Input, Role::Screens];
        assert_eq!(Roles::named(&later), Roles::NONE, "nothing to give yet");
        assert_eq!(Roles::named(&[]), Roles::NONE);
        assert!(Roles::ALL.has(Roles::PANEL.with(Roles::OVERLAY)));
        assert!(!Roles::NONE.has(Roles::ALL));
    }

    #[test]
    fn a_program_is_called_what_it_calls_itself_if_that_is_known() {
        assert_eq!(called_by(Some("waybar\n")), "waybar");
        assert_eq!(called_by(Some("  \n")), "a program");
        assert_eq!(called_by(None), "a program");
    }
}
