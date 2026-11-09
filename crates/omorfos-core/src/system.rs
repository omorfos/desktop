//! What the computer omorfos runs on is, for "About" to show and to copy:
//! made from what the kernel and the system say of it, given here as the
//! text of their files, so that it is told the same way whatever machine
//! it is worked out on. Nothing in it tells one machine from another of
//! its kind: no name of the machine or of its user, no serial number.

/// A graphics card, as the kernel lists it: the text of its files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Card {
    /// Its name, as `card0`, by which cards are put in order.
    pub name: String,
    /// The number of its maker, as `0x8086`; empty where it is not on a
    /// PCI bus, as a card built into a phone's chip is not.
    pub vendor: String,
    /// The number of the card, as its maker numbers it.
    pub device: String,
    /// The driver that drives it, as `i915`.
    pub driver: String,
}

/// A disk, as the kernel lists it: the text of its files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Disk {
    /// Its name, as `nvme0n1`.
    pub name: String,
    /// `1` for one that can be taken out, as a USB stick can.
    pub removable: String,
    /// How large it is, in sectors of 512 bytes.
    pub size: String,
    /// What it is, as its maker calls it.
    pub model: String,
}

/// A screen, as the compositor drives it.
#[derive(Debug, Clone, PartialEq)]
pub struct Monitor {
    /// The name of the connector, as `eDP-1`.
    pub name: String,
    /// How many pixels across and down it is driven at.
    pub pixels: (i32, i32),
    /// How many pictures it shows a second, in thousandths; none if the
    /// backend does not say.
    pub refresh: i32,
    /// How many pixels it draws to a unit of layout.
    pub scale: f64,
}

/// What the kernel and the system say of the computer, as read from them.
/// What could not be read is empty, and is left out of what is shown.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Said {
    /// `/proc/cpuinfo`.
    pub cpuinfo: String,
    /// The name of the processor's architecture, as `x86_64`, for one whose
    /// `cpuinfo` does not name it.
    pub architecture: String,
    /// `/proc/meminfo`.
    pub meminfo: String,
    /// The graphics cards.
    pub cards: Vec<Card>,
    /// The list of the numbers of PCI devices and their names, `pci.ids`.
    pub pci_ids: String,
    /// The disks.
    pub disks: Vec<Disk>,
    /// The version of the kernel, `/proc/sys/kernel/osrelease`.
    pub kernel: String,
    /// What the system is, `/etc/os-release`.
    pub os_release: String,
    /// The screens.
    pub screens: Vec<Monitor>,
}

/// What the computer is, as it is shown: each kind of thing with what
/// there is of it, a line each, those of which nothing is known left out.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct System {
    /// Each kind of thing, as "Processor", with a line for each there is.
    pub things: Vec<(&'static str, Vec<String>)>,
}

impl System {
    /// What the computer is, from what was `said` of it.
    #[must_use]
    pub fn of(said: &Said) -> Self {
        // A card's connectors and the node it is drawn through are listed
        // beside it, as `card0-eDP-1` and `renderD128`.
        let a_card = |card: &&Card| {
            let number = card.name.strip_prefix("card").unwrap_or_default();
            !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
        };
        let mut cards: Vec<&Card> = said.cards.iter().filter(a_card).collect();
        cards.sort_by(|one, other| one.name.cmp(&other.name));
        let mut disks: Vec<&Disk> = said.disks.iter().collect();
        disks.sort_by(|one, other| one.name.cmp(&other.name));
        let all: [(&str, Vec<String>); 6] = [
            (
                "Processor",
                processor(&said.cpuinfo, &said.architecture)
                    .into_iter()
                    .collect(),
            ),
            ("Memory", memory(&said.meminfo).into_iter().collect()),
            (
                "Graphics",
                cards
                    .into_iter()
                    .filter_map(|card| graphics(card, &said.pci_ids))
                    .collect(),
            ),
            ("Storage", disks.into_iter().filter_map(disk).collect()),
            (
                "System",
                system(&said.kernel, &said.os_release).into_iter().collect(),
            ),
            ("Screens", said.screens.iter().map(screen).collect()),
        ];
        let things = all.into_iter().filter(|(_, lines)| !lines.is_empty());
        Self {
            things: things.collect(),
        }
    }

    /// It as plain text, to be copied, after what omorfos is: a line for
    /// each thing, its kind before it.
    #[must_use]
    pub fn text(&self, omorfos: &str) -> String {
        let lines = self
            .things
            .iter()
            .flat_map(|(kind, lines)| lines.iter().map(move |line| format!("{kind}: {line}\n")));
        let head = format!("omorfos {omorfos}\n");
        std::iter::once(head).chain(lines).collect()
    }
}

/// What it is called and how many threads it runs at once: `cpuinfo`'s
/// own name for it, or its architecture's where it gives none, as on ARM.
fn processor(cpuinfo: &str, architecture: &str) -> Option<String> {
    let threads = cpuinfo
        .lines()
        .filter(|line| key(line) == Some("processor"))
        .count();
    let named = ["model name", "Model", "Hardware"]
        .into_iter()
        .find_map(|name| field(cpuinfo, name));
    let name = named.map_or_else(|| architecture.to_owned(), words);
    match threads {
        0 => None,
        1 => Some(format!("{name} \u{b7} 1 thread")),
        _ => Some(format!("{name} \u{b7} {threads} threads")),
    }
}

/// How much memory there is for the system to use, which is a little less
/// than is fitted, in powers of 1024 as memory is counted.
fn memory(meminfo: &str) -> Option<String> {
    let kilobytes = field(meminfo, "MemTotal")?.trim_end_matches("kB").trim();
    let bytes = kilobytes.parse::<u64>().ok()?.saturating_mul(1024);
    Some(size(bytes, BINARY))
}

/// What a card is, as `pci_ids` names it, then its driver: its maker's
/// short name and the name the card is sold under, where the list has
/// them, or the numbers where it does not.
fn graphics(card: &Card, pci_ids: &str) -> Option<String> {
    let number = |text: &str| u16::from_str_radix(text.trim().trim_start_matches("0x"), 16).ok();
    let name = number(&card.vendor)
        .zip(number(&card.device))
        .map(|numbers| named(pci_ids, numbers));
    let driver = Some(card.driver.trim().to_owned()).filter(|driver| !driver.is_empty());
    let both: Vec<String> = name.into_iter().chain(driver).collect();
    Some(both.join(" \u{b7} ")).filter(|both| !both.is_empty())
}

/// The name `pci_ids` gives the device `device` of the maker `vendor`.
fn named(pci_ids: &str, (vendor, device): (u16, u16)) -> String {
    let maker = format!("{vendor:04x}  ");
    let mut lines = pci_ids.lines().skip_while(|line| !line.starts_with(&maker));
    let made_by = lines.next().map(|line| line[maker.len()..].trim());
    let card = format!("\t{device:04x}  ");
    // Its devices follow it, a tab in, until the next maker.
    let mut devices = lines.take_while(|line| line.starts_with('\t') || line.starts_with('#'));
    let called = devices.find_map(|line| line.strip_prefix(&card));
    match (made_by, called) {
        (Some(maker), Some(card)) => format!("{} {}", short(maker), short(card.trim())),
        (Some(maker), None) => format!("{} {device:04x}", short(maker)),
        (None, _) => format!("{vendor:04x}:{device:04x}"),
    }
}

/// The name in square brackets at the end of a name in `pci.ids`, which is
/// the one it is known by, as `AMD/ATI` or `UHD Graphics 620`; or all of
/// it, where there is none.
fn short(name: &str) -> &str {
    let bracketed = name.strip_suffix(']').and_then(|name| {
        let start = name.rfind('[')?;
        Some(&name[start + 1..])
    });
    bracketed.unwrap_or(name)
}

/// What a disk is and how large, in powers of 1000 as disks are sold; none
/// for one that can be taken out, or that is not a disk at all but the
/// kernel's own: a loop device, memory, a CD, or one made of others.
fn disk(disk: &Disk) -> Option<String> {
    const NOT_DISKS: [&str; 9] = ["loop", "ram", "zram", "dm-", "md", "sr", "fd", "nbd", "zd"];
    let name = disk.name.trim();
    let kernels = NOT_DISKS.iter().any(|start| name.starts_with(start));
    let removable = disk.removable.trim() == "1";
    let sectors = disk
        .size
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|_| !kernels && !removable);
    let bytes = sectors.filter(|sectors| *sectors > 0)?.saturating_mul(512);
    let model = words(&disk.model);
    let what = if model.is_empty() { name } else { &model };
    Some(format!("{what} \u{b7} {}", size(bytes, DECIMAL)))
}

/// What the system is, as it calls itself, and the kernel's version.
fn system(kernel: &str, os_release: &str) -> Option<String> {
    let named = os_release.lines().find_map(|line| {
        let value = line.strip_prefix("PRETTY_NAME=")?;
        Some(value.trim().trim_matches('"').to_owned())
    });
    let kernel = kernel.trim();
    let kernel = (!kernel.is_empty()).then(|| format!("Linux {kernel}"));
    let both: Vec<String> = named.into_iter().chain(kernel).collect();
    Some(both.join(" \u{b7} ")).filter(|both| !both.is_empty())
}

/// A screen: its connector, how many pixels it is driven at and how often,
/// and its scale.
fn screen(screen: &Monitor) -> String {
    let (across, down) = screen.pixels;
    let hertz = (screen.refresh + 500) / 1000;
    let often = if screen.refresh > 0 {
        format!(" at {hertz} Hz")
    } else {
        String::new()
    };
    let (name, scale) = (&screen.name, screen.scale);
    format!("{name} \u{b7} {across}\u{d7}{down}{often} \u{b7} scale {scale}")
}

/// What comes before the colon of a line of `/proc`'s, without the room
/// around it.
fn key(line: &str) -> Option<&str> {
    line.split_once(':').map(|(key, _)| key.trim())
}

/// What comes after `name`'s colon in the first line with it, if there is
/// anything.
fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        let value = value.trim();
        (key.trim() == name && !value.is_empty()).then_some(value)
    })
}

/// `text` with every run of room in it made one space, as the names of
/// disks are padded.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Powers of 1024, as memory is counted.
const BINARY: (u64, [&str; 5]) = (1024, ["B", "KiB", "MiB", "GiB", "TiB"]);
/// Powers of 1000, as disks are sold.
const DECIMAL: (u64, [&str; 5]) = (1000, ["B", "kB", "MB", "GB", "TB"]);

/// `bytes` in the largest of `units` there is one of, to a tenth.
fn size(bytes: u64, (step, units): (u64, [&str; 5])) -> String {
    let mut unit = 1_u128;
    let mut name = units[0];
    for (power, next) in units.iter().enumerate().skip(1) {
        let larger = u128::from(step).pow(u32::try_from(power).unwrap_or(0));
        if u128::from(bytes) >= larger {
            (unit, name) = (larger, next);
        }
    }
    let tenths = (u128::from(bytes) * 10 + unit / 2) / unit;
    format!("{}.{} {name}", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTEL: &str = "\
processor\t: 0
vendor_id\t: GenuineIntel
model name\t: Intel(R) Core(TM) i7-8565U CPU @ 1.80GHz

processor\t: 1
model name\t: Intel(R) Core(TM) i7-8565U CPU @ 1.80GHz
";

    /// As an ARM processor says it: a part number, and no name.
    const ARM: &str = "processor\t: 0\nBogoMIPS\t: 48.00\nCPU part\t: 0xd08\n\n\
                       processor\t: 1\nprocessor\t: 2\nprocessor\t: 3\n";

    const PCI_IDS: &str = "\
# The list
1002  Advanced Micro Devices, Inc. [AMD/ATI]
\t15bf  Phoenix1
\t73bf  Navi 21 [Radeon RX 6800/6800 XT / 6900 XT]
\t\t1002 0e3a  Radeon RX 6900 XT
10de  NVIDIA Corporation
# A note among them
\t2684  AD102 [GeForce RTX 4090]
8086  Intel Corporation
\t3ea0  WhiskeyLake-U GT2 [UHD Graphics 620]
C 00  Unclassified device
";

    fn card(vendor: &str, device: &str, driver: &str) -> Card {
        Card {
            name: "card0".to_owned(),
            vendor: vendor.to_owned(),
            device: device.to_owned(),
            driver: driver.to_owned(),
        }
    }

    fn named_disk(name: &str, removable: &str, size: &str, model: &str) -> Disk {
        Disk {
            name: name.to_owned(),
            removable: removable.to_owned(),
            size: size.to_owned(),
            model: model.to_owned(),
        }
    }

    #[test]
    fn a_processor_is_its_name_and_how_many_threads_it_runs() {
        assert_eq!(
            processor(INTEL, "x86_64").as_deref(),
            Some("Intel(R) Core(TM) i7-8565U CPU @ 1.80GHz \u{b7} 2 threads")
        );
        assert_eq!(
            processor(ARM, "aarch64").as_deref(),
            Some("aarch64 \u{b7} 4 threads"),
            "named by its architecture where it gives no name"
        );
        let pi = "processor\t: 0\nModel\t\t: Raspberry Pi 4 Model B Rev 1.4\n";
        assert_eq!(
            processor(pi, "aarch64").as_deref(),
            Some("Raspberry Pi 4 Model B Rev 1.4 \u{b7} 1 thread")
        );
        assert_eq!(processor("", "x86_64"), None, "nothing read");
    }

    #[test]
    fn memory_is_what_the_system_has_in_powers_of_1024() {
        let meminfo = "MemTotal:       32617844 kB\nMemFree:  1000 kB\n";
        assert_eq!(memory(meminfo).as_deref(), Some("31.1 GiB"));
        assert_eq!(memory("MemTotal: 8388608 kB\n").as_deref(), Some("8.0 GiB"));
        assert_eq!(memory("MemFree: 1 kB\n"), None);
        assert_eq!(memory("MemTotal: lots\n"), None);
        assert_eq!(memory(""), None);
    }

    #[test]
    fn a_card_is_named_as_pci_ids_names_it_then_its_driver() {
        let name = |vendor, device, driver| graphics(&card(vendor, device, driver), PCI_IDS);
        assert_eq!(
            name("0x8086\n", "0x3ea0\n", "i915").as_deref(),
            Some("Intel Corporation UHD Graphics 620 \u{b7} i915")
        );
        assert_eq!(
            name("0x1002", "0x73bf", "amdgpu").as_deref(),
            Some("AMD/ATI Radeon RX 6800/6800 XT / 6900 XT \u{b7} amdgpu")
        );
        assert_eq!(
            name("0x1002", "0x15bf", "").as_deref(),
            Some("AMD/ATI Phoenix1"),
            "only a chip's name"
        );
        assert_eq!(
            name("0x10de", "0x2684", "nvidia").as_deref(),
            Some("NVIDIA Corporation GeForce RTX 4090 \u{b7} nvidia"),
            "past a note"
        );
        assert_eq!(
            name("0x10de", "0x0e3a", "").as_deref(),
            Some("NVIDIA Corporation 0e3a"),
            "not among its maker's"
        );
        assert_eq!(
            name("0x1234", "0x5678", "").as_deref(),
            Some("1234:5678"),
            "nor its maker"
        );
        assert_eq!(
            graphics(&card("0x8086", "0x3ea0", ""), "").as_deref(),
            Some("8086:3ea0"),
            "without the list"
        );
        assert_eq!(
            name("", "", "panfrost").as_deref(),
            Some("panfrost"),
            "not on a PCI bus"
        );
        assert_eq!(name("", "", ""), None);
        assert_eq!(short("Odd name]"), "Odd name]", "a bracket never opened");
    }

    #[test]
    fn a_disk_is_what_it_is_and_how_large_and_what_is_not_a_disk_is_left_out() {
        let shown = |name, removable, size, model| disk(&named_disk(name, removable, size, model));
        assert_eq!(
            shown(
                "nvme0n1",
                "0\n",
                "1953525168\n",
                "Samsung SSD 990 PRO 1TB          \n"
            )
            .as_deref(),
            Some("Samsung SSD 990 PRO 1TB \u{b7} 1.0 TB")
        );
        assert_eq!(
            shown("sda", "0", "1000215216", "").as_deref(),
            Some("sda \u{b7} 512.1 GB"),
            "named by the kernel where it has no model"
        );
        assert_eq!(shown("sdb", "1", "1000215216", "Stick"), None, "taken out");
        for name in ["loop0", "ram0", "zram0", "dm-0", "md127", "sr0", "nbd0"] {
            assert_eq!(shown(name, "0", "1000", ""), None, "{name}");
        }
        assert_eq!(shown("sdc", "0", "0", ""), None, "empty");
        assert_eq!(shown("sdc", "0", "", ""), None, "unread");
    }

    #[test]
    fn the_system_is_its_name_and_the_kernels_version() {
        let os = "NAME=\"Debian GNU/Linux\"\nPRETTY_NAME=\"Debian GNU/Linux 13 (trixie)\"\n";
        assert_eq!(
            system("6.12.115+deb13.1-amd64\n", os).as_deref(),
            Some("Debian GNU/Linux 13 (trixie) \u{b7} Linux 6.12.115+deb13.1-amd64")
        );
        assert_eq!(system("6.12.0", "").as_deref(), Some("Linux 6.12.0"));
        assert_eq!(
            system("", "PRETTY_NAME=Arch Linux\n").as_deref(),
            Some("Arch Linux"),
            "unquoted"
        );
        assert_eq!(system("", ""), None);
    }

    #[test]
    fn a_screen_is_its_connector_its_pixels_how_often_it_shows_them_and_its_scale() {
        let laptop = Monitor {
            name: "eDP-1".to_owned(),
            pixels: (2880, 1800),
            refresh: 119_998,
            scale: 2.0,
        };
        assert_eq!(
            screen(&laptop),
            "eDP-1 \u{b7} 2880\u{d7}1800 at 120 Hz \u{b7} scale 2"
        );
        let unsaid = Monitor {
            refresh: 0,
            scale: 1.25,
            ..laptop
        };
        assert_eq!(
            screen(&unsaid),
            "eDP-1 \u{b7} 2880\u{d7}1800 \u{b7} scale 1.25"
        );
    }

    #[test]
    fn sizes_are_given_to_a_tenth_in_the_largest_unit_there_is_one_of() {
        assert_eq!(size(0, DECIMAL), "0.0 B");
        assert_eq!(size(999, DECIMAL), "999.0 B");
        assert_eq!(size(1_000, DECIMAL), "1.0 kB");
        assert_eq!(size(1_000_204_886_016, DECIMAL), "1.0 TB");
        assert_eq!(size(4_000_000_000_000_000, DECIMAL), "4000.0 TB");
        assert_eq!(size(1_536, BINARY), "1.5 KiB");
        assert_eq!(size(u64::MAX, BINARY), "16777216.0 TiB");
    }

    #[test]
    fn the_system_is_shown_and_copied_with_what_nothing_is_known_of_left_out() {
        let said = Said {
            cpuinfo: INTEL.to_owned(),
            architecture: "x86_64".to_owned(),
            meminfo: "MemTotal: 8388608 kB\n".to_owned(),
            cards: vec![
                Card {
                    name: "card1".to_owned(),
                    ..card("", "", "amdgpu")
                },
                card("", "", "i915"),
                Card {
                    name: "card0-eDP-1".to_owned(),
                    ..card("", "", "i915")
                },
                Card {
                    name: "renderD128".to_owned(),
                    ..card("", "", "i915")
                },
                Card {
                    name: "card".to_owned(),
                    ..card("", "", "i915")
                },
            ],
            pci_ids: String::new(),
            disks: vec![
                named_disk("sdb", "0", "2000", "Second"),
                named_disk("loop0", "0", "2000", ""),
                named_disk("nvme0n1", "0", "2000", "First"),
            ],
            kernel: String::new(),
            os_release: String::new(),
            screens: Vec::new(),
        };
        let system = System::of(&said);
        let kinds: Vec<&str> = system.things.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(
            kinds,
            ["Processor", "Memory", "Graphics", "Storage"],
            "nothing of the system or the screens"
        );
        assert_eq!(
            system.things[2].1,
            ["i915", "amdgpu"],
            "in their order, and only cards"
        );
        assert_eq!(
            system.things[3].1,
            ["First \u{b7} 1.0 MB", "Second \u{b7} 1.0 MB"]
        );
        let text = system.text("0.1.0");
        assert!(
            text.starts_with("omorfos 0.1.0\nProcessor: Intel"),
            "{text}"
        );
        assert!(
            text.contains("\nGraphics: i915\nGraphics: amdgpu\n"),
            "{text}"
        );
        assert!(text.ends_with("Storage: Second \u{b7} 1.0 MB\n"), "{text}");
        assert_eq!(System::of(&Said::default()).things, []);
    }
}
