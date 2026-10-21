//! How loud or bright it is, shown as a notification that takes the place
//! of the one before it, by the hints notification services know; and
//! what went wrong, shown until it is dismissed.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::{Failure, Level};

/// How long it stays, in milliseconds: long enough to read, short enough
/// to be gone by the time it is not wanted.
const STAYS: i32 = 1500;

/// Says that something went wrong that the user is to know of, until it
/// is dismissed.
pub(crate) fn warn(session: &Connection, title: &str, body: &str) -> Result<(), Failure> {
    let hints = HashMap::from([("urgency", Value::U8(2))]);
    let actions: Vec<&str> = Vec::new();
    let sent = (
        "omorfos",
        0,
        "dialog-warning",
        title,
        body,
        actions,
        hints,
        0,
    );
    notify(session, &sent)
}

/// Shows `level` among the notifications.
pub(crate) fn show(session: &Connection, level: &Level) -> Result<(), Failure> {
    let hints = HashMap::from([
        ("value", Value::I32(i32::from(level.value))),
        ("synchronous", Value::from(level.tag)),
        // Not one to be kept for later, as something that happened is.
        ("transient", Value::Bool(true)),
        ("urgency", Value::U8(0)),
    ]);
    let actions: Vec<&str> = Vec::new();
    let body = (
        "omorfos",
        0_u32,
        level.icon,
        level.title,
        "",
        actions,
        hints,
        STAYS,
    );
    notify(session, &body)
}

/// A notification as the specification has it be sent: from whom, what
/// it replaces, its icon, title and words, what can be done with it, what
/// else it says, and for how long it is shown.
type Notification<'a> = (
    &'a str,
    u32,
    &'a str,
    &'a str,
    &'a str,
    Vec<&'a str>,
    HashMap<&'a str, Value<'a>>,
    i32,
);

/// Sends a notification.
fn notify(session: &Connection, body: &Notification<'_>) -> Result<(), Failure> {
    let name = "org.freedesktop.Notifications";
    session.call_method(
        Some(name),
        "/org/freedesktop/Notifications",
        Some(name),
        "Notify",
        body,
    )?;
    Ok(())
}
