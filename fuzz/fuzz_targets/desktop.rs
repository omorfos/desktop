//! What can be done to windows, workspaces and screens, done to the
//! desktop in any order: a byte says what, and those after it what to and
//! where. After each, the desktop is checked to be whole: each window is in
//! one frame, on a workspace and a screen there are, and at most one has the
//! keyboard. Nothing done may make it panic or come apart.

#![no_main]

use std::collections::HashSet;

use libfuzzer_sys::fuzz_target;
use omorfos_core::geometry::{Point, Rect, Size};
use omorfos_core::{Arrangement, Desktop, Half, Settings, WindowId};

/// The halves of a screen, by a number.
const HALVES: [Half; 4] = [Half::Left, Half::Right, Half::Top, Half::Bottom];

/// The names screens taken away are known by, by a number.
const SCREENS: [&str; 2] = ["one", "two"];

fuzz_target!(|data: &[u8]| {
    let mut desktop = Desktop::new(Settings::default(), 2);
    let mut bytes = data.iter().copied();
    let mut next = || bytes.next();
    let mut made: Vec<WindowId> = Vec::new();
    while let Some(what) = next() {
        let Some(at) = next() else { break };
        let at = usize::from(at);
        // A window made, or one there never was, as a program may name one
        // that has gone.
        let window = made.get(at % (made.len() + 1)).copied();
        let other = made.get((at / 7) % (made.len() + 1)).copied();
        let point = Point::new(i32::from(at as u8) * 16 - 512, i32::from(what) * 8 - 256);
        match what % 32 {
            0 => {
                let size = Size::new(u32::from(what), u32::try_from(at).unwrap_or(0) * 4);
                made.push(desktop.add_window("app", "title", size, point, at % 2 == 0));
            }
            1 => {
                let parent = window;
                let size = Size::new(200, 100);
                made.push(desktop.add_window_of(parent, "app", "dialog", size, point, true));
            }
            2 => window.into_iter().for_each(|id| desktop.remove_window(id)),
            3 => {
                // One to four screens, side by side, of sizes the byte says.
                let count = at % 4 + 1;
                let screens = (0..count).map(|index| {
                    let width = 320 + 160 * u32::try_from((at >> 2) % 8).unwrap_or(0);
                    let x = i32::try_from(index).unwrap_or(0) * 2560;
                    Rect::new(Point::new(x, 0), Size::new(width, 600))
                });
                desktop.set_outputs(screens);
            }
            4 => desktop.remove_output(at % 4),
            5 => desktop.remove_output_known(at % 4, SCREENS[at % 2]),
            6 => desktop.bring_back(at % 4, SCREENS[at % 2]),
            7 => desktop.set_primary(at % 4),
            8 => desktop.set_workspace_count(at % 6),
            9 => desktop.switch_workspace(at % 8),
            10 => desktop.new_workspace(),
            11 => desktop.next_workspace(),
            12 => desktop.previous_workspace(),
            13 => desktop.slide(i32::from(at as u8 as i8) * 40),
            14 => window.into_iter().for_each(|id| desktop.move_to_workspace(id, at % 8)),
            15 => window.into_iter().for_each(|id| desktop.move_to_new_workspace(id)),
            16 => window.into_iter().for_each(|id| desktop.move_to_next_output(id)),
            17 => window.into_iter().for_each(|id| desktop.tile(id, HALVES[at % 4])),
            18 => window.into_iter().for_each(|id| {
                let arrangement = Arrangement::ALL[at % Arrangement::ALL.len()];
                desktop.arrange(id, arrangement, at / 8 % 4);
            }),
            19 => window.into_iter().for_each(|id| desktop.swap_toward(id, HALVES[at % 4])),
            20 => desktop.focus_toward(HALVES[at % 4]),
            21 => window
                .zip(other)
                .into_iter()
                .for_each(|(id, target)| desktop.attach(id, target)),
            22 => window.into_iter().for_each(|id| desktop.detach(id)),
            23 => window.into_iter().for_each(|id| desktop.toggle_maximized(id)),
            24 => window.into_iter().for_each(|id| desktop.set_fullscreen(id, at % 2 == 0)),
            25 => window.into_iter().for_each(|id| desktop.minimize(id)),
            26 => window.into_iter().for_each(|id| desktop.activate(id)),
            27 => window.into_iter().for_each(|id| desktop.clicked(id)),
            28 => desktop.focus_next(),
            29 => desktop.focus_previous(),
            30 => window.into_iter().for_each(|id| {
                let geometry = Rect::new(point, Size::new(u32::from(what), 300));
                desktop.set_geometry(id, geometry);
            }),
            _ => desktop.turn_to(point),
        }
        whole(&desktop);
    }
});

/// Checks that `desktop` is whole: each window listed once, in a frame
/// that holds it, on a workspace and a screen there are; at most one
/// focused, and that one listed.
fn whole(desktop: &Desktop) {
    // Every workspace, desktops and those made for a fullscreen window or
    // for tiles alike, by its screen and its place among the screen's.
    let there: HashSet<(usize, usize)> = desktop
        .workspaces()
        .map(|place| (place.output, place.index))
        .collect();
    let mut listed = HashSet::new();
    let mut focused = 0;
    for each in desktop.windows() {
        let id = each.window.id;
        assert!(listed.insert(id), "{id:?} listed twice");
        let frame = desktop.frame_of(id);
        let frame = frame.unwrap_or_else(|| panic!("{id:?} in no frame"));
        assert!(frame.tabs.iter().any(|tab| tab.id == id), "{id:?} not in its frame");
        assert!(frame.active < frame.tabs.len(), "{id:?}'s frame shows no tab");
        assert_eq!(desktop.workspace_of(id), Some(each.workspace));
        assert_eq!(desktop.output_of(id), Some(each.output));
        let on = (each.output, each.workspace);
        assert!(there.contains(&on), "{id:?} on {on:?}, which is not there");
        focused += usize::from(each.focused);
    }
    assert!(focused <= 1, "{focused} windows focused");
    if let Some(id) = desktop.focused() {
        assert!(listed.contains(&id), "{id:?} focused, and not listed");
    }
    for frame in desktop.frames() {
        assert!(!frame.tabs.is_empty(), "a frame of nothing drawn");
        for tab in frame.tabs {
            assert!(listed.contains(&tab.id), "{:?} drawn, and not listed", tab.id);
        }
    }
    assert!(desktop.workspace_count() >= 1, "no desktop");
    for (output, shown) in desktop.shown_workspaces().enumerate() {
        assert!(there.contains(&(output, shown)), "screen {output} shows {shown}, not there");
    }
}
