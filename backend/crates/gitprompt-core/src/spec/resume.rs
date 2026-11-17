//! The resume action of a session: what the dashboard (and one day the CLI) shows to pick it up.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::expand::shell_quote;

/// The command that resumes a session, and the folder to run it in. Only present when the tool's
/// spec has a `resume` template and the session has what it needs: no template, no action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(optional_fields)]
pub struct Resume {
    /// A shell command, values already shell-quoted.
    pub command: String,
    /// The session's working folder (where it started): `cd` there first. Absent when no event
    /// of the session said. Raw, not quoted: for display.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// What to paste into a terminal: `cd <folder> && <command>` with the folder shell-quoted, or
    /// just `command` without a folder. The one place that line is built.
    pub paste: String,
}

impl Resume {
    /// The action for `command` run from `cwd` (an empty folder counts as none).
    pub fn new(command: String, cwd: Option<&str>) -> Resume {
        let cwd = cwd.filter(|cwd| !cwd.is_empty()).map(str::to_owned);
        let mut resume = Resume { command, cwd, paste: String::new() };
        resume.paste = resume.line();
        resume
    }

    /// What to paste into a terminal: `cd <folder> && <command>` (the folder shell-quoted), or the
    /// command alone when no folder is known. The one form the dashboard copies and
    /// `gitprompt resume` prints (same as `Resume::line` of the CLI ticket).
    pub fn line(&self) -> String {
        match &self.cwd {
            Some(cwd) => format!("cd {} && {}", shell_quote(cwd), self.command),
            None => self.command.clone(),
        }
    }
}
