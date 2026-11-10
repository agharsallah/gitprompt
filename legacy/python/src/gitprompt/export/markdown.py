"""A report as Markdown (GitHub flavoured): charts become tables of their values."""

import re

from gitprompt.export.formats import Export, register
from gitprompt.export.report import Chart, Figures, Items, Paragraph, Quote, Report, Table


def _cell(text: str) -> str:
    return re.sub(r"\s+", " ", text).replace("|", "\\|").strip() or " "


def _inline(text: str) -> str:
    """Text placed in a sentence: Markdown that could turn into a link or an image is escaped."""
    return re.sub(r"([\\`*_\[\]<>!#])", r"\\\1", text)


def _table(columns: list[str], rows: list[list[str]], numeric: list[bool]) -> list[str]:
    align = [("---:" if i < len(numeric) and numeric[i] else "---") for i in range(len(columns))]
    out = ["| " + " | ".join(_cell(c) for c in columns) + " |", "| " + " | ".join(align) + " |"]
    return out + ["| " + " | ".join(_cell(v) for v in row) + " |" for row in rows]


def _fence(text: str) -> str:
    ticks = "`" * max(3, max((len(m) for m in re.findall(r"`+", text)), default=0) + 1)
    return f"{ticks}text\n{text}\n{ticks}"


def _block(block) -> list[str]:
    if isinstance(block, Figures):
        return [f"- **{_inline(f.label)}:** {_inline(f.value)}" for f in block.items]
    if isinstance(block, Chart):
        if not block.items:
            return []
        return [f"**{_inline(block.title)}**", "", *_table(["", "Value"], [[b.label, b.display] for b in block.items], [False, True])]
    if isinstance(block, Paragraph):
        return [f"_{_inline(block.text)}_" if block.muted else _inline(block.text)]
    if isinstance(block, Quote):
        # Prompt text keeps its own Markdown out of the report: it is shown as typed, in a fence.
        label = "You typed" if block.voice == "you" else block.caption or "System text"
        extra = f" ({block.caption})" if block.voice == "you" and block.caption else ""
        return [f"{label}{extra}:", "", _fence(block.text)]
    if isinstance(block, Items):
        return [f"- `{i}`" if block.mono and "`" not in i else f"- {_inline(i)}" for i in block.items]
    if isinstance(block, Table):
        return _table(block.columns, block.rows, block.numeric) if block.rows else []
    raise TypeError(f"unknown block {type(block).__name__}")


def markdown(report: Report) -> str:
    lines = [f"# {_inline(report.title)}", ""]
    if report.subtitle:
        lines += [_inline(report.subtitle), ""]
    for section in report.sections:
        lines += ["#" * section.level + " " + _inline(section.title), ""]
        if section.subtitle:
            lines += [f"_{_inline(section.subtitle)}_", ""]
        for block in section.blocks:
            if out := _block(block):
                lines += [*out, ""]
    lines += ["---", "", f"_Exported from GitPrompt on {report.generated_at}._", ""]
    lines += [f"- {_inline(n)}" for n in report.notes]
    return "\n".join(lines) + "\n"


@register("md", "Markdown", "text/markdown; charset=utf-8", ".md")
def render_markdown(export: Export) -> str:
    return markdown(export.report)
