//! Sleep, restart and shut down, asked of logind, which decides whether
//! this user may: on a machine of one user at its keyboard, it may.

use zbus::blocking::Connection;

use crate::Failure;
use crate::watch::{LOGIND, MANAGER, MANAGER_PATH};

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

/// Asks logind for `power`.
pub(crate) fn ask(system: &Connection, power: Power) -> Result<(), Failure> {
    let method = match power {
        Power::Sleep => "Suspend",
        Power::Restart => "Reboot",
        Power::ShutDown => "PowerOff",
    };
    // Interactive: where this user may only with leave, polkit is to ask.
    system.call_method(Some(LOGIND), MANAGER_PATH, Some(MANAGER), method, &(true,))?;
    Ok(())
}
