//! How to pick a session up again: the spec's `resume` command filled in for one session, and the
//! folder it ran in.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use gitprompt_core::Json;
use gitprompt_core::spec::Spec;
use gitprompt_store::EventRow;

/// The command that resumes a session, and the folder to run it in. Only present when the tool's
/// spec has a `resume` template and the session has what it needs: no template, no action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Resume {
    /// A shell command, values already shell-quoted.
    pub command: String,
    /// The session's working folder (where it started): `cd` there first. Absent when no event
    /// of the session said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

/// The resume action of a session of the tool `spec` describes. The folder is the first `cwd` any
/// event of the session carries (where it started: tools such as Claude Code look a session up in
/// the folder it began in), else `prompt_cwd`, the one of the prompt being shown.
pub fn resume(spec: &Spec, session_id: &str, events: &[EventRow], prompt_cwd: Option<&str>) -> Option<Resume> {
    let from_events = events.iter().find_map(|event| event.fields.get("cwd").and_then(Json::as_str).filter(|cwd| !cwd.is_empty()));
    let cwd = from_events.or_else(|| prompt_cwd.filter(|cwd| !cwd.is_empty()));
    let command = spec.resume_command(session_id, cwd)?;
    Some(Resume { command, cwd: cwd.map(str::to_owned) })
}
