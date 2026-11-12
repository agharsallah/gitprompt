//! GitPrompt's shared kernel. No HTTP, no SQL: everything here is usable from the daemon, the CLI
//! and the Tauri desktop shell alike.
//!
//! Port of `src/gitprompt/core/*`, `sources/spec.py` and `insights/redaction.py`.

pub mod blank;
pub mod env;
pub mod error;
pub mod files;
pub mod git;
pub mod json;
pub mod numbers;
pub mod preferences;
mod random;
pub mod secrets;
pub mod settings;
pub mod spec;

pub use blank::Blank;
pub use env::Env;
pub use error::{Error, Result};
pub use json::{Json, JsonMap};
pub use settings::Settings;
