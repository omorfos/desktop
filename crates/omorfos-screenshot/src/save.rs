//! Where pictures are kept, and writing them there.

use std::fs::File;
use std::io::{self, BufWriter, Write};
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
    let written = file.and_then(|file| encode(shot, BufWriter::new(file)));
    written.map(|()| path)
}

/// Writes a picture as a PNG.
///
/// # Errors
///
/// Returns an error if it cannot be written, or if there is nothing of
/// it: a PNG has at least one pixel.
pub fn encode(shot: &Shot, to: impl Write) -> io::Result<()> {
    let mut encoder = png::Encoder::new(to, shot.width, shot.height);
    encoder.set_depth(png::BitDepth::Eight);
    // A picture that is see-through somewhere is kept so.
    let see_through = shot.alpha.as_ref().map(|alpha| {
        let colours = shot.pixels.as_chunks::<3>().0.iter();
        let pixels = colours
            .zip(alpha)
            .flat_map(|([red, green, blue], alpha)| [*red, *green, *blue, *alpha]);
        pixels.collect::<Vec<u8>>()
    });
    let colour = if see_through.is_some() {
        png::ColorType::Rgba
    } else {
        png::ColorType::Rgb
    };
    encoder.set_color(colour);
    let pixels = see_through.as_deref().unwrap_or(&shot.pixels);
    let begun = encoder.write_header();
    let done = begun.and_then(|mut writer| writer.write_image_data(pixels));
    done.map_err(io::Error::other)
}

/// One picture of several screens, side by side in the order given, each
/// from the top, on black where one is shorter than another. Screens are
/// solid all over.
#[must_use]
pub fn together(shots: &[Shot]) -> Shot {
    let width: u32 = shots.iter().map(|shot| shot.width).sum();
    let height = shots
        .iter()
        .map(|shot| shot.height)
        .max()
        .unwrap_or_default();
    let mut pixels = Vec::new();
    for row in 0..height {
        for shot in shots {
            let across = usize::try_from(shot.width).unwrap_or_default() * 3;
            let from = usize::try_from(row).unwrap_or_default() * across;
            let part = shot.pixels.get(from..from + across);
            // Below the end of a shorter screen there is nothing.
            pixels.extend(part.map_or(vec![0; across], <[u8]>::to_vec));
        }
    }
    let names: Vec<&str> = shots.iter().map(|shot| shot.screen.as_str()).collect();
    Shot {
        screen: names.join("+"),
        width,
        height,
        pixels,
        alpha: None,
    }
}

/// The part of a picture in `area`, as far as it is in the picture.
#[must_use]
pub fn crop(shot: &Shot, area: omorfos_picker::Area) -> Shot {
    let x = area.x.min(shot.width);
    let y = area.y.min(shot.height);
    let width = area.width.min(shot.width - x);
    let height = area.height.min(shot.height - y);
    let row = |at: u32| usize::try_from(at).unwrap_or_default();
    let mut pixels = Vec::new();
    for down in y..y + height {
        let from = (row(down) * row(shot.width) + row(x)) * 3;
        let part = shot.pixels.get(from..from + row(width) * 3);
        pixels.extend(part.unwrap_or_default());
    }
    Shot {
        screen: shot.screen.clone(),
        width,
        height,
        pixels,
        alpha: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_is_cut_from_its_picture_as_far_as_it_is_in_it() {
        // Three pixels across and two down, each its own colour.
        let shot = Shot {
            screen: "DP-1".to_owned(),
            width: 3,
            height: 2,
            pixels: (0..18).collect(),
            alpha: None,
        };
        let area = |x, y, width, height| omorfos_picker::Area {
            x,
            y,
            width,
            height,
        };
        let cut = crop(&shot, area(1, 0, 2, 2));
        assert_eq!((cut.width, cut.height), (2, 2));
        assert_eq!(cut.pixels, [3, 4, 5, 6, 7, 8, 12, 13, 14, 15, 16, 17]);
        let past = crop(&shot, area(2, 1, 10, 10));
        assert_eq!(
            (past.width, past.height, past.pixels),
            (1, 1, vec![15, 16, 17])
        );
        // Nor from pixels the picture says it has and has not.
        let truncated = Shot {
            pixels: vec![1, 2, 3],
            ..shot
        };
        assert_eq!(crop(&truncated, area(0, 1, 1, 1)).pixels, Vec::<u8>::new());
    }

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
            alpha: None,
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
    fn a_see_through_picture_is_saved_see_through() {
        let shot = Shot {
            screen: "window".to_owned(),
            width: 2,
            height: 1,
            pixels: vec![255, 0, 0, 0, 0, 255],
            alpha: Some(vec![128, 255]),
        };
        let mut png = Vec::new();
        encode(&shot, &mut png).unwrap();
        let mut reader = png::Decoder::new(io::Cursor::new(png)).read_info().unwrap();
        let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut pixels).unwrap();
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(pixels[..8], [255, 0, 0, 128, 0, 0, 255, 255]);
    }

    #[test]
    fn several_screens_make_one_picture_side_by_side() {
        let red = [255, 0, 0];
        let blue = [0, 0, 255];
        let tall = Shot {
            screen: "left".to_owned(),
            width: 1,
            height: 2,
            pixels: [red, red].concat(),
            alpha: None,
        };
        let short = Shot {
            screen: "right".to_owned(),
            width: 2,
            height: 1,
            pixels: [blue, blue].concat(),
            alpha: None,
        };
        let both = together(&[tall.clone(), short]);
        assert_eq!((both.width, both.height), (3, 2));
        assert_eq!(both.screen, "left+right");
        let black = [0, 0, 0];
        assert_eq!(both.pixels, [red, blue, blue, red, black, black].concat());

        assert_eq!(together(std::slice::from_ref(&tall)), tall);
        let none = together(&[]);
        assert_eq!((none.width, none.height), (0, 0));
        assert!(encode(&none, Vec::new()).is_err(), "nothing is no picture");
    }

    #[test]
    fn a_picture_that_cannot_be_saved_is_an_error() {
        let folder = tempfile::TempDir::new().unwrap();
        let shot = Shot {
            screen: "one".to_owned(),
            width: 1,
            height: 1,
            pixels: vec![0, 0, 0],
            alpha: None,
        };
        assert!(save(&folder.path().join("missing"), "now", None, &shot).is_err());
    }
}
