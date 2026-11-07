"""A prompt's facts (insights/facts.py): one block every page reads, from an index row and the spec."""

from gitprompt.insights import facts
from gitprompt.sources.spec import load_specs

ROW = {"source": "codex", "api_requests": 0, "cost_usd": 0, "input_tokens": 0, "output_tokens": 0}


def test_the_model_comes_from_model_calls_else_from_the_hooks():
    assert facts.main_model({"models": {"gpt-a": 1, "gpt-b": 3}}, {"model": "gpt-a"}) == "gpt-b"
    assert facts.main_model({}, {"model": "gpt-6-luna"}) == "gpt-6-luna"  # a turn whose tool reported no call
    assert facts.main_model({"models": {None: 2}}, {}) is None


def test_no_model_call_means_no_tokens_and_no_cost_not_zeros(settings):
    codex = load_specs(settings)["codex"]
    f = facts.facts(ROW | {"model": "gpt-6-luna"}, codex, "done")
    assert f["tokens"] is None and f["cost_usd"] is None and f["model_source"] == "hook"
    assert f["connector"] == {"id": "codex", "name": "OpenAI Codex CLI", "label": "Codex", "assistant": "Codex"}


def test_tokens_add_up_and_cost_needs_a_tool_that_reports_one(settings):
    specs = load_specs(settings)
    row = ROW | {"api_requests": 2, "input_tokens": 100, "output_tokens": 20, "cache_read_tokens": 900, "reasoning_tokens": 5}
    codex = facts.facts(row | {"model": "gpt-6-luna"}, specs["codex"])
    assert codex["tokens"] == {"input": 100, "output": 20, "cache_read": 900, "cache_creation": 0, "reasoning": 5, "total": 1020}
    assert codex["cost_usd"] is None and codex["model_source"] == "usage"  # Codex reports no cost
    claude = facts.facts(row | {"source": "claude-code"}, specs["claude-code"])
    assert claude["cost_usd"] == 0  # telemetry maps a cost: a free call is $0, not unknown


def test_calls_without_token_counts_show_no_tokens(settings):
    """Codex reports a cancelled call (Ctrl-C) with no token_usage: that is unknown, not zero."""
    f = facts.facts(ROW | {"api_requests": 1, "model": "gpt-6-luna"}, load_specs(settings)["codex"], "interrupted")
    assert f["tokens"] is None and f["api_requests"] == 1 and f["model_source"] == "usage"


def test_an_unknown_source_is_named_by_its_id():
    assert facts.facts(ROW | {"source": "mystery"}, None)["connector"]["label"] == "mystery"
