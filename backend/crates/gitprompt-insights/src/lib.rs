//! Insights: everything GitPrompt derives from stored data on read. Nothing here is stored as truth,
//! so a better rule applies to old prompts too; the prompt index (gitprompt-store) only caches what
//! [`indexer`] projects from these derivations. Port of `src/gitprompt/insights/*`
//! (`redaction.py` lives in `gitprompt_core::secrets`).
//!
//! | module          | derives                                                                   |
//! |-----------------|---------------------------------------------------------------------------|
//! | [`attention`]   | what needs you: sessions to pick up, things worth a look (one feed, every reader) |
//! | [`target`]      | where an item or a notification opens in the dashboard                   |
//! | [`notify`]      | what deserves a notification (rules per kind, a rate limit per kind)      |
//! | [`origin`]      | who produced a prompt, the text the user typed, the injected context      |
//! | [`surface`]     | where a prompt was typed (CLI, VS Code, a desktop app...)                 |
//! | [`derivations`] | prompt stats, the turn summary, the session summary, [`derivations::derive`] |
//! | [`delta`]       | each key figure against the period before (the Overview's headline)     |
//! | [`facts`]       | the few values every page shows about a turn (ADR-0021)                   |
//! | [`label`]      | the default name of a session: what you first typed in it                 |
//! | [`leaks`]       | possible leaks in a turn: secrets, personal data, sensitive files (masked) |
//! | [`lineage`]   | similarity, word diff and outcome change between versions of a prompt     |
//! | [`stories`]     | session chapters, project phases, commit attribution, a prompt's versions |
//! | [`redone`]      | what the turns you had to redo cost (failed, interrupted, reworded)     |
//! | [`recap`]       | the weekly recap: its words and when it is due                           |
//! | [`snippets`]    | search result snippets with every match marked                            |
//! | [`find`]        | one search over prompts, sessions and projects, ranked and grouped        |
//! | [`catalog`]    | the Fields page: fields seen next to the fields each tool documents       |
//! | [`coverage`]    | which payload keys are mapped, derived, shown or ignored by a spec        |
//! | [`captures`]    | the newest prompt of each connector and surface (`gitprompt check`)       |
//! | [`indexer`]     | keeps the prompt index in step with the raw rows                          |
//! | [`zone`]        | the timezone day and hour buckets use (injectable; never the process TZ in tests) |
//!
//! Pure functions take rows as values (`PromptView`, `EventRow`, `IndexRow`...), never a database;
//! only [`captures`] and [`indexer`] read a [`gitprompt_store::Store`].
//!
//! Payload values a typed field expects as text but that arrive as another JSON type (a number, an
//! object) are kept as their JSON text, so nothing a tool reported is silently dropped.

use std::collections::BTreeMap;
use std::sync::Arc;

use gitprompt_core::spec::Spec;

pub mod attention;
pub mod captures;
pub mod catalog;
pub mod coverage;
pub mod delta;
pub mod derivations;
pub mod facts;
pub mod find;
pub mod indexer;
pub mod label;
pub mod leaks;
pub mod lineage;
pub mod notify;
pub mod origin;
pub mod recap;
pub mod redone;
pub mod snippets;
pub mod stories;
pub mod surface;
pub mod target;
pub mod zone;

mod difflib;
mod raw;
mod text;
mod time;

/// The usable specs by id, as `gitprompt_core::spec::load_specs` returns them.
pub type Specs = BTreeMap<String, Arc<Spec>>;
