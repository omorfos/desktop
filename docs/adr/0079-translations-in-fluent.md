# 79. Translations in Fluent

- Status: accepted
- Date: 2026-11-13

## Context

Everything omorfos says was written in English in the code. The roadmap
asked for the shell in the user's language, Polish first: the mechanism
and the English in this repository, so that CI can test every program in
every language; each language's words in files of their own, so that
translating needs no review of code; English always loaded, so that a
message a language lacks, or has wrong, is said in English, a message at
a time; and CI checking every language against the English. Which
format, Fluent or gettext, was left to this record.

## Decision

**Project Fluent, by the `fluent-bundle` crate, in a crate of our own,
`omorfos-i18n`.**

- **A file for each program in each language**: `i18n/en/panel.ftl`, and
  `i18n/pl/panel.ftl` for the same in Polish. They are built into the
  programs, so nothing is looked for when one starts, and a test fails
  while a file there is not built in, or one built in is not there.
- **The language is the one the environment asks for**, as the C library
  reads it: `LANGUAGE`'s list first, then the first of `LC_ALL`,
  `LC_MESSAGES` and `LANG` that is set, `C` and `POSIX` being English.
  The first that the program has words in is chosen.
- **English beneath it**: a message is looked up in the language chosen
  and then in English, so a translation that has not caught up says the
  rest in English. A message neither has is said by its name, which the
  tests of each program make sure never happens.
- **CI holds each translation to the English**: every file can be read
  whole; no message is in a translation that is not in English; a
  message uses no value the English one is not given (it may use fewer,
  as Polish needs no count where English needs none); and a message that
  chooses by a number has every plural form the language has for whole
  numbers, by CLDR's rules, as Polish's one, few and many.
- **Words, not marks**: Fluent's marks round what a message is given, for
  text that runs both ways, are left out, as what draws the text would
  show them.
- **A translation made by a machine says so** at its top, until someone
  who speaks the language has read it; then the line goes, and the
  language gets an owner in `CODEOWNERS`, who reviews its files.

The panel is the first program translated; the others follow, a program
at a time.

## Alternatives

- **gettext**, which translators' tools know best. Its plural rules are
  a C expression in each file's header, its placeholders are positional,
  and English is the key rather than a name, so a change of English
  wording changes every translation's key; checking a translation would
  need more of our own code. The pure-Rust `gettext` crate reads only
  compiled `.mo` files, a step more in the build.
- **Files read at run time**, from `/usr/share/omorfos/i18n`. A language
  could be added without building, but each packaging would carry
  another folder, and a program could start with words that do not match
  it.
- **Our own format.** Less to depend on, but Polish's plurals, and every
  other language's, would be ours to get right.

## Consequences

- `omorfos-i18n` brings `fluent-bundle` and what it needs, a handful of
  small crates, into every program translated.
- A message's name, not its English, is what code asks for: the English
  can be reworded without touching a translation, though a translation
  then says what the English used to until it is read again.
- What a program says in a test is its English, unless the test asks for
  another language.
- PAM's modules speak only English to the lock screen's helper: their
  translations are used once a process has set its locale, which from
  Rust needs `unsafe`, and pam_u2f has none. What the fingerprint reader
  and a security key say is said in the lock screen's own words instead,
  which loses only which finger and which reader fprintd names. Anything
  else a module says, as an account being locked, is shown as it says it.
