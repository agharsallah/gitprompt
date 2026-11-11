//! What `gitprompt doctor` found, as `--json` prints it. Field names are a stable contract for
//! scripts and bug reports; the text output is rendered from the same values.

use serde::Serialize;

use gitprompt_capture::connection::{ConnectorState, HookForm};
use gitprompt_core::Json;
use gitprompt_core::preferences::ValueSource;
use gitprompt_core::spec::{SpecFile, SpecProblem};
use gitprompt_server::DashboardInfo;
use gitprompt_server::health::HealthOut;
use gitprompt_server::status::{IndexStatus, SpoolBacklog};

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Report {
    pub version: String,
    pub backend: String,
    pub data: Data,
    /// `None` without a database.
    pub index: Option<IndexStatus>,
    pub daemon: DaemonCheck,
    /// What `gitprompt serve` would serve (`GITPROMPT_DASHBOARD`, else the embedded copy).
    pub dashboard: DashboardInfo,
    pub specs: SpecsCheck,
    pub connectors: Vec<ConnectorCheck>,
    pub spool: Vec<SpoolBacklog>,
    /// Preferences that are not at their default.
    pub preferences: Vec<PreferenceSet>,
    /// Things to fix: the exit code is 1 when there is any.
    pub problems: Vec<Finding>,
    /// Worth knowing, nothing to fix.
    pub notes: Vec<Finding>,
    pub healthy: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Data {
    pub home: String,
    pub home_exists: bool,
    pub db_path: String,
    pub db_exists: bool,
    pub db_bytes: u64,
    pub prompts: u64,
    pub events: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct DaemonCheck {
    pub port: u16,
    pub receiver: String,
    /// Something accepts connections on the port.
    pub listening: bool,
    /// `/healthz` answered `{"ok": true}`.
    pub healthz: bool,
    /// What the daemon's `/api/status` says (`None`: not running, or not the Rust backend).
    pub running: Option<RunningDaemon>,
    pub lock: LockCheck,
    pub token_file: String,
    /// The receiver token exists (its value is never shown).
    pub token_present: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct RunningDaemon {
    pub version: String,
    pub backend: String,
    pub started_at: String,
    pub uptime_s: u64,
    pub home: String,
    pub dashboard: DashboardInfo,
    /// Route latencies, writer contention, memory, CPU and recent warnings since it started.
    pub health: HealthOut,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct LockCheck {
    pub file: String,
    /// Another process holds the OS lock (a daemon runs for this data directory).
    pub held: bool,
    /// The pid written in the lock file (by the daemon that last held it).
    pub pid: Option<u32>,
    /// Whether that pid is a running process; `None` when there is no pid or it can't be told.
    pub pid_alive: Option<bool>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SpecsCheck {
    /// Where user spec files go.
    pub user_dir: String,
    /// Every loaded spec and the file it was read from, by id.
    pub files: Vec<SpecFileCheck>,
    pub problems: Vec<SpecProblem>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SpecFileCheck {
    pub id: String,
    #[serde(flatten)]
    pub file: SpecFile,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct ConnectorCheck {
    pub id: String,
    pub name: String,
    pub part_of: Option<String>,
    pub enabled: bool,
    pub detected: bool,
    pub connected: bool,
    /// How the installed hooks deliver (`None` when not connected).
    pub install: Option<HookForm>,
    pub config_file: String,
    /// As the Connectors page computes it.
    pub state: ConnectorState,
    pub state_label: String,
    pub prompts: u64,
    pub events: u64,
    pub last_at: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct PreferenceSet {
    pub key: String,
    pub value: Json,
    pub source: ValueSource,
    /// The variable that sets (or would lock) it.
    pub env: String,
}

/// A problem or a note, with what to do about it in one line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Finding {
    pub what: String,
    pub fix: String,
}

impl Finding {
    pub(super) fn new(what: impl Into<String>, fix: impl Into<String>) -> Self {
        Finding { what: what.into(), fix: fix.into() }
    }
}
