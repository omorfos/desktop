//! Drawing a picture of the screen straight into a buffer in the memory of
//! the graphics card, which a program that takes pictures handed over.
//!
//! Only a real buffer of a card reaches the end of this, and CI runners
//! have no card, so the file is left out of the coverage gate and listed in
//! `docs/adr/0002-coverage-policy.md`. It decides nothing: whether a
//! picture goes this way, and what is said when it could not be drawn, is
//! decided where it is called. It is tested on a machine with a card.

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::{Bind, Color32F, Renderer, Texture};
use smithay::utils::{Physical, Size, Transform};

/// Draws `elements` into `buffer`, a picture `size` pixels large at
/// `scale`, on `behind`, and waits until the card has. Returns whether it
/// was drawn.
pub(crate) fn draw<R, E>(
    renderer: &mut R,
    buffer: &mut Dmabuf,
    (size, scale): (Size<i32, Physical>, f64),
    elements: &[E],
    behind: Color32F,
) -> bool
where
    R: Renderer + Bind<Dmabuf>,
    R::TextureId: Texture,
    E: RenderElement<R>,
{
    // All of it is drawn each time: the program's buffers take turns, and
    // drawing on the card costs little.
    let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    let target = renderer.bind(buffer).ok();
    let drawn = target.and_then(|mut target| {
        let drawn = tracker.render_output(renderer, &mut target, 0, elements, behind);
        drawn.ok().map(|drawn| drawn.sync.clone())
    });
    drawn.is_some_and(|sync| sync.wait().is_ok())
}
