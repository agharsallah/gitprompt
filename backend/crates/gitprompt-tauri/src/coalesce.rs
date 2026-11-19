//! One rebuild for a burst of changes, with no timer: a change while a rebuild runs asks for one
//! more run after it, however many changes came meanwhile. A turn sends dozens of notices; the tray
//! reads the store once or twice for them, never once per notice.

use std::sync::{Mutex, PoisonError};

#[derive(Debug, Default)]
pub struct Coalesce {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    running: bool,
    again: bool,
}

impl Coalesce {
    /// Something changed. `true`: start a run (none is running); `false`: the running one will
    /// run again ([`Coalesce::finished`]).
    pub fn changed(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        if state.running {
            state.again = true;
            return false;
        }
        state.running = true;
        true
    }

    /// A run finished. `true`: run again (something changed during it).
    pub fn finished(&self) -> bool {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.running = std::mem::take(&mut state.again);
        state.running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_during_a_run_is_one_more_run() {
        let c = Coalesce::default();
        assert!(c.changed(), "the first change starts a run");
        assert!(!c.changed() && !c.changed() && !c.changed(), "the burst waits for the run");
        assert!(c.finished(), "one more run for the whole burst");
        assert!(!c.finished(), "nothing changed during it: done");
    }

    #[test]
    fn a_change_after_a_run_starts_a_new_one() {
        let c = Coalesce::default();
        assert!(c.changed());
        assert!(!c.finished());
        assert!(c.changed());
    }
}
