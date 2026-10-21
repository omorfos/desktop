//! Typing through an input method: the program that turns keys into the
//! characters of a language that has more of them than a keyboard has
//! keys, and shows the ones it could mean beside where they are typed.
//!
//! A program's text field says what it is (`text-input-v3`), and the input
//! method, told of it, says what to put there (`input-method-v2`). It may
//! take every key while a field is being typed in, and pass on those it
//! has no use for through a keyboard of its own (`virtual-keyboard-v1`).
//! What it is shown is what is typed, passwords among it, and what it
//! types goes wherever the keyboard is, so both are for a program with
//! the `input` role alone (ADR 37).
//!
//! See `docs/adr/0047-input-methods.md`.

use smithay::backend::renderer::element::Kind;
use smithay::backend::renderer::element::surface::render_elements_from_surface_tree;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer, Texture};
use smithay::desktop::utils::bbox_from_surface_tree;
use smithay::desktop::{WindowSurfaceType, layer_map_for_output};
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::input_method::{InputMethodHandler, InputMethodManagerState, PopupSurface};
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::virtual_keyboard::VirtualKeyboardManagerState;
use smithay::{
    delegate_input_method_manager, delegate_text_input_manager, delegate_virtual_keyboard_manager,
};

use crate::compositor::{Compositor, Element, View};
use crate::outputs::Head;

impl Compositor {
    /// Where the top left of `surface` is, if it is the main surface of a
    /// window or of a panel that is shown: what has the keyboard, and so
    /// what text is typed into. While the session is locked no popup is
    /// shown at all.
    fn surface_origin(&self, surface: &WlSurface) -> Option<Point<i32, Logical>> {
        let window = self.scene.iter().find(|shown| shown.surface == *surface);
        let window = window.map(|shown| shown.origin - shown.window.geometry().loc);
        let panel = || {
            self.heads.iter().find_map(|head| {
                let layers = layer_map_for_output(&head.output);
                let layer = layers.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL);
                let within = layer.and_then(|layer| layers.layer_geometry(layer));
                within.map(|within| head.area().loc + within.loc)
            })
        };
        window.or_else(panel)
    }

    /// Where on the screens an input method's popup is shown: under the
    /// text it is for, as the program says where that is, and kept on the
    /// screen it is on, going above the text where there is no room under.
    fn popup_area(&self, popup: &PopupSurface) -> Option<Rectangle<i32, Logical>> {
        let parent = popup.get_parent();
        let origin = parent.and_then(|parent| self.surface_origin(&parent.surface));
        let text = popup.text_input_rectangle();
        let text = origin.map(|origin| Rectangle::new(origin + text.loc, text.size));
        let size = bbox_from_surface_tree(popup.wl_surface(), (0, 0)).size;
        // The screen the text is on, or for text past the edge of every
        // screen, the one its window or panel begins on.
        let on = |at: Point<i32, Logical>| self.head_at(at.to_f64()).map(Head::area);
        let screen = text.zip(origin).and_then(|(text, origin)| {
            let screen = on(text.loc).or_else(|| on(origin));
            screen.map(|screen| (text, screen))
        });
        screen.map(|(text, screen)| {
            let below = text.loc.y + text.size.h;
            let room = below + size.h <= screen.loc.y + screen.size.h;
            let y = if room { below } else { text.loc.y - size.h };
            let right = screen.loc.x + screen.size.w - size.w;
            let x = text.loc.x.min(right).max(screen.loc.x);
            Rectangle::new((x, y.max(screen.loc.y)).into(), size)
        })
    }

    /// The popups of the input method, over everything but the pointer,
    /// since the text they are for may be in anything, the launcher too.
    pub(crate) fn popup_elements<R>(
        &self,
        renderer: &mut R,
        view: View,
        elements: &mut Vec<Element<R>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Texture + Send + 'static,
    {
        let placed = self.typing.popups.iter();
        let placed = placed.filter_map(|popup| self.popup_area(popup).map(|area| (popup, area)));
        for (popup, area) in placed {
            let shown = render_elements_from_surface_tree(
                renderer,
                popup.wl_surface(),
                view.pixel(area.loc.to_f64()),
                view.scale.factor(),
                1.0,
                Kind::Unspecified,
            );
            elements.extend(shown.into_iter().map(Element::Surface));
        }
    }
}

/// The input method, and what is known of it.
#[derive(Debug)]
pub(crate) struct Typing {
    /// Its popups that are to be shown, by the text they are for.
    pub(crate) popups: Vec<PopupSurface>,
    /// Text fields saying what they are; the input method, for a program
    /// with the role; and the keyboard it types with, for the same.
    _offered: (
        TextInputManagerState,
        InputMethodManagerState,
        VirtualKeyboardManagerState,
    ),
}

impl Typing {
    /// Offers what fields and an input method speak.
    pub(crate) fn offer(display: &DisplayHandle) -> Self {
        Self {
            popups: Vec::new(),
            _offered: (
                TextInputManagerState::new::<Compositor>(display),
                InputMethodManagerState::new::<Compositor, _>(display, Compositor::types),
                VirtualKeyboardManagerState::new::<Compositor, _>(display, Compositor::types),
            ),
        }
    }
}

impl InputMethodHandler for Compositor {
    // Smithay dismisses a popup before it is shown again, for other text.
    fn new_popup(&mut self, surface: PopupSurface) {
        self.typing.popups.push(surface);
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        self.typing.popups.retain(|popup| *popup != surface);
    }

    // It is placed each time it is drawn, by where the text is then.
    fn popup_repositioned(&mut self, _surface: PopupSurface) {}

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        let origin = self.surface_origin(parent).unwrap_or_default();
        Rectangle::new(origin, (0, 0).into())
    }
}

delegate_text_input_manager!(Compositor);
delegate_input_method_manager!(Compositor);
delegate_virtual_keyboard_manager!(Compositor);
