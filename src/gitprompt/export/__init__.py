"""Exports (plan E5, ADR-0018): a story, the Overview or a prompt selection as a file you send yourself.

    data ──redact.redact──► clean data ──build.BUILDERS[kind]──► Report + rows ──formats.FORMATS[f]──► file

One redaction pass for every format; builders are pure; a format only sees the Report and rows. To
add a format, register a renderer (formats.register) in a module imported below; to add a kind of
export, register a builder (build.builder) and collect its data in api/routes/export.py.
"""

from typing import Any

from gitprompt.export import charts, html, markdown, tabular  # noqa: F401  (each registers its formats)
from gitprompt.export.build import BUILDERS
from gitprompt.export.formats import FORMATS, Export, Format
from gitprompt.export.redact import Options, redact
from gitprompt.export.report import Report

__all__ = ["BUILDERS", "FORMATS", "Export", "Format", "Options", "Report", "prepare", "render"]


def prepare(kind: str, data: dict[str, Any], options: Options, home: str | None = None, roots: set[str] | None = None) -> Export:
    """Redact `data`, then build its report and rows."""
    report, rows = BUILDERS[kind](redact(data, options, home, roots), options)
    return Export(report, rows, options)


def render(export: Export, fmt: str, chart: str | None = None) -> str:
    return FORMATS[fmt].render(Export(export.report, export.rows, export.options, chart))
