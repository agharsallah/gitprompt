//! SQLite storage. Raw payloads are always kept, so mapped fields can be rebuilt; everything else is
//! derived on read, and the values filters and statistics need in SQL are cached in a rebuildable
//! index. Port of `storage/*`, `capture/compact.py` and `capture/retention.py`.
//!
//! The schema is the one the Python daemon writes: the Rust daemon opens an existing
//! `~/.gitprompt/gitprompt.db` as is (no migration).
//!
//! Concurrency (new): one writer connection plus a small pool of read connections in WAL mode, so
//! dashboard reads never wait behind ingest. [`Store`] is `Send + Sync`; every method takes `&self`.

pub mod filter;
pub mod housekeeping;
pub mod index;
pub mod rows;
pub mod stats;
pub mod store;
pub mod timeline;

pub use filter::{Has, Outcome, PromptFilter};
pub use rows::{Annotation, EventRow, PromptRow, PromptView};
pub use store::Store;
