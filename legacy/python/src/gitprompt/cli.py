"""Command line entry point: `gitprompt serve` (default), `gitprompt compact`, `gitprompt reindex` and `gitprompt check`.

DEPRECATED: the daemon is now the Rust `gitprompt` binary (rust/, see docs/adr/0023-rust-backend.md).
This entry point hands every command to it unchanged (`exec`, same arguments), after one notice on
stderr. To run this Python daemon instead (e.g. to compare both with tools/compare_backends.py),
pass `--legacy` or set GITPROMPT_LEGACY=1.

The Rust binary is looked up in this order: $GITPROMPT_BIN, the repository's rust/target/release and
rust/target/debug builds, ~/.cargo/bin, then $PATH (skipping this Python script itself).
"""

import argparse
import os
import shutil
import sys
from dataclasses import replace
from pathlib import Path

LEGACY_FLAG = "--legacy"
REPO = Path(__file__).resolve().parents[4]  # legacy/python/src/gitprompt/cli.py -> the repository root

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


def check(settings: Settings, args: argparse.Namespace) -> None:
    """Per connector: is it connected, and the newest prompt of each app it was typed in, with how it
    was attributed. Type one prompt in each app, then run this to see what arrived."""
    from datetime import UTC, datetime

    from gitprompt.insights.captures import recent
    from gitprompt.insights.indexer import Indexer
    from gitprompt.sources import probe
    from gitprompt.sources.connection import Connection
    from gitprompt.sources.spec import load_specs
    from gitprompt.storage.store import Store

    specs = load_specs(settings)
    if not settings.db_path.exists():
        print(f"No database yet at {settings.db_path}: start GitPrompt and send a prompt first.")
        rows = []
    else:
        store = Store(settings.db_path)
        try:
            Indexer(store, specs).ensure()
            rows = recent(store, specs)
        finally:
            store.close()
    now = datetime.now(UTC)
    for spec in (s for s in specs.values() if not s.part_of):
        layers = [spec, *(s for s in specs.values() if s.part_of == spec.id)]
        state = ", ".join(f"{s.layer or 'hooks'} {'connected' if Connection(s, settings).connected else 'not connected'}" for s in layers)
        found = "" if probe.detected(spec, settings) else " (not detected on this machine)"
        print(f"\n{spec.name}: {state}{found}")
        mine = [r for r in rows if r["connector"]["id"] == spec.id]
        for r in mine:
            where = r["surface"]["name"] if r["surface"] else "surface unknown"
            project = (r["project"] or {}).get("name") or "no folder"
            branch = f" ({r['branch']})" if r["branch"] else ""
            signals = " ".join(f"{k}={v}" for k, v in r["signals"].items())
            print(
                f"  {where:<16} {_ago(now, r['received_at']):>9}  {project}{branch}  session {str(r['session'] or '-')[:12]}"
                f"  {r['model'] or 'no model'}  {r['prompts']:,} prompts{'  ' + signals if signals else ''}"
            )
        if not mine:
            print("  no prompt captured yet")
        for app, why in spec.cannot_capture.items():
            print(f"  not capturable: {app}: {why}")


def _ago(now, iso: str) -> str:
    from datetime import datetime

    seconds = int((now - datetime.fromisoformat(iso)).total_seconds())
    for unit, size in (("d", 86400), ("h", 3600), ("min", 60)):
        if seconds >= size:
            return f"{seconds // size} {unit} ago"
    return f"{seconds} s ago"


def _mb(n: int) -> str:
    return f"{n / 1e6:,.1f} MB"


def _days(value: str) -> int:
    days = int(value)
    if days < 1:
        raise argparse.ArgumentTypeError("must be at least 1")
    return days


def rust_binary(environ: dict[str, str] | None = None) -> Path | None:
    """The Rust `gitprompt` binary to hand commands to, or None (see the module docs for the order)."""
    environ = os.environ if environ is None else environ
    if explicit := environ.get("GITPROMPT_BIN"):
        return Path(explicit) if Path(explicit).is_file() else None
    me = Path(sys.argv[0]).resolve() if sys.argv and sys.argv[0] else None
    candidates = [REPO / "rust" / "target" / profile / "gitprompt" for profile in ("release", "debug")]
    candidates.append(Path.home() / ".cargo" / "bin" / "gitprompt")
    for directory in environ.get("PATH", "").split(os.pathsep):
        if found := shutil.which("gitprompt", path=directory):
            candidates.append(Path(found))
    for candidate in candidates:
        if candidate.is_file() and os.access(candidate, os.X_OK) and candidate.resolve() != me and _is_native(candidate):
            return candidate
    return None


def _is_native(path: Path) -> bool:
    """A compiled binary, not another Python entry-point script (which starts with a #! line)."""
    try:
        with path.open("rb") as file:
            return file.read(2) != b"#!"
    except OSError:
        return False


def delegate(argv: list[str], environ: dict[str, str] | None = None) -> None:
    """Replace this process with the Rust daemon's CLI, or exit 2 with how to build it."""
    binary = rust_binary(environ)
    if binary is None:
        print(
            "gitprompt: the Python daemon is deprecated and the Rust binary was not found.\n"
            "  Build it:   cargo build --release --manifest-path rust/Cargo.toml -p gitprompt-cli   (or `just build`)\n"
            "  Or set GITPROMPT_BIN to its path, or run the legacy Python daemon with --legacy.",
            file=sys.stderr,
        )
        raise SystemExit(2)
    print(f"gitprompt: the Python daemon is deprecated; running {binary} (pass --legacy for the old one)", file=sys.stderr)
    sys.stdout.flush()
    os.execv(binary, [str(binary), *argv])


def main() -> None:
    argv = sys.argv[1:]
    if LEGACY_FLAG in argv or os.environ.get("GITPROMPT_LEGACY") == "1":
        sys.argv = [sys.argv[0], *(a for a in argv if a != LEGACY_FLAG)]
        legacy_main()
    else:
        delegate(argv)


def legacy_main() -> None:
    parser = argparse.ArgumentParser(prog="gitprompt", description="Capture your AI prompts locally (legacy Python daemon).")
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

    p = commands.add_parser("check", help="per connector and app: the newest captured prompt and how it was attributed")
    p.set_defaults(run=check)

    args = parser.parse_args()
    args.run(Settings(), args)
