//! The icons of programs: finding the picture a desktop entry names and
//! reading it; and the name an application's desktop entry gives it.
//!
//! Icons are other programs' files, in two formats: PNG, in one directory
//! for each size it comes in, and SVG, which is drawn at whatever size is
//! wanted. Only so much of a file is read, a picture is only so large, and
//! one that cannot be made sense of is no icon.
//!
//! The signs of the interface are omorfos's own, built in: those of
//! Lucide that are used, in `signs/` as Lucide gives them (ADR 70).

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use omorfos_config::Color;
use omorfos_core::Sign;
use omorfos_ui::Image;
use resvg::{tiny_skia, usvg};
use rustix::fs::{Mode, OFlags};

/// The most of an icon's file that is read, in bytes.
const LARGEST_FILE: u64 = 2 * 1024 * 1024;

/// The most pixels an icon may have along a side.
const LARGEST_SIDE: u32 = 1024;

/// The theme icons are looked for in unless another is chosen: the one that
/// has the icons programs name without bringing their own, such as
/// `dialog-information`.
const THEME: &str = "Adwaita";

/// The theme every program's own icon is installed in, which every theme
/// comes back to.
const HICOLOR: &str = "hicolor";

/// The most themes icons are looked for in: the one chosen, those it
/// inherits from, and `hicolor`. Themes are other programs' files, and one
/// may name more, or name one that names it back.
const MOST_THEMES: usize = 8;

/// The most of a theme's `index.theme` that is read, in bytes. hicolor's
/// lists some five hundred folders in 55 KiB.
const LONGEST_INDEX: u64 = 256 * 1024;

/// How far either side of its size a folder's pictures do, unless its
/// index says otherwise.
const THRESHOLD: u32 = 2;

/// The kinds of icon a theme sorts its pictures into, each a directory,
/// where its index does not list its folders.
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

/// The most icons kept as found, each at a size and in a colour: more
/// than any shell shows at once, and few enough that names programs choose
/// cannot make it grow without end.
const MOST_FOUND: usize = 512;

/// The icons there are, and those that have been looked up so far.
#[derive(Debug)]
pub struct Icons {
    /// Where icons are installed, the user's own first.
    directories: Vec<PathBuf>,
    /// The themes icons are looked for in, in order, each with its
    /// folders as its index lists them.
    themes: Vec<Theme>,
    /// What each name came to at each size it was wanted at.
    found: HashMap<(String, u32, Color), Option<Image>>,
    /// Each sign as it was drawn at each size and in each colour.
    signs: HashMap<(Sign, u32, Color), Option<Image>>,
}

impl Default for Icons {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl Icons {
    /// The icons installed in `directories`, the first of which counts
    /// where several have one of the same name. A directory holds themes
    /// (`hicolor/48x48/apps/name.png`), or pictures directly (`name.png`).
    ///
    /// They are looked for in Adwaita's theme until another is chosen.
    #[must_use]
    pub fn new(directories: Vec<PathBuf>) -> Self {
        let themes = laid_out(chain(THEME, &directories), &directories);
        Self {
            directories,
            themes,
            found: HashMap::new(),
            signs: HashMap::new(),
        }
    }

    /// Looks icons up in the theme called `theme` from now on: in it, then
    /// in the themes its `index.theme` says it inherits from, then in
    /// `hicolor`, each in every directory before the next. What was found
    /// in the theme before is forgotten.
    pub fn set_theme(&mut self, theme: &str) {
        let names = chain(theme, &self.directories);
        self.themes = laid_out(names, &self.directories);
        self.found.clear();
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
        icon_directories(&data_directories(data_home, home, data_dirs))
    }

    /// The icon called `name`, to be drawn `pixels` pixels across beside
    /// text of the colour `ink`. `name` is as a program gives it: a name,
    /// or the whole path of a picture. Nothing if there is no such icon or
    /// it cannot be read.
    ///
    /// Where a theme has an icon only as a shape, the shape is what is
    /// found, drawn in `ink`.
    pub fn get(&mut self, name: &str, pixels: u32, ink: Color) -> Option<&Image> {
        // A name may be a program's own, as one that has no icon of its
        // name: past the most kept, everything found is let go of and found
        // again as it is wanted.
        let wanted = (name.to_owned(), pixels, ink);
        if self.found.len() >= MOST_FOUND && !self.found.contains_key(&wanted) {
            self.found.clear();
        }
        let known = self.found.entry(wanted);
        let (directories, themes) = (&self.directories, &self.themes);
        known
            .or_insert_with(|| {
                let file = locate((directories, themes), name, pixels);
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

impl Icons {
    /// The sign `sign`, drawn `pixels` pixels across in `ink`. Each is
    /// Lucide's drawing, on a grid of 24, with its lines 2.25 wide where
    /// Lucide has them 2, so that they are 1.5 wide at 16 across, as
    /// `docs/DESIGN.md` has them. Nothing only if it cannot be drawn,
    /// which the tests see to for each.
    pub fn sign(&mut self, sign: Sign, pixels: u32, ink: Color) -> Option<&Image> {
        let drawn = self.signs.entry((sign, pixels, ink)).or_insert_with(|| {
            let source = drawing(sign).replacen(LUCIDE_LINE, SIGN_LINE, 1);
            svg(source.as_bytes(), pixels).map(|shape| shape.tinted(ink))
        });
        drawn.as_ref()
    }
}

/// How wide Lucide draws its lines, as its drawings say it.
const LUCIDE_LINE: &str = "stroke-width=\"2\"";

/// How wide the lines of a sign are drawn instead.
const SIGN_LINE: &str = "stroke-width=\"2.25\"";

/// Lucide's drawing of each sign, as Lucide gives it.
const fn drawing(sign: Sign) -> &'static str {
    match sign {
        Sign::Close => include_str!("../signs/x.svg"),
        Sign::Minimize => include_str!("../signs/minus.svg"),
        Sign::Fullscreen => include_str!("../signs/fullscreen.svg"),
        Sign::Tile => include_str!("../signs/layout-panel-left.svg"),
        Sign::LeftHalf => include_str!("../signs/panel-left.svg"),
        Sign::RightHalf => include_str!("../signs/panel-right.svg"),
        Sign::TopHalf => include_str!("../signs/panel-top.svg"),
        Sign::BottomHalf => include_str!("../signs/panel-bottom.svg"),
        // To send to the end of a line, as Tab does.
        Sign::Send | Sign::Tab => include_str!("../signs/arrow-right-to-line.svg"),
        Sign::Add => include_str!("../signs/plus.svg"),
        Sign::Expand => include_str!("../signs/expand.svg"),
        Sign::Shade => include_str!("../signs/panel-top-close.svg"),
        Sign::Pin => include_str!("../signs/pin.svg"),
        Sign::Everywhere => include_str!("../signs/view.svg"),
        Sign::Awake => include_str!("../signs/coffee.svg"),
        Sign::Search => include_str!("../signs/search.svg"),
        Sign::Reload => include_str!("../signs/refresh-cw.svg"),
        Sign::About => include_str!("../signs/info.svg"),
        Sign::Quit => include_str!("../signs/log-out.svg"),
        Sign::Lock => include_str!("../signs/lock.svg"),
        Sign::Sleep => include_str!("../signs/moon.svg"),
        Sign::Restart => include_str!("../signs/rotate-ccw.svg"),
        Sign::ShutDown => include_str!("../signs/power.svg"),
        Sign::Super => include_str!("../signs/layout-grid.svg"),
        Sign::Ctrl => include_str!("../signs/chevron-up.svg"),
        Sign::Alt => include_str!("../signs/option.svg"),
        Sign::Shift => include_str!("../signs/arrow-big-up.svg"),
        Sign::Enter => include_str!("../signs/corner-down-left.svg"),
        Sign::Space => include_str!("../signs/space.svg"),
        Sign::Up => include_str!("../signs/arrow-up.svg"),
        Sign::Down => include_str!("../signs/arrow-down.svg"),
        Sign::Left => include_str!("../signs/arrow-left.svg"),
        Sign::Right => include_str!("../signs/arrow-right.svg"),
        Sign::Backspace => include_str!("../signs/delete.svg"),
        Sign::Browser => include_str!("../signs/globe.svg"),
        Sign::Files => include_str!("../signs/folder.svg"),
        Sign::Editor => include_str!("../signs/file-pen.svg"),
        Sign::Terminal => include_str!("../signs/terminal.svg"),
        Sign::Settings => include_str!("../signs/settings.svg"),
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

/// Where icons are installed, given where programs install what they
/// share (see [`data_directories`]): the `icons` directory of each, and
/// then `/usr/share/pixmaps`, where programs without a theme put theirs.
#[must_use]
pub fn icon_directories(data: &[PathBuf]) -> Vec<PathBuf> {
    let themed = data.iter().map(|directory| directory.join("icons"));
    themed
        .chain([PathBuf::from("/usr/share/pixmaps")])
        .collect()
}

/// The most of a desktop entry that is read for an application's name, in
/// bytes. Real ones are a few kilobytes, most of it translations.
const LONGEST_ENTRY: u64 = 256 * 1024;

/// The longest identifier an application is looked up by.
const LONGEST_ID: usize = 255;

/// What an application calls itself, by its identifier `app_id`, such as
/// `org.mozilla.firefox`: the name its desktop entry gives, from the
/// `applications` directory of the first of `directories` (see
/// [`data_directories`]) that has one. Nothing, for an identifier that is
/// not one, since it is another program's word and is not to reach
/// outside the directory, or where no entry gives a name.
#[must_use]
pub fn named(app_id: &str, directories: &[PathBuf]) -> Option<String> {
    entry_says(app_id, directories, "Name")
}

/// The icon an application's desktop entry names for it (`Icon`), found
/// as [`named`] finds its name: a name to look up, or the whole path of a
/// picture.
#[must_use]
pub fn icon_of(app_id: &str, directories: &[PathBuf]) -> Option<String> {
    entry_says(app_id, directories, "Icon")
}

/// What the desktop entry of `app_id` gives for `key`, from the first of
/// `directories` whose entry gives it.
fn entry_says(app_id: &str, directories: &[PathBuf], key: &str) -> Option<String> {
    let allowed = |byte: u8| byte.is_ascii_alphanumeric() || b"._-".contains(&byte);
    let valid = (1..=LONGEST_ID).contains(&app_id.len()) && app_id.bytes().all(allowed);
    let file = format!("{app_id}.desktop");
    let entries = directories.iter().filter(|_| valid);
    let entries = entries.map(|directory| directory.join("applications").join(&file));
    let mut read = entries.filter_map(|path| {
        let mut entry = String::new();
        let file = File::open(path).ok();
        let read = file.and_then(|file| file.take(LONGEST_ENTRY).read_to_string(&mut entry).ok());
        read.map(|_| entry)
    });
    read.find_map(|entry| value(&entry, "[Desktop Entry]", key))
}

/// What a desktop entry, or a theme's index, gives for `key`, untranslated,
/// in its `section`, if it is not empty.
fn value(entry: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    entry.lines().map(str::trim).find_map(|line| {
        if line.starts_with('[') {
            inside = line == section;
        }
        let named = line.split_once('=').filter(|_| inside);
        let named = named.filter(|(given, _)| given.trim() == key);
        let named = named.map(|(_, value)| value.trim());
        named.filter(|value| !value.is_empty()).map(str::to_owned)
    })
}

/// Reads a PNG from memory, as an icon would be read from a file. Nothing
/// if it is not one or is larger than an icon may be.
#[must_use]
pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    png(bytes)
}

/// The themes icons are looked for in when `theme` is chosen: it, then
/// those it inherits from as its `index.theme` in the first of `directories`
/// that has one says, each before what it inherits from in turn, and then
/// `hicolor`; each once, no more than [`MOST_THEMES`], and only those whose
/// names name one directory where themes are.
fn chain(theme: &str, directories: &[PathBuf]) -> Vec<String> {
    let mut themes: Vec<String> = Vec::new();
    let mut waiting = vec![theme.to_owned()];
    while let Some(name) = waiting.pop() {
        if themes.len() + 1 >= MOST_THEMES {
            break;
        }
        if name == HICOLOR || themes.contains(&name) || !plain(&name) {
            continue;
        }
        let inherits = inherits(&name, directories);
        waiting.extend(inherits.into_iter().rev());
        themes.push(name);
    }
    themes.push(HICOLOR.to_owned());
    themes
}

/// Whether `name` names one directory and no other: something, with no
/// `/` or `\\` or anything unseen in it, and no `.` to begin it.
fn plain(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
}

/// The index of the theme called `name`, `index.theme`, in the first of
/// `directories` that has one; nothing written if none has.
fn index(name: &str, directories: &[PathBuf]) -> String {
    let indexes = directories
        .iter()
        .map(|directory| directory.join(name).join("index.theme"));
    let mut read = indexes.filter_map(|path| {
        let mut index = String::new();
        let file = File::open(path).ok();
        let read = file.and_then(|file| file.take(LONGEST_INDEX).read_to_string(&mut index).ok());
        read.map(|_| index)
    });
    read.next().unwrap_or_default()
}

/// The themes the theme called `name` says it inherits from, in the first
/// of `directories` that has it.
fn inherits(name: &str, directories: &[PathBuf]) -> Vec<String> {
    let index = index(name, directories);
    let named = value(&index, "[Icon Theme]", "Inherits").unwrap_or_default();
    let named = named
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty());
    named.map(str::to_owned).collect()
}

/// A theme icons are looked for in.
#[derive(Debug)]
struct Theme {
    /// Its name, which is a directory's in each directory of icons.
    name: String,
    /// The folders its index lists, in its order; or nothing, if it lists
    /// none, and its folders are known by their names.
    folders: Option<Vec<Folder>>,
}

/// A folder of a theme, the sizes in pixels its pictures do for, from the
/// least to the most, and the names of the files in it in each directory
/// of icons, in their order.
#[derive(Debug)]
struct Folder {
    path: PathBuf,
    sizes: (u32, u32),
    /// Read once, when the theme is chosen: a theme lists hundreds of
    /// folders, mostly empty, and looking in each for each icon not there
    /// took a tenth of a second.
    files: Vec<HashSet<String>>,
}

/// The themes called `names`, each with its folders as its index, in the
/// first of `directories` that has one, lists them, and what is in each.
fn laid_out(names: Vec<String>, directories: &[PathBuf]) -> Vec<Theme> {
    let themes = names.into_iter().map(|name| {
        let mut folders = folders(&index(&name, directories));
        for folder in folders.iter_mut().flatten() {
            let places = directories.iter();
            let places = places.map(|directory| directory.join(&name).join(&folder.path));
            folder.files = places.map(|place| listing(&place)).collect();
        }
        Theme { name, folders }
    });
    themes.collect()
}

/// The names of the files in the folder at `place`; none if there is no
/// such folder.
fn listing(place: &Path) -> HashSet<String> {
    let entries = fs::read_dir(place).into_iter().flatten().flatten();
    entries
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// The folders a theme's `index` lists, in `Directories` and then in
/// `ScaledDirectories`, as the icon theme specification has them; nothing
/// if it lists none. A folder is left out that is not described, has no
/// size, or would be outside the theme.
fn folders(index: &str) -> Option<Vec<Folder>> {
    let sections = sections(index);
    let theme = sections.get("Icon Theme")?;
    let lists = ["Directories", "ScaledDirectories"].map(|key| theme.get(key));
    let named = lists.into_iter().flatten().flat_map(|list| list.split(','));
    let named: Vec<&str> = named.map(str::trim).filter(|name| inside(name)).collect();
    (!named.is_empty()).then(|| {
        let described = named.into_iter().filter_map(|name| {
            let section = sections.get(name)?;
            let number = |key: &str| section.get(key).and_then(|value| value.parse::<u32>().ok());
            let size = number("Size")?;
            // A picture twice as sharp is twice as many pixels.
            let scale = number("Scale").unwrap_or(1).max(1);
            let (least, most) = match section.get("Type").copied() {
                Some("Fixed") => (size, size),
                Some("Scalable") => (
                    number("MinSize").unwrap_or(size),
                    number("MaxSize").unwrap_or(size),
                ),
                _ => {
                    let either = number("Threshold").unwrap_or(THRESHOLD);
                    (size.saturating_sub(either), size.saturating_add(either))
                }
            };
            let sizes = (least.saturating_mul(scale), most.saturating_mul(scale));
            Some(Folder {
                path: PathBuf::from(name),
                sizes,
                files: Vec::new(),
            })
        });
        described.collect()
    })
}

/// Whether a folder an index names is one inside its theme: a path of
/// names alone, with no `..`, no root and nothing unseen.
fn inside(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && !name.chars().any(char::is_control)
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

/// Each section of a theme's index with its keys and their values, read
/// once: the first value of a key counts, and what comes before any
/// section is no part of one.
fn sections(index: &str) -> HashMap<&str, HashMap<&str, &str>> {
    let mut sections: HashMap<&str, HashMap<&str, &str>> = HashMap::new();
    let mut section = None;
    for line in index.lines().map(str::trim) {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = Some(name);
        } else if let Some(((key, value), name)) = line.split_once('=').zip(section) {
            let keys = sections.entry(name).or_default();
            keys.entry(key.trim()).or_insert(value.trim());
        }
    }
    sections
}

/// Finds the file of the icon called `name` that suits `pixels` best: the
/// icon itself if any theme has it, in the order of `themes`, or a picture
/// with no theme does, and failing that its shape.
fn locate(
    (directories, themes): (&[PathBuf], &[Theme]),
    name: &str,
    pixels: u32,
) -> Option<PathBuf> {
    let given = Path::new(name);
    if given.is_absolute() {
        return given.is_file().then(|| given.to_owned());
    }
    if name.is_empty() || name.contains('/') {
        return None;
    }
    let wanted = [name.to_owned(), format!("{name}{SHAPE}")];
    wanted.iter().find_map(|name| {
        let mut places = themes.iter().flat_map(|theme| {
            let directories = directories.iter().enumerate();
            directories.map(move |(which, directory)| (which, directory.join(&theme.name), theme))
        });
        let found = places.find_map(|(which, place, theme)| match &theme.folders {
            Some(folders) => listed((which, &place), folders, name, pixels),
            None => within(&place, name, pixels),
        });
        // Pictures with no theme lie in a directory as they are.
        found.or_else(|| {
            let files = ["png", "svg"].map(|extension| format!("{name}.{extension}"));
            let mut loose = directories
                .iter()
                .flat_map(|directory| files.iter().map(move |file| directory.join(file)));
            loose.find(|file| file.is_file())
        })
    })
}

/// Finds the file of the icon called `name` in one theme, in the directory
/// of icons that is the `which`th, among the `folders` its index lists, as
/// they were when it was chosen: the first whose sizes
/// take in `pixels`; failing that, the one whose least size is the least
/// above it; and failing that, the one whose most is the most below it. A
/// PNG comes before a drawing in the same folder.
fn listed(
    (which, theme): (usize, &Path),
    folders: &[Folder],
    name: &str,
    pixels: u32,
) -> Option<PathBuf> {
    let files = ["png", "svg"].map(|extension| format!("{name}.{extension}"));
    let candidates = folders.iter().flat_map(|folder| {
        let held = folder.files.get(which);
        let files = files
            .iter()
            .filter(move |file| held.is_some_and(|held| held.contains(*file)));
        files.map(move |file| (folder.sizes, theme.join(&folder.path).join(file)))
    });
    let candidates = candidates.enumerate();
    let best = candidates.min_by_key(|(order, ((least, most), _))| {
        let fit = if *least > pixels {
            (1, least - pixels)
        } else if *most < pixels {
            (2, pixels - most)
        } else {
            (0, 0)
        };
        (fit, *order)
    });
    best.map(|(_, (_, file))| file)
}

/// Finds the file of the icon called `name` in one theme, in one directory
/// of icons, whose index lists no folders: by the names of its folders.
fn within(theme: &Path, name: &str, pixels: u32) -> Option<PathBuf> {
    let files = ["png", "svg"].map(|extension| format!("{name}.{extension}"));
    let themes = sizes(theme);
    let sorted = themes.into_iter().flat_map(|(size, folder)| {
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
    let mut candidates: Vec<(u32, PathBuf)> = sorted.filter(|(_, file)| file.is_file()).collect();
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

/// Reads a picture, to be drawn `pixels` pixels across. Nothing if it is
/// not a plain file when it is opened: a path can be a program's, and
/// opening a pipe, or a file under a lease, would wait on that program.
fn load(file: &Path, pixels: u32) -> Option<Image> {
    let mut bytes = Vec::new();
    let flags = OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC;
    let opened = rustix::fs::open(file, flags, Mode::empty())
        .ok()
        .map(File::from);
    let plain = |opened: &File| opened.metadata().is_ok_and(|opened| opened.is_file());
    let opened = opened.filter(plain);
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
    // A drawing may name a file to draw in it, which would be read whole,
    // whatever it is, as `/dev/zero` is never done being read: an icon is
    // drawn from what is written in it alone, pictures written in it
    // included.
    let options = usvg::Options {
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            ..usvg::ImageHrefResolver::default()
        },
        ..usvg::Options::default()
    };
    let tree = usvg::Tree::from_data(bytes, &options).ok();
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
#[expect(
    clippy::disallowed_methods,
    reason = "a test ends with its threads, and one that cannot be started fails it"
)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_that_cannot_be_read_at_once_is_not_waited_for() {
        // A pipe of no writer waits in `open` for one, as a file under a
        // lease waits for its holder: a path a program gives can be either.
        let directory = tempfile::tempdir().unwrap();
        let pipe = directory.path().join("icon.png");
        let mode = Mode::RUSR | Mode::WUSR;
        rustix::fs::mknodat(rustix::fs::CWD, &pipe, rustix::fs::FileType::Fifo, mode, 0).unwrap();
        let (done, loaded) = std::sync::mpsc::channel();
        std::thread::spawn(move || done.send(load(&pipe, 16).is_none()));
        let loaded = loaded.recv_timeout(std::time::Duration::from_secs(2));
        assert_eq!(loaded, Ok(true));
    }

    #[test]
    fn no_more_icons_are_kept_as_found_than_the_most() {
        let mut icons = Icons::new(Vec::new());
        for name in 0..2000 {
            let _ = icons.get(
                &format!("no-such-icon-{name}"),
                16,
                Color::rgba(0, 0, 0, 0xff),
            );
            assert!(icons.found.len() <= MOST_FOUND);
        }
        // One wanted again while it is kept is not looked for again.
        let _ = icons.get("no-such-icon-1999", 16, Color::rgba(0, 0, 0, 0xff));
        assert!(icons.found.contains_key(&(
            "no-such-icon-1999".to_owned(),
            16,
            Color::rgba(0, 0, 0, 0xff)
        )));
    }

    #[test]
    fn an_application_is_named_by_its_desktop_entry() {
        let named = |entry: &str| value(entry, "[Desktop Entry]", "Name");
        let entry = "[Desktop Entry]\nType=Application\nName[pl]=Przykład\nName = Example \n";
        assert_eq!(named(entry).as_deref(), Some("Example"), "not translated");
        let elsewhere = "[Desktop Action new]\nName=New Window\n[Desktop Entry]\nName=Main\n";
        assert_eq!(
            named(elsewhere).as_deref(),
            Some("Main"),
            "its own section only"
        );
        assert_eq!(named("[Desktop Entry]\nName=\n"), None, "nor an empty name");
        assert_eq!(named("Name=Stray\n"), None, "nor one outside any section");
    }

    #[test]
    fn an_application_is_looked_up_where_entries_are_installed_and_nowhere_else() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let applications = second.path().join("applications");
        fs::create_dir_all(&applications).unwrap();
        let entry = "[Desktop Entry]\nName=Example App\n";
        fs::write(applications.join("org.example.App.desktop"), entry).unwrap();
        fs::write(second.path().join("outside.desktop"), entry).unwrap();
        let directories = [first.path().to_owned(), second.path().to_owned()];
        let found = named("org.example.App", &directories);
        assert_eq!(
            found.as_deref(),
            Some("Example App"),
            "in the first that has it"
        );
        assert_eq!(named("org.example.Other", &directories), None);
        assert_eq!(
            icon_of("org.example.App", &directories),
            None,
            "it gives none"
        );
        let iconic = "[Desktop Entry]\nName=Iconic\nIcon=org.example.Iconic\n";
        fs::write(applications.join("org.example.Iconic.desktop"), iconic).unwrap();
        assert_eq!(
            icon_of("org.example.Iconic", &directories).as_deref(),
            Some("org.example.Iconic")
        );
        // An identifier is another program's word: one that is not a name,
        // or that would reach outside, is looked for nowhere.
        let long = "a".repeat(256);
        for wrong in [
            "",
            "../outside",
            "applications/../../outside",
            "a b",
            long.as_str(),
        ] {
            assert_eq!(named(wrong, &directories), None, "{wrong:?}");
        }
    }

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

    /// Every sign there is.
    const SIGNS: [Sign; 40] = [
        Sign::Close,
        Sign::Minimize,
        Sign::Fullscreen,
        Sign::Tile,
        Sign::LeftHalf,
        Sign::RightHalf,
        Sign::TopHalf,
        Sign::BottomHalf,
        Sign::Send,
        Sign::Add,
        Sign::Expand,
        Sign::Shade,
        Sign::Pin,
        Sign::Everywhere,
        Sign::Awake,
        Sign::Search,
        Sign::Reload,
        Sign::About,
        Sign::Quit,
        Sign::Lock,
        Sign::Sleep,
        Sign::Restart,
        Sign::ShutDown,
        Sign::Super,
        Sign::Ctrl,
        Sign::Alt,
        Sign::Shift,
        Sign::Enter,
        Sign::Space,
        Sign::Tab,
        Sign::Up,
        Sign::Down,
        Sign::Left,
        Sign::Right,
        Sign::Backspace,
        Sign::Browser,
        Sign::Files,
        Sign::Editor,
        Sign::Terminal,
        Sign::Settings,
    ];

    /// How much ink there is down the column `x` of `image` drawn on white,
    /// in pixels' worth: 1 for each pixel all ink.
    fn ink_down(image: &Image, x: u32) -> f64 {
        let (width, height) = image.size();
        let mut canvas = omorfos_ui::Canvas::new(width, height);
        canvas.fill((0, 0, width, height), Color::rgb(255, 255, 255));
        canvas.draw_image((0, 0, width, height), image);
        let red = (0..height).map(|y| canvas.at(x, y) >> 16 & 0xff);
        red.map(|red| f64::from(255 - red) / 255.0).sum()
    }

    #[test]
    fn every_sign_is_drawn_in_the_colour_of_its_text_with_lines_as_wide_as_the_design_says() {
        let mut icons = Icons::default();
        for sign in SIGNS {
            assert_eq!(drawing(sign).matches(LUCIDE_LINE).count(), 1, "{sign:?}");
            let drawn = icons.sign(sign, 16, INK).unwrap();
            assert_eq!(drawn.size(), (16, 16));
            let mut canvas = omorfos_ui::Canvas::new(16, 16);
            canvas.fill((0, 0, 16, 16), Color::rgb(255, 255, 255));
            canvas.draw_image((0, 0, 16, 16), drawn);
            let pixels: Vec<u32> = (0..16)
                .flat_map(|y| (0..16).map(move |x| (x, y)))
                .map(|(x, y)| canvas.at(x, y))
                .collect();
            assert!(
                pixels.iter().any(|&pixel| pixel != 0xffff_ffff),
                "{sign:?} is drawn"
            );
            // Blue on white is white less red and green alike.
            let inked =
                |pixel: u32| pixel >> 16 & 0xff == pixel >> 8 & 0xff && pixel & 0xff == 0xff;
            assert!(
                pixels.iter().all(|&pixel| inked(pixel)),
                "{sign:?} is in its ink"
            );
        }
        // The line of the minus goes across the middle, 1.5 wide at 16
        // across and 3 at 32.
        let small = ink_down(icons.sign(Sign::Minimize, 16, INK).unwrap(), 8);
        assert!((small - 1.5).abs() < 0.1, "{small}");
        let large = ink_down(icons.sign(Sign::Minimize, 32, INK).unwrap(), 16);
        assert!((large - 3.0).abs() < 0.1, "{large}");
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
    fn a_drawn_icon_reads_no_file_it_names_and_draws_what_is_written_in_it() {
        let root = tempfile::TempDir::new().unwrap();
        // A drawing of red beside it, named by its whole path, and one of
        // blue written in it.
        let square = |colour: &str| {
            format!(
                "<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10'>\
                 <rect width='10' height='10' fill='{colour}'/></svg>"
            )
        };
        let red = root.path().join("red.svg");
        fs::write(&red, square("#ff0000")).unwrap();
        let written = base64(square("#0000ff").as_bytes());
        let drawing = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' width='20' height='10'>\
             <image href='{}' x='0' y='0' width='10' height='10'/>\
             <image href='data:image/svg+xml;base64,{written}' x='10' y='0' \
             width='10' height='10'/></svg>",
            red.display()
        );
        let drawn = svg(drawing.as_bytes(), 20).unwrap();
        assert_eq!(pixel(&drawn, 5, 10), 0, "the file named is not read");
        assert_eq!(
            pixel(&drawn, 15, 10),
            0xff00_00ff,
            "what is written is drawn"
        );
    }

    /// `bytes` in base64, as a picture is written in a drawing; with spaces
    /// after them as needed for whole groups of three, which a drawing
    /// passes over.
    fn base64(bytes: &[u8]) -> String {
        const LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut padded = bytes.to_vec();
        padded.resize(bytes.len().next_multiple_of(3), b' ');
        let mut written = String::new();
        for three in padded.chunks(3) {
            let word = u32::from(three[0]) << 16 | u32::from(three[1]) << 8 | u32::from(three[2]);
            for at in 0..4 {
                written.push(char::from(LETTERS[(word >> (18 - 6 * at) & 63) as usize]));
            }
        }
        written
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
    fn the_theme_chosen_comes_first_then_what_it_inherits_then_hicolor() {
        let root = tempfile::TempDir::new().unwrap();
        let (own, system) = (root.path().join("own"), root.path().join("system"));
        // Papirus inherits from Breeze, which inherits from what it names.
        write(
            &system,
            "Papirus/index.theme",
            b"[Icon Theme]\nName=Papirus\nInherits=breeze,hicolor\n",
        );
        write(
            &system,
            "breeze/index.theme",
            b"[Icon Theme]\nInherits=../../escape, .hidden\n",
        );
        write(
            &system,
            "Papirus/32x32/apps/tool.png",
            &png_of(32, [1, 1, 1, 255]),
        );
        write(&system, "breeze/32x32/apps/tool.png", &png_of(31, [0; 4]));
        write(
            &system,
            "breeze/32x32/apps/only-breeze.png",
            &png_of(30, [0; 4]),
        );
        write(
            &system,
            "hicolor/32x32/apps/everywhere.png",
            &png_of(29, [0; 4]),
        );
        // A theme of the user's own, under the same name, comes first.
        write(&own, "Papirus/32x32/apps/mine.png", &png_of(28, [0; 4]));
        write(&system, "Papirus/32x32/apps/mine.png", &png_of(27, [0; 4]));
        write(&system, "hicolor/32x32/apps/mine.png", &png_of(26, [0; 4]));
        // What would be outside the icons is never looked at.
        write(
            root.path(),
            "escape/32x32/apps/away.png",
            &png_of(25, [0; 4]),
        );
        let mut icons = Icons::new(vec![own, system]);
        icons.set_theme("Papirus");
        let side = |icons: &mut Icons, name| icons.get(name, 32, INK).map(|icon| icon.size().0);
        assert_eq!(side(&mut icons, "tool"), Some(32));
        assert_eq!(side(&mut icons, "only-breeze"), Some(30));
        assert_eq!(side(&mut icons, "everywhere"), Some(29));
        assert_eq!(side(&mut icons, "mine"), Some(28));
        assert_eq!(side(&mut icons, "away"), None);
        // Another theme, and what was found under the last is forgotten.
        icons.set_theme("breeze");
        assert_eq!(side(&mut icons, "tool"), Some(31));
        assert_eq!(side(&mut icons, "mine"), Some(26));
        // One that is not there is as good as hicolor alone.
        icons.set_theme("Missing");
        assert_eq!(side(&mut icons, "tool"), None);
        assert_eq!(side(&mut icons, "everywhere"), Some(29));
    }

    #[test]
    fn a_theme_is_looked_through_as_its_index_lays_it_out() {
        let root = tempfile::TempDir::new().unwrap();
        // As Breeze lays itself out: the kind first, then the size, with
        // kinds of its own, and its pictures twice as sharp listed apart.
        let index = "[Icon Theme]\n\
                     Directories=apps/16,apps/24,apps/48,emblems/16,apps/scalable\n\
                     ScaledDirectories=apps/24@2\n\
                     [apps/16]\nSize=16\n\
                     [apps/24]\nSize=24\nType=Fixed\n\
                     [apps/24@2]\nSize=24\nScale=2\nType=Fixed\n\
                     [apps/48]\nSize=48\nType=Threshold\nThreshold=4\n\
                     [emblems/16]\nSize=16\n\
                     [apps/scalable]\nSize=64\nType=Scalable\nMinSize=64\nMaxSize=256\n";
        write(root.path(), "Breeze/index.theme", index.as_bytes());
        write(root.path(), "Breeze/apps/16/tool.png", &png_of(16, [0; 4]));
        write(root.path(), "Breeze/apps/48/tool.png", &png_of(48, [0; 4]));
        write(
            root.path(),
            "Breeze/apps/scalable/tool.svg",
            WIDE_SVG.as_bytes(),
        );
        // A folder the index does not list is not looked in.
        write(root.path(), "Breeze/apps/32/tool.png", &png_of(32, [0; 4]));
        write(root.path(), "Breeze/apps/24/sharp.png", &png_of(24, [0; 4]));
        write(
            root.path(),
            "Breeze/apps/24@2/sharp.png",
            &png_of(48, [0; 4]),
        );
        write(
            root.path(),
            "Breeze/emblems/16/mark.png",
            &png_of(16, [0; 4]),
        );
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        icons.set_theme("Breeze");
        let side = |icons: &mut Icons, name, pixels| {
            icons.get(name, pixels, INK).map(|icon| icon.size().0)
        };
        assert_eq!(side(&mut icons, "tool", 16), Some(16));
        assert_eq!(side(&mut icons, "tool", 18), Some(16), "near enough");
        assert_eq!(
            side(&mut icons, "tool", 32),
            Some(48),
            "the next large enough"
        );
        assert_eq!(side(&mut icons, "tool", 52), Some(48), "near enough");
        assert_eq!(side(&mut icons, "tool", 100), Some(100), "drawn to size");
        assert_eq!(side(&mut icons, "tool", 400), Some(400), "the largest");
        assert_eq!(side(&mut icons, "sharp", 24), Some(24));
        assert_eq!(side(&mut icons, "sharp", 48), Some(48), "twice as sharp");
        assert_eq!(side(&mut icons, "mark", 16), Some(16));
    }

    #[test]
    fn what_an_index_says_of_its_folders_is_taken_with_care() {
        let root = tempfile::TempDir::new().unwrap();
        let outside = root.path().join("outside");
        let index = format!(
            "stray=before any section\n\
             [Icon Theme]\n\
             Directories=../outside,{},./here,unlisted,sizeless,odd,flat,backwards,here\n\
             [../outside]\nSize=16\n\
             [{}]\nSize=16\n\
             [./here]\nSize=16\n\
             [sizeless]\nType=Fixed\n\
             [odd]\nSize=20\nType=Sideways\n\
             [flat]\nSize=30\nScale=0\nType=Fixed\n\
             [backwards]\nSize=40\nType=Scalable\nMinSize=50\nMaxSize=10\n\
             [here]\nSize=16\nSize=99\n",
            outside.display(),
            outside.display(),
        );
        write(root.path(), "T/index.theme", index.as_bytes());
        // Nothing outside the theme is looked at, however the index names it.
        write(&outside, "away.png", &png_of(16, [0; 4]));
        write(root.path(), "T/here/near.png", &png_of(16, [0; 4]));
        write(root.path(), "T/odd/near.png", &png_of(20, [0; 4]));
        // A folder named but not described, or without a size, is not
        // looked in.
        write(root.path(), "T/unlisted/plain.png", &png_of(16, [0; 4]));
        write(root.path(), "T/sizeless/plain.png", &png_of(16, [0; 4]));
        // A kind not known is the usual, a scale of nothing is one, and a
        // range the wrong way round holds nothing.
        write(root.path(), "T/odd/odd.png", &png_of(20, [0; 4]));
        write(root.path(), "T/flat/flat.png", &png_of(30, [0; 4]));
        write(root.path(), "T/here/flat.png", &png_of(16, [0; 4]));
        write(root.path(), "T/backwards/turned.png", &png_of(40, [0; 4]));
        let mut icons = Icons::new(vec![root.path().to_owned()]);
        icons.set_theme("T");
        let side = |icons: &mut Icons, name, pixels| {
            icons.get(name, pixels, INK).map(|icon| icon.size().0)
        };
        assert_eq!(side(&mut icons, "away", 16), None);
        assert_eq!(
            side(&mut icons, "near", 16),
            Some(16),
            "the first size said"
        );
        assert_eq!(side(&mut icons, "plain", 16), None);
        assert_eq!(side(&mut icons, "odd", 22), Some(20), "two either side");
        assert_eq!(side(&mut icons, "flat", 30), Some(30));
        assert_eq!(side(&mut icons, "turned", 20), Some(40), "large enough");
    }

    #[test]
    fn themes_that_inherit_from_each_other_without_end_are_followed_only_so_far() {
        let root = tempfile::TempDir::new().unwrap();
        write(root.path(), "a/index.theme", b"[Icon Theme]\nInherits=b\n");
        write(root.path(), "b/index.theme", b"[Icon Theme]\nInherits=a\n");
        let mut many = String::from("[Icon Theme]\nInherits=");
        many.push_str(
            &(0..40)
                .map(|n| format!("t{n}"))
                .collect::<Vec<_>>()
                .join(","),
        );
        write(root.path(), "wide/index.theme", many.as_bytes());
        let directories = vec![root.path().to_owned()];
        assert_eq!(chain("a", &directories), ["a", "b", "hicolor"]);
        let wide = chain("wide", &directories);
        assert_eq!(wide.len(), MOST_THEMES);
        assert_eq!(wide.last().map(String::as_str), Some("hicolor"));
        assert_eq!(chain("hicolor", &directories), ["hicolor"]);
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
