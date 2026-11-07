"""Prompt lineage: similarity, word diff and outcome change (synthetic texts)."""

import pytest

from gitprompt.insights import lineage
from gitprompt.insights.lineage import change, is_version, normalize, similarity, word_diff


def test_normalize_ignores_case_punctuation_and_spacing():
    assert normalize("  Fix the  Login-bug, please!\n") == "fix the login bug please"
    assert normalize(None) == ""


@pytest.mark.parametrize(
    ("a", "b"),
    [
        ("Fix the login bug", "fix the login bug!"),  # same text
        ("add tests for the parser", "add unit tests for the parser module"),  # extended
        ("rename the config loader", "rename the config loader and its tests"),
        ("explain the cache layer", "explain the caching layer"),  # reworded
    ],
)
def test_versions_of_one_instruction_pass_the_threshold(a, b):
    assert is_version(a, b), similarity(a, b)


@pytest.mark.parametrize(
    ("a", "b"),
    [
        ("open the settings page", "deploy the release"),  # different asks
        ("commit the parser", "deploy the cache"),  # one shared word is not enough
        ("summarize", "summarize " + "a long pasted brief with many words " * 20),  # lengths too far apart
        ("", "anything"),
    ],
)
def test_different_prompts_stay_below_it(a, b):
    assert not is_version(a, b), similarity(a, b)


def test_similarity_is_symmetric_and_bounded():
    a, b = "write the changelog for 2.0", "write a changelog for version 2.0"
    assert similarity(a, b) == similarity(b, a)
    assert 0 < similarity(a, b) < 1 == similarity(a, a.upper())


def test_long_texts_are_compared_on_their_start():
    head = "refactor the importer into small modules " * 120  # longer than COMPARED_CHARS
    assert similarity(head + "tail one", head + "a completely different ending " * 200) == 1.0
    assert len(normalize("x " * lineage.COMPARED_CHARS)) <= lineage.COMPARED_CHARS


def test_word_diff_marks_what_was_removed_and_added():
    ops = word_diff("add tests for the parser", "add unit tests for the lexer")
    assert [(op, text.strip()) for op, text in ops if op != "equal"] == [("insert", "unit"), ("delete", "parser"), ("insert", "lexer")]
    assert word_diff("same text", "same text") == [("equal", "same text")]
    assert word_diff(None, "new") == [("insert", "new")]


def test_word_diff_rebuilds_both_texts():
    before, after = "Fix the bug, then\nrun the tests.", "Fix both bugs; then run all tests!"
    ops = word_diff(before, after)
    assert "".join(t for op, t in ops if op != "insert") == before
    assert "".join(t for op, t in ops if op != "delete") == after


@pytest.mark.parametrize(
    ("before", "after", "expected"),
    [
        ("failed", "done", "improved"),
        ("superseded", "done", "improved"),
        ("done", "failed", "worse"),
        ("open", "superseded", "same"),
        ("done", "running", None),
        (None, "done", None),
    ],
)
def test_change_compares_outcomes(before, after, expected):
    assert change(before, after) == expected
