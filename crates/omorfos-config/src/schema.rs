//! JSON Schemas for the configuration files.
//!
//! Editors with YAML language support use these for completion, hover
//! documentation and inline validation. The schemas are generated from the
//! same types that parse the files, so they cannot drift from the parser.

use std::borrow::Cow;
use std::collections::BTreeSet;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde_json::Value;

use crate::keys::RawKeys;
use crate::menu::RawItem;
use crate::rules::RawMatch;
use crate::{
    Accent, Bounded, Color, Command, General, KeyChain, Keys, MAX_WORKSPACES, Match, Menu,
    MenuItem, NamedOutput, Offset, OutputScale, Outputs, Picture, Private, Role, Rules, ScreenMode,
    ScreenWallpaper, Screens, Speed, Started, Workspaces,
};

/// Generates a schema for each configuration file.
///
/// Returns pairs of the schema's file name, such as `keys.schema.json`, and
/// its pretty-printed JSON.
#[must_use]
pub fn schemas() -> [(&'static str, String); 4] {
    [
        ("config.schema.json", render::<General>()),
        ("keys.schema.json", render::<Keys>()),
        ("rules.schema.json", render::<Rules>()),
        ("menu.schema.json", render::<Menu>()),
    ]
}

fn render<T: JsonSchema>() -> String {
    format!("{:#}\n", root::<T>())
}

/// The schema of `T`, as JSON.
pub(crate) fn root<T: JsonSchema>() -> Value {
    SchemaGenerator::default()
        .into_root_schema_for::<T>()
        .as_value()
        .clone()
}

/// What the configuration's files promise (ADR 73): a line for each key a
/// file may have, by the path written to reach it, as
/// `config.yaml appearance.accent`, and one for each value a key may be
/// given by name, as `config.yaml appearance.accent = "cyan"`; sorted, a
/// line each. A line taken away is a promise taken away, which a commit
/// has to mark as breaking. Kept in `schemas/promised.txt` beside the
/// schemas.
#[must_use]
pub fn promised() -> String {
    let files = [
        ("config.yaml", root::<General>()),
        ("keys.yaml", root::<Keys>()),
        ("rules.yaml", root::<Rules>()),
        ("menu.yaml", root::<Menu>()),
    ];
    let mut lines = BTreeSet::new();
    for (file, schema) in &files {
        let mut promises = Promises {
            file,
            defs: schema.get("$defs"),
            within: Vec::new(),
            lines: &mut lines,
        };
        promises.walk(schema, "");
    }
    lines.into_iter().map(|line| line + "\n").collect()
}

/// A walk through a schema, writing down what it promises.
struct Promises<'a> {
    /// The file it is the schema of.
    file: &'a str,
    /// The definitions its references name.
    defs: Option<&'a Value>,
    /// The definitions the walk is within, so that one that holds itself,
    /// as a menu holds menus, is walked once.
    within: Vec<&'a str>,
    lines: &'a mut BTreeSet<String>,
}

impl<'a> Promises<'a> {
    /// Writes down what `schema` promises, at `path`, the keys to it from
    /// the file's top.
    fn walk(&mut self, schema: &'a Value, path: &str) {
        let referred = schema.get("$ref").and_then(Value::as_str);
        let named = referred.and_then(|reference| reference.strip_prefix("#/$defs/"));
        let defined = named.and_then(|name| {
            let definition = self.defs.and_then(|defs| defs.get(name));
            definition.map(|definition| (name, definition))
        });
        if let Some((name, definition)) = defined.filter(|(name, _)| !self.within.contains(name)) {
            self.within.push(name);
            self.walk(definition, path);
            self.within.pop();
        }
        let alternatives = ["anyOf", "oneOf", "allOf"].into_iter();
        let alternatives = alternatives.filter_map(|key| schema.get(key)?.as_array());
        for alternative in alternatives.flatten() {
            self.walk(alternative, path);
        }
        if let Some(items) = schema.get("items") {
            self.walk(items, &format!("{path}[]"));
        }
        // A map, as `outputs` is, from names of the user's own.
        if let Some(values) = schema
            .get("additionalProperties")
            .filter(|values| values.is_object())
        {
            self.walk(values, &format!("{path}.*"));
        }
        let properties = schema.get("properties").and_then(Value::as_object);
        for (key, property) in properties.into_iter().flatten() {
            let keyed = [path, key.as_str()].join(if path.is_empty() { "" } else { "." });
            self.lines.insert(format!("{} {keyed}", self.file));
            self.walk(property, &keyed);
        }
        let named = schema.get("const").into_iter();
        let listed = schema
            .get("enum")
            .and_then(Value::as_array)
            .into_iter()
            .flatten();
        for value in named.chain(listed) {
            self.lines.insert(format!("{} {path} = {value}", self.file));
        }
    }
}

impl<const MIN: u32, const MAX: u32> JsonSchema for Bounded<MIN, MAX> {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Integer{MIN}To{MAX}").into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({ "type": "integer", "minimum": MIN, "maximum": MAX })
    }
}

impl JsonSchema for Command {
    fn schema_name() -> Cow<'static, str> {
        "Command".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A program followed by its arguments, run without a shell.",
            "type": "array",
            "items": { "type": "string" },
            "minItems": 1,
        })
    }
}

impl JsonSchema for Started {
    fn schema_name() -> Cow<'static, str> {
        "Started".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let run = generator.subschema_for::<Command>();
        let role = generator.subschema_for::<Role>();
        json_schema!({
            "description": "A program to start with the session: its name and arguments, or that and what it may do.",
            "oneOf": [
                run,
                {
                    "type": "object",
                    "properties": {
                        "run": run,
                        "may": {
                            "description": "What it may do that an ordinary program may not.",
                            "type": "array",
                            "items": role,
                        },
                    },
                    "required": ["run"],
                    "additionalProperties": false,
                },
            ],
        })
    }
}

impl JsonSchema for KeyChain {
    fn schema_name() -> Cow<'static, str> {
        "KeyChain".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "Chords such as `Super+Return`, separated by spaces when chained.",
            "type": "string",
            "pattern": "\\S",
        })
    }
}

impl JsonSchema for Private {
    fn schema_name() -> Cow<'static, str> {
        "Private".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                {
                    "description": "Blank in pictures of the screen if true, as `blank`; shown if false.",
                    "type": "boolean",
                },
                {
                    "description": "`blank`: nothing of it can be told. `blur`: what is in it cannot be read, though its colours and larger shapes can be made out.",
                    "enum": ["blank", "blur"],
                },
            ],
        })
    }
}

impl JsonSchema for Offset {
    fn schema_name() -> Cow<'static, str> {
        "Offset".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                { "description": "Units of layout, less than nothing for left or up.", "type": "integer" },
                {
                    "description": "A share of the screen's own length, such as `-30%`.",
                    "type": "string",
                    "pattern": "^-?(100|[1-9]?[0-9])%$",
                },
            ],
        })
    }
}

impl JsonSchema for ScreenMode {
    fn schema_name() -> Cow<'static, str> {
        "ScreenMode".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "The size in pixels, such as `2560x1440`, and how many times a \
                            second it is drawn if that matters, such as `2560x1440@144`.",
            "type": "string",
            "pattern": "^[1-9][0-9]*x[1-9][0-9]*(@[1-9][0-9]*)?$",
        })
    }
}

impl JsonSchema for OutputScale {
    fn schema_name() -> Cow<'static, str> {
        "OutputScale".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "oneOf": [
                { "description": "Whatever suits the screen.", "const": "auto" },
                {
                    "description": "Pixels to a unit of layout.",
                    "type": "number",
                    "minimum": 1,
                    "maximum": 4,
                    "multipleOf": 0.25,
                },
            ],
        })
    }
}

impl JsonSchema for Speed {
    fn schema_name() -> Cow<'static, str> {
        "Speed".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "From -1, the slowest, to 1, the fastest; 0 is what the device is made for.",
            "type": "number",
            "minimum": -1,
            "maximum": 1,
        })
    }
}

impl JsonSchema for Outputs {
    fn schema_name() -> Cow<'static, str> {
        "Outputs".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let output = generator.subschema_for::<NamedOutput>();
        json_schema!({
            "description": "Screens by name, in the order they stand from the left.",
            "type": "array",
            "items": output,
        })
    }
}

impl JsonSchema for Picture {
    fn schema_name() -> Cow<'static, str> {
        "Picture".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A picture or a directory of them, the same whether the desktop is \
                dark or light, or one for each.",
            "oneOf": [
                {
                    "description": "A path from the root, or from the home directory as `~/`.",
                    "type": "string",
                    "pattern": "^(/|~/)",
                },
                {
                    "description": "One for a dark desktop and one for a light one.",
                    "type": "object",
                    "properties": {
                        "dark": { "type": "string", "pattern": "^(/|~/)" },
                        "light": { "type": "string", "pattern": "^(/|~/)" },
                    },
                    "required": ["dark", "light"],
                    "additionalProperties": false,
                },
            ],
        })
    }
}

impl JsonSchema for Accent {
    fn schema_name() -> Cow<'static, str> {
        "Accent".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let names: Vec<&str> = crate::scheme::ACCENTS
            .iter()
            .map(|(name, _)| *name)
            .collect();
        json_schema!({
            "description": "One of the accents that come with omorfos, or any colour that \
                            cannot be seen through, as `#rrggbb`.",
            "type": "string",
            "anyOf": [
                { "enum": names },
                { "pattern": "^#[0-9a-fA-F]{6}$" },
            ],
        })
    }
}

impl JsonSchema for Color {
    fn schema_name() -> Cow<'static, str> {
        "Color".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "description": "A colour as `#rrggbb`, or `#rrggbbaa` with its opacity.",
            "type": "string",
            "pattern": "^#([0-9a-fA-F]{6}|[0-9a-fA-F]{8})$",
        })
    }
}

impl JsonSchema for Screens {
    fn schema_name() -> Cow<'static, str> {
        "Screens".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let screen = generator.subschema_for::<ScreenWallpaper>();
        json_schema!({
            "description": "Screens by name, each named once.",
            "type": "array",
            "items": screen,
        })
    }
}

impl JsonSchema for Workspaces {
    fn schema_name() -> Cow<'static, str> {
        "Workspaces".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "array",
            "items": { "type": "string", "minLength": 1 },
            "minItems": 1,
            "maxItems": MAX_WORKSPACES,
        })
    }
}

impl JsonSchema for Keys {
    fn schema_name() -> Cow<'static, str> {
        RawKeys::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        RawKeys::json_schema(generator)
    }
}

impl JsonSchema for Match {
    fn schema_name() -> Cow<'static, str> {
        RawMatch::schema_name()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let mut schema = RawMatch::json_schema(generator);
        schema.insert("minProperties".to_owned(), 1.into());
        schema
    }
}

impl JsonSchema for MenuItem {
    fn schema_name() -> Cow<'static, str> {
        "MenuItem".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let entry = generator.subschema_for::<RawItem>();
        json_schema!({
            "oneOf": [
                { "description": "A dividing line.", "const": "separator" },
                entry,
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schema(name: &str) -> String {
        let schemas = schemas();
        let (_, text) = schemas.iter().find(|(file, _)| *file == name).unwrap();
        text.clone()
    }

    #[test]
    fn what_a_schema_promises_is_each_key_by_its_path_and_each_value_by_name() {
        let schema = serde_json::json!({
            "properties": {
                "look": { "$ref": "#/$defs/Look" },
                "screens": { "additionalProperties": { "properties": { "scale": {} } } },
                "free": { "additionalProperties": true },
                "nowhere": { "$ref": "#/$defs/Gone" },
                "elsewhere": { "$ref": "https://example.org/schema.json" },
            },
            "$defs": {
                "Look": {
                    "anyOf": [
                        { "enum": ["dark", "light"] },
                        { "const": 7 },
                        { "properties": { "inside": { "items": { "$ref": "#/$defs/Look" } } } },
                    ],
                },
            },
        });
        let mut lines = BTreeSet::new();
        let mut promises = Promises {
            file: "a.yaml",
            defs: schema.get("$defs"),
            within: Vec::new(),
            lines: &mut lines,
        };
        promises.walk(&schema, "");
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(
            lines,
            [
                "a.yaml elsewhere",
                "a.yaml free",
                "a.yaml look",
                "a.yaml look = \"dark\"",
                "a.yaml look = \"light\"",
                "a.yaml look = 7",
                "a.yaml look.inside",
                "a.yaml nowhere",
                "a.yaml screens",
                "a.yaml screens.*.scale",
            ],
            "a look within a look is walked once"
        );
    }

    #[test]
    fn the_configuration_promises_its_keys_and_named_values() {
        let promised = promised();
        for line in [
            "config.yaml appearance.accent\n",
            "config.yaml appearance.accent = \"cyan\"\n",
            "config.yaml outputs[].scale\n",
            "keys.yaml bindings[].keys\n",
            "keys.yaml bindings[].action = \"close\"\n",
            "rules.yaml rules[].private = \"blur\"\n",
            "menu.yaml items[].label\n",
        ] {
            assert!(promised.contains(line), "{line:?} in:\n{promised}");
        }
    }

    #[test]
    fn there_is_one_schema_per_configuration_file_written_in_yaml() {
        let names = schemas().map(|(name, _)| name.replace(".schema.json", ".yaml"));
        let files = crate::FILES.into_iter();
        let yaml: Vec<&str> = files.filter(|name| *name != "theme.css").collect();
        assert_eq!(names.as_slice(), yaml);
    }

    #[test]
    fn schemas_are_pretty_printed_json_objects_ending_in_a_newline() {
        for (name, text) in schemas() {
            assert!(text.starts_with("{\n  \""), "{name}: {text}");
            assert!(text.ends_with("}\n"), "{name}");
        }
    }

    #[test]
    fn settings_are_documented_for_editor_hover() {
        assert!(
            schema("config.schema.json").contains("\"description\": \"How windows gain focus.\"")
        );
    }

    #[test]
    fn value_constraints_are_part_of_the_schema() {
        let config = schema("config.schema.json");
        assert!(config.contains("\"maximum\": 128"), "{config}");
        assert!(config.contains("\"maxItems\": 32"));
        assert!(schema("rules.schema.json").contains("\"minProperties\": 1"));
        assert!(schema("keys.schema.json").contains("\"minItems\": 1"));
        assert!(schema("keys.schema.json").contains("\"pattern\": \"\\\\S\""));
    }

    #[test]
    fn menus_nest_through_a_reference() {
        let menu = schema("menu.schema.json");
        assert!(menu.contains("\"const\": \"separator\""), "{menu}");
        assert!(menu.contains("\"$ref\": \"#/$defs/MenuItem\""), "{menu}");
    }
}
