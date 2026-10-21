# Credits

omorfos is under the [MIT licence](LICENSE). It is drawn with, and
recommends, the work of others. This file names all of it: whatever is
built into omorfos or shipped in its packages is listed here before it
goes in, and each licence's text is carried by the packages.

The Rust crates omorfos is built from are listed, with their licences,
by `cargo deny list`, and are checked against the licences allowed in
[deny.toml](deny.toml).

## Built in

| What | By | Licence | Used for |
|---|---|---|---|
| [Lucide](https://lucide.dev) icons | The Lucide contributors; parts derive from [Feather](https://feathericons.com) by Cole Bemis | ISC; the parts from Feather, MIT | The signs of the interface: only the ones used, as shapes. Chosen in [ADR 32](docs/adr/0032-the-design-language.md); none is built in yet. |

## Recommended by the packages, not shipped

| What | By | Licence | Used for |
|---|---|---|---|
| [Inter](https://rsms.me/inter/) | Rasmus Andersson and the Inter project authors | SIL Open Font Licence 1.1 | The font of the interface |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/) | JetBrains and the JetBrains Mono project authors | SIL Open Font Licence 1.1 | The monospaced font |
| Adwaita cursors | The GNOME project | CC BY-SA 3.0 or LGPL 3 | The pointer, until omorfos has its own |

## Thanks

omorfos stands on [Smithay](https://github.com/Smithay/smithay).
