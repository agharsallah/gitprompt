# 0007 — Browser capture via one engine + declarative site adapters

**Status:** superseded by [0008](0008-universal-capture-declarative-sources.md)

## Context
Web UIs (Copilot, Gemini, later ChatGPT/claude.ai) have no prompt export API (or only bulk exports) and change their internals often.

## Decision
- One MV3 extension. The **engine** handles MAIN-world fetch/XHR interception, DOM fallback, isolated-world validation, IndexedDB queue, pairing and per-site state.
- Each site is a **`SiteAdapter`**: URL matches, request URL pattern + body extractor, conversation id extractor, DOM fallback selectors.
- Sites use `optional_host_permissions` requested at enable time. Only the service worker talks to `127.0.0.1`.
- Bulk exports (Google Takeout etc.) go through the generic export importer on the daemon side.

## Consequences
+ A site breakage is an adapter fix with a recorded fixture, and the engine stays stable.
+ The browser enforces consent at the permission level.
− Request interception depends on undocumented endpoints → DOM fallback + drift telemetry (local only).
