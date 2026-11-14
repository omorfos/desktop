//! The words of the omorfos desktop in the user's language (ADR 79).
//!
//! Each program's words are a file of Project Fluent's messages in
//! English, `i18n/en/<program>.ftl`, and one for each language they are
//! translated into, `i18n/<language>/<program>.ftl`, all built into the
//! program, so that nothing is looked for when it starts. English is loaded
//! under the user's language, so a message a translation does not have yet
//! is said in English, a message at a time; and a message nothing has is
//! said by its name, which a test makes sure never happens.

use std::{env, fmt};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

/// A program's words: in English, and in each language they are
/// translated into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Program {
    /// Its name, as its files are named, as `panel`.
    pub name: &'static str,
    /// Its words in English.
    pub english: &'static str,
    /// Its words in each other language, by the language's code, as `pl`.
    pub translated: &'static [(&'static str, &'static str)],
}

/// The panel's words.
pub const PANEL: Program = Program {
    name: "panel",
    english: include_str!("../../../i18n/en/panel.ftl"),
    translated: &[("pl", include_str!("../../../i18n/pl/panel.ftl"))],
};

/// The lock screen's words.
pub const LOCK: Program = Program {
    name: "lock",
    english: include_str!("../../../i18n/en/lock.ftl"),
    translated: &[("pl", include_str!("../../../i18n/pl/lock.ftl"))],
};

/// The words of the boxes that ask for a password or a passphrase.
pub const ASK: Program = Program {
    name: "ask",
    english: include_str!("../../../i18n/en/ask.ftl"),
    translated: &[("pl", include_str!("../../../i18n/pl/ask.ftl"))],
};

/// The words of the notices `omorfos-session` shows.
pub const SESSION: Program = Program {
    name: "session",
    english: include_str!("../../../i18n/en/session.ftl"),
    translated: &[("pl", include_str!("../../../i18n/pl/session.ftl"))],
};

/// The compositor's own words.
pub const COMPOSITOR: Program = Program {
    name: "compositor",
    english: include_str!("../../../i18n/en/compositor.ftl"),
    translated: &[("pl", include_str!("../../../i18n/pl/compositor.ftl"))],
};

/// Every program whose words are here, which the tests hold every
/// language's files to.
pub const PROGRAMS: &[Program] = &[PANEL, LOCK, ASK, SESSION, COMPOSITOR];

/// What a message is given to say: words, or a number, by which a
/// message may choose among forms that differ with how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value<'a> {
    /// Words, said as they are.
    Words(&'a str),
    /// A number, said as the language writes it.
    Number(i64),
}

/// A program's words, in the language chosen and in English beneath it.
pub struct Words {
    /// The languages to look a message up in, the one chosen first.
    bundles: Vec<FluentBundle<FluentResource>>,
}

impl Words {
    /// `program`'s words in the language the environment asks for, as the
    /// C library reads it, or in English.
    #[must_use]
    pub fn of(program: &Program) -> Self {
        Self::in_languages(program, &wanted(|name| env::var(name).ok()))
    }

    /// `program`'s words in the language of `locale`, as `pl_PL.UTF-8`, for
    /// a program that has read its locale already, or in English.
    #[must_use]
    pub fn in_locale(program: &Program, locale: &str) -> Self {
        let languages = wanted(|name| (name == "LANG").then(|| locale.to_owned()));
        Self::in_languages(program, &languages)
    }

    /// `program`'s words in the first of `languages`, by their codes, that
    /// it is translated into, or in English.
    #[must_use]
    pub fn in_languages(program: &Program, languages: &[String]) -> Self {
        let translated = languages.iter().find_map(|language| {
            let mut translated = program.translated.iter();
            translated.find(|(code, _)| code == language)
        });
        let english = ("en", program.english);
        let bundles = translated.into_iter().copied().chain([english]);
        Self {
            bundles: bundles.map(|(code, text)| bundle(code, text)).collect(),
        }
    }

    /// What the message `name` says, given `values`: in the language
    /// chosen, or in English if it has no such message yet, or `name`
    /// itself if neither has.
    #[must_use]
    pub fn say(&self, name: &str, values: &[(&str, Value<'_>)]) -> String {
        let mut given = FluentArgs::new();
        for (value_name, value) in values {
            let value = match value {
                Value::Words(words) => FluentValue::from(*words),
                Value::Number(number) => FluentValue::from(*number),
            };
            given.set(*value_name, value);
        }
        for bundle in &self.bundles {
            let pattern = bundle.get_message(name).and_then(|message| message.value());
            if let Some(pattern) = pattern {
                // What could not be filled in is said by its name, as
                // `{$at}`, which the tests make sure never happens.
                let mut unfilled = Vec::new();
                return bundle
                    .format_pattern(pattern, Some(&given), &mut unfilled)
                    .into_owned();
            }
        }
        name.to_owned()
    }
}

/// Which languages it has, as what holds it is shown when looking into it.
impl fmt::Debug for Words {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let languages = self.bundles.iter().map(|bundle| {
            let language = bundle.locales.first().map(ToString::to_string);
            language.unwrap_or_default()
        });
        formatter
            .debug_struct("Words")
            .field("languages", &languages.collect::<Vec<_>>())
            .finish()
    }
}

/// A bundle of `text`'s messages in the language `code`.
fn bundle(code: &str, text: &str) -> FluentBundle<FluentResource> {
    let language: LanguageIdentifier = code.parse().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![language]);
    // The words are drawn as they are, by text that would show the marks
    // Fluent puts round what it is given, for text that runs both ways.
    bundle.set_use_isolating(false);
    // What of a file could be read is kept; a test makes sure all can be.
    let resource = FluentResource::try_new(text.to_owned()).unwrap_or_else(|(read, _)| read);
    bundle.add_resource_overriding(resource);
    bundle
}

/// The languages the environment asks for, by their codes, in order:
/// `LANGUAGE`'s, then the first of `LC_ALL`, `LC_MESSAGES` and `LANG` that
/// is set, as the C library reads them. `C` and `POSIX` are English.
fn wanted(get: impl Fn(&str) -> Option<String>) -> Vec<String> {
    let set = |name: &str| get(name).filter(|value| !value.is_empty());
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().find_map(set);
    let listed = set("LANGUAGE").unwrap_or_default();
    let named = listed.split(':').map(str::to_owned).chain(locale);
    let codes = named.map(|named| {
        let language = named.split(['_', '.', '@', '-']).next().unwrap_or_default();
        language.to_lowercase()
    });
    let codes = codes.filter(|code| !code.is_empty() && code != "c" && code != "posix");
    codes.collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    const TOOL: Program = Program {
        name: "tool",
        english: "hello = Hello, { $name }\nfiles = { $count ->\n    [one] One file\n   *[other] { $count } files\n}\nonly = Only in English\n",
        translated: &[
            (
                "pl",
                "hello = Cześć, { $name }\nfiles = { $count ->\n    [one] Jeden plik\n    [few] { $count } pliki\n    [many] { $count } plików\n   *[other] { $count } pliku\n}\n",
            ),
            // What can be read of a file that is wrong in part is kept.
            ("de", "hello = Hallo, { $name }\nbroken = {\n"),
        ],
    };

    fn languages(codes: &[&str]) -> Vec<String> {
        codes.iter().map(|code| (*code).to_owned()).collect()
    }

    #[test]
    fn a_message_is_said_in_the_language_chosen_with_the_values_given() {
        let polish = Words::in_languages(&TOOL, &languages(&["pl"]));
        let name = [("name", Value::Words("Ala"))];
        assert_eq!(polish.say("hello", &name), "Cześć, Ala");
        let count = |count| [("count", Value::Number(count))];
        assert_eq!(polish.say("files", &count(1)), "Jeden plik");
        assert_eq!(polish.say("files", &count(3)), "3 pliki");
        assert_eq!(polish.say("files", &count(5)), "5 plików");
        assert_eq!(polish.say("files", &count(22)), "22 pliki");
        let english = Words::in_languages(&TOOL, &[]);
        assert_eq!(english.say("files", &count(2)), "2 files");
        assert_eq!(
            english.say("hello", &name),
            "Hello, Ala",
            "no marks round Ala"
        );
        let shown = format!("{polish:?}");
        assert_eq!(shown, r#"Words { languages: ["pl", "en"] }"#);
    }

    #[test]
    fn what_a_language_has_not_got_is_said_in_english_and_what_none_has_by_its_name() {
        let polish = Words::in_languages(&TOOL, &languages(&["pl"]));
        assert_eq!(polish.say("only", &[]), "Only in English");
        assert_eq!(polish.say("nowhere", &[]), "nowhere");
        // The first language wanted that is there is chosen.
        let words = Words::in_languages(&TOOL, &languages(&["fr", "de", "pl"]));
        assert_eq!(
            words.say("hello", &[("name", Value::Words("Ala"))]),
            "Hallo, Ala"
        );
        assert_eq!(words.say("broken", &[]), "broken");
    }

    #[test]
    fn the_language_is_the_one_the_environment_asks_for_as_the_c_library_reads_it() {
        let wanted_of = |set: &[(&str, &str)]| {
            let set: HashMap<String, String> = set
                .iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect();
            wanted(|name| set.get(name).cloned())
        };
        assert_eq!(wanted_of(&[]), languages(&[]));
        assert_eq!(wanted_of(&[("LANG", "pl_PL.UTF-8")]), languages(&["pl"]));
        assert_eq!(wanted_of(&[("LANG", "C.UTF-8")]), languages(&[]));
        assert_eq!(wanted_of(&[("LANG", "POSIX")]), languages(&[]));
        assert_eq!(
            wanted_of(&[
                ("LANG", "pl_PL.UTF-8"),
                ("LC_MESSAGES", "de_DE@euro"),
                ("LC_ALL", "")
            ]),
            languages(&["de"]),
            "LC_MESSAGES before LANG, and one empty is not set"
        );
        assert_eq!(
            wanted_of(&[("LANGUAGE", "fr:pt-BR"), ("LC_ALL", "en_GB.UTF-8")]),
            languages(&["fr", "pt", "en"]),
            "LANGUAGE's list first"
        );
    }

    #[test]
    fn a_program_that_has_read_its_locale_is_in_its_language() {
        let polish = Words::in_locale(&TOOL, "pl_PL.UTF-8");
        assert_eq!(
            polish.say("hello", &[("name", Value::Words("Ala"))]),
            "Cześć, Ala"
        );
        let english = Words::in_locale(&TOOL, "C");
        assert_eq!(
            english.say("hello", &[("name", Value::Words("Ala"))]),
            "Hello, Ala"
        );
    }

    #[test]
    fn a_program_is_in_the_language_of_its_environment() {
        // Whatever this machine's language is, the panel has words in it,
        // or in English.
        let words = Words::of(&PANEL);
        assert_ne!(words.say("aeroplane-mode", &[]), "");
        assert!(PROGRAMS.contains(&PANEL));
    }
}
