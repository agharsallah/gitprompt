"""Prompt lineage: the near-duplicate and refined versions of a typed prompt (a retry in the same
session, the same instruction reused in other sessions or projects), how the wording changed, and
whether the outcome got better (ADR-0016).

    normalize(text)          lower case, punctuation as spaces, spacing collapsed
    similarity(a, b)         0..1: the mean of character-trigram and word Jaccard; 0 when the
                             lengths differ more than LENGTH_RATIO allows
    word_diff(a, b)          [(op, text)]: equal / delete / insert runs, word by word
    change(before, after)    improved / worse / same, by outcome

Pure functions, standard library only: no embeddings, nothing leaves the machine.
"""

import re
from collections.abc import Sequence
from dataclasses import dataclass
from difflib import SequenceMatcher
from functools import cached_property
from typing import Literal

# Two typed texts are versions of one prompt from this similarity on. Chosen on the local data
# (ADR-0016): at 0.5 and above, pairs read as one instruction reworded or extended; below, short
# texts that share a verb but ask for different things ("commit the X" / "deploy the X").
THRESHOLD = 0.5
# Texts whose lengths differ more than this are not versions of each other: a short instruction
# and a long pasted brief share many trigrams by vocabulary alone.
LENGTH_RATIO = 0.5
# Texts are compared (and diffed) on their first characters only: enough to tell versions apart,
# and it bounds the cost of a pasted log or document.
COMPARED_CHARS = 4000

_PUNCTUATION = re.compile(r"[^\w\s]")
_TOKENS = re.compile(r"\s+|\w+|[^\w\s]")


def normalize(text: str | None) -> str:
    """Lower case, punctuation as spaces, spacing collapsed: what similarity compares."""
    return " ".join(_PUNCTUATION.sub(" ", (text or "")[:COMPARED_CHARS].lower()).split())


@dataclass(frozen=True)
class Shingles:
    """A text's normalized form, character trigrams and words, computed once per text."""

    text: str

    @cached_property
    def normal(self) -> str:
        return normalize(self.text)

    @cached_property
    def trigrams(self) -> frozenset[str]:
        t = self.normal
        return frozenset(t[i : i + 3] for i in range(max(1, len(t) - 2))) if t else frozenset()

    @cached_property
    def words(self) -> frozenset[str]:
        return frozenset(self.normal.split())


def _jaccard(a: frozenset[str], b: frozenset[str]) -> float:
    union = len(a | b)
    return len(a & b) / union if union else 0.0


def similarity(a: str | Shingles, b: str | Shingles) -> float:
    """0..1. Trigrams catch reworded spellings and word forms; words keep short texts honest."""
    a, b = (x if isinstance(x, Shingles) else Shingles(x) for x in (a, b))
    if not a.normal or not b.normal:
        return 0.0
    if a.normal == b.normal:
        return 1.0
    if min(len(a.normal), len(b.normal)) / max(len(a.normal), len(b.normal)) < LENGTH_RATIO:
        return 0.0
    return round((_jaccard(a.trigrams, b.trigrams) + _jaccard(a.words, b.words)) / 2, 3)


def is_version(a: str | Shingles, b: str | Shingles) -> bool:
    return similarity(a, b) >= THRESHOLD


Op = Literal["equal", "delete", "insert"]


def word_diff(before: str | None, after: str | None) -> list[tuple[Op, str]]:
    """How `before` became `after`, word by word (spacing and punctuation are tokens too), as runs
    of equal, deleted and inserted text. A replaced run is a deletion followed by an insertion."""
    a, b = _TOKENS.findall((before or "")[:COMPARED_CHARS]), _TOKENS.findall((after or "")[:COMPARED_CHARS])
    out: list[tuple[Op, str]] = []

    def add(op: Op, tokens: Sequence[str]) -> None:
        if not tokens:
            return
        text = "".join(tokens)
        if out and out[-1][0] == op:
            out[-1] = (op, out[-1][1] + text)
        else:
            out.append((op, text))

    for tag, i1, i2, j1, j2 in SequenceMatcher(None, a, b, autojunk=False).get_opcodes():
        if tag == "equal":
            add("equal", a[i1:i2])
        else:
            add("delete", a[i1:i2])
            add("insert", b[j1:j2])
    return out


# How good an outcome is, to tell whether a later version did better. `running` has no verdict yet.
OUTCOME_RANK = {"failed": 0, "interrupted": 1, "superseded": 1, "open": 1, "done": 2}
Change = Literal["improved", "worse", "same"]


def change(before: str | None, after: str | None) -> Change | None:
    """improved / worse / same from one version's outcome to the next; None if either is unknown."""
    if before not in OUTCOME_RANK or after not in OUTCOME_RANK:
        return None
    delta = OUTCOME_RANK[after] - OUTCOME_RANK[before]
    return "improved" if delta > 0 else "worse" if delta < 0 else "same"
