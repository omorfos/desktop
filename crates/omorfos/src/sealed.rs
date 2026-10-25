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
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
use smithay::backend::renderer::element::surface::render_elements_from_surface_tree;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::{WindowSurfaceType, layer_map_for_output};
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::shell::wlr_layer::Layer;

use crate::compositor::{Compositor, Element, View, color};
use crate::roles::Roles;

impl Compositor {
    /// The prompt that asks for a secret, if one is up, the screen it is on
    /// and where on the screens it is.
    pub(crate) fn sealed(&self) -> Option<(Output, WlSurface, Point<i32, Logical>)> {
        self.heads.iter().find_map(|head| {
            let layers = layer_map_for_output(&head.output);
            let asks = layers.layers_on(Layer::Overlay).find(|layer| {
                let client = layer.wl_surface().client();
                Self::roles(client.as_ref()).has(Roles::ASK)
            });
            asks.map(|layer| {
                let within = crate::compositor::layer_origin(&layers, layer);
                let at = head.area().loc + within;
                (head.output.clone(), layer.wl_surface().clone(), at)
            })
        })
    }

    /// Points the pointer at the prompt, if it is over it, and at nothing
    /// otherwise: nothing else is to be reached while it asks.
    pub(crate) fn point_at_sealed(&mut self, time: u32) {
        let point = self.pointer_location;
        let focus = self.heads.iter().find_map(|head| {
            let layers = layer_map_for_output(&head.output);
            let asks = layers.layers_on(Layer::Overlay).find(|layer| {
                let client = layer.wl_surface().client();
                Self::roles(client.as_ref()).has(Roles::ASK)
            });
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
    /// and nothing is drawn again for it.
    ///
    /// Nothing half done goes on under a prompt: a menu or a drag would
    /// not see the keys or the button that end it.
    pub(crate) fn update_seal(&mut self) {
        if self.sealed().is_some() {
            self.menu = None;
            self.drag = None;
        }
        let dim = self
            .config()
            .theme
            .style(Part::Sealed, States::NONE)
            .background;
        let heads = self
            .heads
            .iter()
            .map(|head| (head.output.clone(), head.area().size));
        let heads: Vec<_> = heads.collect();
        self.seal
            .retain(|(output, _)| heads.iter().any(|(head, _)| head == output));
        for (output, size) in heads {
            let known = self.seal.iter().position(|(kept, _)| *kept == output);
            let index = known.unwrap_or_else(|| {
                self.seal.push((output, SolidColorBuffer::default()));
                self.seal.len() - 1
            });
            self.seal[index].1.update(size, color(dim));
        }
    }

    /// While a prompt asks for a secret: on its screen the prompt, over a
    /// dim of every screen, under which is what is shown as ever.
    pub(crate) fn sealed_elements<R>(
        &self,
        renderer: &mut R,
        output: &Output,
        view: View,
        elements: &mut Vec<Element<R>>,
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
        let dim = self.seal.iter().find(|(kept, _)| kept == output);
        let dim = dim.map(|(_, dim)| {
            let scale = view.scale.factor();
            SolidColorRenderElement::from_buffer(dim, (0, 0), scale, 1.0, Kind::Unspecified)
        });
        elements.extend(dim.map(Element::Solid));
    }
}
