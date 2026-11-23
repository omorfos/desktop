//! What a screen says it is, read from the block of facts it gives about
//! itself, its EDID: who made it, which of theirs it is, its name and its
//! serial, so that it is known wherever it is plugged in (ADR 103).
//!
//! The block comes from the screen, and a screen may say anything: what is
//! not as the standard has it is not taken, and text is kept to what can be
//! shown as it is.

/// How the block of facts begins.
const HEADER: [u8; 8] = [0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
/// How long its first part is, which holds all of this.
const BLOCK: usize = 128;
/// Where its four descriptors begin, each this long.
const DESCRIPTORS: [usize; 4] = [54, 72, 90, 108];
const DESCRIPTOR: usize = 18;
/// What a descriptor of text is marked with: the screen's name, and its
/// serial.
const NAME: u8 = 0xfc;
const SERIAL: u8 = 0xff;

/// What a screen says it is.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identity {
    /// Who made it, as the three letters it is registered by, such as `DEL`.
    pub maker: String,
    /// Which of the maker's screens it is, by the maker's number for it.
    pub product: u16,
    /// Its name, such as `DELL U2720Q`, if it says one.
    pub name: Option<String>,
    /// Its serial, if it says one: the text it gives, or else the number.
    pub serial: Option<String>,
}

impl Identity {
    /// What the screen whose block of facts is `edid` says it is; none if
    /// the block is not one, or names no maker.
    #[must_use]
    pub fn from_edid(edid: &[u8]) -> Option<Self> {
        let block = edid.get(..BLOCK)?;
        if block[..HEADER.len()] != HEADER {
            return None;
        }
        let maker = maker(u16::from_be_bytes([block[8], block[9]]))?;
        let product = u16::from_le_bytes([block[10], block[11]]);
        let number = u32::from_le_bytes([block[12], block[13], block[14], block[15]]);
        let said = |kind| {
            DESCRIPTORS.iter().find_map(|&at| {
                let descriptor = &block[at..at + DESCRIPTOR];
                (descriptor[..3] == [0, 0, 0] && descriptor[3] == kind)
                    .then(|| text(&descriptor[5..]))
                    .flatten()
            })
        };
        let serial = said(SERIAL).or_else(|| (number != 0).then(|| number.to_string()));
        Some(Self {
            maker,
            product,
            name: said(NAME),
            serial,
        })
    }
}

/// The three letters packed into `packed`, five bits each, `A` being 1;
/// none if any is not a letter.
fn maker(packed: u16) -> Option<String> {
    [10, 5, 0]
        .into_iter()
        .map(|shift| {
            let letter = u8::try_from((packed >> shift) & 0x1f).unwrap_or_default();
            (1..=26)
                .contains(&letter)
                .then(|| char::from(b'A' + letter - 1))
        })
        .collect()
}

/// The text of a descriptor: up to its line's end, without the spaces it
/// is padded with; none if it is empty or holds what is not printable.
fn text(bytes: &[u8]) -> Option<String> {
    let line = bytes
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    let shown = line
        .iter()
        .all(|byte| byte.is_ascii_graphic() || *byte == b' ');
    let text = String::from_utf8_lossy(line).trim().to_owned();
    (shown && !text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A block of facts from `maker`, as letters, of product `product`, with
    /// the number `number` and descriptors of `texts`, each a kind and its
    /// text.
    fn edid(maker: &str, product: u16, number: u32, texts: &[(u8, &str)]) -> Vec<u8> {
        let mut block = vec![0; BLOCK];
        block[..8].copy_from_slice(&HEADER);
        let letters = maker.bytes().map(|letter| u16::from(letter - b'A' + 1));
        let packed = letters.fold(0, |packed, letter| (packed << 5) | letter);
        block[8..10].copy_from_slice(&packed.to_be_bytes());
        block[10..12].copy_from_slice(&product.to_le_bytes());
        block[12..16].copy_from_slice(&number.to_le_bytes());
        // The first descriptor, as on every screen, says how it is driven.
        block[54] = 1;
        for (&at, (kind, said)) in DESCRIPTORS[1..].iter().zip(texts) {
            block[at + 3] = *kind;
            let mut written = said.as_bytes().to_vec();
            written.push(b'\n');
            written.resize(13, b' ');
            block[at + 5..at + DESCRIPTOR].copy_from_slice(&written);
        }
        block
    }

    #[test]
    fn a_monitor_says_its_maker_product_name_and_serial() {
        let said = edid(
            "DEL",
            0xa0e1,
            7,
            &[(NAME, "DELL U2720Q"), (SERIAL, "7X3KD13")],
        );
        let identity = Identity::from_edid(&said).unwrap();
        assert_eq!(
            identity,
            Identity {
                maker: "DEL".to_owned(),
                product: 0xa0e1,
                name: Some("DELL U2720Q".to_owned()),
                serial: Some("7X3KD13".to_owned()),
            }
        );
    }

    #[test]
    fn a_laptops_panel_that_says_no_name_or_serial_is_known_by_maker_and_product() {
        // As the maintainer's BOE NV140QUM-N53, whose model is free text.
        let said = edid(
            "BOE",
            0x07c8,
            0,
            &[(0xfe, "BOE HF"), (0xfe, "NV140QUM-N53")],
        );
        let identity = Identity::from_edid(&said).unwrap();
        assert_eq!((identity.maker.as_str(), identity.product), ("BOE", 0x07c8));
        assert_eq!((identity.name, identity.serial), (None, None));
    }

    #[test]
    fn a_serial_given_only_as_a_number_is_that_number() {
        let identity = Identity::from_edid(&edid("GSM", 1, 123_456, &[])).unwrap();
        assert_eq!(identity.serial.as_deref(), Some("123456"));
    }

    #[test]
    fn what_is_not_a_block_of_facts_says_nothing() {
        assert_eq!(Identity::from_edid(&[]), None);
        assert_eq!(Identity::from_edid(&[0; 200]), None, "no header");
        let mut short = edid("DEL", 1, 0, &[]);
        short.truncate(BLOCK - 1);
        assert_eq!(Identity::from_edid(&short), None, "cut short");
        let mut no_maker = edid("DEL", 1, 0, &[]);
        no_maker[8..10].copy_from_slice(&[0, 0]);
        assert_eq!(Identity::from_edid(&no_maker), None, "no letters");
    }

    #[test]
    fn text_that_cannot_be_shown_as_it_is_is_not_taken() {
        let mut said = edid("DEL", 1, 0, &[(NAME, "fine")]);
        said[DESCRIPTORS[1] + 6] = 0x1b;
        assert_eq!(Identity::from_edid(&said).unwrap().name, None, "an escape");
        let blank = edid("DEL", 1, 0, &[(NAME, "   ")]);
        assert_eq!(Identity::from_edid(&blank).unwrap().name, None, "nothing");
    }
}
