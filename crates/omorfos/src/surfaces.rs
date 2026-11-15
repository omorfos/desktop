//! What programs show, as what is drawn of it: every surface of a window,
//! a pop-up, a panel or a pointer, but one said to be larger than anything
//! is.
//!
//! A surface is as large as its program says, and with `wp_viewporter` four
//! pixels can be said to be two thousand million units across. Nothing that
//! large is a window, and nothing good comes of handing it on: the numbers
//! it is placed by run out, and a renderer may refuse to draw anything at
//! all while it is there, the rest of the screen with it. So a surface
//! larger than a frame may be, [`Desktop::MOST_SIDE`] either way, is left
//! out of what is drawn, and the rest of what its program shows is drawn
//! without it (ADR 91).
//!
//! Everything a program shows is made into what is drawn here and nowhere
//! else, so that this holds for all of it: `clippy.toml` refuses the
//! functions of Smithay's this stands in front of anywhere but in this
//! file.

use smithay::backend::renderer::element::surface::{
    WaylandSurfaceRenderElement, render_elements_from_surface_tree,
};
use smithay::backend::renderer::element::{AsRenderElements, Kind};
use smithay::backend::renderer::{ImportAll, Renderer};
use smithay::desktop::Window;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Physical, Point, Rectangle, Scale};

use omorfos_core::Desktop;

/// What `surface` and the surfaces on it show, from `at`, as `scale` and
/// `opacity` say and as the `kind` of thing it is.
#[expect(
    clippy::disallowed_methods,
    reason = "the one place a tree of surfaces is made into what is drawn"
)]
pub(crate) fn tree<R>(
    renderer: &mut R,
    surface: &WlSurface,
    at: impl Into<Point<i32, Physical>>,
    scale: impl Into<Scale<f64>>,
    opacity: f32,
    kind: Kind,
) -> Vec<WaylandSurfaceRenderElement<R>>
where
    R: Renderer + ImportAll,
    R::TextureId: Clone + 'static,
{
    drawable(render_elements_from_surface_tree(
        renderer, surface, at, scale, opacity, kind,
    ))
}

/// What `shown` shows, a window with its pop-ups or a panel, from `at`, as
/// `scale` and `opacity` say.
#[expect(
    clippy::disallowed_methods,
    reason = "the one place a window or a panel is made into what is drawn"
)]
pub(crate) fn of<R, S>(
    renderer: &mut R,
    shown: &S,
    at: Point<i32, Physical>,
    scale: impl Into<Scale<f64>>,
    opacity: f32,
) -> Vec<WaylandSurfaceRenderElement<R>>
where
    R: Renderer + ImportAll,
    S: AsRenderElements<R, RenderElement = WaylandSurfaceRenderElement<R>>,
{
    drawable(shown.render_elements(renderer, at, scale.into(), opacity))
}

/// The part of what `window`'s program shows that it says is the window
/// proper: where that begins in what it shows, and how large it is, which
/// is taken to be no larger than a frame may be whatever is said.
#[expect(
    clippy::disallowed_methods,
    reason = "the one place a window's own word for how large it is is taken"
)]
pub(crate) fn proper(window: &Window) -> Rectangle<i32, Logical> {
    let said = window.geometry();
    let most = Desktop::MOST_SIDE.cast_signed();
    Rectangle::new(
        said.loc,
        (said.size.w.min(most), said.size.h.min(most)).into(),
    )
}

/// `shown` without the surfaces said to be larger than a frame may be.
fn drawable<R>(
    mut shown: Vec<WaylandSurfaceRenderElement<R>>,
) -> Vec<WaylandSurfaceRenderElement<R>>
where
    R: Renderer + ImportAll,
{
    let most = Desktop::MOST_SIDE.cast_signed();
    shown.retain(|surface| {
        let size = surface.view().dst;
        size.w <= most && size.h <= most
    });
    shown
}
