//! The pictures of omorfos's brand, drawn from its rules: the mark, the
//! icon at each size and the wallpapers. `docs/BRAND.md` says what they are
//! to be, and this is that, written as SVG and drawn with resvg.
//!
//! Everything is drawn the same every time: the dots are scattered by a
//! sequence of numbers that starts in the same place, so that a picture
//! drawn again is the same to the pixel, and what was installed can be told
//! apart from what was meant.

use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, io};

use resvg::{tiny_skia, usvg};

/// The dark ground, and the inside of the mark on it.
const NAVY: &str = "#0c1226";
/// What is raised on the dark ground.
const NAVY_2: &str = "#141c38";
/// The frame of the mark on dark grounds.
const PAPER: &str = "#f3f5fb";
/// The light ground, and the inside of the mark on it.
const CREAM: &str = "#fcf7e3";
/// What is raised on the light ground.
const CREAM_2: &str = "#fffdf4";
/// The spectrum, in its order: cyan, lime, yellow, orange and pink.
const SPECTRUM: [&str; 5] = ["#22d3ee", "#a3e635", "#fde047", "#fb923c", "#f472b6"];

/// The sizes the icon is drawn at, in pixels.
pub const ICONS: [u32; 11] = [16, 22, 24, 32, 48, 64, 96, 128, 256, 512, 1024];
/// The size a wallpaper is drawn at, which a smaller screen, or one of
/// another shape, is given cut to fit.
pub const WALLPAPER: (u32, u32) = (3840, 2160);

/// Dark or light: the grounds and the frame of the mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    /// Navy, with a frame of paper.
    Dark,
    /// Cream, with a frame of navy.
    Light,
}

impl Scheme {
    /// The ground, which is also inside the mark.
    const fn ground(self) -> &'static str {
        match self {
            Self::Dark => NAVY,
            Self::Light => CREAM,
        }
    }

    /// What the ground is lifted towards, at the top left.
    const fn lift(self) -> &'static str {
        match self {
            Self::Dark => NAVY_2,
            Self::Light => CREAM_2,
        }
    }

    /// The frame of the mark, and the dots scattered over the ground.
    const fn frame(self) -> &'static str {
        match self {
            Self::Dark => PAPER,
            Self::Light => NAVY,
        }
    }

    /// What the scheme is called in the names of files.
    const fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// The mark on a square of 300, for its own grounds: what is inside the
/// frame, the five bands across it, and the frame.
fn mark_body(scheme: Scheme) -> String {
    let mut bands = String::new();
    for (band, colour) in (0..).zip(SPECTRUM) {
        let top = 68 + 32 * band;
        let _ = write!(
            bands,
            r#"<rect x="-40" y="{top}" width="380" height="30" fill="{colour}"/>"#
        );
    }
    let (inside, frame) = (scheme.ground(), scheme.frame());
    format!(
        concat!(
            r#"<defs><clipPath id="inside"><rect x="48" y="48" width="204" height="204" rx="44"/></clipPath></defs>"#,
            r#"<g clip-path="url(#inside)"><rect x="48" y="48" width="204" height="204" fill="{inside}"/>"#,
            r#"<g transform="rotate(-45 150 150)">{bands}</g></g>"#,
            r#"<rect x="48" y="48" width="204" height="204" rx="44" fill="none" stroke="{frame}" stroke-width="18"/>"#,
        ),
        inside = inside,
        bands = bands,
        frame = frame,
    )
}

/// The mark, as SVG, for dark grounds or light ones.
#[must_use]
pub fn mark(scheme: Scheme) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 300" role="img" aria-label="omorfos">{}</svg>"#,
        mark_body(scheme)
    )
}

/// The mark in one colour, the colour of what it is drawn in: the frame,
/// and one rounded bar across it for the bands.
#[must_use]
pub fn mono() -> String {
    concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 300 300" role="img" aria-label="omorfos">"#,
        r#"<g transform="rotate(-45 150 150)"><rect x="80" y="135" width="140" height="30" rx="15" fill="currentColor"/></g>"#,
        r#"<rect x="48" y="48" width="204" height="204" rx="44" fill="none" stroke="currentColor" stroke-width="18"/>"#,
        "</svg>",
    )
    .to_owned()
}

/// The name, "omorfos", as the outline of Space Grotesk at a weight of 600
/// and 190 large, 0.04 em closer together than the font has it, on a
/// baseline at 290 across and 212 down: where it is beside the mark. It is
/// kept as an outline so that the font is not needed. It was made once from
/// the font, shaped by `HarfRust` and drawn out by `skrifa`; the font and its
/// licence are named in CREDITS.md.
const NAME: &str = include_str!("name.path");

/// The mark and the name beside it, as SVG, for dark grounds or light ones.
#[must_use]
pub fn lockup(scheme: Scheme) -> String {
    format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1040 300" role="img" aria-label="omorfos">"#,
            r#"{}<path d="{}" fill="{}"/></svg>"#,
        ),
        mark_body(scheme),
        NAME.trim_end(),
        scheme.frame(),
    )
}

/// The icon, as SVG: the mark filling its square, but for 3% at each edge.
fn icon(scheme: Scheme) -> String {
    // The frame's outside is 222 across, about the middle of the mark.
    let side = 222.0 / 0.94;
    let corner = 150.0 - side / 2.0;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{corner:.3} {corner:.3} {side:.3} {side:.3}">{}</svg>"#,
        mark_body(scheme)
    )
}

/// A sequence of numbers that looks random and starts in the same place
/// every time: xorshift64*.
struct Dice(u64);

impl Dice {
    /// The next number, from 0 to 1.
    fn roll(&mut self) -> f64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        let next = self.0.wrapping_mul(0x2545_f491_4f6c_dd1d);
        // The top 53 bits, which a double holds exactly.
        #[expect(clippy::cast_precision_loss, reason = "53 bits fit a double")]
        let fraction = (next >> 11) as f64 / (1_u64 << 53) as f64;
        fraction
    }
}

/// The wallpaper, as SVG, `size` pixels across and down: the ground lifted
/// towards the top left, dots scattered over it, the spectrum spilling
/// under the mark along its angle, and the mark, right of the middle.
#[must_use]
pub fn wallpaper(scheme: Scheme, (width, height): (u32, u32)) -> String {
    let (across, down) = (f64::from(width), f64::from(height));
    // A pixel of a screen 3840 wide, which the rules are written for.
    let unit = across / 3840.0;
    // The frame's outside is 30% of the width, its middle at 71% across
    // and a little under half way down.
    let scale = 0.3 * across / 222.0;
    let (middle_x, middle_y) = (0.71 * across, 0.515 * down);
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
    );
    let raised = scheme.lift();
    let _ = write!(
        svg,
        concat!(
            "<defs>",
            r#"<radialGradient id="lift" gradientUnits="userSpaceOnUse" cx="{x:.1}" cy="{y:.1}" r="{r:.1}">"#,
            r#"<stop offset="0" stop-color="{lift}" stop-opacity="0.9"/>"#,
            r#"<stop offset="1" stop-color="{lift}" stop-opacity="0"/>"#,
            "</radialGradient>",
            r#"<filter id="spill" filterUnits="userSpaceOnUse" x="0" y="0" width="{width}" height="{height}">"#,
            r#"<feGaussianBlur stdDeviation="{spill:.1}"/></filter>"#,
            r#"<filter id="soft" filterUnits="userSpaceOnUse" x="0" y="0" width="{width}" height="{height}">"#,
            r#"<feGaussianBlur stdDeviation="{soft:.2}"/></filter>"#,
            r#"<filter id="grain" filterUnits="userSpaceOnUse" x="0" y="0" width="{width}" height="{height}">"#,
            r#"<feTurbulence type="fractalNoise" baseFrequency="0.8" seed="7"/>"#,
            r#"<feColorMatrix type="saturate" values="0"/></filter>"#,
            "</defs>",
            r#"<rect width="{width}" height="{height}" fill="{ground}"/>"#,
            r#"<rect width="{width}" height="{height}" fill="url(#lift)"/>"#,
            // A grain too fine to be seen, so that the lift, a few shades
            // over a whole screen, does not step from one to the next.
            r#"<rect width="{width}" height="{height}" filter="url(#grain)" opacity="0.05"/>"#,
        ),
        x = 0.18 * across,
        y = 0.12 * down,
        r = 0.75 * across,
        lift = raised,
        width = width,
        height = height,
        spill = 280.0 * unit,
        soft = 1.2 * unit,
        ground = scheme.ground(),
    );
    // About 460 dots on a picture of 3840 by 2160, and as many for its area
    // on others: most small, a few up to 22 across, each faint.
    let dots = 460 * u64::from(width) * u64::from(height) / (3840 * 2160);
    let mut dice = Dice(0x6f6d_6f72_666f_7321);
    let _ = write!(svg, r#"<g filter="url(#soft)" fill="{}">"#, scheme.frame());
    for _ in 0..dots {
        let (x, y) = (dice.roll() * across, dice.roll() * down);
        let radius = 10.0f64.mul_add(dice.roll().powi(3), 1.0) * unit;
        let opacity = 0.095 * 0.6f64.mul_add(dice.roll(), 0.4);
        let _ = write!(
            svg,
            r#"<circle cx="{x:.1}" cy="{y:.1}" r="{radius:.2}" fill-opacity="{opacity:.3}"/>"#
        );
    }
    svg.push_str("</g>");
    // The spill: the bands again, 1.7 times as wide and as far apart, long
    // enough to cross the picture, under the mark, blurred a long way.
    let reach = 2.0 * (across + down);
    let thick = 30.0 * 1.7 * scale;
    let _ = write!(
        svg,
        r#"<g filter="url(#spill)" opacity="0.5"><g transform="rotate(-45 {middle_x:.1} {middle_y:.1})">"#
    );
    for (band, colour) in (0_i32..).zip(SPECTRUM) {
        // Each band's middle, from the mark's, as in the mark: 83 + 32 a
        // band down a square whose middle is 150.
        let along = f64::from(32 * band - 67) * 1.7 * scale;
        let top = middle_y + along - thick / 2.0;
        let left = middle_x - reach / 2.0;
        let _ = write!(
            svg,
            r#"<rect x="{left:.1}" y="{top:.1}" width="{reach:.1}" height="{thick:.1}" fill="{colour}"/>"#
        );
    }
    svg.push_str("</g></g>");
    let (left, top) = (middle_x - 150.0 * scale, middle_y - 150.0 * scale);
    let _ = write!(
        svg,
        r#"<g transform="translate({left:.2} {top:.2}) scale({scale:.5})">{}</g></svg>"#,
        mark_body(scheme)
    );
    svg
}

/// Draws `svg` at `width` by `height` pixels, stretched to them, and
/// returns it as PNG; or nothing, if it cannot be read or has no size.
#[must_use]
pub fn draw(svg: &str, (width, height): (u32, u32)) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok();
    let canvas = tiny_skia::Pixmap::new(width, height);
    tree.zip(canvas).and_then(|(tree, mut canvas)| {
        let drawing = tree.size();
        #[expect(
            clippy::cast_precision_loss,
            reason = "a side is at most a few thousand"
        )]
        let (across, down) = (width as f32, height as f32);
        let scale =
            tiny_skia::Transform::from_scale(across / drawing.width(), down / drawing.height());
        resvg::render(&tree, scale, &mut canvas.as_mut());
        canvas.encode_png().ok()
    })
}

/// A picture of the brand, as it is installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Picture {
    /// The mark, as the icon that scales to any size.
    Mark,
    /// The mark in one colour, as the symbolic icon.
    Mono,
    /// The icon, this many pixels across.
    Icon(u32),
    /// A wallpaper, at this size.
    Wallpaper(Scheme, (u32, u32)),
}

impl Picture {
    /// Where it is installed, under the directory of what is shared
    /// (`share`).
    #[must_use]
    pub fn path(self) -> PathBuf {
        match self {
            Self::Mark => "icons/hicolor/scalable/apps/omorfos.svg".into(),
            Self::Mono => "icons/hicolor/symbolic/apps/omorfos-symbolic.svg".into(),
            Self::Icon(side) => format!("icons/hicolor/{side}x{side}/apps/omorfos.png").into(),
            Self::Wallpaper(scheme, _) => {
                format!("backgrounds/omorfos/prism-{}.png", scheme.name()).into()
            }
        }
    }

    /// What the file holds; or nothing, if it cannot be drawn.
    #[must_use]
    pub fn contents(self) -> Option<Vec<u8>> {
        match self {
            Self::Mark => Some(mark(Scheme::Dark).into_bytes()),
            Self::Mono => Some(mono().into_bytes()),
            Self::Icon(side) => draw(&icon(Scheme::Dark), (side, side)),
            Self::Wallpaper(scheme, size) => draw(&wallpaper(scheme, size), size),
        }
    }
}

/// Every picture that is installed.
#[must_use]
pub fn everything() -> Vec<Picture> {
    let icons = ICONS.map(Picture::Icon);
    let wallpapers =
        [Scheme::Dark, Scheme::Light].map(|scheme| Picture::Wallpaper(scheme, WALLPAPER));
    [Picture::Mark, Picture::Mono]
        .into_iter()
        .chain(icons)
        .chain(wallpapers)
        .collect()
}

/// Draws `pictures`, and writes each where it is installed under `out`.
///
/// # Errors
///
/// Returns an error if a picture cannot be drawn or written.
pub fn write(out: &Path, pictures: &[Picture]) -> io::Result<()> {
    pictures.iter().try_for_each(|picture| {
        let path = out.join(picture.path());
        let contents = picture
            .contents()
            .ok_or_else(|| io::Error::other(format!("{} could not be drawn", path.display())))?;
        let made = path.parent().map_or(Ok(()), fs::create_dir_all);
        made.and_then(|()| fs::write(&path, contents))
    })
}

/// How the program ends, once it has `written` what it was asked to, or
/// failed to, or was asked nothing; saying on `stderr` what went wrong.
pub fn ended(written: Option<io::Result<()>>, stderr: &mut impl Write) -> ExitCode {
    match written {
        Some(Ok(())) => ExitCode::SUCCESS,
        Some(Err(failure)) => {
            let _ = writeln!(stderr, "omorfos-brand: {failure}");
            ExitCode::FAILURE
        }
        None => {
            let _ = writeln!(stderr, "usage: omorfos-brand OUT");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The colour of a pixel of a picture drawn as PNG, and its opacity.
    fn pixel(png: &[u8], (x, y): (u32, u32)) -> [u8; 4] {
        let picture = tiny_skia::Pixmap::decode_png(png).unwrap();
        let colour = picture.pixel(x, y).unwrap().demultiply();
        [colour.red(), colour.green(), colour.blue(), colour.alpha()]
    }

    /// Whether `colour` is the one written as `hex`, give or take what
    /// drawing smooths.
    fn near(colour: [u8; 4], hex: &str) -> bool {
        let value = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap();
        let wanted = [value(1), value(3), value(5)];
        colour[..3]
            .iter()
            .zip(wanted)
            .all(|(have, want)| have.abs_diff(want) <= 12)
    }

    #[test]
    fn the_mark_is_its_frame_round_the_spectrum_in_its_order() {
        for scheme in [Scheme::Dark, Scheme::Light] {
            let png = draw(&mark(scheme), (300, 300)).unwrap();
            assert!(near(pixel(&png, (150, 44)), scheme.frame()), "the frame");
            // Across the middle, along the angle the bands are not at:
            // from the top left corner of the inside to the bottom right.
            assert!(
                near(pixel(&png, (70, 70)), scheme.ground()),
                "a corner left clear"
            );
            assert!(
                near(pixel(&png, (150, 150)), SPECTRUM[2]),
                "yellow in the middle"
            );
            assert!(near(pixel(&png, (120, 120)), SPECTRUM[1]), "lime before it");
            assert!(
                near(pixel(&png, (180, 180)), SPECTRUM[3]),
                "orange after it"
            );
            assert!(
                near(pixel(&png, (230, 230)), scheme.ground()),
                "the other corner"
            );
        }
    }

    #[test]
    fn the_mark_in_one_colour_is_drawn_in_the_colour_it_is_given() {
        let red = mono().replace("currentColor", "#ff0000");
        let png = draw(&red, (300, 300)).unwrap();
        assert!(near(pixel(&png, (150, 44)), "#ff0000"), "the frame");
        assert!(near(pixel(&png, (150, 150)), "#ff0000"), "the bar");
        assert_eq!(pixel(&png, (110, 110))[3], 0, "nothing else");
    }

    #[test]
    fn the_name_is_beside_the_mark_in_the_colour_of_its_frame() {
        for scheme in [Scheme::Dark, Scheme::Light] {
            let png = draw(&lockup(scheme), (1040, 300)).unwrap();
            assert!(near(pixel(&png, (150, 150)), SPECTRUM[2]), "the mark");
            // Across the letters, a little above their baseline.
            let picture = tiny_skia::Pixmap::decode_png(&png).unwrap();
            let ink = (290..1010).filter(|&x| {
                let colour = picture.pixel(x, 180).unwrap().demultiply();
                let colour = [colour.red(), colour.green(), colour.blue(), colour.alpha()];
                colour[3] == 255 && near(colour, scheme.frame())
            });
            assert!(ink.count() > 100, "the letters");
            assert_eq!(pixel(&png, (1030, 150))[3], 0, "and nothing after them");
        }
    }

    #[test]
    fn the_icon_fills_its_square_but_for_a_little_at_the_edges() {
        let png = draw(&icon(Scheme::Dark), (100, 100)).unwrap();
        assert_eq!(pixel(&png, (1, 50))[3], 0, "clear at the edge");
        assert!(
            near(pixel(&png, (5, 50)), PAPER),
            "the frame just inside it"
        );
        assert!(near(pixel(&png, (50, 50)), SPECTRUM[2]));
    }

    #[test]
    fn a_wallpaper_has_its_ground_and_the_mark_right_of_the_middle() {
        let size = (384, 216);
        for scheme in [Scheme::Dark, Scheme::Light] {
            let png = draw(&wallpaper(scheme, size), size).unwrap();
            // The bottom left corner is far from the lift and the spill.
            let corner = pixel(&png, (2, 213));
            let ground = [Scheme::Dark, Scheme::Light].map(|other| near(corner, other.ground()));
            assert_eq!(ground, [scheme == Scheme::Dark, scheme == Scheme::Light]);
            // The mark's middle is 71% across and a little below half way.
            assert!(near(pixel(&png, (273, 111)), SPECTRUM[2]), "the mark");
            assert!(
                !near(pixel(&png, (110, 111)), SPECTRUM[2]),
                "and not left of it"
            );
        }
    }

    #[test]
    fn a_picture_is_drawn_the_same_every_time() {
        let size = (192, 108);
        let once = draw(&wallpaper(Scheme::Dark, size), size);
        assert_eq!(once, draw(&wallpaper(Scheme::Dark, size), size));
        assert_ne!(once, draw(&wallpaper(Scheme::Light, size), size));
    }

    #[test]
    fn what_cannot_be_read_or_has_no_size_is_not_drawn() {
        assert_eq!(draw("<svg", (10, 10)), None);
        assert_eq!(draw(&mark(Scheme::Dark), (0, 10)), None);
    }

    #[test]
    fn everything_is_installed_where_the_icon_theme_and_the_wallpaper_look() {
        let paths: Vec<PathBuf> = everything().into_iter().map(Picture::path).collect();
        assert_eq!(paths.len(), 2 + ICONS.len() + 2);
        for path in [
            "icons/hicolor/scalable/apps/omorfos.svg",
            "icons/hicolor/symbolic/apps/omorfos-symbolic.svg",
            "icons/hicolor/16x16/apps/omorfos.png",
            "icons/hicolor/1024x1024/apps/omorfos.png",
            "backgrounds/omorfos/prism-dark.png",
            "backgrounds/omorfos/prism-light.png",
        ] {
            assert!(paths.contains(&PathBuf::from(path)), "{path}");
        }
        assert!(everything().contains(&Picture::Wallpaper(Scheme::Light, WALLPAPER)));
    }

    #[test]
    fn what_is_drawn_is_written_where_it_is_installed() {
        let out = tempfile::TempDir::new().unwrap();
        let pictures = [
            Picture::Mark,
            Picture::Mono,
            Picture::Icon(16),
            Picture::Wallpaper(Scheme::Dark, (96, 54)),
        ];
        write(out.path(), &pictures).unwrap();
        let read = |picture: Picture| fs::read(out.path().join(picture.path())).unwrap();
        assert_eq!(read(Picture::Mark), mark(Scheme::Dark).into_bytes());
        assert_eq!(read(Picture::Mono), mono().into_bytes());
        assert_eq!(pixel(&read(Picture::Icon(16)), (8, 8))[3], 255);
        let wallpaper = tiny_skia::Pixmap::decode_png(&read(pictures[3])).unwrap();
        assert_eq!((wallpaper.width(), wallpaper.height()), (96, 54));
    }

    #[test]
    fn what_cannot_be_drawn_or_written_is_said() {
        let out = tempfile::TempDir::new().unwrap();
        let failed = write(out.path(), &[Picture::Icon(0)]).unwrap_err();
        assert!(
            failed
                .to_string()
                .contains("0x0/apps/omorfos.png could not be drawn")
        );
        // A file where a directory is to be.
        let file = out.path().join("file");
        fs::write(&file, "").unwrap();
        assert!(write(&file, &[Picture::Mark]).is_err());
    }

    #[test]
    fn the_program_ends_as_what_it_was_asked_to_do_went() {
        let mut said = Vec::new();
        assert_eq!(ended(Some(Ok(())), &mut said), ExitCode::SUCCESS);
        assert_eq!(said, b"");
        let failure = io::Error::other("the disk is full");
        assert_eq!(ended(Some(Err(failure)), &mut said), ExitCode::FAILURE);
        assert_eq!(said, b"omorfos-brand: the disk is full\n");
        said.clear();
        assert_eq!(ended(None, &mut said), ExitCode::from(2));
        assert_eq!(said, b"usage: omorfos-brand OUT\n");
    }
}
