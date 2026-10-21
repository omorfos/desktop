//! The brightness of the screen, read from its backlight and set by
//! logind, which lets the user who is logged in at the machine set it
//! without being able to write to the backlight themselves.

use std::fs;
use std::path::Path;

use zbus::blocking::Connection;

use crate::{Failure, Level, Step};

/// How much of the whole one press of a key changes the brightness, in
/// hundredths.
const STEP: u32 = 5;

/// What a backlight is, in the order one is preferred to another when a
/// machine has several: one the firmware drives knows the screen best,
/// one the graphics card drives directly the least.
const KINDS: [&str; 3] = ["firmware", "platform", "raw"];

/// The backlight of a screen, as it is now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlight {
    /// What the kernel calls it, which is how logind is told which.
    pub name: String,
    /// How bright it is, out of `most`.
    pub now: u32,
    /// As bright as it goes.
    pub most: u32,
}

/// How bright a backlight is to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Changed {
    pub(crate) value: u32,
    most: u32,
}

impl Backlight {
    /// The backlight to change among those in `directory`, as the kernel
    /// lists them: of the kind preferred, and of those the first by name.
    ///
    /// # Errors
    ///
    /// If there is none whose brightness can be read, as on a machine
    /// whose screens have no backlight the kernel can set.
    pub fn find(directory: &Path) -> Result<Self, Failure> {
        let entries = fs::read_dir(directory).into_iter().flatten().flatten();
        let mut found: Vec<(usize, Self)> = entries
            .filter_map(|entry| {
                let path = entry.path();
                let read = |name: &str| fs::read_to_string(path.join(name)).ok();
                let number = |name: &str| read(name).and_then(|it| it.trim().parse::<u32>().ok());
                let kind = read("type").map(|kind| kind.trim().to_owned());
                let rank = KINDS
                    .iter()
                    .position(|known| Some(*known) == kind.as_deref());
                let name = entry.file_name().into_string().ok();
                let most = number("max_brightness").filter(|most| *most > 0);
                let backlight = name.zip(number("brightness")).zip(most);
                let backlight = backlight.map(|((name, now), most)| Self { name, now, most });
                backlight.map(|backlight| (rank.unwrap_or(KINDS.len()), backlight))
            })
            .collect();
        found.sort_by(|(rank, one), (other_rank, other)| {
            (rank, &one.name).cmp(&(other_rank, &other.name))
        });
        let first = found.into_iter().next().map(|(_, backlight)| backlight);
        first.ok_or_else(|| "there is no screen whose brightness can be set".into())
    }

    /// How bright it is to be after a step. A step is a share of the whole,
    /// and never nothing, so that a backlight with few levels still moves;
    /// and it never goes quite dark, which on some screens is off.
    #[must_use]
    pub(crate) fn stepped(&self, step: Step) -> Changed {
        let by = (u64::from(self.most) * u64::from(STEP)).div_ceil(100);
        // A twentieth of what a u32 holds fits in one.
        let by = u32::try_from(by).unwrap_or(u32::MAX).max(1);
        let value = match step {
            Step::Up => self.now.saturating_add(by),
            Step::Down => self.now.saturating_sub(by),
        };
        Changed {
            value: value.clamp(1, self.most),
            most: self.most,
        }
    }
}

impl Changed {
    /// What is shown of it.
    pub(crate) fn shown(self) -> Level {
        let value = u64::from(self.value) * 100 / u64::from(self.most);
        Level {
            tag: "brightness",
            title: "Brightness",
            icon: "display-brightness",
            value: u8::try_from(value.min(100)).unwrap_or(100),
        }
    }
}

/// Asks logind, on the system's bus, to set `backlight` as `changed` has
/// it, for the session this runs in.
pub(crate) fn set(
    system: &Connection,
    backlight: &Backlight,
    changed: Changed,
) -> Result<(), Failure> {
    system.call_method(
        Some("org.freedesktop.login1"),
        "/org/freedesktop/login1/session/auto",
        Some("org.freedesktop.login1.Session"),
        "SetBrightness",
        &("backlight", backlight.name.as_str(), changed.value),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stand_in::backlights;

    fn backlight(now: u32, most: u32) -> Backlight {
        Backlight {
            name: "b".to_owned(),
            now,
            most,
        }
    }

    #[test]
    fn the_backlight_of_the_kind_preferred_is_found_and_then_the_first_by_name() {
        let found = |each: &[(&str, &str, &str, &str)]| {
            let directory = backlights(each);
            Backlight::find(directory.path())
                .map(|found| found.name)
                .ok()
        };
        let raw = ("intel_backlight", "raw", "5", "10");
        let firmware = ("acpi_video1", "firmware", "5", "10");
        let platform = ("thinkpad", "platform", "5", "10");
        let unknown = ("odd", "unheard-of", "5", "10");
        assert_eq!(
            found(&[raw, platform, firmware]).as_deref(),
            Some("acpi_video1")
        );
        assert_eq!(found(&[raw, platform]).as_deref(), Some("thinkpad"));
        assert_eq!(found(&[unknown, raw]).as_deref(), Some("intel_backlight"));
        let first = ("acpi_video0", "firmware", "5", "10");
        assert_eq!(found(&[firmware, first]).as_deref(), Some("acpi_video0"));
        // One of no kind still is one.
        assert_eq!(found(&[("plain", "", "5", "10")]).as_deref(), Some("plain"));
    }

    #[test]
    fn a_backlight_that_cannot_be_read_or_set_is_passed_over() {
        let found = |each: &[(&str, &str, &str, &str)]| {
            let directory = backlights(each);
            Backlight::find(directory.path()).ok()
        };
        let good = ("good", "raw", "5", "10");
        for bad in [
            ("unread", "firmware", "", "10"),
            ("unbounded", "firmware", "5", ""),
            ("nothing", "firmware", "5", "0"),
            ("garbled", "firmware", "bright", "10"),
        ] {
            assert_eq!(
                found(&[bad, good]),
                Some(backlight_named("good", 5, 10)),
                "{bad:?}"
            );
            assert_eq!(found(&[bad]), None);
        }
        assert_eq!(Backlight::find(Path::new("/nowhere")).ok(), None);
    }

    fn backlight_named(name: &str, now: u32, most: u32) -> Backlight {
        Backlight {
            name: name.to_owned(),
            now,
            most,
        }
    }

    #[test]
    fn a_step_is_a_share_of_the_whole_and_never_nothing_or_quite_dark() {
        let value = |backlight: Backlight, step| backlight.stepped(step).value;
        assert_eq!(value(backlight(480, 960), Step::Up), 528);
        assert_eq!(value(backlight(480, 960), Step::Down), 432);
        assert_eq!(
            value(backlight(950, 960), Step::Up),
            960,
            "no brighter than it goes"
        );
        assert_eq!(value(backlight(20, 960), Step::Down), 1, "never quite dark");
        assert_eq!(
            value(backlight(3, 7), Step::Up),
            4,
            "a few levels still move"
        );
        assert_eq!(value(backlight(1, 7), Step::Down), 1);
        assert_eq!(value(backlight(5, u32::MAX), Step::Up), 5 + 214_748_365);
    }

    #[test]
    fn what_is_shown_is_how_bright_out_of_100() {
        let shown = |now, most| backlight(now, most).stepped(Step::Up).shown().value;
        assert_eq!(shown(480, 960), 55);
        assert_eq!(shown(6, 7), 100);
        assert_eq!(shown(0, 1000), 5);
    }
}
