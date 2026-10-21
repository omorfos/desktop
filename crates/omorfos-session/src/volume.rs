//! The volume, set by WirePlumber's `wpctl`.
//!
//! WirePlumber is what decides how a volume is set: on a sound card, its
//! own mixer, through the card's route, kept for next time; elsewhere, the
//! stream's. Asking it, as every other mixer does, keeps the volume the
//! same wherever it is looked at, and leaves to it what only a sound card
//! can show.

use std::ffi::OsStr;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::{Failure, Level, PATIENCE, Volume};

/// The sound that plays, and what records it.
const SINK: &str = "@DEFAULT_AUDIO_SINK@";
const SOURCE: &str = "@DEFAULT_AUDIO_SOURCE@";

/// How much one press of a key changes the volume, as `wpctl` writes it.
const STEP: &str = "5%";

/// Changes the volume, and returns how loud it is then.
pub(crate) fn change(volume: Volume, path: &OsStr) -> Result<Level, Failure> {
    let up = format!("{STEP}+");
    let down = format!("{STEP}-");
    let asked: &[&str] = match volume {
        // No louder than full, which is as loud as the card goes unaided.
        Volume::Up => &["set-volume", "-l", "1.0", SINK, &up],
        Volume::Down => &["set-volume", SINK, &down],
        Volume::Mute => &["set-mute", SINK, "toggle"],
    };
    let (value, muted) = set(asked, SINK, path)?;
    let icon = match value {
        _ if muted => "audio-volume-muted",
        0..34 => "audio-volume-low",
        34..67 => "audio-volume-medium",
        _ => "audio-volume-high",
    };
    let title = if muted { "Muted" } else { "Volume" };
    Ok(Level {
        tag: "volume",
        title,
        icon,
        value,
    })
}

/// Mutes the microphone, or lets it be heard again, and returns which.
pub(crate) fn microphone(path: &OsStr) -> Result<Level, Failure> {
    let (value, muted) = set(&["set-mute", SOURCE, "toggle"], SOURCE, path)?;
    let (title, icon) = if muted {
        ("Microphone muted", "microphone-sensitivity-muted")
    } else {
        ("Microphone on", "microphone-sensitivity-high")
    };
    Ok(Level {
        tag: "microphone",
        title,
        icon,
        value,
    })
}

/// Asks `wpctl` for what `asked` says, and then how loud `what` is, out of
/// 100, and whether it is muted.
fn set(asked: &[&str], what: &str, path: &OsStr) -> Result<(u8, bool), Failure> {
    wpctl(asked, path)?;
    heard(&wpctl(&["get-volume", what], path)?)
}

/// How loud, out of 100, and whether muted, from what `wpctl get-volume`
/// says: `Volume: 0.45`, with ` [MUTED]` after it if it is. Louder than
/// full is shown as full.
fn heard(said: &str) -> Result<(u8, bool), Failure> {
    let mut words = said.split_whitespace();
    let volume = words.nth(1).and_then(|volume| volume.parse::<f64>().ok());
    let volume = volume.filter(|volume| volume.is_finite() && *volume >= 0.0);
    let volume = volume.ok_or_else(|| format!("wpctl said \"{}\"", said.trim()))?;
    let muted = words.any(|word| word == "[MUTED]");
    // Within 0 to 100, so the conversion is exact.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "held to 0 to 100 first"
    )]
    let value = (volume * 100.0).round().min(100.0) as u8;
    Ok((value, muted))
}

/// Runs `wpctl` with `arguments`, found on `path`, and returns what it
/// said, if it did as asked in time.
fn wpctl(arguments: &[&str], path: &OsStr) -> Result<String, Failure> {
    let mut command = Command::new("wpctl");
    command.args(arguments).env("PATH", path);
    command.stdin(Stdio::null()).stdout(Stdio::piped());
    let mut child = command.spawn().map_err(|error| format!("wpctl: {error}"))?;
    let given = Instant::now() + PATIENCE;
    // One that cannot be asked whether it has finished is waited for as
    // one that has not.
    while child.try_wait().ok().flatten().is_none() {
        if Instant::now() >= given {
            let _ = child.kill();
            let _ = child.wait();
            return Err("wpctl did not answer in time".into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output().ok();
    let output = output.filter(|output| output.status.success());
    let output = output.ok_or_else(|| format!("wpctl {} failed", arguments.join(" ")))?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
