//! A report as Markdown (GitHub flavoured): charts become tables of their values; text placed in a
//! sentence has its Markdown escaped, and quoted prompt text goes in a fence long enough to hold
//! it, so its links and images stay inert. Port of `export/markdown.py`.

use crate::report::{Block, Chart, Figures, Items, Paragraph, Quote, Report, Section, Table, Voice};

/// The report as one Markdown document: `# title`, subtitle, sections (`##`/`###`), then a `---`
/// footer with the export date and notes. Ends with a newline.
pub fn markdown(report: &Report) -> String {
    let mut lines = vec![format!("# {}", inline(&report.title)), String::new()];
    if !report.subtitle.is_empty() {
        lines.extend([inline(&report.subtitle), String::new()]);
    }
    for section in &report.sections {
        push_section(&mut lines, section);
    }
    lines.extend(["---".to_string(), String::new(), format!("_Exported from GitPrompt on {}._", report.generated_at), String::new()]);
    lines.extend(report.notes.iter().map(|note| format!("- {}", inline(note))));
    lines.join("\n") + "\n"
}

fn push_section(lines: &mut Vec<String>, section: &Section) {
    let hashes = match section.level {
        crate::report::Level::Two => "##",
        crate::report::Level::Three => "###",
    };
    lines.extend([format!("{hashes} {}", inline(&section.title)), String::new()]);
    if !section.subtitle.is_empty() {
        lines.extend([format!("_{}_", inline(&section.subtitle)), String::new()]);
    }
    for block in &section.blocks {
        let out = block_lines(block);
        if !out.is_empty() {
            lines.extend(out);
            lines.push(String::new());
        }
    }
}

/// A block's lines; none for a chart or table without rows.
fn block_lines(block: &Block) -> Vec<String> {
    match block {
        Block::Figures(figures) => figures_lines(figures),
        Block::Chart(chart) => chart_lines(chart),
        Block::Paragraph(Paragraph { text, muted }) => vec![if *muted { format!("_{}_", inline(text)) } else { inline(text) }],
        Block::Quote(quote) => quote_lines(quote),
        Block::Items(items) => items_lines(items),
        Block::Table(Table { columns, rows, numeric }) if !rows.is_empty() => table(columns, rows, numeric),
        Block::Table(_) => Vec::new(),
    }
}

fn figures_lines(figures: &Figures) -> Vec<String> {
    figures.items.iter().map(|f| format!("- **{}:** {}", inline(&f.label), inline(&f.value))).collect()
}

fn chart_lines(chart: &Chart) -> Vec<String> {
    if chart.items.is_empty() {
        return Vec::new();
    }
    let columns = [String::new(), "Value".to_string()];
    let rows: Vec<Vec<String>> = chart.items.iter().map(|bar| vec![bar.label.clone(), bar.display.clone()]).collect();
    let mut lines = vec![format!("**{}**", inline(&chart.title)), String::new()];
    lines.extend(table(&columns, &rows, &[false, true]));
    lines
}

/// Prompt text keeps its own Markdown out of the report: it is shown as typed, in a fence.
fn quote_lines(quote: &Quote) -> Vec<String> {
    let head = match quote.voice {
        Voice::You if quote.caption.is_empty() => "You typed".to_string(),
        Voice::You => format!("You typed ({})", quote.caption),
        Voice::System if quote.caption.is_empty() => "System text".to_string(),
        Voice::System => quote.caption.clone(),
    };
    vec![format!("{head}:"), String::new(), fence(&quote.text)]
}

fn items_lines(items: &Items) -> Vec<String> {
    items
        .items
        .iter()
        .map(|item| if items.mono && !item.contains('`') { format!("- `{item}`") } else { format!("- {}", inline(item)) })
        .collect()
}

fn table(columns: &[String], rows: &[Vec<String>], numeric: &[bool]) -> Vec<String> {
    let row = |cells: &[String]| format!("| {} |", cells.iter().map(|c| cell(c)).collect::<Vec<_>>().join(" | "));
    let align: Vec<&str> = (0..columns.len()).map(|i| if numeric.get(i).copied().unwrap_or(false) { "---:" } else { "---" }).collect();
    let mut lines = vec![row(columns), format!("| {} |", align.join(" | "))];
    lines.extend(rows.iter().map(|cells| row(cells)));
    lines
}

/// Text placed in a sentence: `\ ` `` ` `` `* _ [ ] < > ! #` are backslash-escaped, so nothing can
/// turn into a link, an image or a heading.
pub fn inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '!' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A table cell: whitespace runs (newlines included) become one space, `|` is escaped, trimmed;
/// an empty cell is one space.
pub fn cell(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ").replace('|', "\\|");
    if collapsed.is_empty() { " ".to_string() } else { collapsed }
}

/// `text` in a fence of backticks one longer than its longest backtick run (at least three),
/// tagged `text`.
pub fn fence(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat((longest + 1).max(3));
    format!("{ticks}text\n{text}\n{ticks}")
}
