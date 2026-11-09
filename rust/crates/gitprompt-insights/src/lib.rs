//! Insights: everything GitPrompt derives from stored data on read. Nothing here is stored as truth,
//! so a better rule applies to old prompts too; the prompt index (gitprompt-store) only caches what
//! [`indexer`] projects from these derivations. Port of `src/gitprompt/insights/*`
//! (`redaction.py` lives in `gitprompt_core::secrets`).
//!
//! | module          | derives                                                                   |
//! |-----------------|---------------------------------------------------------------------------|
//! | [`origin`]      | who produced a prompt, the text the user typed, the injected context      |
//! | [`surface`]     | where a prompt was typed (CLI, VS Code, a desktop app...)                 |
//! | [`derivations`] | prompt stats, the turn summary, the session summary, [`derivations::derive`] |
//! | [`facts`]       | the few values every page shows about a turn (ADR-0021)                   |
//! | [`lineage`]     | similarity, word diff and outcome change between versions of a prompt     |
//! | [`stories`]     | session chapters, project phases, commit attribution, a prompt's versions |
//! | [`snippets`]    | search result snippets with every match marked                            |
//! | [`catalog`]     | the Fields page: fields seen next to the fields each tool documents       |
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

pub mod captures;
pub mod catalog;
pub mod coverage;
pub mod derivations;
pub mod facts;
pub mod indexer;
pub mod lineage;
pub mod origin;
pub mod snippets;
pub mod stories;
pub mod surface;
pub mod zone;

/// The usable specs by id, as `gitprompt_core::spec::load_specs` returns them.
pub type Specs = BTreeMap<String, Arc<Spec>>;
