//! What each screen was last given, kept on disk so that it is shown the
//! moment the next session starts, without the picture being read again.
//!
//! A screen's file holds the pixels drawn for it, with what they were drawn
//! from: the picture, its size and when it was last changed, and how it was
//! laid on a screen of how many pixels. They are shown again only if all of
//! that is still so. The file is the wallpaper's own, but it is read as
//! carefully as any other: what does not add up is not shown.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

/// What a file starts with: whose it is, and the version that wrote it,
/// since another may lay pictures down otherwise.
const MAGIC: &str = concat!("omorfos-wallpaper ", env!("CARGO_PKG_VERSION"), "\n");

/// The longest a path or a description in a file may be.
const LONGEST: u32 = 64 * 1024;

/// The most pixels a file may hold, as many as a picture may have.
const MOST_PIXELS: u64 = 1 << 26;

/// Where what was drawn is kept, if anywhere.
#[derive(Debug, Clone)]
pub(crate) struct Cache {
    directory: Option<PathBuf>,
}

/// The pixels a screen was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Kept {
    /// How many there are along each side.
    pub(crate) size: (u32, u32),
    /// Four bytes each, as a canvas has them.
    pub(crate) pixels: Vec<u8>,
}

impl Cache {
    /// What is kept in `directory`, or nothing kept at all.
    pub(crate) const fn new(directory: Option<PathBuf>) -> Self {
        Self { directory }
    }

    /// Where what the screen called `screen` was given is kept: nowhere for
    /// a screen that is not yet named, or whose name would be a path.
    fn file(&self, screen: &str) -> Option<PathBuf> {
        let plain = screen
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_".contains(c));
        let named = !screen.is_empty() && plain;
        let directory = self.directory.as_ref().filter(|_| named);
        directory.map(|directory| directory.join(screen))
    }

    /// The pixels `screen` was last given, if they were drawn from
    /// `picture` as `drawn` describes.
    pub(crate) fn recall(&self, screen: &str, picture: &Path, drawn: &str) -> Option<Kept> {
        let file = self.file(screen).and_then(|file| File::open(file).ok());
        let mut file = BufReader::new(file?);
        let read = header(&mut file).filter(|(from, how)| from == picture && how == drawn);
        read?;
        let (mut width, mut height) = ([0; 4], [0; 4]);
        file.read_exact(&mut width).ok()?;
        file.read_exact(&mut height).ok()?;
        let size = (u32::from_le_bytes(width), u32::from_le_bytes(height));
        let count = u64::from(size.0) * u64::from(size.1);
        let length = (count <= MOST_PIXELS).then_some(count * 4)?;
        // One byte more than there should be, to tell if there is.
        let mut pixels = Vec::new();
        // One that cannot be read to the end is found short.
        let _ = file.take(length + 1).read_to_end(&mut pixels);
        let whole = u64::try_from(pixels.len()) == Ok(length) && count > 0;
        whole.then_some(Kept { size, pixels })
    }

    /// The picture `screen` was last given pixels of, whatever they were
    /// laid on it as.
    pub(crate) fn picture(&self, screen: &str) -> Option<PathBuf> {
        let file = self.file(screen).and_then(|file| File::open(file).ok());
        header(&mut BufReader::new(file?)).map(|(picture, _)| picture)
    }

    /// Keeps the pixels `screen` was given, drawn from `picture` as `drawn`
    /// describes, in place of what was kept for it before.
    ///
    /// # Errors
    ///
    /// Returns an error if they cannot be written.
    pub(crate) fn keep(
        &self,
        screen: &str,
        picture: &Path,
        drawn: &str,
        (size, pixels): ((u32, u32), &[u8]),
    ) -> io::Result<()> {
        let Some(file) = self.file(screen) else {
            return Ok(());
        };
        // Only the one whose session it is may see what is on its screens.
        let mut directory = DirBuilder::new();
        directory.recursive(true).mode(0o700);
        let made = file
            .parent()
            .map_or(Ok(()), |parent| directory.create(parent));
        // Written beside it and put in its place whole, so that a session
        // that ends meanwhile leaves the one before, not half of this.
        let mut partial = file.clone().into_os_string();
        partial.push(".new");
        let opened = made.and_then(|()| {
            let mut options = OpenOptions::new();
            options.write(true).create(true).truncate(true).mode(0o600);
            options.open(&partial)
        });
        let written = opened.and_then(|mut opened| {
            let mut head = MAGIC.as_bytes().to_vec();
            for part in [picture.as_os_str().as_bytes(), drawn.as_bytes()] {
                let length = u32::try_from(part.len()).unwrap_or(u32::MAX);
                head.extend(length.to_le_bytes());
                head.extend(part);
            }
            head.extend(size.0.to_le_bytes());
            head.extend(size.1.to_le_bytes());
            let head = opened.write_all(&head);
            head.and_then(|()| opened.write_all(pixels))
        });
        written.and_then(|()| fs::rename(&partial, &file))
    }
}

/// The picture a file's pixels were drawn from and how, as its start says,
/// if it is a file of this version's.
fn header(file: &mut impl Read) -> Option<(PathBuf, String)> {
    let mut magic = vec![0; MAGIC.len()];
    file.read_exact(&mut magic).ok()?;
    let ours = magic == MAGIC.as_bytes();
    let mut part = || {
        let mut length = [0; 4];
        file.read_exact(&mut length).ok()?;
        let length = u32::from_le_bytes(length);
        let mut bytes = Vec::new();
        let short = length <= LONGEST;
        let taken = file.take(u64::from(length)).read_to_end(&mut bytes).ok();
        taken.filter(|read| short && u64::try_from(*read) == Ok(u64::from(length)))?;
        Some(bytes)
    };
    let picture = part().filter(|_| ours)?;
    let drawn = part().and_then(|drawn| String::from_utf8(drawn).ok())?;
    let picture = PathBuf::from(std::ffi::OsStr::from_bytes(&picture));
    Some((picture, drawn))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PICTURE: &str = "/srv/hills.png";
    const DRAWN: &str = "fill 2x1";

    fn kept() -> Kept {
        Kept {
            size: (2, 1),
            pixels: vec![1, 2, 3, 4, 5, 6, 7, 8],
        }
    }

    fn keep(cache: &Cache, screen: &str) {
        let kept = kept();
        let pixels = (kept.size, kept.pixels.as_slice());
        cache
            .keep(screen, Path::new(PICTURE), DRAWN, pixels)
            .unwrap();
    }

    #[test]
    fn what_a_screen_was_given_is_given_again_if_drawn_from_the_same() {
        let directory = tempfile::TempDir::new().unwrap();
        let cache = Cache::new(Some(directory.path().join("wallpaper")));
        let picture = Path::new(PICTURE);
        assert_eq!(cache.recall("eDP-1", picture, DRAWN), None, "nothing yet");
        keep(&cache, "eDP-1");
        assert_eq!(cache.recall("eDP-1", picture, DRAWN), Some(kept()));
        assert_eq!(cache.picture("eDP-1"), Some(picture.to_owned()));
        // Not drawn from another picture, or as another size, or for
        // another screen.
        assert_eq!(
            cache.recall("eDP-1", Path::new("/srv/sea.png"), DRAWN),
            None
        );
        assert_eq!(cache.recall("eDP-1", picture, "fill 4x2"), None);
        assert_eq!(cache.recall("DP-1", picture, DRAWN), None);
        assert_eq!(cache.picture("DP-1"), None);
        // What was given since takes its place.
        let other = ((1, 1), [9, 9, 9, 9].as_slice());
        cache.keep("eDP-1", picture, "fill 1x1", other).unwrap();
        assert_eq!(cache.recall("eDP-1", picture, DRAWN), None);
        let again = cache.recall("eDP-1", picture, "fill 1x1").unwrap();
        assert_eq!(again.pixels, [9, 9, 9, 9]);
    }

    #[test]
    fn only_the_one_whose_session_it_is_may_read_it() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::TempDir::new().unwrap();
        let within = directory.path().join("omorfos/wallpaper");
        keep(&Cache::new(Some(within.clone())), "eDP-1");
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&within), 0o700);
        assert_eq!(mode(&within.join("eDP-1")), 0o600);
        let names: Vec<_> = fs::read_dir(&within).unwrap().flatten().collect();
        assert_eq!(names.len(), 1, "nothing left half written");
    }

    #[test]
    fn nothing_is_kept_for_a_screen_whose_name_would_be_a_path_or_nowhere() {
        let directory = tempfile::TempDir::new().unwrap();
        let cache = Cache::new(Some(directory.path().to_owned()));
        for name in ["", "..", "a/b", "eDP-1\n"] {
            keep(&cache, name);
            assert_eq!(
                cache.recall(name, Path::new(PICTURE), DRAWN),
                None,
                "{name:?}"
            );
        }
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 0);
        let nowhere = Cache::new(None);
        keep(&nowhere, "eDP-1");
        assert_eq!(nowhere.recall("eDP-1", Path::new(PICTURE), DRAWN), None);
    }

    #[test]
    fn what_cannot_be_kept_says_why() {
        let directory = tempfile::TempDir::new().unwrap();
        let file = directory.path().join("file");
        fs::write(&file, "").unwrap();
        // A directory where a file is.
        let cache = Cache::new(Some(file.join("wallpaper")));
        let pixels = ((1, 1), [0; 4].as_slice());
        assert!(
            cache
                .keep("eDP-1", Path::new(PICTURE), DRAWN, pixels)
                .is_err()
        );
        // A directory where the file is to be put.
        let cache = Cache::new(Some(directory.path().to_owned()));
        fs::create_dir_all(directory.path().join("DP-1/inside")).unwrap();
        assert!(
            cache
                .keep("DP-1", Path::new(PICTURE), DRAWN, pixels)
                .is_err()
        );
        // Its partial file a directory.
        fs::create_dir_all(directory.path().join("DP-2.new")).unwrap();
        assert!(
            cache
                .keep("DP-2", Path::new(PICTURE), DRAWN, pixels)
                .is_err()
        );
    }

    #[test]
    fn a_file_that_does_not_add_up_is_not_shown() {
        let directory = tempfile::TempDir::new().unwrap();
        let cache = Cache::new(Some(directory.path().to_owned()));
        keep(&cache, "eDP-1");
        let path = directory.path().join("eDP-1");
        let whole = fs::read(&path).unwrap();
        let recalled = |bytes: &[u8]| {
            fs::write(&path, bytes).unwrap();
            cache.recall("eDP-1", Path::new(PICTURE), DRAWN)
        };
        assert!(recalled(&whole).is_some());
        // Cut anywhere, or with more after it.
        for length in 0..whole.len() {
            assert_eq!(recalled(&whole[..length]), None, "cut at {length}");
        }
        assert_eq!(recalled(&[whole.as_slice(), &[0]].concat()), None);
        // Of another version.
        let mut changed = whole.clone();
        changed[MAGIC.len() - 2] ^= 1;
        assert_eq!(recalled(&changed), None);
        // Saying it has more pixels than may be, none, or a description
        // longer than may be, or not text.
        let sides = whole.len() - 8 - 8;
        let mut huge = whole.clone();
        huge[sides..sides + 8].copy_from_slice(&[[0, 0, 1, 0], [0, 0, 1, 0]].concat());
        assert_eq!(recalled(&huge), None);
        let mut none = whole.clone();
        none[sides..sides + 8].copy_from_slice(&[0; 8]);
        assert_eq!(recalled(&[&none[..sides + 8]].concat()), None);
        let mut long = whole[..MAGIC.len()].to_vec();
        long.extend((LONGEST + 1).to_le_bytes());
        long.extend(vec![b'/'; usize::try_from(LONGEST).unwrap() + 1]);
        assert_eq!(recalled(&long), None);
        let at = MAGIC.len() + 4 + PICTURE.len() + 4;
        let mut garbled = whole.clone();
        garbled[at] = 0xff;
        assert_eq!(recalled(&garbled), None);
    }
}
