//! Holds every language's words to the English (ADR 79): each file there
//! is is built in and can be read whole; a translation has no message the
//! English has not, gives none a value the English is not given, and
//! where a message chooses by how many, has every form the language has.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use fluent_syntax::ast::{
    Entry, Expression, InlineExpression, Pattern, PatternElement, VariantKey,
};
use fluent_syntax::parser;
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use omorfos_i18n::PROGRAMS;

/// What a test needs to know of a message: the values it is given, and,
/// for each choice it makes by a number, the forms it has.
#[derive(Debug, Default)]
struct Said {
    values: BTreeSet<String>,
    plurals: Vec<BTreeSet<String>>,
}

/// Every message of `text`, by name, read whole.
fn messages(text: &str, file: &str) -> Vec<(String, Said)> {
    let resource = parser::parse(text);
    assert!(
        resource.is_ok(),
        "{file} cannot be read whole: {resource:?}"
    );
    let resource = resource.unwrap_or_else(|(resource, _)| resource);
    let mut messages = Vec::new();
    for entry in resource.body {
        if let Entry::Message(message) = entry {
            let mut said = Said::default();
            if let Some(value) = &message.value {
                pattern(value, &mut said);
            }
            messages.push((message.id.name.to_owned(), said));
        }
    }
    messages
}

/// Notes what `pattern` is given and how it chooses.
fn pattern(pattern: &Pattern<&str>, said: &mut Said) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            self::expression(expression, said);
        }
    }
}

fn expression(expression: &Expression<&str>, said: &mut Said) {
    match expression {
        Expression::Inline(inline) => self::inline(inline, said),
        Expression::Select { selector, variants } => {
            inline(selector, said);
            let keys: BTreeSet<String> = variants
                .iter()
                .map(|variant| match &variant.key {
                    VariantKey::Identifier { name } => (*name).to_owned(),
                    VariantKey::NumberLiteral { value } => (*value).to_owned(),
                })
                .collect();
            let plural = ["zero", "one", "two", "few", "many", "other"];
            if keys
                .iter()
                .all(|key| plural.contains(&key.as_str()) || key.parse::<f64>().is_ok())
            {
                said.plurals.push(keys);
            }
            for variant in variants {
                pattern(&variant.value, said);
            }
        }
    }
}

fn inline(inline: &InlineExpression<&str>, said: &mut Said) {
    match inline {
        InlineExpression::VariableReference { id } => {
            said.values.insert(id.name.to_owned());
        }
        InlineExpression::Placeable { expression } => self::expression(expression, said),
        _ => {}
    }
}

/// The plural forms `language` has for whole numbers, which are all a
/// message is given.
fn forms(language: &str) -> Result<BTreeSet<String>, String> {
    let language: unic_langid::LanguageIdentifier =
        language.parse().map_err(|error| format!("{error}"))?;
    let rules = PluralRules::create(language, PluralRuleType::CARDINAL)?;
    let mut forms = BTreeSet::new();
    for number in 0..=1000_u32 {
        let form = match rules.select(number)? {
            PluralCategory::ZERO => "zero",
            PluralCategory::ONE => "one",
            PluralCategory::TWO => "two",
            PluralCategory::FEW => "few",
            PluralCategory::MANY => "many",
            PluralCategory::OTHER => "other",
        };
        forms.insert(form.to_owned());
    }
    Ok(forms)
}

#[test]
fn every_file_of_words_is_built_in_and_every_one_built_in_is_a_file() {
    let top = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../i18n");
    let mut on_disk = BTreeSet::new();
    for language in fs::read_dir(&top).unwrap() {
        let language = language.unwrap();
        for file in fs::read_dir(language.path()).unwrap() {
            let file = file.unwrap();
            let text = fs::read_to_string(file.path()).unwrap();
            let name = file.file_name().to_string_lossy().into_owned();
            on_disk.insert((
                language.file_name().to_string_lossy().into_owned(),
                name,
                text,
            ));
        }
    }
    let mut built_in = BTreeSet::new();
    for program in PROGRAMS {
        let file = format!("{}.ftl", program.name);
        built_in.insert(("en".to_owned(), file.clone(), program.english.to_owned()));
        for (language, text) in program.translated {
            built_in.insert(((*language).to_owned(), file.clone(), (*text).to_owned()));
        }
    }
    assert_eq!(on_disk, built_in);
}

#[test]
fn every_translation_says_only_what_the_english_says_with_what_it_is_given() {
    for program in PROGRAMS {
        let english = messages(program.english, &format!("en/{}.ftl", program.name));
        for said in english.iter().flat_map(|(_, said)| &said.plurals) {
            assert!(
                said.is_superset(&forms("en").unwrap()),
                "{}: {said:?}",
                program.name
            );
        }
        for (language, text) in program.translated {
            let file = format!("{language}/{}.ftl", program.name);
            for (name, said) in messages(text, &file) {
                let original = english.iter().find(|(english, _)| *english == name);
                let original = original.map(|(_, said)| said);
                assert!(original.is_some(), "{file}: `{name}` is not in English");
                let given = original.map(|said| &said.values);
                assert!(
                    given.is_some_and(|given| said.values.is_subset(given)),
                    "{file}: `{name}` uses {:?}, and is given {given:?}",
                    said.values
                );
                let forms = forms(language).unwrap();
                for keys in &said.plurals {
                    assert!(
                        keys.is_superset(&forms),
                        "{file}: `{name}` has {keys:?} of {forms:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_checks_find_what_they_are_for() {
    let polish = forms("pl").unwrap();
    assert_eq!(polish, ["few", "many", "one"].map(str::to_owned).into());
    let said = messages(
        "files = { $count ->\n    [one] { $count } plik\n   *[other] { $count } plików\n}\nname = { $who }\n",
        "test",
    );
    assert_eq!(
        said[0].1.plurals,
        [["one", "other"].map(str::to_owned).into()]
    );
    assert!(!said[0].1.plurals[0].is_superset(&polish), "few is missing");
    assert_eq!(said[1].1.values, ["who".to_owned()].into());
    // A choice by anything else is not a choice of plural forms.
    let kinds = messages(
        "radio = { $kind ->\n    [wlan] Wi-Fi\n   *[other] A radio\n}\n",
        "test",
    );
    assert_eq!(kinds[0].1.plurals, Vec::<BTreeSet<String>>::new());
}
