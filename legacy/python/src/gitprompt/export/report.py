"""A report as plain data: what every document format renders (ADR-0018). Builders (build.py) turn a
redacted story or statistics into a Report and its rows; formats (markdown.py, html.py, tabular.py,
charts.py) turn a Report into a file. Neither side knows the other."""

from collections.abc import Iterator
from dataclasses import dataclass, field
from typing import Literal


@dataclass(frozen=True)
class Figure:
    label: str
    value: str


@dataclass(frozen=True)
class Figures:
    """A row of headline numbers."""

    items: list[Figure]


@dataclass(frozen=True)
class Bar:
    label: str
    value: float
    display: str  # the value as read, e.g. "$1.20" or "12"


@dataclass(frozen=True)
class Chart:
    """bars: one horizontal bar per item, longest first. columns: one column per item, in order
    (a time series); only a few labels are drawn."""

    id: str  # stable within a report: a single chart is exported by it
    title: str
    kind: Literal["bars", "columns"]
    items: list[Bar]
    caption: str = ""


@dataclass(frozen=True)
class Paragraph:
    text: str
    muted: bool = False


@dataclass(frozen=True)
class Quote:
    """Text from the data, shown as quoted: what you typed (`voice` you) or system text."""

    text: str
    voice: Literal["you", "system"] = "you"
    caption: str = ""


@dataclass(frozen=True)
class Items:
    items: list[str]
    mono: bool = False  # file paths, commit hashes


@dataclass(frozen=True)
class Table:
    columns: list[str]
    rows: list[list[str]]
    numeric: list[bool] = field(default_factory=list)  # per column: right-aligned


Block = Figures | Chart | Paragraph | Quote | Items | Table


@dataclass(frozen=True)
class Section:
    title: str
    blocks: list[Block]
    subtitle: str = ""
    level: Literal[2, 3] = 2


@dataclass(frozen=True)
class Report:
    kind: str  # session, project, overview, prompts
    title: str
    subtitle: str
    generated_at: str  # local time, as read
    sections: list[Section]
    notes: list[str]  # what this export includes and leaves out

    def charts(self) -> Iterator[Chart]:
        for s in self.sections:
            yield from (b for b in s.blocks if isinstance(b, Chart))

    def chart(self, id: str) -> Chart | None:
        return next((c for c in self.charts() if c.id == id), None)
