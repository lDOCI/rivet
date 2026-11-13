//! The printer's time zone: which one is set, and setting it.
//!
//! Klipper Mod points `/etc/localtime` at `printer_data/config/timezone`, a copy
//! of a zoneinfo file put there to choose a zone; the link names no zone. So
//! the zone is found by comparing that file with the zoneinfo database, and set
//! by copying a zoneinfo file over it. Systems whose `/etc/localtime` points into
//! the zoneinfo database name the zone in the link. Z-Mod sets the zone with its
//! SET_TIMEZONE macro (a link into the database, copied into its chroot as
//! /tmp/localtime): there the macro is used, never a file in /tmp.

use std::path::{Path, PathBuf};

const LOCALTIME: &str = "/etc/localtime";
const ZONEINFO: &str = "/usr/share/zoneinfo";

/// The zones offered on the screen: Russia's, its neighbours', Europe's and
/// America's main ones, and UTC.
pub const ZONES: &[&str] = &[
    "Europe/Kaliningrad",
    "Europe/Moscow",
    "Europe/Samara",
    "Asia/Yekaterinburg",
    "Asia/Omsk",
    "Asia/Novosibirsk",
    "Asia/Krasnoyarsk",
    "Asia/Irkutsk",
    "Asia/Yakutsk",
    "Asia/Vladivostok",
    "Asia/Magadan",
    "Asia/Kamchatka",
    "Europe/Minsk",
    "Europe/Kyiv",
    "Asia/Almaty",
    "Asia/Tashkent",
    "Asia/Tbilisi",
    "Asia/Yerevan",
    "Asia/Baku",
    "Europe/London",
    "Europe/Berlin",
    "Europe/Paris",
    "Europe/Warsaw",
    "Europe/Prague",
    "Europe/Istanbul",
    "America/New_York",
    "America/Chicago",
    "America/Denver",
    "America/Los_Angeles",
    "UTC",
];

/// The zone set on the printer: `Some(name)`, `Some("")` when one is set but
/// is none of [`ZONES`], `None` when none is set (the clock is then hidden).
pub fn current() -> Option<String> {
    let link = std::fs::read_link(LOCALTIME).ok();
    if let Some(name) = link.as_ref().and_then(|l| {
        let l = l.to_string_lossy().into_owned();
        l.split_once("zoneinfo/").map(|(_, z)| z.to_owned())
    }) {
        return Some(name);
    }
    let set = std::fs::read(LOCALTIME).ok()?;
    Some(
        ZONES
            .iter()
            .find(|z| std::fs::read(Path::new(ZONEINFO).join(z)).is_ok_and(|b| b == set))
            .map_or_else(String::new, |z| (*z).to_owned()),
    )
}

/// Where a chosen zone is written, when this system takes one from Rivet:
/// `/etc/localtime` is a link to a file outside the zoneinfo database
/// (Klipper Mod's `printer_data/config/timezone`), and the database is there.
pub fn target() -> Option<PathBuf> {
    let link = std::fs::read_link(LOCALTIME).ok()?;
    let link = if link.is_absolute() {
        link
    } else {
        Path::new("/etc").join(link)
    };
    let path = link.to_string_lossy().into_owned();
    let inside_db = path.contains("zoneinfo/");
    // A copy in RAM (Z-Mod's /tmp/localtime) would be gone at the next boot.
    let volatile = path.starts_with("/tmp/") || path.starts_with("/run/");
    (!inside_db && !volatile && Path::new(ZONEINFO).is_dir()).then_some(link)
}

/// Set the zone: copy its zoneinfo file over the target, atomically.
pub fn set(zone: &str) -> std::io::Result<()> {
    if !ZONES.contains(&zone) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unknown zone",
        ));
    }
    let target = target().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "the time zone is not set here",
        )
    })?;
    let tmp = target.with_extension("rivet-tmp");
    std::fs::copy(Path::new(ZONEINFO).join(zone), &tmp)?;
    std::fs::rename(&tmp, &target)
}
