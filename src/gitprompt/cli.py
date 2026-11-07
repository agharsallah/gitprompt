"""Command line entry point: `gitprompt serve` (default), `gitprompt compact` and `gitprompt reindex`."""

import argparse
from dataclasses import replace

from gitprompt.core.config import Settings


def serve(settings: Settings, args: argparse.Namespace) -> None:
    import uvicorn

    from gitprompt.api.app import create_app

    if args.port is not None:
        settings = replace(settings, port=args.port)
    print(f"GitPrompt is running on {settings.receiver}")
    uvicorn.run(create_app(settings), host=settings.host, port=settings.port, log_level=settings.log_level)


def compact(settings: Settings, args: argparse.Namespace) -> None:
    from gitprompt.capture.compact import compact as run
    from gitprompt.sources.spec import load_specs
    from gitprompt.storage.store import Store

    store = Store(settings.db_path)
    try:
        report = run(store, load_specs(settings), older_than_days=args.events_older_than, apply=args.apply)
    finally:
        store.close()
    for r in report.removals:
        print(f"{r.rows:>9,}  {_mb(r.bytes):>9}  {r.source} {r.event}  ({r.why})")
    print(f"{report.rows:>9,}  {_mb(report.bytes):>9}  events to remove, database is {_mb(report.size_before)}")
    if report.size_after is None:
        print("Dry run: nothing changed. Run again with --apply to delete these events and VACUUM.")
    else:
        print(f"Done: database is now {_mb(report.size_after)}.")


def reindex(settings: Settings, args: argparse.Namespace) -> None:
    import time

    from gitprompt.insights.indexer import Indexer
    from gitprompt.sources.spec import load_specs
    from gitprompt.storage.store import Store

    store = Store(settings.db_path)
    try:
        start = time.perf_counter()
        count = Indexer(store, load_specs(settings)).rebuild()
    finally:
        store.close()
    print(f"Indexed {count:,} prompts in {time.perf_counter() - start:.1f} s.")


def _mb(n: int) -> str:
    return f"{n / 1e6:,.1f} MB"


def _days(value: str) -> int:
    days = int(value)
    if days < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return days


def main() -> None:
    parser = argparse.ArgumentParser(prog="gitprompt", description="Capture your AI prompts locally.")
    port = {"type": int, "help": "overrides GITPROMPT_PORT (default 7878)"}
    parser.add_argument("--port", **port)  # `gitprompt --port N` still serves
    parser.set_defaults(run=serve)
    commands = parser.add_subparsers(title="commands")

    p = commands.add_parser("serve", help="run the receiver and dashboard (default)")
    p.add_argument("--port", default=argparse.SUPPRESS, **port)
    p.set_defaults(run=serve)

    p = commands.add_parser("compact", help="delete stored events the source specs no longer keep, then VACUUM")
    p.add_argument("--apply", action="store_true", help="actually delete (default: dry run, show what would go)")
    p.add_argument("--events-older-than", type=_days, metavar="DAYS", help="also delete raw events older than DAYS days")
    p.set_defaults(run=compact)

    p = commands.add_parser("reindex", help="rebuild the prompt index (filters, search, statistics) from the stored prompts")
    p.set_defaults(run=reindex)

    args = parser.parse_args()
    args.run(Settings(), args)
