//! `gitprompt resume <session id or name> | --last [--json]`: prints the command that resumes a
//! session, `cd <folder> && <command>`, and never runs it.

use serde::Serialize;

use gitprompt_core::Settings;
use gitprompt_core::spec::Resume;
use gitprompt_server::model::FindHit;
use gitprompt_server::service::{SessionPick, session_id};

use super::{block, open};
use crate::commands::Outcome as Exit;
use crate::print::ago;
use crate::{Io, ResumeArgs};

pub(crate) fn resume(settings: &Settings, args: &ResumeArgs, io: &mut Io<'_>) -> Exit {
    let Some(service) = open(settings, io)? else { return Ok(1) };
    let reference = args.session.as_deref().filter(|_| !args.last);
    let hit = match block(service.find_session(reference))?? {
        SessionPick::One(hit) => hit,
        SessionPick::Several(hits) => return ambiguous(reference.unwrap_or_default(), &hits, io),
        SessionPick::Nothing => return missing(reference, io),
    };
    let Some(id) = session_id(&hit) else { return missing(reference, io) };
    let found = block(service.resume_session(id))??;
    let Some(resume) = found.resume else {
        writeln!(io.stderr, "{} has no way to resume a session from the command line, so there is nothing to print.", found.tool)?;
        return Ok(1);
    };
    if args.json {
        let record = Record { session: id, name: hit.title.as_deref(), tool: &found.tool, resume: &resume };
        writeln!(io.stdout, "{}", serde_json::to_string_pretty(&record)?)?;
    } else {
        writeln!(io.stdout, "{}", resume.line())?;
    }
    Ok(0)
}

fn missing(reference: Option<&str>, io: &mut Io<'_>) -> Exit {
    match reference {
        Some(reference) => writeln!(io.stderr, "No session is called or numbered {reference:?}.")?,
        None => writeln!(io.stderr, "No sessions yet.")?,
    }
    Ok(1)
}

/// Says which sessions the name matches, so the next try can use an id.
fn ambiguous(reference: &str, hits: &[FindHit], io: &mut Io<'_>) -> Exit {
    writeln!(io.stderr, "{} sessions match {reference:?}; run it again with the id of one:", hits.len())?;
    for hit in hits {
        let id = session_id(hit).unwrap_or_default();
        writeln!(io.stderr, "  {id}  {}  {}", ago(io.now, &hit.at), hit.title.as_deref().unwrap_or("(untitled)"))?;
    }
    Ok(1)
}

/// The answer for scripts.
#[derive(Serialize)]
struct Record<'a> {
    session: &'a str,
    /// What the session is called: its name, label or title.
    name: Option<&'a str>,
    tool: &'a str,
    /// `command`, `cwd` and `paste` (what `gitprompt resume` prints and the dashboard copies).
    #[serde(flatten)]
    resume: &'a Resume,
}
