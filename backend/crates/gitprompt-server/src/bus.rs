//! In-process fan-out of "something was captured" notices to every open dashboard stream (SSE at
//! `/api/prompts/stream`) and to the desktop shell. Port of `api/bus.py` over a tokio broadcast
//! channel.
//!
//! A notice only means "reload": a stalled subscriber that falls more than `backlog` notices behind
//! loses the older ones, and publishing never blocks, fails or waits for subscribers.
//!
//! Notifications the daemon decided on (the notifier, `notify.rs`) travel on a second channel of the same
//! bus, so a reader of notices never sees one and the notices stay exactly what they were.

use serde::{Deserialize, Serialize};

use gitprompt_insights::notify::Notification;

/// What one ingest stored: `{"prompts": n, "events": m}` (the SSE `data:` payload).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notice {
    pub prompts: u64,
    pub events: u64,
}

use tokio::sync::broadcast::{self, error::RecvError, error::TryRecvError};

impl From<gitprompt_capture::pipeline::Notice> for Notice {
    fn from(n: gitprompt_capture::pipeline::Notice) -> Self {
        Notice { prompts: n.prompts, events: n.events }
    }
}

#[derive(Debug, Clone)]
pub struct Bus {
    tx: broadcast::Sender<Notice>,
    notifications: broadcast::Sender<Notification>,
}

impl Bus {
    /// A bus whose subscribers may fall `backlog` notices (or notifications) behind (at least 1).
    pub fn new(backlog: usize) -> Self {
        let (tx, _) = broadcast::channel(backlog.max(1));
        let (notifications, _) = broadcast::channel(backlog.max(1));
        Bus { tx, notifications }
    }

    /// Send to every current subscriber. No subscribers: nothing happens.
    pub fn publish(&self, notice: Notice) {
        // `send` only fails when nobody listens, which is not an error for a "reload" hint.
        let _ = self.tx.send(notice);
    }

    /// Notices published from now on.
    pub fn subscribe(&self) -> Subscription {
        Subscription { rx: self.tx.subscribe() }
    }

    /// Open subscriptions to notices (a dropped [`Subscription`] no longer counts).
    pub fn subscribers(&self) -> usize {
        self.tx.receiver_count()
    }

    /// Send a notification to every current subscriber of notifications. None: nothing happens.
    pub fn notify(&self, notification: Notification) {
        // As for notices: nobody listening (no window, no shell) is not an error.
        let _ = self.notifications.send(notification);
    }

    /// Notifications sent from now on.
    pub fn notifications(&self) -> Subscription<Notification> {
        Subscription { rx: self.notifications.subscribe() }
    }
}

/// What one subscriber receives: notices by default, or notifications ([`Bus::notifications`]).
#[derive(Debug)]
pub struct Subscription<T = Notice> {
    rx: broadcast::Receiver<T>,
}

impl<T: Clone + Send + 'static> Subscription<T> {
    /// The next notice, waiting for one. Notices lost to lag are skipped silently; `None` only when
    /// the bus is gone.
    pub async fn next(&mut self) -> Option<T> {
        loop {
            match self.rx.recv().await {
                Ok(notice) => return Some(notice),
                Err(RecvError::Lagged(_)) => {}
                Err(RecvError::Closed) => return None,
            }
        }
    }

    /// The next notice already received, without waiting (lag skipped as in [`Self::next`]).
    pub fn try_next(&mut self) -> Option<T> {
        loop {
            match self.rx.try_recv() {
                Ok(notice) => return Some(notice),
                Err(TryRecvError::Lagged(_)) => {}
                Err(TryRecvError::Empty | TryRecvError::Closed) => return None,
            }
        }
    }

    /// The subscription as a stream of notices (lag skipped), for the SSE route.
    pub fn into_stream(self) -> impl tokio_stream::Stream<Item = T> + Send + 'static {
        use tokio_stream::StreamExt;
        tokio_stream::wrappers::BroadcastStream::new(self.rx).filter_map(Result::ok)
    }
}
