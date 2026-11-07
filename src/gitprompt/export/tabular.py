"""The rows under a report, as CSV or JSON: one row per chapter, prompt or statistics bucket."""

import csv
import io
import json
from typing import Any

from gitprompt.export.formats import Export, register

# A cell starting with one of these runs as a formula in a spreadsheet: quoted with an apostrophe.
FORMULA = ("=", "+", "-", "@", "\t", "\r")


def _columns(rows: list[dict[str, Any]]) -> list[str]:
    return list(dict.fromkeys(k for row in rows for k in row))


def _cell(value: Any) -> Any:
    if isinstance(value, dict | list):
        value = json.dumps(value, ensure_ascii=False)
    if isinstance(value, str) and value.startswith(FORMULA):
        return "'" + value
    return "" if value is None else value


@register("csv", "CSV", "text/csv; charset=utf-8", ".csv")
def render_csv(export: Export) -> str:
    out = io.StringIO()
    columns = _columns(export.rows)
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(columns)
    for row in export.rows:
        writer.writerow([_cell(row.get(c)) for c in columns])
    return out.getvalue()


@register("json", "JSON", "application/json", ".json")
def render_json(export: Export) -> str:
    r = export.report
    document = {
        "kind": r.kind,
        "title": r.title,
        "subtitle": r.subtitle,
        "generated_at": r.generated_at,
        "options": export.options.as_dict(),
        "notes": r.notes,
        "rows": export.rows,
    }
    return json.dumps(document, ensure_ascii=False, indent=2) + "\n"
