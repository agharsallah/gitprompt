//! Shared test data for the export tests: synthetic inputs of every kind with planted secrets,
//! email addresses, account ids, commit authors and home paths (ADR-0018 verification), plus small
//! report builders. Every value is synthetic; nothing touches the network or the real home folder.
#![allow(dead_code, reason = "each test binary uses a different subset of these helpers")]

use std::collections::BTreeSet;

use indexmap::IndexMap;
use jiff::tz::Offset;
use serde_json::json;

use gitprompt_core::spec::Origin;
use gitprompt_export::input::{
    Chapter, Commit, Connector, Diffstat, ExportInput, Facts, Marker, Measures, OverviewExport, OverviewGroups, Phase, PhaseReason,
    ProjectExport, ProjectPrompt, ProjectRef, ProjectStory, PromptItem, PromptsExport, SessionExport, SessionStory, SessionTotals,
    StoryItem, Surface, Tokens,
};
use gitprompt_export::report::{Bar, Block, Chart, ChartKind, Level, Report, Section};
use gitprompt_export::{Context, Export, Kind, Options, Zone};
use gitprompt_store::index::{IndexRow, UsageTotals};
use gitprompt_store::stats::Bucket;
use gitprompt_store::timeline::IndexedPrompt;
use gitprompt_store::{Outcome, PromptFilter};

// --- planted values --------------------------------------------------------------------------------

pub const HOME: &str = "/Users/sam";
pub const REPO: &str = "/Users/sam/code/shop";
pub const OTHER_HOME: &str = "/Users/jdoe";
pub const SECRET: &str = concat!("sk-ant-", "api03-abcdefghijklmnopqrstuvwxyz0123");
pub const AWS_KEY: &str = concat!("AKIA", "QWERTYUIOPASDFGH");
pub const GITHUB_TOKEN: &str = concat!("ghp_", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8");
pub const EMAIL: &str = "sam@example.com";
pub const ACCOUNT_EMAIL: &str = "account.owner@example.org";
pub const ORG_ID: &str = "org-5f1c0ffee";
pub const USER_ID: &str = "u-123456";
pub const ACCOUNT_UUID: &str = "acc-9";
pub const AUTHOR: &str = "Jane Doe";
pub const TITLE: &str = "Wire the shop";

/// What a typed prompt says in every fixture: secrets, an email, another user's home, a repo path.
pub fn planted_text() -> String {
    format!(
        "use {SECRET} and write to {REPO}/src/app.py, then mail {EMAIL}; deploy with {AWS_KEY} and {GITHUB_TOKEN}, notes in {OTHER_HOME}/notes.txt"
    )
}

/// The planted prompt as it reads after redaction with prompt text on.
pub const PLANTED_REDACTED: &str = "use [hidden Anthropic API key] and write to src/app.py, then mail [hidden email]; deploy with [hidden AWS access key] and [hidden GitHub token], notes in ~/notes.txt";

/// Values no export may contain with the default options.
pub fn never_by_default() -> Vec<String> {
    let mut v = never_with_text();
    v.extend([USER_ID, ACCOUNT_UUID, AUTHOR, ORG_ID, "use [hidden", "write to", TITLE, "done, ping"].map(String::from));
    v
}

/// Values no export may contain even with prompt text on (identity and paths still at default).
pub fn never_with_text() -> Vec<String> {
    vec![
        SECRET.into(),
        AWS_KEY.into(),
        GITHUB_TOKEN.into(),
        EMAIL.into(),
        ACCOUNT_EMAIL.into(),
        ORG_ID.into(),
        OTHER_HOME.into(),
        HOME.into(),
    ]
}

/// Values no export may contain whatever the options: the secrets.
pub fn never_ever() -> Vec<String> {
    vec![SECRET.into(), AWS_KEY.into(), GITHUB_TOKEN.into()]
}

/// Fails naming every needle found in `body`.
pub fn assert_absent(body: &str, needles: &[String]) {
    let leaked: Vec<&String> = needles.iter().filter(|n| body.contains(n.as_str())).collect();
    assert!(leaked.is_empty(), "leaked {leaked:?} in:\n{body}");
}

// --- context -------------------------------------------------------------------------------------

pub fn utc() -> Zone {
    Zone::Fixed(Offset::UTC)
}

pub fn offset(hours: i32) -> Zone {
    Zone::Fixed(Offset::from_seconds(hours * 3600).unwrap())
}

/// The test machine: home `/Users/sam`, UTC.
pub fn context() -> Context {
    Context { home: Some(HOME.into()), roots: BTreeSet::new(), zone: utc() }
}

pub fn text_on() -> Options {
    Options { prompt_text: true, ..Options::default() }
}

/// Every option opened: text, absolute paths, identity, raw payloads.
pub fn all_open() -> Options {
    Options { prompt_text: true, relative_paths: false, drop_identity: false, drop_raw: false }
}

// --- facts ---------------------------------------------------------------------------------------

pub fn facts() -> Facts {
    Facts {
        connector: Some(Connector { id: Some("claude-code".into()), name: Some("Claude Code".into()), label: Some("Claude Code".into()) }),
        surface: Some(Surface { id: Some("desktop".into()), name: Some("Desktop app".into()), icon: Some("app-window".into()) }),
        model: Some("claude-haiku".into()),
        permission_mode: Some("default".into()),
        tool_version: Some("2.1.0".into()),
        api_requests: Some(2),
        tokens: Some(Tokens { input: 1000, output: 200, total: 1200, reasoning: 30, ..Tokens::default() }),
        cost_usd: Some(0.5),
    }
}

// --- a session -----------------------------------------------------------------------------------

pub fn diffstat() -> Diffstat {
    Diffstat { commits: 1, files: 1, insertions: 3, deletions: 0 }
}

pub fn commit() -> Commit {
    Commit {
        hash: "abc1234def".into(),
        subject: "Add app".into(),
        author: Some(AUTHOR.into()),
        time: Some("2026-03-02T09:00:30+00:00".into()),
        files: 1,
        insertions: 3,
        deletions: 0,
    }
}

/// The first prompt of the session (as /api/stories/session returns it, synthetic).
pub fn chapter() -> Chapter {
    Chapter {
        id: 7,
        turn: 1,
        received_at: "2026-03-02T09:00:00+00:00".into(),
        origin: "typed".into(),
        text: Some(planted_text()),
        clipped: false,
        slash_command: None,
        branch: Some("main".into()),
        outcome: "done".into(),
        response_s: Some(42.0),
        cost_usd: 0.12,
        tokens: 900,
        tool_calls: 3,
        tool_failures: 0,
        tools: IndexMap::from([("write".to_string(), 2), ("read".to_string(), 1)]),
        files_written: 1,
        files: vec![format!("{REPO}/src/app.py"), format!("{HOME}/.zshrc")],
        subagents: 0,
        git_commits: 1,
        commits: vec![commit()],
        diffstat: Some(diffstat()),
        retry_of: None,
        facts: None,
    }
}

pub fn marker() -> Marker {
    Marker {
        kind: "task_notification".into(),
        id: 8,
        received_at: "2026-03-02T09:02:00+00:00".into(),
        label: None,
        detail: None,
        excerpt: Some(format!("done, ping {EMAIL}, see {HOME}/Desktop/log.txt")),
        clipped: false,
    }
}

pub fn session_story() -> SessionStory {
    SessionStory {
        session: "s-0001-0002".into(),
        title: Some(TITLE.into()),
        source: Some("claude-code".into()),
        connector: None,
        models: IndexMap::new(),
        project: Some(ProjectRef { id: Some("x".into()), repo: Some(REPO.into()), name: Some("shop".into()), git: true }),
        branches: vec!["main".into()],
        started_at: Some("2026-03-02T09:00:00+00:00".into()),
        ended_at: Some("2026-03-02T09:03:00+00:00".into()),
        totals: SessionTotals {
            chapters: 1,
            markers: 1,
            cost_usd: 0.12,
            tool_calls: 3,
            files_written: 1,
            outcomes: IndexMap::from([("done".to_string(), 1)]),
            retries: 0,
            diffstat: Some(diffstat()),
        },
        items: vec![StoryItem::Chapter(chapter()), StoryItem::Markers { items: vec![marker()] }],
    }
}

pub fn environment() -> IndexMap<String, String> {
    IndexMap::from([
        ("app.version".to_string(), "2.1.0".to_string()),
        ("user.email".to_string(), ACCOUNT_EMAIL.to_string()),
        ("organization.id".to_string(), ORG_ID.to_string()),
        ("user.account_uuid".to_string(), ACCOUNT_UUID.to_string()),
        ("user.id".to_string(), USER_ID.to_string()),
    ])
}

pub fn session_export() -> SessionExport {
    SessionExport { generated_at: "2026-03-03T10:00:00+00:00".into(), story: session_story(), environment: environment() }
}

// --- a project -----------------------------------------------------------------------------------

pub fn project_prompt() -> ProjectPrompt {
    ProjectPrompt {
        id: 11,
        received_at: "2026-03-02T09:00:00+00:00".into(),
        session: Some("s1".into()),
        origin: "typed".into(),
        branch: Some("main".into()),
        text: Some(planted_text()),
        clipped: false,
        chars: 120,
        outcome: "done".into(),
        response_s: Some(60.0),
        cost_usd: 0.5,
        tool_calls: 3,
        tool_failures: 0,
        tools: IndexMap::from([("write".to_string(), 2), ("read".to_string(), 1)]),
        files_written: 1,
        git_commits: 1,
        commits: vec![commit()],
        retry_of: None,
        facts: Some(facts()),
    }
}

pub fn measures(prompts: i64, first: &str, last: &str) -> Measures {
    Measures {
        prompts,
        sessions: 1,
        cost_usd: 0.5,
        tool_calls: 3,
        files_written: 1,
        outcomes: IndexMap::from([("done".to_string(), prompts)]),
        diffstat: Some(diffstat()),
        retries: 0,
        median_chars: 120,
        with_mention: 0,
        with_code: 0,
        first_at: Some(first.into()),
        last_at: Some(last.into()),
    }
}

pub fn project_story() -> ProjectStory {
    let second = ProjectPrompt {
        id: 12,
        received_at: "2026-03-02T13:00:00+00:00".into(),
        text: Some("tidy up".into()),
        branch: None,
        commits: vec![],
        tools: IndexMap::new(),
        facts: None,
        cost_usd: 0.0,
        ..project_prompt()
    };
    ProjectStory {
        id: Some("p-shop".into()),
        repo: Some(REPO.into()),
        name: Some("shop".into()),
        git: true,
        system: 2,
        totals: measures(2, "2026-03-02T09:00:00+00:00", "2026-03-02T13:00:00+00:00"),
        phases: vec![
            Phase {
                index: 1,
                reason: PhaseReason::Start,
                gap_s: None,
                branches: vec!["main".into()],
                measures: measures(1, "2026-03-02T09:00:00+00:00", "2026-03-02T09:00:00+00:00"),
                items: vec![project_prompt()],
            },
            Phase {
                index: 2,
                reason: PhaseReason::Gap,
                gap_s: Some(14_400.0),
                branches: vec![],
                measures: measures(1, "2026-03-02T13:00:00+00:00", "2026-03-02T13:00:00+00:00"),
                items: vec![second],
            },
        ],
    }
}

pub fn project_export() -> ProjectExport {
    ProjectExport { generated_at: "2026-03-03T10:00:00+00:00".into(), story: project_story() }
}

// --- the overview --------------------------------------------------------------------------------

pub fn bucket(key: Option<&str>, prompts: i64) -> Bucket {
    Bucket { key: key.map(String::from), prompts, sessions: 1, ..Bucket::default() }
}

pub fn overview_export() -> OverviewExport {
    OverviewExport {
        generated_at: "2026-03-03T10:00:00+00:00".into(),
        filters: PromptFilter {
            since: Some("2026-03-01T00:00:00+00:00".into()),
            origin: vec![Origin::Typed, Origin::SlashCommand],
            ..PromptFilter::default()
        },
        totals: Bucket {
            prompts: 3,
            sessions: 2,
            api_requests: 4.0,
            cost_usd: 1.5,
            input_tokens: 1000.0,
            output_tokens: 500.0,
            tool_calls: 7.0,
            failed: 1.0,
            git_commits: 2.0,
            response_s_p50: Some(42.0),
            ..Bucket::default()
        },
        groups: OverviewGroups {
            day: vec![bucket(Some("2026-03-01"), 1), bucket(Some("2026-03-02"), 2)],
            origin: vec![bucket(Some("typed"), 2), bucket(Some("task_notification"), 1)],
            model: vec![Bucket { cost_usd: 1.5, ..bucket(Some("claude-haiku"), 2) }, bucket(None, 1)],
            tool_kind: vec![Bucket { tool_calls: 5.0, ..bucket(Some("write"), 2) }, Bucket { tool_calls: 2.0, ..bucket(Some("read"), 1) }],
            outcome: vec![bucket(Some("done"), 2), bucket(Some("failed"), 1), bucket(None, 0)],
            repo: vec![
                Bucket { cost_usd: 1.5, tool_calls: 7.0, ..bucket(Some(REPO), 2) },
                Bucket { detail: Some("acme/api".into()), ..bucket(Some("/srv/api"), 1) },
            ],
            command: vec![bucket(Some("/review"), 1), bucket(None, 2)],
        },
    }
}

// --- a prompt selection --------------------------------------------------------------------------

pub fn indexed(id: i64, origin: &str, outcome: Outcome, typed: &str) -> IndexedPrompt {
    IndexedPrompt {
        row: IndexRow {
            id,
            prompt_id: Some(format!("p{id}")),
            source: "claude-code".into(),
            session: Some("s1".into()),
            repo: Some(REPO.into()),
            branch: Some("main".into()),
            title: Some(TITLE.into()),
            received_at: format!("2026-03-02T09:0{id}:00+00:00"),
            day: "2026-03-02".into(),
            hour: 9,
            weekday: 1,
            origin: origin.into(),
            chars: typed.chars().count() as i64,
            typed_text: Some(typed.into()),
            ..IndexRow::default()
        },
        outcome,
    }
}

pub fn raw_payload() -> serde_json::Value {
    json!({
        "hook_event_name": "UserPromptSubmit",
        "prompt": planted_text(),
        "cwd": REPO,
        "user_email": ACCOUNT_EMAIL,
        "transcript_path": format!("{HOME}/.claude/projects/-Users-sam-code-shop/s1.jsonl"),
    })
}

pub fn prompt_item() -> PromptItem {
    let mut prompt = indexed(1, "typed", Outcome::Done, &planted_text());
    prompt.row.usage = UsageTotals { api_requests: 2, cost_usd: 0.5, input_tokens: 1000, output_tokens: 200, ..UsageTotals::default() };
    prompt.row.tool_calls = 2;
    prompt.row.response_s = Some(60.0);
    PromptItem { prompt, facts: Some(facts()), raw: Some(raw_payload()) }
}

pub fn notification_item() -> PromptItem {
    let prompt = indexed(2, "task_notification", Outcome::Open, &format!("done, ping {EMAIL}"));
    PromptItem { prompt, facts: None, raw: None }
}

pub fn prompts_export() -> PromptsExport {
    PromptsExport {
        generated_at: "2026-03-03T10:00:00+00:00".into(),
        filters: PromptFilter { origin: vec![Origin::Typed], ..PromptFilter::default() },
        total: 2,
        prompts: vec![prompt_item(), notification_item()],
    }
}

/// The planted input of a kind.
pub fn planted(kind: Kind) -> ExportInput {
    match kind {
        Kind::Session => ExportInput::Session(session_export()),
        Kind::Project => ExportInput::Project(project_export()),
        Kind::Overview => ExportInput::Overview(overview_export()),
        Kind::Prompts => ExportInput::Prompts(prompts_export()),
    }
}

/// Prepare the planted input of a kind on the test machine.
pub fn prepared(kind: Kind, options: Options) -> Export {
    gitprompt_export::prepare(&planted(kind), &options, &context()).expect("prepare")
}

// --- reports -------------------------------------------------------------------------------------

pub fn bar(label: &str, value: f64) -> Bar {
    Bar { label: label.into(), value, display: format!("{value}") }
}

pub fn chart(id: &str, kind: ChartKind, items: Vec<Bar>) -> Chart {
    Chart { id: id.into(), title: format!("Chart {id}"), kind, items, caption: String::new() }
}

pub fn section(title: &str, blocks: Vec<Block>) -> Section {
    Section { title: title.into(), blocks, subtitle: String::new(), level: Level::Two }
}

/// A report of `kind` with these sections, a subtitle and two notes.
pub fn report(sections: Vec<Section>) -> Report {
    Report {
        kind: Kind::Session,
        title: "A title".into(),
        subtitle: "A subtitle".into(),
        generated_at: "2026-03-03 10:00".into(),
        sections,
        notes: vec!["First note.".into(), "Second note.".into()],
    }
}

pub fn export_of(report: Report, rows: Vec<serde_json::Value>) -> Export {
    Export { report, rows: rows.into_iter().map(|r| r.as_object().expect("row object").clone()).collect(), options: Options::default() }
}
