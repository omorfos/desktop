//! What an omorfos session does besides draw, as ADR 35 has it. Run with
//! no command, as the compositor runs it, it runs all session and locks
//! the screen when logind asks and before the machine sleeps (see
//! [`watch`]). What the keys for the volume, the brightness and what is
//! playing ask for, and what the menu's sleep, restart and shut down
//! do, are each asked for as a command:
//!
//! ```text
//! omorfos-session volume up|down|mute
//! omorfos-session microphone mute
//! omorfos-session brightness up|down
//! omorfos-session media play-pause|next|previous|stop
//! omorfos-session sleep|restart|shut-down
//! omorfos-session autostart
//! ```
//!
//! The volume is set by WirePlumber's `wpctl`, which sets a sound card's
//! own volume as every other mixer does; the brightness by logind, which
//! lets the user who is logged in set it; and what is playing is told over
//! D-Bus, as players listen for (MPRIS). How loud or how bright it is
//! then is shown as a notification, which takes the place of the one
//! before. Sleep and the rest are asked of logind. `autostart` starts what
//! the desktop entries in `~/.config/autostart` and the system's say is to
//! start with the session, which the compositor runs once, at login.

mod autostart;
mod brightness;
mod media;
mod power;
mod shown;
#[cfg(test)]
mod stand_in;
mod volume;
mod watch;

use std::error::Error;
use std::ffi::OsString;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use zbus::blocking::Connection;
use zbus::blocking::connection::Builder;

pub use autostart::directories;
pub use brightness::Backlight;
pub use power::Power;
pub use watch::watch;

/// Why what was asked could not be done.
pub type Failure = Box<dyn Error + Send + Sync>;

/// How long anything spoken to is given to answer before it is given up
/// on: long enough for a busy machine, short enough that a key pressed
/// again is not kept waiting behind it.
pub const PATIENCE: Duration = Duration::from_secs(2);

/// Where programs are looked for when the environment does not say.
const PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Where the system bus is when the environment does not say.
const SYSTEM_BUS: &str = "unix:path=/var/run/dbus/system_bus_socket";

/// What is asked for: by a key, by the menu, or by the compositor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// The session watched over for as long as it lasts, which is what
    /// the compositor asks of it.
    Watch,
    /// The sound that plays: louder, quieter, or muted and back.
    Volume(Volume),
    /// The microphone muted, or heard again.
    Microphone,
    /// The screen brighter or darker.
    Brightness(Step),
    /// What is playing told to play or pause, go on or back, or stop.
    Media(Media),
    /// The machine to sleep, start again or turn off.
    Power(Power),
    /// What is to start with the session started.
    Autostart,
}

/// What is asked of the volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Volume {
    /// Louder, by a step, as far as full.
    Up,
    /// Quieter, by a step.
    Down,
    /// Muted, or heard again.
    Mute,
}

/// Which way a level goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Higher.
    Up,
    /// Lower.
    Down,
}

/// What is asked of what is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Media {
    /// Play if it is paused, and pause if it is playing.
    PlayPause,
    /// The next track.
    Next,
    /// The track before.
    Previous,
    /// Stop.
    Stop,
}

/// What was asked was not something this does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unknown;

impl fmt::Display for Unknown {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "usage: omorfos-session [volume up|down|mute | microphone mute | \
             brightness up|down | media play-pause|next|previous|stop | \
             sleep | restart | shut-down | autostart]",
        )
    }
}

impl Error for Unknown {}

impl Request {
    /// What the arguments after the program's name ask for.
    ///
    /// # Errors
    ///
    /// [`Unknown`] if they ask for nothing this does.
    pub fn parse<S: AsRef<str>>(arguments: &[S]) -> Result<Self, Unknown> {
        let words: Vec<&str> = arguments.iter().map(AsRef::as_ref).collect();
        match words.as_slice() {
            [] => Ok(Self::Watch),
            ["volume", "up"] => Ok(Self::Volume(Volume::Up)),
            ["volume", "down"] => Ok(Self::Volume(Volume::Down)),
            ["volume", "mute"] => Ok(Self::Volume(Volume::Mute)),
            ["microphone", "mute"] => Ok(Self::Microphone),
            ["brightness", "up"] => Ok(Self::Brightness(Step::Up)),
            ["brightness", "down"] => Ok(Self::Brightness(Step::Down)),
            ["media", "play-pause"] => Ok(Self::Media(Media::PlayPause)),
            ["media", "next"] => Ok(Self::Media(Media::Next)),
            ["media", "previous"] => Ok(Self::Media(Media::Previous)),
            ["media", "stop"] => Ok(Self::Media(Media::Stop)),
            ["sleep"] => Ok(Self::Power(Power::Sleep)),
            ["restart"] => Ok(Self::Power(Power::Restart)),
            ["shut-down"] => Ok(Self::Power(Power::ShutDown)),
            ["autostart"] => Ok(Self::Autostart),
            _ => Err(Unknown),
        }
    }
}

/// Where what is spoken to is found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Places {
    /// The directory the screens' backlights are in.
    pub backlights: PathBuf,
    /// Where programs are looked for, `wpctl` among them.
    pub path: OsString,
    /// The session's bus, which players and the notifications are on.
    pub session: Option<String>,
    /// The system's bus, which logind is on.
    pub system: String,
    /// The compositor's socket.
    pub compositor: Option<PathBuf>,
    /// Where the desktop entries of what is to start with the session are,
    /// the user's own first.
    pub autostart: Vec<PathBuf>,
    /// What the session's desktop is called, as an entry may name it to
    /// be started only in it, or never: `omorfos`, unless told otherwise.
    pub desktops: Vec<String>,
    /// What starts what is to start with the session in a scope of its
    /// own that ends with the session: `systemd-run`, where systemd's user
    /// manager runs, and nothing otherwise.
    pub scoper: Option<PathBuf>,
}

impl Places {
    /// Where they are on this machine, as the environment says: looked up
    /// by `variable`, which is passed in so that this can be tested
    /// without touching the environment.
    #[must_use]
    pub fn from(variable: &dyn Fn(&str) -> Option<OsString>) -> Self {
        let address = |name| variable(name).and_then(|value| value.into_string().ok());
        Self {
            backlights: PathBuf::from("/sys/class/backlight"),
            path: variable("PATH").unwrap_or_else(|| OsString::from(PATH)),
            session: address("DBUS_SESSION_BUS_ADDRESS"),
            system: address("DBUS_SYSTEM_BUS_ADDRESS").unwrap_or_else(|| SYSTEM_BUS.to_owned()),
            compositor: omorfos_ipc::socket_path(
                variable("OMORFOS_SOCKET").as_deref(),
                variable("XDG_RUNTIME_DIR").as_deref(),
                variable("WAYLAND_DISPLAY").as_deref(),
            ),
            autostart: directories(
                variable("XDG_CONFIG_HOME").as_deref(),
                variable("HOME").as_deref(),
                variable("XDG_CONFIG_DIRS").as_deref(),
            ),
            scoper: user_manager(variable("XDG_RUNTIME_DIR").as_deref())
                .then(|| PathBuf::from("systemd-run")),
            desktops: address("XDG_CURRENT_DESKTOP")
                .unwrap_or_else(|| "omorfos".to_owned())
                .split(':')
                .map(str::to_owned)
                .collect(),
        }
    }
}

/// Whether systemd's user manager runs for the session whose runtime
/// directory is `runtime`, as its socket there says.
fn user_manager(runtime: Option<&std::ffi::OsStr>) -> bool {
    runtime.is_some_and(|runtime| {
        std::path::Path::new(runtime)
            .join("systemd/private")
            .exists()
    })
}

/// Does what was asked, and shows how loud or bright it is then; or
/// watches over the session until it ends.
///
/// # Errors
///
/// If what it is asked of is not there or does not answer in time: no
/// `wpctl`, no screen whose brightness can be set, no player, a bus that
/// cannot be reached, or logind refusing.
pub fn run(request: Request, places: &Places) -> Result<(), Failure> {
    let session = || connect(places.session.as_deref().ok_or("there is no session bus")?);
    let level = match request {
        Request::Volume(volume) => volume::change(volume, &places.path)?,
        Request::Microphone => volume::microphone(&places.path)?,
        Request::Brightness(step) => {
            let backlight = Backlight::find(&places.backlights)?;
            let changed = backlight.stepped(step);
            brightness::set(&connect(&places.system)?, &backlight, changed)?;
            changed.shown()
        }
        Request::Media(media) => return media::tell(&session()?, media),
        Request::Power(power) => return power::ask(&connect(&places.system)?, power),
        Request::Watch => return watch(places),
        Request::Autostart => return autostart::start(places),
    };
    // That it changed is what matters; showing it is a courtesy, and is
    // done without where there is nothing to show it.
    if let Ok(bus) = session() {
        let _ = shown::show(&bus, &level);
    }
    Ok(())
}

/// A connection to the bus at `address`, which gives up on a call that is
/// not answered in time.
fn connect(address: &str) -> Result<Connection, Failure> {
    let builder = Builder::address(address)?.method_timeout(PATIENCE);
    Ok(builder.build()?)
}

/// What is shown once a level has changed.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Level {
    /// What it is of, which only one is shown of at a time.
    tag: &'static str,
    /// What it says.
    title: &'static str,
    /// The icon it is shown with.
    icon: &'static str,
    /// How full, out of 100.
    value: u8,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stand_in::{self, Daemon};

    /// Places with nothing in them but what a test gives.
    fn places(session: Option<&Daemon>, system: &Daemon, wpctl: &str) -> Places {
        Places {
            backlights: PathBuf::from("/nowhere"),
            path: stand_in::wpctl(wpctl).into_os_string(),
            session: session.map(|daemon| daemon.address.clone()),
            system: system.address.clone(),
            compositor: None,
            autostart: Vec::new(),
            desktops: Vec::new(),
            scoper: None,
        }
    }

    fn heard(icon: &str, title: &str, value: i32, tag: &str) -> stand_in::Heard {
        (icon.to_owned(), title.to_owned(), value, tag.to_owned())
    }

    #[test]
    fn the_volume_is_changed_and_how_loud_it_is_then_is_shown() {
        let daemon = Daemon::start();
        let (_service, shown) = stand_in::notifications(&daemon);
        let places = places(Some(&daemon), &daemon, "wpctl");
        // The stand-in fails on anything but what each is to ask.
        for volume in [Volume::Up, Volume::Down, Volume::Mute] {
            run(Request::Volume(volume), &places).unwrap();
        }
        let medium = heard("audio-volume-medium", "Volume", 45, "volume");
        assert_eq!(
            *shown.lock().unwrap(),
            [medium.clone(), medium.clone(), medium]
        );
        run(Request::Microphone, &places).unwrap();
        let on = heard(
            "microphone-sensitivity-high",
            "Microphone on",
            100,
            "microphone",
        );
        assert_eq!(shown.lock().unwrap().last(), Some(&on));
    }

    #[test]
    fn how_loud_it_is_is_shown_by_an_icon_for_how_loud() {
        let daemon = Daemon::start();
        let (_service, shown) = stand_in::notifications(&daemon);
        for kind in ["quiet", "loud", "muted"] {
            let places = places(Some(&daemon), &daemon, kind);
            run(Request::Volume(Volume::Up), &places).unwrap();
        }
        run(
            Request::Microphone,
            &places(Some(&daemon), &daemon, "muted"),
        )
        .unwrap();
        let expected = [
            heard("audio-volume-low", "Volume", 10, "volume"),
            // Louder than full is shown as full.
            heard("audio-volume-high", "Volume", 100, "volume"),
            heard("audio-volume-muted", "Muted", 30, "volume"),
            heard(
                "microphone-sensitivity-muted",
                "Microphone muted",
                30,
                "microphone",
            ),
        ];
        assert_eq!(*shown.lock().unwrap(), expected);
    }

    #[test]
    fn the_volume_is_changed_where_there_is_nothing_to_show_it_on() {
        let daemon = Daemon::start();
        // No session bus, and a bus with nobody showing notifications.
        run(Request::Volume(Volume::Up), &places(None, &daemon, "wpctl")).unwrap();
        run(
            Request::Volume(Volume::Up),
            &places(Some(&daemon), &daemon, "wpctl"),
        )
        .unwrap();
    }

    #[test]
    fn a_wpctl_that_is_not_there_fails_or_does_not_answer_is_said_to() {
        let daemon = Daemon::start();
        let failed = |kind| {
            let places = places(Some(&daemon), &daemon, kind);
            run(Request::Volume(Volume::Up), &places)
                .unwrap_err()
                .to_string()
        };
        assert!(failed("nowhere").starts_with("wpctl: "), "not there");
        assert_eq!(
            failed("broken"),
            "wpctl set-volume -l 1.0 @DEFAULT_AUDIO_SINK@ 5%+ failed"
        );
        assert_eq!(failed("garbled"), "wpctl said \"Volume: loud\"");
        let started = std::time::Instant::now();
        assert_eq!(failed("slow"), "wpctl did not answer in time");
        assert!(
            started.elapsed() < PATIENCE * 3,
            "and it is not waited for long"
        );
    }

    #[test]
    fn a_wpctl_that_sets_but_cannot_say_how_loud_or_mute_the_microphone_is_said_to() {
        let daemon = Daemon::start();
        let unheard = places(Some(&daemon), &daemon, "unheard");
        let failure = run(Request::Volume(Volume::Down), &unheard).unwrap_err();
        assert_eq!(
            failure.to_string(),
            "wpctl get-volume @DEFAULT_AUDIO_SINK@ failed"
        );
        let broken = places(Some(&daemon), &daemon, "broken");
        let failure = run(Request::Microphone, &broken).unwrap_err();
        assert_eq!(
            failure.to_string(),
            "wpctl set-mute @DEFAULT_AUDIO_SOURCE@ toggle failed"
        );
    }

    #[test]
    fn a_bus_that_cannot_be_reached_or_is_not_written_as_one_is_said_to() {
        let daemon = Daemon::start();
        let backlights = stand_in::backlights(&[("acpi_video0", "firmware", "5", "10")]);
        let unreachable = Places {
            backlights: backlights.path().to_owned(),
            system: "unix:path=/nowhere/bus".to_owned(),
            ..places(Some(&daemon), &daemon, "wpctl")
        };
        assert!(run(Request::Brightness(Step::Up), &unreachable).is_err());
        let garbled = Places {
            session: Some("not an address".to_owned()),
            ..places(Some(&daemon), &daemon, "wpctl")
        };
        assert!(run(Request::Media(Media::Next), &garbled).is_err());
    }

    #[test]
    fn the_brightness_is_set_by_logind_and_shown() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let (_service, shown) = stand_in::notifications(&daemon);
        let backlights = stand_in::backlights(&[("intel_backlight", "raw", "480", "960")]);
        let places = Places {
            backlights: backlights.path().to_owned(),
            ..places(Some(&daemon), &daemon, "wpctl")
        };
        run(Request::Brightness(Step::Up), &places).unwrap();
        run(Request::Brightness(Step::Down), &places).unwrap();
        let set = |value| ("backlight".to_owned(), "intel_backlight".to_owned(), value);
        assert_eq!(*logind.brightness.lock().unwrap(), [set(528), set(432)]);
        let expected =
            [55, 45].map(|value| heard("display-brightness", "Brightness", value, "brightness"));
        assert_eq!(*shown.lock().unwrap(), expected);
    }

    #[test]
    fn the_brightness_is_not_set_without_a_backlight_or_logind() {
        let daemon = Daemon::start();
        let without = places(Some(&daemon), &daemon, "wpctl");
        let failure = run(Request::Brightness(Step::Up), &without).unwrap_err();
        assert_eq!(
            failure.to_string(),
            "there is no screen whose brightness can be set"
        );
        let backlights = stand_in::backlights(&[("acpi_video0", "firmware", "5", "10")]);
        let no_logind = Places {
            backlights: backlights.path().to_owned(),
            ..places(Some(&daemon), &daemon, "wpctl")
        };
        assert!(run(Request::Brightness(Step::Up), &no_logind).is_err());
    }

    #[test]
    fn what_is_playing_is_told_and_one_playing_is_told_before_one_that_is_not() {
        let daemon = Daemon::start();
        let (stopped_player, stopped) = stand_in::player(&daemon, "a", "Stopped");
        let (paused_player, paused) = stand_in::player(&daemon, "b", "Paused");
        let (playing_player, playing) = stand_in::player(&daemon, "c", "Playing");
        let places = places(Some(&daemon), &daemon, "wpctl");
        for media in [Media::PlayPause, Media::Next, Media::Previous, Media::Stop] {
            run(Request::Media(media), &places).unwrap();
        }
        assert_eq!(
            *playing.lock().unwrap(),
            ["PlayPause", "Next", "Previous", "Stop"]
        );
        drop(playing_player);
        run(Request::Media(Media::Next), &places).unwrap();
        assert_eq!(*paused.lock().unwrap(), ["Next"], "then one that is paused");
        drop(paused_player);
        run(Request::Media(Media::Next), &places).unwrap();
        assert_eq!(*stopped.lock().unwrap(), ["Next"], "then any");
        drop(stopped_player);
    }

    #[test]
    fn a_player_that_does_not_answer_is_said_to() {
        let daemon = Daemon::start();
        let _mute = stand_in::mute(&daemon, "mute");
        let failure = run(
            Request::Media(Media::Next),
            &places(Some(&daemon), &daemon, "wpctl"),
        );
        assert!(failure.is_err());
    }

    #[test]
    fn with_nothing_playing_or_no_bus_nothing_is_told() {
        let daemon = Daemon::start();
        let failure = run(
            Request::Media(Media::Next),
            &places(Some(&daemon), &daemon, "wpctl"),
        );
        assert_eq!(
            failure.unwrap_err().to_string(),
            "nothing is playing that can be told to"
        );
        let failure = run(Request::Media(Media::Next), &places(None, &daemon, "wpctl"));
        assert_eq!(failure.unwrap_err().to_string(), "there is no session bus");
    }

    #[test]
    fn sleep_restart_and_shut_down_are_asked_of_logind_which_may_ask_first() {
        let daemon = Daemon::start();
        let logind = stand_in::logind(&daemon);
        let places = places(None, &daemon, "wpctl");
        for power in [Power::Sleep, Power::Restart, Power::ShutDown] {
            run(Request::Power(power), &places).unwrap();
        }
        let asked = ["Suspend true", "Reboot true", "PowerOff true"];
        assert_eq!(*logind.asked.lock().unwrap(), asked);
        // Gone, or going: either way it is not asked.
        drop(logind);
        assert!(run(Request::Power(Power::Sleep), &places).is_err());
        let unreachable = Places {
            system: "unix:path=/nowhere/bus".to_owned(),
            ..places
        };
        assert!(run(Request::Power(Power::Sleep), &unreachable).is_err());
    }

    #[test]
    fn what_is_to_start_with_the_session_is_started_when_asked() {
        let daemon = Daemon::start();
        let places = places(None, &daemon, "wpctl");
        assert!(
            run(Request::Autostart, &places).is_ok(),
            "with nothing to start"
        );
    }

    #[test]
    fn the_session_is_watched_over_when_nothing_else_is_asked() {
        let daemon = Daemon::start();
        let failure = run(Request::Watch, &places(None, &daemon, "wpctl")).unwrap_err();
        assert_eq!(
            failure.to_string(),
            "there is no compositor to lock the session of"
        );
    }

    #[test]
    fn what_is_asked_is_read_from_the_words_after_the_name() {
        let read = |words: &[&str]| Request::parse(words);
        assert_eq!(read(&["volume", "up"]), Ok(Request::Volume(Volume::Up)));
        assert_eq!(read(&["volume", "down"]), Ok(Request::Volume(Volume::Down)));
        assert_eq!(read(&["volume", "mute"]), Ok(Request::Volume(Volume::Mute)));
        assert_eq!(read(&["microphone", "mute"]), Ok(Request::Microphone));
        assert_eq!(
            read(&["brightness", "up"]),
            Ok(Request::Brightness(Step::Up))
        );
        assert_eq!(
            read(&["brightness", "down"]),
            Ok(Request::Brightness(Step::Down))
        );
        let media = [
            ("play-pause", Media::PlayPause),
            ("next", Media::Next),
            ("previous", Media::Previous),
            ("stop", Media::Stop),
        ];
        for (word, media) in media {
            assert_eq!(read(&["media", word]), Ok(Request::Media(media)));
        }
        let power = [
            ("sleep", Power::Sleep),
            ("restart", Power::Restart),
            ("shut-down", Power::ShutDown),
        ];
        for (word, power) in power {
            assert_eq!(read(&[word]), Ok(Request::Power(power)));
        }
        assert_eq!(read(&[]), Ok(Request::Watch), "as the compositor runs it");
        assert_eq!(read(&["autostart"]), Ok(Request::Autostart));
    }

    #[test]
    fn anything_else_is_not_understood_and_says_what_would_be() {
        for words in [
            &["sleep", "now"][..],
            &["volume"],
            &["volume", "loud"],
            &["volume", "up", "now"],
        ] {
            assert_eq!(Request::parse(words), Err(Unknown));
        }
        assert!(
            Unknown
                .to_string()
                .starts_with("usage: omorfos-session [volume up")
        );
    }

    #[test]
    fn places_are_where_the_environment_says_and_the_system_bus_is_known() {
        let none = Places::from(&|_| None);
        assert_eq!(none.backlights, PathBuf::from("/sys/class/backlight"));
        assert_eq!((none.path.to_str(), none.session), (Some(PATH), None));
        assert_eq!(none.system, SYSTEM_BUS);
        assert_eq!(none.compositor, None);
        assert_eq!(none.desktops, ["omorfos"]);
        assert_eq!(none.autostart, [PathBuf::from("/etc/xdg/autostart")]);
        assert_eq!(none.scoper, None);
        let all = Places::from(&|name| Some(OsString::from(format!("<{name}>"))));
        assert_eq!(all.path, OsString::from("<PATH>"));
        assert_eq!(all.session.as_deref(), Some("<DBUS_SESSION_BUS_ADDRESS>"));
        assert_eq!(all.system, "<DBUS_SYSTEM_BUS_ADDRESS>");
        assert_eq!(all.compositor, Some(PathBuf::from("<OMORFOS_SOCKET>")));
        assert_eq!(all.desktops, ["<XDG_CURRENT_DESKTOP>"]);
        // Found by the display, as every program of the session finds it.
        let found = Places::from(&|name| {
            let known = ["XDG_RUNTIME_DIR", "WAYLAND_DISPLAY"].contains(&name);
            known.then(|| OsString::from(name))
        });
        let socket = PathBuf::from("XDG_RUNTIME_DIR/omorfos-WAYLAND_DISPLAY.sock");
        assert_eq!(found.compositor, Some(socket));
        assert_eq!(found.scoper, None, "no user manager there");
        // Where systemd's user manager runs, what is started is scoped.
        let runtime = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(runtime.path().join("systemd")).unwrap();
        std::fs::write(runtime.path().join("systemd/private"), "").unwrap();
        let managed = Places::from(&|name| {
            (name == "XDG_RUNTIME_DIR").then(|| runtime.path().as_os_str().to_owned())
        });
        assert_eq!(managed.scoper, Some(PathBuf::from("systemd-run")));
    }
}
