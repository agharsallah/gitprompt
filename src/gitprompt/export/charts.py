"""Charts as inline SVG (ADR-0018): drawn from a report Chart with no script and no external file.
Colours are CSS variables, so a chart follows the light or dark theme of the page around it; a
standalone chart (the `svg` format, and the PNG the dashboard draws from it) carries its own.

    svg(chart)              for a page that defines the variables (html.py STYLE)
    svg(chart, alone=True)  a file of its own: title, background and both themes inside
"""

from html import escape

from gitprompt.export.formats import Export, register
from gitprompt.export.report import Chart

BAR_W = 480  # bar charts: drawn narrow, so labels stay readable once scaled to a phone
ROW_H = 34
COL_W, COL_H = 640, 200  # column charts
FONT = "system-ui, -apple-system, 'Segoe UI', Roboto, sans-serif"
LABEL_CHARS = 44

# The dashboard's chart slots and neutrals (web/src/lib/design/tokens.css), light then dark.
PALETTE = {
    "light": {"bg": "#fcfdfd", "text": "#151c1b", "muted": "#5f6c69", "grid": "#e6ebea", "axis": "#c9d2d0", "chart": "#1a9384"},
    "dark": {"bg": "#151c1b", "text": "#e6ecea", "muted": "#8e9b98", "grid": "#212a28", "axis": "#3b4745", "chart": "#2fa898"},
}


def theme_vars(theme: str) -> str:
    return ";".join(f"--gp-{k}:{v}" for k, v in PALETTE[theme].items())


CHART_STYLE = (
    ".gp-bar{fill:var(--gp-chart)}.gp-track{fill:var(--gp-grid)}.gp-axis{stroke:var(--gp-axis);stroke-width:1}"
    ".gp-text{fill:var(--gp-text)}.gp-muted{fill:var(--gp-muted)}.gp-num{font-variant-numeric:tabular-nums}"
)


def _clip(text: str, chars: int = LABEL_CHARS) -> str:
    return text if len(text) <= chars else text[: chars - 1].rstrip() + "…"


def _bars(chart: Chart, top: int) -> tuple[list[str], int, int]:
    peak = max((b.value for b in chart.items), default=0) or 1
    out = []
    for i, b in enumerate(chart.items):
        y = top + i * ROW_H
        w = max(2.0, BAR_W * b.value / peak)
        out.append(
            f"<g><title>{escape(b.label)}: {escape(b.display)}</title>"
            f'<text class="gp-text" x="0" y="{y + 13}" font-size="13">{escape(_clip(b.label))}</text>'
            f'<text class="gp-muted gp-num" x="{BAR_W}" y="{y + 13}" font-size="13" text-anchor="end">{escape(b.display)}</text>'
            f'<rect class="gp-track" x="0" y="{y + 19}" width="{BAR_W}" height="8" rx="4"/>'
            f'<rect class="gp-bar" x="0" y="{y + 19}" width="{w:.1f}" height="8" rx="4"/></g>'
        )
    return out, BAR_W, top + len(chart.items) * ROW_H


def _columns(chart: Chart, top: int) -> tuple[list[str], int, int]:
    items = chart.items
    peak = max((b.value for b in items), default=0) or 1
    left, plot_h = 0, COL_H - 40
    base = top + 20 + plot_h
    step = COL_W / max(len(items), 1)
    width = max(1.0, step * 0.72)
    out = [
        f'<text class="gp-muted gp-num" x="0" y="{top + 12}" font-size="14">{escape(_peak(items, peak))}</text>',
        f'<line class="gp-axis" x1="0" x2="{COL_W}" y1="{base + 0.5}" y2="{base + 0.5}"/>',
    ]
    for i, b in enumerate(items):
        h = plot_h * b.value / peak
        x = left + i * step + (step - width) / 2
        out.append(
            f'<rect class="gp-bar" x="{x:.1f}" y="{base - h:.1f}" width="{width:.1f}" height="{max(h, 0):.1f}" rx="2">'
            f"<title>{escape(b.label)}: {escape(b.display)}</title></rect>"
        )
    ticks = sorted({0, len(items) // 2, len(items) - 1}) if len(items) > 4 else range(len(items))
    for i in ticks:
        anchor = "start" if i == 0 and len(items) > 1 else "end" if i == len(items) - 1 and len(items) > 1 else "middle"
        x = 0 if anchor == "start" else COL_W if anchor == "end" else left + i * step + step / 2
        out.append(
            f'<text class="gp-muted" x="{x:.1f}" y="{base + 22}" font-size="14" text-anchor="{anchor}">{escape(items[i].label)}</text>'
        )
    return out, COL_W, base + 30


def _peak(items, peak: float) -> str:
    return next((b.display for b in items if b.value == peak), str(peak))


def svg(chart: Chart, alone: bool = False) -> str:
    """The chart as one <svg> element (a whole file when `alone`)."""
    if not chart.items:
        return ""
    top = 36 if alone else 0
    marks, width, height = (_bars if chart.kind == "bars" else _columns)(chart, top)
    pad = 16 if alone else 0
    desc = "; ".join(f"{b.label}: {b.display}" for b in chart.items)
    head = ""
    if alone:
        style = f":root{{{theme_vars('light')}}}@media (prefers-color-scheme: dark){{:root{{{theme_vars('dark')}}}}}{CHART_STYLE}"
        head = (
            f"<style>{style}</style>"
            f'<rect x="{-pad}" y="{-pad}" width="{width + 2 * pad}" height="{height + 2 * pad}" fill="var(--gp-bg)"/>'
            f'<text class="gp-text" x="0" y="14" font-size="16" font-weight="600">{escape(chart.title)}</text>'
        )
    # A file of its own needs the namespace and a size; inline in HTML, neither (no URL in the page).
    size = f' xmlns="http://www.w3.org/2000/svg" width="{width + 2 * pad}" height="{height + 2 * pad}"' if alone else ""
    return (
        f'<svg viewBox="{-pad} {-pad} {width + 2 * pad} {height + 2 * pad}"{size}'
        f' role="img" font-family="{FONT}" aria-label="{escape(chart.title)}"><desc>{escape(desc)}</desc>{head}{"".join(marks)}</svg>'
    )


@register("svg", "Chart (SVG)", "image/svg+xml", ".svg", single_chart=True)
def render_svg(export: Export) -> str:
    chart = export.report.chart(export.chart or "") or next(export.report.charts(), None)
    if chart is None or not chart.items:
        raise LookupError("this export has no chart with data")
    return '<?xml version="1.0" encoding="UTF-8"?>\n' + svg(chart, alone=True) + "\n"
