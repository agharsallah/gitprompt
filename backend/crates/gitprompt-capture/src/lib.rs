//! Capture: everything between a tool and the store.
//!
//! - [`otlp`] decodes OpenTelemetry (OTLP/HTTP JSON) exports into one plain record per log/metric point.
//! - [`pipeline`] runs one request body through a source spec into prompts and events.
//! - [`probe`] reads what is on this machine: is the tool installed, its version, the policy.
//! - [`installer`] holds the JSON-config mechanics (merge, strip, backup, diff).
//! - [`connection`] connects/disconnects a source with consent, and computes a connector's state.
//! - [`rotation`] rotates the receiver token and reconnects every connected source.
//! - [`spool`] (new) queues hook payloads on disk while the daemon is down; the daemon drains it.
//! - [`forwarder`] (new) the logic of the hook forwarder: build the request, send, or spool, and
//!   print the reply the tool expects.
//!
//! Port of `src/gitprompt/capture/{pipeline,otlp}.py` and `src/gitprompt/sources/{probe,installer,
//! connection,rotation}.py`. Everything here is synchronous: the store is sync, and the server calls
//! [`pipeline::capture`] from `spawn_blocking`.

pub mod connection;
pub mod forwarder;
pub mod installer;
pub mod otlp;
pub mod pipeline;
pub mod probe;
pub mod rotation;
pub mod spool;

pub use connection::{Change, Connection, ConnectorState};
pub use pipeline::{CaptureOptions, Captured, Observer, capture, capture_at, capture_then};
pub use spool::Spool;
