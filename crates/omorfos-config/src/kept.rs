//! What the session keeps of the screens it has seen, between logins
//! (ADR 104): the scale it guessed for each, at each size it was driven
//! at, and the way the display key last kept them standing in. It is the
//! session's to write, in `$XDG_STATE_HOME/omorfos/screens.yaml`, and the
//! user's files are laid over it.
//!
//! The file is read as anything is that the user could have written: what
//! is not as it should be is not taken.

use std::fmt::Write as _;

use serde::Deserialize;

/// What the session keeps of the screens.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct KeptScreens {
    /// The scale guessed for each screen, at each size it was driven at.
    pub scales: Vec<KeptScale>,
    /// The way the display key last kept the screens standing in.
    pub way: Option<KeptWay>,
}

/// The scale guessed for a screen at a size.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, try_from = "RawScale")]
pub struct KeptScale {
    /// The screen, as what it says it is.
    pub screen: String,
    /// The size it was driven at, in pixels.
    pub size: (u32, u32),
    /// The scale, from 1 to 4 in quarters.
    pub scale: f64,
}

/// A kept scale as written.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScale {
    screen: String,
    size: String,
    scale: f64,
}

impl TryFrom<RawScale> for KeptScale {
    type Error = String;

    fn try_from(raw: RawScale) -> Result<Self, Self::Error> {
        let size = raw
            .size
            .split_once('x')
            .and_then(|(across, down)| Some((across.parse().ok()?, down.parse().ok()?)));
        let quarters = raw.scale * 4.0;
        let whole = quarters.fract() == 0.0 && (4.0..=16.0).contains(&quarters);
        match size {
            Some(size) if whole => Ok(Self {
                screen: raw.screen,
                size,
                scale: raw.scale,
            }),
            Some(_) => Err(format!(
                "a kept scale of {} is not 1 to 4 in quarters",
                raw.scale
            )),
            None => Err(format!("a kept size of `{}` is not as 3840x2160", raw.size)),
        }
    }
}

/// The way the display key kept the screens standing in.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeptWay {
    /// How they stand.
    pub stands: Stands,
    /// The screens in the order the display key goes through them, by the
    /// names the configuration knows them by.
    pub order: Vec<String>,
    /// The screens it is kept for, by those names, in order.
    pub names: Vec<String>,
}

/// How the screens stand, as the display key had them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stands {
    /// All showing what the one that comes first shows.
    Mirrored,
    /// The screen of this name alone, the others off.
    Alone(String),
}

impl KeptScreens {
    /// What `text` keeps; what it says otherwise than it should, why not.
    ///
    /// # Errors
    ///
    /// Returns why `text` is not as the session writes it.
    pub fn parse(text: &str) -> Result<Self, String> {
        let parsed = serde_saphyr::from_str::<Option<Self>>(text);
        parsed
            .map(Option::unwrap_or_default)
            .map_err(|error| error.to_string())
    }

    /// The scale kept for `screen` driven at `size`, if one is.
    #[must_use]
    pub fn scale(&self, screen: &str, size: (u32, u32)) -> Option<f64> {
        let kept = self
            .scales
            .iter()
            .find(|kept| kept.screen == screen && kept.size == size);
        kept.map(|kept| kept.scale)
    }

    /// Keeps `scale` for `screen` driven at `size`, in place of any kept.
    pub fn keep_scale(&mut self, screen: &str, size: (u32, u32), scale: f64) {
        self.scales
            .retain(|kept| !(kept.screen == screen && kept.size == size));
        self.scales.push(KeptScale {
            screen: screen.to_owned(),
            size,
            scale,
        });
    }

    /// Forgets every scale kept for `screen`, at any size.
    pub fn forget_scales(&mut self, screen: &str) {
        self.scales.retain(|kept| kept.screen != screen);
    }

    /// It, written as the session keeps it, which [`KeptScreens::parse`]
    /// reads back.
    #[must_use]
    pub fn to_text(&self) -> String {
        let mut text = String::from(
            "# What omorfos keeps of the screens it has seen: the scale it guessed for each,\n\
             # at each size it was driven at, and the way the display key last kept them in.\n\
             # Deleted, it starts again; config.yaml is laid over it.\n",
        );
        text.push_str("scales:");
        if self.scales.is_empty() {
            text.push_str(" []");
        }
        text.push('\n');
        for kept in &self.scales {
            let (across, down) = kept.size;
            // Writing to a string does not fail.
            let _ = writeln!(
                text,
                "  - {{ screen: {}, size: {across}x{down}, scale: {} }}",
                quoted(&kept.screen),
                kept.scale
            );
        }
        if let Some(way) = &self.way {
            let stands = match &way.stands {
                Stands::Mirrored => "mirrored".to_owned(),
                Stands::Alone(alone) => format!("{{ alone: {} }}", quoted(alone)),
            };
            let listed = |names: &[String]| {
                let quoted: Vec<String> = names.iter().map(|name| quoted(name)).collect();
                format!("[{}]", quoted.join(", "))
            };
            let _ = writeln!(
                text,
                "way:\n  stands: {stands}\n  order: {}\n  names: {}",
                listed(&way.order),
                listed(&way.names)
            );
        }
        text
    }
}

/// `text` as a string YAML reads back as it is: in double quotes, with a
/// quote, a backslash and what cannot be shown escaped.
fn quoted(text: &str) -> String {
    let mut quoted = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            shown if !shown.is_control() => quoted.push(shown),
            other => {
                let _ = write!(quoted, "\\u{:04x}", u32::from(other));
            }
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_kept_is_read_back_as_it_was_written() {
        let mut kept = KeptScreens::default();
        kept.keep_scale("BOE 0x07c8 on eDP-1", (3840, 2160), 2.0);
        kept.keep_scale("DEL 0xa0e1 \"U2720Q\"\\x\u{7}", (2560, 1440), 1.25);
        kept.way = Some(KeptWay {
            stands: Stands::Alone("office".to_owned()),
            order: vec!["laptop".to_owned(), "office".to_owned()],
            names: vec!["laptop".to_owned(), "office".to_owned()],
        });
        let text = kept.to_text();
        assert_eq!(KeptScreens::parse(&text), Ok(kept.clone()), "{text}");
        kept.way = Some(KeptWay {
            stands: Stands::Mirrored,
            order: Vec::new(),
            names: Vec::new(),
        });
        assert_eq!(KeptScreens::parse(&kept.to_text()), Ok(kept));
        let none = KeptScreens::default();
        assert_eq!(KeptScreens::parse(&none.to_text()), Ok(none.clone()));
        assert_eq!(
            KeptScreens::parse(""),
            Ok(none),
            "an empty file keeps nothing"
        );
    }

    #[test]
    fn a_scale_is_kept_for_a_screen_at_a_size_until_it_is_forgotten() {
        let mut kept = KeptScreens::default();
        kept.keep_scale("a", (3840, 2160), 2.5);
        kept.keep_scale("a", (3840, 2160), 2.0);
        kept.keep_scale("a", (1920, 1080), 1.0);
        kept.keep_scale("b", (3840, 2160), 1.5);
        assert_eq!(
            kept.scale("a", (3840, 2160)),
            Some(2.0),
            "the later in place"
        );
        assert_eq!(kept.scale("a", (1920, 1080)), Some(1.0));
        assert_eq!(kept.scale("a", (2560, 1440)), None);
        kept.forget_scales("a");
        assert_eq!(kept.scale("a", (1920, 1080)), None);
        assert_eq!(kept.scale("b", (3840, 2160)), Some(1.5));
    }

    #[test]
    fn what_is_not_as_the_session_writes_it_is_not_taken() {
        let wrong = |text: &str| KeptScreens::parse(text).unwrap_err();
        let scale = |size: &str, scale: &str| {
            format!("scales:\n  - {{ screen: a, size: {size}, scale: {scale} }}\n")
        };
        assert!(wrong(&scale("3840x2160", "2.1")).contains("1 to 4 in quarters"));
        assert!(wrong(&scale("3840x2160", "5")).contains("1 to 4 in quarters"));
        assert!(wrong(&scale("3840", "2")).contains("not as 3840x2160"));
        assert!(wrong(&scale("axb", "2")).contains("not as 3840x2160"));
        assert!(wrong(&scale("3840xb", "2")).contains("not as 3840x2160"));
        assert!(wrong("sizes: []\n").contains("unknown field"));
    }
}
