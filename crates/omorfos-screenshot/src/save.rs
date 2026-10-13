//! Where pictures are kept, and writing them there.

use std::fs::File;
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use crate::Shot;

/// The folder the user keeps pictures in: what `user-dirs.dirs` says, if
/// it was read and says, or `Pictures` in the home folder.
///
/// That file is written by `xdg-user-dirs` in the user's language, as
/// lines such as `XDG_PICTURES_DIR="$HOME/Bilder"`.
#[must_use]
pub fn pictures(told: Option<&str>, home: &Path) -> PathBuf {
    let lines = told.unwrap_or_default().lines();
    let mut said = lines.filter_map(|line| line.trim().strip_prefix("XDG_PICTURES_DIR="));
    let said = said.next_back().map(|value| value.trim_matches('"'));
    let under_home = said.and_then(|value| value.strip_prefix("$HOME/"));
    let elsewhere = said.filter(|value| value.starts_with('/'));
    let known = under_home.map(|rest| home.join(rest));
    let known = known.or_else(|| elsewhere.map(PathBuf::from));
    known.unwrap_or_else(|| home.join("Pictures"))
}

/// What a picture is called: when it was taken, the screen if that is to
/// be said, and a number if there is one of that name already.
fn name(stamp: &str, screen: Option<&str>, again: u32) -> String {
    let screen = screen
        .map(|screen| format!(" {screen}"))
        .unwrap_or_default();
    let again = if again > 1 {
        format!(" ({again})")
    } else {
        String::new()
    };
    format!("Screenshot {stamp}{screen}{again}.png")
}

/// Saves a picture in `folder` as a PNG named after `stamp`, and after
/// `screen` if one is given, and returns where. A picture already there
/// is never written over: the new one is numbered.
///
/// # Errors
///
/// Returns an error if the file cannot be made or written.
pub fn save(folder: &Path, stamp: &str, screen: Option<&str>, shot: &Shot) -> io::Result<PathBuf> {
    let candidates = (1..=u32::MAX).map(|again| folder.join(name(stamp, screen, again)));
    let mut candidates = candidates;
    // Were every number taken, there would be no name, and making a file
    // of no name fails.
    let path = candidates.find(|path| !path.exists()).unwrap_or_default();
    let file = File::create_new(&path);
    let written = file.and_then(|file| {
        let mut encoder = png::Encoder::new(BufWriter::new(file), shot.width, shot.height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let begun = encoder.write_header();
        let done = begun.and_then(|mut writer| writer.write_image_data(&shot.pixels));
        done.map_err(io::Error::other)
    });
    written.map(|()| path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_are_kept_where_the_user_keeps_pictures() {
        let home = Path::new("/home/ada");
        let told = "# written by xdg-user-dirs\nXDG_DESKTOP_DIR=\"$HOME/Schreibtisch\"\n\
                    XDG_PICTURES_DIR=\"$HOME/Bilder\"\n";
        assert_eq!(pictures(Some(told), home), home.join("Bilder"));
        let elsewhere = "XDG_PICTURES_DIR=\"/data/pictures\"\n";
        assert_eq!(pictures(Some(elsewhere), home), Path::new("/data/pictures"));
    }

    #[test]
    fn pictures_go_in_the_home_folder_when_nothing_says_where() {
        let home = Path::new("/home/ada");
        assert_eq!(pictures(None, home), home.join("Pictures"));
        assert_eq!(
            pictures(Some("XDG_MUSIC_DIR=\"$HOME/Music\"\n"), home),
            home.join("Pictures")
        );
        // What is neither under the home folder nor a whole path says
        // nothing that can be used.
        assert_eq!(
            pictures(Some("XDG_PICTURES_DIR=\"Pictures\"\n"), home),
            home.join("Pictures")
        );
    }

    #[test]
    fn a_picture_is_named_after_when_and_of_what_it_was_taken() {
        assert_eq!(
            name("2026-10-13 14-03-22", None, 1),
            "Screenshot 2026-10-13 14-03-22.png"
        );
        assert_eq!(
            name("2026-10-13 14-03-22", Some("DP-1"), 1),
            "Screenshot 2026-10-13 14-03-22 DP-1.png"
        );
        assert_eq!(
            name("2026-10-13 14-03-22", Some("DP-1"), 3),
            "Screenshot 2026-10-13 14-03-22 DP-1 (3).png"
        );
    }

    #[test]
    fn a_picture_is_saved_as_a_png_and_never_over_another() {
        let folder = tempfile::TempDir::new().unwrap();
        let shot = Shot {
            screen: "one".to_owned(),
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 0, 0, 255],
        };
        let first = save(folder.path(), "now", None, &shot).unwrap();
        let second = save(folder.path(), "now", None, &shot).unwrap();
        assert_eq!(first, folder.path().join("Screenshot now.png"));
        assert_eq!(second, folder.path().join("Screenshot now (2).png"));

        let decoder = png::Decoder::new(io::BufReader::new(File::open(&first).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(pixels[..6], [255, 0, 0, 0, 0, 255]);
    }

    #[test]
    fn a_picture_that_cannot_be_saved_is_an_error() {
        let folder = tempfile::TempDir::new().unwrap();
        let shot = Shot {
            screen: "one".to_owned(),
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0],
        };
        assert!(save(&folder.path().join("missing"), "now", None, &shot).is_err());
    }
}
