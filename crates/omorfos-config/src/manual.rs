//! The manual pages of the files a user writes, in section 5: each key of
//! the four YAML files, by the path written to reach it, with what its
//! schema says of it, and the parts and states of the theme. Made from the
//! same schemas as editors are given, so that the pages cannot say other
//! than the parser does; and kept in `man/` by a test that makes them
//! again.

use std::collections::BTreeMap;

use omorfos_ipc::manual::Page;
use serde_json::Value;

use crate::schema::root;
use crate::{General, Keys, Menu, Part, Rules, State};

/// The pages every page of the configuration points to, at its end.
const SEE_ALSO: &[&str] = &[
    "omorfos(1)",
    "omorfosctl(1)",
    "omorfos-config(5)",
    "omorfos-keys(5)",
    "omorfos-rules(5)",
    "omorfos-menu(5)",
    "omorfos-theme(5)",
];

/// The example a page ends with, for each file: those of the guide, which
/// a test holds valid, `config.yaml`'s at every default but the names of
/// the workspaces, and the theme's the built-in one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Examples<'a> {
    /// `config.yaml`.
    pub config: &'a str,
    /// `keys.yaml`.
    pub keys: &'a str,
    /// `rules.yaml`.
    pub rules: &'a str,
    /// `menu.yaml`.
    pub menu: &'a str,
    /// `theme.css`.
    pub theme: &'a str,
}

/// A YAML file a user writes, and what its page says besides its keys.
struct Written {
    /// The page's name, as `omorfos-config`.
    name: &'static str,
    /// The file's name, as `config.yaml`.
    file: &'static str,
    /// What it is, in the page's name line.
    what: &'static str,
    /// What it holds, for the page's description.
    holds: &'static str,
    /// What is said of its example first; nothing for nothing.
    shown: &'static str,
}

/// The pages of the YAML files, in the order the guide has them.
const FILES: [Written; 4] = [
    Written {
        name: "omorfos-config",
        file: "config.yaml",
        what: "config.yaml, how the omorfos desktop behaves",
        holds: "How the desktop behaves, but for its keys, rules and menu: how it looks whatever \
                the theme, the screens, the keyboard, touchpad and mouse, workspaces, the panel \
                and the other parts of the desktop, and what starts with the session.",
        shown: "Every key at its default, but for the names of the workspaces:",
    },
    Written {
        name: "omorfos-keys",
        file: "keys.yaml",
        what: "keys.yaml, the keys of the omorfos desktop and what they do",
        holds: "The keys bound to actions, beside the built-in bindings, and the built-in \
                bindings done without. `omorfosctl action` lists every action, with what it \
                takes and what it does.",
        shown: "Three keys bound, one of them in place of a built-in binding, and one built-in \
                binding done without:",
    },
    Written {
        name: "omorfos-rules",
        file: "rules.yaml",
        what: "rules.yaml, how the omorfos desktop treats particular windows",
        holds: "Rules for particular windows, by the program they are of or their title: the \
                workspace they open on, whether they open maximized, fullscreen, shaded, kept \
                above or on every workspace, with or without a frame, and whether they are \
                private, veiled in pictures of the screen.",
        shown: "",
    },
    Written {
        name: "omorfos-menu",
        file: "menu.yaml",
        what: "menu.yaml, the root menu of the omorfos desktop",
        holds: "The root menu, which the `show_menu` action opens at the pointer: its entries, \
                each a label and an action, or a menu of its own.",
        shown: "",
    },
];

/// The page of each file, as the file name it is kept under in `man/`, as
/// `omorfos-config.5`, and the page.
#[must_use]
pub fn pages(examples: &Examples<'_>) -> [(String, String); 5] {
    let [config, keys, rules, menu] = FILES;
    [
        keyed(&config, &root::<General>(), examples.config),
        keyed(&keys, &root::<Keys>(), examples.keys),
        keyed(&rules, &root::<Rules>(), examples.rules),
        keyed(&menu, &root::<Menu>(), examples.menu),
        ("omorfos-theme.5".to_owned(), theme(examples.theme)),
    ]
}

/// The page of a YAML file, from its `schema`: where it is read from, what
/// it holds, each key it may have and what it may be given, and `example`.
fn keyed(file: &Written, schema: &Value, example: &str) -> (String, String) {
    let mut page = Page::new(file.name, 5, file.what);
    page.section("Synopsis");
    page.paragraph(&format!(
        "`$XDG_CONFIG_HOME/omorfos/{0}`, usually `~/.config/omorfos/{0}`",
        file.file
    ));
    page.section("Description");
    page.paragraph(file.holds);
    page.paragraph(
        "It is read again as soon as it is saved. If it has a mistake, its contents before \
         are kept, and the mistake is shown among the notifications and printed by \
         `omorfosctl config check`, with its line and column; the other files still apply.",
    );
    page.section("Keys");
    let mut entries = Entries::default();
    entries.walk(schema, &schema["$defs"], "", &mut Vec::new());
    for (path, key) in &entries.0 {
        page.entry(path, &key.said());
        if !key.values.is_empty() {
            page.values(&key.values);
        }
    }
    page.section("Example");
    if !file.shown.is_empty() {
        page.paragraph(file.shown);
    }
    page.verbatim(example);
    page.see_also(&others(file.name));
    (format!("{}.5", file.name), page.finish())
}

/// The page of `theme.css`: what it is, the parts and states a rule may
/// name, and the built-in theme, `built_in`, which tells of every part.
fn theme(built_in: &str) -> String {
    let name = "omorfos-theme";
    let mut page = Page::new(name, 5, "theme.css, how the omorfos desktop looks");
    page.section("Synopsis");
    page.paragraph("`$XDG_CONFIG_HOME/omorfos/theme.css`, usually `~/.config/omorfos/theme.css`");
    page.section("Description");
    page.paragraph(
        "The theme is a stylesheet, in a small part of CSS. A rule names parts of the desktop, \
         perhaps in some state, and gives them properties. Yours is laid over the built-in \
         theme, so you only write what you want changed. A rule of yours always wins over a \
         built-in one; among your own, the one that names more states wins, and then the later \
         one. It is read again as soon as it is saved, and kept as it was before if it has a \
         mistake.",
    );
    page.section("Parts");
    let parts: Vec<&str> = Part::ALL.iter().map(|part| part.name()).collect();
    page.paragraph(&format!("`{}`", parts.join("`, `")));
    page.section("States");
    let states: Vec<String> = State::ALL
        .iter()
        .map(|state| format!(":{}", state.name()))
        .collect();
    page.paragraph(&format!("`{}`", states.join("`, `")));
    page.section("The built-in theme");
    page.paragraph("Every part there is:");
    page.verbatim(built_in);
    page.see_also(&others(name));
    page.finish()
}

/// The pages to read with the page `name`.
fn others(name: &str) -> Vec<&'static str> {
    let own = format!("{name}(5)");
    SEE_ALSO
        .iter()
        .copied()
        .filter(|other| *other != own)
        .collect()
}

/// What `schema` says it is, in a line; nothing where it says nothing.
fn described(schema: &Value) -> String {
    let description = schema.get("description").and_then(Value::as_str);
    description.unwrap_or_default().to_owned()
}

/// What a page says of one key.
#[derive(Debug, Default)]
struct Key {
    /// What it is.
    description: String,
    /// What its value may be, in a sentence, as "From 0 to 86400.".
    bounds: String,
    /// What it is when it is not given, if the schema says.
    default: String,
    /// The values it may be given by name, and what each means.
    values: Vec<(String, String)>,
}

impl Key {
    /// What is said of it beside its path.
    fn said(&self) -> String {
        let said = [&self.description, &self.bounds, &self.default];
        let said = said.into_iter().filter(|text| !text.is_empty());
        said.map(String::as_str).collect::<Vec<_>>().join(" ")
    }
}

/// Every key of a file, by its path from the file's top.
#[derive(Debug, Default)]
struct Entries(BTreeMap<String, Key>);

impl Entries {
    /// Notes what `schema` says of the key at `path`, and walks what is
    /// under it, as `schema::promised` does; `within` the definitions it is
    /// in, so that one that holds itself, as a menu holds menus, is walked
    /// once.
    fn walk<'a>(
        &mut self,
        schema: &'a Value,
        defs: &'a Value,
        path: &str,
        within: &mut Vec<&'a str>,
    ) {
        let referred = schema.get("$ref").and_then(Value::as_str);
        let named = referred.and_then(|reference| reference.strip_prefix("#/$defs/"));
        let defined = named.and_then(|name| defs.get(name).map(|definition| (name, definition)));
        if let Some((name, definition)) = defined.filter(|(name, _)| !within.contains(name)) {
            within.push(name);
            if let Some(key) = self
                .0
                .get_mut(path)
                .filter(|key| key.description.is_empty())
            {
                key.description = described(definition);
            }
            self.walk(definition, defs, path, within);
            within.pop();
        }
        let alternatives = ["anyOf", "oneOf", "allOf"].into_iter();
        let alternatives = alternatives.filter_map(|key| schema.get(key)?.as_array());
        for alternative in alternatives.flatten() {
            self.walk(alternative, defs, path, within);
        }
        if let Some(items) = schema.get("items") {
            self.walk(items, defs, &format!("{path}[]"), within);
        }
        // A map, as `outputs` is, from names of the user's own, each a key.
        if let Some(values) = schema
            .get("additionalProperties")
            .filter(|values| values.is_object())
        {
            let named = format!("{path}.*");
            self.0.entry(named.clone()).or_default().description = described(values);
            self.walk(values, defs, &named, within);
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        for (name, property) in properties.into_iter().flatten() {
            let keyed = [path, name.as_str()].join(if path.is_empty() { "" } else { "." });
            let key = self.0.entry(keyed.clone()).or_default();
            key.description = described(property);
            self.walk(property, defs, &keyed, within);
        }
        // What each item of a list may be is said of the list.
        if let Some(key) = self.0.get_mut(path.trim_end_matches("[]")) {
            key.note(schema);
        }
    }
}

impl Key {
    /// Notes what `schema`, one of the shapes the key's value may take,
    /// says it may be: a value by name, bounds, or a default.
    fn note(&mut self, schema: &Value) {
        // A value by itself says what it means; a list of them, as `enum`
        // is, is described as a whole, which the key says already.
        if let Some(value) = schema.get("const") {
            self.values.push((plain(value), described(schema)));
        }
        let listed = schema.get("enum").and_then(Value::as_array);
        for value in listed.into_iter().flatten() {
            self.values.push((plain(value), String::new()));
        }
        let bound = |name: &str| schema.get(name).map(plain);
        let bounds = match (bound("minimum"), bound("maximum")) {
            (Some(least), Some(most)) => format!("From {least} to {most}."),
            (Some(least), None) => format!("At least {least}."),
            (None, Some(most)) => format!("At most {most}."),
            (None, None) => String::new(),
        };
        if self.bounds.is_empty() {
            self.bounds = bounds;
        }
        // A key left out, which is what `null` is, needs no saying.
        let default = schema.get("default").filter(|default| !default.is_null());
        if let Some(default) = default.filter(|_| self.default.is_empty()) {
            self.default = format!("By default `{}`.", plain(default));
        }
    }
}

/// `value` as it is written in YAML: a word bare, anything else, nothing
/// written among it, as JSON, which YAML reads the same.
fn plain(value: &Value) -> String {
    let word = value.as_str().filter(|word| !word.is_empty());
    word.map_or_else(|| value.to_string(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn keys(schema: &Value) -> Entries {
        let mut entries = Entries::default();
        entries.walk(schema, &schema["$defs"], "", &mut Vec::new());
        entries
    }

    #[test]
    fn each_key_is_told_by_its_path_with_what_it_may_be_and_its_default() {
        let schema = json!({
            "description": "A file.",
            "properties": {
                "size": { "description": "How large.", "type": "integer", "minimum": 0, "maximum": 9, "default": 3 },
                "few": { "description": "How few.", "type": "integer", "maximum": 4 },
                "many": { "description": "How many.", "type": "integer", "minimum": 1 },
                "plain": { "description": "Plain.", "type": "string", "default": null },
                "kept": { "$ref": "#/$defs/Kept" },
                "places": { "type": "array", "items": { "$ref": "#/$defs/Place" } },
                "may": { "description": "What it may do.", "type": "array", "items": { "enum": ["panel"] } },
                "named": { "type": "object", "additionalProperties": { "type": "boolean", "description": "On or off." } },
            },
            "$defs": {
                "Kept": {
                    "description": "Where it goes.",
                    "oneOf": [
                        { "const": "file", "description": "In a file." },
                        { "const": "both", "description": "Both." },
                    ],
                },
                "Place": {
                    "description": "A place, holding places.",
                    "properties": {
                        "inner": { "description": "More of them.", "$ref": "#/$defs/Place" },
                        "side": { "enum": ["left", "right"], "description": "The side." },
                    },
                },
            },
        });
        let keys = keys(&schema);
        let said = |path: &str| keys.0.get(path).map(Key::said);
        let paths: Vec<&str> = keys.0.keys().map(String::as_str).collect();
        assert_eq!(
            paths,
            [
                "few",
                "kept",
                "many",
                "may",
                "named",
                "named.*",
                "places",
                "places[].inner",
                "places[].side",
                "plain",
                "size"
            ]
        );
        assert_eq!(
            said("size").as_deref(),
            Some("How large. From 0 to 9. By default `3`.")
        );
        assert_eq!(said("few").as_deref(), Some("How few. At most 4."));
        assert_eq!(said("many").as_deref(), Some("How many. At least 1."));
        assert_eq!(said("plain").as_deref(), Some("Plain."));
        // Told by its definition where the key says nothing of itself.
        assert_eq!(said("kept").as_deref(), Some("Where it goes."));
        let kept = &keys.0["kept"].values;
        assert_eq!(kept[0], ("file".to_owned(), "In a file.".to_owned()));
        let side = &keys.0["places[].side"].values;
        assert_eq!(
            side,
            &[
                ("left".to_owned(), String::new()),
                ("right".to_owned(), String::new())
            ]
        );
        assert_eq!(said("places[].inner").as_deref(), Some("More of them."));
        assert_eq!(said("named.*").as_deref(), Some("On or off."));
        assert_eq!(said("places").as_deref(), Some(""));
        assert_eq!(keys.0["may"].values, [("panel".to_owned(), String::new())]);
        assert_eq!(plain(&json!([1, "a"])), "[1,\"a\"]");
        assert_eq!(plain(&json!("")), "\"\"");
    }

    #[test]
    fn every_file_has_a_page_with_its_keys_or_parts_and_its_example() {
        let examples = Examples {
            config: "workspaces: []\n",
            keys: "bindings: []\n",
            rules: "rules: []\n",
            menu: "- label: Terminal\n",
            theme: "* { font-size: 11pt; }\n",
        };
        let pages = pages(&examples);
        let names: Vec<&str> = pages.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            [
                "omorfos-config.5",
                "omorfos-keys.5",
                "omorfos-rules.5",
                "omorfos-menu.5",
                "omorfos-theme.5"
            ]
        );
        let config = &pages[0].1;
        assert!(config.contains(".SH KEYS\n"), "{config}");
        assert!(config.contains("\n\\fBappearance.accent\\fR\n"), "{config}");
        assert!(
            config.contains("\nEvery key at its default, but for the names of the workspaces:\n"),
            "{config}"
        );
        assert!(config.contains("\nworkspaces: []\n"), "{config}");
        assert!(
            !config.contains("\\fBomorfos\\-config\\fR(5)"),
            "it points not to itself"
        );
        let rules = &pages[2].1;
        assert!(
            rules.contains(".SH EXAMPLE\n.PP\n.nf\n"),
            "nothing said first: {rules}"
        );
        let theme = &pages[4].1;
        assert!(theme.contains("\\fBswitcher\\-window\\fR"), "{theme}");
        assert!(theme.contains("\\fB:hover\\fR"), "{theme}");
        assert!(theme.contains("\n* { font\\-size: 11pt; }\n"), "{theme}");
    }
}
