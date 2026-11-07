"""Search snippets: a window around the first match, every match marked (synthetic texts)."""

from gitprompt.insights.snippets import ELLIPSIS, highlight


def text_of(segments):
    return "".join(s["text"] for s in segments)


def hits(segments):
    return [s["text"] for s in segments if s["hit"]]


def test_marks_every_word_ignoring_case():
    out = highlight("Fix the Parser, then test the parser again", "parser test")
    assert hits(out) == ["Parser", "test", "parser"]
    assert text_of(out) == "Fix the Parser, then test the parser again"


def test_overlapping_matches_are_one_mark():
    assert hits(highlight("a configuration file", "config configuration")) == ["configuration"]


def test_window_starts_near_the_first_match_on_a_word_boundary():
    text = "lorem ipsum " * 40 + "the needle is here " + "dolor sit " * 40
    out = highlight(text, "needle", width=80)
    assert out[0] == {"text": ELLIPSIS, "hit": False} and out[-1] == {"text": ELLIPSIS, "hit": False}
    assert hits(out) == ["needle"]
    assert not out[1]["text"].startswith(("orem", "psum")), out[1]  # never starts inside a word
    assert len(text_of(out)) <= 80 + 2


def test_spacing_is_flattened_and_no_match_shows_the_start():
    assert text_of(highlight("one\n\n  two", "two")) == "one two"
    assert highlight("short text", "absent") == [{"text": "short text", "hit": False}]
    assert highlight("short text", None) == [{"text": "short text", "hit": False}]
    assert highlight(None, "x") == []


def test_short_words_match_like_the_filter():
    assert hits(highlight("go to the CI page", "ci go")) == ["go", "CI"]
