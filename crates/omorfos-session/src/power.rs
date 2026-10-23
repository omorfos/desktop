//! Sleep, restart and shut down, asked of logind, which decides whether
//! this user may: on a machine of one user at its keyboard, it may. And
//! what a locked screen may offer of them, and of logging out.

use std::fmt::Write as _;
use std::path::Path;

use omorfos_ipc::{Client, Reply, Request};
use zbus::blocking::Connection;

use crate::Failure;
use crate::watch::{LOGIND, MANAGER, MANAGER_PATH};

/// The longest a program's name or reason for keeping the machine on is
/// told, in characters: any program may say anything there, and a locked
/// screen shows it.
const LONGEST_SAID: usize = 80;

/// What is asked of the machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Power {
    /// Sleep, which locks the session first.
    Sleep,
    /// Start again.
    Restart,
    /// Turn off.
    ShutDown,
}

impl Power {
    /// The word it is asked for by.
    const fn word(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Restart => "restart",
            Self::ShutDown => "shut-down",
        }
    }
}

/// Asks logind for `power`; `interactive`, where this user may only with
/// leave, for polkit to ask for it.
pub(crate) fn ask(system: &Connection, power: Power, interactive: bool) -> Result<(), Failure> {
    let method = match power {
        Power::Sleep => "Suspend",
        Power::Restart => "Reboot",
        Power::ShutDown => "PowerOff",
    };
    system.call_method(
        Some(LOGIND),
        MANAGER_PATH,
        Some(MANAGER),
        method,
        &(interactive,),
    )?;
    Ok(())
}

/// What a locked screen may offer, a line each: `can` and the word of each
/// of sleep, restart and shut down that logind says may be done without
/// asking anyone, and of `log-out` where there is a compositor to ask;
/// then `held`, what it is held from (`sleep` or `shut-down`), and who is
/// holding it and why, for each program keeping the machine from either.
/// What logind does not answer is not offered, and holds nothing.
pub(crate) fn offered(system: &Connection, compositor: bool) -> String {
    let asked =
        |method: &str| system.call_method(Some(LOGIND), MANAGER_PATH, Some(MANAGER), method, &());
    let mut said = String::new();
    for (power, method) in [
        (Power::Sleep, "CanSuspend"),
        (Power::Restart, "CanReboot"),
        (Power::ShutDown, "CanPowerOff"),
    ] {
        // `challenge` is asking for a password, which a locked screen
        // cannot show; `no` and `na` are not to be had at all.
        let answer = asked(method).ok();
        let answer = answer.and_then(|reply| reply.body().deserialize::<String>().ok());
        if answer.as_deref() == Some("yes") {
            let _ = writeln!(said, "can {}", power.word());
        }
    }
    if compositor {
        said.push_str("can log-out\n");
    }
    let listed = asked("ListInhibitors").ok();
    let holding: Vec<(String, String, String, String, u32, u32)> = listed
        .and_then(|reply| reply.body().deserialize().ok())
        .unwrap_or_default();
    // Only a block holds the machine; a delay, as the session's own before
    // sleep, only has it wait a moment.
    let blocking = holding.iter().filter(|held| held.3 == "block");
    for (what, who, why, ..) in blocking {
        let kinds = what.split(':');
        let held = kinds.filter_map(|kind| match kind {
            "sleep" => Some("sleep"),
            "shutdown" => Some("shut-down"),
            _ => None,
        });
        for from in held {
            let _ = writeln!(said, "held {from} {}: {}", plain(who), plain(why));
        }
    }
    said
}

/// What a program said, on one line, no longer than is told.
fn plain(said: &str) -> String {
    let plain = said.chars().map(|character| {
        if character.is_control() {
            ' '
        } else {
            character
        }
    });
    plain.take(LONGEST_SAID).collect()
}

/// Asks the compositor at `compositor` to end the session.
pub(crate) fn log_out(compositor: &Path) -> Result<(), Failure> {
    let quit = Request::Perform {
        action: serde_json::Value::from("quit"),
    };
    let connected = Client::connect(compositor);
    let answer =
        connected.and_then(|mut client| client.send(&quit).and_then(|()| client.receive()));
    match answer {
        Ok(Reply::Error { message }) => Err(message.into()),
        Ok(_) => Ok(()),
        Err(failure) => Err(failure.into()),
    }
}
