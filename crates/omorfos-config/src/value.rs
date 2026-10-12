//! Understanding the values a stylesheet gives: colours, sizes, numbers.
//!
//! Each function takes a value as written, with nothing around it, and
//! returns what it means or says what is wrong with it.

use crate::Color;
use crate::style::CLEAR;

/// Splits a value into its words. What is in brackets is one word, spaces
/// and all, so that `1px solid rgb(0, 0, 0)` is three.
pub(crate) fn words(value: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let (mut start, mut depth) = (None, 0_usize);
    for (index, character) in value.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
        let apart = character.is_whitespace() && depth == 0;
        match (start, apart) {
            (None, false) => start = Some(index),
            (Some(from), true) => {
                words.extend(value.get(from..index));
                start = None;
            }
            _ => {}
        }
    }
    words.extend(start.and_then(|from| value.get(from..)));
    words
}

/// A colour: `#rgb`, `#rrggbb`, either with opacity as a further digit or
/// pair, `rgb()`, `hsl()`, a name such as `steelblue`, or `transparent`.
pub(crate) fn color(value: &str) -> Result<Color, String> {
    let lowered = value.to_ascii_lowercase();
    let known = hex(&lowered)
        .or_else(|| function(&lowered))
        .or_else(|| named(&lowered));
    known.ok_or_else(|| {
        format!(
            "`{value}` is not a color; write `#rrggbb`, `rgb(r, g, b)`, `hsl(h, s%, l%)`, any \
             of them with opacity, a color's name, or `transparent`"
        )
    })
}

/// `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
fn hex(value: &str) -> Option<Color> {
    let digits = value.strip_prefix('#')?;
    // The short forms give each digit once.
    let doubled: String = digits.chars().flat_map(|digit| [digit, digit]).collect();
    let long = if matches!(digits.len(), 3 | 4) {
        format!("#{doubled}")
    } else {
        value.to_owned()
    };
    long.parse().ok()
}

/// `rgb(...)` or `hsl(...)`, with or without an `a` and a fourth number.
fn function(value: &str) -> Option<Color> {
    let (name, rest) = value.split_once('(')?;
    let inside = rest.strip_suffix(')')?;
    let separators = |c: char| c == ',' || c == '/' || c.is_whitespace();
    let given = inside.split(separators).filter(|part| !part.is_empty());
    let given: Vec<&str> = given.collect();
    let (channels, opacity) = match given.as_slice() {
        [first, second, third] => ([*first, *second, *third], "1"),
        [first, second, third, opacity] => ([*first, *second, *third], *opacity),
        _ => return None,
    };
    let alpha = fraction(opacity)?;
    let [red, green, blue] = match name.trim() {
        "rgb" | "rgba" => rgb(channels)?,
        "hsl" | "hsla" => hsl(channels)?,
        _ => return None,
    };
    Some(Color::rgba(red, green, blue, alpha))
}

/// Three channels, each from 0 to 255 or a percentage.
fn rgb(channels: [&str; 3]) -> Option<[u8; 3]> {
    let channel = |text: &str| match text.strip_suffix('%') {
        Some(percent) => fraction(&format!("{percent}%")),
        None => number(text).filter(|value| *value <= 255.0).map(byte),
    };
    let [red, green, blue] = channels.map(channel);
    Some([red?, green?, blue?])
}

/// A hue in degrees, and a saturation and lightness in percent.
fn hsl([hue, saturation, lightness]: [&str; 3]) -> Option<[u8; 3]> {
    let hue = number(hue.strip_suffix("deg").unwrap_or(hue))?;
    let percent = |text: &str| {
        let value = text.strip_suffix('%').and_then(number);
        value
            .filter(|value| *value <= 100.0)
            .map(|value| value / 100.0)
    };
    let (saturation, lightness) = (percent(saturation)?, percent(lightness)?);
    // The usual conversion: how far each channel is from the lightness
    // depends on where the hue is around the circle.
    let reach = saturation * lightness.min(1.0 - lightness);
    let channel = |offset: f32| {
        let around = (offset + hue / 30.0).rem_euclid(12.0);
        let distance = (around - 3.0).min(9.0 - around).clamp(-1.0, 1.0);
        byte((lightness - reach * distance) * 255.0)
    };
    Some([channel(0.0), channel(8.0), channel(4.0)])
}

/// A number that is not negative, as a stylesheet writes one.
fn number(text: &str) -> Option<f32> {
    let plain = text
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.');
    let value = text.parse::<f32>().ok().filter(|_| plain);
    value.filter(|value| value.is_finite())
}

/// A share of the whole, from `0` to `1` or `0%` to `100%`, as a byte.
pub(crate) fn fraction(text: &str) -> Option<u8> {
    let share = match text.strip_suffix('%') {
        Some(percent) => number(percent).map(|value| value / 100.0),
        None => number(text),
    };
    share
        .filter(|share| *share <= 1.0)
        .map(|share| byte(share * 255.0))
}

/// The nearest byte to a value from 0 to 255.
pub(crate) fn byte(value: f32) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the value is from 0 to 255, and a cast of anything else saturates"
    )]
    let byte = value.round() as u8;
    byte
}

/// A whole number of `unit` from `least` to `most`. The unit may be left
/// off.
pub(crate) fn size(value: &str, unit: &str, least: u32, most: u32) -> Result<u32, String> {
    let digits = value.strip_suffix(unit).unwrap_or(value);
    let number = digits.parse::<u32>().ok();
    let number = number.filter(|number| (least..=most).contains(number));
    number.ok_or_else(|| {
        format!("`{value}` is not a size from {least}{unit} to {most}{unit}, in whole {unit}")
    })
}

/// How long something takes, in milliseconds: `150ms` or `0.2s`, up to
/// `most`.
pub(crate) fn time(value: &str, most: u32) -> Result<u32, String> {
    let millis = match (value.strip_suffix("ms"), value.strip_suffix('s')) {
        (Some(millis), _) => number(millis),
        (None, Some(seconds)) => number(seconds).map(|seconds| seconds * 1000.0),
        // Nothing takes no time whatever it is counted in.
        (None, None) => number(value).filter(|none| *none == 0.0),
    };
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "the value is from nothing to `most`, which is small"
    )]
    let millis = millis
        .filter(|millis| *millis <= most as f32)
        .map(|millis| millis.round() as u32);
    millis.ok_or_else(|| {
        format!("`{value}` is not a time from 0 to {most}ms; write it as `150ms` or `0.2s`")
    })
}

/// A colour by its name.
fn named(name: &str) -> Option<Color> {
    if name == "transparent" {
        return Some(CLEAR);
    }
    let found = NAMED.binary_search_by_key(&name, |(known, _)| known).ok();
    let found = found.and_then(|index| NAMED.get(index));
    found.map(|(_, value)| {
        let [_, red, green, blue] = value.to_be_bytes();
        Color::rgb(red, green, blue)
    })
}

/// The colours CSS has names for, in alphabetical order.
const NAMED: [(&str, u32); 148] = [
    ("aliceblue", 0x00f0_f8ff),
    ("antiquewhite", 0x00fa_ebd7),
    ("aqua", 0x0000_ffff),
    ("aquamarine", 0x007f_ffd4),
    ("azure", 0x00f0_ffff),
    ("beige", 0x00f5_f5dc),
    ("bisque", 0x00ff_e4c4),
    ("black", 0x0000_0000),
    ("blanchedalmond", 0x00ff_ebcd),
    ("blue", 0x0000_00ff),
    ("blueviolet", 0x008a_2be2),
    ("brown", 0x00a5_2a2a),
    ("burlywood", 0x00de_b887),
    ("cadetblue", 0x005f_9ea0),
    ("chartreuse", 0x007f_ff00),
    ("chocolate", 0x00d2_691e),
    ("coral", 0x00ff_7f50),
    ("cornflowerblue", 0x0064_95ed),
    ("cornsilk", 0x00ff_f8dc),
    ("crimson", 0x00dc_143c),
    ("cyan", 0x0000_ffff),
    ("darkblue", 0x0000_008b),
    ("darkcyan", 0x0000_8b8b),
    ("darkgoldenrod", 0x00b8_860b),
    ("darkgray", 0x00a9_a9a9),
    ("darkgreen", 0x0000_6400),
    ("darkgrey", 0x00a9_a9a9),
    ("darkkhaki", 0x00bd_b76b),
    ("darkmagenta", 0x008b_008b),
    ("darkolivegreen", 0x0055_6b2f),
    ("darkorange", 0x00ff_8c00),
    ("darkorchid", 0x0099_32cc),
    ("darkred", 0x008b_0000),
    ("darksalmon", 0x00e9_967a),
    ("darkseagreen", 0x008f_bc8f),
    ("darkslateblue", 0x0048_3d8b),
    ("darkslategray", 0x002f_4f4f),
    ("darkslategrey", 0x002f_4f4f),
    ("darkturquoise", 0x0000_ced1),
    ("darkviolet", 0x0094_00d3),
    ("deeppink", 0x00ff_1493),
    ("deepskyblue", 0x0000_bfff),
    ("dimgray", 0x0069_6969),
    ("dimgrey", 0x0069_6969),
    ("dodgerblue", 0x001e_90ff),
    ("firebrick", 0x00b2_2222),
    ("floralwhite", 0x00ff_faf0),
    ("forestgreen", 0x0022_8b22),
    ("fuchsia", 0x00ff_00ff),
    ("gainsboro", 0x00dc_dcdc),
    ("ghostwhite", 0x00f8_f8ff),
    ("gold", 0x00ff_d700),
    ("goldenrod", 0x00da_a520),
    ("gray", 0x0080_8080),
    ("green", 0x0000_8000),
    ("greenyellow", 0x00ad_ff2f),
    ("grey", 0x0080_8080),
    ("honeydew", 0x00f0_fff0),
    ("hotpink", 0x00ff_69b4),
    ("indianred", 0x00cd_5c5c),
    ("indigo", 0x004b_0082),
    ("ivory", 0x00ff_fff0),
    ("khaki", 0x00f0_e68c),
    ("lavender", 0x00e6_e6fa),
    ("lavenderblush", 0x00ff_f0f5),
    ("lawngreen", 0x007c_fc00),
    ("lemonchiffon", 0x00ff_facd),
    ("lightblue", 0x00ad_d8e6),
    ("lightcoral", 0x00f0_8080),
    ("lightcyan", 0x00e0_ffff),
    ("lightgoldenrodyellow", 0x00fa_fad2),
    ("lightgray", 0x00d3_d3d3),
    ("lightgreen", 0x0090_ee90),
    ("lightgrey", 0x00d3_d3d3),
    ("lightpink", 0x00ff_b6c1),
    ("lightsalmon", 0x00ff_a07a),
    ("lightseagreen", 0x0020_b2aa),
    ("lightskyblue", 0x0087_cefa),
    ("lightslategray", 0x0077_8899),
    ("lightslategrey", 0x0077_8899),
    ("lightsteelblue", 0x00b0_c4de),
    ("lightyellow", 0x00ff_ffe0),
    ("lime", 0x0000_ff00),
    ("limegreen", 0x0032_cd32),
    ("linen", 0x00fa_f0e6),
    ("magenta", 0x00ff_00ff),
    ("maroon", 0x0080_0000),
    ("mediumaquamarine", 0x0066_cdaa),
    ("mediumblue", 0x0000_00cd),
    ("mediumorchid", 0x00ba_55d3),
    ("mediumpurple", 0x0093_70db),
    ("mediumseagreen", 0x003c_b371),
    ("mediumslateblue", 0x007b_68ee),
    ("mediumspringgreen", 0x0000_fa9a),
    ("mediumturquoise", 0x0048_d1cc),
    ("mediumvioletred", 0x00c7_1585),
    ("midnightblue", 0x0019_1970),
    ("mintcream", 0x00f5_fffa),
    ("mistyrose", 0x00ff_e4e1),
    ("moccasin", 0x00ff_e4b5),
    ("navajowhite", 0x00ff_dead),
    ("navy", 0x0000_0080),
    ("oldlace", 0x00fd_f5e6),
    ("olive", 0x0080_8000),
    ("olivedrab", 0x006b_8e23),
    ("orange", 0x00ff_a500),
    ("orangered", 0x00ff_4500),
    ("orchid", 0x00da_70d6),
    ("palegoldenrod", 0x00ee_e8aa),
    ("palegreen", 0x0098_fb98),
    ("paleturquoise", 0x00af_eeee),
    ("palevioletred", 0x00db_7093),
    ("papayawhip", 0x00ff_efd5),
    ("peachpuff", 0x00ff_dab9),
    ("peru", 0x00cd_853f),
    ("pink", 0x00ff_c0cb),
    ("plum", 0x00dd_a0dd),
    ("powderblue", 0x00b0_e0e6),
    ("purple", 0x0080_0080),
    ("rebeccapurple", 0x0066_3399),
    ("red", 0x00ff_0000),
    ("rosybrown", 0x00bc_8f8f),
    ("royalblue", 0x0041_69e1),
    ("saddlebrown", 0x008b_4513),
    ("salmon", 0x00fa_8072),
    ("sandybrown", 0x00f4_a460),
    ("seagreen", 0x002e_8b57),
    ("seashell", 0x00ff_f5ee),
    ("sienna", 0x00a0_522d),
    ("silver", 0x00c0_c0c0),
    ("skyblue", 0x0087_ceeb),
    ("slateblue", 0x006a_5acd),
    ("slategray", 0x0070_8090),
    ("slategrey", 0x0070_8090),
    ("snow", 0x00ff_fafa),
    ("springgreen", 0x0000_ff7f),
    ("steelblue", 0x0046_82b4),
    ("tan", 0x00d2_b48c),
    ("teal", 0x0000_8080),
    ("thistle", 0x00d8_bfd8),
    ("tomato", 0x00ff_6347),
    ("turquoise", 0x0040_e0d0),
    ("violet", 0x00ee_82ee),
    ("wheat", 0x00f5_deb3),
    ("white", 0x00ff_ffff),
    ("whitesmoke", 0x00f5_f5f5),
    ("yellow", 0x00ff_ff00),
    ("yellowgreen", 0x009a_cd32),
];

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);

    #[test]
    fn a_value_is_split_into_words_keeping_what_is_in_brackets_together() {
        assert_eq!(words("1px solid #fff"), ["1px", "solid", "#fff"]);
        assert_eq!(
            words("  2px   rgb(1, 2, 3)  solid "),
            ["2px", "rgb(1, 2, 3)", "solid"]
        );
        assert_eq!(words("rgb( 1 2 3 / 50% )"), ["rgb( 1 2 3 / 50% )"]);
        assert_eq!(words(""), [""; 0]);
        assert_eq!(words("   "), [""; 0]);
        // A stray bracket does not swallow the rest.
        assert_eq!(words(") a b"), [")", "a", "b"]);
    }

    #[test]
    fn colours_may_be_written_in_hexadecimal_short_or_long() {
        assert_eq!(color("#f00"), Ok(RED));
        assert_eq!(color("#F00"), Ok(RED));
        assert_eq!(color("#ff0000"), Ok(RED));
        assert_eq!(color("#f008"), Ok(Color::rgba(255, 0, 0, 0x88)));
        assert_eq!(color("#ff000080"), Ok(Color::rgba(255, 0, 0, 0x80)));
        for wrong in ["#ff", "#ff000", "#gggggg", "ff0000", "#", ""] {
            let message = color(wrong).unwrap_err();
            assert!(message.contains("is not a color"), "{wrong}: {message}");
        }
    }

    #[test]
    fn colours_may_be_written_as_red_green_and_blue() {
        assert_eq!(color("rgb(255, 0, 0)"), Ok(RED));
        assert_eq!(color("rgb(255 0 0)"), Ok(RED));
        assert_eq!(color("RGB(100%, 0%, 0%)"), Ok(RED));
        assert_eq!(
            color("rgba(255, 0, 0, 0.5)"),
            Ok(Color::rgba(255, 0, 0, 128))
        );
        assert_eq!(color("rgba(255,0,0,.5)"), Ok(Color::rgba(255, 0, 0, 128)));
        assert_eq!(color("rgb(255 0 0 / 50%)"), Ok(Color::rgba(255, 0, 0, 128)));
        assert_eq!(color("rgba(20, 22, 27, 1)"), Ok(Color::rgb(20, 22, 27)));
        assert_eq!(color("rgb( 0 , 0 , 0 , 0 )"), Ok(Color::rgba(0, 0, 0, 0)));
        for wrong in [
            "rgb(255, 0)",
            "rgb(1, 2, 3, 4, 5)",
            "rgb(256, 0, 0)",
            "rgb(-1, 0, 0)",
            "rgb(red, 0, 0)",
            "rgb(0, 0, 0, 2)",
            "rgb(0, 0, 0, 200%)",
            "rgb(0, 0, 150%)",
            "rgb(0, 0, 0",
            "rgb(0, nan, 0)",
            "rgb(1e2, 0, 0)",
            "cmyk(0, 0, 0)",
        ] {
            assert!(color(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn colours_may_be_written_as_hue_saturation_and_lightness() {
        assert_eq!(color("hsl(0, 100%, 50%)"), Ok(RED));
        assert_eq!(color("hsl(120deg 100% 50%)"), Ok(Color::rgb(0, 255, 0)));
        assert_eq!(color("hsl(240, 100%, 50%)"), Ok(Color::rgb(0, 0, 255)));
        assert_eq!(color("hsl(360, 100%, 50%)"), Ok(RED), "all the way round");
        assert_eq!(color("hsl(0, 0%, 100%)"), Ok(Color::rgb(255, 255, 255)));
        assert_eq!(color("hsl(0, 0%, 0%)"), Ok(Color::rgb(0, 0, 0)));
        assert_eq!(color("hsl(210, 50%, 40%)"), Ok(Color::rgb(51, 102, 153)));
        assert_eq!(
            color("hsla(0, 100%, 50%, 0.25)"),
            Ok(Color::rgba(255, 0, 0, 64))
        );
        for wrong in [
            "hsl(0, 100, 50)",
            "hsl(0, 101%, 50%)",
            "hsl(0, 100%, 150%)",
            "hsl(red, 100%, 50%)",
            "hsl(0, 100%)",
        ] {
            assert!(color(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn colours_may_be_called_by_name() {
        assert_eq!(color("red"), Ok(RED));
        assert_eq!(color("Red"), Ok(RED));
        assert_eq!(color("steelblue"), Ok(Color::rgb(0x46, 0x82, 0xb4)));
        assert_eq!(color("aliceblue"), Ok(Color::rgb(0xf0, 0xf8, 0xff)));
        assert_eq!(color("yellowgreen"), Ok(Color::rgb(0x9a, 0xcd, 0x32)));
        assert_eq!(color("black"), Ok(Color::rgb(0, 0, 0)));
        assert_eq!(color("transparent"), Ok(CLEAR));
        assert!(color("banana").is_err());
        // The names can only be found if they are in order.
        assert!(NAMED.is_sorted_by_key(|(name, _)| name));
    }

    #[test]
    fn a_share_is_a_number_up_to_one_or_a_percentage() {
        assert_eq!(fraction("0"), Some(0));
        assert_eq!(fraction("1"), Some(255));
        assert_eq!(fraction("0.5"), Some(128));
        assert_eq!(fraction("50%"), Some(128));
        assert_eq!(fraction("100%"), Some(255));
        for wrong in ["1.5", "150%", "-0.5", "half", "", "%", "0.5.5", "inf"] {
            assert_eq!(fraction(wrong), None, "{wrong}");
        }
    }

    #[test]
    fn a_time_is_written_in_seconds_or_thousandths_of_one() {
        assert_eq!(time("150ms", 10_000), Ok(150));
        assert_eq!(time("0.2s", 10_000), Ok(200));
        assert_eq!(time("2s", 10_000), Ok(2000));
        assert_eq!(time("0", 10_000), Ok(0));
        assert_eq!(time("0s", 10_000), Ok(0));
        assert_eq!(time("10000ms", 10_000), Ok(10_000));
        for wrong in ["150", "11s", "10001ms", "-1s", "fast", "", "s", "ms", "1m"] {
            let message = time(wrong, 10_000).unwrap_err();
            assert!(message.contains("from 0 to 10000ms"), "{wrong}: {message}");
        }
    }

    #[test]
    fn a_size_is_a_whole_number_within_bounds_with_or_without_its_unit() {
        assert_eq!(size("4px", "px", 0, 16), Ok(4));
        assert_eq!(size("4", "px", 0, 16), Ok(4));
        assert_eq!(size("0", "px", 0, 16), Ok(0));
        assert_eq!(size("12pt", "pt", 6, 72), Ok(12));
        for wrong in ["17px", "1.5px", "2em", "-2px", "px", ""] {
            let message = size(wrong, "px", 0, 16).unwrap_err();
            assert!(message.contains("from 0px to 16px"), "{wrong}: {message}");
        }
    }
}
