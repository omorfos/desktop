//! The `omorfos-session` executable: watches over the session, or does
//! what a key or the menu asks.

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;
use std::{env, io};

use omorfos_ipc::help::{About, answered};
use omorfos_session::{Places, Request};

/// What it says when asked what it is.
const ABOUT: About = About {
    name: "omorfos-session",
    what: "What an omorfos session does besides draw: locking before sleep, starting programs \
           at login, the volume, the brightness and what is playing.",
    usage: " [COMMAND]",
    commands: &[
        (
            "volume up|down|mute",
            "Turn the sound that plays up or down by five hundredths, or mute it \
             and back",
        ),
        ("microphone mute", "Mute the microphone, or hear it again"),
        (
            "brightness up|down",
            "Make the screen a twentieth brighter or darker, never quite dark",
        ),
        (
            "media play-pause",
            "Tell the player that is playing, or else one that is paused, to \
             play or to pause",
        ),
        (
            "media next|previous|stop",
            "Tell it to go on to the next, back to the one before, or stop",
        ),
        (
            "sleep|restart|shut-down",
            "Ask logind for the machine to sleep, start again or turn off",
        ),
        ("log-out", "End the session, as the compositor ends it"),
        (
            "offered",
            "Print which of sleep, restart, shut-down and log-out a locked screen \
             may offer, and what holds the machine",
        ),
        (
            "autostart",
            "Start what is to start with the session, as at login",
        ),
        (
            "forget ssh|gpg...",
            "Have ssh-agent forget its keys, or gpg-agent its passphrases",
        ),
    ],
    options: &[(
        "--locked",
        "With sleep, restart or shut-down: asked from a locked screen, so done \
         only where no password is asked for",
    )],
    examples: &[
        "omorfos-session volume up",
        "omorfos-session media next",
        "omorfos-session forget ssh gpg",
    ],
    notes: "Without a command it watches over the session for as long as it lasts, locking \
            the screen before the machine sleeps; the compositor starts it so at login. With \
            one, it does what a key or the menu asks, and can be bound to keys in keys.yaml as \
            `spawn: [omorfos-session, volume, up]`.",
    version: env!("CARGO_PKG_VERSION"),
};

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    if answered(&ABOUT, &arguments, &mut io::stdout()) {
        return ExitCode::SUCCESS;
    }
    let arguments: Vec<String> = env::args().skip(1).collect();
    let done = Request::parse(&arguments).map_err(|unknown| (unknown.into(), ExitCode::from(2)));
    let places = Places::from(&|name| env::var_os(name));
    let done = done.and_then(|request| {
        omorfos_session::run(request, &places).map_err(|failure| (failure, ExitCode::FAILURE))
    });
    match done {
        Ok(said) => {
            // What is offered goes to whoever asked, if they still listen.
            let _ = io::stdout().write_all(said.as_bytes());
            ExitCode::SUCCESS
        }
        Err((failure, code)) => {
            // If even that cannot be said, there is nobody to say it to.
            let _ = writeln!(io::stderr(), "omorfos-session: {failure}");
            code
        }
    }
}
