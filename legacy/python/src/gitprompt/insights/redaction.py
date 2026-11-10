"""Finding credentials in text, and hiding them in stored JSON. To detect a new kind of secret,
add one named pattern to SECRETS."""

import re
from typing import Any

from gitprompt.core.payload import map_strings

SECRETS: dict[str, re.Pattern[str]] = {
    name: re.compile(pattern)
    for name, pattern in {
        "private key": r"-----BEGIN [A-Z ]*PRIVATE KEY-----[\s\S]*?(?:-----END [A-Z ]*PRIVATE KEY-----|$)",
        "AWS access key": r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b",
        "GitHub token": r"\b(?:gh[pousr]_[A-Za-z0-9]{36,}|github_pat_\w{22,})",
        "Anthropic API key": r"\bsk-ant-[\w-]{20,}",
        "OpenAI API key": r"\bsk-(?!ant-)(?:proj-)?[\w-]{20,}",
        "Slack token": r"\bxox[abposr]-[\w-]{10,}",
        "Google API key": r"\bAIza[\w-]{35}",
        "Stripe key": r"\b[rs]k_live_[A-Za-z0-9]{20,}",
        "JWT": r"\beyJ[\w-]{10,}\.[\w-]{10,}\.[\w-]{10,}",
        "password or key assignment": r"(?i)\b(?:password|passwd|pwd|secret|api[_-]?key|token)\s*[:=]\s*['\"]?[^\s'\"]{8,}",
    }.items()
}


def find_secrets(text: str) -> list[str]:
    """Names of the kinds of secrets found (never the values)."""
    return [name for name, pattern in SECRETS.items() if pattern.search(text)]


def hide(text: str) -> str:
    """Replace every secret match in a text with the name of its kind."""
    for name, pattern in SECRETS.items():
        text = pattern.sub(f"[hidden {name}]", text)
    return text


def redact(value: Any) -> Any:
    """Replace every secret match in a JSON value, keeping its structure."""
    return map_strings(value, hide)
