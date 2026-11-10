"""The format registry (ADR-0018). A format is one function from an Export to the file's text; to add
one, write a module with a `@register(...)` function and import it in export/__init__.py."""

from collections.abc import Callable
from dataclasses import dataclass, field
from typing import Any

from gitprompt.export.redact import Options
from gitprompt.export.report import Report


@dataclass(frozen=True)
class Export:
    """What a format renders: the report, its rows (already redacted, like the report) and the
    options that made them. `chart`: the one chart a chart format draws."""

    report: Report
    rows: list[dict[str, Any]]
    options: Options = field(default_factory=Options)
    chart: str | None = None


@dataclass(frozen=True)
class Format:
    name: str  # the `format` query value
    label: str
    media_type: str
    extension: str
    render: Callable[[Export], str]
    single_chart: bool = False  # renders `Export.chart` only


FORMATS: dict[str, Format] = {}


def register(name: str, label: str, media_type: str, extension: str, single_chart: bool = False):
    def add(render: Callable[[Export], str]) -> Callable[[Export], str]:
        FORMATS[name] = Format(name, label, media_type, extension, render, single_chart)
        return render

    return add
