//! JSON Schemas for the configuration files.
//!
//! Editors with YAML language support use these for completion, hover
//! documentation and inline validation. The schemas are generated from the
//! same types that parse the files, so they cannot drift from the parser.

use std::borrow::Cow;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};

use crate::keys::RawKeys;
use crate::menu::RawItem;
use crate::rules::RawMatch;
use crate::{
    Bounded, Color, Command, General, KeyChain, Keys, MAX_WORKSPACES, Match, Menu, MenuItem,
    NamedOutput, Offset, OutputScale, Outputs, Picture, Role, Rules, ScreenMode, ScreenWallpaper,
    Screens, Speed, Started, Workspaces,
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
    format!(
        "{:#}\n",
        SchemaGenerator::default()
            .into_root_schema_for::<T>()
            .as_value()
    )
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
            "description": "A path from the root, or from the home directory as `~/`.",
            "type": "string",
            "pattern": "^(/|~/)",
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
