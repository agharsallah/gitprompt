# Connectors

How GitPrompt connects to each tool, what every connector state means, which apps it can tell apart, and how to add or fix a connector. For what each vendor documents, see [sources.md](sources.md).

## The Connectors page

The **Connectors** page (`/sources`) shows each tool as one card, with its connectors as steps:

- **Claude Code**: **Hooks** (prompts, tools, turn ends) and the optional **Telemetry** layer (model, tokens, cost).
- **GitHub Copilot CLI**: its hooks (also read by the Copilot agent in VS Code), installed as a file of its own, `~/.copilot/hooks/gitprompt.json`.
- **OpenAI Codex CLI**: its hooks in `$CODEX_HOME/hooks.json` (default `~/.codex/hooks.json`), which Codex runs only once you trust them: after connecting (and after an update or a token rotation), open Codex and type `/hooks`.
- **Cursor**: its hooks in `~/.cursor/hooks.json` (restart Cursor after connecting; your Cursor account email is removed before anything is stored).

Each layer is in exactly one state, shown with an icon and a label (never colour alone), one sentence and the one action it needs now:

| State | Meaning | Action |
|---|---|---|
| Turned off | Turned off in GitPrompt: what it sends is discarded | Turn on |
| Not installed | The tool isn't on this machine | Check again |
| Ready to connect | Installed, not connected | Review and connect |
| Check before connecting | A setting may stop GitPrompt's entries from running (listed) | Review and connect |
| Blocked by policy | A setting GitPrompt won't override turns it off | Check again |
| Update available | GitPrompt now captures more than what is installed | Update connection |
| Payload drift | Prompts arrived without text: the tool's payload changed | Re-map stored events |
| Waiting for the first event | Connected, nothing received yet | Check again |
| Quiet | Connected, no event for a day | View prompts |
| Receiving | Events are arriving | View prompts |

A connected layer also shows its heartbeat (last event and events per hour over the last day), its prompts by origin, and **What this collects**: the events it stores, those it receives but drops, and those it never subscribes to, with the reason, straight from its spec.

Each card also says what the tool can't provide (Copilot hooks have no model, tokens or cost; Codex hooks have the model, and tokens from Codex 0.164 on, but no cost: on an older Codex the pages say which version adds tokens), and **Spec in use** shows the spec file the layer runs on (bundled or your override, with its path and contents). Pages that need a capability a tool lacks say "not available for GitHub Copilot CLI" instead of showing zeros.

**Turn off** keeps a connector's entries in the tool's settings but discards what it sends. Settings › Connectors summarizes the same states, with the spec file and Turn on/off for each layer. `/sources?connect=<id>` opens a connector's review directly (the Overview's first-run steps link there).

**Disconnect** removes only what GitPrompt added. After an upgrade, a connected layer may offer **Update connection** (with the diff first).

## Desktop apps, and where each prompt was typed

Every prompt shows its tool and, when the tool says, the app it was typed in, after a hairline on the same badge: **Claude Code | Desktop app**, **Claude Code | VS Code**, **Claude Code | CLI**. Filter by it with the **App** select on the Overview and the Prompts page. A prompt whose tool doesn't say shows the tool alone: GitPrompt never guesses.

| App | Captured? | How it is told apart |
|---|---|---|
| Claude Desktop, Code tab | Yes, by the Claude Code connector (Desktop runs the hooks in `~/.claude/settings.json`) | Telemetry layer: `service.name` `claude-code-desktop` (or `app.entrypoint` `claude-desktop`). Without telemetry: no app shown |
| Claude Desktop, chat | **No**: no hook, telemetry or local API exists, and GitPrompt never reads the app's own storage | |
| Claude Code CLI, VS Code extension, `claude -p` / Agent SDK | Yes | Telemetry `app.entrypoint`: `cli`, `claude-vscode`, `sdk-*` |
| Codex desktop app, CLI, IDE extension | Yes, by the Codex connector (they share `~/.codex/hooks.json`) | Not possible: no hook field names the client, so Codex prompts show no app |
| Cursor (the app's agent) | Yes, by the Cursor connector | Shown as Cursor; its CLI can't be told apart (and its prompt hooks are unreliable) |

The connector cards list, under **Where your prompts come from**, the apps each connector captures and the ones it can't, with why. A prompt sent from a git worktree (Claude Desktop, Claude Code and the Codex app can run sessions in one) belongs to its main repository's project; the worktree and its branch are kept. A Cursor window with several folders uses the first one as the prompt's folder and keeps the others.

**Check it live:** type one prompt in each app, then run `gitprompt check` (or open **Recent captures** at the top of the Connectors page). For each tool it says whether each layer is connected and lists, per app, the newest prompt: when, project and branch, session, model, how many prompts, and what the app rules read (`app.entrypoint=…`). No row means nothing arrived; "surface unknown" means the tool doesn't say where it was typed.

## Adding or fixing a connector

Connectors are YAML files in `backend/crates/gitprompt-core/specs/` (compiled into the binary). To patch one without a release, copy it to `~/.gitprompt/sources/`, edit it, then use **Re-map stored events** on its card.

| Spec key | What it declares | ADR |
|---|---|---|
| `part_of`, `layer`, `summary` | A layer added to another tool, its short name and one-line summary | [0015](adr/0015-connector-layers-and-states.md) |
| `capabilities`, `join: session` | What the connector yields; `join: session` for a tool that sends no prompt id | [0017](adr/0017-connector-capabilities.md) |
| `assistant`, `after_connect`, `tool_files` | Who answers in the tool, what the user still does once connected, files named inside a text input such as a patch | [0019](adr/0019-codex-connector.md) |
| `usage`, `capability_since` | Events that report model calls and where their tokens are; the tool version that adds a capability | [0020](adr/0020-model-call-usage-and-version-capabilities.md) |
| `surface`, `captures`, `cannot_capture`, `event_names`, `answer`, `discard` | Which app a prompt was typed in; which apps the connector reaches; event-name mapping; where the final answer is; payload keys dropped before storing | [0022](adr/0022-surfaces-and-desktop-apps.md) |

### Resume commands

How to pick one of a tool's sessions up again is connector data, not code: an optional `resume` key in the spec (decision D9, [ADR 0024](adr/0024-dashboard-redesign-v3.md)). **No template, no action**: a tool without one shows nothing, because a wrong command is worse than none.

```yaml
resume:
  command: "claude --resume {session_id}"
  verified: "Claude Code 2.1.326, 2026-11-16: `claude --help` and https://code.claude.com/docs/en/cli-reference"
```

| Key | Meaning |
|---|---|
| `command` | A shell command. Placeholders between braces; any other name, an unclosed or a lone brace makes the spec invalid (`{{` and `}}` write a literal brace) |
| `verified` | What it was checked against: the tool's version, the date and the source. Not read by the code; it says when to check again |

| Placeholder | Value |
|---|---|
| `{session_id}` | The session's id, the `session_id` the tool's hooks send |
| `{cwd}` | The session's working folder: the first `cwd` any event of the session carries, else the prompt's |

Each value is **shell-quoted** (POSIX) when filled in: left as it is if it only has letters, digits and `_ - . / : @ % + = ,`, else in single quotes with every `'` written `'\''`, so `/Users/me/My Project` becomes `'/Users/me/My Project'` and a value can never add a second command. The template itself is yours: quote around placeholders and the result is quoted twice. If the session has no value for a placeholder the template uses (no `cwd`), there is no command rather than one with a hole in it. POSIX shells only; Windows is not supported yet.

The dashboard gets `session.resume` on a prompt's detail (`{command, cwd?}`, left out when there is no command); `cwd` is the folder to `cd` to first (Claude Code looks a session up in the folder it started in).

**Verifying a template for a new tool version** (do not start an interactive session):

1. Run `<tool> --version` and `<tool> --help` (and the subcommand's: `codex resume --help`) and find the exact way to resume by id; read the official CLI reference for the same version.
2. Check the id is the right one: the `session_id` in a stored hook payload (`GET /api/prompts/<id>`, `raw`) must equal what the tool itself calls the session (its transcript file name, its session folder, what its picker lists). A tool whose hooks name a different id than its resume takes gets no template.
3. If a flag, a separator (`--resume=<id>` against `--resume <id>`) or the id kind is not confirmed, leave `resume` out and say why in a comment.
4. Put the version, the date and the source in `verified`, and the command in `backend/crates/gitprompt-core/tests/spec_resume.rs` (`bundled_templates_are_the_verified_ones`).

Checked 2026-11-16: Claude Code 2.1.326 (`claude --resume <id>`, `claude --help` and the CLI reference), Codex CLI 0.167.0 (`codex resume <id>`, `codex resume --help`), GitHub Copilot CLI 1.0.112 (`copilot --resume=<id>`, `copilot --help`). **Cursor has no template**: its CLI resumes with `agent --resume [chatId]`, but nothing confirms that id is the `conversation_id` the IDE's hooks send, and the connector captures the IDE.

A spec file that can't be read is listed at the top of the Connectors page and in Settings with its error; it is skipped, every other connector (and capture) keeps working, and a broken copy of a bundled spec falls back to the bundled one.

A connector is done when its conformance fixtures pass: synthetic payloads from the tool's documented examples in `backend/testdata/fixtures/<id>/`, each with the fields, origin and turn it must map to (replayed by `backend/crates/gitprompt-capture/tests/conformance.rs` and `backend/crates/gitprompt-insights/tests/conformance.rs`; `tests/coverage.rs` there fails on any payload key nobody decided about). Where every other kind of change goes: [extending.md](extending.md).
