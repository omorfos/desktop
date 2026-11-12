//! Every action, as a person who asks what there is is told it: by
//! `omorfosctl action`, and later by the manual pages. Here, in the crate
//! every program of omorfos already has, rather than beside the
//! configuration's own types, so that `omorfosctl` needs none of what
//! reading the configuration brings in.

/// An action as it is told to a person who asks what there is, as
/// `omorfosctl action` lists them: in a group of its kind, by its name,
/// with what it takes and what it does in a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Told {
    /// The kind of thing it does.
    pub group: &'static str,
    /// Its name, as keys.yaml, menu.yaml and `omorfosctl action` give it.
    pub name: &'static str,
    /// What it takes after its name, or nothing.
    pub takes: &'static str,
    /// What it does, in a line.
    pub does: &'static str,
}

/// An action as it is told: in a group, by its name, taking what, doing
/// what. A macro rather than a function, so that nothing is left to run
/// once the list is made.
macro_rules! told {
    ($group:expr, $name:expr, $takes:expr, $does:expr $(,)?) => {
        Told {
            group: $group,
            name: $name,
            takes: $takes,
            does: $does,
        }
    };
}

/// Every action, told as a person is told them, in groups. A test checks
/// that every action the configuration knows is here.
pub const TOLD: &[Told] = &[
    told!("Windows", "close", "", "Ask the focused window to close"),
    told!(
        "Windows",
        "maximize",
        "",
        "Maximize the focused window, or restore it",
    ),
    told!(
        "Windows",
        "fullscreen",
        "",
        "Make the focused window fullscreen, or not",
    ),
    told!("Windows", "minimize", "", "Hide the focused window"),
    told!(
        "Windows",
        "shade",
        "",
        "Roll up the focused window, or let it down",
    ),
    told!(
        "Windows",
        "keep_above",
        "",
        "Keep the focused window on top, or not",
    ),
    told!(
        "Windows",
        "stick",
        "",
        "Show the focused window everywhere, or not",
    ),
    told!(
        "Windows",
        "focus_next",
        "",
        "Focus the next window on the workspace",
    ),
    told!(
        "Windows",
        "focus_previous",
        "",
        "Focus the previous window on the workspace",
    ),
    told!(
        "Windows",
        "next_tab",
        "",
        "Show the next tab of the focused window",
    ),
    told!(
        "Windows",
        "previous_tab",
        "",
        "Show the previous tab of the focused window",
    ),
    told!(
        "Windows",
        "tile",
        SIDE,
        "Tile the focused window to that half",
    ),
    told!(
        "Windows",
        "focus",
        SIDE,
        "Focus the window beside the focused one",
    ),
    told!(
        "Windows",
        "swap",
        SIDE,
        "Swap the focused window with the one beside it",
    ),
    told!(
        "Windows",
        "move_to_next_output",
        "",
        "Send the focused window to the next screen",
    ),
    told!(
        "Switching",
        "switch_windows",
        "",
        "Open the switcher, or go on to the next window",
    ),
    told!(
        "Switching",
        "switch_windows_back",
        "",
        "The same, going back",
    ),
    told!(
        "Switching",
        "overview",
        "",
        "Open the overview of the workspace, or close it",
    ),
    told!("Workspaces", "workspace", "N", "Show workspace N"),
    told!(
        "Workspaces",
        "next_workspace",
        "",
        "Show the next workspace",
    ),
    told!(
        "Workspaces",
        "previous_workspace",
        "",
        "Show the workspace before",
    ),
    told!(
        "Workspaces",
        "new_workspace",
        "",
        "Make a workspace after the last, and show it",
    ),
    told!(
        "Workspaces",
        "move_to_workspace",
        "N",
        "Send the focused window to workspace N",
    ),
    told!(
        "Workspaces",
        "move_to_new_workspace",
        "",
        "Send the focused window to a new workspace",
    ),
    told!(
        "Workspaces",
        "join_workspace",
        "ID",
        "Put the focused window in a shared workspace",
    ),
    told!(
        "Programs",
        "spawn",
        "PROGRAM [ARG]...",
        "Start a program, with no shell",
    ),
    told!("Programs", "show_launcher", "", "Open the launcher"),
    told!(
        "Programs",
        "show_menu",
        "",
        "Open the root menu at the pointer",
    ),
    told!(
        "Programs",
        "about",
        "",
        "Show \"About omorfos\", or hide it",
    ),
    told!(
        "Screens and input",
        "next_screens",
        "",
        "Arrange the screens the next way",
    ),
    told!(
        "Screens and input",
        "next_layout",
        "",
        "Type with the next keyboard layout",
    ),
    told!(
        "Screens and input",
        "night_light",
        "",
        "Turn night light on or off until it next turns",
    ),
    told!("Session", "lock", "", "Lock the session"),
    told!(
        "Session",
        "stay_awake",
        "",
        "Keep the session awake, or let it rest again",
    ),
    told!(
        "Session",
        "stay_awake_for",
        "SECONDS",
        "Keep the session awake a while, a day at most",
    ),
    told!(
        "Session",
        "reload_config",
        "",
        "Read the configuration again",
    ),
    told!(
        "Session",
        "restart_compositor",
        "",
        "Start the compositor again, keeping the login",
    ),
    told!("Session", "quit", "", "End the session"),
];

/// What an action that takes a side takes.
const SIDE: &str = "left|right|top|bottom";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_there_is_is_told_once_and_nothing_else_is() {
        // Every action the configuration promises, by its name, from the
        // list of what it promises.
        let promised = include_str!("../../../schemas/promised.txt");
        let mut known: Vec<&str> = promised
            .lines()
            .filter_map(|line| {
                let plain = line.strip_prefix("keys.yaml bindings[].action = \"");
                let plain = plain.and_then(|rest| rest.strip_suffix('"'));
                let taking = line.strip_prefix("keys.yaml bindings[].action.");
                let taking = taking.filter(|name| !name.contains(['.', ' ', '[']));
                plain.or(taking)
            })
            .collect();
        known.sort_unstable();
        let mut told: Vec<&str> = TOLD.iter().map(|told| told.name).collect();
        told.sort_unstable();
        assert_eq!(told, known);
        // Each in a line of 80 beside its name and what it takes, in a
        // column after the widest of them.
        let named = |told: &Told| told.name.len() + told.takes.len() + 1;
        let widest = TOLD.iter().map(named).max().unwrap_or_default();
        for told in TOLD {
            assert!(2 + widest + 2 + told.does.len() <= 80, "{told:?}");
        }
    }
}
