//! Programs that hold the pointer (`pointer-constraints`): a game that
//! locks it in place to turn by the mouse, or a remote desktop that keeps
//! it within its window. And the distances a mouse or touchpad moves, told
//! as they are (`relative-pointer`), which is all a pointer held still
//! still says.
//!
//! A program holds the pointer only while its window has the keyboard and
//! the pointer is over the part of it the program asked for. Whatever
//! takes the keyboard away, a click elsewhere being out of reach, as a key
//! that switches windows does, lets the pointer go; it is held again when
//! the window has the keyboard back, with the pointer still over it.

use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::input::pointer::{PointerHandle, RelativeMotionEvent};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{RegionAttributes, get_parent};
use smithay::wayland::pointer_constraints::{
    PointerConstraint, PointerConstraintRef, PointerConstraintsHandler, PointerConstraintsState,
    with_pointer_constraint,
};
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::{delegate_pointer_constraints, delegate_relative_pointer};

use crate::compositor::Compositor;

/// What the compositor keeps of programs holding the pointer.
#[derive(Debug)]
pub(crate) struct Holding {
    /// The surface that has the keyboard, as last told: a program holds
    /// the pointer only while that is its window.
    pub(crate) keyboard: Option<WlSurface>,
    _constraints: PointerConstraintsState,
    _relative: RelativePointerManagerState,
}

impl Holding {
    /// Offers the protocols on `display`.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        Self {
            keyboard: None,
            _constraints: PointerConstraintsState::new::<Compositor>(display),
            _relative: RelativePointerManagerState::new::<Compositor>(display),
        }
    }
}

/// What a program holding the pointer lets it do.
pub(crate) enum Held {
    /// Nothing: it stays where it is.
    Locked,
    /// Move within a part of the surface under it.
    Confined(Bounds),
}

/// A part of a surface the pointer is kept in, or has to be in for a
/// program to take hold of it.
pub(crate) struct Bounds {
    surface: WlSurface,
    /// Where the surface is, in the space the outputs show.
    origin: Point<f64, Logical>,
    /// The part of it, or all of it.
    region: Option<RegionAttributes>,
}

impl Bounds {
    /// Whether `at`, in the space the outputs show, is within.
    fn contains(&self, at: Point<f64, Logical>) -> bool {
        let size = with_renderer_surface_state(&self.surface, |drawn| drawn.surface_size());
        let size = size.flatten().unwrap_or_default().to_f64();
        let local = at - self.origin;
        let on = (0.0..size.w).contains(&local.x) && (0.0..size.h).contains(&local.y);
        let part = |region: &RegionAttributes| region.contains(local.to_i32_floor());
        on && self.region.as_ref().is_none_or(part)
    }

    /// Where moving the pointer from `from` by `by` takes it within: as far
    /// as asked, or else along the edge it meets, or else nowhere.
    pub(crate) fn slide(&self, from: Point<f64, Logical>, by: (f64, f64)) -> Point<f64, Logical> {
        let tried = [by, (by.0, 0.0), (0.0, by.1)].map(|by| from + Point::from(by));
        let within = tried.into_iter().find(|to| self.contains(*to));
        within.unwrap_or(from)
    }
}

impl Compositor {
    /// What the program the pointer is over lets it do, if it holds it.
    pub(crate) fn held(&self) -> Option<Held> {
        let (surface, origin) = self.hover.pointed.0.clone()?;
        with_pointer_constraint(&surface, &self.pointer, |constraint| {
            let constraint = constraint.filter(|constraint| constraint.is_active())?;
            Some(match &*constraint {
                PointerConstraint::Locked(_) => Held::Locked,
                PointerConstraint::Confined(confined) => Held::Confined(Bounds {
                    surface: surface.clone(),
                    origin,
                    region: confined.region().cloned(),
                }),
            })
        })
    }

    /// Tells the program the pointer is over how far it moved, as the
    /// device says, with and without what makes a fast move go further.
    /// `time` is in thousandths of a second.
    pub(crate) fn moved_relatively(&mut self, by: (f64, f64), raw: (f64, f64), time: u32) {
        let event = RelativeMotionEvent {
            delta: by.into(),
            delta_unaccel: raw.into(),
            utime: u64::from(time) * 1000,
        };
        let pointer = self.pointer.clone();
        let focus = self.hover.pointed.0.clone();
        pointer.relative_motion(self, focus, &event);
        pointer.frame(self);
    }

    /// Lets the program the pointer is over hold it, if it asked to and
    /// may now, and lets it go if it may no longer: when its window has
    /// lost the keyboard.
    pub(crate) fn hold(&self) {
        let Some((surface, origin)) = self.hover.pointed.0.clone() else {
            return;
        };
        let mut root = surface.clone();
        while let Some(parent) = get_parent(&root) {
            root = parent;
        }
        let typed_into = self.holding.keyboard.as_ref() == Some(&root);
        let asked = with_pointer_constraint(&surface, &self.pointer, |constraint| {
            constraint.map(|constraint| (constraint.is_active(), constraint.region().cloned()))
        });
        let Some((active, region)) = asked else {
            return;
        };
        let bounds = Bounds {
            surface: surface.clone(),
            origin,
            region,
        };
        let may = typed_into && bounds.contains(self.pointer_location);
        if may != active {
            with_pointer_constraint(&surface, &self.pointer, |constraint| {
                let constraint = constraint.into_iter();
                if may {
                    constraint.for_each(|constraint| constraint.activate());
                } else {
                    constraint.for_each(PointerConstraintRef::deactivate);
                }
            });
        }
    }
}

impl PointerConstraintsHandler for Compositor {
    fn new_constraint(&mut self, _: &WlSurface, _: &PointerHandle<Self>) {
        self.hold();
    }

    /// A program that has the pointer locked says where it would have
    /// moved to, so that it is there once let go of.
    fn cursor_position_hint(
        &mut self,
        surface: &WlSurface,
        pointer: &PointerHandle<Self>,
        location: Point<f64, Logical>,
    ) {
        let active = with_pointer_constraint(surface, pointer, |constraint| {
            constraint.is_some_and(|constraint| constraint.is_active())
        });
        let under = self.hover.pointed.0.as_ref();
        let over = under.filter(|(on, _)| active && on == surface);
        if let Some((_, origin)) = over {
            self.pointer_location = *origin + location;
        }
    }
}

delegate_pointer_constraints!(Compositor);
delegate_relative_pointer!(Compositor);
