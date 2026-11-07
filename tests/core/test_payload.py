from gitprompt.core.payload import clip, extract, pluck


def test_pluck_follows_dots_and_indexes():
    data = {"a": {"b": [{"c": 1}]}}
    assert pluck(data, "a.b[0].c") == 1
    assert pluck(data, "a.b[5].c") is None
    assert pluck(data, "a.x") is None


def test_extract_takes_first_non_empty_candidate():
    mapping = {"session_id": ["session_id", "sessionId"], "model": ["model"]}
    assert extract({"session_id": "", "sessionId": "s1"}, mapping) == {"session_id": "s1"}


def test_clip_keeps_short_values():
    assert clip({"a": "x" * 5, "b": ["y" * 20]}, limit=10) == {"a": "xxxxx", "b": ["y" * 10 + "… [10 more characters not stored]"]}
