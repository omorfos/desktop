//! Sums, worked out as they are typed: in the launcher, and in the
//! calculator that is to come, so that a sum comes out the same in each.
//!
//! What is typed is read here and nowhere else: numbers, `+`, `-`, `*` or
//! `×`, `/` or `÷`, `^` and brackets. Nothing typed is ever run.

use std::iter::Peekable;
use std::str::Chars;

/// The deepest that brackets, signs and powers may stand one inside
/// another. The launcher holds no more characters than this, so it is only
/// reached by what is typed to overflow the stack.
const DEEPEST: usize = 64;

/// How many figures of an answer are kept: as many as an `f64` holds
/// truly, so that `0.1+0.2` is `0.3`.
const FIGURES: usize = 15;

/// The signs a sum not yet finished may end with, and a bracket opened.
const UNFINISHED: [char; 8] = ['+', '-', '*', '×', '/', '÷', '^', '('];

/// The answer to what is typed, written as a person would write it, if
/// it is a sum, with at least one sign between two numbers and a number
/// for an answer. A sum not yet finished, ending in a sign or with a
/// bracket open, is answered as far as it goes, so that the answer stays
/// as the next sign is typed: `60 + 60 +` is `120`.
#[must_use]
pub fn answer(typed: &str) -> Option<String> {
    whole(typed).or_else(|| {
        let begun = typed.trim_end_matches(|c: char| c.is_whitespace() || UNFINISHED.contains(&c));
        let open = begun.matches('(').count();
        let open = open.saturating_sub(begun.matches(')').count());
        whole(&format!("{begun}{}", ")".repeat(open)))
    })
}

/// The answer to what is typed, if it is a whole sum.
fn whole(typed: &str) -> Option<String> {
    let mut reading = Reading {
        rest: typed.chars().peekable(),
        depth: 0,
        worked: false,
    };
    let value = reading.sum()?;
    let whole = reading.next().is_none();
    (whole && reading.worked && value.is_finite()).then(|| written(value))
}

/// An answer, rounded to [`FIGURES`] figures, without a sign if it is
/// nothing, and with a power of ten if it is very large or very small.
fn written(value: f64) -> String {
    let rounded: f64 = format!("{value:.*e}", FIGURES - 1).parse().unwrap_or(value);
    // Nothing is written without a sign, however it came.
    let rounded = rounded + 0.0;
    let size = rounded.abs();
    if size != 0.0 && !(1e-9..1e15).contains(&size) {
        format!("{rounded:e}")
    } else {
        format!("{rounded}")
    }
}

/// What is left to read of what was typed.
struct Reading<'a> {
    rest: Peekable<Chars<'a>>,
    /// How deep inside one another what is being read stands.
    depth: usize,
    /// Whether two numbers have been put together, which makes it a sum.
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

    /// Terms added or taken away, from the left.
    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        while let Some(sign) = self.take(&['+', '-']) {
            let term = self.product()?;
            value = if sign == '+' {
                value + term
            } else {
                value - term
            };
            self.worked = true;
        }
        Some(value)
    }

    /// Factors multiplied or divided, from the left.
    fn product(&mut self) -> Option<f64> {
        let mut value = self.signed()?;
        while let Some(sign) = self.take(&['*', '×', '/', '÷']) {
            let factor = self.signed()?;
            value = if matches!(sign, '*' | '×') {
                value * factor
            } else {
                value / factor
            };
            self.worked = true;
        }
        Some(value)
    }

    /// A power, with as many signs before it as there are: `-2^2` is
    /// `-4`, as it is written by hand.
    fn signed(&mut self) -> Option<f64> {
        self.depth += 1;
        let value = if self.depth > DEEPEST {
            None
        } else if let Some(sign) = self.take(&['+', '-']) {
            let value = self.signed();
            value.map(|value| if sign == '-' { -value } else { value })
        } else {
            self.power()
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

    /// A number written out, or a sum in brackets.
    fn number(&mut self) -> Option<f64> {
        if self.take(&['(']).is_some() {
            let value = self.sum()?;
            return self.take(&[')']).map(|_| value);
        }
        self.skip_spaces();
        let mut digits = String::new();
        while let Some(digit) = self.rest.next_if(|c| c.is_ascii_digit() || *c == '.') {
            digits.push(digit);
        }
        digits.parse().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sum_is_worked_out() {
        assert_eq!(answer("2+2").as_deref(), Some("4"));
        assert_eq!(answer(" 12 - 20 ").as_deref(), Some("-8"));
        assert_eq!(answer("6 * 7").as_deref(), Some("42"));
        assert_eq!(answer("6×7").as_deref(), Some("42"));
        assert_eq!(answer("7/2").as_deref(), Some("3.5"));
        assert_eq!(answer("7÷2").as_deref(), Some("3.5"));
        assert_eq!(answer("2^10").as_deref(), Some("1024"));
        assert_eq!(answer(".5 + 1.").as_deref(), Some("1.5"));
    }

    #[test]
    fn multiplying_comes_before_adding_and_powers_before_both() {
        assert_eq!(answer("2+3*4").as_deref(), Some("14"));
        assert_eq!(answer("(2+3)*4").as_deref(), Some("20"));
        assert_eq!(answer("10-4-3").as_deref(), Some("3"), "from the left");
        assert_eq!(answer("2^3^2").as_deref(), Some("512"), "from the right");
        assert_eq!(answer("-2^2").as_deref(), Some("-4"));
        assert_eq!(answer("2*-3").as_deref(), Some("-6"));
        assert_eq!(answer("2^-1").as_deref(), Some("0.5"));
        assert_eq!(answer("--3+1").as_deref(), Some("4"));
        assert_eq!(answer("+3-1").as_deref(), Some("2"));
    }

    #[test]
    fn a_number_alone_is_not_a_sum() {
        // Else "7" would put itself above 7-Zip, with nothing to work out.
        assert_eq!(answer("7"), None);
        assert_eq!(answer("-7"), None);
        assert_eq!(answer("(7)"), None);
        assert_eq!(answer(""), None);
    }

    #[test]
    fn what_is_not_a_whole_sum_has_no_answer() {
        for typed in [
            "2+", "2*", "2^", "(2*", "*2", "2 3", "2+3)", "()", "1.2.3+1", "2+x", "firefox", ".",
            "2..+1",
        ] {
            assert_eq!(answer(typed), None, "{typed}");
        }
    }

    #[test]
    fn a_sum_not_yet_finished_is_answered_as_far_as_it_goes() {
        // As another sign is typed, the answer so far stays, to go on from.
        assert_eq!(answer("60 + 60 +").as_deref(), Some("120"));
        assert_eq!(answer("60 + 60 * ").as_deref(), Some("120"));
        assert_eq!(answer("60+60+5").as_deref(), Some("125"));
        // Brackets left open are closed.
        assert_eq!(answer("2*(3+4").as_deref(), Some("14"));
        assert_eq!(answer("2*(3+(").as_deref(), Some("6"));
        // A number alone is still no sum, however it ends.
        assert_eq!(answer("60 +"), None);
        assert_eq!(answer("(60"), None);
    }

    #[test]
    fn what_has_no_number_for_an_answer_has_none() {
        assert_eq!(answer("1/0"), None);
        assert_eq!(answer("0/0"), None);
        assert_eq!(answer("10^400"), None);
    }

    #[test]
    fn an_answer_is_rounded_as_a_person_would_write_it() {
        assert_eq!(answer("0.1+0.2").as_deref(), Some("0.3"));
        assert_eq!(answer("1/3").as_deref(), Some("0.333333333333333"));
        assert_eq!(answer("2/3").as_deref(), Some("0.666666666666667"));
        assert_eq!(answer("0*-1").as_deref(), Some("0"), "never -0");
        assert_eq!(answer("10^20+1").as_deref(), Some("1e20"));
        assert_eq!(answer("10^15-1").as_deref(), Some("999999999999999"));
        assert_eq!(answer("1/10^10").as_deref(), Some("1e-10"));
        assert_eq!(answer("-3/2^40").as_deref(), Some("-2.72848410531878e-12"));
    }

    #[test]
    fn brackets_deeper_than_a_launcher_holds_are_not_followed() {
        let deep = |depth: usize| format!("{}1+1{}", "(".repeat(depth), ")".repeat(depth));
        assert_eq!(answer(&deep(DEEPEST - 1)).as_deref(), Some("2"));
        assert_eq!(answer(&deep(10_000)), None);
        assert_eq!(answer(&format!("1+{}1", "-".repeat(10_000))), None);
    }
}
