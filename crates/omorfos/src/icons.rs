//! The icons of windows: the one a program gives its window
//! (`xdg-toplevel-icon`), or else its application's, from its desktop
//! entry or by its name.
//!
//! An icon a program names is looked up in the icon themes, as the
//! notifications and the launcher look theirs up; one it gives as pixels is
//! in memory it shares, which only the renderer reads (ADR 40), so it is
//! drawn by the renderer and not painted.

use std::collections::HashMap;
use std::path::PathBuf;

use omorfos_config::Color;
use omorfos_icons::Icons;
use omorfos_ui::Image;
use smithay::backend::renderer::buffer_dimensions;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::with_states;
use smithay::wayland::xdg_toplevel_icon::{ToplevelIconCachedState, XdgToplevelIconHandler};

use crate::compositor::Compositor;

/// Where a window's icon comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Icon {
    /// An icon of a theme, or a picture's path from a desktop entry.
    Named(String),
    /// Pixels the program gave, in memory it shares.
    Given(WlBuffer),
}

/// Where icons are installed, and those looked up so far.
#[derive(Debug, Default)]
pub(crate) struct WindowIcons {
    /// Where programs install what they share, desktop entries among it.
    data: Vec<PathBuf>,
    icons: Icons,
    /// The icon each application's desktop entry names, by identifier.
    entries: HashMap<String, Option<String>>,
}

impl WindowIcons {
    /// The icons installed under `data`, the user's own first (see
    /// [`omorfos_icons::data_directories`]).
    pub(crate) fn new(data: Vec<PathBuf>) -> Self {
        let icons = Icons::new(omorfos_icons::icon_directories(&data));
        Self {
            data,
            icons,
            entries: HashMap::new(),
        }
    }

    /// The icon of a window of the application `app_id`, to be drawn
    /// `pixels` across beside text of the colour `ink`: the one its
    /// program names, if a theme has it; else the pixels it gave; else
    /// the one its desktop entry names, or the one of its own name.
    pub(crate) fn of(
        &mut self,
        surface: &WlSurface,
        app_id: &str,
        pixels: u32,
        ink: Color,
    ) -> Option<Icon> {
        let (named, given) = with_states(surface, |states| {
            let mut cached = states.cached_state.get::<ToplevelIconCachedState>();
            let current = cached.current();
            let named = current.icon_name().map(str::to_owned);
            (named, largest(current.buffers(), pixels))
        });
        // A program names an icon of a theme. A path would have the
        // compositor read a file of the program's choosing.
        let named = named.filter(|name| !name.contains('/'));
        let mut found = |name: &str| self.icons.get(name, pixels, ink).is_some();
        if let Some(name) = named.filter(|name| found(name)) {
            return Some(Icon::Named(name));
        }
        if let Some(buffer) = given {
            return Some(Icon::Given(buffer));
        }
        let data = &self.data;
        let entry = self
            .entries
            .entry(app_id.to_owned())
            .or_insert_with(|| omorfos_icons::icon_of(app_id, data))
            .clone();
        let mut found = |name: &str| self.icons.get(name, pixels, ink).is_some();
        let name = entry
            .into_iter()
            .chain([app_id.to_owned()])
            .find(|name| found(name));
        name.map(Icon::Named)
    }

    /// The picture of an icon of a theme, as [`Self::of`] found it.
    pub(crate) fn picture(&mut self, name: &str, pixels: u32, ink: Color) -> Option<&Image> {
        self.icons.get(name, pixels, ink)
    }
}

/// Of the buffers an icon was given, the smallest at least `pixels` across,
/// and failing that the largest.
fn largest(buffers: &[(WlBuffer, i32)], pixels: u32) -> Option<WlBuffer> {
    let sized = buffers.iter().map(|(buffer, _)| {
        let across = buffer_dimensions(buffer).map_or(0, |size| size.w);
        (u32::try_from(across).unwrap_or(0), buffer)
    });
    sized
        .min_by_key(|(across, _)| (*across < pixels, across.abs_diff(pixels)))
        .map(|(_, buffer)| buffer.clone())
}

impl XdgToplevelIconHandler for Compositor {}

smithay::delegate_xdg_toplevel_icon!(Compositor);
