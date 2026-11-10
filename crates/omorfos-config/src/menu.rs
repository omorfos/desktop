//! `menu.yaml`: the root menu.

use std::fmt;

use serde::Deserialize;
use serde::de::value::MapAccessDeserializer;
use serde::de::{Deserializer, Error, MapAccess, Unexpected, Visitor};

use crate::{Action, Command};

/// The contents of `menu.yaml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Menu {
    /// The entries of the root menu, top to bottom.
    pub items: Vec<MenuItem>,
}

impl Default for Menu {
    fn default() -> Self {
        let entry = |label: &str, action| MenuItem::Entry {
            label: label.to_owned(),
            action,
            icon: None,
        };
        Self {
            items: vec![
                entry("Terminal", Action::Spawn(Command::terminal().into())),
                entry("Run\u{2026}", Action::ShowLauncher),
                MenuItem::Separator,
                entry("Stay awake", Action::StayAwake),
                entry("Reload configuration", Action::ReloadConfig),
                entry("About omorfos", Action::About),
                entry("Quit", Action::Quit),
            ],
        }
    }
}

/// One line of a menu.
///
/// In YAML an item is the word `separator`, or a mapping with a `label` and
/// either an `action` (an entry) or nested `items` (a submenu), and an
/// `icon` if the one chosen for it is not wanted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    /// Runs an action when chosen.
    Entry {
        /// The text shown.
        label: String,
        /// What choosing the entry does.
        action: Action,
        /// What stands before the text in place of what its action is
        /// shown by: the name of an icon of the icon theme, or the path of
        /// a picture.
        icon: Option<String>,
    },
    /// Opens a nested menu.
    Submenu {
        /// The text shown.
        label: String,
        /// The entries of the nested menu.
        items: Vec<Self>,
        /// What stands before the text: the name of an icon of the icon
        /// theme, or the path of a picture. Nothing, unless said.
        icon: Option<String>,
    },
    /// A dividing line.
    Separator,
}

/// A menu mapping as written, before checking it is an entry or a submenu.
#[derive(Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(rename = "MenuEntry"))]
#[serde(deny_unknown_fields)]
pub(crate) struct RawItem {
    /// The text shown.
    label: String,
    /// What choosing the item does. Give either this or `items`.
    #[serde(default)]
    action: Option<Action>,
    /// A nested menu. Give either this or `action`.
    #[serde(default)]
    items: Option<Vec<MenuItem>>,
    /// What stands before the label in place of what is chosen for it:
    /// the name of an icon of the icon theme, or the path of a picture.
    #[serde(default)]
    icon: Option<String>,
}

impl<'de> Deserialize<'de> for MenuItem {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(MenuItemVisitor)
    }
}

struct MenuItemVisitor;

impl<'de> Visitor<'de> for MenuItemVisitor {
    type Value = MenuItem;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("`separator` or a mapping with a `label` and either an `action` or `items`")
    }

    fn visit_str<E: Error>(self, word: &str) -> Result<Self::Value, E> {
        if word == "separator" {
            Ok(MenuItem::Separator)
        } else {
            Err(E::invalid_value(Unexpected::Str(word), &self))
        }
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        let raw = RawItem::deserialize(MapAccessDeserializer::new(map))?;
        match (raw.action, raw.items) {
            (Some(action), None) => Ok(MenuItem::Entry {
                label: raw.label,
                action,
                icon: raw.icon,
            }),
            (None, Some(items)) => Ok(MenuItem::Submenu {
                label: raw.label,
                items,
                icon: raw.icon,
            }),
            _ => Err(A::Error::custom(format_args!(
                "menu item `{}` needs exactly one of `action` or `items`",
                raw.label
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Result<Menu, String> {
        serde_saphyr::from_str(yaml).map_err(|error| error.without_snippet().to_string())
    }

    #[test]
    fn the_built_in_menu_starts_programs_keeps_the_session_awake_says_what_it_is_and_ends_it() {
        let menu = Menu::default();
        let labels: Vec<&str> = menu
            .items
            .iter()
            .filter_map(|item| match item {
                MenuItem::Entry { label, .. } => Some(label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            labels,
            [
                "Terminal",
                "Run\u{2026}",
                "Stay awake",
                "Reload configuration",
                "About omorfos",
                "Quit"
            ]
        );
        assert_eq!(menu.items[2], MenuItem::Separator);
    }

    #[test]
    fn items_are_entries_separators_and_nested_submenus_with_an_icon_if_one_is_given() {
        let yaml = "
items:
  - label: Terminal
    action: { spawn: [foot] }
  - separator
  - label: Session
    icon: system-shutdown
    items:
      - { label: Quit, action: quit, icon: /usr/share/pixmaps/quit.png }
";
        let menu = parse(yaml).unwrap();
        let quit = MenuItem::Entry {
            label: "Quit".to_owned(),
            action: Action::Quit,
            icon: Some("/usr/share/pixmaps/quit.png".to_owned()),
        };
        assert_eq!(menu.items.len(), 3);
        let terminal = MenuItem::Entry {
            label: "Terminal".to_owned(),
            action: Action::Spawn(Command::terminal().into()),
            icon: None,
        };
        assert_eq!(menu.items[0], terminal, "an icon only if one is given");
        assert_eq!(menu.items[1], MenuItem::Separator);
        assert_eq!(
            menu.items[2],
            MenuItem::Submenu {
                label: "Session".to_owned(),
                items: vec![quit],
                icon: Some("system-shutdown".to_owned()),
            }
        );
    }

    #[test]
    fn an_item_with_both_an_action_and_items_is_rejected() {
        let error = parse("items:\n  - { label: Both, action: quit, items: [] }\n").unwrap_err();
        assert!(
            error.contains("menu item `Both` needs exactly one of `action` or `items`"),
            "{error}"
        );
    }

    #[test]
    fn an_item_with_neither_an_action_nor_items_is_rejected() {
        let error = parse("items:\n  - label: Neither\n").unwrap_err();
        assert!(
            error.contains("menu item `Neither` needs exactly one"),
            "{error}"
        );
    }

    #[test]
    fn an_unknown_word_is_rejected() {
        let error = parse("items:\n  - divider\n").unwrap_err();
        assert!(error.contains("`separator` or a mapping"), "{error}");
    }

    #[test]
    fn a_value_of_the_wrong_type_is_rejected() {
        let error = parse("items:\n  - [nested]\n").unwrap_err();
        assert!(error.contains("`separator` or a mapping"), "{error}");
    }
}
