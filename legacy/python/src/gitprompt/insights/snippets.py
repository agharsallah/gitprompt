"""Search result snippets: the part of a typed text around the first match, with every match of every
searched word marked. Matching is the same as the `q` filter (store._words): each word as a
substring, ignoring case. A pure function, so the dashboard renders `<mark>` from data, never HTML.
"""

import re
from typing import TypedDict

WIDTH = 180  # characters of context shown
ELLIPSIS = "…"


class Segment(TypedDict):
    text: str
    hit: bool


def _spans(text: str, words: list[str]) -> list[tuple[int, int]]:
    """Where the words occur, overlapping matches merged, in order."""
    spans = sorted((m.start(), m.end()) for w in words for m in re.finditer(re.escape(w), text, re.IGNORECASE))
    merged: list[tuple[int, int]] = []
    for start, end in spans:
        if merged and start <= merged[-1][1]:
            merged[-1] = (merged[-1][0], max(end, merged[-1][1]))
        else:
            merged.append((start, end))
    return merged


def _flat(text: str) -> str:
    return re.sub(r"\s+", " ", text)


def highlight(text: str | None, q: str | None, width: int = WIDTH) -> list[Segment]:
    """[{text, hit}] for a window of `text` starting a little before the first match. Without a match
    (or a query), the start of the text."""
    text = text or ""
    words = list(dict.fromkeys(w.lower() for w in (q or "").split()))
    spans = _spans(text, words)
    start = 0
    if spans and spans[0][0] > width // 3:
        start = spans[0][0] - width // 3
        start = next((i + 1 for i in range(start, spans[0][0]) if text[i].isspace()), start)  # a word boundary
    end = min(len(text), start + width)
    if end < len(text) and spans and spans[0][1] > end:
        end = spans[0][1]  # never cut the first match
    if end < len(text):
        end = next((i for i in range(end, max(start, end - 30), -1) if text[i].isspace()), end)
    out: list[Segment] = []
    position = start
    for s, e in spans:
        if e <= start or s >= end:
            continue
        s, e = max(s, start), min(e, end)
        if s > position:
            out.append({"text": _flat(text[position:s]), "hit": False})
        out.append({"text": _flat(text[s:e]), "hit": True})
        position = e
    if position < end:
        out.append({"text": _flat(text[position:end]), "hit": False})
    if out and start > 0:
        out.insert(0, {"text": ELLIPSIS, "hit": False})
    if out and end < len(text):
        out.append({"text": ELLIPSIS, "hit": False})
    return out
