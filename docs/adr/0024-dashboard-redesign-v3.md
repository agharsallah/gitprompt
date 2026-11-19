# 0024 — Dashboard redesign v3: what "done" means, and ten decisions

**Status:** accepted, decided 2026-11-16. Changing any answer is a new ADR, not an edit of this one.
**Builds on:** [0012](0012-prompt-index-and-stats.md) (prompt index, stats), [0014](0014-insights-stats.md) (insights dashboard), [0016](0016-prompt-stories.md) (stories), [0018](0018-export.md) (export), [0023](0023-rust-backend.md) (Rust backend). Plan: [docs/redesign/index.html](../redesign/index.html) (decisions in §12), tickets: [docs/redesign/tickets.html](../redesign/tickets.html) (ticket 00 holds the working copy of the table below), starting line: [docs/redesign/baseline.md](../redesign/baseline.md).

## Context
The dashboard's data is rich and accurate, but its pages show it as inventories: an Overview of many panels with no headline, phases without names, tall session cards, filters before content. The redesign leads with four jobs people come back for (find it again, pick it up, explain the code, stay in control) and reads the story at four zoom levels (Overview, Project, Session, Prompt). Ten choices in the review change direction, add a kind of stored data, or touch a permanent non-goal in [PLAN.md](../../PLAN.md) §2, so they are decided here, before the tickets that depend on them. Every recommendation of §12 of the review was accepted.

## Decision
Five decisions are built now, one is settled by a test, four are deferred with a trigger. A deferred decision reopens on the fact named in its "Revisit when", not on a mood.

### Build now

**D1 — What "done" means. Answer: v3.** A step is done when its job works and its "It worked if" has a number. Every existing feature stays reachable, as a constraint.
- *Why:* "Reachable" measures nothing about whether the app helps. The constraint keeps the round safe.
- *Changes:* all tickets; gates G1–G5; the §08 checklist is checked at every checkpoint.
- *Revisit when:* after M6, panels nobody opens get a removal decision.

**D2 — Local usage counts. Answer: yes.** Counts only, from a closed list of event names, in the local database, shown under Settings › Developer, never sent.
- *Why:* Without them every checkpoint is an opinion and every later cut a guess. Nothing leaves the machine, so the non-goal holds.
- *Changes:* ticket 01 (new stored data and an API snapshot: asks first).
- *Revisit when:* never sent is permanent. The list of names grows only with a ticket.

**D3 — Redone work in the headlines. Answer: yes**, as cost and agent time. Never as a count of failures. This reverses the v2 review choice to keep failures out of the headlines.
- *Why:* It is the one figure that changes what you do next. Cost reads as waste to remove; a failure count reads as blame.
- *Changes:* tickets 22, then 25.
- *Revisit when:* at M3, if the figure is wrong on your own week: fix the rule in 33 or drop it from the headline.

**D9 — Resume as connector data. Answer: yes.** A `resume:` template in each spec, verified per tool version. No template, no action shown.
- *Why:* A new tool is a data change, not code. A wrong command is worse than none.
- *Changes:* tickets 07, then 08 and 10.
- *Revisit when:* a tool changes its resume syntax: update the spec and its version check.

**D10 — Navigation order. Answer: far to near:** Overview, Projects, Sessions, Prompts.
- *Why:* The sidebar and the zoom bar then teach the same model once.
- *Changes:* tickets 06, then 34.
- *Revisit when:* not expected.

### Settled by a test

**D6 — Chapters or phases. Answer: run the naming test** on three projects. Chapters if you would keep 8 names in 10 unedited; otherwise phases with dates. Names come from branch names and commit subjects, which a person wrote, before typed words.
- *Why:* Grouping helps only when the names are good, and that is cheap to measure before building.
- *Changes:* ticket 46 is gate G5; its result sets the scope of tickets 47–52.
- *Revisit when:* the test result is final for this round.

### Deferred, with a trigger

**D4 — Open in Terminal. Answer: no. Copy only.** `gitprompt resume` prints the command and never runs it.
- *Why:* It touches the permanent non-goal "automating any tool" in PLAN.md §2. Copy gives most of the value with none of the risk.
- *Changes:* tickets 08 and 10 ship Copy; the rest stays in Later.
- *Revisit when:* at M1, if resume is copied weekly: rule whether one visible command on a click is within the non-goal.

**D5 — Agents reading the history. Answer: CLI now, no MCP server.** `find` and `why` are commands you run.
- *Why:* An agent querying on its own is close to the permanent non-goal "adding context to prompts" in PLAN.md §2. The CLI keeps you in control of what an agent sees.
- *Changes:* tickets 10, 18; MCP stays in Later.
- *Revisit when:* after M2, when `find` and `why` are stable and you paste their output into agents often.

**D7 — Team groundwork. Answer: all of it with teams.** Self-hosted first when that day comes.
- *Why:* A new crate and a team server change the architecture map for a feature nobody uses yet. A hosted service would break "stays on this machine".
- *Changes:* nothing now. No identity columns, no sync crate, `rules.rs` untouched.
- *Revisit when:* a second person asks to share a history.

**D8 — AI providers. Answer: both** a local model and your own API key. Off by default, figures only unless you allow prompt text, a preview before every request.
- *Why:* Local alone is too weak for some users; hosted alone breaks the default promise. Opt-in with a preview keeps both honest.
- *Changes:* nothing now; shape noted in Later.
- *Revisit when:* AI analysis is picked up, after M6.

## Consequences
- **No ticket is parked.** The four deferred answers (D4, D5, D7, D8) were already in Later; no "no" removes a ticket.
- PLAN.md's non-goals stand unchanged. D4 and D5 are the two decisions that touch them ("automating any tool", "adding context to prompts"), and both stay on the safe side until their trigger is met and ruled on in a new ADR.
- Ticket 01 adds a new kind of stored data and an API snapshot, and ticket 22 changes the stats and attention snapshots: both ask first, as before. Nothing here edits `rules.rs`, `docs/extending.md` or any config.
- The baseline in [docs/redesign/baseline.md](../redesign/baseline.md) gives every "It worked if" and every checkpoint (M1–M6) a number to compare with.

## Alternatives considered
- **Keep v2's "done" (every feature reachable) as the goal.** Rejected: it measures nothing about whether the app helps.
- **No local usage counts.** Rejected: judgements about the redesign would be opinion.
- **Open in Terminal now.** Deferred: a click that runs one visible command may be within the rule, but that is a ruling to make at M1, not an assumption.
- **An MCP server now.** Deferred: the CLI gives the same answers and keeps the user in control of what an agent sees.
- **Team groundwork now (identity columns, sync crate).** Deferred: it changes the architecture map for a feature nobody uses yet; a hosted service would break local-first.
- **Group phases into chapters without a test.** Rejected: grouping helps only when the names are good.

## Phase 3 thresholds (ticket 33, 2026-11-19)

Every phase 3 threshold is a named constant or a preference, one place per crate. These are the starting values, before the two weeks of use that tune them; a value changed later is noted here with its reason.

| What | Value | Where |
|---|---|---|
| Worth a look, Pick up rows | 3, 3 | `insights/src/attention/` (`WORTH_A_LOOK`, `PICK_UP`) |
| Running, still to pick up | 10 min, 7 days | `attention/pick_up.rs` (`RUNNING_WINDOW`, `PICK_UP_WITHIN`) |
| A prompt worth a command | 5 sends, 2 sessions, 12 characters | `attention/repeated.rs` |
| Notifications per kind | 3 per hour | `insights/src/notify/` (`LIMIT`) |
| Long turn, quiet connector | 10 min, 30 min (preferences) | `core/src/preferences.rs` |
| Weekly recap | Monday 09:00 (preferences) | `core/src/preferences.rs` |
| Leak scan per text, findings per turn | 256 K characters, 20 | `insights/src/leaks/` |
| Key and password entropy | 3.0 and 2.5 bits per character | `core/src/secrets/plausible.rs` |
| Addresses on a line that make a list | 3 | `core/src/personal/checks.rs` (`LIST_ADDRESSES`) |
| Headline: stands out, floors | ±50%; $5, 1 h, 30 min redone, 10 commits, 1 leak | `web/src/lib/stories/headline.ts` |
| Week strip: day that stands out | 1.5 × the median active day | `web/src/lib/overview/week-strip.ts` |
