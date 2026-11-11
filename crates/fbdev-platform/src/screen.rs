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
            } else {
                Self::None
            };
            this.set_on(true);
            this
        }

        pub(crate) fn available(&self) -> bool {
            !matches!(self, Self::None)
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
                Self::None => {}
            }
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
