//! The sealed prompt: what asks for a secret, such as the administrator's
//! password, shown so that no program can imitate it.
//!
//! It is a surface on the overlay layer of a program with the `ask` role.
//! While one is up the compositor dims everything else, which no program
//! can do to the screen, and gives the keyboard to the prompt alone: no
//! binding, no other program and no input method sees a key, and the
//! pointer reaches nothing else. A window that draws a password box is
//! just a window, with the desktop bright round it.
//!
//! See `docs/adr/0037-who-may-do-what.md` and `docs/adr/0048-asking-for-a-secret.md`.

use omorfos_config::{Part, States};
use omorfos_ui::Area;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::render_elements_from_surface_tree;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle, Size};
use smithay::wayland::compositor::{RectangleKind, SurfaceAttributes, with_states};
use smithay::wayland::shell::wlr_layer::Layer;

use crate::Scale;
use crate::compositor::{Compositor, Element, View, color};
use crate::glass::{self, Pane};
use crate::roles::Roles;
use crate::shadows::Shadowing;

/// What is drawn round a prompt that asks for a secret.
#[derive(Debug, Default)]
pub(crate) struct Seal {
    /// What dims each screen.
    dims: Vec<(Output, SolidColorBuffer)>,
    /// The shadow the prompt's box casts on the dim, as a question's.
    shadow: Option<Shadowing>,
}

/// Whether `layer` is a prompt that asks for a secret: its program has
/// the role.
fn asks(layer: &LayerSurface) -> bool {
    let client = layer.wl_surface().client();
    Compositor::roles(client.as_ref()).has(Roles::ASK)
}

/// Where the prompt's box is on its surface of `size`: within the bounds
/// of the region it takes the pointer in, as it keeps room beside the box
/// to shake it in and moves that region with the box; the whole surface
/// where it says no region, as other panels are taken to be.
fn boxed(surface: &WlSurface, size: Size<i32, Logical>) -> Rectangle<i32, Logical> {
    let whole = Rectangle::from_size(size);
    let region = with_states(surface, |states| {
        let mut attributes = states.cached_state.get::<SurfaceAttributes>();
        attributes.current().input_region.clone()
    });
    let rects = region.iter().flat_map(|region| region.rects.iter());
    let added = rects.filter(|(kind, _)| matches!(kind, RectangleKind::Add));
    let bounds = added.map(|(_, rect)| *rect).reduce(Rectangle::merge);
    bounds
        .and_then(|bounds| bounds.intersection(whole))
        .unwrap_or(whole)
}

impl Compositor {
    /// The prompt that asks for a secret, if one is up, the screen it is on
    /// and where on the screens it is.
    pub(crate) fn sealed(&self) -> Option<(Output, WlSurface, Point<i32, Logical>)> {
        self.heads.iter().find_map(|head| {
            let layers = layer_map_for_output(&head.output);
            let asks = layers.layers_on(Layer::Overlay).find(|layer| asks(layer));
            asks.map(|layer| {
                let within = crate::compositor::layer_origin(&layers, layer);
                let at = head.area().loc + within;
                (head.output.clone(), layer.wl_surface().clone(), at)
            })
        })
    }

    /// Where the prompt's box is on the screens, if one is up, and the
    /// scale of the screen it is on.
    fn sealed_box(&self) -> Option<(Area, Scale)> {
        self.heads.iter().find_map(|head| {
            let layers = layer_map_for_output(&head.output);
            let asks = layers.layers_on(Layer::Overlay).find(|layer| asks(layer));
            asks.map(|layer| {
                let size = layers.layer_geometry(layer).unwrap_or_default().size;
                let within = boxed(layer.wl_surface(), size);
                let at = head.area().loc + crate::compositor::layer_origin(&layers, layer);
                let at = at + within.loc;
                let size = within.size;
                let area = (at.x, at.y, size.w.unsigned_abs(), size.h.unsigned_abs());
                (area, head.scale)
            })
        })
    }

    /// Points the pointer at the prompt, if it is over it, and at nothing
    /// otherwise: nothing else is to be reached while it asks.
    pub(crate) fn point_at_sealed(&mut self, time: u32) {
        let point = self.pointer_location;
        let focus = self.heads.iter().find_map(|head| {
            let layers = layer_map_for_output(&head.output);
            let asks = layers.layers_on(Layer::Overlay).find(|layer| asks(layer));
            asks.and_then(|layer| {
                let within = crate::compositor::layer_origin(&layers, layer);
                let origin = head.area().loc + within;
                let under = layer.surface_under(point - origin.to_f64(), WindowSurfaceType::ALL);
                under.map(|(surface, offset)| (surface, (offset + origin).to_f64()))
            })
        });
        self.move_pointer(focus, time);
    }

    /// Keeps the dim of each screen as large as the screen and of the
    /// theme's colour, so that it is the same from one frame to the next
    /// and nothing is drawn again for it; and the shadow the prompt's box
    /// casts, as a question's, where the box is.
    ///
    /// Nothing half done goes on under a prompt: a menu or a drag would
    /// not see the keys or the button that end it.
    pub(crate) fn update_seal(&mut self) {
        if self.sealed().is_some() {
            self.menu = None;
            self.drag = None;
        }
        let theme = &self.store.current().theme;
        let dim = theme.style(Part::Sealed, States::NONE).ground();
        let question = theme.style(Part::Question, States::NONE);
        let had = self.seal.shadow.take();
        self.seal.shadow = self
            .sealed_box()
            .and_then(|(area, scale)| self.animation.shades.cast(&question, scale, area, had));
        let heads = self
            .heads
            .iter()
            .map(|head| (head.output.clone(), head.area().size));
        let heads: Vec<_> = heads.collect();
        self.seal
            .dims
            .retain(|(output, _)| heads.iter().any(|(head, _)| head == output));
        for (output, size) in heads {
            let known = self.seal.dims.iter().position(|(kept, _)| *kept == output);
            let index = known.unwrap_or_else(|| {
                self.seal.dims.push((output, SolidColorBuffer::default()));
                self.seal.dims.len() - 1
            });
            self.seal.dims[index].1.update(size, color(dim));
        }
    }

    /// While a prompt asks for a secret: on its screen the prompt, its
    /// glass if the theme makes a question glass, and its shadow, over a
    /// dim of every screen, under which is what is shown as ever.
    pub(crate) fn sealed_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        (elements, panes): (&mut Vec<Element<R>>, &mut Vec<Pane>),
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let Some((on, surface, at)) = self.sealed() else {
            return;
        };
        let here = (on == *output).then_some(&surface).into_iter();
        let shown = here.flat_map(|surface| {
            render_elements_from_surface_tree(
                renderer,
                surface,
                view.pixel(at.to_f64()),
                view.scale.factor(),
                1.0,
                Kind::Unspecified,
            )
        });
        elements.extend(shown.map(Element::Surface));
        // Its glass is under its box, and what is behind that begins under
        // its shadow: the dim, and the session under it.
        let question = self.config().theme.style(Part::Question, States::NONE);
        let boxed = self.sealed_box().filter(|_| on == *output);
        let glass = boxed.and_then(|((x, y, width, height), _)| {
            let area = Rectangle::new(
                (x, y).into(),
                (width.cast_signed(), height.cast_signed()).into(),
            );
            let key = glass::key(&"the sealed prompt");
            Pane::of(&question, (area, view), (key, elements.len(), 0), 1.0)
        });
        let shadow = self.seal.shadow.as_ref().filter(|_| on == *output);
        if let Some(shadow) = shadow {
            shadow.elements(renderer, 1.0, view, elements);
        }
        panes.extend(glass.map(|pane| pane.under(elements.len())));
        let dim = self.seal.dims.iter().find(|(kept, _)| kept == output);
        let dim = dim.map(|(_, dim)| {
            let scale = view.scale.factor();
            SolidColorRenderElement::from_buffer(dim, (0, 0), scale, 1.0, Kind::Unspecified)
        });
        elements.extend(dim.map(Element::Solid));
        // The session is blurred under the dim, if the theme says so. The
        // glass has no colour of its own, the dim being its tint: where
        // blur cannot be drawn there is the dim alone, as there was before
        // the session was ever blurred, and not a screen of solid colour.
        let sealed = self.config().theme.style(Part::Sealed, States::NONE);
        let head = self.heads.iter().find(|head| head.output == *output);
        let whole = head.map(|head| {
            let area = head.area();
            let from = view.pixel(area.loc.to_f64());
            let to = view.pixel((area.loc + area.size.to_point()).to_f64());
            Rectangle::new(from, (to - from).to_size())
        });
        let key = glass::key(&("the sealed dim", output.name()));
        let under = elements.len();
        let blurred = whole.filter(|_| sealed.blur > 0 && sealed.see_through());
        let blurred = blurred.and_then(|whole| {
            Pane::behind(vec![whole], (sealed.blur, view), (key, under, under), 1.0)
        });
        panes.extend(blurred);
    }
}
