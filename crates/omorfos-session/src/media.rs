//! What is playing, told what to do over the session's bus, as players
//! listen for (MPRIS).

use zbus::blocking::Connection;
use zbus::zvariant::OwnedValue;

use crate::{Failure, Media};

/// What a player's name on the bus begins with.
const PLAYER: &str = "org.mpris.MediaPlayer2.";

/// Where a player answers, and as what.
const PATH: &str = "/org/mpris/MediaPlayer2";
const INTERFACE: &str = "org.mpris.MediaPlayer2.Player";

/// The most players asked whether they are playing. A bus with more is
/// not one a person is listening to all of.
const MOST: usize = 16;

/// Tells the player that matters what `media` asks: the first that is
/// playing, or else the first that is paused, or else the first there is.
pub(crate) fn tell(session: &Connection, media: Media) -> Result<(), Failure> {
    // A bus that will not say who is on it has nobody to tell.
    let reply = session.call_method(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        Some("org.freedesktop.DBus"),
        "ListNames",
        &(),
    );
    let names = reply.ok().and_then(|reply| reply.body().deserialize().ok());
    let mut names: Vec<String> = names.unwrap_or_default();
    names.retain(|name| name.starts_with(PLAYER));
    names.sort();
    names.truncate(MOST);
    let rank = |name: &String| match status(session, name).as_deref() {
        Some("Playing") => 0,
        Some("Paused") => 1,
        _ => 2,
    };
    let player = names.iter().min_by_key(|name| rank(name));
    let player = player.ok_or("nothing is playing that can be told to")?;
    let member = match media {
        Media::PlayPause => "PlayPause",
        Media::Next => "Next",
        Media::Previous => "Previous",
        Media::Stop => "Stop",
    };
    session.call_method(Some(player.as_str()), PATH, Some(INTERFACE), member, &())?;
    Ok(())
}

/// Whether a player says it is playing, paused or stopped, if it says.
fn status(session: &Connection, name: &str) -> Option<String> {
    let asked = (INTERFACE, "PlaybackStatus");
    let properties = Some("org.freedesktop.DBus.Properties");
    let reply = session.call_method(Some(name), PATH, properties, "Get", &asked);
    let value: Option<OwnedValue> = reply.ok().and_then(|reply| reply.body().deserialize().ok());
    value.and_then(|value| String::try_from(value).ok())
}
