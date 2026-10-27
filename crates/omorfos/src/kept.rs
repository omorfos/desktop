//! Where windows are, and where the session is, as the compositor hands
//! the relay to keep for the next compositor, and takes back from it (ADR
//! 61, step 3c).
//!
//! The relay keeps what it is handed as text it does not read, so its
//! form is the compositor's alone. Outputs are named, as their order may
//! differ in the next compositor; a name with a space in it cannot be
//! written, and what names it is not read, so that window comes back as a
//! new one would.

use std::collections::HashMap;
use std::os::fd::{AsFd, OwnedFd};

use omorfos_core::geometry::{Point, Rect, Size};
use omorfos_core::{Desk, Kept, Standing, States};
use omorfos_ipc::relay::{self, Hand};
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::shell::xdg::ToplevelSurface;

use crate::compositor::{ClientState, Compositor};

/// What is handed to the relay to be kept as programs are, and what it
/// hands back (ADR 61).
#[derive(Debug, Default)]
pub(crate) struct Parts {
    /// The connection to the relay, where the compositor is relayed.
    pub(crate) relay: Option<OwnedFd>,
    /// Whether the relay has handed over every program it kept.
    pub(crate) kept: bool,
    /// Where each window kept was, as the relay handed it back, by its
    /// program's number and the program's number for its toplevel, until it
    /// is shown again.
    places: HashMap<(u32, u32), String>,
    /// Where the relay has been told each window is, by the same.
    told: HashMap<(u32, u32), Kept>,
    /// Where the relay has been told the session is, and the pointer.
    desk: Option<(Desk, (i32, i32))>,
}

impl Compositor {
    /// Takes in where the window of the program numbered `number` whose
    /// toplevel is its `toplevel` was, as the relay handed it back, `place`,
    /// for it to come back there.
    pub(crate) fn kept_place(&mut self, number: u32, toplevel: u32, place: String) {
        self.autostarted
            .parts
            .places
            .insert((number, toplevel), place);
    }

    /// Takes in where the session was, as the relay handed it back: what
    /// each output showed, which had the keyboard, and where the pointer
    /// was. Outputs no longer there are passed over.
    pub(crate) fn relay_desk(&mut self, text: &str) {
        let _ = read_desk(text).map(|said| self.desk_back(said));
    }

    /// Has the session be where `said` says it was.
    fn desk_back(&mut self, said: Said<'_>) {
        let Said {
            shown,
            current,
            pointer,
        } = said;
        let mut desk = self.desktop.desk();
        let named = |name: &str| {
            self.heads
                .iter()
                .position(|head| head.output.name() == name)
        };
        for (name, workspace) in shown {
            let at = named(name).and_then(|at| desk.shown.get_mut(at));
            let _ = at.map(|at| *at = workspace);
        }
        desk.current = named(current).unwrap_or(desk.current);
        self.desktop.set_desk(&desk);
        self.pointer_location = (f64::from(pointer.0), f64::from(pointer.1)).into();
        // Held to where there is a screen.
        self.pointer_location = self.moved_by(0.0, 0.0);
    }

    /// Where the window the relay keeps by `key` was, if it was kept
    /// through a change of compositor and the relay said: on the output of
    /// that name, or the current one if there is none.
    pub(crate) fn kept_window(&mut self, key: Option<(u32, u32)>) -> Option<Kept> {
        let places = &mut self.autostarted.parts.places;
        let place = key.and_then(|key| places.remove(&key));
        let (mut kept, output) = place.and_then(|place| read_place(&place))?;
        let named = self
            .heads
            .iter()
            .position(|head| head.output.name() == output);
        kept.output = named.unwrap_or(self.desktop.current_output());
        Some(kept)
    }

    /// Tells the relay where each window of a program it keeps is, and
    /// where the session is, as far as either changed since it was last
    /// told.
    pub(crate) fn tell_relay(&mut self) {
        let Parts {
            relay,
            told,
            desk: told_desk,
            ..
        } = &mut self.autostarted.parts;
        let Some(relay) = relay.as_ref() else {
            return;
        };
        let name = |at: usize| self.heads.get(at).map(|head| head.output.name());
        for mapped in &self.windows {
            let kept = self.desktop.kept(mapped.id);
            let Some(((number, toplevel), kept)) = mapped.kept_by.zip(kept) else {
                continue;
            };
            if told.get(&(number, toplevel)) == Some(&kept) {
                continue;
            }
            let output = name(kept.output).unwrap_or_default();
            let place = place(&kept, &output);
            let hand = Hand::Placed {
                number,
                toplevel,
                place,
            };
            // A relay that cannot be told has gone, and the compositor with it.
            let _ = relay::send(relay.as_fd(), &hand, &[]);
            told.insert((number, toplevel), kept);
        }
        let shown = self.desktop.desk();
        let at = self.pointer_location;
        // Truncation is meant: a pixel's worth matters to nobody here.
        #[expect(clippy::cast_possible_truncation, reason = "a position on a screen")]
        let pointer = (at.x as i32, at.y as i32);
        let now = Some((shown, pointer));
        if *told_desk != now {
            let names: Vec<String> = self.heads.iter().map(|head| head.output.name()).collect();
            let said = now
                .as_ref()
                .map(|(shown, pointer)| desk(shown, *pointer, &names));
            let hand = said.map(Hand::Desk);
            let _ = hand.map(|hand| relay::send(relay.as_fd(), &hand, &[]));
            *told_desk = now;
        }
    }
}

/// What a window is kept by: the number the relay gave its program, and
/// the program's number for its toplevel, which the relay keeps across
/// compositors; none for one not relayed.
pub(crate) fn key(toplevel: &ToplevelSurface) -> Option<(u32, u32)> {
    let client = toplevel.wl_surface().client();
    let relayed = client.and_then(|client| {
        client
            .get_data::<ClientState>()
            .and_then(|state| state.relayed)
    });
    let number = relayed.and_then(|program| program.number);
    number.map(|number| (number, toplevel.xdg_toplevel().id().protocol_id()))
}

/// The letters that say a kept window's states and standing, in the order
/// written; `-` for none.
const FLAGS: [char; 7] = ['m', 'f', 'n', 's', 'a', 'k', 'c'];

/// How the relay keeps where `kept` is, it being on the output named
/// `output`.
pub(crate) fn place(kept: &Kept, output: &str) -> String {
    let set = [
        kept.states.maximized,
        kept.states.fullscreen,
        kept.states.minimized,
        kept.standing.shaded,
        kept.standing.above,
        kept.standing.sticky,
        kept.focused,
    ];
    let flags: String = FLAGS
        .iter()
        .zip(set)
        .filter(|(_, set)| *set)
        .map(|(flag, _)| *flag)
        .collect();
    let flags = if flags.is_empty() {
        "-".to_owned()
    } else {
        flags
    };
    let Rect { origin, size } = kept.geometry;
    format!(
        "{} {} {} {} {} {flags} {} {output}",
        kept.desktop, origin.x, origin.y, size.width, size.height, kept.under
    )
}

/// Where a window is, as [`place`] wrote it, and the name of its output;
/// the output it says is the first, to be found by that name.
pub(crate) fn read_place(text: &str) -> Option<(Kept, String)> {
    let mut words = text.splitn(8, ' ');
    let mut number = || words.next()?.parse::<i64>().ok();
    let (desktop, x, y) = (number()?, number()?, number()?);
    let (width, height) = (number()?, number()?);
    let flags = words.next()?;
    let under = words.next()?.parse().ok()?;
    let output = words.next()?;
    let set = |flag: char| flags.contains(flag);
    let geometry = Rect::new(
        Point::new(i32::try_from(x).ok()?, i32::try_from(y).ok()?),
        Size::new(u32::try_from(width).ok()?, u32::try_from(height).ok()?),
    );
    let kept = Kept {
        output: 0,
        desktop: usize::try_from(desktop).ok()?,
        geometry,
        states: States {
            maximized: set('m'),
            fullscreen: set('f'),
            minimized: set('n'),
        },
        standing: Standing {
            shaded: set('s'),
            above: set('a'),
            sticky: set('k'),
        },
        under,
        focused: set('c'),
    };
    Some((kept, output.to_owned()))
}

/// How the relay keeps where the session is: `desk`, with the outputs it
/// counts named by `names`, and where the pointer is, `pointer`.
pub(crate) fn desk(desk: &Desk, pointer: (i32, i32), names: &[String]) -> String {
    let current = names.get(desk.current).map_or("", String::as_str);
    let shown = names.iter().zip(&desk.shown);
    let shown: Vec<String> = shown
        .map(|(name, shown)| format!("{name}:{shown}"))
        .collect();
    format!("{} {} {current} {}", pointer.0, pointer.1, shown.join(" "))
}

/// Where the session is, as [`desk`] wrote it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Said<'a> {
    /// The desktop each output shows, by its name.
    pub(crate) shown: Vec<(&'a str, usize)>,
    /// The name of the output with the keyboard.
    pub(crate) current: &'a str,
    /// Where the pointer is.
    pub(crate) pointer: (i32, i32),
}

/// Where the session is, as [`desk`] wrote it.
pub(crate) fn read_desk(text: &str) -> Option<Said<'_>> {
    let mut words = text.split(' ');
    let x = words.next().and_then(|word| word.parse().ok())?;
    let y = words.next().and_then(|word| word.parse().ok())?;
    let current = words.next()?;
    let shown = words.filter(|word| !word.is_empty()).map(|word| {
        let (name, shown) = word.rsplit_once(':')?;
        Some((name, shown.parse().ok()?))
    });
    let shown: Option<Vec<(&str, usize)>> = shown.collect();
    Some(Said {
        shown: shown?,
        current,
        pointer: (x, y),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept() -> Kept {
        Kept {
            output: 0,
            desktop: 2,
            geometry: Rect::new(Point::new(-10, 20), Size::new(640, 480)),
            states: States {
                maximized: true,
                fullscreen: false,
                minimized: true,
            },
            standing: Standing {
                shaded: false,
                above: true,
                sticky: false,
            },
            under: 3,
            focused: true,
        }
    }

    #[test]
    fn where_a_window_is_is_read_as_it_is_written() {
        let written = place(&kept(), "DP-1");
        assert_eq!(written, "2 -10 20 640 480 mnac 3 DP-1");
        assert_eq!(read_place(&written), Some((kept(), "DP-1".to_owned())));
        let plain = Kept {
            states: States::default(),
            standing: Standing::default(),
            focused: false,
            ..kept()
        };
        let written = place(&plain, "eDP-1");
        assert_eq!(written, "2 -10 20 640 480 - 3 eDP-1");
        assert_eq!(read_place(&written), Some((plain, "eDP-1".to_owned())));
    }

    #[test]
    fn what_is_not_a_place_is_not_read() {
        for text in [
            "",
            "x 0 0 1 1 - 0 a",
            "0 x 0 1 1 - 0 a",
            "0 0 x 1 1 - 0 a",
            "0 0 0 x 1 - 0 a",
            "0 0 0 1 x - 0 a",
            "0 0 0 1 1",
            "0 0 9999999999 1 1 - 0 a",
            "0 0 0 1 -1 - 0 a",
            "0 0 0 1 1 - 0",
            "0 0 0 1 1 -",
            "0 0 0 1",
            "-1 0 0 1 1 - 0 a",
            "0 9999999999 0 1 1 - 0 a",
            "0 0 0 -1 1 - 0 a",
            "0 0 0 1 1 - x a",
        ] {
            assert_eq!(read_place(text), None, "{text:?}");
        }
    }

    #[test]
    fn where_the_session_is_is_read_as_it_is_written() {
        let names = ["eDP-1".to_owned(), "DP-1".to_owned()];
        let shown = Desk {
            shown: vec![0, 2],
            current: 1,
        };
        let written = desk(&shown, (400, -3), &names);
        assert_eq!(written, "400 -3 DP-1 eDP-1:0 DP-1:2");
        let read = read_desk(&written);
        let said = Said {
            shown: vec![("eDP-1", 0), ("DP-1", 2)],
            current: "DP-1",
            pointer: (400, -3),
        };
        assert_eq!(read, Some(said));
        // With no output, there is nothing to name.
        let alone = desk(
            &Desk {
                shown: vec![0],
                current: 0,
            },
            (0, 0),
            &[],
        );
        let nothing = Said {
            shown: Vec::new(),
            current: "",
            pointer: (0, 0),
        };
        assert_eq!(read_desk(&alone), Some(nothing));
        for text in ["", "1", "1 2", "x 2 a", "1 x a", "1 2 a b", "1 2 a b:x"] {
            assert_eq!(read_desk(text), None, "{text:?}");
        }
    }
}
