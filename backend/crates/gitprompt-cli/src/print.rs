//! What the commands print for a person: relative times, aligned columns, colour only where it is
//! wanted. Time comes in (the entry point's clock), so the same input prints the same text.

use jiff::Timestamp;

use gitprompt_core::Env;

/// How a piece of text is set off. Only matters where colour is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Plain,
    /// Secondary: numbers, times.
    Dim,
    /// Something to look for: a project, a name.
    Accent,
}

/// Whether to colour, decided once: only on a terminal, never when `NO_COLOR` is set (to anything
/// but empty, <https://no-color.org>) or the terminal is `dumb`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Style {
    color: bool,
}

impl Style {
    pub(crate) fn new(stdout_is_terminal: bool, env: &Env) -> Self {
        Style { color: stdout_is_terminal && env.get("NO_COLOR").is_none() && env.get("TERM") != Some("dumb") }
    }

    pub(crate) fn paint(self, tone: Tone, text: &str) -> String {
        let code = match tone {
            Tone::Plain => return text.to_owned(),
            Tone::Dim => "2",
            Tone::Accent => "36",
        };
        if self.color { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_owned() }
    }
}

/// `<n> <unit> ago`, the largest unit that fits (`3 min ago`, `2 d ago`); a time in the future or
/// one that is not a timestamp counts as now.
pub(crate) fn ago(now: Timestamp, iso: &str) -> String {
    let seconds = iso.parse::<Timestamp>().map_or(0, |at| (now - at).get_seconds().max(0));
    for (unit, size) in [("d", 86_400), ("h", 3600), ("min", 60)] {
        if seconds >= size {
            return format!("{} {unit} ago", seconds / size);
        }
    }
    format!("{seconds} s ago")
}

/// The first line of `text` that has words, cut to `max` characters with `…`.
pub(crate) fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default();
    if line.chars().count() <= max {
        return line.to_owned();
    }
    let cut: String = line.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}

/// One cell of a row.
pub(crate) struct Cell {
    text: String,
    tone: Tone,
}

impl Cell {
    pub(crate) fn new(text: impl Into<String>, tone: Tone) -> Self {
        Cell { text: text.into(), tone }
    }
}

/// Rows as lines: every column but the last padded to its widest cell (measured on the plain
/// text, so colour never skews the alignment), columns two spaces apart.
pub(crate) fn columns(rows: &[Vec<Cell>], style: Style) -> Vec<String> {
    let width = |col: usize| rows.iter().filter_map(|row| row.get(col)).map(|cell| cell.text.chars().count()).max().unwrap_or(0);
    rows.iter()
        .map(|row| {
            let last = row.len().saturating_sub(1);
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(col, cell)| {
                    let pad = if col == last { 0 } else { width(col).saturating_sub(cell.text.chars().count()) };
                    format!("{}{}", style.paint(cell.tone, &cell.text), " ".repeat(pad))
                })
                .collect();
            cells.join("  ").trim_end().to_owned()
        })
        .collect()
}
