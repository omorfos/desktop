//! The state of the session: windows, tab groups, workspaces and focus.

use crate::decoration::{Decoration, Metrics};
use crate::geometry::{Point, Rect, Size};
use crate::placement::{CASCADE_STEP, Placement, place};
use crate::rules::{Rule, resolve};

/// Identifies a window for as long as it exists. Never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(u64);

impl WindowId {
    /// The identifier as a plain number, for showing to users and sending
    /// to other programs.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Rebuilds an identifier from a number another program sent back.
    ///
    /// Nothing guarantees such a window exists; the desktop ignores
    /// identifiers it does not know.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

/// A window together with where it is, as listed by [`Desktop::windows`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed<'a> {
    /// The window.
    pub window: &'a Window,
    /// The output whose set of workspaces it is in, counted in the order
    /// the outputs were given. Zero while there is no output.
    pub output: usize,
    /// The zero-based workspace it is on, among that output's.
    pub workspace: usize,
    /// Whether its frame is minimized.
    pub minimized: bool,
    /// Whether it is the tab its frame is showing.
    pub shown: bool,
    /// Whether it has keyboard focus.
    pub focused: bool,
}

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

/// An output: a monitor, or the window a nested compositor draws into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Screen {
    /// The whole output, in the global space.
    pub area: Rect,
    /// The part of it left for windows once panels and docks have reserved
    /// their edges. New windows are placed in it and maximized windows fill
    /// it; a fullscreen window covers the whole area.
    pub usable: Rect,
}

impl From<Rect> for Screen {
    /// An output with nothing reserved: all of it is usable.
    fn from(area: Rect) -> Self {
        Self { area, usable: area }
    }
}

/// How a window that rules open maximized or fullscreen should first be
/// told to draw itself, as answered by [`Desktop::opening`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Opening {
    /// The size the window itself will have.
    pub size: Size,
    /// Whether it opens maximized.
    pub maximized: bool,
    /// Whether it opens fullscreen.
    pub fullscreen: bool,
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
    /// How it stands apart from the others, as the user asked.
    pub standing: Standing,
}

/// How a frame stands apart from the others, each as the user asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Standing {
    /// Rolled up into its titlebar, which is all that shows of it. A
    /// frame without a titlebar cannot be.
    pub shaded: bool,
    /// Kept above the frames that are not, whichever has focus.
    pub above: bool,
    /// On whichever workspace its output is showing.
    pub sticky: bool,
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
    standing: Standing,
}

impl Group {
    /// Where it is among the frames of its workspace, whatever order they
    /// were raised in: those kept above are over those that are not, and
    /// fullscreen ones over both.
    fn level(&self) -> u8 {
        u8::from(self.standing.above) + 2 * u8::from(self.fullscreen)
    }

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

/// The workspaces of one output, and which of them it is showing.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Set {
    /// Never empty.
    workspaces: Vec<Workspace>,
    active: usize,
}

impl Set {
    fn new(workspaces: usize) -> Self {
        Self {
            workspaces: vec![Workspace::default(); workspaces],
            active: 0,
        }
    }

    /// Takes on everything in `other`, workspace by workspace, on top of
    /// what is here. Workspaces `other` has more of go to the last.
    fn absorb(&mut self, other: Self) {
        let last = self.workspaces.len() - 1;
        for (index, workspace) in other.workspaces.into_iter().enumerate() {
            let onto = &mut self.workspaces[index.min(last)];
            onto.groups.extend(workspace.groups);
            onto.repair();
        }
    }
}

/// Where a window is: its output's set, its workspace there, its group and
/// its tab.
#[derive(Clone, Copy)]
struct Location {
    set: usize,
    workspace: usize,
    group: usize,
    tab: usize,
}

/// The state of the session.
///
/// Windows live in frames (tab groups of one or more) and frames live on
/// workspaces. Each output has a set of workspaces of its own and shows one
/// of them, so switching workspace on one screen leaves the others as they
/// are. One output is the current one: its focused frame has the keyboard,
/// and it is the one workspace switching applies to.
///
/// Frames are positioned in the one space all the outputs show. A frame
/// belongs to an output's set, and follows that output if it moves.
/// Methods that take a [`WindowId`] do nothing if the window is unknown, so
/// a late event for a window that has just closed is harmless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Desktop {
    settings: Settings,
    rules: Vec<Rule>,
    outputs: Vec<Screen>,
    /// One set of workspaces for each output, in the same order, or a
    /// single set while there is no output. Never empty.
    sets: Vec<Set>,
    /// The set that has the keyboard.
    current: usize,
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
            sets: vec![Set::new(workspaces.max(1))],
            current: 0,
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

    /// Replaces the outputs. A plain rectangle stands for an output that
    /// is usable in full.
    ///
    /// Outputs are told apart by their place in the list: the first given
    /// is the first there was, and so on. One more than before is a new
    /// output, with an empty set of workspaces. Fewer means the last ones
    /// have gone; to take away another, say which with
    /// [`Desktop::remove_output`] first.
    ///
    /// A frame keeps its place on its output if the output moves, and one
    /// left entirely off its output is moved onto it.
    pub fn set_outputs<S: Into<Screen>>(&mut self, outputs: impl IntoIterator<Item = S>) {
        let outputs: Vec<Screen> = outputs.into_iter().map(Into::into).collect();
        while self.sets.len() > outputs.len().max(1) {
            self.remove_set(self.sets.len() - 1);
        }
        let count = self.workspace_count();
        self.sets
            .resize_with(outputs.len().max(1), || Set::new(count));
        for (index, (set, output)) in self.sets.iter_mut().zip(&outputs).enumerate() {
            // Where the output was, if it was anywhere.
            let before = self.outputs.get(index).map(|output| output.area.origin);
            let moved = before.map_or((0, 0), |before| {
                let now = output.area.origin;
                (
                    now.x.saturating_sub(before.x),
                    now.y.saturating_sub(before.y),
                )
            });
            let groups = set.workspaces.iter_mut().flat_map(|on| &mut on.groups);
            for group in groups {
                group.geometry.origin = group.geometry.origin.offset(moved.0, moved.1);
                if output.area.intersection(group.geometry).is_none() {
                    group.geometry.origin = output.usable.origin;
                }
            }
        }
        self.outputs = outputs;
    }

    /// Takes away the output at `index`, counted in the order the outputs
    /// were given. Its frames move to the first output that remains, each
    /// to the workspace of the same number, and its place in the list is
    /// closed up. Does nothing if there is no such output.
    pub fn remove_output(&mut self, index: usize) {
        if index < self.outputs.len() {
            self.outputs.remove(index);
            self.remove_set(index);
            // The frames that moved are placed by the next `set_outputs`,
            // which the outputs' new positions will come with. Until then
            // they are on their new output or off every one.
            let outputs = std::mem::take(&mut self.outputs);
            self.set_outputs(outputs);
        }
    }

    /// Removes a set of workspaces, giving its frames to the first of the
    /// sets that remain. The last set there is stays.
    fn remove_set(&mut self, index: usize) {
        if self.sets.len() > 1 {
            let gone = self.sets.remove(index);
            self.sets[0].absorb(gone);
            self.current = match self.current {
                current if current == index => 0,
                current if current > index => current - 1,
                current => current,
            };
        }
    }

    /// Changes the number of workspaces each output has (at least one).
    ///
    /// Frames on removed workspaces move to the last remaining one of the
    /// same output.
    pub fn set_workspace_count(&mut self, count: usize) {
        let last = count.max(1) - 1;
        for set in &mut self.sets {
            let orphans: Vec<Group> = set
                .workspaces
                .iter_mut()
                .skip(last + 1)
                .flat_map(|workspace| workspace.groups.drain(..))
                .collect();
            set.workspaces.resize_with(last + 1, Workspace::default);
            set.workspaces[last].groups.extend(orphans);
            set.workspaces[last].repair();
            set.active = set.active.min(last);
        }
    }

    // --- Windows -----------------------------------------------------------

    /// Adds a new window in a frame of its own and returns its identifier.
    ///
    /// `size` is the size of the window itself; the frame is larger by its
    /// decoration. `wants_decoration` says whether the window expects the
    /// compositor to draw its titlebar and border; a rule overrides it
    /// either way. It is placed on the output under `pointer`, on the
    /// workspace that output is showing unless a rule names another of its
    /// workspaces, and becomes the focused, topmost frame there. That
    /// output becomes the current one.
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
        let set = self.set_at(pointer);
        let last = self.workspace_count() - 1;
        let workspace = effects
            .workspace
            .map_or(self.sets[set].active, |workspace| workspace.min(last));
        let decorated = effects.decorations.unwrap_or(wants_decoration);
        let area = self.output_at(pointer).usable;
        let fills = effects.maximized == Some(true) || effects.fullscreen == Some(true);
        let size = match (fills, decorated) {
            // The window drew itself to fill the output, so its own idea of
            // a size was never seen. When restored it gets a moderate one.
            (true, _) => Size::new(area.size.width / 3 * 2, area.size.height / 3 * 2),
            (false, true) => self.settings.decoration.outer_size(size),
            (false, false) => size,
        };
        let index = self.sets[set].workspaces[workspace].groups.len();
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
            standing: Standing::default(),
        };
        self.push_focused(set, workspace, group);
        self.current = set;
        id
    }

    /// Says how a window about to be added should first draw itself, if
    /// rules open it maximized or fullscreen; `None` if it may choose its
    /// own size.
    ///
    /// Asking before the window has drawn anything lets it appear at its
    /// final size, instead of at its natural size for one frame.
    #[must_use]
    pub fn opening(
        &self,
        app_id: &str,
        title: &str,
        pointer: Point,
        wants_decoration: bool,
    ) -> Option<Opening> {
        let effects = resolve(&self.rules, app_id, title);
        let fullscreen = effects.fullscreen == Some(true);
        let maximized = effects.maximized == Some(true);
        let screen = self.output_at(pointer);
        let decorated = effects.decorations.unwrap_or(wants_decoration);
        let size = match (fullscreen, decorated) {
            (true, _) => screen.area.size,
            (false, true) => {
                Decoration::new(screen.usable, self.settings.decoration, 1)
                    .client()
                    .size
            }
            (false, false) => screen.usable.size,
        };
        (fullscreen || maximized).then_some(Opening {
            size,
            maximized,
            fullscreen,
        })
    }

    /// Removes a window, for example because it closed.
    pub fn remove_window(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.sets[at.set].workspaces[at.workspace].groups[at.group].remove(at.tab);
        self.sets[at.set].workspaces[at.workspace].repair();
    }

    /// Updates a window's title.
    pub fn set_title(&mut self, id: WindowId, title: impl Into<String>) {
        let Some(at) = self.locate(id) else { return };
        self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs[at.tab].title =
            title.into();
    }

    /// Looks a window up.
    #[must_use]
    pub fn window(&self, id: WindowId) -> Option<&Window> {
        self.locate(id)
            .map(|at| &self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs[at.tab])
    }

    /// Every window on every workspace of every output, in the order frames
    /// were created and then tab order: a stable order suited to a task
    /// list.
    pub fn windows(&self) -> impl Iterator<Item = Listed<'_>> {
        let workspaces = self.sets.iter().enumerate().flat_map(|(output, set)| {
            let on = set.workspaces.iter().enumerate();
            on.map(move |(index, workspace)| (output, index, workspace))
        });
        let mut groups: Vec<(usize, usize, &Workspace, &Group)> = workspaces
            .flat_map(|(output, index, workspace)| {
                let groups = workspace.groups.iter();
                groups.map(move |group| (output, index, workspace, group))
            })
            .collect();
        groups.sort_by_key(|(_, _, _, group)| group.serial);
        let current = (self.current, self.sets[self.current].active);
        groups
            .into_iter()
            .flat_map(move |(output, index, workspace, group)| {
                let focused = (output, index) == current && workspace.focused == Some(group.serial);
                group
                    .tabs
                    .iter()
                    .enumerate()
                    .map(move |(tab, window)| Listed {
                        window,
                        output,
                        workspace: index,
                        minimized: group.minimized,
                        shown: tab == group.active,
                        focused: focused && tab == group.active,
                    })
            })
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
        self.sets[at.set].workspaces[at.workspace].groups[at.group].tabs[at.tab].decorated =
            decorated;
    }

    // --- Frame state -------------------------------------------------------

    /// Records that a window resized itself to `size`, and resizes its frame
    /// to fit. Ignored while the frame is maximized or fullscreen, when its
    /// size is not the window's to choose, and for a tab that is not shown.
    pub fn set_client_size(&mut self, id: WindowId, size: Size) {
        let Some(at) = self.locate(id) else { return };
        let metrics = self.settings.decoration;
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        if group.active == at.tab && !group.maximized && !group.fullscreen {
            group.geometry.size = if group.decorated() {
                metrics.outer_size(size)
            } else {
                size
            };
        }
    }

    /// Moves and resizes a window's frame, leaving maximized and fullscreen.
    ///
    /// A frame moved so that most of it is on another output now belongs
    /// there: it joins the workspace that output is showing, and that
    /// output becomes the current one.
    pub fn set_geometry(&mut self, id: WindowId, geometry: Rect) {
        let Some(at) = self.locate(id) else { return };
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        group.geometry = geometry;
        group.maximized = false;
        group.fullscreen = false;
        let onto = self.set_for(geometry).unwrap_or(at.set);
        if onto != at.set {
            self.rehome(at, onto);
        }
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

    /// Rolls a window's frame up into its titlebar, or lets it down again.
    pub fn toggle_shaded(&mut self, id: WindowId) {
        self.stand(id, |standing| standing.shaded = !standing.shaded);
    }

    /// Keeps a window's frame above those that are not kept so, or stops.
    pub fn toggle_above(&mut self, id: WindowId) {
        self.stand(id, |standing| standing.above = !standing.above);
    }

    /// Has a window's frame on whichever workspace its output is showing,
    /// or leaves it on the one it is on.
    pub fn toggle_sticky(&mut self, id: WindowId) {
        self.stand(id, |standing| standing.sticky = !standing.sticky);
    }

    /// Changes how a window's frame stands apart from the others.
    fn stand(&mut self, id: WindowId, change: impl FnOnce(&mut Standing)) {
        if let Some(group) = self.group_mut(id) {
            change(&mut group.standing);
        }
    }

    /// Hides a window's frame. Focus passes to the topmost remaining frame.
    /// [`Desktop::activate`] brings it back.
    pub fn minimize(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.sets[at.set].workspaces[at.workspace].groups[at.group].minimized = true;
        self.sets[at.set].workspaces[at.workspace].repair();
    }

    // --- Focus -------------------------------------------------------------

    /// Brings a window to the user's attention: switches to its workspace,
    /// restores it if minimized, shows its tab, focuses and raises it.
    pub fn activate(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.turn(at.set, at.workspace);
        self.current = at.set;
        // It may have been carried along with what its output is showing,
        // and is then somewhere else than it was found.
        let at = self.locate(id).unwrap_or(at);
        self.show(at, true);
    }

    /// Reports a click on a window, which focuses it and shows its tab.
    pub fn clicked(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        self.current = at.set;
        self.show(at, self.settings.raise_on_focus);
    }

    /// Reports the pointer moving onto a window, which focuses it when focus
    /// follows the pointer.
    pub fn pointer_entered(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        if self.settings.focus_follows_pointer {
            self.current = at.set;
            self.sets[at.set].workspaces[at.workspace]
                .focus(at.group, self.settings.raise_on_focus);
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
    /// the workspace the current output is showing.
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
        if (from.set, from.workspace, from.group) == (to.set, to.workspace, to.group) {
            return;
        }
        let window =
            self.sets[from.set].workspaces[from.workspace].groups[from.group].remove(from.tab);
        let group = &mut self.sets[to.set].workspaces[to.workspace].groups[to.group];
        group.tabs.push(window);
        group.active = group.tabs.len() - 1;
        self.sets[to.set].workspaces[to.workspace].focus(to.group, true);
        self.sets[from.set].workspaces[from.workspace].repair();
    }

    /// Moves a window out of a shared frame into a frame of its own, just
    /// off the corner of the old one, and focuses it.
    pub fn detach(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        let serial = self.serial();
        let source = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
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
            standing: Standing::default(),
        };
        self.push_focused(at.set, at.workspace, group);
    }

    // --- Workspaces --------------------------------------------------------

    /// The number of workspaces each output has. At least one.
    #[must_use]
    pub fn workspace_count(&self) -> usize {
        self.sets[0].workspaces.len()
    }

    /// Makes the output at `point` the current one: the one whose focused
    /// frame has the keyboard, and which switching workspace applies to. A
    /// point on no output leaves things as they are.
    pub fn turn_to(&mut self, point: Point) {
        let containing = |output: &Screen| output.area.contains(point);
        if let Some(index) = self.outputs.iter().position(containing) {
            self.current = index;
        }
    }

    /// The output that is current, counted in the order the outputs were
    /// given. Zero while there is none.
    #[must_use]
    pub const fn current_output(&self) -> usize {
        self.current
    }

    /// The zero-based workspace the current output is showing.
    #[must_use]
    pub fn active_workspace(&self) -> usize {
        self.sets[self.current].active
    }

    /// The zero-based workspace each output is showing, in the order the
    /// outputs were given. One entry while there is no output.
    pub fn shown_workspaces(&self) -> impl Iterator<Item = usize> {
        self.sets.iter().map(|set| set.active)
    }

    /// Shows a workspace on the current output. Does nothing if it does not
    /// exist.
    pub fn switch_workspace(&mut self, workspace: usize) {
        if workspace < self.workspace_count() {
            self.turn(self.current, workspace);
        }
    }

    /// Shows the following workspace on the current output, wrapping
    /// around.
    pub fn next_workspace(&mut self) {
        let count = self.workspace_count();
        let next = (self.sets[self.current].active + 1) % count;
        self.turn(self.current, next);
    }

    /// Shows the preceding workspace on the current output, wrapping
    /// around.
    pub fn previous_workspace(&mut self) {
        let count = self.workspace_count();
        let previous = (self.sets[self.current].active + count - 1) % count;
        self.turn(self.current, previous);
    }

    /// Sends a window's whole frame to another workspace of its output,
    /// where it becomes the focused, topmost frame. The workspace being
    /// shown does not change.
    pub fn move_to_workspace(&mut self, id: WindowId, workspace: usize) {
        let Some(at) = self.locate(id) else { return };
        if workspace >= self.workspace_count() || workspace == at.workspace {
            return;
        }
        let mut group = self.sets[at.set].workspaces[at.workspace]
            .groups
            .remove(at.group);
        // Sent to one workspace, it is no longer on all of them.
        group.standing.sticky = false;
        self.sets[at.set].workspaces[at.workspace].repair();
        self.push_focused(at.set, workspace, group);
    }

    /// Sends a window's whole frame to the next output, wrapping around,
    /// keeping its place relative to the output's corner where that fits.
    /// It joins the workspace that output is showing, and that output
    /// becomes the current one. Does nothing with fewer than two outputs.
    pub fn move_to_next_output(&mut self, id: WindowId) {
        let Some(at) = self.locate(id) else { return };
        let onto = (at.set + 1) % self.sets.len();
        let (Some(from), Some(to)) = (self.outputs.get(at.set), self.outputs.get(onto)) else {
            return;
        };
        if onto == at.set {
            return;
        }
        let (from, to) = (from.usable, *to);
        let group = &mut self.sets[at.set].workspaces[at.workspace].groups[at.group];
        let within = (
            group.geometry.origin.x.saturating_sub(from.origin.x),
            group.geometry.origin.y.saturating_sub(from.origin.y),
        );
        group.geometry.origin = to.usable.origin.offset(within.0, within.1);
        if to.area.intersection(group.geometry).is_none() {
            group.geometry.origin = to.usable.origin;
        }
        self.rehome(at, onto);
    }

    // --- What to show ------------------------------------------------------

    /// The frames to draw, bottom to top: those on the workspace each
    /// output is showing. Minimized frames are left out, fullscreen frames
    /// are above all others, and the current output's are above the rest
    /// where frames of two outputs overlap.
    pub fn frames(&self) -> impl Iterator<Item = Frame<'_>> {
        // The current set last, so that its frames are on top.
        let order = (0..self.sets.len())
            .filter(|index| *index != self.current)
            .chain([self.current]);
        let shown = move |level: u8| {
            order.clone().flat_map(move |index| {
                let set = &self.sets[index];
                let workspace = &set.workspaces[set.active];
                let groups = workspace.groups.iter();
                groups
                    .filter(move |group| !group.minimized && group.level() == level)
                    .map(move |group| (index, workspace, group))
            })
        };
        (0..4).flat_map(shown).map(|(index, workspace, group)| {
            // A frame fills its own output; with none, it has only
            // itself to fill.
            let own = self.outputs.get(index).copied();
            let screen = own.unwrap_or_else(|| Screen::from(group.geometry));
            let mut geometry = match (group.fullscreen, group.maximized) {
                (true, _) => screen.area,
                (false, true) => screen.usable,
                (false, false) => group.geometry,
            };
            let decorated = group.decorated() && !group.fullscreen;
            // Rolled up, a frame is as tall as its titlebar and no more.
            let shaded = group.standing.shaded && decorated;
            let titlebar = self.settings.decoration.titlebar;
            let rolled = geometry.size.height.min(titlebar);
            geometry.size.height = if shaded { rolled } else { geometry.size.height };
            Frame {
                tabs: &group.tabs,
                active: group.active,
                geometry,
                decoration: decorated
                    .then(|| Decoration::new(geometry, self.settings.decoration, group.tabs.len())),
                maximized: group.maximized,
                fullscreen: group.fullscreen,
                focused: index == self.current && workspace.focused == Some(group.serial),
                standing: Standing {
                    shaded,
                    ..group.standing
                },
            }
        })
    }

    // --- Internals ---------------------------------------------------------

    const fn serial(&mut self) -> u64 {
        self.next_serial += 1;
        self.next_serial
    }

    fn locate(&self, id: WindowId) -> Option<Location> {
        self.sets.iter().enumerate().find_map(|(set, in_set)| {
            let workspaces = in_set.workspaces.iter().enumerate();
            workspaces.into_iter().find_map(|(workspace, on)| {
                on.groups.iter().enumerate().find_map(|(group, in_group)| {
                    let tab = in_group.tabs.iter().position(|window| window.id == id)?;
                    Some(Location {
                        set,
                        workspace,
                        group,
                        tab,
                    })
                })
            })
        })
    }

    fn group_mut(&mut self, id: WindowId) -> Option<&mut Group> {
        self.locate(id)
            .map(|at| &mut self.sets[at.set].workspaces[at.workspace].groups[at.group])
    }

    fn push_focused(&mut self, set: usize, workspace: usize, group: Group) {
        let workspace = &mut self.sets[set].workspaces[workspace];
        workspace.focused = Some(group.serial);
        workspace.groups.push(group);
        // A minimized frame can be moved here; it must not hold focus.
        workspace.repair();
    }

    /// Has an output show another of its workspaces. The frames that are on
    /// whichever workspace it shows go along, on top of what is there, and
    /// one of them that had focus keeps it.
    fn turn(&mut self, set: usize, to: usize) {
        let set = &mut self.sets[set];
        let from = std::mem::replace(&mut set.active, to);
        if from == to {
            return;
        }
        let leaving = &mut set.workspaces[from];
        let focused = leaving.focused;
        let carried = leaving.groups.extract_if(.., |group| group.standing.sticky);
        let carried: Vec<Group> = carried.collect();
        leaving.repair();
        let arriving = &mut set.workspaces[to];
        let mut kept = carried.iter().map(|group| group.serial);
        let kept = kept.find(|serial| Some(*serial) == focused);
        arriving.groups.extend(carried);
        arriving.focused = kept.or(arriving.focused);
        arriving.repair();
    }

    /// Gives a frame to another output's set, on the workspace that output
    /// is showing, focused and on top, and makes that output current.
    fn rehome(&mut self, at: Location, onto: usize) {
        let from = &mut self.sets[at.set].workspaces[at.workspace];
        let group = from.groups.remove(at.group);
        from.repair();
        let workspace = self.sets[onto].active;
        self.push_focused(onto, workspace, group);
        self.current = onto;
    }

    /// The set of the output at `point`, or failing that the first.
    fn set_at(&self, point: Point) -> usize {
        let containing = |output: &Screen| output.area.contains(point);
        self.outputs.iter().position(containing).unwrap_or(0)
    }

    /// The set of the output most of a frame at `geometry` lies on, if it
    /// lies on any.
    fn set_for(&self, geometry: Rect) -> Option<usize> {
        let overlap = |output: &Screen| {
            let shared = output.area.intersection(geometry);
            shared.map_or(0, |shared| shared.size.area())
        };
        // `max_by_key` keeps the last of equal maxima; reversing makes ties
        // resolve to the first output.
        let most = self.outputs.iter().enumerate().rev();
        most.max_by_key(|(_, output)| overlap(output))
            .filter(|(_, output)| overlap(output) > 0)
            .map(|(index, _)| index)
    }

    fn show(&mut self, at: Location, raise: bool) {
        let workspace = &mut self.sets[at.set].workspaces[at.workspace];
        workspace.groups[at.group].active = at.tab;
        workspace.focus(at.group, raise);
    }

    fn cycle_focus(&mut self, backwards: bool) {
        let set = &mut self.sets[self.current];
        let workspace = &mut set.workspaces[set.active];
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
    fn output_at(&self, point: Point) -> Screen {
        let containing = self
            .outputs
            .iter()
            .find(|output| output.area.contains(point));
        containing
            .or(self.outputs.first())
            .copied()
            .unwrap_or_default()
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
    fn a_window_that_rules_leave_alone_chooses_its_own_size() {
        let desktop = decorated_desktop();
        assert_eq!(desktop.opening("foot", "", POINTER, true), None);
    }

    #[test]
    fn a_window_a_rule_maximizes_is_told_the_size_it_will_fill() {
        let mut desktop = decorated_desktop();
        desktop.set_rules(vec![
            rule(
                "editor",
                Effects {
                    maximized: Some(true),
                    ..Effects::default()
                },
            ),
            rule(
                "bare",
                Effects {
                    maximized: Some(true),
                    decorations: Some(false),
                    ..Effects::default()
                },
            ),
        ]);

        let framed = desktop.opening("editor", "", POINTER, true).unwrap();
        assert_eq!(
            framed,
            Opening {
                size: Size::new(1916, 1058),
                maximized: true,
                fullscreen: false
            }
        );
        let bare = desktop.opening("bare", "", POINTER, true).unwrap();
        assert_eq!(bare.size, SCREEN.size, "no frame to leave room for");
        let own_titlebar = desktop.opening("editor", "", POINTER, false).unwrap();
        assert_eq!(own_titlebar.size, SCREEN.size);
    }

    #[test]
    fn a_window_a_rule_makes_fullscreen_is_told_the_whole_output() {
        let mut desktop = decorated_desktop();
        let usable = Rect::new(Point::new(0, 30), Size::new(1920, 1050));
        desktop.set_outputs([Screen {
            area: SCREEN,
            usable,
        }]);
        desktop.set_rules(vec![rule(
            "mpv",
            Effects {
                fullscreen: Some(true),
                ..Effects::default()
            },
        )]);

        let opening = desktop.opening("mpv", "", POINTER, true).unwrap();
        assert_eq!(
            opening,
            Opening {
                size: SCREEN.size,
                maximized: false,
                fullscreen: true
            }
        );
    }

    #[test]
    fn a_window_opened_filling_the_output_restores_to_a_moderate_size() {
        let mut desktop = desktop();
        desktop.set_rules(vec![rule(
            "editor",
            Effects {
                maximized: Some(true),
                ..Effects::default()
            },
        )]);
        let id = desktop.add_window("editor", "", SCREEN.size, POINTER, true);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);

        desktop.set_maximized(id, false);
        assert_eq!(
            frame(&desktop, id).geometry.size,
            Size::new(1280, 720),
            "two thirds of the output"
        );
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
    fn every_window_is_listed_with_where_it_is() {
        let mut desktop = desktop();
        let a = open(&mut desktop, "a");
        let b = open(&mut desktop, "b");
        let c = open(&mut desktop, "c");
        let d = open(&mut desktop, "d");
        desktop.attach(b, a);
        desktop.minimize(c);
        desktop.move_to_workspace(d, 2);

        let listed: Vec<_> = desktop
            .windows()
            .map(|entry| {
                (
                    entry.window.id,
                    entry.workspace,
                    entry.minimized,
                    entry.shown,
                    entry.focused,
                )
            })
            .collect();
        assert_eq!(
            listed,
            [
                (a, 0, false, false, false),
                (b, 0, false, true, true),
                (c, 0, true, true, false),
                (d, 2, false, true, false),
            ]
        );
    }

    #[test]
    fn a_window_focused_on_a_hidden_workspace_does_not_have_keyboard_focus() {
        let mut desktop = desktop();
        open(&mut desktop, "here");
        desktop.switch_workspace(1);
        assert!(desktop.windows().all(|entry| !entry.focused));
    }

    #[test]
    fn identifiers_survive_a_round_trip_through_a_number() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        assert_eq!(WindowId::from_raw(id.raw()), id);
        desktop.activate(WindowId::from_raw(9999));
        assert_eq!(desktop.focused(), Some(id), "an unknown number is ignored");
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
        buttons: [0; 3],
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

    /// A window opened with the pointer on the second output.
    fn open_on_second(desktop: &mut Desktop, app_id: &str) -> WindowId {
        let pointer = SECOND_SCREEN.origin.offset(10, 10);
        desktop.add_window(app_id, format!("{app_id} window"), SIZE, pointer, true)
    }

    /// A desktop with two outputs side by side.
    fn two_outputs() -> Desktop {
        let mut desktop = desktop();
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        desktop
    }

    fn shown(desktop: &Desktop) -> Vec<usize> {
        desktop.shown_workspaces().collect()
    }

    fn visible(desktop: &Desktop, id: WindowId) -> bool {
        desktop.frames().any(|frame| frame.window().id == id)
    }

    #[test]
    fn each_output_shows_a_workspace_of_its_own() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        assert_eq!(shown(&desktop), [0, 0]);
        assert_eq!(desktop.current_output(), 1, "where the last window opened");

        desktop.switch_workspace(1);
        assert_eq!(shown(&desktop), [0, 1], "only the second output changed");
        assert!(visible(&desktop, left));
        assert!(!visible(&desktop, right));
        assert_eq!(desktop.active_workspace(), 1);

        // Turning to the first output, switching applies there.
        desktop.turn_to(POINTER);
        assert_eq!(desktop.current_output(), 0);
        assert_eq!(desktop.active_workspace(), 0);
        desktop.next_workspace();
        desktop.next_workspace();
        desktop.previous_workspace();
        assert_eq!(shown(&desktop), [1, 1]);
        assert!(!visible(&desktop, left));
        assert!(!visible(&desktop, right), "each on the first of its own");

        // A point on no output turns nowhere.
        desktop.turn_to(Point::new(-50, -50));
        assert_eq!(desktop.current_output(), 0);
    }

    #[test]
    fn a_window_is_listed_with_its_output_and_its_workspace_there() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        desktop.move_to_workspace(right, 2);

        let listed: Vec<(WindowId, usize, usize)> = desktop
            .windows()
            .map(|listed| (listed.window.id, listed.output, listed.workspace))
            .collect();
        assert_eq!(listed, [(left, 0, 0), (right, 1, 2)]);
        assert_eq!(desktop.workspace_of(right), Some(2));
        assert!(
            visible(&desktop, left),
            "the first output still shows its own"
        );
    }

    #[test]
    fn only_the_current_output_has_a_focused_frame() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        assert_eq!(desktop.focused(), Some(right));
        assert_eq!(
            stack(&desktop),
            [left, right],
            "the current output's on top"
        );

        desktop.clicked(left);
        assert_eq!(desktop.current_output(), 0);
        assert_eq!(desktop.focused(), Some(left));
        assert_eq!(stack(&desktop), [right, left]);
        assert_eq!(desktop.frames().filter(|frame| frame.focused).count(), 1);

        desktop.activate(right);
        assert_eq!(desktop.current_output(), 1);
        assert_eq!(desktop.focused(), Some(right));

        // Cycling stays on the output that has the keyboard.
        desktop.focus_next();
        assert_eq!(desktop.focused(), Some(right));
    }

    #[test]
    fn the_pointer_takes_focus_to_another_output_when_focus_follows_it() {
        let mut desktop = two_outputs();
        desktop.set_settings(Settings {
            focus_follows_pointer: true,
            ..Settings::default()
        });
        let left = open(&mut desktop, "left");
        open_on_second(&mut desktop, "right");
        desktop.pointer_entered(left);
        assert_eq!(desktop.current_output(), 0);
        assert_eq!(desktop.focused(), Some(left));
    }

    #[test]
    fn a_frame_dragged_onto_another_output_joins_the_workspace_shown_there() {
        let mut desktop = two_outputs();
        let dragged = open(&mut desktop, "dragged");
        let other = open_on_second(&mut desktop, "other");
        desktop.switch_workspace(1);
        assert_eq!(shown(&desktop), [0, 1]);
        desktop.turn_to(POINTER);

        // Half across is not most of it.
        let straddling = Rect::new(Point::new(1920 - 500, 100), SIZE);
        desktop.set_geometry(dragged, straddling);
        assert_eq!(desktop.windows().next().unwrap().output, 0);
        assert_eq!(desktop.current_output(), 0);

        let across = Rect::new(Point::new(1920 - 100, 100), SIZE);
        desktop.set_geometry(dragged, across);
        let listed = desktop.windows().next().unwrap();
        assert_eq!((listed.output, listed.workspace), (1, 1));
        assert!(visible(&desktop, dragged), "it did not vanish on the way");
        assert!(
            !visible(&desktop, other),
            "which is on the workspace not shown"
        );
        assert_eq!(desktop.current_output(), 1);
        assert_eq!(desktop.focused(), Some(dragged));
        assert_eq!(frame(&desktop, dragged).geometry, across);
    }

    #[test]
    fn a_frame_can_be_sent_to_the_next_output_keeping_its_place() {
        let mut desktop = two_outputs();
        let id = open(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(Point::new(100, 200), SIZE));

        desktop.move_to_next_output(id);
        assert_eq!(desktop.windows().next().unwrap().output, 1);
        assert_eq!(
            frame(&desktop, id).geometry.origin,
            SECOND_SCREEN.origin.offset(100, 200)
        );
        assert_eq!(desktop.current_output(), 1);

        // And round again to the first.
        desktop.move_to_next_output(id);
        assert_eq!(desktop.windows().next().unwrap().output, 0);
        assert_eq!(frame(&desktop, id).geometry.origin, Point::new(100, 200));

        // A place that the next output does not reach becomes its corner.
        desktop.set_geometry(id, Rect::new(Point::new(1500, 1050), SIZE));
        desktop.move_to_next_output(id);
        assert_eq!(frame(&desktop, id).geometry.origin, SECOND_SCREEN.origin);
    }

    #[test]
    fn with_one_output_or_none_there_is_no_next_one() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let before = desktop.clone();
        desktop.move_to_next_output(id);
        assert_eq!(desktop, before);

        desktop.set_outputs(Vec::<Rect>::new());
        let before = desktop.clone();
        desktop.move_to_next_output(id);
        assert_eq!(desktop, before);
    }

    #[test]
    fn frames_of_an_output_that_goes_move_to_the_same_workspace_of_the_first() {
        let mut desktop = two_outputs();
        let stays = open(&mut desktop, "stays");
        let first = open_on_second(&mut desktop, "first");
        desktop.switch_workspace(1);
        let second = open_on_second(&mut desktop, "second");
        assert_eq!(desktop.current_output(), 1);

        desktop.remove_output(1);
        desktop.set_outputs(vec![SCREEN]);

        assert_eq!(desktop.current_output(), 0, "the output that is left");
        let listed: Vec<(WindowId, usize, usize)> = desktop
            .windows()
            .map(|listed| (listed.window.id, listed.output, listed.workspace))
            .collect();
        assert_eq!(listed, [(stays, 0, 0), (first, 0, 0), (second, 0, 1)]);
        assert_eq!(shown(&desktop), [0], "the first output shows what it did");
        assert_eq!(frame(&desktop, stays).geometry.origin, SCREEN.origin);
        assert_eq!(
            frame(&desktop, first).geometry.origin,
            SCREEN.origin,
            "brought onto the output it now belongs to"
        );
        desktop.switch_workspace(1);
        assert_eq!(frame(&desktop, second).geometry.origin, SCREEN.origin);

        // An output that is not there cannot be taken away.
        let before = desktop.clone();
        desktop.remove_output(1);
        assert_eq!(desktop, before);
    }

    #[test]
    fn taking_away_the_first_output_makes_the_next_the_first() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");
        desktop.turn_to(SECOND_SCREEN.origin);
        desktop.switch_workspace(2);

        desktop.remove_output(0);
        desktop.set_outputs(vec![SECOND_SCREEN]);

        assert_eq!(shown(&desktop), [2], "what the second output was showing");
        assert_eq!(desktop.current_output(), 0);
        desktop.switch_workspace(0);
        assert!(visible(&desktop, left));
        assert!(visible(&desktop, right));
        assert!(
            SECOND_SCREEN.contains(frame(&desktop, left).geometry.origin),
            "on the output that is left"
        );
    }

    #[test]
    fn fewer_outputs_than_before_means_the_last_ones_went() {
        let mut desktop = two_outputs();
        let left = open(&mut desktop, "left");
        let right = open_on_second(&mut desktop, "right");

        desktop.set_outputs(vec![SCREEN]);
        assert!(visible(&desktop, left));
        assert_eq!(frame(&desktop, right).geometry.origin, SCREEN.origin);
        assert_eq!(desktop.windows().last().unwrap().output, 0);
    }

    #[test]
    fn a_frame_keeps_its_place_on_an_output_that_moves() {
        let mut desktop = two_outputs();
        let id = open_on_second(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(SECOND_SCREEN.origin.offset(40, 60), SIZE));

        // The first output becomes narrower, so the second starts further
        // to the left.
        let narrower = Rect::new(Point::new(0, 0), Size::new(1280, 1080));
        let moved = Rect::new(Point::new(1280, 0), SECOND_SCREEN.size);
        desktop.set_outputs(vec![narrower, moved]);

        assert_eq!(
            frame(&desktop, id).geometry.origin,
            moved.origin.offset(40, 60)
        );
        assert_eq!(desktop.windows().next().unwrap().output, 1);
    }

    #[test]
    fn a_new_output_has_as_many_workspaces_as_the_others() {
        let mut desktop = desktop();
        desktop.set_workspace_count(5);
        desktop.set_outputs(vec![SCREEN, SECOND_SCREEN]);
        let id = open_on_second(&mut desktop, "app");
        desktop.move_to_workspace(id, 4);
        assert_eq!(desktop.workspace_of(id), Some(4));

        // Fewer workspaces means fewer on every output.
        desktop.turn_to(SECOND_SCREEN.origin);
        desktop.switch_workspace(4);
        desktop.set_workspace_count(2);
        assert_eq!(desktop.workspace_count(), 2);
        assert_eq!(shown(&desktop), [0, 1]);
        assert_eq!(desktop.workspace_of(id), Some(1));
    }

    #[test]
    fn a_rule_names_a_workspace_of_the_output_the_window_opens_on() {
        let mut desktop = two_outputs();
        desktop.set_rules(vec![Rule {
            app_id: Some(Pattern::new("chat")),
            title: None,
            effects: Effects {
                workspace: Some(2),
                ..Effects::default()
            },
        }]);
        let id = open_on_second(&mut desktop, "chat");
        let listed = desktop.windows().next().unwrap();
        assert_eq!(
            (listed.window.id, listed.output, listed.workspace),
            (id, 1, 2)
        );
        assert!(!visible(&desktop, id), "that output is showing another");
    }

    #[test]
    fn removing_every_output_leaves_frames_where_they_are() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let geometry = Rect::new(Point::new(300, 200), SIZE);
        desktop.set_geometry(id, geometry);
        desktop.set_outputs(Vec::<Rect>::new());
        assert_eq!(frame(&desktop, id).geometry, geometry);
    }

    #[test]
    fn windows_keep_clear_of_the_part_of_an_output_reserved_for_panels() {
        let mut desktop = Desktop::new(Settings::default(), 1);
        // A 30 pixel panel along the top and a 50 pixel dock on the left.
        let usable = Rect::new(Point::new(50, 30), Size::new(1870, 1050));
        desktop.set_outputs([Screen {
            area: SCREEN,
            usable,
        }]);

        let id = open(&mut desktop, "app");
        assert_eq!(
            frame(&desktop, id).geometry.origin,
            usable.origin,
            "placed in the usable part"
        );

        desktop.set_maximized(id, true);
        assert_eq!(
            frame(&desktop, id).geometry,
            usable,
            "maximized up to the panels"
        );

        desktop.set_fullscreen(id, true);
        assert_eq!(
            frame(&desktop, id).geometry,
            SCREEN,
            "fullscreen covers them"
        );
    }

    #[test]
    fn a_stranded_frame_moves_to_the_usable_part_of_the_first_output() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        desktop.set_geometry(id, Rect::new(Point::new(5000, 5000), SIZE));
        let usable = Rect::new(Point::new(0, 40), Size::new(1920, 1040));

        desktop.set_outputs([Screen {
            area: SCREEN,
            usable,
        }]);

        assert_eq!(frame(&desktop, id).geometry.origin, usable.origin);
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
        assert_eq!(desktop.sets.len(), desktop.outputs.len().max(1));
        assert!(
            desktop.current < desktop.sets.len(),
            "the current set exists"
        );
        let count = desktop.workspace_count();
        for set in &desktop.sets {
            assert_eq!(set.workspaces.len(), count, "every output has as many");
            assert!(set.active < count, "the workspace shown exists");
        }
        let workspaces = desktop.sets.iter().flat_map(|set| &set.workspaces);
        for workspace in workspaces.clone() {
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
        let mut ids: Vec<WindowId> = workspaces
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
            let screens = [SCREEN, SECOND_SCREEN];
            match random(30) {
                0 | 1 => ids.push(open(&mut desktop, "app")),
                27 => desktop.toggle_shaded(id),
                28 => desktop.toggle_above(id),
                29 => desktop.toggle_sticky(id),
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
                21 => desktop.set_outputs(vec![SCREEN; random(3)]),
                22 => desktop.set_outputs(screens[..random(3)].to_vec()),
                23 => desktop.remove_output(random(3)),
                24 => desktop.move_to_next_output(id),
                25 => desktop.turn_to(Point::new(
                    i32::try_from(random(3200)).unwrap(),
                    i32::try_from(random(1000)).unwrap(),
                )),
                _ => {
                    let origin = Point::new(
                        i32::try_from(random(3200)).unwrap() - 200,
                        i32::try_from(random(1000)).unwrap(),
                    );
                    desktop.set_geometry(id, Rect::new(origin, Size::new(400, 300)));
                }
            }
            assert_consistent(&desktop);
            assert!(
                desktop.frames().filter(|frame| frame.focused).count() <= 1,
                "step {step}"
            );
        }
    }

    // --- Standing apart ------------------------------------------------------

    #[test]
    fn a_shaded_frame_is_rolled_up_into_its_titlebar() {
        let mut desktop = decorated_desktop();
        let id = open(&mut desktop, "app");
        let whole = frame(&desktop, id).geometry;
        assert!(!frame(&desktop, id).standing.shaded);

        desktop.toggle_shaded(id);
        let rolled = frame(&desktop, id);
        assert!(rolled.standing.shaded);
        assert_eq!(rolled.geometry.origin, whole.origin);
        assert_eq!(
            rolled.geometry.size,
            Size::new(whole.size.width, METRICS.titlebar)
        );
        assert_eq!(rolled.client().size.height, 0, "none of the window shows");

        // Maximized, it is as wide as the screen and still only a titlebar.
        desktop.toggle_maximized(id);
        let size = frame(&desktop, id).geometry.size;
        assert_eq!(size, Size::new(SCREEN.size.width, METRICS.titlebar));
        desktop.toggle_maximized(id);

        desktop.toggle_shaded(id);
        assert_eq!(frame(&desktop, id).geometry, whole, "let down as it was");
    }

    #[test]
    fn a_frame_without_a_titlebar_cannot_be_rolled_up_into_one() {
        let mut desktop = decorated_desktop();
        let bare = desktop.add_window("bare", "bare", SIZE, POINTER, false);
        let whole = frame(&desktop, bare).geometry;
        desktop.toggle_shaded(bare);
        assert_eq!(frame(&desktop, bare).geometry, whole);
        assert!(!frame(&desktop, bare).standing.shaded);

        // Nor one that is fullscreen, which has none while it is.
        let id = open(&mut desktop, "app");
        desktop.toggle_shaded(id);
        desktop.toggle_fullscreen(id);
        assert_eq!(frame(&desktop, id).geometry, SCREEN);
        assert!(!frame(&desktop, id).standing.shaded);
        desktop.toggle_fullscreen(id);
        assert!(
            frame(&desktop, id).standing.shaded,
            "it is again afterwards"
        );
    }

    #[test]
    fn a_frame_kept_above_is_over_the_others_whichever_has_focus() {
        let mut desktop = desktop();
        let first = open(&mut desktop, "first");
        let second = open(&mut desktop, "second");
        let third = open(&mut desktop, "third");
        assert_eq!(stack(&desktop), [first, second, third]);

        desktop.toggle_above(first);
        assert!(frame(&desktop, first).standing.above);
        assert_eq!(stack(&desktop), [second, third, first]);
        desktop.clicked(second);
        assert_eq!(stack(&desktop), [third, second, first], "raised under it");
        assert_eq!(desktop.focused(), Some(second));

        // A fullscreen frame is over even that.
        desktop.toggle_fullscreen(third);
        assert_eq!(stack(&desktop), [second, first, third]);
        desktop.toggle_fullscreen(third);

        desktop.toggle_above(first);
        assert_eq!(
            stack(&desktop),
            [first, third, second],
            "where it was raised to"
        );
    }

    #[test]
    fn a_sticky_frame_is_on_whichever_workspace_its_output_shows() {
        let mut desktop = desktop();
        let staying = open(&mut desktop, "staying");
        let sticky = open(&mut desktop, "sticky");
        desktop.toggle_sticky(sticky);
        assert!(frame(&desktop, sticky).standing.sticky);

        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [sticky]);
        assert_eq!(desktop.focused(), Some(sticky), "it had focus and keeps it");
        desktop.next_workspace();
        assert_eq!(stack(&desktop), [sticky]);
        desktop.previous_workspace();
        desktop.previous_workspace();
        assert_eq!(stack(&desktop), [staying, sticky], "over what was there");

        // One that did not have focus does not take it.
        desktop.clicked(staying);
        desktop.switch_workspace(1);
        let other = open(&mut desktop, "other");
        desktop.switch_workspace(0);
        assert_eq!(desktop.focused(), Some(staying));
        desktop.switch_workspace(1);
        assert_eq!(desktop.focused(), Some(other));
        assert_eq!(stack(&desktop), [other, sticky]);

        // Showing the workspace it is on already changes nothing.
        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), [other, sticky]);

        desktop.toggle_sticky(sticky);
        desktop.switch_workspace(2);
        assert_eq!(stack(&desktop), [], "it stays where it was left");
    }

    #[test]
    fn a_sticky_frame_comes_along_when_another_window_is_brought_up() {
        let mut desktop = desktop();
        let elsewhere = open(&mut desktop, "elsewhere");
        desktop.switch_workspace(1);
        let sticky = open(&mut desktop, "sticky");
        desktop.toggle_sticky(sticky);

        desktop.activate(elsewhere);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(stack(&desktop), [sticky, elsewhere]);
        assert_eq!(desktop.focused(), Some(elsewhere));
        // Brought up itself, it is where it already was.
        desktop.activate(sticky);
        assert_eq!(desktop.active_workspace(), 0);
        assert_eq!(desktop.focused(), Some(sticky));
    }

    #[test]
    fn a_frame_sent_to_one_workspace_is_no_longer_on_all_of_them() {
        let mut desktop = desktop();
        let sticky = open(&mut desktop, "sticky");
        desktop.toggle_sticky(sticky);
        desktop.move_to_workspace(sticky, 2);
        assert_eq!(stack(&desktop), []);
        desktop.switch_workspace(1);
        assert_eq!(stack(&desktop), []);
        desktop.switch_workspace(2);
        assert_eq!(stack(&desktop), [sticky]);
        assert!(!frame(&desktop, sticky).standing.sticky);
    }

    #[test]
    fn standing_apart_is_not_asked_of_a_window_that_is_not_there() {
        let mut desktop = desktop();
        let id = open(&mut desktop, "app");
        let gone = open(&mut desktop, "gone");
        desktop.remove_window(gone);
        desktop.toggle_shaded(gone);
        desktop.toggle_above(gone);
        desktop.toggle_sticky(gone);
        assert_eq!(frame(&desktop, id).standing, Standing::default());
    }
}
