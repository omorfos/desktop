//! What is behind a program blurred where it asks through KDE's protocol
//! (`org_kde_kwin_blur`), which Ghostty, KDE's programs and fcitx5 ask
//! through where they might ask through `ext-background-effect-v1`
//! (ADR 64).
//!
//! What it asks for is put where `backdrops` puts what is asked through
//! the standard, and drawn the same; whichever was asked through last is
//! what a surface has. It differs as KDE's compositor, which programs that ask through
//! it are written for, has it differ:
//!
//! - A region is set on an object of its own, and asked for once that
//!   object is committed, then the surface; a region of nothing, or none,
//!   asks for all of the surface.
//! - A surface may have many such objects. The last committed is what it
//!   asks for: letting go of it takes that back, letting go of another
//!   does not, so that a program that commits a new one before letting go
//!   of the one before keeps its blur.
//! - A program is told nothing of whether it may have blur, as the
//!   protocol has no way to: where the theme gives programs none, nothing
//!   is drawn.
//! - Nothing a program asks is an error, as the protocol has none.
//
// TODO: take this out once the programs that ask through it ask through
// `ext-background-effect-v1`; ADR 64 says how to tell.

use std::cell::RefCell;
use std::sync::{Mutex, PoisonError};

use smithay::reexports::wayland_server::backend::{ClientId, ObjectId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};
use smithay::wayland::compositor::{SurfaceData, get_region_attributes, with_states};
use wayland_protocols_plasma::blur::server::{
    org_kde_kwin_blur::{self, OrgKdeKwinBlur},
    org_kde_kwin_blur_manager::{self, OrgKdeKwinBlurManager},
};

use crate::backdrops::{Asked, rectangles};
use crate::compositor::Compositor;

/// Offers the protocol on `display`.
pub(crate) fn offer(display: &DisplayHandle) {
    display.create_global::<Compositor, OrgKdeKwinBlurManager, ()>(1, ());
}

/// An object a program asks for blur behind `surface` through, and the
/// region last set on it, which its commit asks for.
#[derive(Debug)]
pub(crate) struct Blur {
    surface: WlSurface,
    region: Mutex<Asked>,
}

/// Which of a surface's objects last committed what it asks for.
#[derive(Debug, Default)]
struct Last(RefCell<Option<ObjectId>>);

/// Asks, from the surface's next commit, for `asked`, as `by` last
/// committed, or as none did.
fn ask(states: &SurfaceData, asked: Asked, by: Option<ObjectId>) {
    *states.cached_state.get::<Asked>().pending() = asked;
    states.data_map.get_or_insert(Last::default).0.replace(by);
}

impl GlobalDispatch<OrgKdeKwinBlurManager, ()> for Compositor {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        manager: New<OrgKdeKwinBlurManager>,
        (): &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(manager, ());
    }
}

impl Dispatch<OrgKdeKwinBlurManager, ()> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &OrgKdeKwinBlurManager,
        request: org_kde_kwin_blur_manager::Request,
        (): &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        // Each of its two requests is the other's way past: a match would
        // need an arm for a request this version does not have.
        if let org_kde_kwin_blur_manager::Request::Unset { surface } = &request {
            with_states(surface, |states| ask(states, Asked::Nothing, None));
        }
        if let org_kde_kwin_blur_manager::Request::Create { id, surface } = request {
            let region = Mutex::new(Asked::Whole);
            data_init.init(id, Blur { surface, region });
        }
    }
}

impl Dispatch<OrgKdeKwinBlur, Blur> for Compositor {
    fn request(
        _: &mut Self,
        _: &Client,
        object: &OrgKdeKwinBlur,
        request: org_kde_kwin_blur::Request,
        blur: &Blur,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let mut region = blur.region.lock().unwrap_or_else(PoisonError::into_inner);
        match request {
            org_kde_kwin_blur::Request::SetRegion { region: set } => {
                let set = set.map(|set| rectangles(&get_region_attributes(&set)));
                *region = match set {
                    Some(set) if !set.is_empty() => Asked::Within(set),
                    _ => Asked::Whole,
                };
            }
            // Its surface may be gone, which only its own request says.
            org_kde_kwin_blur::Request::Commit if blur.surface.is_alive() => {
                with_states(&blur.surface, |states| {
                    ask(states, region.clone(), Some(object.id()));
                });
            }
            _ => {}
        }
    }

    fn destroyed(_: &mut Self, _: ClientId, object: &OrgKdeKwinBlur, blur: &Blur) {
        if blur.surface.is_alive() {
            with_states(&blur.surface, |states| {
                let last = states.data_map.get_or_insert(Last::default);
                if last.0.borrow().as_ref() == Some(&object.id()) {
                    ask(states, Asked::Nothing, None);
                }
            });
        }
    }
}
