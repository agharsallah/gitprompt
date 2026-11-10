#!/usr/bin/env python3
"""Compare two GitPrompt daemons endpoint by endpoint (stdlib only).

Used while the Python daemon is deprecated: run the legacy Python daemon and the Rust daemon on two
copies of the same database, then

    python3 tools/compare_backends.py http://127.0.0.1:7990 http://127.0.0.1:7991

Every GET the dashboard makes is sent to both; JSON answers are compared value by value and each
difference is printed with its JSON path. Exit status 1 when anything differs (or a request fails).
`--only SUBSTR` limits the run to matching paths, `--max N` caps the differences shown per endpoint.

Known, deliberate differences are normalized away unless `--strict` (see `normalize`): values that
name the daemon's own data directory or port, the order of connectors (Rust lists them by id; the
dashboard groups them itself; the Fields page's "not captured" list follows it), the key of a usage
group with no name (`"None"` was a Python artifact; Rust writes `"null"`), and a connector's `last_at`
(Rust: its latest prompt or event, as the dashboard's "Last event" says; Python: its latest prompt).

Not normalized: a prompt's model. Rust shows the main thread's model; Python the model of most calls
of any kind, so it differs (in facts, stats by model, stories, search and exports) on turns where
background or subagent calls on another model outnumber the main thread's (ADR-0023).

Each line also shows how long each daemon took (ms); the totals are printed at the end.
"""

import argparse
import json
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

GROUPS = [
    "day", "day_hour", "hour", "weekday", "weekday_hour", "origin", "source", "surface", "session", "session_length", "repo",
    "branch", "command", "size", "text", "outcome", "model", "tool", "tool_kind", "work", "query_source",
]  # fmt: skip
TYPED = "origin=typed&origin=slash_command"


def timed(base: str, path: str) -> tuple[int, object, float]:
    start = time.perf_counter()
    status, body = get(base, path)
    return status, body, (time.perf_counter() - start) * 1000


def get(base: str, path: str) -> tuple[int, object]:
    request = urllib.request.Request(base + path, headers={"Host": "127.0.0.1"})
    try:
        with urllib.request.urlopen(request, timeout=60) as response:
            body = response.read()
            status = response.status
            kind = response.headers.get("content-type", "")
    except urllib.error.HTTPError as e:
        body, status, kind = e.read(), e.code, e.headers.get("content-type", "")
    if "json" in kind:
        return status, json.loads(body or b"null")
    return status, body.decode("utf-8", "replace")


def paths(base: str) -> list[str]:
    """Endpoints to compare, with ids taken from the first daemon's data."""
    out = ["/api/prompts?limit=50", f"/api/prompts?limit=50&{TYPED}", "/api/prompts?limit=200", "/api/sources", "/api/sources/recent"]
    out += ["/api/sources/problems", "/api/fields", "/api/settings", "/api/data", "/api/security", f"/api/stats/telemetry?{TYPED}"]
    out += [f"/api/stats?group_by={g}&{TYPED}" for g in GROUPS] + ["/api/stats", "/api/stats?group_by=origin"]
    out += [f"/api/stats?group_by=model&split_by=origin&{TYPED}", "/api/stats?group_by=day&order=cost_usd&limit=5"]
    _, prompts = get(base, "/api/prompts?limit=200")
    prompts = prompts if isinstance(prompts, list) else []
    ids = [p["id"] for p in prompts][:: max(1, len(prompts) // 15)]
    sessions = list(dict.fromkeys(p["fields"].get("session_id") for p in prompts if p.get("fields", {}).get("session_id")))[:5]
    out += [f"/api/prompts/{i}" for i in ids] + [f"/api/stories/prompt/{i}" for i in ids]
    out += [f"/api/stories/session/{urllib.parse.quote(s)}" for s in sessions]
    _, projects = get(base, "/api/stats?group_by=repo&limit=5")
    for group in (projects or {}).get("groups", []) if isinstance(projects, dict) else []:
        if group.get("key"):
            _, story = get(base, f"/api/stories/session/{urllib.parse.quote(sessions[0])}") if sessions else (0, None)
            project = (story or {}).get("project", {}).get("id") if isinstance(story, dict) else None
            if project:
                out.append(f"/api/stories/project/{urllib.parse.quote(project)}")
            break
    for word in ("test", "fix", "the rust", "a"):
        out.append(f"/api/search?q={urllib.parse.quote(word)}")
    for kind in ("overview", "prompts"):
        out += [f"/api/export/{kind}?format=json", f"/api/export/{kind}/charts"]
    if sessions:
        out.append(f"/api/export/session?id={urllib.parse.quote(sessions[0])}&format=json")
    return out


# Keys whose values depend on where each daemon runs, not on the data.
PER_INSTALL = {"home", "db_path", "file", "receiver", "token_file", "token_changed", "db_bytes", "path"}


def normalize(path: str, value: object) -> object:
    """Remove the deliberate differences (see the module docs)."""
    if isinstance(value, dict):
        out = {}
        for key, item in value.items():
            if key in PER_INSTALL or (key == "last_at" and path.split("?")[0] == "/api/sources"):
                continue
            if key in ("work_usage", "model_usage") and isinstance(item, dict):
                item = {("null" if k == "None" else k): v for k, v in item.items()}
            out[key] = normalize(path, item)
        if path.startswith("/api/security") and isinstance(out.get("connected"), list):
            out["connected"] = sorted(out["connected"])
        if path.startswith("/api/fields") and isinstance(out.get("not_captured"), list):
            out["not_captured"] = sorted(out["not_captured"], key=lambda n: (str(n.get("source")), str(n.get("event"))))
        return out
    if isinstance(value, list):
        items = [normalize(path, v) for v in value]
        if path.split("?")[0] == "/api/sources" and all(isinstance(v, dict) and "id" in v for v in items):
            items.sort(key=lambda v: v["id"])
        return items
    return value


def diff(a: object, b: object, path: str = "$") -> list[str]:
    if isinstance(a, dict) and isinstance(b, dict):
        out = []
        for key in list(a) + [k for k in b if k not in a]:
            if key not in b:
                out.append(f"{path}.{key}: only in A = {short(a[key])}")
            elif key not in a:
                out.append(f"{path}.{key}: only in B = {short(b[key])}")
            else:
                out += diff(a[key], b[key], f"{path}.{key}")
        return out
    if isinstance(a, list) and isinstance(b, list):
        out = [f"{path}: length {len(a)} vs {len(b)}"] if len(a) != len(b) else []
        for i, (x, y) in enumerate(zip(a, b, strict=False)):
            out += diff(x, y, f"{path}[{i}]")
        return out
    if isinstance(a, int | float) and isinstance(b, int | float) and not isinstance(a, bool) and not isinstance(b, bool):
        return [] if abs(a - b) <= 1e-6 * max(1.0, abs(a), abs(b)) else [f"{path}: {a!r} vs {b!r}"]
    return [] if a == b else [f"{path}: {short(a)} vs {short(b)}"]


def short(value: object, limit: int = 160) -> str:
    text = json.dumps(value, ensure_ascii=False) if not isinstance(value, str) else repr(value)
    return text if len(text) <= limit else text[:limit] + "…"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("a", help="first daemon, e.g. http://127.0.0.1:7990 (legacy Python)")
    parser.add_argument("b", help="second daemon, e.g. http://127.0.0.1:7991 (Rust)")
    parser.add_argument("--only", default="", help="compare only paths containing this text")
    parser.add_argument("--max", type=int, default=12, help="differences shown per endpoint")
    parser.add_argument("--strict", action="store_true", help="compare as is, without normalizing known differences")
    args = parser.parse_args()
    failed, total_a, total_b = 0, 0.0, 0.0
    for path in [p for p in paths(args.a) if args.only in p]:
        (sa, a, ms_a), (sb, b, ms_b) = timed(args.a, path), timed(args.b, path)
        total_a, total_b = total_a + ms_a, total_b + ms_b
        took = f"[{ms_a:7.1f} ms vs {ms_b:7.1f} ms]"
        problems = [f"status {sa} vs {sb}"] if sa != sb else []
        if not args.strict:
            a, b = normalize(path, a), normalize(path, b)
        problems += diff(a, b)
        if problems:
            failed += 1
            print(f"✗ {took} {path}  ({len(problems)} differences)")
            for line in problems[: args.max]:
                print(f"    {line}")
        else:
            print(f"✓ {took} {path}")
    print(f"\ntotal: {total_a:.0f} ms (A) vs {total_b:.0f} ms (B)")
    print(f"{failed} endpoint(s) differ" if failed else "all endpoints match")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
