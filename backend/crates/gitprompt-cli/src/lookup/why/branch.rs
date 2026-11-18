//! `gitprompt why --branch <name> [--project P]`: the prompts behind a branch, the Markdown block
//! the server renders for the dashboard (`Service::export_branch`), printed as is.

use gitprompt_core::Settings;
use gitprompt_server::model::BranchQuery;

use crate::Io;
use crate::commands::Outcome as Exit;
use crate::lookup::{block, open};

pub(super) fn print(settings: &Settings, branch: &str, project: Option<&str>, io: &mut Io<'_>) -> Exit {
    let Some(service) = open(settings, io)? else { return Ok(1) };
    let Some(id) = block(service.branch_project(branch, project))?? else { return nothing(branch, project, io) };
    let file = block(service.export_branch(BranchQuery { project: id, branch: branch.to_owned() }))??;
    io.stdout.write_all(&file.body)?;
    Ok(0)
}

/// What to say, on stderr, when no prompt you typed was recorded on the branch.
fn nothing(branch: &str, project: Option<&str>, io: &mut Io<'_>) -> Exit {
    let place = project.map(|p| format!(" in project {p:?}")).unwrap_or_default();
    writeln!(io.stderr, "No prompts you typed were recorded on branch {branch:?}{place}.")?;
    Ok(1)
}
