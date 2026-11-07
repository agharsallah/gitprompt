"""Pure helpers over JSON payloads (dicts, lists, scalars). No I/O, no knowledge of any tool."""

import re
from collections.abc import Callable
from typing import Any

Mapping = dict[str, list[str]]  # field -> candidate paths, first non-empty value wins

NO_VALUE = (None, "", [])  # a mapped field with one of these falls through to the next candidate
EMPTY = (*NO_VALUE, {})


def pluck(data: Any, path: str) -> Any:
    """Follow a dotted path such as `message.content` or `workspace_roots[0]`. A key that itself
    contains dots (OpenTelemetry's `prompt.id`) is matched as-is first."""
    if isinstance(data, dict) and path in data:
        return data[path]
    for key in re.findall(r"[^.\[\]]+", path):
        if isinstance(data, dict):
            data = data.get(key)
        elif isinstance(data, list) and key.isdigit() and int(key) < len(data):
            data = data[int(key)]
        else:
            return None
    return data


def first(data: Any, paths: list[str]) -> Any:
    """The first non-empty value among candidate paths, or None."""
    return next((value for path in paths if (value := pluck(data, path)) not in NO_VALUE), None)


def extract(payload: dict[str, Any], mapping: Mapping) -> dict[str, Any]:
    """Apply a spec's field mapping to a payload. Fields with no value are left out."""
    found = {field: first(payload, paths) for field, paths in mapping.items()}
    return {k: v for k, v in found.items() if v is not None}


def map_strings(value: Any, fn: Callable[[str], str]) -> Any:
    """Apply `fn` to every string in a JSON value, keeping its structure."""
    if isinstance(value, str):
        return fn(value)
    if isinstance(value, dict):
        return {k: map_strings(v, fn) for k, v in value.items()}
    if isinstance(value, list):
        return [map_strings(v, fn) for v in value]
    return value


def clip(value: Any, limit: int = 10_000) -> Any:
    """Shorten long strings (file contents in tool results) before storing an event."""

    def shorten(text: str) -> str:
        return f"{text[:limit]}… [{len(text) - limit} more characters not stored]" if len(text) > limit else text

    return map_strings(value, shorten)


def flatten(value: Any, path: str = "") -> list[tuple[str, Any]]:
    """(path, leaf value) pairs; list items share one path ending in `[]`."""
    if isinstance(value, dict) and value:
        return [x for k, v in value.items() for x in flatten(v, f"{path}.{k}" if path else k)]
    if isinstance(value, list) and value:
        return [x for v in value for x in flatten(v, f"{path}[]")]
    return [(path, value)]


def compact(data: dict[str, Any]) -> dict[str, Any]:
    """Drop empty and zero values, so derived summaries only show what happened."""
    return {k: v for k, v in data.items() if v not in (*EMPTY, 0)}
