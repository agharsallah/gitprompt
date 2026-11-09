//! The hook forwarder's logic (new in the Rust port): what `gitprompt forward <source>` does with the
//! payload a tool pipes to it. It must never disturb the tool: whatever happens it prints only the
//! configured reply (Cursor reads `{"continue":true}` / `{}`), and never fails (exit 0).
//!
//! - The request is `POST <receiver>/ingest/<source>` with `Authorization: Bearer <token>`,
//!   `Content-Type: application/json`, the payload bytes unchanged, a 2 s timeout.
//! - Daemon unreachable (refused, timed out): the payload is appended to the [`Spool`], to be drained
//!   at the daemon's next start. A payload that isn't a JSON object is not spooled (it could never be
//!   ingested).
//! - Any HTTP answer counts as delivered from the forwarder's point of view; an error status (stale
//!   token, broken daemon) is not spooled, because replaying it would fail the same way.
//!
//! The network is behind [`Transport`]; the CLI provides the real one.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::spool::Spool;

/// curl's `-m 2` in the old shell forwarder.
pub const TIMEOUT: Duration = Duration::from_secs(2);

/// An HTTP request to the receiver.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookRequest {
    pub method: &'static str,
    pub url: String,
    /// (name, value), in this order: `Content-Type`, `Authorization`.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub timeout: Duration,
}

/// The receiver couldn't be reached at all (no HTTP answer).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("receiver unreachable: {0}")]
pub struct Unreachable(pub String);

/// Sends a request; `Ok(status)` for any HTTP answer.
pub trait Transport {
    fn send(&self, request: &HookRequest) -> Result<u16, Unreachable>;
}

/// The request for one payload.
pub fn build_request(_receiver: &str, _source: &str, _token: &str, _payload: &[u8]) -> HookRequest {
    todo!()
}

/// What happened to one payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The receiver answered 2xx.
    Delivered { status: u16 },
    /// The receiver answered with an error status; not queued.
    Rejected { status: u16 },
    /// Unreachable: queued in the spool.
    Spooled,
    /// Unreachable and not queueable (not a JSON object, or the spool couldn't be written).
    Dropped { reason: String },
}

/// The result of one forwarder run: what to print on stdout (the reply, or nothing) and what
/// happened. The process always exits 0.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forwarded {
    pub stdout: String,
    pub outcome: Outcome,
}

pub struct Forwarder<'a> {
    /// `Settings::receiver()`.
    pub receiver: String,
    pub token: String,
    pub spool: Spool,
    pub transport: &'a dyn Transport,
}

impl Forwarder<'_> {
    /// Forward the payload read from stdin for `source`; `reply` is printed whatever happens.
    pub fn forward(&self, _source: &str, _stdin: &[u8], _reply: Option<&str>) -> Forwarded {
        todo!()
    }
}
