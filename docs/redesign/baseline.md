# Baseline: today's app, before the redesign

The starting line for [docs/adr/0024-dashboard-redesign-v3.md](../adr/0024-dashboard-redesign-v3.md) and ticket 00. Every "It worked if" and every checkpoint compares with a number here. Each row names the command or the steps that produced it, so it can be measured again: add a column per checkpoint (M1–M6) and fill it with the same command.

- **Measured:** 2026-11-16, commit `a9f0971` (`git rev-parse --short HEAD`), on the author's Mac (Darwin 25.6, Apple silicon), working tree with no code changes (only the two redesign pages and these docs were staged or new). At 16:34, after the quality run had finished, another agent staged web changes (panel states) in the same tree; they are not in any number here.
- **Machine, data:** timings move a few percent between runs and depend on the machine, so compare on the same one. Benchmarks and Lighthouse run on **synthetic data** (the load benchmark's 2,000 generated turns; `tools/seed-demo.py`'s 222 prompts in 53 sessions), never on the user's real database. The screen numbers are on that demo data too.
- **How the quality numbers were produced:** `just quality all --no-open --force` (every check and every measurement, rebuilt, no browser opened; 15 of 15 reports built in 9m49s), then read from the report files `target/quality/data/<area>/<report>.js` (git-ignored; the same numbers are on the dashboard, `just quality --view`). One report alone: `just quality <area>/<report> --no-open --force`, for example `just quality web/lighthouse --no-open --force`.
- **Measured later the same day, still at `a9f0971`** (a clean checkout of the commit, so no later change is in them): the desktop app ([§1](#1-code-quality-just-quality), with the window visible and hidden) and the project-story read ([§2](#2-server-reads-just-perf)). The Prompts row of Lighthouse was measured again once the page stopped failing on the demo data ([§1](#1-code-quality-just-quality)).
- **Not taken:** the two stopwatch numbers. See [Two numbers from you](#two-numbers-from-you-with-a-stopwatch).

## 1. Code quality (`just quality`)

| Number | Baseline | Command or steps (report) |
|---|---|---|
| Architecture modularity score | **90 / 100**; 0 rule findings; propagation cost 27%; 10 crates, 91 modules; 0% of code in files over 400 lines | `just quality backend/architecture --no-open --force` (also `just arch`, which fails on any finding) |
| Rust tests | 1,909 passed, 0 failed, 107 test binaries | `just quality backend/tests --no-open --force` |
| Rust line coverage, workspace | **88%** (11,387 of 12,985 lines) | same (`just cov` underneath; excludes `tests/`, `examples/`, `main.rs`) |
| Coverage: gitprompt-capture | 91% (1,278 / 1,398) | same |
| Coverage: gitprompt-cli | 10% (82 / 859) | same (the end-to-end tests run the binary as a separate process) |
| Coverage: gitprompt-core | 95% (1,220 / 1,288) | same |
| Coverage: gitprompt-dashboard | 93% (38 / 41) | same |
| Coverage: gitprompt-desktop | 23% (64 / 276) | same |
| Coverage: gitprompt-export | 98% (1,687 / 1,714) | same |
| Coverage: gitprompt-insights | 98% (2,379 / 2,427) | same |
| Coverage: gitprompt-server | 94% (2,297 / 2,447) | same |
| Coverage: gitprompt-store | 94% (1,858 / 1,972) | same |
| Coverage: gitprompt-tauri | 86% (484 / 563) | same |
| Web tests | 632 passed, 0 failed, 44 files | `just quality web/tests --no-open --force` |
| Web line coverage | **98%** (1,098 of 1,120 lines); branches 95%, functions 96% | same (V8 coverage of `src/lib`) |
| Web lint, types, format | 0 findings | `just quality web/lint --no-open --force` |
| Complexity over a limit | Rust 0, web 0 | `just quality complexity --no-open --force` |
| Dependencies | 0 findings | `just quality backend/dependencies --no-open --force` |
| Bundle, JavaScript gzipped | **212 KB** (216,702 bytes), 122 files; largest chunk 23 KB (23,935 bytes) | `just quality web/bundle --no-open --force` |
| Bundle, all files gzipped | 541 KB (553,965 bytes), 198 files (993 KB raw); fonts 281 KB, CSS 36 KB | same |
| First load, cold, over the wire | 509 KB transferred, 128 requests; relaunch (warm cache) 125 KB, 127 requests | `just quality web/responsiveness --no-open --force` ("Loads" table). Headless Chrome on the release daemon with demo data. This is the number "first load within 10 KB of the baseline" compares with |
| First paint / data shown, cold | 168 ms / 167 ms (relaunch: 112 ms / 143 ms) | same |
| Page switch (visited before) | Overview 83 ms, Sessions 66 ms, Connectors 66 ms, Prompts 65 ms, Projects 49 ms | same |
| Dashboard idle (15 s, Overview) | main thread 0.02% busy, 0 requests sent, JS heap 7.1 MB, DOM 8,724 nodes | same |
| Lighthouse, average of 4 pages | Performance **96**, Accessibility **98**, Best practices **99**, SEO **91** | `just quality web/lighthouse --no-open --force` (lighthouse@12.8.2 via `pnpm dlx`, desktop preset, headless Chrome, demo data; needs network the first time) |
| Lighthouse: Overview `/` | Perf 94 · A11y 97 · BP 100 · SEO 91 · LCP 1.46 s · TBT 2 ms · CLS 0.004 | same |
| Lighthouse: Prompts `/prompts` | Perf 96 · A11y 100 · BP 100 · SEO 91 · LCP 1.19 s · TBT 0 ms · CLS 0.004, measured again with the page fixed (the day groups, see the finding under [Screen](#4-the-screen-1280800)); the other three pages scored the same as before within a point. At `a9f0971` the page showed its error state on this data and scored Perf 96 · A11y 100 · BP 96 (the console error) · SEO 91 · LCP 1.23 s | `just quality web/lighthouse --no-open --force` |
| Lighthouse: Sessions `/sessions` | Perf 97 · A11y 96 · BP 100 · SEO 91 · LCP 1.15 s · TBT 0 ms · CLS 0.012 | same |
| Lighthouse: Sources `/sources` | Perf 97 · A11y 100 · BP 100 · SEO 91 · LCP 1.02 s · TBT 0 ms · CLS 0.000 | same |
| Daemon cold start (fresh install, until it answers) | **59 ms** (median of 3; 51–92 ms) | `just quality backend/startup --no-open --force`. Release `gitprompt serve`, performance profile, alone on the machine |
| Daemon relaunch, until it answers | **28 ms** median (25–107 ms) | same |
| Daemon first start with 2,443 spooled payloads | answers 77 ms; spool drained 686 ms | same |
| Daemon idle | **0.00% CPU**, 10.3 MB resident (34.7 MB right after start) | same (13 s idle after relaunch) |
| Desktop app, cold launch until the data shows | **1.44 s** median (window built 748 ms) | `just quality backend/desktop --no-open --force` on a clean checkout of `a9f0971` (release build, performance profile, demo data; opens the app's window for about two minutes: keep it visible and the screen awake, a covered window stops painting and the run fails with "no frame painted") |
| Desktop app, relaunch until the data shows | **1.25 s** median | same |
| Desktop app, page switch | **85 ms** median; first visit to a page 97 ms; worst window resize 59 ms | same |
| Desktop app idle, window visible | **0.3% CPU**, 146.1 MB, 1,278 DOM nodes | same (15 s idle after the last relaunch, the app and its webview processes) |
| Desktop app idle, window **hidden** | **0.07% CPU**, 83.0 MB (app 0.02%, webview processes 0.05%); visible in the same run: 0.65%, 109.8 MB | Release `gitprompt-desktop` of `a9f0971` on fresh demo data (`python3 tools/seed-demo.py <home>`, then the app with `GITPROMPT_HOME=<home>`), 25 s to start and drain, window hidden through System Events (`set visible of (first process whose unix id is <pid>) to false`), 15 s to settle, then 60 s of `familyIdle` (`tools/quality/lib/app-processes.mjs`, the app and its webview processes) |

## 2. Server reads (`just perf`)

`just perf` (= `cargo run --release -p gitprompt-server --example load -- --check`, performance profile): 2,000 generated turns × 22 events = 44,000 events into a 157 MB database, reads in-process through the axum router. Budget: every read under 300 ms at p95. **Result: every budget met, exit 0.** Run on 2026-11-16, commit `a9f0971`, run alone.

Ingest: 3,935 events/s (0.25 ms per event); index build 2,104 ms; peak memory 82 MiB under load; database writer: 0 entries kept over 500 ms.

| Read | Path | p50 | **p95 alone** | max | p95 under load |
|---|---|---|---|---|---|
| prompts (limit=50) | `/api/prompts?limit=50` | 26.2 ms | **39.9 ms** | 40.7 ms | 62.8 ms |
| prompt detail | `/api/prompts/1975` | 5.25 ms | **5.9 ms** | 6.5 ms | 25.7 ms |
| stats by day | `/api/stats?group_by=day` | 7.21 ms | **8.4 ms** | 8.6 ms | 26.9 ms |
| stats by model | `/api/stats?group_by=model` | 3.36 ms | **4.4 ms** | 6.5 ms | not run under load |
| stats by tool | `/api/stats?group_by=tool` | 29.3 ms | **32.7 ms** | 40.1 ms | not run under load |
| sources | `/api/sources` | 151 ms | **237.5 ms** | 248 ms | 215 ms |
| data | `/api/data` | 0.04 ms | **0.08 ms** | 0.08 ms | not run under load |
| session story | `/api/stories/session/bench-session-00078` | 8.22 ms | **10.4 ms** | 10.8 ms | not run under load |
| project story | `/api/stories/project/L3dvcmsvcHJvamVjdC02` | 33.1 ms | **34.7 ms** | 42.9 ms | not run under load |
| search | `/api/search?q=retry` | 11.3 ms | **17.0 ms** | 28.5 ms | not run under load |
| fields | `/api/fields` | 92.4 ms | **255.6 ms** | 331.6 ms | not run under load |

Variance: the `sources` and `fields` reads are the slow ones and move a lot between runs. The same benchmark inside `just quality all` (other work finishing nearby) gave `sources` 167 ms alone and 270 ms under load, and `fields` 97 ms alone. Compare those two with a margin (re-run twice and take the lower of the worst); the others are stable. The project-story read, the one ticket 46 compares with ("within 20% of the baseline"), was added to the benchmark afterwards (`examples/load/bench.rs`: the project of the sampled session). It was measured by running that benchmark against the server code of `a9f0971` (a clean checkout with only `bench.rs` replaced) with `just perf`, every budget met. The project id is `/work/project-6` in base64, one of the benchmark's generated projects.

## 3. Time dependencies (`just timing --list`)

| Number | Baseline | Command |
|---|---|---|
| Time dependencies, total | **129** (backend 110, web 19) | `just timing --list` (or `node tools/timing/cli.mjs --list`) |
| Blocking (accepted in `tools/timing/ledger.json`) | **34**; none new | `just timing` ("129 time dependencies (34 blocking, all accepted; 95 reported), none new.") |
| Reported only (timers, timeouts, stopwatches) | 95 | same |

## 4. The screen, 1280×800

Rendered in headless Chrome (viewport 1280×800, scale 1) against the release daemon on the demo data (222 prompts, 53 sessions, last 7 days default range; `python3 tools/seed-demo.py <folder>`, then `GITPROMPT_HOME=<folder> GITPROMPT_PORT=7979 backend/target/release/gitprompt serve`). A script drove the page through the DevTools protocol and read the layout (`tools/quality/lib/chrome.mjs` is the helper; the throwaway script is not committed). The real-data figures in `docs/redesign/index.html` §01 (3,600 px, about 150 px per session) come from the author's own database and differ a little: re-measure on the real data (steps below) for the number that matters.

| Number | Baseline | How |
|---|---|---|
| Overview, page height | **3,843 px** (about 4.8 screens of 800 px), after every lazy panel has loaded | Open `/` at 1280×800, scroll to the bottom so the panels that load when seen all load, read `document.documentElement.scrollHeight` |
| Overview, panels | **14** question panels under 4 section headings, **plus** the key-figures row of 5 tiles and the filter bar: 15 parts, the ones usage counts track (ticket 01). In code: `web/src/routes/+page.svelte` renders `KeyFigures`, `ActivityPanel`, `RhythmPanel`, `SessionsPanel`, then 11 lazy panels (Origin, Commands, Writing, Reuse, Models, Spend, Performance, Agent, Delegation, Outcomes, Starred). A first count said 16: `main .panel .title` also matches the title of an empty state inside a panel (two on this data, "No lines of code and commits yet" and "No starred prompts here") | Count the `<Panel` tags in `web/src/lib/insights/panels/` (one per file) and the `KeyFigures` row |
| Overview, first screen | to measure by eye: what is visible without scrolling at 1280×800 (key figures and the start of Activity, by the layout) | Open `/`, no scrolling, take a screenshot |
| Prompts, filter rows before the first prompt | **6** rows of controls (search with tag and Search button; five selects: tool, app, model, outcome, project; origin chips; toggles: Starred, Failed, Changed files, Ran commands, Possible secret) | Open `/prompts?range=24h`, count the distinct vertical positions of controls inside `form[role=search]` |
| Prompts, first prompt | the filter block spans y 183 to 409 px (226 px); the first prompt starts at **y = 496 px**, 62% down an 800 px screen | same page: `getBoundingClientRect().top` of the form and of the first list item |
| Prompts, reader before a selection | empty: the "Select a prompt" panel (on this data it was shown next to the list) | open `/prompts` with no `id` |
| Sessions, per screen | **2 whole cards and 3 partly visible** (cards are 193–205 px tall, the first starts at y = 237 px); 10 cards load | Open `/sessions`, count `main article` whose box lies inside, or touches, the 800 px viewport |
| Sessions, per screen on real data | to measure | the same steps on your own database (`just serve`) |

**Finding while measuring, since fixed.** On this demo data `/prompts` showed "Couldn't show the prompts" with the Svelte error `each_key_duplicate` for the default range and for 7d, 30d and All; it rendered for `?range=24h`. The cause: the list comes newest id first, but `PromptList.svelte` grouped prompts by the day they arrived only where they sat next to each other, so a day split in two wherever ids and arrival disagree (imported history, or the demo seed) and gave two groups the same key. `lib/prompts/days.ts` now keeps each day one group, newest day first, with tests. The Prompts layout numbers above come from `?range=24h`, measured before the fix; the Prompts Lighthouse row was measured again after it.

## Not measured

| Number | Why not | Command or steps |
|---|---|---|
| Overview, Prompts and Sessions pixel numbers on **your** database | The numbers above are on demo data, which every checkpoint can rebuild; real data is only on the author's machine | See the "How" column of the screen table, with `just serve` (or the desktop app) and your own data |
| Seconds to find a prompt you remember, and to get back into yesterday's session | Not taken at the baseline, by decision (2026-11-16). On the baseline commit the Prompts page also failed on most ranges (see the finding above), so a timing of it would have measured the bug | Taken first at checkpoint M1, with the steps below; M1's numbers are the reference for the later checkpoints. The "It worked if" of finding and resuming compares with M1, not with the baseline |

## Two numbers from you, with a stopwatch

The two the redesign must beat. Not taken at the baseline (above): take them at M1 and at every checkpoint after it, on your real data, in the same way each time.

**Seconds to find a prompt you remember.** Pick a prompt you wrote in the last two weeks and remember roughly (a topic, not the exact words). Open the dashboard on its Overview page. Start the stopwatch, find that prompt using whatever you like in the dashboard (search, filters, lists, ⌘K), stop when the prompt's own detail is open and you can see its text. Go back to the Overview. Five tries with five different prompts; record the median.

**Seconds to get back into yesterday's session.** Pick a session from yesterday (or the last working day). Start on the Overview. Start the stopwatch, find that session, and stop when you could continue it: its resume command is copied, or, where a tool has none, its page is open with its last turn on screen. Five tries with five different sessions; record the median.

| Checkpoint | Commit | Find a prompt, median s | Resume a session, median s |
|---|---|---|---|
| M1 | | | |

## Measuring again at a checkpoint

1. `git rev-parse --short HEAD`, and note whether the tree is clean.
2. `just quality all --no-open --force` (about ten minutes; close other work first, the measurements run alone but share the machine). Read the numbers from the dashboard (`just quality --view`) or `target/quality/data/`.
3. `just perf` and `just timing --list`.
4. The screen numbers: the same steps as above on the same data.
5. The stopwatch numbers (from M1), then the desktop rows: `just quality backend/desktop --no-open --force` with the window kept visible, and the hidden-window steps in §1.
