//! The state of the session: windows, tab groups, workspaces and focus.

use crate::decoration::{Decoration, Metrics};
use crate::geometry::{Point, Rect, Size};
use crate::placement::{CASCADE_STEP, Placement, place};
use crate::rules::{Rule, resolve};

/// Identifies a window for as long as it exists. Never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(u64);

/// An application window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    /// The window's identifier.
    pub id: WindowId,
    /// The application the window belongs to.
    pub app_id: String,
    /// The window's title.
    pub title: String,
    /// Whether the window gets a titlebar and border.
    pub decorated: bool,
}

/// Behaviour that the user can configure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// Whether moving the pointer onto a window focuses it. When `false`, a
    /// window has to be clicked.
    pub focus_follows_pointer: bool,
    /// Whether a window is raised when the pointer or a click focuses it.
    pub raise_on_focus: bool,
    /// Where new windows appear.
    pub placement: Placement,
    /// The thickness of the titlebar and borders around decorated windows.
    pub decoration: Metrics,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            focus_follows_pointer: false,
            raise_on_focus: true,
            placement: Placement::default(),
            decoration: Metrics::default(),
        }
    }
}

/// A frame on screen: one or more windows sharing a position as tabs.
///
/// This is a read-only view produced by [`Desktop::frames`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame<'a> {
    /// The windows in the frame, in tab order. Never empty.
    pub tabs: &'a [Window],
    /// The index in `tabs` of the window being shown.
    pub active: usize,
    /// Where the frame is on screen, decoration included, taking maximized
    /// and fullscreen into account.
    pub geometry: Rect,
    /// The layout of the titlebar and borders, if the frame has them. A
    /// frame is decorated when its shown window is, or when it holds
    /// several tabs, which need a titlebar to be reachable. A fullscreen
    /// frame never is.
    pub decoration: Option<Decoration>,
    /// Whether the frame fills its output.
    pub maximized: bool,
    /// Whether the frame covers its output without decorations.
    pub fullscreen: bool,
    /// Whether the frame has keyboard focus.
    pub focused: bool,
}

impl Frame<'_> {
    /// The window being shown.
    #[must_use]
    pub fn window(&self) -> &Window {
        &self.tabs[self.active]
    }

    /// The part of the frame the window itself occupies.
    #[must_use]
    pub fn client(&self) -> Rect {
        self.decoration
            .map_or(self.geometry, |decoration| decoration.client())
    }
}

/// One or more windows sharing a position as tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Group {
    /// Creation order, which is also the order focus cycles in.
    serial: u64,
    tabs: Vec<Window>,
    active: usize,
    /// The position when neither maximized nor fullscreen.
    geometry: Rect,
    maximized: bool,
    fullscreen: bool,
    minimized: bool,
}

impl Group {
    /// Whether the frame has a titlebar and borders when not fullscreen.
    fn decorated(&self) -> bool {
        self.tabs.len() > 1 || self.tabs[self.active].decorated
    }

    fn remove(&mut self, tab: usize) -> Window {
        let window = self.tabs.remove(tab);
        // Keep showing the same window; if it was the one removed, show its
        // right-hand neighbour, or the new last tab if there is none.
        if tab < self.active || self.active == self.tabs.len() {
            self.active = self.active.saturating_sub(1);
        }
        window
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Workspace {
    /// Bottom to top.
    groups: Vec<Group>,
    focused: Option<u64>,
}

impl Workspace {
    /// Drops emptied groups and, if the focused group is gone or hidden,
    /// focuses the topmost one that can take focus.
    fn repair(&mut self) {
        self.groups.retain(|group| !group.tabs.is_empty());
        let focusable = |group: &&Group| !group.minimized;
        let focused = self.focused;
        if !self
            .groups
            .iter()
            .filter(focusable)
            .any(|group| Some(group.serial) == focused)
        {
            self.focused = self
                .groups
                .iter()
                .rev()
                .find(focusable)
                .map(|group| group.serial);
        }
    }

    fn focus(&mut self, group: usize, raise: bool) {
        self.groups[group].minimized = false;
        self.focused = Some(self.groups[group].serial);
        if raise {
            let raised = self.groups.remove(group);
            self.groups.push(raised);
        }
    }
}

/// Where a window is: its workspace, its group there, and its tab.
#[derive(Clone, Copy)]
struct Location {
    workspace: usize,
    group: usize,
    tab: usize,
}

/// The state of the session.
///
/// Windows live in frames (tab groups of one or more), frames live on
/// workspaces, and one workspace is shown at a time across all outputs.
/// Methods that take a [`WindowId`] do nothing if the window is unknown, so
/// a late event for a window that has just closed is harmless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desktop {
    settings: Settings,
    rules: Vec<Rule>,
    outputs: Vec<Rect>,
    /// Never empty.
    workspaces: Vec<Workspace>,
    active: usize,
    next_serial: u64,
}

impl Desktop {
    /// Creates an empty desktop with `workspaces` workspaces (at least one).
    #[must_use]
    pub fn new(settings: Settings, workspaces: usize) -> Self {
        Self {
            settings,
            rules: Vec::new(),
            outputs: Vec::new(),
            workspaces: vec![Workspace::default(); workspaces.max(1)],
            active: 0,
            next_serial: 0,
        }
    }

    // --- Configuration -----------------------------------------------------

    /// Replaces the configurable behaviour.
    pub const fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Replaces the rules applied to windows added from now on.
    pub fn set_rules(&mut self, rules: Vec<Rule>) {
        self.rules = rules;
    }

    /// Replaces the outputs, given as rectangles in the global space.
    ///
    /// Frames left entirely off-screen are moved onto the first output.
    pub fn set_outputs(&mut self, outputs: Vec<Rect>) {
        self.outputs = outputs;
        if let Some(first) = self.outputs.first() {
            let outputs = &self.outputs;
            for group in self
                .workspaces
                .iter_mut()
                .flat_map(|workspace| &mut workspace.groups)
            {
                if !outputs
                    .iter()
                    .any(|output| output.intersection(group.geometry).is_some())
                {
                    group.geometry.origin = first.origin;
                }
            }
        }
    }

    /// Changes the number of workspaces (at least one).
    ///
    /// Frames on removed workspaces move to the last remaining one.
    pub fn set_workspace_count(&mut self, count: usize) {
        let last = count.max(1) - 1;
        let orphans: Vec<Group> = self
            .workspaces
            .iter_mut()
            .skip(last + 1)
            .flat_map(|workspace| workspace.groups.drain(..))
            .collect();
        self.workspaces.resize_with(last + 1, Workspace::default);
        self.workspaces[last].groups.extend(orphans);
        self.workspaces[last].repair();
        self.active = self.active.min(last);
    }

    // --- Windows -----------------------------------------------------------

    /// Adds a new window in a frame of its own and returns its identifier.
    ///
    /// `size` is the size of the window itself; the frame is larger by its
    /// decoration. `wants_decoration` says whether the window expects the
    /// compositor to draw its titlebar and border; a rule overrides it
    /// either way. Rules also decide its workspace and initial state;
    /// otherwise it opens on the current workspace. It is placed on the
    /// output under `pointer` and becomes the focused, topmost frame of its
    /// workspace.
    pub fn add_window(
        &mut self,
        app_id: impl Into<String>,
        title: impl Into<String>,
        size: Size,
        pointer: Point,
        wants_decoration: bool,
    ) -> WindowId {
        let (app_id, title) = (app_id.into(), title.into());
        let effects = resolve(&self.rules, &app_id, &title);
        let last = self.workspaces.len() - 1;
        let workspace = effects
            .workspace
            .map_or(self.active, |workspace| workspace.min(last));
        let decorated = effects.decorations.unwrap_or(wants_decoration);
        let size = if decorated {
            self.settings.decoration.outer_size(size)
        } else {
            size
        };
        let area = self.output_at(pointer);
        let index = self.workspaces[workspace].groups.len();
        let origin = place(self.settings.placement, size, area, pointer, index);
        let id = WindowId(self.serial());
        let window = Window {
            id,
            app_id,
            title,
            decorated,
        };
        let group = Group {
            serial: self.serial(),
            tabs: vec![window],
            active: 0,
            geometry: Rect::new(origin, size),
            maximized: effects.maximized.unwrap_or(false),
            fullscreen: effects.fullscreen.unwrap_or(false),
            minimized: false,
        };
        self.push_focused(workspace, group);
        id
    }

    /// Removes a window, for example because it closed.
    pub fn remove_window(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.workspaces[at.workspace].groups[at.group].remove(at.tab);
        self.workspaces[at.workspace].repair();
    }

    /// Updates a window's title.
    pub fn set_title(&mut self, id: WindowId, title: impl Into<String>) {
        let Some(at) = self.locate(id) else { return };
        self.workspaces[at.workspace].groups[at.group].tabs[at.tab].title = title.into();
    }

    /// Looks a window up.
    #[must_use]
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.locate(id)
            .map(|at| &self.workspaces[at.workspace].groups[at.group].tabs[at.tab])
    }

    /// The zero-based workspace a window is on.
    #[must_use]
    pub fn workspace_of(&self, id: WindowId) -> Option<usize> {
        self.locate(id).map(|at| at.workspace)
    }

    /// Sets whether a window gets a titlebar and border, for example
    /// because it turned out to draw its own. The frame keeps its place.
    pub fn set_decorated(&mut self, id: WindowId, decorated: bool) {
        let Some(at) = self.locate(id) else { return };
        self.workspaces[at.workspace].groups[at.group].tabs[at.tab].decorated = decorated;
    }

    // --- Frame state -------------------------------------------------------

    /// Records that a window resized itself to `size`, and resizes its frame
    /// to fit. Ignored while the frame is maximized or fullscreen, when its
    /// size is not the window's to choose, and for a tab that is not shown.
    pub fn set_client_size(&mut self, id: WindowId, size: Size) {
        let Some(at) = self.locate(id) else { return };
        let metrics = self.settings.decoration;
        let group = &mut self.workspaces[at.workspace].groups[at.group];
        if group.active == at.tab && !group.maximized && !group.fullscreen {
            group.geometry.size = if group.decorated() {
                metrics.outer_size(size)
            } else {
                size
            };
        }
    }

    /// Moves and resizes a window's frame, leaving maximized and fullscreen.
    pub fn set_geometry(&mut self, id: WindowId, geometry: Rect) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.geometry = geometry;
        group.maximized = false;
        group.fullscreen = false;
    }

    /// Maximizes a window's frame, or restores it.
    pub fn set_maximized(&mut self, id: WindowId, maximized: bool) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.maximized = maximized;
    }

    /// Switches a window's frame between maximized and restored.
    pub fn toggle_maximized(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.maximized = !group.maximized;
    }

    /// Makes a window's frame fullscreen, or returns it to how it was.
    pub fn set_fullscreen(&mut self, id: WindowId, fullscreen: bool) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.fullscreen = fullscreen;
    }

    /// Switches a window's frame in or out of fullscreen.
    pub fn toggle_fullscreen(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.fullscreen = !group.fullscreen;
    }

    /// Hides a window's frame. Focus passes to the topmost remaining frame.
    /// [`Desktop::activate`] brings it back.
    pub fn minimize(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.workspaces[at.workspace].groups[at.group].minimized = true;
        self.workspaces[at.workspace].repair();
    }

    // --- Focus -------------------------------------------------------------

    /// Brings a window to the user's attention: switches to its workspace,
    /// restores it if minimized, shows its tab, focuses and raises it.
    pub fn activate(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.active = at.workspace;
        self.show(at, true);
    }

    /// Reports a click on a window, which focuses it and shows its tab.
    pub fn clicked(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.show(at, self.settings.raise_on_focus);
    }

    /// Reports the pointer moving onto a window, which focuses it when focus
    /// follows the pointer.
    pub fn pointer_entered(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        if self.settings.focus_follows_pointer {
            self.workspaces[at.workspace].focus(at.group, self.settings.raise_on_focus);
        }
    }

    /// Focuses and raises the frame created after the focused one, wrapping
    /// around.
    pub fn focus_next(&mut self) {
        self.cycle_focus(false);
    }

    /// Focuses and raises the frame created before the focused one, wrapping
    /// around.
    pub fn focus_previous(&mut self) {
        self.cycle_focus(true);
    }

    /// The window with keyboard focus: the shown tab of the focused frame on
    /// the current workspace.
    #[must_use]
    pub fn focused(&self) -> Option<WindowId> {
        self.frames()
            .find(|frame| frame.focused)
            .map(|frame| frame.window().id)
    }

    // --- Tabs --------------------------------------------------------------

    /// Shows the tab after the current one in a window's frame, wrapping.
    pub fn next_tab(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.active = (group.active + 1) % group.tabs.len();
    }

    /// Shows the tab before the current one in a window's frame, wrapping.
    pub fn previous_tab(&mut self, id: WindowId) {
        let Some(group) = self.group_mut(id) else {
            return;
        };
        group.active = (group.active + group.tabs.len() - 1) % group.tabs.len();
    }

    /// Moves a window into `target`'s frame as its last and shown tab, and
    /// focuses that frame.
    pub fn attach(&mut self, id: WindowId, target: WindowId) {
        let (Some(from), Some(to)) = (self.locate(id), self.locate(target)) else {
            return;
        };
        if (from.workspace, from.group) == (to.workspace, to.group) {
            return;
        }
        let window = self.workspaces[from.workspace].groups[from.group].remove(from.tab);
        let group = &mut self.workspaces[to.workspace].groups[to.group];
        group.tabs.push(window);
        group.active = group.tabs.len() - 1;
        self.workspaces[to.workspace].focus(to.group, true);
        self.workspaces[from.workspace].repair();
    }

    /// Moves a window out of a shared frame into a frame of its own, just
    /// off the corner of the old one, and focuses it.
    pub fn detach(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        let serial = self.serial();
        let source = &mut self.workspaces[at.workspace].groups[at.group];
        if source.tabs.len() < 2 {
            return;
        }
        let step = CASCADE_STEP.cast_signed();
        let group = Group {
            serial,
            tabs: vec![source.remove(at.tab)],
            active: 0,
            geometry: Rect::new(
                source.geometry.origin.offset(step, step),
                source.geometry.size,
            ),
            maximized: false,
            fullscreen: false,
            minimized: false,
        };
        self.push_focused(at.workspace, group);
    }

    // --- Workspaces --------------------------------------------------------

    /// The number of workspaces. At least one.
    #[must_use]
    pub const fn workspace_count(&self) -> usize {
        self.workspaces.len()
    }

    /// The zero-based workspace being shown.
    #[must_use]
    pub const fn active_workspace(&self) -> usize {
        self.active
    }

    /// Shows a workspace. Does nothing if it does not exist.
    pub const fn switch_workspace(&mut self, workspace: usize) {
        if workspace < self.workspaces.len() {
            self.active = workspace;
        }
    }

    /// Shows the following workspace, wrapping around.
    pub const fn next_workspace(&mut self) {
        self.active = (self.active + 1) % self.workspaces.len();
    }

    /// Shows the preceding workspace, wrapping around.
    pub const fn previous_workspace(&mut self) {
        self.active = (self.active + self.workspaces.len() - 1) % self.workspaces.len();
    }

    /// Sends a window's whole frame to another workspace, where it becomes
    /// the focused, topmost frame. The workspace being shown does not change.
    pub fn move_to_workspace(&mut self, id: WindowId, workspace: usize) {
        let Some(at) = self.locate(id) else { return };
        if workspace >= self.workspaces.len() || workspace == at.workspace {
            return;
        }
        let group = self.workspaces[at.workspace].groups.remove(at.group);
        self.workspaces[at.workspace].repair();
        self.push_focused(workspace, group);
    }

    // --- What to show ------------------------------------------------------

    /// The frames to draw on the current workspace, bottom to top. Minimized
    /// frames are left out, and fullscreen frames are above all others.
    pub fn frames(&self) -> impl Iterator<Item = Frame<'_>> {
        let workspace = &self.workspaces[self.active];
        let shown = |fullscreen: bool| {
            workspace
                .groups
                .iter()
                .filter(move |group| !group.minimized && group.fullscreen == fullscreen)
        };
        shown(false).chain(shown(true)).map(|group| {
            let geometry = if group.maximized || group.fullscreen {
                self.output_for(group.geometry)
            } else {
                group.geometry
            };
            let decorated = group.decorated() && !group.fullscreen;
            Frame {
                tabs: &group.tabs,
                active: group.active,
                geometry,
                decoration: decorated
                    .then(|| Decoration::new(geometry, self.settings.decoration, group.tabs.len())),
                maximized: group.maximized,
                fullscreen: group.fullscreen,
                focused: workspace.focused == Some(group.serial),
            }
        })
    }

    // --- Internals ---------------------------------------------------------

    const fn serial(&mut self) -> u64 {
        self.next_serial += 1;
        self.next_serial
    }

    fn locate(&self, id: WindowId) -> Option<Location> {
        self.workspaces
            .iter()
            .enumerate()
            .find_map(|(workspace, on)| {
                on.groups.iter().enumerate().find_map(|(group, in_group)| {
                    let tab = in_group.tabs.iter().position(|window| window.id == id)?;
                    Some(Location {
                        workspace,
                        group,
                        tab,
                    })
                })
            })
    }

    fn group_mut(&mut self, id: WindowId) -> Option<&mut Group> {
        self.locate(id)
            .map(|at| &mut self.workspaces[at.workspace].groups[at.group])
    }

    fn push_focused(&mut self, workspace: usize, group: Group) {
        let workspace = &mut self.workspaces[workspace];
        workspace.focused = Some(group.serial);
        workspace.groups.push(group);
        // A minimized frame can be moved here; it must not hold focus.
        workspace.repair();
    }

    fn show(&mut self, at: Location, raise: bool) {
        let workspace = &mut self.workspaces[at.workspace];
        workspace.groups[at.group].active = at.tab;
        workspace.focus(at.group, raise);
    }

    fn cycle_focus(&mut self, backwards: bool) {
        let workspace = &mut self.workspaces[self.active];
        let mut serials: Vec<u64> = workspace
            .groups
            .iter()
            .filter(|group| !group.minimized)
            .map(|group| group.serial)
            .collect();
        serials.sort_unstable();
        if backwards {
            serials.reverse();
        }
        let focused = workspace.focused;
        let after_focused = serials
            .iter()
            .skip_while(|serial| Some(**serial) != focused)
            .nth(1);
        let next = after_focused.or(serials.first()).copied();
        if let Some(group) = workspace
            .groups
            .iter()
            .position(|group| Some(group.serial) == next)
        {
            workspace.focus(group, true);
        }
    }

    /// The output containing `point`, or failing that the first one.
    fn output_at(&self, point: Point) -> Rect {
        let containing = self.outputs.iter().find(|output| output.contains(point));
        containing
            .or(self.outputs.first())
            .copied()
            .unwrap_or_default()
    }

    /// The output a frame at `geometry` mostly lies on. With no outputs,
    /// the geometry itself.
    fn output_for(&self, geometry: Rect) -> Rect {
        let overlap = |output: &&Rect| {
            output
                .intersection(geometry)
                .map_or(0, |shared| shared.size.area())
        };
        // `max_by_key` keeps the last of equal maxima; reversing makes ties,
        // including no overlap at all, resolve to the first output.
        self.outputs
            .iter()
            .rev()
            .max_by_key(overlap)
            .copied()
            .unwrap_or(geometry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{Effects, Pattern};

    const SCREEN: Rect = Rect::new(Point::new(0, 0), Size::new(1920, 1080));
    const SECOND_SCREEN: Rect = Rect::new(Point::new(1920, 0), Size::new(1280, 1024));
    const SIZE: Size = Size::new(800, 600);
    const POINTER: Point = Point::new(10, 10);

    fn desktop() -> Desktop {
        let mut desktop = Desktop::new(Settings::default(), 3);
        desktop.set_outputs(vec![SCREEN]);
        desktop
    }

    fn open(desktop: &mut Desktop, app_id: &str) -> WindowId {
        desktop.add_window(app_id, format!("{app_id} window"), SIZE, POINTER, true)
    }

    /// The shown window of each frame on the current workspace, bottom to top.
    fn stack(desktop: &Desktop) -> Vec<WindowId> {
        desktop.frames().map(|frame| frame.window().id).collect()
    }

    fn frame(desktop: &Desktop, id: WindowId) -> Frame<'_> {
        desktop
            .frames()
            .find(|frame| frame.tabs.iter().any(|tab| tab.id == id))
            .unwrap()
    }

    fn rule(app_id: &str, effects: Effects) -> Rule {
        Rule {
            app_id: Some(Pattern::new(app_id)),
            title: None,
            effects,
        }
    }

    // --- Construction ------------------------------------------------------

    #[test]
    fn a_new_desktop_is_empty_and_shows_the_first_workspace() {
        let desktop = desktop();
        assert_eq!(desktop.workspace_count(), 3);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), []);
        assert_eq!(desktop.focused(), None);
    }

    #[test]
    fn there_is_always_at_least_one_workspace() {
        assert_eq!(Desktop::new(Settings::default(), 0).workspace_count(), 1);
    }

    // --- Windows -----------------------------------------------------------

    #[test]
    fn a_new_window_is_focused_and_on_top() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        assert_eq!(stack(&desktop), [first, second]);
        assert_eq!(desktop.focused(), Some(second));
        assert!(frame(&desktop, second).focused);
        assert!(!frame(&desktop, first).focused);
    }

    #[test]
    fn a_new_window_is_decorated_normal_and_alone_in_its_frame() {
        let mut desktop = desktop();
        let id = desktop.add_window("foot", "~", SIZE, POINTER, true);
        let frame = frame(&desktop, id);
        assert_eq!(
            frame.tabs,
            [Window {
                id,
                app_id: "foot".to_owned(),
                title: "~".to_owned(),
                decorated: true
            }]
        );
        assert_eq!(
            (frame.active, frame.maximized, frame.fullscreen),
            (0, false, false)
        );
        assert_eq!(desktop.workspace_of(id), Some(0));
    }

    #[test]
    fn new_windows_follow_the_placement_setting() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        assert_eq!(
            frame(&desktop, first).geometry,
            Rect::new(Point::new(0, 0), SIZE)
        );
        assert_eq!(
            frame(&desktop, second).geometry,
            Rect::new(Point::new(32, 32), SIZE)
        );

        desktop.set_settings(Settings {
            placement: Placement::Center,
            ..Settings::default()
        });
        let third = open(&mut desktop, "third");
        assert_eq!(
            frame(&desktop, third).geometry,
            Rect::new(Point::new(560, 240), SIZE)
        );
    }

    #[test]
    fn a_new_window_opens_on_the_output_under_the_pointer() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = desktop.add_window("app", "", SIZE, Point::new(2000, 500), true);
        assert_eq!(frame(&desktop, id).geometry.origin, SECOND_SCREEN.origin);
    }

    #[test]
    fn a_new_window_opens_on_the_first_output_when_the_pointer_is_on_none() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SECOND_SCREEN, SCREEN]);
        let id = desktop.add_window("app", "", SIZE, Point::new(-500, -500), true);
        assert_eq!(frame(&desktop, id).geometry.origin, SECOND_SCREEN.origin);
    }

    #[test]
    fn a_window_can_be_added_before_any_output_exists() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        let id = open(&mut desktop, "early");
        assert_eq!(
            frame(&desktop, id).geometry,
            Rect::new(Point::new(0, 0), SIZE)
        );
    }

    #[test]
    fn rules_set_the_initial_state_of_matching_windows() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "mpv",
            Effects {
                workspace: None,
                maximized: Some(true),
                fullscreen: Some(true),
                decorations: Some(false),
            },
        )]);
        let player = open(&mut desktop, "mpv");
        let other = open(&mut desktop, "foot");
        let frame = frame(&desktop, player);
        assert!(frame.maximized && frame.fullscreen && !frame.window().decorated);
        assert!(!self::frame(&desktop, other).maximized);
    }

    #[test]
    fn a_rule_can_open_a_window_on_another_workspace_without_showing_it() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "chat",
            Effects {
                workspace: Some(1),
                ..Effects::default()
            },
        )]);
        let here = open(&mut desktop, "foot");
        let chat = open(&mut desktop, "chat");

        assert_eq!(stack(&desktop), [here]);
        assert_eq!(desktop.focused(), Some(here));
        assert_eq!(desktop.workspace_of(chat), Some(1));

        desktop.switch_workspace(1);
        assert_eq!(desktop.focused(), Some(chat));
    }

    #[test]
    fn a_rule_naming_a_missing_workspace_uses_the_last_one() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "*",
            Effects {
                workspace: Some(99),
                ..Effects::default()
            },
        )]);
        let id = open(&mut desktop, "app");
        assert_eq!(desktop.workspace_of(id), Some(2));
    }

    #[test]
    fn removing_the_focused_window_focuses_the_topmost_remaining_frame() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");

        desktop.remove_window(third);
        assert_eq!(stack(&desktop), [first, second]);
        assert_eq!(desktop.focused(), Some(second));

        desktop.remove_window(second);
        desktop.remove_window(first);
        assert_eq!(desktop.focused(), None);
        assert_eq!(desktop.window(first), None);
    }

    #[test]
    fn removing_another_window_leaves_focus_alone() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.remove_window(first);
        assert_eq!(desktop.focused(), Some(second));
    }

    #[test]
    fn a_title_can_change() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "editor");
        desktop.set_title(id, "notes.txt");
        assert_eq!(
            desktop.window(id).map(|window| window.title.as_str()),
            Some("notes.txt")
        );
    }

    #[test]
    fn window_identifiers_are_never_reused() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.remove_window(first);
        assert_ne!(open(&mut desktop, "second"), first);
    }

    // --- Decorations -------------------------------------------------------

    const METRICS: Metrics = Metrics {
        border: 2,
        titlebar: 20,
    };

    fn decorated_desktop() -> Desktop {
        let mut desktop = desktop();
        desktop.set_settings(Settings {
            decoration: METRICS,
            ..Settings::default()
        });
        desktop
    }

    #[test]
    fn a_frame_is_larger_than_its_window_by_the_decoration() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let frame = frame(&desktop, id);
        assert_eq!(
            frame.geometry,
            Rect::new(Point::new(0, 0), Size::new(804, 622))
        );
        assert_eq!(frame.client(), Rect::new(Point::new(2, 20), SIZE));
        assert_eq!(
            frame.decoration.map(|decoration| decoration.tabs()),
            Some(1)
        );
    }

    #[test]
    fn a_window_a_rule_leaves_undecorated_fills_its_frame() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![rule(
            "dock",
            Effects {
                decorations: Some(false),
                ..Effects::default()
            },
        )]);
        let id = open(&mut desktop, "dock");
        let frame = frame(&desktop, id);
        assert_eq!(frame.decoration, None);
        assert_eq!(frame.client(), frame.geometry);
        assert_eq!(frame.geometry.size, SIZE);
    }

    #[test]
    fn a_window_that_draws_its_own_titlebar_is_left_alone_unless_a_rule_insists() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![rule(
            "forced",
            Effects {
                decorations: Some(true),
                ..Effects::default()
            },
        )]);
        let own = desktop.add_window("gtk", "", SIZE, POINTER, false);
        let forced = desktop.add_window("forced", "", SIZE, POINTER, false);
        assert_eq!(frame(&desktop, own).decoration, None);
        assert!(frame(&desktop, forced).decoration.is_some());
    }

    #[test]
    fn a_window_can_gain_or_lose_its_decoration_later() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let outer = frame(&desktop, id).geometry;

        desktop.set_decorated(id, false);
        assert_eq!(frame(&desktop, id).decoration, None);
        assert_eq!(
            frame(&desktop, id).client(),
            outer,
            "the frame keeps its place"
        );

        desktop.set_decorated(id, true);
        assert_eq!(frame(&desktop, id).client().size, SIZE);
    }

    #[test]
    fn a_frame_with_several_tabs_is_decorated_even_if_its_windows_are_not() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![rule(
            "*",
            Effects {
                decorations: Some(false),
                ..Effects::default()
            },
        )]);
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);
        assert_eq!(
            frame(&desktop, a)
                .decoration
                .map(|decoration| decoration.tabs()),
            Some(2)
        );
    }

    #[test]
    fn a_maximized_frame_keeps_its_decoration_and_a_fullscreen_one_does_not() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");

        desktop.set_maximized(id, true);
        let inside = Rect::new(Point::new(2, 20), Size::new(1916, 1058));
        assert_eq!(frame(&desktop, id).client(), inside);

        desktop.set_fullscreen(id, true);
        assert_eq!(frame(&desktop, id).decoration, None);
        assert_eq!(frame(&desktop, id).client(), SCREEN);
    }

    #[test]
    fn a_window_that_resizes_itself_resizes_its_frame() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "terminal");
        desktop.set_client_size(id, Size::new(640, 480));
        assert_eq!(frame(&desktop, id).client().size, Size::new(640, 480));
        assert_eq!(frame(&desktop, id).geometry.size, Size::new(644, 502));

        desktop.set_decorated(id, false);
        desktop.set_client_size(id, Size::new(100, 100));
        assert_eq!(frame(&desktop, id).geometry.size, Size::new(100, 100));
    }

    #[test]
    fn a_window_cannot_resize_a_maximized_or_fullscreen_frame_or_one_showing_another_tab() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let normal = frame(&desktop, id).geometry;

        desktop.set_maximized(id, true);
        desktop.set_client_size(id, Size::new(10, 10));
        desktop.set_maximized(id, false);
        desktop.set_fullscreen(id, true);
        desktop.set_client_size(id, Size::new(10, 10));
        desktop.set_fullscreen(id, false);
        assert_eq!(frame(&desktop, id).geometry, normal);

        let other = open(&mut desktop, "other");
        desktop.attach(other, id);
        desktop.set_client_size(id, Size::new(10, 10));
        assert_eq!(
            frame(&desktop, id).geometry,
            normal,
            "`other` is the shown tab"
        );
    }

    #[test]
    fn fullscreen_frames_are_drawn_above_the_others() {
        let mut desktop = desktop();
        let video = open(&mut desktop, "video");
        let chat = open(&mut desktop, "chat");
        desktop.set_fullscreen(video, true);
        assert_eq!(stack(&desktop), [chat, video]);
        assert_eq!(
            desktop.focused(),
            Some(chat),
            "stacking does not move focus"
        );
    }

    // --- Frame state -------------------------------------------------------

    #[test]
    fn a_maximized_frame_fills_its_output_and_restores_afterwards() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let normal = frame(&desktop, id).geometry;

        desktop.set_maximized(id, true);
        assert!(frame(&desktop, id).maximized);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);

        desktop.set_maximized(id, false);
        assert_eq!(frame(&desktop, id).geometry, normal);

        desktop.toggle_maximized(id);
        assert!(frame(&desktop, id).maximized);
        desktop.toggle_maximized(id);
        assert!(!frame(&desktop, id).maximized);
    }

    #[test]
    fn a_frame_maximizes_onto_the_output_it_mostly_covers() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = open(&mut desktop, "app");

        desktop.set_geometry(id, Rect::new(Point::new(1800, 100), SIZE));
        desktop.set_maximized(id, true);
        assert_eq!(frame(&desktop, id).geometry, SECOND_SCREEN);

        desktop.set_geometry(id, Rect::new(Point::new(1000, 100), SIZE));
        desktop.set_maximized(id, true);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
    }

    #[test]
    fn a_frame_touching_no_output_maximizes_onto_the_first() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = open(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(Point::new(-5000, -5000), SIZE));
        desktop.set_maximized(id, true);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
    }

    #[test]
    fn without_outputs_a_maximized_frame_keeps_its_own_geometry() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        let id = open(&mut desktop, "app");
        desktop.set_maximized(id, true);
        assert_eq!(
            frame(&desktop, id).geometry,
            Rect::new(Point::new(0, 0), SIZE)
        );
    }

    #[test]
    fn fullscreen_covers_the_output_and_returns_to_the_previous_state() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.set_maximized(id, true);

        desktop.set_fullscreen(id, true);
        assert!(frame(&desktop, id).fullscreen);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);

        desktop.set_fullscreen(id, false);
        assert!(frame(&desktop, id).maximized, "it was maximized before");

        desktop.set_maximized(id, false);
        desktop.toggle_fullscreen(id);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
        desktop.toggle_fullscreen(id);
        assert_eq!(frame(&desktop, id).geometry.size, SIZE);
    }

    #[test]
    fn moving_or_resizing_a_frame_leaves_maximized_and_fullscreen() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.set_maximized(id, true);
        desktop.set_fullscreen(id, true);

        let geometry = Rect::new(Point::new(50, 60), Size::new(300, 200));
        desktop.set_geometry(id, geometry);

        let frame = frame(&desktop, id);
        assert_eq!(
            (frame.geometry, frame.maximized, frame.fullscreen),
            (geometry, false, false)
        );
    }

    #[test]
    fn minimizing_hides_a_frame_and_passes_focus_on() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");

        desktop.minimize(second);
        assert_eq!(stack(&desktop), [first]);
        assert_eq!(desktop.focused(), Some(first));
        assert!(desktop.window(second).is_some(), "the window still exists");

        desktop.minimize(first);
        assert_eq!(desktop.focused(), None);
    }

    #[test]
    fn minimizing_an_unfocused_frame_leaves_focus_alone() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.minimize(first);
        assert_eq!(desktop.focused(), Some(second));
    }

    #[test]
    fn activating_a_minimized_window_restores_focuses_and_raises_it() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.minimize(first);

        desktop.activate(first);
        assert_eq!(stack(&desktop), [second, first]);
        assert_eq!(desktop.focused(), Some(first));
    }

    // --- Focus -------------------------------------------------------------

    #[test]
    fn clicking_a_window_focuses_and_raises_it() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.clicked(first);
        assert_eq!(desktop.focused(), Some(first));
        assert_eq!(stack(&desktop), [second, first]);
    }

    #[test]
    fn clicking_focuses_without_raising_when_raise_on_focus_is_off() {
        let mut desktop = desktop();
        desktop.set_settings(Settings {
            raise_on_focus: false,
            ..Settings::default()
        });
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.clicked(first);
        assert_eq!(desktop.focused(), Some(first));
        assert_eq!(stack(&desktop), [first, second]);
    }

    #[test]
    fn the_pointer_does_not_move_focus_under_click_to_focus() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.pointer_entered(first);
        assert_eq!(desktop.focused(), Some(second));
    }

    #[test]
    fn the_pointer_moves_focus_when_focus_follows_it() {
        let mut desktop = desktop();
        let sloppy = Settings {
            focus_follows_pointer: true,
            ..Settings::default()
        };
        desktop.set_settings(sloppy);
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");

        desktop.pointer_entered(first);
        assert_eq!(desktop.focused(), Some(first));
        assert_eq!(stack(&desktop), [second, first]);

        desktop.set_settings(Settings {
            raise_on_focus: false,
            ..sloppy
        });
        desktop.pointer_entered(second);
        assert_eq!(desktop.focused(), Some(second));
        assert_eq!(
            stack(&desktop),
            [second, first],
            "focused without being raised"
        );
    }

    #[test]
    fn focus_cycles_through_frames_in_the_order_they_were_created() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");

        let mut visited = Vec::new();
        for _ in 0..4 {
            desktop.focus_next();
            visited.push(desktop.focused().unwrap());
        }
        assert_eq!(
            visited,
            [first, second, third, first],
            "raising does not disturb the order"
        );
        assert_eq!(
            stack(&desktop).last(),
            Some(&first),
            "the focused frame is raised"
        );

        visited.clear();
        for _ in 0..3 {
            desktop.focus_previous();
            visited.push(desktop.focused().unwrap());
        }
        assert_eq!(visited, [third, second, first]);
    }

    #[test]
    fn focus_cycling_skips_minimized_frames() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");
        desktop.minimize(second);
        desktop.clicked(first);

        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(third));
        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(first));
    }

    #[test]
    fn focus_cycling_with_one_or_no_frames_changes_nothing() {
        let mut desktop = desktop();
        desktop.focus_next();
        desktop.focus_previous();
        assert_eq!(desktop.focused(), None);

        let only = open(&mut desktop, "only");
        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(only));
    }

    #[test]
    fn activating_a_window_switches_to_its_workspace() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.switch_workspace(2);
        desktop.activate(id);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(desktop.focused(), Some(id));
    }

    // --- Tabs --------------------------------------------------------------

    #[test]
    fn attaching_a_window_makes_it_the_shown_tab_of_the_target_frame() {
        let mut desktop = desktop();
        let editor = open(&mut desktop, "editor");
        let terminal = open(&mut desktop, "terminal");
        let other = open(&mut desktop, "other");
        let editor_geometry = frame(&desktop, editor).geometry;

        desktop.attach(terminal, editor);

        assert_eq!(desktop.frames().count(), 2);
        let frame = frame(&desktop, editor);
        let tabs: Vec<WindowId> = frame.tabs.iter().map(|tab| tab.id).collect();
        assert_eq!(tabs, [editor, terminal]);
        assert_eq!(frame.window().id, terminal);
        assert_eq!(
            frame.geometry, editor_geometry,
            "the window takes the frame's position"
        );
        assert_eq!(desktop.focused(), Some(terminal));
        assert_eq!(stack(&desktop), [other, terminal], "the frame is raised");
    }

    #[test]
    fn attaching_a_window_to_its_own_frame_does_nothing() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        desktop.attach(second, first);
        let before = desktop.clone();

        desktop.attach(first, second);
        desktop.attach(first, first);
        assert_eq!(desktop, before);
    }

    #[test]
    fn attaching_across_workspaces_moves_the_window_and_refocuses_the_source() {
        let mut desktop = desktop();
        let target = open(&mut desktop, "target");
        desktop.switch_workspace(1);
        let stays = open(&mut desktop, "stays");
        let moves = open(&mut desktop, "moves");

        desktop.attach(moves, target);

        assert_eq!(desktop.focused(), Some(stays));
        assert_eq!(desktop.workspace_of(moves), Some(0));
        desktop.switch_workspace(0);
        assert_eq!(desktop.focused(), Some(moves));
    }

    #[test]
    fn tabs_cycle_in_both_directions_and_focus_follows_the_shown_tab() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        desktop.attach(b, a);
        desktop.attach(c, a);
        assert_eq!(desktop.focused(), Some(c));

        desktop.next_tab(c);
        assert_eq!(desktop.focused(), Some(a), "wrapped to the first tab");
        desktop.next_tab(a);
        assert_eq!(desktop.focused(), Some(b));
        desktop.previous_tab(b);
        assert_eq!(desktop.focused(), Some(a));
        desktop.previous_tab(a);
        assert_eq!(desktop.focused(), Some(c), "wrapped to the last tab");
    }

    #[test]
    fn clicking_or_activating_a_tab_shows_it() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);

        desktop.clicked(a);
        assert_eq!(desktop.focused(), Some(a));
        desktop.activate(b);
        assert_eq!(desktop.focused(), Some(b));
    }

    #[test]
    fn closing_a_tab_keeps_the_frame_and_picks_a_sensible_tab_to_show() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        let d = open(&mut desktop, "d");
        desktop.attach(b, a);
        desktop.attach(c, a);
        desktop.attach(d, a);
        desktop.activate(c);

        desktop.remove_window(a);
        assert_eq!(
            desktop.focused(),
            Some(c),
            "closing an earlier tab keeps the shown one"
        );

        desktop.remove_window(c);
        assert_eq!(
            desktop.focused(),
            Some(d),
            "closing the shown tab shows its right neighbour"
        );

        desktop.remove_window(d);
        assert_eq!(
            desktop.focused(),
            Some(b),
            "closing the last tab shows the new last one"
        );
        assert_eq!(desktop.frames().count(), 1);
    }

    #[test]
    fn detaching_a_tab_gives_it_a_focused_frame_just_off_the_old_one() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);
        desktop.set_maximized(a, true);
        let origin = Point::new(0, 0);

        desktop.detach(a);

        assert_eq!(stack(&desktop), [b, a]);
        assert_eq!(desktop.focused(), Some(a));
        let detached = frame(&desktop, a);
        assert_eq!(detached.geometry, Rect::new(origin.offset(32, 32), SIZE));
        assert!(!detached.maximized, "the new frame starts out normal");
        assert_eq!(frame(&desktop, b).tabs.len(), 1);
    }

    #[test]
    fn detaching_a_window_that_is_alone_in_its_frame_does_nothing() {
        let mut desktop = desktop();
        let only = open(&mut desktop, "only");
        let geometry = frame(&desktop, only).geometry;
        desktop.detach(only);
        assert_eq!(desktop.frames().count(), 1);
        assert_eq!(frame(&desktop, only).geometry, geometry);
    }

    // --- Workspaces --------------------------------------------------------

    #[test]
    fn each_workspace_has_its_own_frames_and_focus() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        desktop.switch_workspace(1);
        assert_eq!(desktop.active_workspace(), 1);
        assert_eq!(desktop.focused(), None);

        let second = open(&mut desktop, "second");
        assert_eq!(stack(&desktop), [second]);

        desktop.switch_workspace(0);
        assert_eq!(stack(&desktop), [first]);
        assert_eq!(desktop.focused(), Some(first));
    }

    #[test]
    fn switching_to_a_workspace_that_does_not_exist_is_ignored() {
        let mut desktop = desktop();
        desktop.switch_workspace(3);
        assert_eq!(desktop.active_workspace(), 0);
    }

    #[test]
    fn next_and_previous_workspace_wrap_around() {
        let mut desktop = desktop();
        desktop.previous_workspace();
        assert_eq!(desktop.active_workspace(), 2);
        desktop.next_workspace();
        assert_eq!(desktop.active_workspace(), 0);
        desktop.next_workspace();
        assert_eq!(desktop.active_workspace(), 1);
    }

    #[test]
    fn moving_a_frame_to_another_workspace_takes_every_tab_and_stays_put() {
        let mut desktop = desktop();
        let stays = open(&mut desktop, "stays");
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        desktop.attach(b, a);

        desktop.move_to_workspace(a, 2);

        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [stays]);
        assert_eq!(desktop.focused(), Some(stays));
        assert_eq!(
            (desktop.workspace_of(a), desktop.workspace_of(b)),
            (Some(2), Some(2))
        );

        desktop.switch_workspace(2);
        assert_eq!(desktop.focused(), Some(b));
    }

    #[test]
    fn a_minimized_frame_moved_to_another_workspace_stays_hidden_and_unfocused() {
        let mut desktop = desktop();
        let hidden = open(&mut desktop, "hidden");
        desktop.switch_workspace(1);
        let shown = open(&mut desktop, "shown");
        desktop.switch_workspace(0);
        desktop.minimize(hidden);

        desktop.move_to_workspace(hidden, 1);

        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [shown]);
        assert_eq!(desktop.focused(), Some(shown));
        desktop.activate(hidden);
        assert_eq!(desktop.focused(), Some(hidden));
    }

    #[test]
    fn moving_a_frame_to_its_own_or_a_missing_workspace_does_nothing() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let before = desktop.clone();
        desktop.move_to_workspace(id, 0);
        desktop.move_to_workspace(id, 3);
        assert_eq!(desktop, before);
    }

    #[test]
    fn removing_workspaces_moves_their_frames_to_the_last_one_left() {
        let mut desktop = desktop();
        let kept = open(&mut desktop, "kept");
        desktop.switch_workspace(2);
        let orphan = open(&mut desktop, "orphan");

        desktop.set_workspace_count(2);

        assert_eq!(desktop.workspace_count(), 2);
        assert_eq!(
            desktop.active_workspace(),
            1,
            "the shown workspace no longer exists"
        );
        assert_eq!(desktop.workspace_of(orphan), Some(1));
        assert_eq!(desktop.focused(), Some(orphan));
        assert_eq!(desktop.workspace_of(kept), Some(0));
    }

    #[test]
    fn workspaces_can_be_added_and_never_drop_below_one() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");

        desktop.set_workspace_count(5);
        assert_eq!(desktop.workspace_count(), 5);
        assert_eq!(desktop.workspace_of(id), Some(0));

        desktop.set_workspace_count(0);
        assert_eq!(desktop.workspace_count(), 1);
        assert_eq!(desktop.focused(), Some(id));
    }

    // --- Outputs -----------------------------------------------------------

    #[test]
    fn frames_left_off_screen_move_to_the_first_output() {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let stays = open(&mut desktop, "stays");
        let stranded = open(&mut desktop, "stranded");
        desktop.switch_workspace(1);
        let hidden = open(&mut desktop, "hidden");
        desktop.set_geometry(stranded, Rect::new(Point::new(2500, 100), SIZE));
        desktop.set_geometry(hidden, Rect::new(Point::new(2500, 100), SIZE));
        let stays_at = Rect::new(Point::new(0, 0), SIZE);

        desktop.set_outputs(vec![SCREEN]);

        desktop.switch_workspace(0);
        assert_eq!(frame(&desktop, stays).geometry, stays_at);
        assert_eq!(
            frame(&desktop, stranded).geometry,
            Rect::new(SCREEN.origin, SIZE)
        );
        desktop.switch_workspace(1);
        assert_eq!(frame(&desktop, hidden).geometry.origin, SCREEN.origin);
    }

    #[test]
    fn removing_every_output_leaves_frames_where_they_are() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let geometry = Rect::new(Point::new(300, 200), SIZE);
        desktop.set_geometry(id, geometry);
        desktop.set_outputs(Vec::new());
        assert_eq!(frame(&desktop, id).geometry, geometry);
    }

    // --- Robustness --------------------------------------------------------

    #[test]
    fn events_for_a_window_that_has_closed_are_ignored() {
        let mut desktop = desktop();
        let open_window = open(&mut desktop, "open");
        let closed = open(&mut desktop, "closed");
        desktop.remove_window(closed);
        let before = desktop.clone();

        desktop.remove_window(closed);
        desktop.set_title(closed, "ghost");
        desktop.set_decorated(closed, false);
        desktop.set_client_size(closed, SIZE);
        desktop.set_geometry(closed, SCREEN);
        desktop.set_maximized(closed, true);
        desktop.toggle_maximized(closed);
        desktop.set_fullscreen(closed, true);
        desktop.toggle_fullscreen(closed);
        desktop.minimize(closed);
        desktop.activate(closed);
        desktop.clicked(closed);
        desktop.pointer_entered(closed);
        desktop.next_tab(closed);
        desktop.previous_tab(closed);
        desktop.attach(closed, open_window);
        desktop.attach(open_window, closed);
        desktop.detach(closed);
        desktop.move_to_workspace(closed, 1);

        assert_eq!(desktop, before);
        assert_eq!(desktop.window(closed), None);
        assert_eq!(desktop.workspace_of(closed), None);
    }

    /// Checks what must hold after any sequence of operations.
    fn assert_consistent(desktop: &Desktop) {
        assert!(desktop.active < desktop.workspaces.len());
        for workspace in &desktop.workspaces {
            let focusable: Vec<u64> = workspace
                .groups
                .iter()
                .filter(|group| !group.minimized)
                .map(|group| group.serial)
                .collect();
            match workspace.focused {
                Some(serial) => assert!(focusable.contains(&serial), "focus is on a shown frame"),
                None => assert_eq!(focusable, [], "a shown frame exists but nothing is focused"),
            }
            for group in &workspace.groups {
                assert!(group.active < group.tabs.len(), "the shown tab exists");
            }
        }
        let mut ids: Vec<WindowId> = desktop
            .workspaces
            .iter()
            .flat_map(|workspace| &workspace.groups)
            .flat_map(|group| &group.tabs)
            .map(|window| window.id)
            .collect();
        let total = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), total, "every window is in exactly one frame");
    }

    #[test]
    fn the_desktop_stays_consistent_under_arbitrary_sequences_of_operations() {
        // A small linear congruential generator keeps the run reproducible
        // without a dependency.
        let mut state: u64 = 0x5eed;
        let mut random = |bound: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            usize::try_from(state >> 33).unwrap() % bound
        };
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let mut ids = vec![open(&mut desktop, "seed")];

        for step in 0..5000 {
            let id = ids[random(ids.len())];
            let other = ids[random(ids.len())];
            match random(22) {
                0 | 1 => ids.push(open(&mut desktop, "app")),
                2 => desktop.remove_window(id),
                3 => desktop.minimize(id),
                4 => desktop.activate(id),
                5 => desktop.clicked(id),
                6 => desktop.attach(id, other),
                7 => desktop.detach(id),
                8 => desktop.next_tab(id),
                9 => desktop.previous_tab(id),
                10 => desktop.move_to_workspace(id, random(4)),
                11 => desktop.switch_workspace(random(4)),
                12 => desktop.focus_next(),
                13 => desktop.focus_previous(),
                14 => desktop.set_workspace_count(1 + random(4)),
                15 => desktop.toggle_maximized(id),
                16 => desktop.toggle_fullscreen(id),
                17 => desktop.next_workspace(),
                19 => desktop.set_decorated(id, random(2) == 0),
                20 => desktop.set_client_size(id, Size::new(100, 100)),
                18 => desktop.previous_workspace(),
                _ => desktop.set_outputs(vec![SCREEN; random(3)]),
            }
            assert_consistent(&desktop);
            assert!(
                desktop.frames().filter(|frame| frame.focused).count() <= 1,
                "step {step}"
            );
        }
    }
}
