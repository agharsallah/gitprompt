//! Wire types: what each route answers and the Service returns. Field names and optionality are the
//! wire contract with the dashboard, whose TypeScript types are generated from these
//! (`web/src/lib/api/generated.ts`, `just types`; `tests/ts_types.rs` fails while it is stale).
//!
//! Shapes computed by the insights crate (prompt stats and turn summary, session derivations, the
//! field catalog, stories, lineage) are re-exported from there, so they are typed end to end.
//!
//! One submodule per API area; everything is re-exported here, so `crate::model::X` is the path.

mod attention;
mod data;
mod export;
mod find;
mod leaks;
mod names;
mod prompts;
mod security;
mod settings;
mod sources;
mod stats;
mod stories;
mod usage;

pub use attention::{Attention, PickUp, PickUpPresence, Target, WorthAction, WorthItem, WorthKind};
pub use data::{Backup, CompactIn, DataOut, DeleteIn, RemovalOut, ReportOut, SourceEvents};
pub use export::{BranchQuery, ChartInfo, ExportFile, ExportFormat, ExportKind, ExportQuery};
pub use find::{FindHit, FindQuery, FindResults, FindTarget};
pub use leaks::{LeakDecision, LeakIn, LeakOut, LeakPlace, LeakSaved, Leaks, LeaksChecked};
pub use names::{NameIn, NameOut};
pub use prompts::{Annotation, Injected, PromptDetail, PromptOut, PromptQuery, Updated};
pub use security::{Reconnect, Rotation, SecurityOut};
pub use settings::{SettingOut, SettingsOut, SettingsSaved};
pub use sources::{CapturedSurface, Change, Enabled, HOURS, ProjectRef, RecentCapture, SourceOut, SpecFileOut, SpecProblemOut, SpecText};
pub use stats::{Stats, StatsQuery, Telemetry};
pub use stories::{CommitLookup, CommitQuery, FoundCommit, Lineage, ProjectCommits, ProjectStory, SearchHit, SearchResults, SessionStory};
pub use usage::{UsageDay, UsageHit, UsageIn, UsageOut};

/// What the live stream sends as `event: notification` (decided by insights, sent by the notifier).
pub use gitprompt_insights::notify::{Kind as NotificationKind, Notification};

/// What every page shows about a turn (insights facts, ADR-0021), and its parts.
pub use gitprompt_insights::facts::{Connector, Facts, ModelSource, Tokens};

/// `GET /api/fields`: every captured field next to what the tool documents.
pub use gitprompt_insights::catalog::Catalog;
