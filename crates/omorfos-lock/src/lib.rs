//! The lock screen of the omorfos desktop.
//!
//! It locks the session through the compositor's session lock, puts a
//! surface on every screen, and unlocks it once the user has shown who
//! they are, in whichever of the ways PAM is set up for: a password, a
//! finger, a security key. Each way is asked at once, by a helper of its
//! own, [`check`]; whichever says yes first unlocks. See
//! `docs/adr/0052-the-lock-screen.md`.

pub mod check;
mod screen;
mod ways;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use omorfos_config::Lock as Offered;

pub use screen::{Failure, Lock};
pub use ways::Way;

/// The ways the configuration offers: the password always, and a finger
/// and a key unless it says not to.
#[must_use]
pub fn offered(lock: Offered) -> Vec<Way> {
    let ways = [
        (true, Way::Password),
        (lock.fingerprint, Way::Fingerprint),
        (lock.key, Way::Key),
    ];
    ways.into_iter()
        .filter(|(offered, _)| *offered)
        .map(|(_, way)| way)
        .collect()
}

/// How the lock screen ended, as its exit code, having said why to `say`
/// if it was not with the session unlocked.
pub fn ended(
    started: Result<(), Failure>,
    say: &mut impl std::io::Write,
) -> std::process::ExitCode {
    started.map_or_else(
        |failure| {
            // If even that cannot be said, there is nobody to say it to.
            let _ = writeln!(say, "omorfos-lock: {failure}");
            std::process::ExitCode::FAILURE
        },
        |()| std::process::ExitCode::SUCCESS,
    )
}

/// Who the lock screen is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// The name they log in with, which the helpers ask PAM about.
    pub name: String,
    /// What the lock screen calls them: their full name where the system
    /// has one, and else their name.
    pub called: String,
    /// The files their face may be in, as the login screens keep it: the
    /// first that can be read is shown.
    pub faces: Vec<PathBuf>,
}

/// The user `uid` is, as `passwd` lists users, or else as the environment
/// says, named `user` and at home in `home`.
#[must_use]
pub fn who(passwd: &str, uid: u32, user: Option<&OsStr>, home: Option<&OsStr>) -> User {
    // A name, the password's place, the number, the group's, and then
    // what is known of them: their full name first, before any comma.
    let listed = passwd.lines().find_map(|line| {
        let (name, rest) = line.split_once(':')?;
        let mut fields = rest.split(':');
        let number: u32 = fields.nth(1)?.parse().ok()?;
        let full = fields.nth(1).and_then(|about| about.split(',').next());
        (number == uid).then(|| (name.to_owned(), full.unwrap_or_default().trim().to_owned()))
    });
    let named = user.map(|user| user.to_string_lossy().into_owned());
    let (name, full) = listed.unwrap_or_else(|| (named.unwrap_or_default(), String::new()));
    // AccountsService keeps the face a login screen shows, by the name,
    // which is never a path; older ones are kept at home.
    let kept = (!name.is_empty() && !name.contains('/'))
        .then(|| Path::new("/var/lib/AccountsService/icons").join(&name));
    let home = home.map(Path::new);
    let at_home = [".face", ".face.icon"].map(|face| home.map(|home| home.join(face)));
    User {
        called: if full.is_empty() { name.clone() } else { full },
        name,
        faces: kept
            .into_iter()
            .chain(at_home.into_iter().flatten())
            .collect(),
    }
}

/// Locks the session the environment describes, until it is unlocked or
/// the lock screen cannot go on, and returns why not if it could not.
///
/// The Wayland socket is `wayland_display` (`WAYLAND_DISPLAY`) in
/// `runtime_dir` (`XDG_RUNTIME_DIR`), and the configuration directory is
/// found from `config_home` (`XDG_CONFIG_HOME`) and `home` (`HOME`). The
/// helpers are `helper`, this program, and ask about `user`.
///
/// # Errors
///
/// Returns why the lock screen could not go on.
pub fn start(
    (runtime_dir, wayland_display): (Option<&OsStr>, Option<&OsStr>),
    (config_home, home): (Option<&OsStr>, Option<&OsStr>),
    (helper, user): (PathBuf, User),
) -> Result<(), Failure> {
    let wayland = runtime_dir.zip(wayland_display);
    let wayland = wayland.map(|(directory, display)| Path::new(directory).join(display));
    let config = omorfos_config::config_dir(config_home, home);
    let (wayland, config) = wayland.zip(config).ok_or(
        "this is not an omorfos session: XDG_RUNTIME_DIR, WAYLAND_DISPLAY or HOME is not set",
    )?;
    let ways = offered(omorfos_config::Store::open(&config).current().general.lock);
    let connected = omorfos_shell::connect(&wayland).map_err(Failure::from);
    let lock = connected.and_then(|connected| Lock::new(connected, &config, (helper, user), &ways));
    lock.and_then(|mut lock| lock.serve())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_user_is_who_the_system_says_or_else_who_the_environment_does() {
        let passwd = "root:x:0:0::/root:/bin/sh\n\
                      someone:x:1000:1000: Some One ,Room 1,,:/home/someone:/bin/sh\n\
                      plain:x:1001:1001::/home/plain:/bin/sh\n";
        let home = Some(OsStr::new("/home/someone"));
        let someone = who(passwd, 1000, Some(OsStr::new("other")), home);
        assert_eq!(someone.name, "someone");
        assert_eq!(someone.called, "Some One", "by their full name");
        assert_eq!(
            someone.faces,
            [
                "/var/lib/AccountsService/icons/someone",
                "/home/someone/.face",
                "/home/someone/.face.icon"
            ]
            .map(PathBuf::from)
        );
        let plain = who(passwd, 1001, None, None);
        assert_eq!(
            (plain.name.as_str(), plain.called.as_str()),
            ("plain", "plain")
        );
        assert_eq!(
            plain.faces.len(),
            1,
            "with no home, only what is kept for them"
        );
        let other = who(passwd, 1002, Some(OsStr::new("other")), None);
        assert_eq!(
            (other.name.as_str(), other.called.as_str()),
            ("other", "other")
        );
        // A name that is a path is not looked for a face by.
        let odd = who("", 1000, Some(OsStr::new("../x")), None);
        assert_eq!(odd.faces, Vec::<PathBuf>::new());
        let nobody = who("broken\nno:x:many\nshort:x\n", 1000, None, None);
        assert_eq!(nobody.name, "");
        assert_eq!(nobody.faces, Vec::<PathBuf>::new());
    }

    #[test]
    fn the_password_is_always_offered_and_the_rest_unless_turned_off() {
        let all = Offered::default();
        assert_eq!(offered(all), [Way::Password, Way::Fingerprint, Way::Key]);
        let none = Offered {
            fingerprint: false,
            key: false,
        };
        assert_eq!(offered(none), [Way::Password]);
        assert_eq!(Way::Key.service(), "omorfos-lock-key");
    }

    #[test]
    fn the_lock_screen_ends_well_once_unlocked_and_says_why_if_not() {
        let mut said = Vec::new();
        assert_eq!(ended(Ok(()), &mut said), std::process::ExitCode::SUCCESS);
        assert_eq!(said, b"");
        let failed = ended(Err("gone".into()), &mut said);
        assert_eq!(failed, std::process::ExitCode::FAILURE);
        assert_eq!(String::from_utf8(said).unwrap(), "omorfos-lock: gone\n");
    }

    #[test]
    fn outside_a_session_the_lock_screen_says_so() {
        let someone = who("", 0, Some(OsStr::new("someone")), None);
        let helper = (PathBuf::from("omorfos-lock"), someone);
        let home = Some(OsStr::new("/nowhere/home"));
        let failure = start((None, None), (None, home), helper.clone()).unwrap_err();
        assert!(failure.to_string().contains("not an omorfos session"));
        // Nor is a session joined whose socket is not there.
        let directory = tempfile::TempDir::new().unwrap();
        let runtime = Some(directory.path().as_os_str());
        let display = Some(OsStr::new("wayland-none"));
        let failure = start((runtime, display), (runtime, None), helper).unwrap_err();
        assert!(!failure.to_string().contains("not an omorfos session"));
    }
}
