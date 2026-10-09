//! The screens: where each one is in the space windows live in, and how
//! large things are drawn on it.
//!
//! Windows, the pointer and everything else are positioned in one space,
//! measured in units of layout. Each output shows a rectangle of it. The
//! outputs stand side by side in the order they were added, and each has
//! its own scale, so a rectangle of the same size in units can be a
//! different number of pixels on each.
//!
//! See `docs/adr/0015-several-outputs.md`.

use std::cell::Cell;

use omorfos_core::Screen;
use omorfos_core::geometry::{Point as DomainPoint, Rect, Size as DomainSize};
use smithay::desktop::layer_map_for_output;
use smithay::output::{Mode, Output, PhysicalProperties, Scale as OutputScale, Subpixel};
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::utils::{Logical, Physical, Point, Rectangle, Size, Transform};

use crate::compositor::Compositor;
use crate::{Scale, bridge};

/// An output, with what the compositor keeps about it.
#[derive(Debug)]
pub(crate) struct Head {
    pub(crate) output: Output,
    /// What clients see the output as, withdrawn when it goes.
    global: GlobalId,
    /// How the backend's picture of it is oriented.
    transform: Transform,
    /// The scale the backend says suits it.
    suggested: Scale,
    /// How many pixels it draws to a unit of layout.
    pub(crate) scale: Scale,
    /// Whether a frame showing nothing of the session has been drawn on it
    /// since the session was locked. Set while drawing, which does not
    /// change anything else.
    pub(crate) blanked: Cell<bool>,
}

impl Head {
    /// The size of the output in pixels.
    fn pixels(&self) -> Size<i32, Physical> {
        let mode = self.output.current_mode();
        mode.map(|mode| mode.size).unwrap_or_default()
    }

    /// The part of the space the output shows, in units of layout.
    pub(crate) fn area(&self) -> Rectangle<i32, Logical> {
        let pixels = self.pixels();
        let size = (self.scale.units(pixels.w), self.scale.units(pixels.h));
        Rectangle::new(self.output.current_location(), size.into())
    }

    /// Whether `point` is on the output.
    fn contains(&self, point: Point<f64, Logical>) -> bool {
        self.area().to_f64().contains(point)
    }

    /// Sets the output's size in pixels, keeping everything else.
    fn set_pixels(&self, (width, height): (i32, i32)) {
        let mode = Mode {
            size: (width, height).into(),
            refresh: 60_000,
        };
        let scale = OutputScale::Fractional(self.scale.factor());
        self.output
            .change_current_state(Some(mode), Some(self.transform), Some(scale), None);
        self.output.set_preferred(mode);
    }
}

/// A rectangle of the space, as the window-management model has it.
fn domain(rectangle: Rectangle<i32, Logical>) -> Rect {
    Rect::new(
        DomainPoint::new(rectangle.loc.x, rectangle.loc.y),
        DomainSize::new(
            rectangle.size.w.unsigned_abs(),
            rectangle.size.h.unsigned_abs(),
        ),
    )
}

impl Compositor {
    /// Adds an output of `size` pixels, to the right of those there are.
    ///
    /// `name` is what clients are told it is called. `transform` describes
    /// how the backend's framebuffer is oriented; a backend drawing through
    /// OpenGL into a window is flipped.
    ///
    /// The value returned names the output in every other call about it.
    pub fn add_output(&mut self, name: &str, size: (i32, i32), transform: Transform) -> Output {
        let output = Output::new(
            name.to_owned(),
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: Subpixel::Unknown,
                make: "omorfos".to_owned(),
                model: name.to_owned(),
            },
        );
        let suggested = Scale::ONE;
        let head = Head {
            global: output.create_global::<Self>(&self.display),
            output: output.clone(),
            transform,
            suggested,
            scale: bridge::scale(self.config().general.output.scale, suggested),
            blanked: Cell::new(false),
        };
        head.set_pixels(size);
        self.heads.push(head);
        self.lay_out();
        output
    }

    /// Takes an output away. Windows left stranded move to the first of
    /// those that remain, and panels that were on it are closed.
    pub fn remove_output(&mut self, output: &Output) {
        let Some(index) = self.heads.iter().position(|head| head.output == *output) else {
            return;
        };
        let head = self.heads.remove(index);
        for layer in layer_map_for_output(&head.output).layers() {
            layer.layer_surface().send_close();
        }
        self.space.unmap_output(&head.output);
        self.display.remove_global::<Self>(head.global);
        self.forget_lock_surface(&head.output);
        self.lay_out();
        // The pointer does not stay behind where there is no screen.
        let at = self.moved_by(0.0, 0.0);
        self.pointer_location = at;
    }

    /// Changes the size of an output, in pixels.
    pub fn resize_output(&mut self, output: &Output, size: (i32, i32)) {
        if let Some(head) = self.head(output) {
            head.set_pixels(size);
            self.lay_out();
        }
    }

    /// Says what scale suits an output, which a backend knows from how
    /// dense the screen is. It is used unless the configuration names one.
    pub fn suggest_scale(&mut self, output: &Output, scale: Scale) {
        let known = self.heads.iter_mut().find(|head| head.output == *output);
        if let Some(head) = known {
            head.suggested = scale;
            self.apply_scale();
        }
    }

    /// How many pixels an output draws to a unit of layout. One for an
    /// output that is not there.
    #[must_use]
    pub fn scale_of(&self, output: &Output) -> Scale {
        self.head(output).map_or(Scale::ONE, |head| head.scale)
    }

    /// The part of the space an output shows, in units of layout, which is
    /// what windows and the pointer are positioned in. Empty for an output
    /// that is not there.
    #[must_use]
    pub fn area_of(&self, output: &Output) -> Rectangle<i32, Logical> {
        self.head(output).map(Head::area).unwrap_or_default()
    }

    pub(crate) fn head(&self, output: &Output) -> Option<&Head> {
        self.heads.iter().find(|head| head.output == *output)
    }

    /// The output a point of the space is on.
    pub(crate) fn head_at(&self, point: Point<f64, Logical>) -> Option<&Head> {
        self.heads.iter().find(|head| head.contains(point))
    }

    /// The output the pointer is on, or failing that the first.
    pub(crate) fn pointer_head(&self) -> Option<&Head> {
        self.head_at(self.pointer_location)
            .or_else(|| self.heads.first())
    }

    /// The scale things at the pointer are drawn at.
    pub(crate) fn pointer_scale(&self) -> Scale {
        self.pointer_head().map_or(Scale::ONE, |head| head.scale)
    }

    /// The output most of a rectangle of the space is on, or failing that
    /// the first.
    pub(crate) fn head_for(&self, rectangle: Rect) -> Option<&Head> {
        let overlap = |head: &&Head| {
            let shared = domain(head.area()).intersection(rectangle);
            shared.map_or(0, |shared| {
                u64::from(shared.size.width) * u64::from(shared.size.height)
            })
        };
        let most = self.heads.iter().max_by_key(overlap);
        most.filter(|head| overlap(head) > 0)
            .or_else(|| self.heads.first())
    }

    /// Where the pointer ends up after moving by a distance: anywhere on an
    /// output, and otherwise as far as the edge of the one it was on.
    pub(crate) fn moved_by(&self, dx: f64, dy: f64) -> Point<f64, Logical> {
        let from = self.pointer_location;
        let to = from + Point::from((dx, dy));
        if self.head_at(to).is_some() {
            return to;
        }
        let Some(area) = self.pointer_head().map(|head| head.area().to_f64()) else {
            return from;
        };
        let last = area.loc + area.size.to_point() - Point::from((1.0, 1.0));
        Point::from((
            to.x.clamp(area.loc.x, last.x.max(area.loc.x)),
            to.y.clamp(area.loc.y, last.y.max(area.loc.y)),
        ))
    }

    /// Settles each output's scale, from the configuration or else the
    /// backend's suggestion, and if any changed lays the outputs out again.
    pub(crate) fn apply_scale(&mut self) {
        let configured = self.config().general.output.scale;
        let mut changed = false;
        for head in &mut self.heads {
            let scale = bridge::scale(configured, head.suggested);
            changed |= scale != head.scale;
            head.scale = scale;
        }
        if changed {
            for head in &self.heads {
                let pixels = head.pixels();
                head.set_pixels((pixels.w, pixels.h));
            }
            self.lay_out();
        }
    }

    /// Stands the outputs side by side, left to right in the order they
    /// were added, and brings everything that depends on where they are up
    /// to date.
    fn lay_out(&mut self) {
        let mut left = 0;
        for head in &self.heads {
            let at = Point::from((left, 0));
            head.output.change_current_state(None, None, None, Some(at));
            self.space.map_output(&head.output, at);
            layer_map_for_output(&head.output).arrange();
            left += head.area().size.w;
        }
        self.update_screens();
        self.configure_lock_surfaces();
    }

    /// Tells the desktop where the outputs are and how much of each panels
    /// have left for windows, if any of that changed.
    pub(crate) fn update_screens(&mut self) {
        let screen = |head: &Head| {
            let area = head.area();
            let mut usable = layer_map_for_output(&head.output).non_exclusive_zone();
            usable.loc += area.loc;
            Screen {
                area: domain(area),
                usable: domain(usable),
            }
        };
        let screens: Vec<Screen> = self.heads.iter().map(screen).collect();
        if screens != self.screens {
            self.desktop.set_outputs(screens.iter().copied());
            self.screens = screens;
        }
    }
}
