# 0015 — Connectors: tools with layers, and one explicit state per connector

**Status:** accepted (implemented)
**Builds on:** [0008](0008-universal-capture-declarative-sources.md) (declarative source specs), [0011](0011-drop-noise-events.md) (`drop_events`)

## Context
The Sources page showed one card per spec, with a status badge built from four booleans and a separate freshness dot, and the Overview's first-run steps named `claude-code` and `claude-code-otel` in code. Users think of *Claude Code*, not of two specs: hooks and telemetry are two optional layers of one tool. The plan (E2) asks for an explicit, exhaustive state model with one action per state, a heartbeat, and a first run of at most three clicks plus one confirmation; E6 needs the same UI to work for a tool that has only hooks, or no telemetry.

## Decision
1. **Spec fields for layers (optional, additive):** `part_of` (id of the tool's main spec; none = this is the main one), `layer` (short name: "Hooks", "Telemetry") and `summary` (one line: what the layer adds). `load_specs` refuses a `part_of` that does not name a main spec. Grouping is data, so no `if source == ...` and no id suffix tests (`-otel`) remain in the dashboard.
2. **`GET /api/sources` reports raw facts only**, now also: `part_of`, `layer`, `summary`; `hourly` (prompts + events per UTC hour for the last 24 hours, the current hour last); `collects` (documented events the spec keeps), `not_stored` (`drop_events`) and `not_collected` (`not_captured`). `hourly` reads only rows newer than the window: rows are inserted in time order, so the newest older row is found by walking ids backwards (no new index on a large `event` table).
3. **The state is derived in one pure module, `web/src/lib/sources/state.ts`**, not on the server: it needs the viewer's clock (quiet), changes presentation only, and is unit-tested against one fixture per state (`fixtures.ts`), which the Design system page also draws. Nine states, exactly one applies:

| State | When | Primary action |
|---|---|---|
| `not_installed` | not connected, tool not found | Check again |
| `blocked_by_policy` | a `block_if`/`block_if_set` rule matched (connected or not) | Check again |
| `warning` | not connected, a `warn_*` rule matched | Review and connect |
| `available` | not connected, nothing in the way | Review and connect |
| `outdated` | connected, the spec now installs more or other entries | Update connection |
| `drift` | connected, prompts stored without text (`unmapped`) | Re-map stored events |
| `waiting` | connected, nothing received yet | Check again |
| `quiet` | connected, no event for 24 hours | View prompts |
| `receiving` | connected, events arriving | View prompts |

   Disconnect stays a secondary action on every connected state. `waiting` is added to the plan's list: right after connecting, "quiet" would read as a fault while the user hasn't sent anything yet. **Quiet = 24 hours**, the heartbeat's window: gaps of hours are ordinary use, so a shorter threshold would flag every morning; shorter gaps show in the sparkline. Warnings don't make a connected connector `warning`: whether events arrive says more, and the warning is still listed.
4. **First run:** `/sources?connect=<id>` opens that connector's consent review on arrival, and the Overview's steps link there. From an empty Overview: *Connect* (1 click) → review with the diff, focused → *Connect* (the confirmation) → send a prompt in the tool; the page announces the first event.

## Not built
- **"Send test event".** A synthetic event posted by the daemon to its own receiver would only prove that the daemon reaches itself. The failures it could catch (wrong port or token in the installed entries) already show as `outdated`, because the rendered entries embed the receiver URL and token; the failures that matter (the tool reads another config dir, a session started before connecting) are invisible to it. It would also add an ingest path that must never store, against "raw is truth". The real confirmation, the first event from the tool, is announced instead.
- **Cross-layer checks** (hooks receiving while telemetry is silent): useful, but a heuristic; left for when a second tool shows which pairs are meaningful.

## Consequences
+ A new tool's spec chooses its layers; the page, the Settings summary and first-run steps follow without code.
+ Every page shows a connector the same way (icon, label, tone, sentence), never colour alone.
− `hourly` assumes rows arrive in time order; a clock jump backwards can under-count one window until it passes.
− A user spec override must keep `part_of` valid, or loading specs fails (as for any invalid spec today).
