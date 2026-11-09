# Credits

omorfos is under the [MIT licence](LICENSE). It is drawn with, and
recommends, the work of others. This file names all of it: whatever is
built into omorfos or shipped in its packages is listed here before it
goes in, and each licence's text is carried by the packages.

The Rust crates omorfos is built from are listed, with their licences,
by `cargo deny list`, and are checked against the licences allowed in
[deny.toml](deny.toml). As it is packaged, `cargo about` gathers the text
of each into `LICENSE-crates`, with the crates under it, which the
packages carry beside the others.

## Built in

| What | By | Licence | Used for |
|---|---|---|---|
| The [time zone database](https://www.iana.org/time-zones)'s `zone1970.tab`, from tzdata 2026f | Paul Eggert and the tz contributors | Public domain | Where the principal city of each time zone is, for `appearance.location` to name a time zone and for the system's to be where the sun is watched |
| [Lucide](https://lucide.dev) icons | The Lucide contributors; parts derive from [Feather](https://feathericons.com) by Cole Bemis | ISC; the parts from Feather, MIT | The signs of the interface: only the ones used, as shapes. Chosen in [ADR 32](docs/adr/0032-the-design-language.md); none is built in yet. |

## Used to make what is shipped

| What | By | Licence | Used for |
|---|---|---|---|
| [Space Grotesk](https://github.com/floriankarsten/space-grotesk) | Florian Karsten and the Space Grotesk project authors | SIL Open Font Licence 1.1, in [LICENSE-space-grotesk](LICENSE-space-grotesk) | The name beside the mark, drawn once as outlines; the font itself is neither shipped nor needed |

omorfos's own pictures, its mark, its name as drawn, its icon and its
wallpapers, are under [CC BY-SA 4.0](LICENSE-pictures), and the
[brand](docs/BRAND.md) says how they are made.

## Recommended by the packages, not shipped

| What | By | Licence | Used for |
|---|---|---|---|
| [Inter](https://rsms.me/inter/) | Rasmus Andersson and the Inter project authors | SIL Open Font Licence 1.1 | The font of the interface |
| [JetBrains Mono](https://www.jetbrains.com/lp/mono/) | JetBrains and the JetBrains Mono project authors | SIL Open Font Licence 1.1 | The monospaced font |
| Adwaita icons | The GNOME project | CC BY-SA 3.0 or LGPL 3 | The icons programs name, unless another theme is chosen |

## Thanks

omorfos stands on [Smithay](https://github.com/Smithay/smithay).
