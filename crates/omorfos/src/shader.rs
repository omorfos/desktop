//! The blur behind glass made by a shader on the graphics card (ADR 33):
//! the same passes as `omorfos_ui::Blur` makes on the CPU, tap for tap,
//! each drawn into a texture of the size it says.
//!
//! No test in CI has a graphics card, so the file is left out of the
//! coverage gate and listed in `docs/adr/0002-coverage-policy.md`. It
//! decides nothing: what is blurred, when, by how much, at what sizes, and
//! what is done if it cannot be, or if it blurs otherwise than the software
//! does, is decided in covered code. The compositor tries it at startup
//! against the software blur, and the tests that need a card try it so.

use std::any::Any;
use std::rc::Rc;

use omorfos_ui::Blur;
use smithay::backend::allocator::Fourcc;
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::renderer::element::{Element, Id, RenderElement};
use smithay::backend::renderer::gles::{
    GlesError, GlesFrame, GlesRenderer, GlesTexProgram, GlesTexture, Uniform, UniformName,
    UniformType,
};
use smithay::backend::renderer::multigpu::gbm::GbmGlesBackend;
use smithay::backend::renderer::multigpu::{Error as MultiError, MultiRenderer};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::backend::renderer::{Bind, Frame as _, Offscreen, Renderer, Texture};
use smithay::utils::{Buffer, Physical, Rectangle, Scale, Size, Transform};

use crate::glass::Frosting;

/// What every pass begins with: Smithay's place for its own definitions,
/// what it samples, and the sampling itself. Beyond an edge is what is
/// inside it, mirrored, as the software takes it to be: the point is
/// folded back over the edge, then kept to the middles of the pixels along
/// it, between which the sampler would otherwise blend in the other side,
/// since textures made to be drawn into are not told to stop at their
/// edges. Folding the point and then keeping it from the edge samples as a
/// sampler told to mirror would.
const COMMON: &str = r"#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

#if defined(GL_FRAGMENT_PRECISION_HIGH)
precision highp float;
#else
precision mediump float;
#endif

#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

#if defined(DEBUG_FLAGS)
uniform float tint;
#endif

uniform vec2 apart;
uniform vec2 edge;

vec4 tap(vec2 offset) {
    vec2 within = mod(v_coords + offset, 2.0);
    vec2 folded = min(within, vec2(2.0) - within);
    return texture2D(tex, clamp(folded, edge, vec2(1.0) - edge));
}

vec4 shown(vec4 color) {
#if defined(NO_ALPHA)
    color = vec4(color.rgb, 1.0);
#endif
    color = color * alpha;
#if defined(DEBUG_FLAGS)
    if (tint == 1.0)
        color = vec4(0.0, 0.2, 0.0, 0.2) + color * 0.8;
#endif
    return color;
}
";

/// A pass down: the point four times and the four `apart` diagonally,
/// over eight.
const DOWN: &str = r"
void main() {
    vec4 sum = tap(vec2(0.0)) * 4.0
        + tap(-apart) + tap(apart)
        + tap(vec2(apart.x, -apart.y)) + tap(vec2(-apart.x, apart.y));
    gl_FragColor = shown(sum / 8.0);
}
";

/// A pass up: the four twice `apart` across and down once each, and the
/// four `apart` diagonally twice each, over twelve; rounded to a byte, as
/// the software does, then moved away from its luminance by `saturation`,
/// within its alpha.
const UP: &str = r"
uniform float saturation;

void main() {
    vec2 far = apart * 2.0;
    vec4 sum = tap(vec2(-far.x, 0.0)) + tap(vec2(far.x, 0.0))
        + tap(vec2(0.0, -far.y)) + tap(vec2(0.0, far.y))
        + (tap(-apart) + tap(apart)
            + tap(vec2(apart.x, -apart.y)) + tap(vec2(-apart.x, apart.y))) * 2.0;
    vec4 color = floor(sum / 12.0 * 255.0 + 0.5) / 255.0;
    float luminance = dot(color.rgb, vec3(0.2126, 0.7152, 0.0722));
    color.rgb = clamp((color.rgb - luminance) * saturation + luminance, 0.0, color.a);
    gl_FragColor = shown(color);
}
";

/// A renderer the shader runs on: OpenGL ES, by itself, as the nested
/// backend draws, or under the manager of several graphics cards, as the
/// hardware backend does.
pub trait Gles: Renderer {
    /// The renderer underneath.
    fn gles(&mut self) -> &mut GlesRenderer;

    /// The frame underneath `frame`.
    fn frame<'a, 'frame, 'buffer>(
        frame: &'a mut Self::Frame<'frame, 'buffer>,
    ) -> &'a mut GlesFrame<'frame, 'buffer>
    where
        'buffer: 'frame,
        Self: 'frame;

    /// `error`, as this renderer says it.
    fn error(error: GlesError) -> Self::Error;
}

impl Gles for GlesRenderer {
    fn gles(&mut self) -> &mut GlesRenderer {
        self
    }

    fn frame<'a, 'frame, 'buffer>(
        frame: &'a mut GlesFrame<'frame, 'buffer>,
    ) -> &'a mut GlesFrame<'frame, 'buffer>
    where
        'buffer: 'frame,
        Self: 'frame,
    {
        frame
    }

    fn error(error: GlesError) -> GlesError {
        error
    }
}

/// The hardware backend's renderer: OpenGL ES on a card it opened with GBM.
type Card = GbmGlesBackend<GlesRenderer, DrmDeviceFd>;

impl Gles for MultiRenderer<'_, '_, Card, Card> {
    fn gles(&mut self) -> &mut GlesRenderer {
        self.as_mut()
    }

    fn frame<'a, 'frame, 'buffer>(
        frame: &'a mut Self::Frame<'frame, 'buffer>,
    ) -> &'a mut GlesFrame<'frame, 'buffer>
    where
        'buffer: 'frame,
        Self: 'frame,
    {
        frame.as_mut()
    }

    fn error(error: GlesError) -> Self::Error {
        MultiError::Render(error)
    }
}

/// The passes of the blur, compiled for a renderer.
#[derive(Debug, Clone)]
pub struct Shader {
    down: GlesTexProgram,
    up: GlesTexProgram,
}

impl Shader {
    /// The shader, compiled for `renderer`.
    ///
    /// # Errors
    ///
    /// Returns what went wrong if it does not compile.
    pub fn new(renderer: &mut GlesRenderer) -> Result<Self, GlesError> {
        let apart = UniformName::new("apart", UniformType::_2f);
        let edge = UniformName::new("edge", UniformType::_2f);
        let saturation = UniformName::new("saturation", UniformType::_1f);
        let down = renderer.compile_custom_texture_shader(
            format!("{COMMON}{DOWN}"),
            &[apart.clone(), edge.clone()],
        )?;
        let up = renderer
            .compile_custom_texture_shader(format!("{COMMON}{UP}"), &[apart, edge, saturation])?;
        Ok(Self { down, up })
    }
}

/// What the shader made of a picture, kept to be drawn into again: the
/// picture at each size it is made smaller to, and blurred; and what the
/// screen knows it by.
#[derive(Debug)]
struct Made {
    halved: Vec<GlesTexture>,
    blurred: GlesTexture,
    id: Id,
    commit: CommitCounter,
}

impl<R: Gles> Frosting<R, GlesTexture> for Shader {
    fn blur(
        &mut self,
        renderer: &mut R,
        (picture, size): (&mut GlesTexture, Size<i32, Physical>),
        blur: &Blur,
        made: &mut Option<Box<dyn Any>>,
    ) -> bool {
        let gles = renderer.gles();
        let sizes = blur.sizes((size.w.unsigned_abs(), size.h.unsigned_abs()));
        let kept = made.take().and_then(|made| made.downcast::<Made>().ok());
        let kept = kept
            .map(|kept| *kept)
            .or_else(|| made_for(gles, &sizes, size));
        let Some(mut kept) = kept else {
            return false;
        };
        let done = self.passes(gles, picture, &mut kept, blur);
        kept.commit.increment();
        *made = Some(Box::new(kept));
        done.is_ok()
    }

    fn element(
        &self,
        _renderer: &mut R,
        made: &dyn Any,
        (area, _scale, opacity): (Rectangle<i32, Physical>, f64, f32),
    ) -> Option<Rc<dyn RenderElement<R>>> {
        made.downcast_ref::<Made>().map(|made| {
            Rc::new(Blurred {
                id: made.id.clone(),
                commit: made.commit,
                texture: made.blurred.clone(),
                area,
                opacity,
            }) as Rc<dyn RenderElement<R>>
        })
    }
}

impl Shader {
    /// Draws `picture` into `made`'s textures, smaller pass by pass and
    /// back up, as `blur` says.
    fn passes(
        &self,
        gles: &mut GlesRenderer,
        picture: &GlesTexture,
        made: &mut Made,
        blur: &Blur,
    ) -> Result<(), GlesError> {
        let offset = blur.offset();
        let mut from = picture.clone();
        for into in &mut made.halved {
            pass(gles, (&self.down, &from, into), offset, None)?;
            from = into.clone();
        }
        // Back up through the same sizes, the last into the picture's own,
        // with more saturation.
        for index in (0..made.halved.len().saturating_sub(1)).rev() {
            let into = &mut made.halved[index];
            pass(gles, (&self.up, &from, into), offset, Some(1.0))?;
            from = into.clone();
        }
        let saturation = Some(Blur::SATURATION);
        pass(
            gles,
            (&self.up, &from, &mut made.blurred),
            offset,
            saturation,
        )
    }
}

/// Textures for a picture `size` large, made smaller to `sizes`.
fn made_for(
    gles: &mut GlesRenderer,
    sizes: &[(u32, u32)],
    size: Size<i32, Physical>,
) -> Option<Made> {
    let mut texture = |(width, height): (i32, i32)| {
        let size = Size::<i32, Buffer>::from((width, height));
        Offscreen::<GlesTexture>::create_buffer(gles, Fourcc::Abgr8888, size).ok()
    };
    let halved: Option<Vec<_>> = sizes
        .iter()
        .map(|&(width, height)| texture((width.cast_signed(), height.cast_signed())))
        .collect();
    let blurred = texture((size.w, size.h));
    halved.zip(blurred).map(|(halved, blurred)| Made {
        halved,
        blurred,
        id: Id::new(),
        commit: CommitCounter::default(),
    })
}

/// Draws `from` into the whole of `into` with `program`, its samples
/// `offset` pixels of `from` apart, and with `saturation` if it is a pass
/// up.
fn pass(
    gles: &mut GlesRenderer,
    (program, from, into): (&GlesTexProgram, &GlesTexture, &mut GlesTexture),
    offset: f32,
    saturation: Option<f32>,
) -> Result<(), GlesError> {
    let (source, made) = (from.size(), into.size());
    let whole = Rectangle::<i32, Physical>::from_size((made.w, made.h).into());
    let src = Rectangle::<i32, Buffer>::from_size(source).to_f64();
    let (width, height) = (float(source.w), float(source.h));
    let mut uniforms = vec![
        Uniform::new("apart", (offset / width, offset / height)),
        Uniform::new("edge", (0.5 / width, 0.5 / height)),
    ];
    uniforms.extend(saturation.map(|saturation| Uniform::new("saturation", saturation)));
    let mut framebuffer = gles.bind(into)?;
    let mut frame = gles.render(&mut framebuffer, whole.size, Transform::Normal)?;
    // Drawn as opaque, so that what it makes is written as it is, over
    // nothing.
    frame.render_texture_from_to(
        from,
        src,
        whole,
        &[whole],
        &[whole],
        Transform::Normal,
        1.0,
        Some(program),
        &uniforms,
    )?;
    frame.finish().map(|_| ())
}

/// A count of pixels as a number to work with.
fn float(value: i32) -> f32 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "a picture is some thousands of pixels across at most"
    )]
    let value = value as f32;
    value
}

/// What is behind glass, blurred, where it is drawn.
#[derive(Debug)]
struct Blurred {
    id: Id,
    commit: CommitCounter,
    texture: GlesTexture,
    area: Rectangle<i32, Physical>,
    opacity: f32,
}

impl Element for Blurred {
    fn id(&self) -> &Id {
        &self.id
    }

    fn current_commit(&self) -> CommitCounter {
        self.commit
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        Rectangle::from_size(self.texture.size()).to_f64()
    }

    fn geometry(&self, _scale: Scale<f64>) -> Rectangle<i32, Physical> {
        self.area
    }

    fn alpha(&self) -> f32 {
        self.opacity
    }
}

impl<R: Gles> RenderElement<R> for Blurred {
    fn draw(
        &self,
        frame: &mut R::Frame<'_, '_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
        opaque_regions: &[Rectangle<i32, Physical>],
    ) -> Result<(), R::Error> {
        let frame = R::frame(frame);
        let transform = Transform::Normal;
        frame
            .render_texture_from_to(
                &self.texture,
                src,
                dst,
                damage,
                opaque_regions,
                transform,
                self.opacity,
                None,
                &[],
            )
            .map_err(R::error)
    }
}
