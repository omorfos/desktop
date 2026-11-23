//! `screens` in `config.yaml`: screens known by what they say they are,
//! each by a name of the user's, which `outputs`, `arrangements` and
//! `mirror` take in place of a socket's (ADR 103).

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::Outputs;

/// Screens known by what they say they are, by names of the user's.
///
/// A name here stands, in `outputs`, `arrangements` and `mirror`, for the
/// first screen plugged in that says what it gives and that no other entry
/// before it has taken; any other name is a socket's. A name may not end
/// in a dash and a number, as sockets' names do.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[serde(try_from = "BTreeMap<String, ScreenMatch>")]
pub struct NamedScreens(BTreeMap<String, ScreenMatch>);

impl NamedScreens {
    /// What the screen called `name` here says it is, if it is here.
    #[must_use]
    pub fn named(&self, name: &str) -> Option<&ScreenMatch> {
        self.0.get(name)
    }

    /// Every screen here, by its name.
    pub fn each(&self) -> impl Iterator<Item = (&str, &ScreenMatch)> {
        self.0.iter().map(|(name, said)| (name.as_str(), said))
    }
}

impl TryFrom<BTreeMap<String, ScreenMatch>> for NamedScreens {
    type Error = String;

    fn try_from(screens: BTreeMap<String, ScreenMatch>) -> Result<Self, Self::Error> {
        let socket = screens.keys().find(|name| like_a_socket(name));
        if screens.keys().any(String::is_empty) {
            Err("a screen's name must not be empty".to_owned())
        } else if let Some(name) = socket {
            Err(format!(
                "screen `{name}` is named as a socket is, with a dash and a number at the end: \
                 another name tells it from one"
            ))
        } else {
            Ok(Self(screens))
        }
    }
}

/// Whether `name` is written as a socket's is: ending in a dash and a
/// number, as `eDP-1` and `HDMI-A-1`.
fn like_a_socket(name: &str) -> bool {
    name.rsplit_once('-')
        .is_some_and(|(_, number)| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

/// What a screen says it is, all of which it is to say to be taken: as
/// `omorfosctl screens` shows it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(try_from = "Said", deny_unknown_fields)]
pub struct ScreenMatch(Said);

/// What a screen is to say, as written.
#[derive(Debug, Clone, PartialEq, Eq, Default, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(default, deny_unknown_fields)]
pub struct Said {
    /// Who made it, by the three capital letters it is registered by, such
    /// as `DEL`.
    pub maker: Option<String>,
    /// The maker's number for it, such as `0x07c8`.
    pub product: Option<u16>,
    /// Its name, such as `DELL U2720Q`.
    pub name: Option<String>,
    /// Its serial.
    pub serial: Option<String>,
}

impl TryFrom<Said> for ScreenMatch {
    type Error = String;

    fn try_from(said: Said) -> Result<Self, Self::Error> {
        let letters = said.maker.as_deref().is_none_or(|maker| {
            maker.len() == 3 && maker.bytes().all(|letter| letter.is_ascii_uppercase())
        });
        if said == Said::default() {
            Err(
                "a screen is to say at least one of `maker`, `product`, `name` and `serial`"
                    .to_owned(),
            )
        } else if !letters {
            Err("a screen's `maker` is three capital letters, such as `DEL`".to_owned())
        } else {
            Ok(Self(said))
        }
    }
}

impl ScreenMatch {
    /// Whether a screen that says `it` is this one: it says all this does.
    #[must_use]
    pub fn matches(&self, it: &Facts<'_>) -> bool {
        let Said {
            maker,
            product,
            name,
            serial,
        } = &self.0;
        let same = |wanted: &Option<String>, said: Option<&str>| {
            wanted.as_deref().is_none_or(|wanted| said == Some(wanted))
        };
        same(maker, Some(it.maker))
            && product.is_none_or(|product| product == it.product)
            && same(name, it.name)
            && same(serial, it.serial)
    }

    /// What it is to say, as written.
    #[must_use]
    pub const fn said(&self) -> &Said {
        &self.0
    }
}

/// What a screen plugged in says it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts<'a> {
    /// Who made it, as three letters.
    pub maker: &'a str,
    /// The maker's number for it.
    pub product: u16,
    /// Its name, if it says one.
    pub name: Option<&'a str>,
    /// Its serial, if it says one.
    pub serial: Option<&'a str>,
}

impl Outputs {
    /// The name each of `plugged`, a socket's name and what the screen there
    /// says it is, if it says, goes by in this list with `screens`: that of
    /// the first entry naming a screen in `screens` that it is, and that no
    /// entry before has taken; or else its socket's.
    #[must_use]
    pub fn known_as(
        &self,
        screens: &NamedScreens,
        plugged: &[(&str, Option<Facts<'_>>)],
    ) -> Vec<String> {
        let mut known: Vec<String> = plugged
            .iter()
            .map(|(socket, _)| (*socket).to_owned())
            .collect();
        // A socket this lists by its own name is that one's, whatever it is.
        let listed = self.listed();
        let by_socket = |socket: &str| {
            listed
                .iter()
                .any(|output| output.name == socket && screens.named(socket).is_none())
        };
        let mut taken: Vec<bool> = plugged
            .iter()
            .map(|(socket, _)| by_socket(socket))
            .collect();
        let wanted = listed.iter().filter_map(|output| {
            screens
                .named(&output.name)
                .map(|said| (output.name.as_str(), said))
        });
        for (name, said) in wanted {
            let found = plugged.iter().zip(&taken).position(|((_, facts), taken)| {
                !taken && facts.as_ref().is_some_and(|facts| said.matches(facts))
            });
            if let Some(index) = found {
                name.clone_into(&mut known[index]);
                taken[index] = true;
            }
        }
        known
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NamedOutput;

    fn screens(yaml: &str) -> Result<NamedScreens, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.to_string())
    }

    fn listed(names: &[&str]) -> Outputs {
        let yaml = names
            .iter()
            .fold(String::new(), |yaml, name| yaml + "- name: " + name + "\n");
        serde_saphyr::from_str::<Vec<NamedOutput>>(&yaml)
            .ok()
            .and_then(|outputs| Outputs::try_from(outputs).ok())
            .unwrap_or_default()
    }

    const DELL: Facts<'static> = Facts {
        maker: "DEL",
        product: 0xa0e1,
        name: Some("DELL U2720Q"),
        serial: Some("7X3KD13"),
    };
    const PANEL: Facts<'static> = Facts {
        maker: "BOE",
        product: 0x07c8,
        name: None,
        serial: None,
    };

    #[test]
    fn a_screen_is_known_by_any_of_what_it_says() {
        let known = screens(
            "laptop: { maker: BOE, product: 0x07c8 }\n\
             office: { maker: DEL, name: DELL U2720Q }\n\
             this: { serial: 7X3KD13 }\n",
        )
        .unwrap();
        let matches = |name: &str, facts| known.named(name).unwrap().matches(&facts);
        assert!(matches("laptop", PANEL));
        assert!(!matches("laptop", DELL));
        assert!(matches("office", DELL));
        assert!(!matches("office", PANEL), "says no name");
        assert!(matches("this", DELL));
        let other = Facts {
            serial: Some("other"),
            ..DELL
        };
        assert!(!matches("this", other));
        assert_eq!(known.each().count(), 3);
        assert_eq!(known.named("laptop").unwrap().said().product, Some(0x07c8));
    }

    #[test]
    fn a_screen_that_says_nothing_or_a_maker_not_of_three_letters_is_wrong() {
        let wrong = |yaml| screens(yaml).unwrap_err();
        assert!(wrong("office: {}\n").contains("at least one of"));
        assert!(wrong("office: { maker: Dell }\n").contains("three capital letters"));
        assert!(wrong("office: { maker: del }\n").contains("three capital letters"));
        assert!(wrong("office: { model: X }\n").contains("unknown field"));
    }

    #[test]
    fn a_screen_named_as_a_socket_is_or_not_at_all_is_wrong() {
        let wrong = |yaml| screens(yaml).unwrap_err();
        assert!(wrong("HDMI-A-1: { maker: DEL }\n").contains("as a socket is"));
        assert!(wrong("'': { maker: DEL }\n").contains("must not be empty"));
        // A dash with no number, or a number with no dash, is a name.
        assert!(
            screens("office-left: { maker: DEL }\nroom2: { maker: DEL }\nend-: { maker: DEL }\n")
                .is_ok()
        );
    }

    #[test]
    fn each_screen_plugged_in_is_known_by_the_first_entry_it_is_that_has_not_been_taken() {
        let known = screens(
            "left: { maker: DEL }\n\
             right: { maker: DEL }\n\
             laptop: { maker: BOE }\n",
        )
        .unwrap();
        let outputs = listed(&["laptop", "left", "right", "DP-3"]);
        let plugged = [
            ("DP-1", Some(DELL)),
            ("eDP-1", Some(PANEL)),
            ("DP-2", Some(DELL)),
            ("DP-3", Some(DELL)),
            ("HDMI-A-1", None),
        ];
        // Two alike, taken in the order they were plugged in; one listed by
        // its socket is that socket's, though it is alike too; one that says
        // nothing is known by its socket.
        assert_eq!(
            outputs.known_as(&known, &plugged),
            ["left", "laptop", "right", "DP-3", "HDMI-A-1"]
        );
        // With one fewer alike, the second goes unmatched.
        assert_eq!(outputs.known_as(&known, &plugged[..2]), ["left", "laptop"]);
    }
}
