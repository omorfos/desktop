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

use std::time::Duration;

use omorfos_config::{Part, States};
use omorfos_ui::Area;
use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::solid::{SolidColorBuffer, SolidColorRenderElement};
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
use crate::surfaces;

/// What is drawn round a prompt that asks for a secret.
#[derive(Debug, Default)]
pub(crate) struct Seal {
    /// What dims each screen.
    dims: Vec<(Output, SolidColorBuffer)>,
    /// The shadow the prompt's box casts on the dim, as a question's.
    shadow: Option<Shadowing>,
}

/// How long the keyboard is held back from everything once a prompt that
/// asked for a secret has gone without being answered, after the last key
/// went down or up: as long as browsers keep a question from being
/// answered by what was meant for something else.
pub(crate) const HELD_BACK: Duration = Duration::from_millis(500);

/// What becomes of the keyboard as a prompt that asks for a secret goes
/// (#180, finding 26). A prompt goes when the user answers it, and also
/// when what asked gives up, as polkit does when the program that asked
/// ends its request: then whoever is typing goes on typing the secret, and
/// Enter, into whatever would have the keyboard next. So a prompt that
/// goes unanswered leaves the keyboard to nothing until every key is up and
/// [`HELD_BACK`] has gone by with none pressed, or until the pointer is
/// pressed, where the user means to type next; what is typed meanwhile
/// reaches nothing.
#[derive(Debug, Default)]
pub(crate) struct Withholding {
    /// Whether a prompt was up as the keyboard was last given out.
    up: bool,
    /// Whether the last thing done to it was an answer: Enter or Escape,
    /// or a press of the pointer.
    answered: bool,
    /// While the keyboard is held back: when a key last went down or up,
    /// or the prompt went, by the compositor's clock.
    since: Option<Duration>,
}

impl Withholding {
    /// Takes in that a prompt is `up` or not, at `now`: one that has gone
    /// unanswered holds the keyboard back.
    pub(crate) fn seen(&mut self, up: bool, now: Duration) {
        if self.up && !up && !self.answered {
            self.since = Some(now);
        }
        // A prompt put up anew has the keyboard, and what became of the
        // last no longer counts.
        if up && !self.up {
            self.answered = false;
            self.since = None;
        }
        self.up = up;
    }

    /// Takes in a key pressed in the prompt, `answer` if it answers it.
    pub(crate) const fn typed(&mut self, answer: bool) {
        self.answered = answer;
    }

    /// Whether the keyboard is held back from everything.
    pub(crate) const fn holding(&self) -> bool {
        self.since.is_some()
    }

    /// Takes in a key going down or up while the keyboard is held back, at
    /// `now`: it is held back from then.
    pub(crate) const fn touched(&mut self, now: Duration) {
        self.since = Some(now);
    }

    /// Lets the keyboard go to whoever would have it, as a press of the
    /// pointer does.
    pub(crate) const fn release(&mut self) {
        self.since = None;
    }

    /// Lets the keyboard go if no key is `down` and it has been held back
    /// long enough by `now`.
    pub(crate) fn settle(&mut self, down: bool, now: Duration) {
        let long = self.since.is_some_and(|since| now >= since + HELD_BACK);
        if long && !down {
            self.since = None;
        }
    }

    /// When the keyboard is next to be looked at again, by the compositor's
    /// clock, while it is held back.
    pub(crate) fn due(&self) -> Option<Duration> {
        self.since.map(|since| since + HELD_BACK)
    }
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

    /// What seals the session, if anything does: the screen it is on, where
    /// its box is on the screens and the scale of that screen. A prompt
    /// that asks for a secret, or the question whether to start the
    /// machine again or turn it off, which is drawn alike.
    fn sealing(&self) -> Option<(Output, Area, Scale)> {
        let asking = self.cards.leave.shown.as_ref();
        let asking = asking.filter(|_| self.cards.leave.asked.is_some());
        let asked = asking.map(crate::leaving::Asking::sealed);
        self.sealed_box().or(asked)
    }

    /// Where the prompt's box is on the screens, if one is up, the screen
    /// it is on and its scale.
    fn sealed_box(&self) -> Option<(Output, Area, Scale)> {
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
                (head.output.clone(), area, head.scale)
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
        if self.sealing().is_some() {
            self.menu = None;
            self.drag = None;
        }
        let theme = &self.store.current().theme;
        let dim = theme.style(Part::Sealed, States::NONE).ground();
        let question = theme.style(Part::Question, States::NONE);
        let had = self.seal.shadow.take();
        self.seal.shadow = self
            .sealing()
            .and_then(|(_, area, scale)| self.animation.shades.cast(&question, scale, area, had));
        let heads = &self.heads;
        let dims = &mut self.seal.dims;
        dims.retain(|(output, _)| heads.iter().any(|head| head.output == *output));
        for head in heads {
            let (output, size) = (&head.output, head.area().size);
            let known = dims.iter().position(|(kept, _)| kept == output);
            let index = known.unwrap_or_else(|| {
                dims.push((output.clone(), SolidColorBuffer::default()));
                dims.len() - 1
            });
            dims[index].1.update(size, color(dim));
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
        // The question whether to leave, over the dim as a prompt is.
        let asked = self.cards.leave.asked.is_some();
        let asking = self.cards.leave.shown.as_ref();
        let asking = asking.filter(|asking| asked && asking.output == *output);
        let card = asking.and_then(|asking| {
            let at = asking.at.to_f64();
            asking.pixels.element(renderer, at, view, 1.0)
        });
        elements.extend(card);
        // The prompt itself, if it is a program's that asks for a secret.
        let prompt = self.sealed().filter(|(on, ..)| on == output);
        let shown = prompt.into_iter().flat_map(|(_, surface, at)| {
            surfaces::tree(
                renderer,
                &surface,
                view.pixel(at.to_f64()),
                view.scale.factor(),
                1.0,
                Kind::Unspecified,
            )
        });
        elements.extend(shown.map(Element::Surface));
        let Some((on, ..)) = self.sealing() else {
            return;
        };
        // Its glass is under its box, and what is behind that begins under
        // its shadow: the dim, and the session under it.
        let question = self.config().theme.style(Part::Question, States::NONE);
        let boxed = self.sealing().filter(|_| on == *output);
        let glass = boxed.and_then(|(_, (x, y, width, height), _)| {
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
        let whole = head.map(|head| view.pixels(head.area()));
        let key = glass::key(&("the sealed dim", output.name()));
        let under = elements.len();
        let blurred = whole.filter(|_| sealed.blur > 0 && sealed.see_through());
        let blurred = blurred.and_then(|whole| {
            Pane::behind(vec![whole], (sealed.blur, view), (key, under, under), 1.0)
        });
        panes.extend(blurred);
    }
}
