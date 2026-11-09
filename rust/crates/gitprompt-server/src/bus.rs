//! In-process fan-out of "something was captured" notices to every open dashboard stream (SSE at
//! `/api/prompts/stream`) and to the desktop shell. Port of `api/bus.py` over a tokio broadcast
//! channel.
//!
//! A notice only means "reload": a stalled subscriber that falls more than `backlog` notices behind
//! loses the older ones, and publishing never blocks, fails or waits for subscribers.

use serde::{Deserialize, Serialize};

/// What one ingest stored: `{"prompts": n, "events": m}` (the SSE `data:` payload).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub prompts: u64,
    pub events: u64,
}

#[derive(Clone)]
pub struct Bus {
    _tx: tokio::sync::broadcast::Sender<Notice>,
}

impl Bus {
    pub fn new(_backlog: usize) -> Self {
        todo!()
    }

    /// Send to every current subscriber. No subscribers: nothing happens.
    pub fn publish(&self, _notice: Notice) {
        todo!()
    }

    /// Notices published from now on.
    pub fn subscribe(&self) -> Subscription {
        todo!()
    }

    /// Open subscriptions (a dropped [`Subscription`] no longer counts).
    pub fn subscribers(&self) -> usize {
        todo!()
    }
}

pub struct Subscription {
    _rx: tokio::sync::broadcast::Receiver<Notice>,
}

impl Subscription {
    /// The next notice, waiting for one. Notices lost to lag are skipped silently; `None` only when
    /// the bus is gone.
    pub async fn next(&mut self) -> Option<Notice> {
        todo!()
    }

    /// The next notice already received, without waiting (lag skipped as in [`Self::next`]).
    pub fn try_next(&mut self) -> Option<Notice> {
        todo!()
    }
}
