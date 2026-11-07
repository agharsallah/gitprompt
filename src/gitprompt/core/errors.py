"""Domain errors. Raised by the core modules, turned into HTTP responses in one place (api/app.py),
so nothing outside `api/` needs to know about HTTP."""


class GitPromptError(Exception):
    """Base class: `status` is the HTTP status the API answers with."""

    status = 400


class NotFound(GitPromptError):
    status = 404


class Conflict(GitPromptError):
    """The request is valid, but the current state on this machine doesn't allow it."""

    status = 409


class ConfigError(Conflict):
    """A tool's config file can't be safely edited (invalid JSON, not an object...)."""


class PolicyBlocked(Conflict):
    """Organization or user settings turn this source off. GitPrompt reports it, never works around it."""


class SpecError(GitPromptError):
    """A source spec (bundled or user override) can't be loaded."""

    status = 500
