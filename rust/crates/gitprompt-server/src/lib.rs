//! GitPrompt's receiver and dashboard API, the live-update bus, and the [`Service`] facade the Tauri
//! desktop app calls directly. Port of `src/gitprompt/api/**`.
//!
//! Layers, outside in:
//!
//! * [`App::router`] — axum: the security layer ([`security`]), thin route adapters ([`routes`]),
//!   the static dashboard with its SPA fallback ([`dashboard`]), `/healthz`.
//! * [`Service`] — one typed async method per route, returning the wire types of [`model`]. The
//!   HTTP routes only parse the request, call it and serialize the answer, so a Tauri command and
//!   an HTTP call return the same data.
//! * [`Daemon`] — a running instance for the CLI and the desktop shell: single-instance lock,
//!   graceful shutdown, background retention, spool drain at start.
//!
//! Everything nondeterministic the server reads (time, tool versions, git) comes in through
//! [`AppOptions`], so tests pin it.

pub mod app;
pub mod bus;
pub mod daemon;
pub mod dashboard;
pub mod error;
pub mod model;
pub mod options;
pub mod query;
pub mod routes;
pub mod security;
pub mod service;

pub use app::App;
pub use bus::{Bus, Notice, Subscription};
pub use daemon::{Daemon, DaemonError, DaemonOptions};
pub use error::ApiError;
pub use options::{
    AppOptions, Clock, FixedClock, FixedVersion, GitProvider, NoGit, ProbeVersions, SystemClock, SystemGit, ToolVersionProbe,
};
pub use service::Service;
