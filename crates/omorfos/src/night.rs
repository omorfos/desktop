//! Night light as the compositor keeps it: the temperature the screens are
//! to be at, asked of the configuration's schedule as it falls due, and
//! switched by hand until the schedule next turns.

use std::time::{Instant, SystemTime};

use jiff::Zoned;
use omorfos_config::DAYLIGHT;

use crate::compositor::Compositor;

/// Night light, as it stands.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Night {
    /// The temperature the screens are to be at, in kelvin.
    kelvin: u16,
    /// When the schedule is to be asked again.
    due: Option<SystemTime>,
    /// Switched by hand: the temperature asked for, and whether the
    /// schedule had the screens warm then. It lasts until that changes.
    switched: Option<(u16, bool)>,
}

impl Default for Night {
    fn default() -> Self {
        Self {
            kelvin: DAYLIGHT,
            due: None,
            switched: None,
        }
    }
}

impl Compositor {
    /// The temperature the screens are to be at, in kelvin: as they are,
    /// at 6500, unless night light makes them warmer.
    #[must_use]
    pub fn night_kelvin(&self) -> u16 {
        self.night.kelvin
    }

    /// Asks night light's schedule how warm the screens are to be at `now`,
    /// as it falls due, and as the configuration changes. A switch by hand
    /// lasts until the schedule turns the screens warm, or back.
    pub fn light_the_night(&mut self, now: &Zoned) {
        let general = &self.config().general;
        let (scheduled, due) = general.night_light.at(&general.appearance, now);
        let warm = scheduled < DAYLIGHT;
        let switched = self.night.switched.filter(|(_, was)| *was == warm);
        self.night = Night {
            kelvin: switched.map_or(scheduled, |(kelvin, _)| kelvin),
            due,
            switched,
        };
    }

    /// Switches night light by hand, until the schedule next turns: as the
    /// screens are if they are warm now, and as warm as the configuration
    /// says if they are not.
    pub(crate) fn switch_night_light(&mut self) {
        let general = &self.config().general;
        let warmest = u16::try_from(general.night_light.temperature.get()).unwrap_or(DAYLIGHT);
        let kelvin = if self.night.kelvin < DAYLIGHT {
            DAYLIGHT
        } else {
            warmest
        };
        // What the schedule says now, which the switch lasts as long as.
        let warm = self
            .night
            .switched
            .map_or(self.night.kelvin < DAYLIGHT, |(_, warm)| warm);
        self.night.switched = Some((kelvin, warm));
        self.night.kelvin = kelvin;
    }

    /// Asks night light's schedule again if it has fallen due, by the
    /// clock on the wall at `wall`.
    pub fn light_the_night_if_due(&mut self, wall: SystemTime) {
        if self.night.due.is_some_and(|due| due <= wall) {
            self.light_the_night(&Zoned::now());
        }
    }

    /// When night light's schedule is next to be asked, by the clock the
    /// event loop waits on, from `now` and the clock on the wall at `wall`.
    #[must_use]
    pub fn night_due(&self, now: Instant, wall: SystemTime) -> Option<Instant> {
        let due = self.night.due?;
        Some(now + due.duration_since(wall).unwrap_or_default())
    }
}
