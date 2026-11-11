//! Slint platform that draws into a Linux framebuffer device and reads touch
//! input from evdev. No libinput, udev, libseat or DRM required, so it runs on
//! minimal Buildroot systems such as Klipper Mod for the Flashforge AD5M.
//!
//! Configuration comes from environment variables, see [`Config::from_env`].

pub mod pixel;
pub mod reconnect;
pub mod screen;
pub mod touch_state;

#[cfg(target_os = "linux")]
mod framebuffer;
#[cfg(target_os = "linux")]
mod platform;
#[cfg(target_os = "linux")]
mod touch;

#[cfg(target_os = "linux")]
pub use platform::init;

use std::path::PathBuf;

pub use touch_state::TouchTransform;

/// Display rotation applied by the renderer, clockwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl Rotation {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "0" => Some(Self::Deg0),
            "90" => Some(Self::Deg90),
            "180" => Some(Self::Deg180),
            "270" => Some(Self::Deg270),
            _ => None,
        }
    }

    /// Whether width and height swap between the panel and the UI.
    pub fn is_transposed(self) -> bool {
        matches!(self, Self::Deg90 | Self::Deg270)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Framebuffer device, default `/dev/fb0`.
    pub framebuffer: PathBuf,
    /// Touch device. `None` = pick the first touch-capable `/dev/input/event*`,
    /// preferring tslib's calibrated `ts_uinput` device.
    pub touch: Option<PathBuf>,
    pub rotation: Rotation,
    pub touch_transform: TouchTransform,
}

impl Config {
    /// Reads:
    /// - `RIVET_FB` — framebuffer path (default `/dev/fb0`)
    /// - `RIVET_TOUCH` — evdev path (default: autodetect)
    /// - `RIVET_ROTATION` — `0` | `90` | `180` | `270`
    /// - `RIVET_TOUCH_SWAP_XY`, `RIVET_TOUCH_INVERT_X`, `RIVET_TOUCH_INVERT_Y` — `1` to enable
    pub fn from_env() -> Self {
        let flag = |name: &str| std::env::var(name).is_ok_and(|v| v == "1" || v == "true");
        Self {
            framebuffer: std::env::var_os("RIVET_FB")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/dev/fb0")),
            touch: std::env::var_os("RIVET_TOUCH").map(PathBuf::from),
            rotation: std::env::var("RIVET_ROTATION")
                .ok()
                .and_then(|v| Rotation::parse(&v))
                .unwrap_or_default(),
            touch_transform: TouchTransform {
                swap_xy: flag("RIVET_TOUCH_SWAP_XY"),
                invert_x: flag("RIVET_TOUCH_INVERT_X"),
                invert_y: flag("RIVET_TOUCH_INVERT_Y"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rotation() {
        assert_eq!(Rotation::parse("90"), Some(Rotation::Deg90));
        assert_eq!(Rotation::parse(" 270 "), Some(Rotation::Deg270));
        assert_eq!(Rotation::parse("45"), None);
        assert!(Rotation::Deg90.is_transposed());
        assert!(!Rotation::Deg180.is_transposed());
    }
}
