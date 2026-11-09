//! What the computer is, read from what the kernel and the system say of
//! it as "About" opens. Only read here: what to make of it, and what to
//! leave out, is `omorfos_core::System`'s.

use std::fs;
use std::path::{Path, PathBuf};

use omorfos_core::{Card, Disk, Monitor, Said};

/// Where `pci.ids` is kept: by `hwdata` on Arch and Fedora, by `pci.ids`
/// on Debian.
const PCI_IDS: [&str; 2] = ["usr/share/hwdata/pci.ids", "usr/share/misc/pci.ids"];

/// What the system under `root` says of the computer, which has `screens`.
/// What cannot be read is left empty.
pub(crate) fn said(root: &Path, screens: Vec<Monitor>) -> Said {
    let read = |path: &Path| fs::read_to_string(root.join(path)).unwrap_or_default();
    let named = |path: &Path| {
        let name = path.file_name().unwrap_or_default();
        name.to_string_lossy().into_owned()
    };
    let cards = listed(root.join("sys/class/drm")).map(|card| Card {
        name: named(&card),
        vendor: read(&card.join("device/vendor")),
        device: read(&card.join("device/device")),
        driver: fs::read_link(card.join("device/driver"))
            .map(|driver| named(&driver))
            .unwrap_or_default(),
    });
    let disks = listed(root.join("sys/block")).map(|disk| Disk {
        name: named(&disk),
        removable: read(&disk.join("removable")),
        size: read(&disk.join("size")),
        model: read(&disk.join("device/model")),
    });
    let mut pci_ids = PCI_IDS.into_iter().map(|path| read(Path::new(path)));
    Said {
        cpuinfo: read(Path::new("proc/cpuinfo")),
        architecture: std::env::consts::ARCH.to_owned(),
        meminfo: read(Path::new("proc/meminfo")),
        cards: cards.collect(),
        pci_ids: pci_ids.find(|ids| !ids.is_empty()).unwrap_or_default(),
        disks: disks.collect(),
        kernel: read(Path::new("proc/sys/kernel/osrelease")),
        os_release: read(Path::new("etc/os-release")),
        screens,
    }
}

/// What is in the directory `path`, or nothing if it cannot be read.
fn listed(path: PathBuf) -> impl Iterator<Item = PathBuf> {
    let entries = fs::read_dir(path).into_iter().flatten().flatten();
    entries.map(|entry| entry.path())
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    use super::*;

    /// Writes `text` at `path` under `root`, making the directories it is in.
    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn what_the_kernel_and_the_system_say_is_read_from_where_they_say_it() {
        let root = TempDir::new().unwrap();
        let root = root.path();
        write(root, "proc/cpuinfo", "processor : 0\n");
        write(root, "proc/meminfo", "MemTotal: 1024 kB\n");
        write(root, "proc/sys/kernel/osrelease", "6.12.0\n");
        write(root, "etc/os-release", "PRETTY_NAME=Test\n");
        write(root, "usr/share/misc/pci.ids", "8086  Intel Corporation\n");
        write(root, "sys/class/drm/card0/device/vendor", "0x8086\n");
        write(root, "sys/class/drm/card0/device/device", "0x3ea0\n");
        fs::create_dir_all(root.join("sys/bus/pci/drivers/i915")).unwrap();
        symlink(
            root.join("sys/bus/pci/drivers/i915"),
            root.join("sys/class/drm/card0/device/driver"),
        )
        .unwrap();
        write(root, "sys/block/sda/removable", "0\n");
        write(root, "sys/block/sda/size", "2000\n");
        write(root, "sys/block/sda/device/model", "Disk\n");
        let screen = Monitor {
            name: "test-0".to_owned(),
            pixels: (640, 480),
            refresh: 60_000,
            scale: 1.0,
        };

        let said = said(root, vec![screen.clone()]);
        assert_eq!(said.cpuinfo, "processor : 0\n");
        assert_eq!(said.architecture, std::env::consts::ARCH);
        assert_eq!(said.meminfo, "MemTotal: 1024 kB\n");
        assert_eq!(said.kernel, "6.12.0\n");
        assert_eq!(said.os_release, "PRETTY_NAME=Test\n");
        assert_eq!(
            said.pci_ids, "8086  Intel Corporation\n",
            "the second place"
        );
        assert_eq!(
            said.cards,
            [Card {
                name: "card0".to_owned(),
                vendor: "0x8086\n".to_owned(),
                device: "0x3ea0\n".to_owned(),
                driver: "i915".to_owned(),
            }]
        );
        assert_eq!(
            said.disks,
            [Disk {
                name: "sda".to_owned(),
                removable: "0\n".to_owned(),
                size: "2000\n".to_owned(),
                model: "Disk\n".to_owned(),
            }]
        );
        assert_eq!(said.screens, [screen]);
    }

    #[test]
    fn what_is_not_there_is_left_empty() {
        let root = TempDir::new().unwrap();
        let said = said(root.path(), Vec::new());
        assert_eq!(
            said,
            Said {
                architecture: std::env::consts::ARCH.to_owned(),
                ..Said::default()
            }
        );
    }
}
