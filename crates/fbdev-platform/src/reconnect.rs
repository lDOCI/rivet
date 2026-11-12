//! Retry schedule for a device that may be missing or vanish (touch panel).
//!
//! Platform-independent so it can be unit-tested anywhere: the caller passes
//! the current time instead of this module reading a clock.

use std::time::{Duration, Instant};

/// Wait between attempts to reopen the device.
pub const RETRY_INTERVAL: Duration = Duration::from_secs(2);

/// Decides when the next reopen attempt is due and whether a failure is worth
/// logging (only the first one in a row, so a missing panel doesn't flood the log).
#[derive(Debug, Default)]
pub struct Retry {
    next: Option<Instant>,
    failures: u32,
}

impl Retry {
    /// A reopen attempt may run now.
    pub fn due(&self, now: Instant) -> bool {
        self.next.is_none_or(|at| now >= at)
    }

    /// The attempt failed. Returns `true` for the first failure since the last
    /// success, which is the only one the caller should log loudly.
    pub fn failed(&mut self, now: Instant) -> bool {
        self.next = Some(now + RETRY_INTERVAL);
        self.failures = self.failures.saturating_add(1);
        self.failures == 1
    }

    /// The device is open again, or was just lost: restart the schedule so the
    /// first attempt happens after one interval, not in a tight loop.
    pub fn lost(&mut self, now: Instant) {
        self.next = Some(now + RETRY_INTERVAL);
        self.failures = 0;
    }

    /// The device opened successfully.
    pub fn recovered(&mut self) {
        self.next = None;
        self.failures = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_attempt_is_immediate() {
        assert!(Retry::default().due(Instant::now()));
    }

    #[test]
    fn waits_one_interval_after_failure() {
        let t0 = Instant::now();
        let mut r = Retry::default();
        r.failed(t0);
        assert!(!r.due(t0));
        assert!(!r.due(t0 + RETRY_INTERVAL - Duration::from_millis(1)));
        assert!(r.due(t0 + RETRY_INTERVAL));
    }

    #[test]
    fn only_first_failure_is_loud() {
        let t0 = Instant::now();
        let mut r = Retry::default();
        assert!(r.failed(t0));
        assert!(!r.failed(t0 + RETRY_INTERVAL));
        assert!(!r.failed(t0 + RETRY_INTERVAL * 2));
    }

    #[test]
    fn loss_after_recovery_is_loud_again() {
        let t0 = Instant::now();
        let mut r = Retry::default();
        r.failed(t0);
        r.recovered();
        assert!(r.due(t0));
        r.lost(t0);
        assert!(!r.due(t0));
        assert!(r.failed(t0 + RETRY_INTERVAL));
    }
}
