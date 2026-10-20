//! The screens: where each one is in the space windows live in, and how
//! large things are drawn on it.
//!
//! Windows, the pointer and everything else are positioned in one space,
//! measured in units of layout. Each output shows a rectangle of it. The
//! outputs stand where the configuration says, next to one another, or
//! else side by side in the order they were added, and each has its own
//! scale, so a rectangle of the same size in units can be a
//! different number of pixels on each.
//!
//! See `docs/adr/0015-several-outputs.md`.

use std::cell::Cell;

use omorfos_config::{Align, OfferedMode, Offset, Place, Rotation};
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
    /// Which way the configuration says the screen is turned.
    rotation: Transform,
    /// The ways the backend says it can be driven, and the one it is
    /// driven in: none, from a backend that does not say.
    pub(crate) modes: Vec<OfferedMode>,
    pub(crate) mode: Option<OfferedMode>,
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
    /// The size of the output in pixels, as the screen is made.
    pub(crate) fn pixels(&self) -> Size<i32, Physical> {
        let mode = self.output.current_mode();
        mode.map(|mode| mode.size).unwrap_or_default()
    }

    /// How what is drawn on it is turned: as the configuration says the
    /// screen is, unless the backend's picture is turned already, as that
    /// of one drawing into a window is, which a turn would only spoil.
    pub(crate) fn turned(&self) -> Transform {
        if self.transform == Transform::Normal {
            self.rotation
        } else {
            self.transform
        }
    }

    /// The size of the output in pixels as it stands, the right way up.
    pub(crate) fn upright(&self) -> Size<i32, Physical> {
        self.turned().transform_size(self.pixels())
    }

    /// The part of the space the output shows, in units of layout.
    pub(crate) fn area(&self) -> Rectangle<i32, Logical> {
        let pixels = self.upright();
        let size = (self.scale.units(pixels.w), self.scale.units(pixels.h));
        Rectangle::new(self.output.current_location(), size.into())
    }

    /// Whether `point` is on the output.
    pub(crate) fn contains(&self, point: Point<f64, Logical>) -> bool {
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
            .change_current_state(Some(mode), Some(self.turned()), Some(scale), None);
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
            rotation: self.rotation_for(name),
            modes: Vec::new(),
            mode: None,
            suggested,
            scale: self.scale_for(name, suggested),
            blanked: Cell::new(false),
        };
        head.set_pixels(size);
        self.heads.push(head);
        // Another arrangement may now be the one for the screens there are.
        self.apply_scale();
        output
    }

    /// Says the ways `output` can be driven, as its backend has them, and
    /// the one it is driven in, which programs that ask are told.
    pub fn offer_modes(&mut self, output: &Output, modes: &[OfferedMode], driven: OfferedMode) {
        let head = self.heads.iter_mut().find(|head| head.output == *output);
        if let Some(head) = head {
            head.modes = modes.to_vec();
            head.mode = Some(driven);
        }
    }

    /// Takes an output away. Windows left stranded move to the first of
    /// those that remain, and panels that were on it are closed.
    pub fn remove_output(&mut self, output: &Output) {
        let Some(index) = self.heads.iter().position(|head| head.output == *output) else {
            return;
        };
        let head = self.heads.remove(index);
        // Its windows go to the first output that is left.
        self.desktop.remove_output(index);
        for layer in layer_map_for_output(&head.output).layers() {
            layer.layer_surface().send_close();
        }
        self.space.unmap_output(&head.output);
        self.display.remove_global::<Self>(head.global);
        self.forget_lock_surface(&head.output);
        self.apply_scale();
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

    /// The screens as the configuration has them for those called `names`:
    /// the first arrangement that names just those, or else `outputs`.
    pub(crate) fn arrangement_for<'a>(
        &self,
        names: &(impl Iterator<Item = &'a str> + Clone),
    ) -> &omorfos_config::Outputs {
        let general = &self.config().general;
        let mut arrangements = general.arrangements.iter();
        let found = arrangements.find(|arrangement| arrangement.are(names.clone()));
        found.unwrap_or(&general.outputs)
    }

    /// The screens as the configuration has them for those plugged in.
    pub(crate) fn arrangement(&self) -> &omorfos_config::Outputs {
        let names: Vec<String> = self.heads.iter().map(|head| head.output.name()).collect();
        self.arrangement_for(&names.iter().map(String::as_str))
    }

    /// Where among the outputs the one that comes first is: the one the
    /// configuration makes primary, if it is plugged in, or else the
    /// first plugged in.
    pub(crate) fn primary(&self) -> usize {
        let listed = self.arrangement().listed().iter();
        let named = listed
            .filter(|listed| listed.primary)
            .map(|listed| listed.name.as_str());
        let named: Vec<&str> = named.collect();
        let at = self
            .heads
            .iter()
            .position(|head| named.contains(&head.output.name().as_str()));
        at.unwrap_or(0)
    }

    /// The output that comes first, if there is one.
    pub(crate) fn primary_head(&self) -> Option<&Head> {
        self.heads.get(self.primary())
    }

    /// The output the pointer is on, or failing that the one that comes
    /// first.
    pub(crate) fn pointer_head(&self) -> Option<&Head> {
        self.head_at(self.pointer_location)
            .or_else(|| self.primary_head())
    }

    /// The scale things at the pointer are drawn at.
    pub(crate) fn pointer_scale(&self) -> Scale {
        self.pointer_head().map_or(Scale::ONE, |head| head.scale)
    }

    /// The output most of a rectangle of the space is on, or failing that
    /// the one that comes first.
    pub(crate) fn head_for(&self, rectangle: Rect) -> Option<&Head> {
        let overlap = |head: &&Head| {
            let shared = domain(head.area()).intersection(rectangle);
            shared.map_or(0, |shared| {
                u64::from(shared.size.width) * u64::from(shared.size.height)
            })
        };
        let most = self.heads.iter().max_by_key(overlap);
        most.filter(|head| overlap(head) > 0)
            .or_else(|| self.primary_head())
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

    /// The scale for the output called `name`: what the configuration says
    /// for it by name, or else for every output, or else what the backend
    /// suggested.
    fn scale_for(&self, name: &str, suggested: Scale) -> Scale {
        let general = &self.config().general;
        let named = self
            .arrangement()
            .named(name)
            .and_then(|output| output.scale);
        bridge::scale(named.unwrap_or(general.output.scale), suggested)
    }

    /// Settles each output's scale and place, from the configuration and
    /// the backend's suggestions, and lays the outputs out again.
    pub(crate) fn apply_scale(&mut self) {
        let scales = self.heads.iter();
        let scales: Vec<(Scale, Transform)> = scales
            .map(|head| {
                let name = head.output.name();
                (
                    self.scale_for(&name, head.suggested),
                    self.rotation_for(&name),
                )
            })
            .collect();
        for (head, (scale, rotation)) in self.heads.iter_mut().zip(scales) {
            if (scale, rotation) != (head.scale, head.rotation) {
                (head.scale, head.rotation) = (scale, rotation);
                let pixels = head.pixels();
                head.set_pixels((pixels.w, pixels.h));
            }
        }
        self.lay_out();
    }

    /// Which way the configuration says the output called `name` is
    /// turned, as the protocol says it: a turn to the left is a quarter
    /// turn against the clock.
    fn rotation_for(&self, name: &str) -> Transform {
        let listed = self.arrangement().named(name);
        match listed.map(|listed| listed.rotate).unwrap_or_default() {
            Rotation::Normal => Transform::Normal,
            Rotation::Left => Transform::_90,
            Rotation::Inverted => Transform::_180,
            Rotation::Right => Transform::_270,
        }
    }

    /// The outputs in the order they stand, from the left: those the
    /// configuration lists, in the order it lists them, and then the rest
    /// in the order they were added.
    pub(crate) fn arranged(&self) -> Vec<usize> {
        let listed = self.arrangement().listed();
        let place = |index: &usize| {
            let name = self.heads[*index].output.name();
            let place = listed.iter().position(|output| output.name == name);
            // Those not listed come after all that are.
            place.unwrap_or(usize::MAX)
        };
        let mut order: Vec<usize> = (0..self.heads.len()).collect();
        // The sort is stable, so those not listed keep their order.
        order.sort_by_key(place);
        order
    }

    /// Stands each output where the configuration says, next to the one
    /// listed before it, or else to the right of all the others, and
    /// brings everything that depends on where they are up to date.
    fn lay_out(&mut self) {
        let order = self.arranged();
        let listed = self.arrangement();
        let placed: Vec<_> = order
            .iter()
            .map(|index| {
                let head = &self.heads[*index];
                let named = listed.named(&head.output.name());
                let placing =
                    named.map(|named| (named.place.unwrap_or_default(), named.align, named.offset));
                (head.area().size, placing)
            })
            .collect();
        for (index, at) in order.into_iter().zip(arrangement(&placed)) {
            let head = &self.heads[index];
            head.output.change_current_state(None, None, None, Some(at));
            self.space.map_output(&head.output, at);
            layer_map_for_output(&head.output).arrange();
        }
        self.desktop.set_primary(self.primary());
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

/// How a listed screen stands next to the one before it: on which side,
/// where along the edge they share it is lined up, if that is said, and how
/// far along it it is moved from there.
type Placing = (Place, Option<Align>, Option<Offset>);

/// Where screens stand, `screens` large, in the order given: each next to
/// the one before it as its placing says, or, for one that is not listed,
/// to the right of all those before it, its top in line with theirs. Each
/// corner, from the top left corner of all they cover.
fn arrangement(screens: &[(Size<i32, Logical>, Option<Placing>)]) -> Vec<Point<i32, Logical>> {
    let mut stood: Vec<Rectangle<i32, Logical>> = Vec::new();
    for &(size, placing) in screens {
        let before = stood.last().copied();
        let all = stood.iter().copied().reduce(Rectangle::merge);
        let corner = match (before, placing) {
            (None, _) => (0, 0),
            (Some(_), None) => all.map_or((0, 0), |all| (all.loc.x + all.size.w, all.loc.y)),
            (Some(before), Some(placing)) => beside(before, size, placing),
        };
        stood.push(Rectangle::new(corner.into(), size));
    }
    let left = stood
        .iter()
        .map(|stood| stood.loc.x)
        .min()
        .unwrap_or_default();
    let top = stood
        .iter()
        .map(|stood| stood.loc.y)
        .min()
        .unwrap_or_default();
    stood
        .into_iter()
        .map(|stood| stood.loc - Point::from((left, top)))
        .collect()
}

/// Where the corner of a screen `size` large goes next to one standing at
/// `before`, as `placing` says: side by side lined up at the start, its top
/// with the other's, unless said otherwise, and above or below at the
/// middle; and then moved along the edge they share by the offset.
fn beside(
    before: Rectangle<i32, Logical>,
    size: Size<i32, Logical>,
    (place, align, offset): Placing,
) -> (i32, i32) {
    let beside = matches!(place, Place::Right | Place::Left);
    let align = align.unwrap_or(if beside { Align::Start } else { Align::Middle });
    // Along the edge they share: where the other begins, how long it is
    // there, and how long this one is.
    let lined = |from: i32, other: i32, length: i32| {
        let along = match align {
            Align::Start => from,
            Align::Middle => from + (other - length) / 2,
            Align::End => from + other - length,
        };
        along + offset.map_or(0, |offset| offset.along(length))
    };
    match place {
        Place::Right => (
            before.loc.x + before.size.w,
            lined(before.loc.y, before.size.h, size.h),
        ),
        Place::Left => (
            before.loc.x - size.w,
            lined(before.loc.y, before.size.h, size.h),
        ),
        Place::Below => (
            lined(before.loc.x, before.size.w, size.w),
            before.loc.y + before.size.h,
        ),
        Place::Above => (
            lined(before.loc.x, before.size.w, size.w),
            before.loc.y - size.h,
        ),
    }
}

/// What is wrong with where the screens are said to stand: the first one
/// listed placed next to the one before it, which there is not, in
/// `outputs` or in any of the arrangements.
pub(crate) fn misplaced(general: &omorfos_config::General) -> Vec<String> {
    let all = std::iter::once(&general.outputs).chain(&general.arrangements);
    all.filter_map(misplaced_in).collect()
}

/// The same, for one list of screens.
fn misplaced_in(outputs: &omorfos_config::Outputs) -> Option<String> {
    let first = outputs.listed().first();
    let placed = first
        .filter(|first| first.place.is_some() || first.align.is_some() || first.offset.is_some());
    placed.map(|first| {
        format!(
            "config.yaml: `{}` is the first screen listed under `outputs`, with none before it \
             to stand next to, so its `place`, `align` and `offset` are not used",
            first.name
        )
    })
}
