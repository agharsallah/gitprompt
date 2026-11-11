//! A reentrant, per-thread gate in front of the writer connection. The thread inside it may enter
//! again (a store call made inside `Store::transaction`), every other thread waits. This is what
//! lets all the store calls made inside a transaction share it without deadlocking.

use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::{self, ThreadId};

#[derive(Default)]
pub(crate) struct Gate {
    state: Mutex<State>,
    released: Condvar,
}

#[derive(Default)]
struct State {
    owner: Option<ThreadId>,
    depth: usize,
}

/// Inside the gate until dropped.
pub(crate) struct Pass<'a> {
    gate: &'a Gate,
}

impl Gate {
    /// Enter, waiting while another thread is inside.
    pub(super) fn enter(&self) -> Pass<'_> {
        let me = thread::current().id();
        let mut state = self.lock();
        while state.owner.is_some_and(|owner| owner != me) {
            state = self.released.wait(state).unwrap_or_else(PoisonError::into_inner);
        }
        state.owner = Some(me);
        state.depth += 1;
        Pass { gate: self }
    }

    /// Whether the current thread is inside (so the writer holds its uncommitted writes).
    pub(super) fn held_here(&self) -> bool {
        self.lock().owner == Some(thread::current().id())
    }

    /// The state is only changed by `enter` and `Pass::drop`, neither of which can panic halfway,
    /// so a poisoned lock still guards a consistent state.
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Pass<'_> {
    fn drop(&mut self) {
        let mut state = self.gate.lock();
        state.depth = state.depth.saturating_sub(1);
        if state.depth == 0 {
            state.owner = None;
            self.gate.released.notify_one();
        }
    }
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
}
