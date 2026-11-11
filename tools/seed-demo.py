#!/usr/bin/env python3
"""Fill an empty GitPrompt data folder with synthetic prompts, for demos and the README screenshots.

    python3 tools/seed-demo.py /tmp/gitprompt-demo
    GITPROMPT_HOME=/tmp/gitprompt-demo GITPROMPT_PORT=7979 backend/target/release/gitprompt serve

About 270 made-up prompts over the last four weeks, from Claude Code (hooks + telemetry), Codex,
Cursor and Copilot CLI, in five fictional projects. It writes <home>/spool/<source>.jsonl only: the
daemon drains the spool at start-up and keeps each payload's backdated `spooled_at` time. Never
point it at your real ~/.gitprompt. Standard library only, deterministic (fixed seed).
"""
import json
import random
import sys
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

random.seed(7)
home = Path(sys.argv[1])
spool = home / "spool"
spool.mkdir(parents=True, exist_ok=True)
files = {}


def emit(source, at, payload):
    f = files.setdefault(source, open(spool / f"{source}.jsonl", "a"))
    iso = at.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.%f") + "Z"
    f.write(json.dumps({"spooled_at": iso, "payload": payload}) + "\n")


PROJECTS = {
    "/Users/dev/code/shop-api": [
        "Add a retry with exponential backoff to the payment client",
        "Why does the checkout test fail only on CI? Look at the fixtures",
        "Refactor the order service so pricing rules live in one module",
        "Add pagination to GET /orders and update the OpenAPI spec",
        "Write integration tests for the refund webhook",
        "The coupon validation accepts expired codes, fix it and add a test",
        "Migrate the users table to add a nullable locale column",
        "Explain how the inventory reservation lock works",
    ],
    "/Users/dev/code/web-dashboard": [
        "Make the revenue chart responsive on mobile",
        "Add a dark mode toggle that follows the system setting",
        "The date picker loses focus after selecting a range, fix it",
        "Replace the hand-rolled table with the shared DataTable component",
        "Add empty states to the customers page",
        "Lazy-load the analytics route and measure the bundle size",
    ],
    "/Users/dev/code/infra": [
        "Add a staging environment to the Terraform config",
        "Why is the nightly backup job timing out?",
        "Write a GitHub Actions workflow that runs migrations before deploy",
        "Tighten the S3 bucket policy to the VPC endpoint only",
    ],
    "/Users/dev/code/mobile-app": [
        "Add pull-to-refresh to the orders list",
        "Crash on Android 14 when opening the camera, investigate",
        "Cache product images on disk with a size limit",
        "Localize the onboarding screens into German and French",
    ],
    "/Users/dev/code/ml-pipeline": [
        "Speed up the feature extraction step, it takes 40 minutes",
        "Add a data validation step before training",
        "Plot the precision-recall curve for the last three runs",
    ],
}
SLASH = ["/review", "/init", "/compact", "/security-review"]
ANSWERS = [
    "Done. I added the change, updated the tests and they all pass.",
    "Fixed: the root cause was a stale fixture. Tests pass locally.",
    "I refactored the module and kept the public API unchanged; 42 tests pass.",
    "Here is the explanation, with the relevant files and line numbers.",
]
FILES = ["src/client.ts", "src/orders/service.ts", "src/api/routes.ts", "tests/orders.test.ts",
         "README.md", "src/components/Chart.svelte", "main.tf", "src/pricing.rs", "app/build.gradle"]
CLAUDE_MODELS = [("claude-opus-5-5", 0.55), ("claude-sonnet-5-5", 0.35), ("claude-haiku-5-5", 0.10)]
ENTRYPOINTS = [("cli", 0.55), ("claude-vscode", 0.3), ("claude-desktop", 0.15)]
TOOLS = [("claude-code", 0.62), ("codex", 0.18), ("cursor", 0.12), ("copilot-cli", 0.08)]


def weighted(pairs):
    r, acc = random.random(), 0.0
    for v, w in pairs:
        acc += w
        if r <= acc:
            return v
    return pairs[-1][0]


def attr(k, v):
    if isinstance(v, bool):
        return {"key": k, "value": {"boolValue": v}}
    if isinstance(v, int):
        return {"key": k, "value": {"intValue": str(v)}}
    if isinstance(v, float):
        return {"key": k, "value": {"doubleValue": v}}
    return {"key": k, "value": {"stringValue": v}}


def otel_log(at, resource, attrs):
    return {"resourceLogs": [{"resource": {"attributes": [attr(k, v) for k, v in resource.items()]},
                              "scopeLogs": [{"logRecords": [{
                                  "timeUnixNano": str(int(at.timestamp() * 1e9)),
                                  "body": {"stringValue": "claude_code." + attrs["event.name"]},
                                  "attributes": [attr(k, v) for k, v in attrs.items()]}]}]}]}


def claude_turn(at, session, cwd, text, entry, model):
    pid = str(uuid.uuid4())
    base = {"session_id": session, "prompt_id": pid, "cwd": cwd, "permission_mode": "default",
            "transcript_path": f"/Users/dev/.claude/projects/x/{session}.jsonl"}
    emit("claude-code", at, {**base, "hook_event_name": "UserPromptSubmit", "prompt": text})
    res = {"service.name": "claude-code-desktop" if entry == "claude-desktop" else "claude-code",
           "app.entrypoint": entry, "app.version": "2.4.1"}
    emit("claude-code-otel", at, otel_log(at, res, {"event.name": "user_prompt", "session.id": session,
                                                    "prompt.id": pid, "prompt_length": len(text)}))
    t = at
    for _ in range(random.randint(2, 9)):
        t += timedelta(seconds=random.randint(4, 40))
        inp, out = random.randint(8000, 60000), random.randint(200, 3000)
        price = {"claude-opus-5-5": 15, "claude-sonnet-5-5": 3, "claude-haiku-5-5": 1}[model]
        emit("claude-code-otel", t, otel_log(t, res, {
            "event.name": "api_request", "session.id": session, "prompt.id": pid, "model": model,
            "input_tokens": inp // 10, "cache_read_tokens": inp, "output_tokens": out,
            "cache_creation_tokens": random.randint(0, 4000),
            "cost_usd": round((inp * 0.1 + out * 5) * price / 1e6, 4),
            "duration_ms": random.randint(1500, 14000)}))
        tool = random.choice(["Read", "Read", "Edit", "Bash", "Grep", "Write"])
        f = f"{cwd}/{random.choice(FILES)}"
        tin = {"command": "pnpm test", "description": "Run tests"} if tool == "Bash" else (
            {"pattern": "retry", "path": cwd} if tool == "Grep" else {"file_path": f, "old_string": "a", "new_string": "b"} if tool == "Edit" else {"file_path": f})
        emit("claude-code", t, {**base, "hook_event_name": "PostToolUse", "tool_name": tool, "tool_input": tin,
                                "tool_response": {"success": True}, "tool_use_id": str(uuid.uuid4())})
    t += timedelta(seconds=random.randint(3, 20))
    emit("claude-code", t, {**base, "hook_event_name": "Stop", "stop_hook_active": False,
                            "last_assistant_message": random.choice(ANSWERS)})


def codex_turn(at, session, cwd, text):
    tid = str(uuid.uuid4())
    base = {"session_id": session, "turn_id": tid, "cwd": cwd, "model": "gpt-5.5-codex", "permission_mode": "default",
            "transcript_path": f"/Users/dev/.codex/sessions/{session}.jsonl"}
    emit("codex", at, {**base, "hook_event_name": "UserPromptSubmit", "prompt": text})
    t = at
    for _ in range(random.randint(2, 6)):
        t += timedelta(seconds=random.randint(5, 40))
        emit("codex", t, {**base, "hook_event_name": "PostModelRequest", "status": "completed", "error": None,
                          "token_usage": {"input_tokens": 20000, "cached_input_tokens": 15000, "cache_write_input_tokens": 0,
                                          "output_tokens": 600, "reasoning_output_tokens": 200, "total_tokens": 20600}})
        emit("codex", t, {**base, "hook_event_name": "PostToolUse", "tool_name": "Bash", "tool_use_id": str(uuid.uuid4()),
                          "tool_input": {"command": "pytest -q"}, "tool_response": "12 passed\n"})
    emit("codex", t + timedelta(seconds=8), {**base, "hook_event_name": "Stop", "stop_hook_active": False,
                                             "last_assistant_message": random.choice(ANSWERS)})


def cursor_turn(at, session, cwd, text):
    gid = str(uuid.uuid4())
    base = {"conversation_id": session, "generation_id": gid, "model": "claude-sonnet-5-5", "cursor_version": "3.4.1",
            "workspace_roots": [cwd], "transcript_path": f"/tmp/cursor/{session}.jsonl"}
    emit("cursor", at, {**base, "hook_event_name": "beforeSubmitPrompt", "prompt": text, "attachments": []})
    t = at + timedelta(seconds=random.randint(10, 60))
    emit("cursor", t, {**base, "hook_event_name": "postToolUse", "tool_name": "Shell", "tool_input": {"command": "pnpm test"},
                       "tool_output": "{\"exit_code\":0}", "tool_use_id": str(uuid.uuid4()), "cwd": cwd, "duration": 3100})
    emit("cursor", t + timedelta(seconds=5), {**base, "hook_event_name": "stop", "status": random.choice(["completed"] * 5 + ["aborted"]), "loop_count": 0})


def copilot_turn(at, session, cwd, text):
    ts = lambda d: d.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S.000Z")
    emit("copilot-cli", at, {"hook_event_name": "UserPromptSubmit", "session_id": session, "timestamp": ts(at), "cwd": cwd, "prompt": text})
    t = at + timedelta(seconds=random.randint(5, 30))
    emit("copilot-cli", t, {"hook_event_name": "PostToolUse", "session_id": session, "timestamp": ts(t), "cwd": cwd, "tool_name": "bash",
                            "tool_input": {"command": "ls src"}, "tool_result": {"result_type": "success", "text_result_for_llm": "ok"}})
    t += timedelta(seconds=10)
    emit("copilot-cli", t, {"hook_event_name": "Stop", "session_id": session, "timestamp": ts(t), "cwd": cwd,
                            "stop_reason": "end_turn", "stop_hook_active": False})


now = datetime.now().astimezone().replace(second=0, microsecond=0)
for day in range(28, -1, -1):
    date = now - timedelta(days=day)
    weekend = date.weekday() >= 5
    if weekend and random.random() < 0.6:
        continue
    for _ in range(random.randint(1, 2 if weekend else 4)):
        start = date.replace(hour=random.choice([9, 10, 11, 14, 15, 16, 17, 21]), minute=random.randint(0, 59))
        if start > now - timedelta(minutes=30):
            start = now - timedelta(hours=random.randint(1, 3))
        cwd = random.choice(list(PROJECTS))
        tool = weighted(TOOLS)
        session = str(uuid.uuid4())
        entry, model = weighted(ENTRYPOINTS), weighted(CLAUDE_MODELS)
        t = start
        for i in range(random.randint(2, 7)):
            text = random.choice(SLASH) if tool == "claude-code" and random.random() < 0.1 else random.choice(PROJECTS[cwd])
            {"claude-code": lambda: claude_turn(t, session, cwd, text, entry, model),
             "codex": lambda: codex_turn(t, session, cwd, text),
             "cursor": lambda: cursor_turn(t, session, cwd, text),
             "copilot-cli": lambda: copilot_turn(t, session, cwd, text)}[tool]()
            t += timedelta(minutes=random.randint(4, 25))
for f in files.values():
    f.close()
print({k: sum(1 for _ in open(spool / f"{k}.jsonl")) for k in files})
