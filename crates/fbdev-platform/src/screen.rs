//! Screen sleep: the backlight goes off after a while without touches and the
//! next touch wakes it. That touch only wakes: it does not reach the UI, so a
//! button under the finger is not pressed blind.
//!
//! The app sets the policy ([`set_sleep_after`], [`set_keep_awake`]); the event
//! loop does the rest. On a printer without a known backlight control the
//! screen simply never sleeps.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

/// Seconds without a touch before the backlight goes off; 0 = never.
static SLEEP_AFTER: AtomicU64 = AtomicU64::new(600);
/// Set while something must stay visible (a print running).
static KEEP_AWAKE: AtomicBool = AtomicBool::new(false);
/// Bumped to ask the event loop to wake the screen (e.g. an error came in).
static WAKE_REQUESTS: AtomicU64 = AtomicU64::new(0);
/// Backlight level asked for, percent (0: none asked yet), and how many asks.
static BRIGHTNESS: AtomicU64 = AtomicU64::new(0);
static BRIGHTNESS_ASKS: AtomicU64 = AtomicU64::new(0);
/// Whether this system has a backlight Rivet can set.
static BACKLIGHT_OK: AtomicBool = AtomicBool::new(false);
/// Milliseconds since `START` at the last touch.
static LAST_TOUCH_MS: AtomicU64 = AtomicU64::new(0);
static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

fn now_ms() -> u64 {
    START
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_millis() as u64
}

/// The event loop calls this on every touch.
// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn touched() {
    LAST_TOUCH_MS.store(now_ms(), Ordering::Relaxed);
}

/// How long since the last touch (or since start).
pub fn idle() -> Duration {
    Duration::from_millis(now_ms().saturating_sub(LAST_TOUCH_MS.load(Ordering::Relaxed)))
}

/// How long without a touch before the screen sleeps; `None` never.
pub fn set_sleep_after(after: Option<Duration>) {
    SLEEP_AFTER.store(after.map_or(0, |d| d.as_secs().max(1)), Ordering::Relaxed);
}

/// Keep the screen on, e.g. while printing; waking it if it slept.
pub fn set_keep_awake(on: bool) {
    if on && !KEEP_AWAKE.swap(true, Ordering::Relaxed) {
        wake();
    } else if !on {
        KEEP_AWAKE.store(false, Ordering::Relaxed);
    }
}

/// Turn the screen on now (the printer stopped, a dialog wants attention, ...).
pub fn wake() {
    WAKE_REQUESTS.fetch_add(1, Ordering::Relaxed);
}

/// Set the backlight, 5–100 %; it applies while the screen is on and again
/// after it wakes.
pub fn set_brightness(percent: u32) {
    BRIGHTNESS.store(u64::from(percent.clamp(5, 100)), Ordering::Relaxed);
    BRIGHTNESS_ASKS.fetch_add(1, Ordering::Relaxed);
}

/// Whether the backlight can be set here (known once the platform started).
pub fn backlight_available() -> bool {
    BACKLIGHT_OK.load(Ordering::Relaxed)
}

// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn set_backlight_available(ok: bool) {
    BACKLIGHT_OK.store(ok, Ordering::Relaxed);
}

/// The brightness asked for (percent) and the count of asks so far.
// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn brightness_asks() -> (u32, u64) {
    (
        BRIGHTNESS.load(Ordering::Relaxed) as u32,
        BRIGHTNESS_ASKS.load(Ordering::Relaxed),
    )
}

/// Idle time before the screen sleeps (or, while printing, shows the screensaver).
pub fn sleep_after() -> Option<Duration> {
    match SLEEP_AFTER.load(Ordering::Relaxed) {
        0 => None,
        s => Some(Duration::from_secs(s)),
    }
}

// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn keep_awake() -> bool {
    KEEP_AWAKE.load(Ordering::Relaxed)
}

// Used by the Linux event loop only.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn wake_requests() -> u64 {
    WAKE_REQUESTS.load(Ordering::Relaxed)
}

/// How the backlight is switched on this system.
#[cfg(target_os = "linux")]
pub(crate) mod backlight {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// The AD5M's tool (Klipper Mod): `backlight [0-255]`, 0 switches it off.
    const AD5M_TOOL: &str = "/usr/bin/backlight";

    pub(crate) enum Backlight {
        /// /sys/class/backlight/<dev>: brightness and bl_power.
        Sysfs(PathBuf),
        /// The AD5M tool, with the brightness to come back to.
        Tool {
            level: u32,
        },
        /// The AD5M's display driver straight (Z-Mod has no such tool), with the
        /// brightness to come back to, 1–255.
        Disp {
            level: u32,
        },
        None,
    }

    impl Backlight {
        /// Find the backlight and make sure it is on: something (the stock screen
        /// going to sleep before it was stopped) may have left it off.
        pub(crate) fn open() -> Self {
            let sysfs = std::fs::read_dir("/sys/class/backlight")
                .ok()
                .and_then(|mut d| d.next())
                .and_then(Result::ok)
                .map(|e| e.path());
            let mut this = if let Some(dir) = sysfs {
                Self::Sysfs(dir)
            } else if Path::new(AD5M_TOOL).exists() {
                let level = tool_level().filter(|l| *l > 0).unwrap_or(128);
                Self::Tool { level }
            } else if Path::new(disp::DEVICE).exists() {
                let level = disp::level().filter(|l| *l > 0).unwrap_or(153);
                Self::Disp { level }
            } else {
                Self::None
            };
            this.set_on(true);
            this
        }

        pub(crate) fn available(&self) -> bool {
            !matches!(self, Self::None)
        }

        /// A new brightness, percent; switched on now when `on`.
        pub(crate) fn set_percent(&mut self, percent: u32, on: bool) {
            match self {
                Self::Sysfs(dir) => {
                    let max: u32 = std::fs::read_to_string(dir.join("max_brightness"))
                        .ok()
                        .and_then(|m| m.trim().parse().ok())
                        .unwrap_or(255);
                    let value = (max * percent / 100).max(1);
                    let _ = std::fs::write(dir.join("brightness"), value.to_string());
                }
                Self::Tool { level } | Self::Disp { level } => {
                    *level = (255 * percent / 100).clamp(1, 255);
                    if on {
                        self.set_on(true);
                    }
                }
                Self::None => {}
            }
        }

        pub(crate) fn set_on(&mut self, on: bool) {
            match self {
                Self::Sysfs(dir) => {
                    let _ = std::fs::write(dir.join("bl_power"), if on { "0" } else { "4" });
                }
                Self::Tool { level } => {
                    // Keep what the user set (another tool may have changed it).
                    if !on {
                        if let Some(l) = tool_level().filter(|l| *l > 0) {
                            *level = l;
                        }
                    }
                    let value = if on { *level } else { 0 };
                    match Command::new(AD5M_TOOL).arg(value.to_string()).output() {
                        Ok(out) if out.status.success() => {}
                        Ok(out) => tracing::warn!(status = %out.status, "backlight tool failed"),
                        Err(err) => tracing::warn!(%err, "backlight tool failed"),
                    }
                }
                Self::Disp { level } => {
                    if !on {
                        if let Some(l) = disp::level().filter(|l| *l > 0) {
                            *level = l;
                        }
                    }
                    if let Err(err) = disp::set(on, *level) {
                        tracing::warn!(%err, "backlight (/dev/disp) failed");
                    }
                }
                Self::None => {}
            }
        }
    }

    /// The Allwinner display driver's backlight calls, as Z-Mod's `backlight`
    /// script makes them: each takes [screen, value, …] as C longs.
    mod disp {
        use std::os::fd::AsRawFd;

        pub(super) const DEVICE: &str = "/dev/disp";
        const SET_BRIGHTNESS: libc::c_ulong = 0x102;
        const GET_BRIGHTNESS: libc::c_ulong = 0x103;
        const BACKLIGHT_ENABLE: libc::c_ulong = 0x104;
        const BACKLIGHT_DISABLE: libc::c_ulong = 0x105;

        fn call(request: libc::c_ulong, value: libc::c_ulong) -> std::io::Result<i32> {
            let f = std::fs::OpenOptions::new().write(true).open(DEVICE)?;
            let mut args: [libc::c_ulong; 4] = [0, value, 0, 0];
            // SAFETY: the driver reads up to four longs from `args`, which
            // outlives the call.
            let r = unsafe { libc::ioctl(f.as_raw_fd(), request as _, args.as_mut_ptr()) };
            if r < 0 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(r)
            }
        }

        /// Brightness 1–255, or off.
        pub(super) fn set(on: bool, level: u32) -> std::io::Result<()> {
            if on {
                call(BACKLIGHT_ENABLE, 0)?;
                call(SET_BRIGHTNESS, libc::c_ulong::from(level.clamp(1, 255)))?;
            } else {
                call(BACKLIGHT_DISABLE, 0)?;
            }
            Ok(())
        }

        /// The brightness now, 0–255.
        pub(super) fn level() -> Option<u32> {
            call(GET_BRIGHTNESS, 0)
                .ok()
                .and_then(|v| u32::try_from(v).ok())
        }
    }

    /// "Current brightness: 191/255 (74%)" → 191.
    fn tool_level() -> Option<u32> {
        let out = Command::new(AD5M_TOOL).output().ok()?;
        parse_level(&String::from_utf8_lossy(&out.stdout))
    }

    pub(crate) fn parse_level(text: &str) -> Option<u32> {
        let after = text.split(':').nth(1)?;
        after.trim().split('/').next()?.trim().parse().ok()
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn reads_the_tools_answer() {
            assert_eq!(
                super::parse_level("Current brightness: 191/255 (74%)\n"),
                Some(191)
            );
            assert_eq!(super::parse_level("nonsense"), None);
        }
    }
}
