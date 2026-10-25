//! The little of the Assuan protocol a pinentry speaks: lines of a command
//! and its argument, with what cannot be in a line written as `%` and two
//! hexadecimal digits.

/// The longest line taken in, in bytes, as the protocol has it.
pub const LONGEST: usize = 1000;

/// `text` as it may be sent in a line: `%`, a line's end and a carriage
/// return each written as `%` and its number in hexadecimal.
#[must_use]
pub fn escaped(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '%' => out.push_str("%25"),
            '\n' => out.push_str("%0A"),
            '\r' => out.push_str("%0D"),
            other => out.push(other),
        }
    }
    out
}

/// What was sent as `text`, with each `%` and two hexadecimal digits the
/// byte they stand for. Anything else after a `%` is kept as it is, and
/// bytes that do not make up text are replaced.
#[must_use]
pub fn unescaped(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let digits = bytes.get(at + 1..at + 3);
        let byte = digits
            .filter(|_| bytes[at] == b'%')
            .and_then(|digits| std::str::from_utf8(digits).ok())
            .and_then(|digits| u8::from_str_radix(digits, 16).ok());
        if let Some(byte) = byte {
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A line split into its command, in capitals as commands are sent, and
/// what follows it, unescaped.
#[must_use]
pub fn command(line: &str) -> (String, String) {
    let line = line.trim_end_matches(['\n', '\r']);
    let (name, rest) = line.split_once(' ').unwrap_or((line, ""));
    (name.to_ascii_uppercase(), unescaped(rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_cannot_be_in_a_line_is_written_by_its_number_and_read_back() {
        let text = "100% sure\r\nof it";
        assert_eq!(escaped(text), "100%25 sure%0D%0Aof it");
        assert_eq!(unescaped(&escaped(text)), text);
        assert_eq!(unescaped("na%C3%AFve"), "naïve", "bytes of more than one");
        assert_eq!(unescaped("50%"), "50%", "a % at the end is itself");
        assert_eq!(
            unescaped("%zz%4"),
            "%zz%4",
            "and so is one not followed by a number"
        );
        assert_eq!(
            unescaped("%FF"),
            "\u{FFFD}",
            "a byte that is not text is replaced"
        );
    }

    #[test]
    fn a_line_is_its_command_and_what_follows() {
        assert_eq!(
            command("setdesc Enter it%0Anow\n"),
            ("SETDESC".to_owned(), "Enter it\nnow".to_owned())
        );
        assert_eq!(command("GETPIN"), ("GETPIN".to_owned(), String::new()));
    }
}
