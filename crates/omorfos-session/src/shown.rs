//! How loud or bright it is, shown as a notification that takes the place
//! of the one before it, by the hints notification services know.

use std::collections::HashMap;

use zbus::blocking::Connection;
use zbus::zvariant::Value;

use crate::{Failure, Level};

/// How long it stays, in milliseconds: long enough to read, short enough
/// to be gone by the time it is not wanted.
const STAYS: i32 = 1500;

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
    let name = "org.freedesktop.Notifications";
    session.call_method(
        Some(name),
        "/org/freedesktop/Notifications",
        Some(name),
        "Notify",
        &body,
    )?;
    Ok(())
}
