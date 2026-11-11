//! A reentrant, per-thread gate in front of the writer connection. The thread inside it may enter
//! again (a store call made inside `Store::transaction`), every other thread waits. This is what
//! lets all the store calls made inside a transaction share it without deadlocking.
//!
//! The gate also measures itself: how long each outermost entry waited to get in, and how long it
//! stayed inside. One long stay (child processes or slow work inside a transaction) stalls every
//! other write and every index refresh before a read, so a wait over [`SLOW_WAIT`] or a stay over
//! [`SLOW_HOLD`] is logged at `warn` (target `gitprompt_store::writer`) with the code location that
//! entered, and both are counted in [`WriterStats`].

use std::panic::Location;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};

/// Waiting longer than this to get the writer is logged and counted.
pub const SLOW_WAIT: Duration = Duration::from_millis(250);
/// Keeping the writer longer than this is logged and counted.
pub const SLOW_HOLD: Duration = Duration::from_millis(500);

/// How the writer has been shared since the store opened (outermost entries only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct WriterStats {
    /// Entries into the writer.
    pub entries: u64,
    /// Entries that waited longer than [`SLOW_WAIT`].
    pub slow_waits: u64,
    /// Entries that kept the writer longer than [`SLOW_HOLD`].
    pub slow_holds: u64,
    pub max_wait_ms: u64,
    pub max_hold_ms: u64,
}

impl WriterStats {
    /// What changed since `before`, an earlier reading of the same store: counts are differences,
    /// and a maximum is kept only when it was set after `before` (0 otherwise).
    #[must_use]
    pub fn since(self, before: WriterStats) -> WriterStats {
        let newer = |now: u64, then: u64| if now > then { now } else { 0 };
        WriterStats {
            entries: self.entries.saturating_sub(before.entries),
            slow_waits: self.slow_waits.saturating_sub(before.slow_waits),
            slow_holds: self.slow_holds.saturating_sub(before.slow_holds),
            max_wait_ms: newer(self.max_wait_ms, before.max_wait_ms),
            max_hold_ms: newer(self.max_hold_ms, before.max_hold_ms),
        }
    }
}

#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<State>,
    released: Condvar,
    entries: AtomicU64,
    slow_waits: AtomicU64,
    slow_holds: AtomicU64,
    max_wait_ms: AtomicU64,
    max_hold_ms: AtomicU64,
}

#[derive(Default)]
struct State {
    owner: Option<ThreadId>,
    depth: usize,
    /// When the outermost entry got in, and where it was made.
    since: Option<(Instant, &'static Location<'static>)>,
}

/// Inside the gate until dropped.
pub(crate) struct Pass<'a> {
    gate: &'a Gate,
}

impl Gate {
    /// Enter, waiting while another thread is inside.
    #[track_caller]
    pub(super) fn enter(&self) -> Pass<'_> {
        let at = Location::caller();
        let me = thread::current().id();
        let asked = Instant::now();
        let mut state = self.lock();
        while state.owner.is_some_and(|owner| owner != me) {
            state = self.released.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
        state.owner = Some(me);
        state.depth += 1;
        let outermost = state.depth == 1;
        let now = Instant::now();
        if outermost {
            state.since = Some((now, at));
        }
        drop(state);
        if outermost {
            self.entries.fetch_add(1, Ordering::Relaxed);
            let waited = now.saturating_duration_since(asked);
            self.max_wait_ms.fetch_max(millis(waited), Ordering::Relaxed);
            if waited > SLOW_WAIT {
                self.slow_waits.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(target: "gitprompt_store::writer", wait_ms = millis(waited), at = %at, "waited for the database writer");
            }
        }
        Pass { gate: self }
    }

    /// Whether the current thread is inside (so the writer holds its uncommitted writes).
    pub(super) fn held_here(&self) -> bool {
        self.lock().owner == Some(thread::current().id())
    }

    pub(super) fn stats(&self) -> WriterStats {
        WriterStats {
            entries: self.entries.load(Ordering::Relaxed),
            slow_waits: self.slow_waits.load(Ordering::Relaxed),
            slow_holds: self.slow_holds.load(Ordering::Relaxed),
            max_wait_ms: self.max_wait_ms.load(Ordering::Relaxed),
            max_hold_ms: self.max_hold_ms.load(Ordering::Relaxed),
        }
    }

    /// The state is only changed by `enter` and `Pass::drop`, neither of which can panic halfway,
    /// so a poisoned lock still guards a consistent state.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn held(&self, since: Instant, at: &'static Location<'static>) {
        let held = since.elapsed();
        self.max_hold_ms.fetch_max(millis(held), Ordering::Relaxed);
        if held > SLOW_HOLD {
            self.slow_holds.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(target: "gitprompt_store::writer", hold_ms = millis(held), at = %at, "held the database writer");
        }
    }
}

impl Drop for Pass<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.depth = state.depth.saturating_sub(1);
        let mut left = None;
        if state.depth == 0 {
            state.owner = None;
            left = state.since.take();
            self.gate.released.notify_one();
        }
        drop(state);
        if let Some((since, at)) = left {
            self.gate.held(since, at); // measured and logged outside the lock
        }
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_thread_inside_can_enter_again() {
        let gate = Gate::default();
        let outer = gate.enter();
        let inner = gate.enter();
        assert!(gate.held_here());
        drop(inner);
        assert!(gate.held_here());
        drop(outer);
        assert!(!gate.held_here());
    }

    #[test]
    fn another_thread_waits_until_the_gate_is_left() {
        let gate = Gate::default();
        let entered = AtomicBool::new(false);
        let pass = gate.enter();
        thread::scope(|scope| {
            let other = scope.spawn(|| {
                let _pass = gate.enter();
                entered.store(true, Ordering::SeqCst);
            });
            thread::sleep(Duration::from_millis(50));
            assert!(!entered.load(Ordering::SeqCst));
            drop(pass);
            other.join().unwrap();
        });
        assert!(entered.load(Ordering::SeqCst));
    }

    #[test]
    fn only_outermost_entries_are_counted_and_a_long_stay_is_a_slow_hold() {
        let gate = Gate::default();
        {
            let _outer = gate.enter();
            let _inner = gate.enter();
            thread::sleep(SLOW_HOLD + Duration::from_millis(20));
        }
        let stats = gate.stats();
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.slow_holds, 1);
        assert!(stats.max_hold_ms >= millis(SLOW_HOLD));
        assert_eq!(stats.slow_waits, 0);
    }

    #[test]
    fn waiting_behind_another_thread_is_a_slow_wait() {
        let gate = Gate::default();
        let pass = gate.enter();
        thread::scope(|scope| {
            let other = scope.spawn(|| drop(gate.enter()));
            thread::sleep(SLOW_WAIT + Duration::from_millis(50));
            drop(pass);
            other.join().unwrap();
        });
        let stats = gate.stats();
        assert_eq!(stats.entries, 2);
        assert_eq!(stats.slow_waits, 1);
        assert!(stats.max_wait_ms >= millis(SLOW_WAIT));
    }
}
