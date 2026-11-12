//! "Now", injected: the one clock the server and its store read, so a test that sets it controls
//! every instant the API derives from (arrival stamps, the durations between them, cutoffs).

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use jiff::{SignedDuration, Timestamp};

/// "Now", as the server sees it: each row's `received_at` (and so every duration measured between
/// arrivals), the `running` window, the heartbeat window of `/api/sources`, the `before` a
/// delete-events dry run fixes, compact and retention cutoffs, export `generated_at` and file names.
pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}

/// The system clock.
#[derive(Clone, Copy, Debug, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        Timestamp::now()
    }
}

/// Always the same instant (tests).
#[derive(Clone, Copy, Debug)]
pub struct FixedClock(pub Timestamp);

impl Clock for FixedClock {
    fn now(&self) -> Timestamp {
        self.0
    }
}

/// An instant the test moves (tests): starts where it is told and only changes on [`Self::set`] or
/// [`Self::advance`]. Clones share the instant, so the test keeps one and gives the app another.
/// Microsecond precision, the precision `received_at` is stored with.
#[derive(Clone, Debug)]
pub struct ManualClock(Arc<AtomicI64>);

impl ManualClock {
    pub fn new(start: Timestamp) -> Self {
        ManualClock(Arc::new(AtomicI64::new(start.as_microsecond())))
    }

    pub fn set(&self, now: Timestamp) {
        self.0.store(now.as_microsecond(), Ordering::SeqCst);
    }

    /// Move the clock by `by` (negative moves it back).
    pub fn advance(&self, by: SignedDuration) {
        let micros = i64::try_from(by.as_micros()).unwrap_or(i64::MAX);
        self.0.fetch_add(micros, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Timestamp {
        Timestamp::from_microsecond(self.0.load(Ordering::SeqCst)).unwrap_or(Timestamp::MAX)
    }
}

/// `clock` as the store's clock, so rows are stamped with the same "now" the server reads.
pub fn for_store(clock: &Arc<dyn Clock>) -> gitprompt_store::store::Clock {
    let clock = Arc::clone(clock);
    Arc::new(move || clock.now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_manual_clock_moves_only_when_told_and_its_clones_follow() {
        let start: Timestamp = "2026-01-01T00:00:00Z".parse().expect("instant");
        let clock = ManualClock::new(start);
        let app_side = clock.clone();
        assert_eq!(app_side.now(), start);
        clock.advance(SignedDuration::from_secs(90));
        assert_eq!(app_side.now(), start + SignedDuration::from_secs(90));
        clock.set(start);
        assert_eq!(app_side.now(), start);
    }

    #[test]
    fn the_store_clock_reads_the_same_instant() {
        let clock = ManualClock::new(Timestamp::UNIX_EPOCH);
        let shared: Arc<dyn Clock> = Arc::new(clock.clone());
        let store_clock = for_store(&shared);
        clock.advance(SignedDuration::from_secs(5));
        assert_eq!(store_clock(), Timestamp::UNIX_EPOCH + SignedDuration::from_secs(5));
    }
}
