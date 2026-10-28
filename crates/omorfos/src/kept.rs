//! Where windows are, and where the session is, as the compositor hands
//! the relay to keep for the next compositor, and takes back from it (ADR
//! 61, step 3c).
//!
//! The relay keeps what it is handed as text it does not read, so its
//! form is the compositor's alone. Outputs are named, as their order may
//! differ in the next compositor; a name with a space in it cannot be
//! written, and what names it is not read, so that window comes back as a
//! new one would. A window's place begins `v2` from when it says the tab
//! and the part it has; one without is read as it was first written, by a
//! compositor before, as one of neither.

use std::collections::HashMap;
use std::os::fd::{AsFd, OwnedFd};

/// The parts of the session kept by the relay, as the desk names each: what
/// it is called, and its process.
type Named<'a> = Vec<(&'a str, u32)>;

use omorfos_core::geometry::{Point, Rect, Size};
use omorfos_core::{Desk, Kept, Standing, States, Tab, Tile};
use omorfos_ipc::relay::{self, Hand};
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::foreign_toplevel_list::ForeignToplevelHandle;
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
    /// Where the relay has been told the session is, the pointer, and the
    /// parts of it kept by the relay.
    desk: Option<(Desk, (i32, i32), Named<'static>)>,
    /// What each part kept by the relay was called by the compositor before,
    /// by its process, as the relay handed it back.
    pub(crate) known: HashMap<u32, String>,
    /// The parts kept from a compositor before, which are not this one's
    /// children, each with what says when it has ended, where it could be
    /// had.
    pub(crate) adopted: Vec<(u32, Option<OwnedFd>)>,
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
            parts,
        } = said;
        let known = parts
            .into_iter()
            .map(|(called, process)| (process, called.to_owned()));
        self.autostarted.parts.known = known.collect();
        let mut desk = self.desktop.desk();
        // What this compositor has made is not what the relay names.
        desk.made.fill(None);
        let named = |name: &str| {
            self.heads
                .iter()
                .position(|head| head.output.name() == name)
        };
        for (name, workspace, made) in shown {
            let at = named(name).filter(|at| *at < desk.shown.len());
            let _ = at.map(|at| (desk.shown[at], desk.made[at]) = (workspace, made));
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
        let parts = self.kept_parts();
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
        let now = Some((shown, pointer, parts));
        if *told_desk != now {
            let names: Vec<String> = self.heads.iter().map(|head| head.output.name()).collect();
            let said = now
                .as_ref()
                .map(|(shown, pointer, parts)| desk(shown, *pointer, &names, parts));
            let hand = said.map(Hand::Desk);
            let _ = hand.map(|hand| relay::send(relay.as_fd(), &hand, &[]));
            *told_desk = now;
        }
    }
}

/// Tells the relay, over `relay` where the compositor is relayed, that the
/// window kept by `window`, if it is relayed, is listed as `listed` says,
/// so that a program that listed it under the compositor before is told of
/// it once (ADR 61, step 3e). Each program it lists it for reads of it only
/// once this has been sent, as what the compositor says is sent at the end
/// of each turn.
pub(crate) fn listed(
    relay: Option<&OwnedFd>,
    listed: &ForeignToplevelHandle,
    window: Option<(u32, u32)>,
) {
    let identifier = listed.identifier();
    let hand = Hand::Listed { identifier, window };
    // A relay that cannot be told has gone, and the compositor with it.
    let _ = relay.map(|relay| relay::send(relay.as_fd(), &hand, &[]));
}

/// Tells the relay, over `relay` where the compositor is relayed, that the
/// window `listed` was is listed no longer.
pub(crate) fn unlisted(relay: Option<&OwnedFd>, listed: &ForeignToplevelHandle) {
    let hand = Hand::Unlisted(listed.identifier());
    let _ = relay.map(|relay| relay::send(relay.as_fd(), &hand, &[]));
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

/// The letters that say a kept window's states and standing, and whether it
/// is the tab its frame shows, in the order written; `-` for none.
const FLAGS: [char; 8] = ['m', 'f', 'n', 's', 'a', 'k', 'c', 't'];

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
        kept.tab.is_some_and(|tab| tab.shown),
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
    let tab = kept
        .tab
        .map_or_else(|| "-".to_owned(), |tab| format!("{}:{}", tab.frame, tab.at));
    let tile = kept.tile.map_or_else(|| "-".to_owned(), |tile| cut(&tile));
    format!(
        "v2 {} {} {} {} {} {flags} {} {tab} {tile} {output}",
        kept.desktop, origin.x, origin.y, size.width, size.height, kept.under
    )
}

/// How the relay keeps the part a window fills and how its workspace is
/// cut: what the workspace is known by, the part, `s` if its sides are one
/// above the other, its three splits, and which places are parts.
fn cut(tile: &Tile) -> String {
    let [first, second, third] = tile.splits;
    let stacked = if tile.stacked { "s" } else { "-" };
    let parts: String = tile
        .parts
        .iter()
        .map(|part| if *part { '1' } else { '0' })
        .collect();
    format!(
        "{}:{}:{stacked}:{first}:{second}:{third}:{parts}",
        tile.workspace, tile.at
    )
}

/// The part a window fills, as [`cut`] wrote it.
fn read_cut(text: &str) -> Option<Tile> {
    let mut words = text.split(':');
    let workspace = words.next().and_then(|word| word.parse().ok())?;
    let at = words.next()?.parse().ok().filter(|at| *at < 4)?;
    let stacked = words.next()? == "s";
    let mut split = || words.next()?.parse().ok();
    let splits = [split()?, split()?, split()?];
    let parts = words.next()?.as_bytes();
    let parts: [u8; 4] = parts.try_into().ok()?;
    Some(Tile {
        workspace,
        stacked,
        splits,
        parts: parts.map(|part| part == b'1'),
        at,
    })
}

/// The frame a window shares as a tab, as [`place`] wrote it, it being the
/// one shown if `shown`.
fn read_tab(text: &str, shown: bool) -> Option<Tab> {
    let (frame, at) = text.split_once(':')?;
    Some(Tab {
        frame: frame.parse().ok()?,
        at: at.parse().ok()?,
        shown,
    })
}

/// Where a window is, as [`place`] wrote it, and the name of its output;
/// the output it says is the first, to be found by that name.
pub(crate) fn read_place(text: &str) -> Option<(Kept, String)> {
    let (second, text) = text
        .strip_prefix("v2 ")
        .map_or((false, text), |text| (true, text));
    let mut words = text.splitn(if second { 10 } else { 8 }, ' ');
    let mut number = || words.next()?.parse::<i64>().ok();
    let (desktop, x, y) = (number()?, number()?, number()?);
    let (width, height) = (number()?, number()?);
    let flags = words.next()?;
    let under = words.next()?.parse().ok()?;
    let set = |flag: char| flags.contains(flag);
    let (tab, tile) = if second {
        let (tab, tile) = (words.next()?, words.next()?);
        let tab = if tab == "-" {
            None
        } else {
            Some(read_tab(tab, set('t'))?)
        };
        let tile = if tile == "-" {
            None
        } else {
            Some(read_cut(tile)?)
        };
        (tab, tile)
    } else {
        (None, None)
    };
    let output = words.next()?;
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
        tab,
        tile,
    };
    Some((kept, output.to_owned()))
}

/// How the relay keeps where the session is: `desk`, with the outputs it
/// counts named by `names`, where the pointer is, `pointer`, and the parts
/// of the session it keeps, `parts`. It begins `d2` from when it says the
/// parts; one without, written before, is read as saying none.
pub(crate) fn desk(
    desk: &Desk,
    pointer: (i32, i32),
    names: &[String],
    parts: &[(&str, u32)],
) -> String {
    let current = names.get(desk.current).map_or("", String::as_str);
    let shown = names.iter().zip(desk.shown.iter().zip(&desk.made));
    let shown: Vec<String> = shown
        .map(|(name, (shown, made))| match made {
            Some(made) => format!("{name}:{shown}/{made}"),
            None => format!("{name}:{shown}"),
        })
        .collect();
    let parts: Vec<String> = parts
        .iter()
        .map(|(called, process)| format!("{called}:{process}"))
        .collect();
    let parts = if parts.is_empty() {
        "-".to_owned()
    } else {
        parts.join(",")
    };
    format!(
        "d2 {} {} {current} {parts} {}",
        pointer.0,
        pointer.1,
        shown.join(" ")
    )
}

/// Where the session is, as [`desk`] wrote it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Said<'a> {
    /// The desktop each output shows, by its name, and the workspace made
    /// for windows to share it shows, if it does.
    pub(crate) shown: Vec<(&'a str, usize, Option<u64>)>,
    /// The name of the output with the keyboard.
    pub(crate) current: &'a str,
    /// Where the pointer is.
    pub(crate) pointer: (i32, i32),
    /// The parts of the session the relay keeps.
    pub(crate) parts: Named<'a>,
}

/// Where the session is, as [`desk`] wrote it.
pub(crate) fn read_desk(text: &str) -> Option<Said<'_>> {
    let (second, text) = text
        .strip_prefix("d2 ")
        .map_or((false, text), |text| (true, text));
    let mut words = text.split(' ');
    let x = words.next().and_then(|word| word.parse().ok())?;
    let y = words.next().and_then(|word| word.parse().ok())?;
    let current = words.next()?;
    let parts = if second {
        read_parts(words.next()?)?
    } else {
        Vec::new()
    };
    let shown = words.filter(|word| !word.is_empty()).map(|word| {
        let (name, shown) = word.rsplit_once(':')?;
        let (shown, made) = match shown.split_once('/') {
            Some((shown, made)) => (shown, Some(made.parse().ok()?)),
            None => (shown, None),
        };
        Some((name, shown.parse().ok()?, made))
    });
    let shown: Option<Vec<(&str, usize, Option<u64>)>> = shown.collect();
    Some(Said {
        shown: shown?,
        current,
        pointer: (x, y),
        parts,
    })
}

/// The parts of the session the relay keeps, as [`desk`] wrote them.
fn read_parts(text: &str) -> Option<Named<'_>> {
    if text == "-" {
        return Some(Vec::new());
    }
    let parts = text.split(',').map(|part| {
        let (called, process) = part.split_once(':')?;
        Some((called, process.parse().ok()?))
    });
    parts.collect()
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
            tab: None,
            tile: None,
        }
    }

    #[test]
    fn where_a_window_is_is_read_as_it_is_written() {
        let written = place(&kept(), "DP-1");
        assert_eq!(written, "v2 2 -10 20 640 480 mnac 3 - - DP-1");
        assert_eq!(read_place(&written), Some((kept(), "DP-1".to_owned())));
        let plain = Kept {
            states: States::default(),
            standing: Standing::default(),
            focused: false,
            ..kept()
        };
        let written = place(&plain, "eDP-1");
        assert_eq!(written, "v2 2 -10 20 640 480 - 3 - - eDP-1");
        assert_eq!(read_place(&written), Some((plain, "eDP-1".to_owned())));
    }

    #[test]
    fn the_tab_and_the_part_a_window_has_are_read_as_they_are_written() {
        let shared = Kept {
            tab: Some(Tab {
                frame: 12,
                at: 1,
                shown: true,
            }),
            tile: Some(Tile {
                workspace: 7,
                stacked: true,
                splits: [30_000, 32_768, 65_536],
                parts: [true, true, false, true],
                at: 3,
            }),
            ..kept()
        };
        let written = place(&shared, "DP-1");
        assert_eq!(
            written,
            "v2 2 -10 20 640 480 mnact 3 12:1 7:3:s:30000:32768:65536:1101 DP-1"
        );
        assert_eq!(read_place(&written), Some((shared, "DP-1".to_owned())));
        let hidden = Kept {
            tab: shared.tab.map(|tab| Tab {
                shown: false,
                ..tab
            }),
            tile: shared.tile.map(|tile| Tile {
                stacked: false,
                ..tile
            }),
            ..shared
        };
        let written = place(&hidden, "DP-1");
        assert_eq!(read_place(&written), Some((hidden, "DP-1".to_owned())));
    }

    #[test]
    fn a_place_written_before_tabs_and_parts_were_is_read_as_neither() {
        let read = read_place("2 -10 20 640 480 mnac 3 DP 1");
        assert_eq!(read, Some((kept(), "DP 1".to_owned())));
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
            "v2 0 0 0 1 1 - 0 - -",
            "v2 0 0 0 1 1 - 0 -",
            "v2 0 0 0 1 1 - 0",
            "v2 0 0 0 1 1 - 0 1 - a",
            "v2 0 0 0 1 1 - 0 x:1 - a",
            "v2 0 0 0 1 1 - 0 1:x - a",
            "v2 0 0 0 1 1 - 0 - 1 a",
            "v2 0 0 0 1 1 - 0 - x:0:-:1:1:1:1111 a",
            "v2 0 0 0 1 1 - 0 - 1:x:-:1:1:1:1111 a",
            "v2 0 0 0 1 1 - 0 - 1:4:-:1:1:1:1111 a",
            "v2 0 0 0 1 1 - 0 - 1:0 a",
            "v2 0 0 0 1 1 - 0 - 1:0:- a",
            "v2 0 0 0 1 1 - 0 - 1:0:-:x:1:1:1111 a",
            "v2 0 0 0 1 1 - 0 - 1:0:-:1:x:1:1111 a",
            "v2 0 0 0 1 1 - 0 - 1:0:-:1:1:x:1111 a",
            "v2 0 0 0 1 1 - 0 - 1:0:-:1:1:1 a",
            "v2 0 0 0 1 1 - 0 - 1:0:-:1:1:1:111 a",
        ] {
            assert_eq!(read_place(text), None, "{text:?}");
        }
    }

    #[test]
    fn where_the_session_is_is_read_as_it_is_written() {
        let names = ["eDP-1".to_owned(), "DP-1".to_owned()];
        let shown = Desk {
            shown: vec![0, 2],
            made: vec![None, Some(9)],
            current: 1,
        };
        let parts = [("panel", 12), ("keeper", 34)];
        let written = desk(&shown, (400, -3), &names, &parts);
        assert_eq!(
            written,
            "d2 400 -3 DP-1 panel:12,keeper:34 eDP-1:0 DP-1:2/9"
        );
        let read = read_desk(&written);
        let said = Said {
            shown: vec![("eDP-1", 0, None), ("DP-1", 2, Some(9))],
            current: "DP-1",
            pointer: (400, -3),
            parts: parts.to_vec(),
        };
        assert_eq!(read, Some(said));
        // With no output, there is nothing to name, and with no part kept,
        // none.
        let alone = desk(
            &Desk {
                shown: vec![0],
                made: vec![None],
                current: 0,
            },
            (0, 0),
            &[],
            &[],
        );
        let nothing = Said {
            shown: Vec::new(),
            current: "",
            pointer: (0, 0),
            parts: Vec::new(),
        };
        assert_eq!(read_desk(&alone), Some(nothing));
        let bad = ["", "1", "1 2", "x 2 a", "1 x a", "1 2 a b", "1 2 a b:x"];
        let bad = bad.into_iter().chain(["1 2 a b:1/x", "1 2 a b:x/1"]);
        let parts = [
            "d2 1 2 a",
            "d2 1 2 a panel",
            "d2 1 2 a panel:x",
            "d2 1 2 a -,panel:1",
        ];
        for text in bad.chain(parts) {
            assert_eq!(read_desk(text), None, "{text:?}");
        }
    }

    #[test]
    fn where_the_session_was_written_before_parts_were_is_read_as_naming_none() {
        let said = Said {
            shown: vec![("eDP-1", 1, None)],
            current: "eDP-1",
            pointer: (4, 5),
            parts: Vec::new(),
        };
        assert_eq!(read_desk("4 5 eDP-1 eDP-1:1"), Some(said));
    }
}
