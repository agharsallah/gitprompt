//! The hook forwarder's logic (new in the Rust port): what `gitprompt hook <source>` does with the
//! payload a tool pipes to it. It must never disturb the tool: whatever happens it prints only the
//! reply the tool expects ([`reply`], from the spec's `install.reply`; Cursor reads
//! `{"continue":true}` / `{}`), and never fails (exit 0).
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

use gitprompt_core::Json;
use gitprompt_core::json::first;
use gitprompt_core::spec::{Spec, by_name};

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
pub fn build_request(receiver: &str, source: &str, token: &str, payload: &[u8]) -> HookRequest {
    HookRequest {
        method: "POST",
        url: format!("{}/ingest/{source}", receiver.trim_end_matches('/')),
        headers: vec![("Content-Type".into(), "application/json".into()), ("Authorization".into(), format!("Bearer {token}"))],
        body: payload.to_vec(),
        timeout: TIMEOUT,
    }
}

/// What the tool expects the hook to print for this payload (spec `install.reply`), as compact JSON:
/// the entry of the tool's own event name (the payload's mapped `event`, before GitPrompt renames
/// it), else the `*` entry; `None` (print nothing) when neither exists. A payload that isn't JSON
/// gets the `*` entry.
pub fn reply(spec: &Spec, payload: &[u8]) -> Option<String> {
    if spec.install.reply.is_empty() {
        return None;
    }
    let payload: Json = serde_json::from_slice(payload).unwrap_or(Json::Null);
    let event = spec.map.get("event").and_then(|paths| first(&payload, paths)).and_then(Json::as_str);
    let answer = by_name(&spec.install.reply, event).or_else(|| spec.install.reply.get("*"))?;
    // A `Json` value always serializes.
    serde_json::to_string(answer).ok()
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
    /// Forward the payload a tool piped to the hook of `spec`, printing the reply the spec names
    /// ([`reply`]) whatever happens.
    pub fn forward_hook(&self, spec: &Spec, stdin: &[u8]) -> Forwarded {
        self.forward(&spec.id, stdin, reply(spec, stdin).as_deref())
    }

    /// Forward the payload read from stdin for `source`; `reply` is printed whatever happens.
    pub fn forward(&self, source: &str, stdin: &[u8], reply: Option<&str>) -> Forwarded {
        let outcome = self.deliver(source, stdin);
        tracing::debug!(%source, ?outcome, "hook payload forwarded");
        Forwarded { stdout: reply.unwrap_or_default().to_owned(), outcome }
    }

    fn deliver(&self, source: &str, stdin: &[u8]) -> Outcome {
        match self.transport.send(&build_request(&self.receiver, source, &self.token, stdin)) {
            Ok(status) if (200..300).contains(&status) => Outcome::Delivered { status },
            Ok(status) => {
                tracing::warn!(%source, status, "the receiver refused a hook payload; not queued");
                Outcome::Rejected { status }
            }
            Err(unreachable) => {
                tracing::info!(%source, error = %unreachable, "GitPrompt is not running; queueing the payload");
                self.queue(source, stdin)
            }
        }
    }

    fn queue(&self, source: &str, stdin: &[u8]) -> Outcome {
        let payload = match serde_json::from_slice::<Json>(stdin) {
            Ok(payload @ Json::Object(_)) => payload,
            _ => return Outcome::Dropped { reason: "the payload is not a JSON object".into() },
        };
        match self.spool.append(source, &payload) {
            Ok(()) => Outcome::Spooled,
            Err(e) => {
                tracing::warn!(%source, error = %e, "could not queue a hook payload");
                Outcome::Dropped { reason: format!("could not queue it: {e}") }
            }
        }
    }
}
