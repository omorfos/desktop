//! The icons of programs: finding the picture a desktop entry names and
//! reading it.
//!
//! Icons are other programs' files, in two formats: PNG, in one directory
//! for each size it comes in, and SVG, which is drawn at whatever size is
//! wanted. Only so much of a file is read, a picture is only so large, and
//! one that cannot be made sense of is no icon.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use omorfos_config::Color;
use omorfos_ui::Image;
use resvg::{tiny_skia, usvg};

/// The most of an icon's file that is read, in bytes.
const LARGEST_FILE: u64 = 2 * 1024 * 1024;

/// The most pixels an icon may have along a side.
const LARGEST_SIDE: u32 = 1024;

/// The themes icons are looked for in: the one every program's own icon is
/// installed in, and then the one that has the icons programs name without
/// bringing their own, such as `dialog-information`.
const THEMES: [&str; 2] = ["hicolor", "Adwaita"];

/// The kinds of icon a theme sorts its pictures into, each a directory.
const KINDS: [&str; 8] = [
    "apps",
    "status",
    "devices",
    "actions",
    "categories",
    "places",
    "mimetypes",
    "legacy",
];

/// What the name of an icon that is only a shape ends with. Such an icon
/// is meant to be drawn in the colour of the text it stands beside.
const SHAPE: &str = "-symbolic";

/// The icons there are, and those that have been looked up so far.
#[derive(Debug, Default)]
pub struct Icons {
    /// Where icons are installed, the user's own first.
    directories: Vec<PathBuf>,
    /// What each name came to at each size it was wanted at.
    found: HashMap<(String, u32, Color), Option<Image>>,
}

impl Icons {
    /// The icons installed in `directories`, the first of which counts
    /// where several have one of the same name. A directory holds themes
    /// (`hicolor/48x48/apps/name.png`), or pictures directly (`name.png`).
    #[must_use]
    pub fn new(directories: Vec<PathBuf>) -> Self {
        Self {
            directories,
            found: HashMap::new(),
        }
    }

    /// Where icons are installed: the `icons` directory of `data_home`
    /// (`XDG_DATA_HOME`, or `.local/share` in `home`) and of each of
    /// `data_dirs` (`XDG_DATA_DIRS`, or `/usr/local/share` and
    /// `/usr/share`), and then `/usr/share/pixmaps`, where programs
    /// without a theme put theirs.
    #[must_use]
    pub fn directories(
        data_home: Option<&OsStr>,
        home: Option<&OsStr>,
        data_dirs: Option<&OsStr>,
    ) -> Vec<PathBuf> {
        let data = data_directories(data_home, home, data_dirs).into_iter();
        let themed = data.map(|directory| directory.join("icons"));
        themed
            .chain([PathBuf::from("/usr/share/pixmaps")])
            .collect()
    }

    /// The icon called `name`, to be drawn `pixels` pixels across beside
    /// text of the colour `ink`. `name` is as a program gives it: a name,
    /// or the whole path of a picture. Nothing if there is no such icon or
    /// it cannot be read.
    ///
    /// Where a theme has an icon only as a shape, the shape is what is
    /// found, drawn in `ink`.
    pub fn get(&mut self, name: &str, pixels: u32, ink: Color) -> Option<&Image> {
        let known = self.found.entry((name.to_owned(), pixels, ink));
        let directories = &self.directories;
        known
            .or_insert_with(|| {
                let file = locate(directories, name, pixels);
                file.and_then(|file| {
                    let stem = file.file_stem().unwrap_or_default().to_string_lossy();
                    let image = load(&file, pixels);
                    if stem.ends_with(SHAPE) {
                        image.map(|shape| shape.tinted(ink))
                    } else {
                        image
                    }
                })
            })
            .as_ref()
    }
}

/// A variable of the environment that was given a value.
fn set(value: Option<&OsStr>) -> Option<&OsStr> {
    value.filter(|value| !value.is_empty())
}

/// Where programs install what they share, the user's own first:
/// `data_home` (`XDG_DATA_HOME`, or `.local/share` in `home`) and then each
/// of `data_dirs` (`XDG_DATA_DIRS`, or `/usr/local/share` and
/// `/usr/share`). Icons are in the `icons` directory of each, and desktop
/// entries in `applications`.
#[must_use]
pub fn data_directories(
    data_home: Option<&OsStr>,
    home: Option<&OsStr>,
    data_dirs: Option<&OsStr>,
) -> Vec<PathBuf> {
    let own = set(data_home).map(PathBuf::from);
    let own = own.or_else(|| set(home).map(|home| Path::new(home).join(".local/share")));
    let shared = set(data_dirs).unwrap_or(OsStr::new("/usr/local/share:/usr/share"));
    let shared = std::env::split_paths(shared).filter(|path| path.is_absolute());
    own.into_iter().chain(shared).collect()
}

/// Reads a PNG from memory, as an icon would be read from a file. Nothing
/// if it is not one or is larger than an icon may be.
#[must_use]
pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    png(bytes)
}

/// Finds the file of the icon called `name` that suits `pixels` best: the
/// icon itself if any directory has it, and failing that its shape.
fn locate(directories: &[PathBuf], name: &str, pixels: u32) -> Option<PathBuf> {
    let given = Path::new(name);
    if given.is_absolute() {
        return given.is_file().then(|| given.to_owned());
    }
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let wanted = [name.to_owned(), format!("{name}{SHAPE}")];
    wanted.iter().find_map(|name| {
        let mut places = directories.iter();
        places.find_map(|directory| within(directory, name, pixels))
    })
}

/// Finds the file of the icon called `name` in one directory of icons.
fn within(directory: &Path, name: &str, pixels: u32) -> Option<PathBuf> {
    let files = ["png", "svg"].map(|extension| format!("{name}.{extension}"));
    let themes = THEMES
        .iter()
        .flat_map(|theme| sizes(&directory.join(theme)));
    let sorted = themes.flat_map(|(size, folder)| {
        let kinds = KINDS.iter().map(move |kind| folder.join(kind));
        // A drawing does for any size, however its directory is named.
        let sized = [size, u32::MAX];
        let files = files.iter().zip(sized);
        kinds.flat_map(move |kind| {
            files
                .clone()
                .map(move |(file, size)| (size, kind.join(file)))
        })
    });
    // Pictures with no theme lie in the directory as they are.
    let loose = files.iter().map(|file| (0, directory.join(file)));
    let mut candidates: Vec<(u32, PathBuf)> = sorted
        .chain(loose)
        .filter(|(_, file)| file.is_file())
        .collect();
    // The smallest that is large enough, and failing that the largest. One
    // that draws at any size counts as the largest of all.
    candidates.sort_by_key(|(size, _)| (*size < pixels, size.abs_diff(pixels)));
    candidates.into_iter().next().map(|(_, file)| file)
}

/// The directories of a theme that hold pictures of one size, with that
/// size in pixels. `48x48@2` holds pictures of 96. A directory named
/// otherwise, such as `scalable`, holds pictures of no particular size.
fn sizes(theme: &Path) -> Vec<(u32, PathBuf)> {
    let folders = fs::read_dir(theme).into_iter().flatten().flatten();
    let sized = folders.map(|folder| {
        let name = folder.file_name();
        let name = name.to_string_lossy();
        let (across, times) = name.split_once('@').unwrap_or((&name, "1"));
        let side = across
            .split_once('x')
            .and_then(|(side, _)| side.parse::<u32>().ok());
        let side = side.zip(times.parse::<u32>().ok());
        let pixels = side.map_or(u32::MAX, |(side, times)| side.saturating_mul(times));
        (pixels, folder.path())
    });
    sized.collect()
}

/// Reads a picture, to be drawn `pixels` pixels across.
fn load(file: &Path, pixels: u32) -> Option<Image> {
    let mut bytes = Vec::new();
    let opened = File::open(file).ok();
    let read = opened.map(|opened| opened.take(LARGEST_FILE).read_to_end(&mut bytes));
    let bytes = read.and_then(Result::ok).map(|_| bytes);
    let drawn = file.extension() == Some(OsStr::new("svg"));
    bytes.and_then(|bytes| {
        if drawn {
            svg(&bytes, pixels)
        } else {
            png(&bytes)
        }
    })
}

/// Reads a PNG.
fn png(bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    // Every picture comes out as eight bits a sample with an opacity,
    // whatever it was stored as: grey with opacity, or colour with it.
    let plain = png::Transformations::EXPAND | png::Transformations::STRIP_16;
    decoder.set_transformations(plain | png::Transformations::ALPHA);
    let mut reader = decoder.read_info().ok();
    let fits = |reader: &png::Reader<std::io::Cursor<&[u8]>>| {
        let info = reader.info();
        info.width <= LARGEST_SIDE && info.height <= LARGEST_SIDE
    };
    let reader = reader.as_mut().filter(|reader| fits(reader));
    reader.and_then(|reader| {
        let mut samples = vec![0; reader.output_buffer_size().unwrap_or(0)];
        let frame = reader.next_frame(&mut samples).ok();
        frame.and_then(|frame| {
            let each = frame.color_type.samples();
            let pixels = samples
                .chunks_exact(each)
                .take(frame.buffer_size() / each.max(1));
            // Grey with opacity is two samples and colour with it four. In
            // both the first is red and the last the opacity; green and
            // blue are the two between, or the grey again.
            let rgba = pixels.flat_map(|pixel| {
                let sample = |place: usize| pixel.get(place).copied().unwrap_or(0);
                [
                    sample(0),
                    sample(each / 4),
                    sample(each / 4 * 2),
                    sample(each - 1),
                ]
            });
            Image::new(frame.width, frame.height, rgba.collect())
        })
    })
}

/// Draws an SVG, `pixels` pixels across.
fn svg(bytes: &[u8], pixels: u32) -> Option<Image> {
    let side = pixels.clamp(1, LARGEST_SIDE);
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).ok();
    let canvas = tiny_skia::Pixmap::new(side, side);
    tree.zip(canvas).and_then(|(tree, mut canvas)| {
        let drawing = tree.size();
        // As large as fits the square, in the middle of it.
        #[expect(clippy::cast_precision_loss, reason = "a side is at most 1024")]
        let room = side as f32;
        let scale = (room / drawing.width()).min(room / drawing.height());
        let (across, down) = (drawing.width() * scale, drawing.height() * scale);
        let middle =
            tiny_skia::Transform::from_translate((room - across) / 2.0, (room - down) / 2.0);
        resvg::render(&tree, middle.pre_scale(scale, scale), &mut canvas.as_mut());
        // It comes out with its colours multiplied by its opacity.
        let pixels = canvas.pixels().iter().flat_map(|pixel| {
            let plain = pixel.demultiply();
            [plain.red(), plain.green(), plain.blue(), plain.alpha()]
        });
        Image::new(side, side, pixels.collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The colour of the text an icon stands beside.
    const INK: Color = Color::rgb(0, 0, 255);

    /// A PNG of one colour, `side` pixels square.
    fn png_of(side: u32, rgba: [u8; 4]) -> Vec<u8> {
        encode(side, png::ColorType::Rgba, &rgba)
    }

    fn encode(side: u32, color: png::ColorType, pixel: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, side, side);
        encoder.set_color(color);
        let mut writer = encoder.write_header().unwrap();
        let count = usize::try_from(side * side).unwrap();
        writer.write_image_data(&pixel.repeat(count)).unwrap();
        writer.finish().unwrap();
        bytes
    }

    /// An SVG of a red square filling a picture twice as wide as tall.
    const WIDE_SVG: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='20' height='10'>\
                            <rect width='20' height='10' fill='#ff0000'/></svg>";

    fn write(root: &Path, file: &str, bytes: &[u8]) {
        let path = root.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn pixel(image: &Image, x: u32, y: u32) -> u32 {
        let mut canvas = omorfos_ui::Canvas::new(image.size().0, image.size().1);
        canvas.draw_image((0, 0, image.size().0, image.size().1), image);
        canvas.at(x, y)
    }

    #[test]
    fn an_icon_is_found_by_name_in_the_size_that_suits() {
        let root = tempfile::TempDir::new().unwrap();
        for side in [16, 32, 64] {
            let file = format!("hicolor/{side}x{side}/apps/tool.png");
            write(root.path(), &file, &png_of(side, [255, 0, 0, 255]));
        }
        write(
            root.path(),
            // Twice 24 is 48.
            "hicolor/24x24@2/apps/tool.png",
            &png_of(48, [0; 4]),
        );
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        let side = |icons: &mut Icons, pixels| icons.get("tool", pixels, INK).unwrap().size().0;
        assert_eq!(side(&mut icons, 16), 16);
        assert_eq!(
            side(&mut icons, 20),
            32,
            "the smallest that is large enough"
        );
        assert_eq!(side(&mut icons, 48), 48);
        assert_eq!(side(&mut icons, 50), 64);
        assert_eq!(side(&mut icons, 200), 64, "failing that, the largest");
        assert_eq!(icons.get("other", 16, INK), None);
        assert_eq!(icons.get("", 16, INK), None);
        assert_eq!(icons.get("../tool", 16, INK), None, "a name is not a path");
    }

    #[test]
    fn an_icon_that_draws_at_any_size_is_drawn_at_the_size_wanted() {
        let root = tempfile::TempDir::new().unwrap();
        write(
            root.path(),
            "hicolor/scalable/apps/tool.svg",
            WIDE_SVG.as_bytes(),
        );
        write(
            root.path(),
            "hicolor/16x16/apps/tool.png",
            &png_of(16, [0, 0, 255, 255]),
        );
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        // The small picture does for a small icon.
        assert_eq!(icons.get("tool", 16, INK).unwrap().size(), (16, 16));
        let large = icons.get("tool", 40, INK).unwrap().clone();
        assert_eq!(large.size(), (40, 40));
        // Its shape is kept, in the middle: red across, clear above.
        assert_eq!(pixel(&large, 20, 20), 0xffff_0000);
        assert_eq!(pixel(&large, 2, 20), 0xffff_0000);
        assert_eq!(pixel(&large, 20, 4), 0);
        assert_eq!(pixel(&large, 20, 35), 0);
    }

    #[test]
    fn an_icon_a_program_does_not_bring_is_found_in_the_desktops_theme() {
        let root = tempfile::TempDir::new().unwrap();
        // As Adwaita has them: by kind, and many only as a shape.
        write(
            root.path(),
            "Adwaita/scalable/devices/printer.svg",
            WIDE_SVG.as_bytes(),
        );
        write(
            root.path(),
            "Adwaita/symbolic/status/dialog-information-symbolic.svg",
            WIDE_SVG.as_bytes(),
        );
        write(
            root.path(),
            "hicolor/32x32/status/battery.png",
            &png_of(32, [0; 4]),
        );
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        assert_eq!(icons.get("battery", 32, INK).unwrap().size().0, 32);
        // A drawing in colour is left in its colours.
        let printer = icons.get("printer", 40, INK).unwrap().clone();
        assert_eq!(pixel(&printer, 20, 20), 0xffff_0000);
        // A shape is drawn in the colour of the text beside it, whether it
        // is asked for by the icon's name or by the shape's own.
        for name in ["dialog-information", "dialog-information-symbolic"] {
            let shape = icons.get(name, 40, INK).unwrap().clone();
            assert_eq!(pixel(&shape, 20, 20), 0xff00_00ff, "{name}");
            assert_eq!(pixel(&shape, 20, 4), 0, "{name}");
        }
        // Asked for beside text of another colour, it is in that colour.
        let red = icons.get("dialog-information", 40, Color::rgb(255, 0, 0));
        assert_eq!(pixel(&red.unwrap().clone(), 20, 20), 0xffff_0000);
    }

    #[test]
    fn the_first_directory_with_an_icon_is_the_one_it_comes_from() {
        let root = tempfile::TempDir::new().unwrap();
        let (own, system, loose) = ("own", "system", "pixmaps");
        write(
            root.path(),
            "own/hicolor/32x32/apps/mine.png",
            &png_of(32, [1, 1, 1, 255]),
        );
        write(
            root.path(),
            "system/hicolor/32x32/apps/mine.png",
            &png_of(31, [2, 2, 2, 255]),
        );
        write(
            root.path(),
            "system/hicolor/32x32/apps/theirs.png",
            &png_of(30, [0; 4]),
        );
        // Pictures with no theme lie in a directory as they are.
        write(root.path(), "pixmaps/loose.png", &png_of(29, [0; 4]));
        write(root.path(), "pixmaps/drawn.svg", WIDE_SVG.as_bytes());
        let directories = [own, system, loose, "missing"].map(|name| root.path().join(name));
        let mut icons = Icons::new(directories.into());
        assert_eq!(icons.get("mine", 32, INK).unwrap().size().0, 32);
        assert_eq!(icons.get("theirs", 32, INK).unwrap().size().0, 30);
        assert_eq!(icons.get("loose", 32, INK).unwrap().size().0, 29);
        assert_eq!(icons.get("drawn", 32, INK).unwrap().size().0, 32);
    }

    #[test]
    fn an_entry_may_give_the_whole_path_of_its_icon() {
        let root = tempfile::TempDir::new().unwrap();
        write(
            root.path(),
            "somewhere/picture.png",
            &png_of(8, [0, 255, 0, 255]),
        );
        let mut icons = Icons::default();
        let path = root.path().join("somewhere/picture.png");
        let image = icons.get(path.to_str().unwrap(), 24, INK).unwrap().clone();
        assert_eq!(image.size(), (8, 8));
        assert_eq!(pixel(&image, 4, 4), 0xff00_ff00);
        let absent = root.path().join("somewhere/absent.png");
        assert_eq!(icons.get(absent.to_str().unwrap(), 24, INK), None);
    }

    #[test]
    fn grey_pictures_and_those_without_opacity_are_read_too() {
        let grey = encode(4, png::ColorType::Grayscale, &[200]);
        let image = png(&grey).unwrap();
        assert_eq!(pixel(&image, 1, 1), 0xffc8_c8c8);
        let faint = encode(4, png::ColorType::GrayscaleAlpha, &[255, 128]);
        assert_eq!(pixel(&png(&faint).unwrap(), 1, 1), 0x8080_8080);
        let solid = encode(4, png::ColorType::Rgb, &[10, 20, 30]);
        assert_eq!(pixel(&png(&solid).unwrap(), 1, 1), 0xff0a_141e);
    }

    #[test]
    fn a_file_that_is_not_a_picture_is_no_icon() {
        let root = tempfile::TempDir::new().unwrap();
        let whole = png_of(16, [255, 0, 0, 255]);
        write(root.path(), "broken.png", &whole[..whole.len() / 2]);
        write(root.path(), "text.png", b"not a picture");
        write(root.path(), "empty.svg", b"");
        write(root.path(), "words.svg", b"<p>not a drawing</p>");
        write(root.path(), "huge.png", &png_of(LARGEST_SIDE + 1, [0; 4]));
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        for name in ["broken", "text", "empty", "words", "huge"] {
            assert_eq!(icons.get(name, 32, INK), None, "{name}");
        }
        // And it is not looked for a second time.
        fs::write(root.path().join("text.png"), whole).unwrap();
        assert_eq!(icons.get("text", 32, INK), None);
    }

    #[test]
    fn what_programs_share_is_looked_for_in_the_users_directory_first() {
        let os = |text: &'static str| Some(OsStr::new(text));
        let shared = os("/opt/share:/usr/share");
        assert_eq!(
            data_directories(os("/home/me/data"), os("/home/me"), shared),
            ["/home/me/data", "/opt/share", "/usr/share"].map(PathBuf::from)
        );
        assert_eq!(
            data_directories(None, os("/home/me"), None),
            ["/home/me/.local/share", "/usr/local/share", "/usr/share"].map(PathBuf::from)
        );
        // Without a home there are still the system's, and a directory
        // that is not a whole path is not one to look in.
        assert_eq!(
            data_directories(os(""), None, os("relative:/usr/share:")),
            [PathBuf::from("/usr/share")]
        );
    }

    #[test]
    fn a_picture_can_be_read_from_memory() {
        let whole = png_of(4, [1, 2, 3, 255]);
        assert_eq!(decode_png(&whole).unwrap().size(), (4, 4));
        assert_eq!(decode_png(b"not a picture"), None);
    }

    #[test]
    fn icons_are_looked_for_beside_the_desktop_entries_and_among_the_loose_ones() {
        let os = |text: &'static str| Some(OsStr::new(text));
        assert_eq!(
            Icons::directories(os("/home/me/data"), None, os("/usr/share")),
            [
                PathBuf::from("/home/me/data/icons"),
                PathBuf::from("/usr/share/icons"),
                PathBuf::from("/usr/share/pixmaps"),
            ]
        );
    }
}
