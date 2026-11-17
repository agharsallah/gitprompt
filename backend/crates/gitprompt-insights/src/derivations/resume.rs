//! How to pick a session up again: the spec's `resume` command filled in for one session, and the
//! folder it ran in.

use gitprompt_core::Json;
use gitprompt_core::spec::{Resume, Spec};
use gitprompt_store::EventRow;

/// The resume action of a session of the tool `spec` describes. The folder is the first `cwd` any
/// event of the session carries (where it started: tools such as Claude Code look a session up in
/// the folder it began in), else `prompt_cwd`, the one of the prompt being shown.
pub fn resume(spec: &Spec, session_id: &str, events: &[EventRow], prompt_cwd: Option<&str>) -> Option<Resume> {
    let from_events = events.iter().find_map(|event| event.fields.get("cwd").and_then(Json::as_str).filter(|cwd| !cwd.is_empty()));
    spec.resume_action(session_id, from_events.or(prompt_cwd))
}
