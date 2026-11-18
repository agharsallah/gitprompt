//! `gitprompt why <commit> [--json]`: which prompt made a commit, and in which session. The commit
//! is looked up the way its project's Commits tab traces it ([`Service::commit`]), so the terminal
//! and the dashboard give the same answer.
//!
//! A hash (or a prefix) is looked up as typed, in any folder; anything else (`HEAD`, `HEAD~2`, a
//! branch) is resolved by git in the current folder first.

use gitprompt_core::Settings;
use gitprompt_core::git::{Revision, resolve_revision};
use gitprompt_insights::stories::MIN_HASH;
use gitprompt_server::Service;
use gitprompt_server::model::{CommitLookup, FoundCommit};
use gitprompt_server::service::{SessionPick, session_id};

use super::verdict;
use crate::Io;
use crate::commands::{Failure, Outcome as Exit};
use crate::lookup::{block, open};
use crate::print::{Cell, Style, Tone, columns};

/// `rev` explained; with `json`, the lookup as JSON.
pub(super) fn print(settings: &Settings, rev: &str, json: bool, io: &mut Io<'_>) -> Exit {
    let rev = rev.trim();
    let Some(hash) = hash_of(rev, io)? else { return Ok(1) };
    let Some(service) = open(settings, io)? else { return Ok(1) };
    let lookup = match block(service.commit(&hash))? {
        Ok(lookup) => lookup,
        Err(e) if e.status() == 404 => return unknown(rev, &hash, io),
        Err(e) => return Err(e.into()),
    };
    if json {
        writeln!(io.stdout, "{}", serde_json::to_string_pretty(&lookup)?)?;
        return Ok(u8::from(matches!(lookup, CommitLookup::Many { .. })));
    }
    match lookup {
        CommitLookup::One { commit } => print_one(&service, settings, &commit, io),
        CommitLookup::Many { candidates } => ambiguous(rev, &candidates, io),
    }
}

/// The hash to look up: what git resolves `rev` to here, else `rev` itself when it reads as a hash
/// (a commit of another repository, or a prefix git finds ambiguous); `None` after saying why
/// there is none.
fn hash_of(rev: &str, io: &mut Io<'_>) -> Result<Option<String>, Failure> {
    let resolved = resolve_revision(&io.cwd, rev);
    if let Revision::Commit(hash) = resolved {
        return Ok(Some(hash));
    }
    if looks_like_hash(rev) {
        return Ok(Some(rev.to_owned()));
    }
    match resolved {
        Revision::NoRepository => writeln!(
            io.stderr,
            "{} is not in a git repository, so {rev:?} names no commit here: run it inside the repository, or give the commit's hash.",
            io.cwd.display()
        )?,
        _ => writeln!(io.stderr, "This repository has no commit {rev:?}.")?,
    }
    Ok(None)
}

fn looks_like_hash(rev: &str) -> bool {
    rev.len() >= MIN_HASH && rev.chars().all(|c| c.is_ascii_hexdigit())
}

fn print_one(service: &Service, settings: &Settings, found: &FoundCommit, io: &mut Io<'_>) -> Exit {
    let trace = &found.trace;
    let alternative = trace.alternative.as_ref().and_then(|a| a.session.as_deref());
    let receiver = settings.receiver();
    let context = verdict::Context {
        session: name_of(service, trace.session.as_deref())?,
        alternative_session: name_of(service, alternative)?,
        dashboard: &receiver,
        now: io.now,
        style: Style::new(io.tty, &settings.env),
    };
    for line in verdict::lines(found, &context) {
        writeln!(io.stdout, "{line}")?;
    }
    Ok(0)
}

/// What a session is called (the name you gave it, else its label), as `find` and `resume` call it.
fn name_of(service: &Service, session: Option<&str>) -> Result<Option<String>, Failure> {
    let Some(id) = session else { return Ok(None) };
    Ok(match block(service.find_session(Some(id)))?? {
        SessionPick::One(hit) if session_id(&hit) == Some(id) => hit.title,
        _ => None,
    })
}

/// Says that no project has the commit, and which commit a revision stood for.
fn unknown(rev: &str, hash: &str, io: &mut Io<'_>) -> Exit {
    let commit = if hash == rev { format!("{rev:?}") } else { format!("{} ({rev})", verdict::short(hash)) };
    writeln!(
        io.stderr,
        "No commit {commit} among your projects' commits: it was made outside the time you prompted in its repository, or in one you never prompted in."
    )?;
    Ok(1)
}

/// Says which commits the prefix names, so the next try can use more of the hash.
fn ambiguous(rev: &str, candidates: &[FoundCommit], io: &mut Io<'_>) -> Exit {
    writeln!(io.stderr, "{} commits start with {rev:?}; run it again with more of the hash:", candidates.len())?;
    let rows: Vec<Vec<Cell>> = candidates
        .iter()
        .map(|found| {
            let commit = &found.trace.commit;
            vec![
                Cell::new(format!("  {}", commit.hash), Tone::Plain),
                Cell::new(found.project.name.clone(), Tone::Plain),
                Cell::new(verdict::day(&commit.time, io.now).unwrap_or_default(), Tone::Plain),
                Cell::new(commit.subject.clone(), Tone::Plain),
            ]
        })
        .collect();
    for line in columns(&rows, Style::new(false, &gitprompt_core::Env::default())) {
        writeln!(io.stderr, "{line}")?;
    }
    Ok(1)
}
