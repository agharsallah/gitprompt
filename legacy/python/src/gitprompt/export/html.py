"""A report as one self-contained HTML file (ADR-0018): inline CSS, inline SVG charts, no script, no
font, no image, no link. A Content-Security-Policy in the file itself forbids every request, so even a
value that slipped through can't make the page load anything. Follows the reader's light or dark
setting and prints on white."""

from html import escape
from itertools import groupby

from gitprompt.export.charts import CHART_STYLE, PALETTE, svg
from gitprompt.export.formats import Export, register
from gitprompt.export.report import Chart, Figures, Items, Paragraph, Quote, Report, Table

CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'"


def _vars(theme: str) -> str:
    p = PALETTE[theme]
    surface, line, accent = ("#f2f5f4", "#dae1df", "#0a7468") if theme == "light" else ("#1b2322", "#2a3432", "#5fc3b3")
    return f"--bg:{p['bg']};--surface:{surface};--text:{p['text']};--muted:{p['muted']};--line:{line};--accent:{accent};" + ";".join(
        f"--gp-{k}:{v}" for k, v in p.items()
    )


STYLE = f"""
:root{{color-scheme:light dark;{_vars("light")}}}
@media (prefers-color-scheme: dark){{:root{{{_vars("dark")}}}}}
*{{box-sizing:border-box}}
html{{-webkit-text-size-adjust:100%}}
body{{margin:0;background:var(--bg);color:var(--text);font:15px/1.55 system-ui,-apple-system,"Segoe UI",Roboto,sans-serif}}
main{{max-width:60rem;margin:0 auto;padding:32px 16px 48px}}
h1{{font-size:1.75rem;line-height:1.2;margin:0 0 6px;overflow-wrap:anywhere}}
h2,h3{{overflow-wrap:anywhere}}
h2{{font-size:1.2rem;margin:40px 0 12px;padding-top:16px;border-top:1px solid var(--line)}}
h3{{font-size:1rem;margin:28px 0 4px}}
.kicker{{color:var(--accent);font-weight:600;font-size:.85rem;margin:0 0 8px}}
.sub,.muted{{color:var(--muted)}}
.sub{{margin:0 0 4px;overflow-wrap:anywhere}}
.section-sub{{color:var(--muted);font-size:.875rem;margin:0 0 12px;overflow-wrap:anywhere}}
dl.figures{{display:grid;grid-template-columns:repeat(auto-fill,minmax(9.5rem,1fr));gap:10px;margin:16px 0}}
dl.figures div{{background:var(--surface);border-radius:8px;padding:10px 12px}}
dt{{color:var(--muted);font-size:.8rem}}
dd{{margin:2px 0 0;font-weight:600;font-variant-numeric:tabular-nums;overflow-wrap:anywhere}}
.charts{{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,22rem),1fr));gap:16px;margin:16px 0}}
figure{{margin:0;background:var(--surface);border-radius:8px;padding:14px 14px 10px;break-inside:avoid}}
figcaption{{font-weight:600;font-size:.9rem;margin-bottom:10px}}
figure svg{{display:block;width:100%;height:auto;overflow:visible}}
figure svg.gp-columns{{max-height:15rem}}
details{{margin-top:8px;font-size:.85rem}}
summary{{color:var(--muted);cursor:pointer}}
blockquote{{margin:10px 0;padding:8px 12px;border-left:3px solid var(--accent);background:var(--surface);border-radius:0 6px 6px 0}}
blockquote.system{{border-left:3px dashed var(--muted);color:var(--muted)}}
blockquote .who{{display:block;font-size:.78rem;color:var(--muted);margin-bottom:4px}}
blockquote .body{{white-space:pre-wrap;overflow-wrap:anywhere;margin:0}}
ul{{padding-left:20px;margin:8px 0}}
ul.mono{{font:13px/1.5 ui-monospace,"SF Mono",Menlo,Consolas,monospace;overflow-wrap:anywhere}}
.table{{overflow-x:auto;margin:12px 0;border:1px solid var(--line);border-radius:8px}}
table{{border-collapse:collapse;width:100%;font-size:.875rem}}
th,td{{padding:6px 10px;text-align:left;vertical-align:top;border-bottom:1px solid var(--line)}}
th{{color:var(--muted);font-weight:600;white-space:nowrap}}
tr:last-child td{{border-bottom:0}}
td:first-child{{white-space:nowrap}}
td.num,th.num{{text-align:right;font-variant-numeric:tabular-nums;white-space:nowrap}}
td.text{{min-width:16rem;white-space:pre-wrap;overflow-wrap:anywhere}}
footer{{margin-top:48px;padding-top:16px;border-top:1px solid var(--line);color:var(--muted);font-size:.85rem}}
footer ul{{padding-left:18px}}
{CHART_STYLE}
@media print{{
:root{{{_vars("light")}}}
body{{font-size:11pt;background:#fff}}
main{{max-width:none;padding:0}}
h2,h3{{break-after:avoid}}
details{{display:none}}
.table{{overflow:visible}}
}}
"""


def _figures(block: Figures) -> str:
    cells = "".join(f"<div><dt>{escape(f.label)}</dt><dd>{escape(f.value)}</dd></div>" for f in block.items)
    return f'<dl class="figures">{cells}</dl>'


def _chart(chart: Chart) -> str:
    if not chart.items:
        return ""
    rows = "".join(f'<tr><td>{escape(b.label)}</td><td class="num">{escape(b.display)}</td></tr>' for b in chart.items)
    caption = f'<p class="muted">{escape(chart.caption)}</p>' if chart.caption else ""
    return (
        f"<figure><figcaption>{escape(chart.title)}</figcaption>{svg(chart)}{caption}"
        f'<details><summary>Numbers</summary><div class="table"><table>{rows}</table></div></details></figure>'
    )


def _table(block: Table) -> str:
    if not block.rows:
        return ""

    def cls(i: int, text: bool = False) -> str:
        if i < len(block.numeric) and block.numeric[i]:
            return ' class="num"'
        return ' class="text"' if text and block.columns[i] == "Prompt" else ""

    head = "".join(f"<th{cls(i)}>{escape(c)}</th>" for i, c in enumerate(block.columns))
    body = "".join("<tr>" + "".join(f"<td{cls(i, True)}>{escape(v)}</td>" for i, v in enumerate(row)) + "</tr>" for row in block.rows)
    return f'<div class="table"><table><thead><tr>{head}</tr></thead><tbody>{body}</tbody></table></div>'


def _block(block) -> str:
    if isinstance(block, Figures):
        return _figures(block)
    if isinstance(block, Paragraph):
        return f'<p class="muted">{escape(block.text)}</p>' if block.muted else f"<p>{escape(block.text)}</p>"
    if isinstance(block, Quote):
        who = "You typed" if block.voice == "you" else block.caption or "System text"
        extra = f" · {block.caption}" if block.voice == "you" and block.caption else ""
        cls = "" if block.voice == "you" else ' class="system"'
        return f'<blockquote{cls}><span class="who">{escape(who + extra)}</span><p class="body">{escape(block.text)}</p></blockquote>'
    if isinstance(block, Items):
        items = "".join(f"<li>{escape(i)}</li>" for i in block.items)
        return f'<ul class="mono">{items}</ul>' if block.mono else f"<ul>{items}</ul>"
    if isinstance(block, Table):
        return _table(block)
    raise TypeError(f"unknown block {type(block).__name__}")


def _blocks(blocks: list) -> str:
    out = []
    for is_chart, run in groupby(blocks, key=lambda b: isinstance(b, Chart)):
        if is_chart:
            charts = "".join(_chart(c) for c in run)
            out.append(f'<div class="charts">{charts}</div>' if charts else "")
        else:
            out += [_block(b) for b in run]
    return "".join(out)


def html(report: Report) -> str:
    body = [f'<p class="kicker">GitPrompt {escape(report.kind)} report</p><h1>{escape(report.title)}</h1>']
    if report.subtitle:
        body.append(f'<p class="sub">{escape(report.subtitle)}</p>')
    for s in report.sections:
        tag = f"h{s.level}"
        sub = f'<p class="section-sub">{escape(s.subtitle)}</p>' if s.subtitle else ""
        body.append(f"<section><{tag}>{escape(s.title)}</{tag}>{sub}{_blocks(s.blocks)}</section>")
    notes = "".join(f"<li>{escape(n)}</li>" for n in report.notes)
    body.append(f"<footer><p>Exported from GitPrompt on {escape(report.generated_at)}.</p><ul>{notes}</ul></footer>")
    return (
        "<!doctype html>\n"
        '<html lang="en"><head><meta charset="utf-8">'
        f'<meta http-equiv="Content-Security-Policy" content="{CSP}">'
        '<meta name="viewport" content="width=device-width, initial-scale=1">'
        '<meta name="color-scheme" content="light dark">'
        '<meta name="referrer" content="no-referrer">'
        f"<title>{escape(report.title)} · GitPrompt</title><style>{STYLE}</style></head>"
        f"<body><main>{''.join(body)}</main></body></html>\n"
    )


@register("html", "HTML", "text/html; charset=utf-8", ".html")
def render_html(export: Export) -> str:
    return html(export.report)
