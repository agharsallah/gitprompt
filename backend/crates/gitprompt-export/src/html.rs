//! A report as one self-contained HTML file (ADR-0018 §5): inline CSS, inline SVG charts, no
//! script, font, image or link. A Content-Security-Policy in the file itself forbids every request,
//! so even a value that slipped through can't make the page load anything. Follows the reader's
//! light or dark setting (`prefers-color-scheme`) and prints on white (`@media print` forces the
//! light palette and hides the "Numbers" disclosures). Port of `export/html.py`.

use std::fmt::Write;

use crate::charts::{CHART_STYLE, DARK, LIGHT, Palette, svg};
use crate::escape::html as esc;
use crate::report::{Block, Chart, Figures, Items, Level, Paragraph, Quote, Report, Section, Table, Voice};

/// The page's own policy; the API sends the same value as a header.
pub const CSP: &str = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'";

/// The page's neutrals besides the chart palette: surface, line and accent.
struct PageColors {
    surface: &'static str,
    line: &'static str,
    accent: &'static str,
}

const LIGHT_PAGE: PageColors = PageColors { surface: "#f2f5f4", line: "#dae1df", accent: "#0a7468" };
const DARK_PAGE: PageColors = PageColors { surface: "#1b2322", line: "#2a3432", accent: "#5fc3b3" };

/// One theme as CSS custom properties: the page's (`--bg`, `--text`...) then the charts' (`--gp-*`).
fn vars(p: &Palette, page: &PageColors) -> String {
    format!(
        "--bg:{};--surface:{};--text:{};--muted:{};--line:{};--accent:{};{}",
        p.bg,
        page.surface,
        p.text,
        p.muted,
        page.line,
        page.accent,
        crate::charts::theme_vars(p)
    )
}

/// The rules between the palettes and the print block (no colour of their own: all variables).
const RULES: &str = r#"*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--text);font:15px/1.55 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}
main{max-width:60rem;margin:0 auto;padding:32px 16px 48px}
h1{font-size:1.75rem;line-height:1.2;margin:0 0 6px;overflow-wrap:anywhere}
h2,h3{overflow-wrap:anywhere}
h2{font-size:1.2rem;margin:40px 0 12px;padding-top:16px;border-top:1px solid var(--line)}
h3{font-size:1rem;margin:28px 0 4px}
.kicker{color:var(--accent);font-weight:600;font-size:.85rem;margin:0 0 8px}
.sub,.muted{color:var(--muted)}
.sub{margin:0 0 4px;overflow-wrap:anywhere}
.section-sub{color:var(--muted);font-size:.875rem;margin:0 0 12px;overflow-wrap:anywhere}
dl.figures{display:grid;grid-template-columns:repeat(auto-fill,minmax(9.5rem,1fr));gap:10px;margin:16px 0}
dl.figures div{background:var(--surface);border-radius:8px;padding:10px 12px}
dt{color:var(--muted);font-size:.8rem}
dd{margin:2px 0 0;font-weight:600;font-variant-numeric:tabular-nums;overflow-wrap:anywhere}
.charts{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,22rem),1fr));gap:16px;margin:16px 0}
figure{margin:0;background:var(--surface);border-radius:8px;padding:14px 14px 10px;break-inside:avoid}
figcaption{font-weight:600;font-size:.9rem;margin-bottom:10px}
figure svg{display:block;width:100%;height:auto;overflow:visible}
figure svg.gp-columns{max-height:15rem}
details{margin-top:8px;font-size:.85rem}
summary{color:var(--muted);cursor:pointer}
blockquote{margin:10px 0;padding:8px 12px;border-left:3px solid var(--accent);background:var(--surface);border-radius:0 6px 6px 0}
blockquote.system{border-left:3px dashed var(--muted);color:var(--muted)}
blockquote .who{display:block;font-size:.78rem;color:var(--muted);margin-bottom:4px}
blockquote .body{white-space:pre-wrap;overflow-wrap:anywhere;margin:0}
ul{padding-left:20px;margin:8px 0}
ul.mono{font:13px/1.5 ui-monospace,"SF Mono",Menlo,Consolas,monospace;overflow-wrap:anywhere}
.table{overflow-x:auto;margin:12px 0;border:1px solid var(--line);border-radius:8px}
table{border-collapse:collapse;width:100%;font-size:.875rem}
th,td{padding:6px 10px;text-align:left;vertical-align:top;border-bottom:1px solid var(--line)}
th{color:var(--muted);font-weight:600;white-space:nowrap}
tr:last-child td{border-bottom:0}
td:first-child{white-space:nowrap}
td.num,th.num{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}
td.text{min-width:16rem;white-space:pre-wrap;overflow-wrap:anywhere}
footer{margin-top:48px;padding-top:16px;border-top:1px solid var(--line);color:var(--muted);font-size:.85rem}
footer ul{padding-left:18px}
"#;

/// The print rules after the light palette.
const PRINT: &str = "body{font-size:11pt;background:#fff}
main{max-width:none;padding:0}
h2,h3{break-after:avoid}
details{display:none}
.table{overflow:visible}
}
";

/// The page's stylesheet: both palettes as CSS variables (`--bg`, `--text`... and the charts'
/// `--gp-*`), `color-scheme: light dark`, the dark palette under `prefers-color-scheme: dark`, the
/// light one again under `@media print`.
pub fn style() -> String {
    let light = vars(&LIGHT, &LIGHT_PAGE);
    let dark = vars(&DARK, &DARK_PAGE);
    format!(
        "\n:root{{color-scheme:light dark;{light}}}\n@media (prefers-color-scheme: dark){{:root{{{dark}}}}}\n{RULES}{CHART_STYLE}\n@media print{{\n:root{{{light}}}\n{PRINT}"
    )
}

/// The report as one HTML document (`<!doctype html>` ... `</html>\n`). Every text is escaped;
/// consecutive charts share one `div.charts` grid, each a `figure` with its inline SVG and a
/// "Numbers" table; a chart or table without rows is left out.
pub fn html(report: &Report) -> String {
    let mut body = format!(r#"<p class="kicker">GitPrompt {} report</p><h1>{}</h1>"#, esc(report.kind.as_str()), esc(&report.title));
    if !report.subtitle.is_empty() {
        let _ = write!(body, r#"<p class="sub">{}</p>"#, esc(&report.subtitle));
    }
    for section in &report.sections {
        push_section(&mut body, section);
    }
    let notes = joined(&report.notes, |out, note| {
        let _ = write!(out, "<li>{}</li>", esc(note));
    });
    let _ = write!(body, "<footer><p>Exported from GitPrompt on {}.</p><ul>{notes}</ul></footer>", esc(&report.generated_at));
    format!(
        concat!(
            "<!doctype html>\n",
            r#"<html lang="en"><head><meta charset="utf-8">"#,
            r#"<meta http-equiv="Content-Security-Policy" content="{csp}">"#,
            r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#,
            r#"<meta name="color-scheme" content="light dark">"#,
            r#"<meta name="referrer" content="no-referrer">"#,
            "<title>{title} · GitPrompt</title><style>{style}</style></head>",
            "<body><main>{body}</main></body></html>\n",
        ),
        csp = CSP,
        title = esc(&report.title),
        style = style(),
        body = body,
    )
}

// `write!` into a `String` cannot fail (`fmt::Write for String` always returns `Ok`), hence the
// `let _ =` below.

/// Each item written into one string by `write`, with no string per item.
fn joined<I: IntoIterator>(items: I, mut write: impl FnMut(&mut String, I::Item)) -> String {
    let mut out = String::new();
    for item in items {
        write(&mut out, item);
    }
    out
}

fn push_section(out: &mut String, section: &Section) {
    let tag = match section.level {
        Level::Two => "h2",
        Level::Three => "h3",
    };
    let _ = write!(out, "<section><{tag}>{}</{tag}>", esc(&section.title));
    if !section.subtitle.is_empty() {
        let _ = write!(out, r#"<p class="section-sub">{}</p>"#, esc(&section.subtitle));
    }
    push_blocks(out, &section.blocks);
    out.push_str("</section>");
}

/// Blocks in order, each run of consecutive charts in one grid (left out when none has items).
fn push_blocks(out: &mut String, blocks: &[Block]) {
    let mut rest = blocks;
    while let Some((first, tail)) = rest.split_first() {
        if matches!(first, Block::Chart(_)) {
            let run = rest.iter().take_while(|b| matches!(b, Block::Chart(_))).count();
            let (charts, after) = rest.split_at_checked(run).unwrap_or((rest, &[]));
            let charts: String = charts.iter().filter_map(|b| if let Block::Chart(c) = b { Some(chart(c)) } else { None }).collect();
            if !charts.is_empty() {
                let _ = write!(out, r#"<div class="charts">{charts}</div>"#);
            }
            rest = after;
        } else {
            out.push_str(&block(first));
            rest = tail;
        }
    }
}

fn block(block: &Block) -> String {
    match block {
        Block::Figures(figures) => figures_html(figures),
        Block::Chart(c) => chart(c),
        Block::Paragraph(Paragraph { text, muted: true }) => format!(r#"<p class="muted">{}</p>"#, esc(text)),
        Block::Paragraph(Paragraph { text, muted: false }) => format!("<p>{}</p>", esc(text)),
        Block::Quote(quote) => quote_html(quote),
        Block::Items(items) => items_html(items),
        Block::Table(t) => table(t),
    }
}

fn figures_html(figures: &Figures) -> String {
    let cells = joined(&figures.items, |out, f| {
        let _ = write!(out, "<div><dt>{}</dt><dd>{}</dd></div>", esc(&f.label), esc(&f.value));
    });
    format!(r#"<dl class="figures">{cells}</dl>"#)
}

fn chart(chart: &Chart) -> String {
    if chart.items.is_empty() {
        return String::new();
    }
    let rows = joined(&chart.items, |out, b| {
        let _ = write!(out, r#"<tr><td>{}</td><td class="num">{}</td></tr>"#, esc(&b.label), esc(&b.display));
    });
    let caption = if chart.caption.is_empty() { String::new() } else { format!(r#"<p class="muted">{}</p>"#, esc(&chart.caption)) };
    format!(
        r#"<figure><figcaption>{}</figcaption>{}{caption}<details><summary>Numbers</summary><div class="table"><table>{rows}</table></div></details></figure>"#,
        esc(&chart.title),
        svg(chart, false),
    )
}

fn quote_html(quote: &Quote) -> String {
    let (class, who) = match quote.voice {
        Voice::You if quote.caption.is_empty() => ("", "You typed".to_string()),
        Voice::You => ("", format!("You typed · {}", quote.caption)),
        Voice::System if quote.caption.is_empty() => (r#" class="system""#, "System text".to_string()),
        Voice::System => (r#" class="system""#, quote.caption.clone()),
    };
    format!(r#"<blockquote{class}><span class="who">{}</span><p class="body">{}</p></blockquote>"#, esc(&who), esc(&quote.text))
}

fn items_html(items: &Items) -> String {
    let list = joined(&items.items, |out, item| {
        let _ = write!(out, "<li>{}</li>", esc(item));
    });
    if items.mono { format!(r#"<ul class="mono">{list}</ul>"#) } else { format!("<ul>{list}</ul>") }
}

/// A table; numeric columns right-aligned, the `Prompt` column's cells wrapped as text.
fn table(t: &Table) -> String {
    if t.rows.is_empty() {
        return String::new();
    }
    let numeric = |i: usize| t.numeric.get(i).copied().unwrap_or(false);
    let class = |i: usize, cell: bool| {
        if numeric(i) {
            r#" class="num""#
        } else if cell && t.columns.get(i).is_some_and(|c| c == "Prompt") {
            r#" class="text""#
        } else {
            ""
        }
    };
    let head = joined(t.columns.iter().enumerate(), |out, (i, c)| {
        let _ = write!(out, "<th{}>{}</th>", class(i, false), esc(c));
    });
    let body = joined(&t.rows, |out, row| {
        out.push_str("<tr>");
        for (i, v) in row.iter().enumerate() {
            let _ = write!(out, "<td{}>{}</td>", class(i, true), esc(v));
        }
        out.push_str("</tr>");
    });
    format!(r#"<div class="table"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>"#)
}
