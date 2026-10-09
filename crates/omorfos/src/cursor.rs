//! Pictures of the pointer, read from the cursor themes installed on the
//! system.
//!
//! A theme is a directory of files in the Xcursor format, one per shape,
//! each holding the shape at several sizes. Themes can name others to fall
//! back on. Theme files are whatever the user installed, so they are read
//! with the same suspicion as configuration: sizes are capped and a file
//! that does not parse is simply not used.

use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use smithay::input::pointer::CursorIcon;

/// The largest cursor file read, in bytes. Real ones with many sizes and
/// animation frames are a few megabytes at most.
const MAX_FILE: u64 = 16 * 1024 * 1024;

/// The largest theme description read, in bytes.
const MAX_INDEX: u64 = 64 * 1024;

/// How many themes are tried, following one theme's fallbacks to the next,
/// before giving up. Guards against themes that name each other.
const MAX_THEMES: usize = 8;

/// The theme every other falls back on in the end.
const FALLBACK: &str = "default";

/// One shape of the pointer at one size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Image {
    /// The pixels, row by row from the top: ARGB with premultiplied alpha,
    /// in the byte order renderers call `Argb8888`.
    pub(crate) pixels: Vec<u8>,
    /// The size in pixels.
    pub(crate) size: (u32, u32),
    /// The pixel that is the pointer's position.
    pub(crate) hotspot: (u32, u32),
}

/// The places cursor themes are looked for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Themes {
    directories: Vec<PathBuf>,
}

impl Themes {
    /// Looks for themes in `directories`, earlier ones first.
    #[must_use]
    pub const fn new(directories: Vec<PathBuf>) -> Self {
        Self { directories }
    }

    /// Looks where cursor themes conventionally are: the directories listed
    /// in `xcursor_path` (the `XCURSOR_PATH` variable, separated by colons)
    /// or, without it, the user's own under `home` and then the system's.
    #[must_use]
    pub fn conventional(xcursor_path: Option<&OsStr>, home: Option<&OsStr>) -> Self {
        let listed = xcursor_path.filter(|path| !path.is_empty());
        let directories = listed.map_or_else(
            || {
                let own = home
                    .map(Path::new)
                    .into_iter()
                    .flat_map(|home| [home.join(".local/share/icons"), home.join(".icons")]);
                let shared = ["/usr/local/share/icons", "/usr/share/icons"];
                own.chain(shared.map(PathBuf::from)).collect()
            },
            |path| std::env::split_paths(path).collect(),
        );
        Self { directories }
    }

    /// Finds the picture for `icon` in `theme`, or failing that in the
    /// themes it falls back on, at the size nearest to `pixels`.
    pub(crate) fn load(&self, theme: &str, icon: CursorIcon, pixels: u32) -> Option<Image> {
        let mut themes: Vec<String> = Some(theme.to_owned())
            .filter(|theme| is_name(theme))
            .into_iter()
            .collect();
        themes.extend(themes.is_empty().then(|| FALLBACK.to_owned()));
        let mut next = 0;
        while let Some(theme) = themes.get(next).cloned() {
            next += 1;
            if let Some(image) = self.load_from(&theme, icon, pixels) {
                return Some(image);
            }
            // Fallbacks are tried after everything already queued, and the
            // common one after those.
            let more = self
                .inherited(&theme)
                .into_iter()
                .chain((next == themes.len()).then(|| FALLBACK.to_owned()));
            for candidate in more {
                if themes.len() < MAX_THEMES && !themes.contains(&candidate) {
                    themes.push(candidate);
                }
            }
        }
        None
    }

    /// Finds the picture for `icon` in one theme, under any of the names
    /// the shape goes by.
    fn load_from(&self, theme: &str, icon: CursorIcon, pixels: u32) -> Option<Image> {
        let names = std::iter::once(icon.name()).chain(icon.alt_names().iter().copied());
        let mut files = names.flat_map(|name| {
            self.directories
                .iter()
                .map(move |directory| directory.join(theme).join("cursors").join(name))
        });
        files.find_map(|file| {
            let images = xcursor::parser::parse_xcursor(&read(&file, MAX_FILE)?)?;
            let nearest = images
                .into_iter()
                .min_by_key(|image| (image.size.abs_diff(pixels), image.delay));
            nearest.map(|image| Image {
                pixels: image.pixels_rgba,
                size: (image.width, image.height),
                hotspot: (image.xhot, image.yhot),
            })
        })
    }

    /// The themes `theme` says to fall back on, from its `index.theme`.
    fn inherited(&self, theme: &str) -> Vec<String> {
        let indexes = self
            .directories
            .iter()
            .map(|directory| directory.join(theme).join("index.theme"));
        let index = indexes
            .filter_map(|file| read(&file, MAX_INDEX))
            .find_map(|index| String::from_utf8(index).ok())
            .unwrap_or_default();
        let listed = index
            .lines()
            .filter_map(|line| line.split_once('='))
            .find(|(key, _)| key.trim() == "Inherits")
            .map(|(_, themes)| themes)
            .unwrap_or_default();
        let names = listed.split([',', ';']).map(str::trim);
        names
            .filter(|name| is_name(name))
            .map(str::to_owned)
            .collect()
    }
}

/// Whether `name` can be a theme's name. A theme is a directory among the
/// themes, so anything that would lead out of them is not one.
fn is_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && name != ".." && name != "."
}

/// Reads a file of at most `limit` bytes. One that is missing, unreadable
/// or larger is treated as not there.
fn read(file: &Path, limit: u64) -> Option<Vec<u8>> {
    let mut contents = Vec::new();
    let file = File::open(file).ok()?;
    // One byte past the limit, so that a larger file is noticed without
    // reading all of it.
    let length = file.take(limit + 1).read_to_end(&mut contents).ok()?;
    (u64::try_from(length).unwrap_or(u64::MAX) <= limit).then_some(contents)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    /// The bytes of a cursor file holding one square image per `(size,
    /// colour)`, whose hotspot is one pixel in from its corner.
    fn file(images: &[(u32, u32)]) -> Vec<u8> {
        let word = |value: u32| value.to_le_bytes();
        let count = u32::try_from(images.len()).unwrap();
        let mut bytes = b"Xcur".to_vec();
        bytes.extend([word(16), word(0x1_0000), word(count)].concat());
        let mut position = 16 + 12 * count;
        for (size, _) in images {
            bytes.extend([word(0xfffd_0002), word(*size), word(position)].concat());
            position += 36 + size * size * 4;
        }
        for (size, color) in images {
            let header = [36, 0xfffd_0002, *size, 1, *size, *size, 1, 1, 0];
            bytes.extend(header.into_iter().flat_map(word));
            bytes.extend(word(*color).repeat(usize::try_from(size * size).unwrap()));
        }
        bytes
    }

    /// Writes cursor files into a theme, creating it.
    fn install(directory: &Path, theme: &str, cursors: &[(&str, &[(u32, u32)])]) {
        let folder = directory.join(theme).join("cursors");
        fs::create_dir_all(&folder).unwrap();
        for (name, images) in cursors {
            fs::write(folder.join(name), file(images)).unwrap();
        }
    }

    fn themes(directory: &TempDir) -> Themes {
        Themes::new(vec![directory.path().to_owned()])
    }

    const RED: u32 = 0xffff_0000;
    const BLUE: u32 = 0xff00_00ff;

    #[test]
    fn a_shape_is_read_from_the_theme_with_its_size_and_hotspot() {
        let directory = TempDir::new().unwrap();
        install(directory.path(), "plain", &[("text", &[(4, RED)])]);
        let image = themes(&directory)
            .load("plain", CursorIcon::Text, 24)
            .unwrap();
        assert_eq!(image.size, (4, 4));
        assert_eq!(image.hotspot, (1, 1));
        assert_eq!(image.pixels.len(), 64);
        assert_eq!(image.pixels[..4], [0x00, 0x00, 0xff, 0xff], "red, as BGRA");
    }

    #[test]
    fn the_size_nearest_to_the_one_wanted_is_chosen() {
        let directory = TempDir::new().unwrap();
        let sizes: &[(u32, u32)] = &[(24, RED), (48, BLUE), (32, RED)];
        install(directory.path(), "plain", &[("default", sizes)]);
        let size = |wanted| {
            let image = themes(&directory).load("plain", CursorIcon::Default, wanted);
            image.unwrap().size.0
        };
        assert_eq!(size(24), 24);
        assert_eq!(size(30), 32);
        assert_eq!(size(45), 48);
        assert_eq!(size(500), 48);
        assert_eq!(size(1), 24);
    }

    #[test]
    fn a_shape_is_found_under_its_older_names() {
        let directory = TempDir::new().unwrap();
        // What `default` and `pointer` were called before the names were
        // agreed on.
        install(
            directory.path(),
            "old",
            &[("left_ptr", &[(4, RED)]), ("hand2", &[(4, BLUE)])],
        );
        let themes = themes(&directory);
        assert!(themes.load("old", CursorIcon::Default, 24).is_some());
        assert!(themes.load("old", CursorIcon::Pointer, 24).is_some());
        assert_eq!(themes.load("old", CursorIcon::Text, 24), None);
    }

    #[test]
    fn a_theme_falls_back_on_the_themes_it_names_and_then_the_default() {
        let directory = TempDir::new().unwrap();
        let path = directory.path();
        install(path, "fancy", &[("default", &[(4, RED)])]);
        install(path, "base", &[("text", &[(8, RED)])]);
        install(path, "default", &[("help", &[(16, RED)])]);
        fs::write(
            path.join("fancy/index.theme"),
            "[Icon Theme]\nName=Fancy\nInherits = missing, base\n",
        )
        .unwrap();
        let themes = themes(&directory);
        let size = |icon| themes.load("fancy", icon, 24).map(|image| image.size.0);
        assert_eq!(size(CursorIcon::Default), Some(4), "its own");
        assert_eq!(size(CursorIcon::Text), Some(8), "from the theme it names");
        assert_eq!(size(CursorIcon::Help), Some(16), "from the common one");
        assert_eq!(size(CursorIcon::Wait), None, "which nobody has");
        assert!(
            themes.load("no-such-theme", CursorIcon::Help, 24).is_some(),
            "a theme that is not installed is the common one"
        );
        for outside in ["../default", "", "..", "."] {
            let found = themes.load(outside, CursorIcon::Default, 24);
            assert_eq!(found, None, "{outside:?} is not a theme's name");
            assert!(themes.load(outside, CursorIcon::Help, 24).is_some());
        }
    }

    #[test]
    fn themes_that_name_each_other_do_not_go_round_for_ever() {
        let directory = TempDir::new().unwrap();
        let path = directory.path();
        for (theme, other) in [("one", "two"), ("two", "one")] {
            install(path, theme, &[]);
            let index = format!("Inherits={other},../../etc,a/b,,..\n");
            fs::write(path.join(theme).join("index.theme"), index).unwrap();
        }
        assert_eq!(themes(&directory).load("one", CursorIcon::Text, 24), None);

        // Nor does a long chain of them.
        for number in 0..20 {
            let theme = format!("chain{number}");
            install(path, &theme, &[]);
            let index = format!("Inherits=chain{}\n", number + 1);
            fs::write(path.join(&theme).join("index.theme"), index).unwrap();
        }
        install(path, "chain20", &[("text", &[(4, RED)])]);
        assert_eq!(
            themes(&directory).load("chain0", CursorIcon::Text, 24),
            None,
            "further away than is followed"
        );
    }

    #[test]
    fn a_file_that_is_not_a_cursor_or_is_too_large_is_not_used() {
        let directory = TempDir::new().unwrap();
        let path = directory.path();
        install(path, "broken", &[]);
        fs::write(path.join("broken/cursors/text"), b"not a cursor").unwrap();
        fs::write(path.join("broken/index.theme"), [0xff, 0xfe, 0x00]).unwrap();
        assert_eq!(
            themes(&directory).load("broken", CursorIcon::Text, 24),
            None
        );

        let huge = File::create(path.join("broken/cursors/help")).unwrap();
        huge.set_len(MAX_FILE + 1).unwrap();
        assert_eq!(
            themes(&directory).load("broken", CursorIcon::Help, 24),
            None
        );
        assert_eq!(
            read(&path.join("broken/cursors/text"), 12).unwrap().len(),
            12
        );
        assert_eq!(read(&path.join("broken/cursors/text"), 11), None);
        // A directory can be opened but not read.
        assert_eq!(read(&path.join("broken/cursors"), 100), None);
    }

    #[test]
    fn earlier_directories_win() {
        let (first, second) = (TempDir::new().unwrap(), TempDir::new().unwrap());
        install(first.path(), "plain", &[("text", &[(4, RED)])]);
        install(
            second.path(),
            "plain",
            &[("text", &[(8, RED)]), ("help", &[(8, RED)])],
        );
        let themes = Themes::new(vec![first.path().to_owned(), second.path().to_owned()]);
        let size = |icon| themes.load("plain", icon, 24).map(|image| image.size.0);
        assert_eq!(size(CursorIcon::Text), Some(4));
        assert_eq!(size(CursorIcon::Help), Some(8), "only in the second");
    }

    #[test]
    fn themes_are_looked_for_in_the_conventional_places() {
        let home = OsStr::new("/home/user");
        assert_eq!(
            Themes::conventional(None, Some(home)).directories,
            [
                "/home/user/.local/share/icons",
                "/home/user/.icons",
                "/usr/local/share/icons",
                "/usr/share/icons",
            ]
            .map(PathBuf::from)
        );
        assert_eq!(
            Themes::conventional(Some(OsStr::new("")), None).directories,
            ["/usr/local/share/icons", "/usr/share/icons"].map(PathBuf::from),
            "an empty list is no list, and without a home there is only the system"
        );
        assert_eq!(
            Themes::conventional(Some(OsStr::new("/a:/b/c")), Some(home)).directories,
            ["/a", "/b/c"].map(PathBuf::from),
            "a list replaces them"
        );
    }
}
