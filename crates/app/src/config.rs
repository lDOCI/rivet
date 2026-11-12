//! `rivet.toml`: the few settings a person may want to change by hand.
//! Command line and environment still win over the file.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Where printer configs live: KlipperMod's (seen on an AD5M Pro), then the
/// usual Klipper install's. A copy in the working directory is read when none
/// exists (development on a computer).
const PRINTER_CONFIG_DIRS: [&str; 3] = [
    "/root/printer_data/config",
    "/home/pi/printer_data/config",
    "/data/printer_data/config",
];

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Interface language: `en`, `ru`.
    pub language: Option<String>,
    /// Moonraker websocket, e.g. `ws://127.0.0.1:7125/websocket`.
    pub moonraker: Option<String>,
    /// Write the log to this file (rotated) instead of the terminal.
    pub log_file: Option<String>,
    /// `error`, `warn`, `info`, `debug`; default `info`.
    pub log_level: Option<String>,
}

impl Config {
    pub fn parse(text: &str) -> Result<Config, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Read the file at `path`; a missing file is the default config, a broken
    /// one is reported and ignored: the screen must still come up.
    pub fn load(path: &Path) -> Config {
        match std::fs::read_to_string(path) {
            Ok(text) => Config::parse(&text).unwrap_or_else(|err| {
                eprintln!("{}: {err}", path.display());
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }
}

/// `--config <path>`, else `rivet.toml` in the printer config folder, else in
/// the working directory.
pub fn config_path(arg: Option<&str>) -> PathBuf {
    if let Some(p) = arg {
        return PathBuf::from(p);
    }
    config_dir().join("rivet.toml")
}

/// The printer config folder Rivet keeps its files in (rivet.toml, the touch
/// calibration), else the working directory.
pub fn config_dir() -> PathBuf {
    PRINTER_CONFIG_DIRS
        .iter()
        .map(Path::new)
        .find(|p| p.is_dir())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_keys() {
        let c = Config::parse("language = \"ru\"\nmoonraker = \"ws://10.0.0.5:7125/websocket\"\n")
            .unwrap();
        assert_eq!(c.language.as_deref(), Some("ru"));
        assert_eq!(c.moonraker.as_deref(), Some("ws://10.0.0.5:7125/websocket"));
        assert_eq!(Config::parse("").unwrap(), Config::default());
    }

    #[test]
    fn typos_are_errors_not_silence() {
        let err = Config::parse("langauge = \"ru\"").unwrap_err();
        assert!(err.contains("langauge"), "{err}");
    }

    #[test]
    fn missing_or_broken_file_is_the_default() {
        assert_eq!(
            Config::load(Path::new("/nonexistent/rivet.toml")),
            Config::default()
        );
        let dir = std::env::temp_dir().join(format!("rivet-cfg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("rivet.toml");
        std::fs::write(&file, "language = ").unwrap();
        assert_eq!(Config::load(&file), Config::default());
        std::fs::write(&file, "language = \"ru\"").unwrap();
        assert_eq!(Config::load(&file).language.as_deref(), Some("ru"));
        let _ = std::fs::remove_dir_all(dir);
    }
}
