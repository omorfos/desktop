//! Parts drawn larger or smaller than they are, as they come and go: from
//! how large their theme's `@starting-style` says, and to how large it says
//! they are while `:closing`.
//!
//! What a part draws is drawn as it is, and then made larger or smaller,
//! about one point, as a picture would be: nothing of it is drawn again,
//! nor its shadow cast again, at each size it passes through.

use smithay::backend::renderer::element::utils::RescaleRenderElement;
use smithay::backend::renderer::element::{Element as Drawn, Id, RenderElement};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{ImportAll, ImportMem, Renderer};
use smithay::utils::{Buffer, Physical, Point, Rectangle, Scale as Factor, Transform};

use crate::compositor::Element;
use crate::glass::Pane;

/// The point a part is made larger or smaller about, and how much.
pub(crate) type Around = (Point<i32, Physical>, f64);

/// One element of a part being made larger or smaller, kept in a box of
/// its own so that an element can hold another.
pub(crate) struct Boxed<R: Renderer>(Box<Element<R>>);

impl<R> Drawn for Boxed<R>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: 'static,
{
    fn id(&self) -> &Id {
        self.0.id()
    }

    fn current_commit(&self) -> CommitCounter {
        self.0.current_commit()
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        self.0.src()
    }

    fn transform(&self) -> Transform {
        self.0.transform()
    }

    fn geometry(&self, scale: Factor<f64>) -> Rectangle<i32, Physical> {
        self.0.geometry(scale)
    }

    fn alpha(&self) -> f32 {
        self.0.alpha()
    }
}

impl<R> RenderElement<R> for Boxed<R>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: 'static,
{
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        self.0.draw(frame, src, dst, damage, opaque_regions)
    }
}

/// How much larger or smaller a part `scale` as large as it is is drawn,
/// and about its `middle`, which is only worked out if it is not drawn as
/// large as it is: most of the time, nothing.
pub(crate) fn around(scale: f32, middle: impl FnOnce() -> Point<i32, Physical>) -> Option<Around> {
    let scale = f64::from(scale);
    ((scale - 1.0).abs() > f64::EPSILON).then(|| (middle(), scale))
}

/// Has the elements from `first` on drawn as large as `around` says, about
/// its point, if they are made larger or smaller.
pub(crate) fn scale<R>(elements: &mut Vec<Element<R>>, first: usize, around: Option<Around>)
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: 'static,
{
    if let Some((middle, scale)) = around {
        let made = elements.split_off(first.min(elements.len()));
        elements.extend(made.into_iter().map(|element| {
            let boxed = Boxed(Box::new(element));
            Element::Scaled(RescaleRenderElement::from_element(boxed, middle, scale))
        }));
    }
}

/// Has the glass from `first` on made as large as `around` says, with the
/// part it is under.
pub(crate) fn scale_panes(panes: &mut Vec<Pane>, first: usize, around: Option<Around>) {
    if let Some(around) = around {
        let made = panes.split_off(first.min(panes.len()));
        panes.extend(made.into_iter().map(|pane| pane.scaled(around)));
    }
}

/// The middle of everything the elements from `first` on cover, at `scale`.
pub(crate) fn middle<R>(
    elements: &[Element<R>],
    first: usize,
    scale: Factor<f64>,
) -> Point<i32, Physical>
where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: 'static,
{
    let drawn = elements.get(first..).unwrap_or_default().iter();
    let all = drawn
        .map(|element| element.geometry(scale))
        .reduce(Rectangle::merge);
    let all = all.unwrap_or_default();
    all.loc + Point::from((all.size.w / 2, all.size.h / 2))
}

/// Where `area` is once made as large as `around` says, about its point.
pub(crate) fn scaled(
    area: Rectangle<i32, Physical>,
    (middle, scale): Around,
) -> Rectangle<i32, Physical> {
    let moved = |point: Point<i32, Physical>| {
        let away = (point - middle).to_f64();
        middle + Point::from((away.x * scale, away.y * scale)).to_i32_round()
    };
    let from = moved(area.loc);
    let to = moved(area.loc + area.size.to_point());
    Rectangle::new(from, (to - from).to_size())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_area_is_made_smaller_towards_the_point_and_larger_away_from_it() {
        let area = Rectangle::new(Point::from((100, 100)), (200, 100).into());
        let middle = Point::from((200, 150));
        let smaller = scaled(area, (middle, 0.5));
        assert_eq!(smaller, Rectangle::new((150, 125).into(), (100, 50).into()));
        let larger = scaled(area, (Point::from((100, 100)), 2.0));
        assert_eq!(larger, Rectangle::new((100, 100).into(), (400, 200).into()));
        assert_eq!(scaled(area, (middle, 1.0)), area);
    }
}
