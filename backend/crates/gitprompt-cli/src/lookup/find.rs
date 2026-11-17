//! `gitprompt find [words] [--project P] [--limit N] [--print N] [--json]`.

use serde::Serialize;

use gitprompt_core::Settings;
use gitprompt_server::service::PromptMatch;
use gitprompt_store::filter::Outcome;

use super::{block, open};
use crate::commands::Outcome as Exit;
use crate::print::{Cell, Style, Tone, ago, columns, first_line};
use crate::{FindArgs, Io};

/// Characters of the first line shown per match.
const LINE: usize = 80;

pub(crate) fn find(settings: &Settings, args: &FindArgs, io: &mut Io<'_>) -> Exit {
    let Some(service) = open(settings, io)? else { return Ok(1) };
    let q = args.words.join(" ");
    let wanted = args.print.map_or(args.limit, |n| n.max(args.limit));
    let mut matches = block(service.find_prompts(&q, args.project.as_deref(), wanted))??;
    if let Some(n) = args.print {
        return print_text(&matches, n, io);
    }
    matches.truncate(args.limit);
    let shown = &matches;
    if args.json {
        writeln!(io.stdout, "{}", serde_json::to_string_pretty(&records(shown))?)?;
    } else if shown.is_empty() {
        return nothing(&q, args.project.as_deref(), io);
    } else {
        let style = Style::new(io.tty, &settings.env);
        for line in columns(&rows(shown, io.now), style) {
            writeln!(io.stdout, "{line}")?;
        }
    }
    Ok(u8::from(shown.is_empty()))
}

/// What to say, on stderr, when nothing matched.
fn nothing(q: &str, project: Option<&str>, io: &mut Io<'_>) -> Exit {
    let words = if q.is_empty() { String::new() } else { format!(" matching {q:?}") };
    let place = project.map(|p| format!(" in project {p:?}")).unwrap_or_default();
    writeln!(io.stderr, "No prompts{words}{place}.")?;
    Ok(1)
}

/// `--print N`: the whole text of match N, as typed, and nothing else, so it pipes. A terminal
/// gets the final newline; a pipe gets the text exactly (a paste must not end in Enter).
fn print_text(matches: &[PromptMatch], n: usize, io: &mut Io<'_>) -> Exit {
    let Some(found) = n.checked_sub(1).and_then(|i| matches.get(i)) else {
        writeln!(io.stderr, "No match {n}: {} prompts matched.", matches.len())?;
        return Ok(1);
    };
    write!(io.stdout, "{}", found.text)?;
    if io.tty {
        writeln!(io.stdout)?;
    }
    Ok(0)
}

/// `1  3 min ago  project  first line`.
fn rows(matches: &[PromptMatch], now: jiff::Timestamp) -> Vec<Vec<Cell>> {
    let number_width = matches.len().to_string().len();
    matches
        .iter()
        .enumerate()
        .map(|(i, m)| {
            vec![
                Cell::new(format!("{:>number_width$}", i + 1), Tone::Dim),
                Cell::new(ago(now, &m.hit.at), Tone::Dim),
                Cell::new(m.project.as_deref().unwrap_or("-"), Tone::Accent),
                Cell::new(first_line(&m.text, LINE), Tone::Plain),
            ]
        })
        .collect()
}

/// One match for scripts.
#[derive(Serialize)]
struct Record<'a> {
    n: usize,
    id: i64,
    at: &'a str,
    project: Option<&'a str>,
    source: &'a str,
    outcome: Option<Outcome>,
    text: &'a str,
}

fn records(matches: &[PromptMatch]) -> Vec<Record<'_>> {
    matches
        .iter()
        .enumerate()
        .filter_map(|(i, m)| {
            let gitprompt_server::model::FindTarget::Prompt { id } = m.hit.target else { return None };
            Some(Record {
                n: i + 1,
                id,
                at: &m.hit.at,
                project: m.project.as_deref(),
                source: &m.source,
                outcome: m.hit.outcome,
                text: &m.text,
            })
        })
        .collect()
}
