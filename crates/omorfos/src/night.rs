//! Night light as the compositor keeps it: the temperature the screens are
//! to be at, asked of the configuration's schedule as it falls due, and
//! switched by hand until the schedule next turns.
//!
//! Each change fades in over [`FADE`], rather than jumping at the user:
//! one by hand, the configuration's, and each step of the schedule's
//! turning. The first a compositor is asked for is there at once, as one
//! started again by night finds the screens warm already, and fading in
//! from daylight would flash them white.

use std::time::{Duration, Instant, SystemTime};

use jiff::Zoned;
use omorfos_config::{DAYLIGHT, Easing, between};

use crate::compositor::Compositor;

/// How long a change of temperature takes to fade in.
const FADE: Duration = Duration::from_secs(2);

/// Night light, as it stands.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Night {
    /// The temperature the screens are to be at, in kelvin.
    kelvin: u16,
    /// Where the screens were on their way from, and when they set out, by
    /// the compositor's clock; nothing while no change has faded in yet.
    fade: Option<(u16, Duration)>,
    /// Whether the schedule has been asked yet.
    asked: bool,
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
            fade: None,
            asked: false,
            due: None,
            switched: None,
        }
    }
}

impl Compositor {
    /// The temperature the screens are at now, in kelvin: as they are, at
    /// 6500, unless night light makes them warmer, and on the way between
    /// two while a change fades in.
    #[must_use]
    pub fn night_kelvin(&self) -> u16 {
        let Some((from, began)) = self.night.fade else {
            return self.night.kelvin;
        };
        let gone = self.clock().saturating_sub(began);
        let share = (gone.as_secs_f32() / FADE.as_secs_f32()).min(1.0);
        between(from, self.night.kelvin, Easing::EaseInOut.at(share))
    }

    /// Whether a change of temperature is still fading in, and the screens
    /// are to be tinted again a frame on.
    pub(crate) fn night_fading(&self) -> bool {
        let fading = |(_, began): (u16, Duration)| self.clock() < began + FADE;
        self.night.fade.is_some_and(fading)
    }

    /// Has the screens go to `kelvin`, fading in from where they are, but
    /// the first time, when they are there at once.
    fn warm_to(&mut self, kelvin: u16) {
        if self.night.asked && kelvin != self.night.kelvin {
            self.night.fade = Some((self.night_kelvin(), self.clock()));
        }
        self.night.asked = true;
        self.night.kelvin = kelvin;
    }

    /// Asks night light's schedule how warm the screens are to be at `now`,
    /// as it falls due, and as the configuration changes. A switch by hand
    /// lasts until the schedule turns the screens warm, or back.
    pub fn light_the_night(&mut self, now: &Zoned) {
        let general = &self.config().general;
        let (scheduled, due) = general.night_light.at(&general.appearance, now);
        let warm = scheduled < DAYLIGHT;
        let switched = self.night.switched.filter(|(_, was)| *was == warm);
        self.warm_to(switched.map_or(scheduled, |(kelvin, _)| kelvin));
        self.night.due = due;
        self.night.switched = switched;
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
        self.warm_to(kelvin);
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
