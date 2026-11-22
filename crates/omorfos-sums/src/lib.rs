//! Sums, worked out as they are typed: in the launcher, and in the
//! calculator that is to come (ADR 86), so that a sum comes out the same
//! in each.
//!
//! What is typed is read here and nowhere else: numbers, written as where
//! the user is writes them ([`Numbers`]), `+`, `-`, `*` or `×`, `/` or `÷`,
//! `^`, `%`, brackets, the functions `sqrt` or `√`, `sin`, `cos`, `tan`, `ln`
//! and `log`, and the constants `pi` or `π` and `e`. Nothing typed is ever
//! run.

use std::f64::consts::{E, PI};
use std::iter::Peekable;
use std::str::Chars;

/// The deepest that brackets, signs, powers and roots may stand one inside
/// another. The launcher holds no more characters than this, so it is only
/// reached by what is typed to overflow the stack.
const DEEPEST: usize = 64;

/// How many figures of an answer are kept: as many as an `f64` holds
/// truly, so that `0.1+0.2` is `0.3`.
const FIGURES: usize = 15;

/// The signs a sum not yet finished may end with, a bracket or a root
/// opened, and the decimals or a thousand begun with no figure after them.
const UNFINISHED: [char; 11] = ['+', '-', '*', '×', '/', '÷', '^', '(', '√', ',', '.'];

/// The functions written in letters, which need something after them,
/// as `sin 30` or `sin(30)`: angles are in degrees, as on a desktop's
/// calculator, and `log` is of ten.
const FUNCTIONS: [(&str, Function); 6] = [
    ("sqrt", f64::sqrt),
    ("sin", sine),
    ("cos", cosine),
    ("tan", tangent),
    ("ln", f64::ln),
    ("log", f64::log10),
];

/// What a function does to the number after it.
type Function = fn(f64) -> f64;

/// The constants written in letters, which stand alone.
const CONSTANTS: [(&str, f64); 2] = [("pi", PI), ("e", E)];

/// The sine of an angle in degrees, to 15 places, so that what is nothing
/// comes out as nothing: `sin 180` is 0, not what is left over.
fn sine(degrees: f64) -> f64 {
    places(degrees.to_radians().sin())
}

/// The cosine of an angle in degrees, as [`sine`].
fn cosine(degrees: f64) -> f64 {
    places(degrees.to_radians().cos())
}

/// The tangent of an angle in degrees, as [`sine`]: none where its
/// cosine is nothing, as `tan 90`, which divides by it.
fn tangent(degrees: f64) -> f64 {
    sine(degrees) / cosine(degrees)
}

/// `value` rounded to 15 places after the point.
fn places(value: f64) -> f64 {
    (value * 1e15).round() / 1e15
}

/// The languages that write a comma for the decimals, by their codes, as
/// the Unicode CLDR has them, but for the places in [`POINT_PLACES`].
const COMMA_LANGUAGES: [&str; 47] = [
    "af", "az", "be", "bg", "bs", "ca", "cs", "da", "de", "el", "es", "et", "eu", "fi", "fo", "fr",
    "gl", "hr", "hu", "hy", "id", "is", "it", "ka", "kk", "ky", "lb", "lt", "lv", "mk", "nb", "nl",
    "nn", "no", "pl", "pt", "ro", "ru", "sk", "sl", "sq", "sr", "sv", "tr", "uk", "uz", "vi",
];

/// The places where a language of [`COMMA_LANGUAGES`] writes a point.
const POINT_PLACES: [&str; 13] = [
    "de_ch", "de_li", "it_ch", "es_do", "es_gt", "es_hn", "es_mx", "es_ni", "es_pa", "es_pe",
    "es_pr", "es_sv", "es_us",
];

/// How numbers are written where the user is: what marks the decimals, and
/// what may stand between thousands. Something between thousands is read
/// only where three figures follow it, and before the decimals, so that
/// `2 3` is no number; an answer is written with the decimals marked as
/// they are read, and nothing between its thousands, to be copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbers {
    /// As in English, `1,234.5`: a point for the decimals, and a comma or
    /// a space between thousands.
    Point,
    /// As in Polish or German, `1 234,5` or `1.234,5`: a comma for the
    /// decimals, and a space or a point between thousands. A point not
    /// before three figures marks the decimals too, as a keypad's does.
    Comma,
}

impl Numbers {
    /// How numbers are written in the locale `named`, as `pl_PL.UTF-8`:
    /// with a point where it is not known to write a comma.
    #[must_use]
    pub fn of_locale(named: &str) -> Self {
        let place = named.split(['.', '@']).next().unwrap_or_default();
        let place = place.to_lowercase().replace('-', "_");
        let language = place.split('_').next().unwrap_or_default();
        let comma = COMMA_LANGUAGES.contains(&language) && !POINT_PLACES.contains(&place.as_str());
        if comma { Self::Comma } else { Self::Point }
    }

    /// Whether `sign` marks the decimals.
    fn decimal(self, sign: char) -> bool {
        sign == '.' || (self == Self::Comma && sign == ',')
    }

    /// Whether `sign` may stand between thousands.
    fn between(self, sign: char) -> bool {
        let other = match self {
            Self::Point => ',',
            Self::Comma => '.',
        };
        sign.is_whitespace() || sign == other
    }
}

/// The answer to what is typed, written as a person would write it where
/// numbers are written as `numbers`, if it is a sum, with something worked
/// out and a number for an answer. A number or a constant alone is no sum.
/// A sum not yet finished, ending in a sign, a word begun or with a
/// bracket open, is answered as far as it goes, so that the answer stays
/// as the next sign is typed: `60 + 60 +` is `120`.
#[must_use]
pub fn answer(typed: &str, numbers: Numbers) -> Option<String> {
    whole(typed, numbers).or_else(|| whole(&finished(typed), numbers))
}

/// What is typed with what is not yet finished at its end taken off, and
/// the brackets left open closed.
fn finished(typed: &str) -> String {
    let mut begun = typed;
    loop {
        let signed = begun.trim_end_matches(|c: char| c.is_whitespace() || UNFINISHED.contains(&c));
        let before = signed.trim_end_matches(|c: char| c.is_alphabetic());
        let word = &signed[before.len()..];
        let trimmed = if unfinished(word) { before } else { signed };
        if trimmed.len() == begun.len() {
            break;
        }
        begun = trimmed;
    }
    let open = begun.matches('(').count();
    let open = open.saturating_sub(begun.matches(')').count());
    format!("{begun}{}", ")".repeat(open))
}

/// Whether `word` is the beginning of a word that stands for something,
/// or a function with nothing after it yet.
fn unfinished(word: &str) -> bool {
    let begins = |whole: &str| {
        let start = whole.get(..word.len());
        start.is_some_and(|start| start.eq_ignore_ascii_case(word))
    };
    let mut functions = FUNCTIONS.iter().map(|(name, _)| *name);
    let mut constants = CONSTANTS.iter().map(|(name, _)| *name);
    let function = functions.any(begins);
    let constant = constants.any(|name| begins(name) && word.len() < name.len());
    !word.is_empty() && (function || constant)
}

/// The answer to what is typed, if it is a whole sum.
fn whole(typed: &str, numbers: Numbers) -> Option<String> {
    let mut reading = Reading {
        rest: typed.chars().peekable(),
        numbers,
        depth: 0,
        worked: false,
    };
    let value = reading.sum()?;
    let whole = reading.next().is_none();
    (whole && reading.worked && value.is_finite()).then(|| written(value, numbers))
}

/// An answer, rounded to [`FIGURES`] figures, without a sign if it is
/// nothing, with a power of ten if it is very large or very small, and its
/// decimals marked as `numbers` mark them.
fn written(value: f64, numbers: Numbers) -> String {
    let rounded: f64 = format!("{value:.*e}", FIGURES - 1).parse().unwrap_or(value);
    // Nothing is written without a sign, however it came.
    let rounded = rounded + 0.0;
    let size = rounded.abs();
    let written = if size != 0.0 && !(1e-9..1e15).contains(&size) {
        format!("{rounded:e}")
    } else {
        format!("{rounded}")
    };
    match numbers {
        Numbers::Point => written,
        Numbers::Comma => written.replace('.', ","),
    }
}

/// What is left to read of what was typed.
struct Reading<'a> {
    rest: Peekable<Chars<'a>>,
    /// How numbers are written in it.
    numbers: Numbers,
    /// How deep inside one another what is being read stands.
    depth: usize,
    /// Whether something has been worked out, which makes it a sum.
    worked: bool,
}

impl Reading<'_> {
    /// The next character that is not a space, taken.
    fn next(&mut self) -> Option<char> {
        self.skip_spaces();
        self.rest.next()
    }

    /// The next character that is not a space, left where it is.
    fn peek(&mut self) -> Option<char> {
        self.skip_spaces();
        self.rest.peek().copied()
    }

    fn skip_spaces(&mut self) {
        while self
            .rest
            .next_if(|character| character.is_whitespace())
            .is_some()
        {}
    }

    /// Takes the next character if it is one of `these`.
    fn take(&mut self, these: &[char]) -> Option<char> {
        let next = self.peek().filter(|character| these.contains(character));
        next.and_then(|_| self.rest.next())
    }

    /// Terms added or taken away, from the left. A percentage alone is
    /// of what it is added to or taken from: `200+10%` is `220`.
    fn sum(&mut self) -> Option<f64> {
        let (mut value, _) = self.product()?;
        while let Some(sign) = self.take(&['+', '-']) {
            let (term, share) = self.product()?;
            let term = if share { value * term } else { term };
            value = if sign == '+' {
                value + term
            } else {
                value - term
            };
            self.worked = true;
        }
        Some(value)
    }

    /// Factors multiplied or divided, from the left, and whether it was a
    /// percentage alone.
    fn product(&mut self) -> Option<(f64, bool)> {
        let mut value = self.signed()?;
        let mut share = self.percent(&mut value);
        while let Some(sign) = self.take(&['*', '×', '/', '÷']) {
            let mut factor = self.signed()?;
            self.percent(&mut factor);
            value = if matches!(sign, '*' | '×') {
                value * factor
            } else {
                value / factor
            };
            share = false;
            self.worked = true;
        }
        Some((value, share))
    }

    /// Makes `value` a hundredth of itself if `%` follows it, and says
    /// whether it did.
    fn percent(&mut self, value: &mut f64) -> bool {
        let share = self.take(&['%']).is_some();
        if share {
            *value /= 100.0;
            self.worked = true;
        }
        share
    }

    /// A power, with as many signs before it as there are: `-2^2` is
    /// `-4`, as it is written by hand.
    fn signed(&mut self) -> Option<f64> {
        self.deeper(|reading| {
            if let Some(sign) = reading.take(&['+', '-']) {
                let value = reading.signed();
                value.map(|value| if sign == '-' { -value } else { value })
            } else {
                reading.power()
            }
        })
    }

    /// What `read` reads, one deeper, if that is not too deep.
    fn deeper(&mut self, read: impl FnOnce(&mut Self) -> Option<f64>) -> Option<f64> {
        self.depth += 1;
        let value = if self.depth > DEEPEST {
            None
        } else {
            read(self)
        };
        self.depth -= 1;
        value
    }

    /// A number, perhaps raised to a power, from the right: `2^3^2` is
    /// `2^9`.
    fn power(&mut self) -> Option<f64> {
        let base = self.number()?;
        if self.take(&['^']).is_some() {
            let exponent = self.signed()?;
            self.worked = true;
            Some(base.powf(exponent))
        } else {
            Some(base)
        }
    }

    /// A number written out, a constant, a function of what follows, or a
    /// sum in brackets.
    fn number(&mut self) -> Option<f64> {
        if self.take(&['(']).is_some() {
            let value = self.sum()?;
            return self.take(&[')']).map(|_| value);
        }
        if self.take(&['√']).is_some() {
            return self.apply(f64::sqrt);
        }
        if self.take(&['π']).is_some() {
            return Some(PI);
        }
        let mut word = self.rest.clone();
        let letters = word.by_ref().take_while(char::is_ascii_alphabetic).count();
        if letters > 0 {
            let spelt: String = self.rest.by_ref().take(letters).collect();
            let named = |name: &&str| spelt.eq_ignore_ascii_case(name);
            let function = FUNCTIONS.iter().find(|(name, _)| named(name));
            if let Some((_, function)) = function {
                return self.apply(*function);
            }
            let constant = CONSTANTS.iter().find(|(name, _)| named(name));
            return constant.map(|(_, value)| *value);
        }
        self.figures()
    }

    /// `function` of the number after it.
    fn apply(&mut self, function: Function) -> Option<f64> {
        self.worked = true;
        self.deeper(|reading| reading.number().map(function))
    }

    /// A number written in figures, with its decimals and what stands
    /// between its thousands as [`Numbers`] has them.
    fn figures(&mut self) -> Option<f64> {
        let numbers = self.numbers;
        let mut figures = String::new();
        let mut decimals = false;
        while let Some(&next) = self.rest.peek() {
            if next.is_ascii_digit() {
                figures.push(next);
            } else if !decimals && !figures.is_empty() && numbers.between(next) && self.thousand() {
                // Read, and left out.
            } else if numbers.decimal(next) {
                decimals = true;
                figures.push('.');
            } else {
                break;
            }
            self.rest.next();
        }
        figures.parse().ok()
    }

    /// Whether three figures, and no more, follow the next character.
    fn thousand(&self) -> bool {
        let mut ahead = self.rest.clone();
        ahead.next();
        let three = (0..3).all(|_| ahead.next().is_some_and(|c| c.is_ascii_digit()));
        three && !ahead.peek().is_some_and(char::is_ascii_digit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The answer to what is typed, as numbers are written in English.
    fn english(typed: &str) -> Option<String> {
        answer(typed, Numbers::Point)
    }

    /// The answer to what is typed, as numbers are written in Polish.
    fn polish(typed: &str) -> Option<String> {
        answer(typed, Numbers::Comma)
    }

    #[test]
    fn a_sum_is_worked_out() {
        assert_eq!(english("2+2").as_deref(), Some("4"));
        assert_eq!(english(" 12 - 20 ").as_deref(), Some("-8"));
        assert_eq!(english("6 * 7").as_deref(), Some("42"));
        assert_eq!(english("6×7").as_deref(), Some("42"));
        assert_eq!(english("7/2").as_deref(), Some("3.5"));
        assert_eq!(english("7÷2").as_deref(), Some("3.5"));
        assert_eq!(english("2^10").as_deref(), Some("1024"));
        assert_eq!(english(".5 + 1.").as_deref(), Some("1.5"));
    }

    #[test]
    fn multiplying_comes_before_adding_and_powers_before_both() {
        assert_eq!(english("2+3*4").as_deref(), Some("14"));
        assert_eq!(english("(2+3)*4").as_deref(), Some("20"));
        assert_eq!(english("10-4-3").as_deref(), Some("3"), "from the left");
        assert_eq!(english("2^3^2").as_deref(), Some("512"), "from the right");
        assert_eq!(english("-2^2").as_deref(), Some("-4"));
        assert_eq!(english("2*-3").as_deref(), Some("-6"));
        assert_eq!(english("2^-1").as_deref(), Some("0.5"));
        assert_eq!(english("--3+1").as_deref(), Some("4"));
        assert_eq!(english("+3-1").as_deref(), Some("2"));
    }

    #[test]
    fn a_number_alone_is_not_a_sum() {
        // Else "7" would put itself above 7-Zip, with nothing to work out.
        assert_eq!(english("7"), None);
        assert_eq!(english("-7"), None);
        assert_eq!(english("(7)"), None);
        assert_eq!(english(""), None);
    }

    #[test]
    fn what_is_not_a_whole_sum_has_no_english() {
        for typed in [
            "2+", "2*", "2^", "(2*", "*2", "2 3", "2+3)", "()", "1.2.3+1", "2+x", "firefox", ".",
            "2..+1",
        ] {
            assert_eq!(english(typed), None, "{typed}");
        }
    }

    #[test]
    fn a_sum_not_yet_finished_is_answered_as_far_as_it_goes() {
        // As another sign is typed, the answer so far stays, to go on from.
        assert_eq!(english("60 + 60 +").as_deref(), Some("120"));
        assert_eq!(english("60 + 60 * ").as_deref(), Some("120"));
        assert_eq!(english("60+60+5").as_deref(), Some("125"));
        // Brackets left open are closed.
        assert_eq!(english("2*(3+4").as_deref(), Some("14"));
        assert_eq!(english("2*(3+(").as_deref(), Some("6"));
        // A number alone is still no sum, however it ends.
        assert_eq!(english("60 +"), None);
        assert_eq!(english("(60"), None);
    }

    #[test]
    fn what_has_no_number_for_an_answer_has_none() {
        assert_eq!(english("1/0"), None);
        assert_eq!(english("0/0"), None);
        assert_eq!(english("10^400"), None);
    }

    #[test]
    fn an_answer_is_rounded_as_a_person_would_write_it() {
        assert_eq!(english("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(english("1/3").as_deref(), Some("0.333333333333333"));
        assert_eq!(english("2/3").as_deref(), Some("0.666666666666667"));
        assert_eq!(english("0*-1").as_deref(), Some("0"), "never -0");
        assert_eq!(english("10^20+1").as_deref(), Some("1e20"));
        assert_eq!(english("10^15-1").as_deref(), Some("999999999999999"));
        assert_eq!(english("1/10^10").as_deref(), Some("1e-10"));
        assert_eq!(english("-3/2^40").as_deref(), Some("-2.72848410531878e-12"));
    }

    #[test]
    fn brackets_deeper_than_a_launcher_holds_are_not_followed() {
        let deep = |depth: usize| format!("{}1+1{}", "(".repeat(depth), ")".repeat(depth));
        assert_eq!(english(&deep(DEEPEST - 1)).as_deref(), Some("2"));
        assert_eq!(english(&deep(10_000)), None);
        assert_eq!(english(&format!("1+{}1", "-".repeat(10_000))), None);
    }

    #[test]
    fn numbers_are_read_as_where_the_user_is_writes_them() {
        assert_eq!(polish("1,5+1").as_deref(), Some("2,5"));
        assert_eq!(polish(",5*2").as_deref(), Some("1"));
        assert_eq!(
            english("1,5+1"),
            None,
            "a comma in English is between thousands"
        );
        // A point, as on a keypad, is still the decimals where a comma is.
        assert_eq!(polish("1.5+1").as_deref(), Some("2,5"));
        assert_eq!(polish("1,5.5+1"), None);
        assert_eq!(english("1.5.5+1"), None);
    }

    #[test]
    fn what_stands_between_thousands_is_read_only_before_three_figures() {
        assert_eq!(english("1,000+1").as_deref(), Some("1001"));
        assert_eq!(english("1,234,567.5*2").as_deref(), Some("2469135"));
        assert_eq!(english("1 000 + 1").as_deref(), Some("1001"));
        assert_eq!(english("1\u{a0}000+1").as_deref(), Some("1001"));
        assert_eq!(polish("1 000,5+1").as_deref(), Some("1001,5"));
        assert_eq!(polish("1.000,5+1").as_deref(), Some("1001,5"));
        assert_eq!(polish("1\u{202f}000+1").as_deref(), Some("1001"));
        for typed in [
            "2 3",
            "1 0000+1",
            "1,0000+1",
            "1,00+1",
            "1.5,000+1",
            "1 00 + 1",
        ] {
            assert_eq!(english(typed), None, "{typed}");
        }
        // After the decimals, nothing stands between thousands.
        assert_eq!(polish("1,000.000+1"), None);
        assert_eq!(polish("1 000 0+1"), None);
        // The point of a keypad is the decimals where it is not before three.
        assert_eq!(polish("1.5000+1").as_deref(), Some("2,5"));
    }

    #[test]
    fn an_answer_is_written_with_the_decimals_where_the_user_is_writes_them() {
        assert_eq!(polish("7/2").as_deref(), Some("3,5"));
        assert_eq!(polish("1/10^10").as_deref(), Some("1e-10"));
        assert_eq!(polish("25/10^21").as_deref(), Some("2,5e-20"));
        assert_eq!(
            polish("1000*1000").as_deref(),
            Some("1000000"),
            "never grouped"
        );
    }

    #[test]
    fn a_percentage_added_or_taken_away_is_of_what_it_is_added_to() {
        assert_eq!(english("200+10%").as_deref(), Some("220"));
        assert_eq!(english("200 - 10 %").as_deref(), Some("180"));
        assert_eq!(english("100+10%+10%").as_deref(), Some("121"));
        assert_eq!(english("200+-10%").as_deref(), Some("180"));
        // Anywhere else, it is a hundredth.
        assert_eq!(english("50%").as_deref(), Some("0.5"));
        assert_eq!(english("200*10%").as_deref(), Some("20"));
        assert_eq!(english("200+10%*2").as_deref(), Some("200.2"));
        assert_eq!(english("200+(10%)").as_deref(), Some("200.1"));
        assert_eq!(english("10%+5").as_deref(), Some("5.1"));
        assert_eq!(english("(10+5)%").as_deref(), Some("0.15"));
        assert_eq!(polish("12,5%").as_deref(), Some("0,125"));
        for typed in ["%", "10%%", "%10", "2+%"] {
            assert_eq!(english(typed), None, "{typed}");
        }
    }

    #[test]
    fn square_roots_and_pi_are_worked_out() {
        assert_eq!(english("sqrt(16)").as_deref(), Some("4"));
        assert_eq!(english("sqrt 16 + 1").as_deref(), Some("5"));
        assert_eq!(english("√16").as_deref(), Some("4"));
        assert_eq!(english("√√16").as_deref(), Some("2"));
        assert_eq!(english("SQRT(2)^2").as_deref(), Some("2"));
        assert_eq!(english("2*pi").as_deref(), Some("6.28318530717959"));
        assert_eq!(english("π*2").as_deref(), Some("6.28318530717959"));
        assert_eq!(english("Pi + 1").as_deref(), Some("4.14159265358979"));
        assert_eq!(english("sqrt(pi*pi)").as_deref(), Some("3.14159265358979"));
        assert_eq!(polish("√6,25").as_deref(), Some("2,5"));
        // A constant alone, as a number alone, is no sum.
        assert_eq!(english("pi"), None);
        assert_eq!(english("π"), None);
        for typed in [
            "sqrt", "√", "sqrt(-1)", "2pi", "pi2", "sqr(4)", "sqrtt(4)", "pie+1", "x+1",
        ] {
            assert_eq!(english(typed), None, "{typed}");
        }
    }

    #[test]
    fn angles_are_in_degrees_and_logarithms_natural_or_of_ten() {
        assert_eq!(english("sin(30)").as_deref(), Some("0.5"));
        assert_eq!(english("cos 60").as_deref(), Some("0.5"));
        assert_eq!(english("tan(45)").as_deref(), Some("1"));
        assert_eq!(english("SIN(90)*2").as_deref(), Some("2"));
        // What is nothing comes out as nothing, not as what is left over.
        assert_eq!(english("sin(180)").as_deref(), Some("0"));
        assert_eq!(english("cos(90)+1").as_deref(), Some("1"));
        assert_eq!(english("ln(e)").as_deref(), Some("1"));
        assert_eq!(english("log(1000)").as_deref(), Some("3"));
        assert_eq!(english("2*e").as_deref(), Some("5.43656365691809"));
        assert_eq!(polish("sin(30)+0,5").as_deref(), Some("1"));
        // What has no number for an answer has none.
        for typed in ["tan(90)", "ln(0)", "ln(-1)", "log(0)", "e", "E"] {
            assert_eq!(english(typed), None, "{typed}");
        }
    }

    #[test]
    fn a_word_or_a_sign_not_yet_finished_is_answered_as_far_as_it_goes() {
        assert_eq!(english("4*2+sqrt(").as_deref(), Some("8"));
        assert_eq!(english("4*2+sq").as_deref(), Some("8"));
        assert_eq!(english("4*2+√").as_deref(), Some("8"));
        assert_eq!(english("4*2*p").as_deref(), Some("8"));
        assert_eq!(english("2*3+co").as_deref(), Some("6"));
        assert_eq!(english("2*3+lo").as_deref(), Some("6"));
        assert_eq!(english("2*3+sin(").as_deref(), Some("6"));
        assert_eq!(english("9+sqrt(16").as_deref(), Some("13"));
        assert_eq!(english("1,000+1,").as_deref(), Some("1001"));
        assert_eq!(polish("2,5+1,").as_deref(), Some("3,5"));
        assert_eq!(english("2.5+1.").as_deref(), Some("3.5"));
        // What could begin no word is no sum, however it ends.
        assert_eq!(english("4*2+x"), None);
        assert_eq!(english("firefox"), None);
    }

    #[test]
    fn roots_deeper_than_a_launcher_holds_are_not_followed() {
        assert_eq!(
            english(&format!("{}16", "√".repeat(DEEPEST - 1))).as_deref(),
            Some("1")
        );
        assert_eq!(english(&format!("{}16", "√".repeat(10_000))), None);
    }

    #[test]
    fn how_numbers_are_written_is_known_by_the_locale() {
        for named in [
            "pl_PL.UTF-8",
            "de_DE",
            "fr_FR.UTF-8@euro",
            "pt_BR",
            "ru",
            "es_ES",
        ] {
            assert_eq!(Numbers::of_locale(named), Numbers::Comma, "{named}");
        }
        for named in [
            "en_US.UTF-8",
            "en_GB",
            "C",
            "POSIX",
            "",
            "ja_JP",
            "de_CH.UTF-8",
            "es_MX",
        ] {
            assert_eq!(Numbers::of_locale(named), Numbers::Point, "{named}");
        }
    }
}
