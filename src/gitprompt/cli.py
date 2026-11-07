"""Command line entry point: `gitprompt serve`."""

import argparse
from dataclasses import replace


def main() -> None:
    import uvicorn

    from gitprompt.api.app import create_app
    from gitprompt.core.config import Settings

    parser = argparse.ArgumentParser(prog="gitprompt", description="Capture your AI prompts locally.")
    parser.add_argument("command", nargs="?", default="serve", choices=["serve"])
    parser.add_argument("--port", type=int, help="overrides GITPROMPT_PORT (default 7878)")
    args = parser.parse_args()
    settings = Settings()
    if args.port is not None:
        settings = replace(settings, port=args.port)
    print(f"GitPrompt is running on {settings.receiver}")
    uvicorn.run(create_app(settings), host=settings.host, port=settings.port, log_level=settings.log_level)
