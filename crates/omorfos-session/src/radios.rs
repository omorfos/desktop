//! The machine's radios, Wi-Fi, Bluetooth and the rest, as the kernel
//! knows them, whatever turned them off or on: a key the kernel handles
//! itself, a switch on the side, or another program.
//!
//! The kernel says of each radio as it comes and goes and each time it is
//! turned off or on, in `/dev/rfkill`, first of every radio there is. What
//! is known is told to the compositor, for its state to say to what shows
//! it, and each change after the first is shown as a notification, as the
//! volume is. Nothing here turns a radio off or on: the kernel has already
//! done what a key asks.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use omorfos_ipc::{Client, Radio, Request};
use rustix::event::{PollFd, PollFlags, Timespec, poll};

use crate::shown;

/// How long what the kernel says of one radio is, in its first form, which
/// every kernel says and later ones add to.
const EVENT: usize = 8;

/// What the kernel says has happened to a radio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Event {
    /// Which radio, by the kernel's number for it.
    radio: u32,
    /// What kind it is, by the kernel's number for that.
    kind: u8,
    /// What happened: it came, went, or changed.
    happened: u8,
    /// Whether a program has it off.
    soft: bool,
    /// Whether a switch, or the firmware, has it off.
    hard: bool,
}

/// It came, went, or was turned off or on, by the kernel's numbers.
const ADDED: u8 = 0;
const REMOVED: u8 = 1;
const CHANGED: u8 = 2;

/// What the kernel's bytes say: its number for the radio, as numbers are
/// on this machine, then the kind, what happened and how it is held.
fn event(bytes: [u8; EVENT]) -> Event {
    let [a, b, c, d, kind, happened, soft, hard] = bytes;
    Event {
        radio: u32::from_ne_bytes([a, b, c, d]),
        kind,
        happened,
        soft: soft != 0,
        hard: hard != 0,
    }
}

/// What each kind is called to the IPC, by the kernel's number for it; a
/// kind it has no name for is left out.
fn kind(number: u8) -> Option<&'static str> {
    Some(match number {
        1 => "wlan",
        2 => "bluetooth",
        3 => "uwb",
        4 => "wimax",
        5 => "wwan",
        6 => "gps",
        7 => "fm",
        8 => "nfc",
        _ => return None,
    })
}

/// Every radio the kernel has said of: its kind, and whether a program or
/// a switch has it off.
#[derive(Debug, Default)]
struct Radios {
    by: BTreeMap<u32, (u8, bool, bool)>,
}

impl Radios {
    /// Takes in what the kernel said.
    fn heard(&mut self, event: Event) {
        match event.happened {
            ADDED | CHANGED => {
                let held = (event.kind, event.soft, event.hard);
                self.by.insert(event.radio, held);
            }
            REMOVED => {
                self.by.remove(&event.radio);
            }
            // All of a kind being turned at once is said of each as well.
            _ => {}
        }
    }

    /// Each kind of radio there is, with whether all of it is off, and
    /// whether a switch holds any of it so.
    fn kinds(&self) -> Vec<Radio> {
        let mut kinds: BTreeMap<u8, Radio> = BTreeMap::new();
        for (number, soft, hard) in self.by.values() {
            let Some(name) = kind(*number) else {
                continue;
            };
            let of = kinds.entry(*number).or_insert_with(|| Radio {
                kind: name.to_owned(),
                off: true,
                switched: false,
            });
            of.off &= *soft || *hard;
            of.switched |= *hard;
        }
        kinds.into_values().collect()
    }
}

/// What is shown of a change from `before` to `after`, if anything: that
/// the machine is in aeroplane mode, all of it off, or no longer is; or
/// that a kind is off or on. With its icon.
fn said(before: &[Radio], after: &[Radio]) -> Option<(String, &'static str)> {
    let all_off = |radios: &[Radio]| !radios.is_empty() && radios.iter().all(|radio| radio.off);
    match (all_off(before), all_off(after)) {
        (false, true) => return Some(("Aeroplane mode on".to_owned(), "airplane-mode-symbolic")),
        (true, false) => {
            return Some((
                "Aeroplane mode off".to_owned(),
                "airplane-mode-disabled-symbolic",
            ));
        }
        _ => {}
    }
    let mut changed = after.iter().filter(|radio| {
        let was = before.iter().find(|was| was.kind == radio.kind);
        was.is_some_and(|was| was.off != radio.off)
    });
    let radio = changed.next()?;
    let called = radio.called();
    let (state, icon) = if radio.off {
        ("off", "network-wireless-disabled-symbolic")
    } else {
        ("on", "network-wireless-symbolic")
    };
    Some((format!("{called} {state}"), icon))
}

/// Follows the radios at `path`, telling the compositor at `compositor`
/// each time they change, and showing each change after the first on the
/// session's bus at `session`, until the kernel says no more. A machine
/// whose radios cannot be read has nothing to say of them.
pub(crate) fn follow(path: &Path, compositor: &Path, session: Option<&str>) {
    let Ok(mut said_by_kernel) = File::open(path) else {
        return;
    };
    let mut radios = Radios::default();
    let mut told: Option<Vec<Radio>> = None;
    let mut kept: Option<Client> = None;
    let mut bytes = [0; EVENT];
    while said_by_kernel.read_exact(&mut bytes).is_ok() {
        radios.heard(event(bytes));
        // The kernel says of every radio at once as it is first read, and
        // of all of a kind as one is turned: what it says together is
        // taken together.
        let mut waiting = [PollFd::new(&said_by_kernel, PollFlags::IN)];
        if poll(&mut waiting, Some(&Timespec::default())).unwrap_or(0) > 0 {
            continue;
        }
        let now = radios.kinds();
        if told.as_ref() == Some(&now) {
            continue;
        }
        let shown = told.as_deref().and_then(|before| said(before, &now));
        let bus = session.and_then(|session| crate::connect(session).ok());
        if let Some(((title, icon), bus)) = shown.zip(bus) {
            let _ = shown::said(&bus, &title, icon);
        }
        kept = tell(kept, compositor, now.clone());
        told = Some(now);
    }
}

/// Tells the compositor what the radios are, over the connection kept for
/// it, or over a new one if that has gone, as it goes when the compositor
/// starts again; and gives back the one that worked.
fn tell(kept: Option<Client>, compositor: &Path, radios: Vec<Radio>) -> Option<Client> {
    let asked = Request::Radios { radios };
    let told = |mut client: Client| {
        let answered = client.send(&asked).and_then(|()| client.receive());
        answered.ok().map(|_| client)
    };
    kept.and_then(told)
        .or_else(|| Client::connect(compositor).ok().and_then(told))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the kernel says of radio `radio` of kind `kind`: what happened,
    /// and whether a program or a switch has it off.
    pub(crate) fn kernel(
        radio: u32,
        kind: u8,
        happened: u8,
        (soft, hard): (bool, bool),
    ) -> [u8; 8] {
        let [a, b, c, d] = radio.to_ne_bytes();
        [a, b, c, d, kind, happened, u8::from(soft), u8::from(hard)]
    }

    fn radio(kind: &str, off: bool, switched: bool) -> Radio {
        Radio {
            kind: kind.to_owned(),
            off,
            switched,
        }
    }

    #[test]
    fn each_kind_the_kernel_numbers_has_its_name_and_others_none() {
        let names: Vec<_> = (1..=8).map(kind).collect();
        assert!(names.iter().all(Option::is_some), "{names:?}");
        assert_eq!(names[0], Some("wlan"));
        assert_eq!(kind(0), None);
        assert_eq!(kind(99), None);
    }

    #[test]
    fn the_kernels_bytes_say_which_radio_what_happened_and_how_it_is_held() {
        assert_eq!(
            event(kernel(3, 2, CHANGED, (true, false))),
            Event {
                radio: 3,
                kind: 2,
                happened: CHANGED,
                soft: true,
                hard: false,
            }
        );
    }

    #[test]
    fn a_kind_is_off_when_all_of_it_is_and_held_so_when_any_switch_holds_it() {
        let mut radios = Radios::default();
        for said in [
            kernel(0, 1, ADDED, (false, false)),
            kernel(1, 2, ADDED, (true, false)),
            kernel(2, 2, ADDED, (false, true)),
            kernel(3, 99, ADDED, (true, true)),
        ] {
            radios.heard(event(said));
        }
        assert_eq!(
            radios.kinds(),
            [radio("wlan", false, false), radio("bluetooth", true, true)]
        );

        radios.heard(event(kernel(0, 1, CHANGED, (true, false))));
        radios.heard(event(kernel(2, 2, REMOVED, (false, false))));
        radios.heard(event(kernel(9, 2, 3, (false, false))));
        assert_eq!(
            radios.kinds(),
            [radio("wlan", true, false), radio("bluetooth", true, false)]
        );
    }

    #[test]
    fn a_change_is_said_in_a_line_and_all_off_is_aeroplane_mode() {
        let on = [
            radio("wlan", false, false),
            radio("bluetooth", false, false),
        ];
        let bluetooth_off = [radio("wlan", false, false), radio("bluetooth", true, false)];
        let all_off = [radio("wlan", true, false), radio("bluetooth", true, true)];
        let said_of =
            |before: &[Radio], after: &[Radio]| said(before, after).map(|(title, _)| title);
        assert_eq!(
            said_of(&on, &bluetooth_off).as_deref(),
            Some("Bluetooth off")
        );
        assert_eq!(
            said_of(&bluetooth_off, &on).as_deref(),
            Some("Bluetooth on")
        );
        assert_eq!(
            said_of(&bluetooth_off, &all_off).as_deref(),
            Some("Aeroplane mode on")
        );
        assert_eq!(
            said_of(&all_off, &on).as_deref(),
            Some("Aeroplane mode off")
        );
        assert_eq!(said_of(&on, &on), None, "nothing changed");
        assert_eq!(said_of(&[], &[]), None);
        let held = [radio("wlan", false, true), radio("bluetooth", false, false)];
        assert_eq!(
            said_of(&on, &held),
            None,
            "a switch alone turns nothing off"
        );
    }

    #[test]
    fn a_kind_without_a_name_to_people_is_a_radio() {
        // Not one the kernel numbers, but said of all the same.
        let odd = [radio("odd", false, false), radio("wlan", false, false)];
        let off = [radio("odd", true, false), radio("wlan", false, false)];
        assert_eq!(
            said(&odd, &off).map(|(title, _)| title).as_deref(),
            Some("A radio off")
        );
    }

    /// Waits for `condition`, for as long as a test can.
    fn eventually(condition: impl Fn() -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !condition() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(condition(), "never came about");
    }

    #[test]
    fn the_radios_are_told_to_the_compositor_and_each_change_after_is_shown() {
        use std::io::Write;

        use crate::stand_in::{self, Compositor, Daemon};

        let directory = tempfile::TempDir::new().unwrap();
        let rfkill = directory.path().join("rfkill");
        let made = std::process::Command::new("mkfifo")
            .arg(&rfkill)
            .status()
            .unwrap();
        assert!(made.success());
        let daemon = Daemon::start();
        let (_notifications, shown) = stand_in::notifications(&daemon);
        let compositor = Compositor::start();
        let (path, at, bus) = (
            rfkill.clone(),
            compositor.path.clone(),
            daemon.address.clone(),
        );
        let following = std::thread::spawn(move || follow(&path, &at, Some(&bus)));

        // The kernel says of every radio there is as it is first read.
        let mut kernel_says = std::fs::OpenOptions::new()
            .write(true)
            .open(&rfkill)
            .unwrap();
        let all = [
            kernel(0, 1, ADDED, (false, false)),
            kernel(1, 2, ADDED, (false, false)),
        ];
        kernel_says.write_all(&all.concat()).unwrap();
        let asked = std::sync::Arc::clone(&compositor.asked);
        let told = |count: usize| {
            let asked = std::sync::Arc::clone(&asked);
            move || asked.lock().unwrap().len() == count
        };
        eventually(told(1));
        let first = compositor.asked.lock().unwrap()[0].clone();
        assert!(
            first.contains("\"wlan\"") && first.contains("\"bluetooth\""),
            "{first}"
        );
        assert!(
            shown.lock().unwrap().is_empty(),
            "what is there at first is not shown"
        );

        // A key turns Bluetooth off.
        kernel_says
            .write_all(&kernel(1, 2, CHANGED, (true, false)))
            .unwrap();
        eventually(|| shown.lock().unwrap().len() == 1);
        let heard = shown.lock().unwrap()[0].clone();
        assert_eq!(
            (heard.1.as_str(), heard.3.as_str()),
            ("Bluetooth off", "radios")
        );
        eventually(told(2));

        // Said again as it is, nothing changes and nothing is said; given
        // time to be read before the kernel says no more.
        kernel_says
            .write_all(&kernel(1, 2, CHANGED, (true, false)))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(200));
        drop(kernel_says);
        following.join().unwrap();
        assert_eq!(compositor.asked.lock().unwrap().len(), 2);
    }

    #[test]
    fn radios_that_cannot_be_read_say_nothing() {
        follow(
            Path::new("/nowhere/rfkill"),
            Path::new("/nowhere/ipc"),
            None,
        );
        assert!(tell(None, Path::new("/nowhere/ipc"), Vec::new()).is_none());
    }
}
