//! Synthetic hook payloads: Claude Code turns with realistic sizes, deterministic for a seed
//! (hook shapes: <https://code.claude.com/docs/en/hooks>).

use serde_json::{Value, json};

/// A small deterministic generator (xorshift64*), so every run ingests the same data.
pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }
}

const WORDS: &[&str] = &[
    "add",
    "retry",
    "upload",
    "client",
    "refactor",
    "parser",
    "test",
    "cache",
    "invalidate",
    "error",
    "handling",
    "config",
    "migrate",
    "schema",
    "endpoint",
    "pagination",
    "timeout",
    "logging",
    "flaky",
    "fixture",
    "render",
    "chart",
    "session",
    "token",
    "rotate",
];
const TOOLS: &[&str] = &["Read", "Edit", "Bash", "Grep", "Write", "Glob"];
const PROJECTS: usize = 8;
const PROMPTS_PER_SESSION: usize = 25;

/// `min` to `min + spread` bytes of words.
fn text(rng: &mut Rng, min: usize, spread: usize) -> String {
    let bytes = min + rng.below(spread);
    let mut out = String::with_capacity(bytes + 16);
    while out.len() < bytes {
        if !out.is_empty() {
            out.push(if rng.below(12) == 0 { '\n' } else { ' ' });
        }
        out.push_str(rng.pick(WORDS));
    }
    out
}

/// `min` to `min + spread` bytes of source-looking lines.
fn code(rng: &mut Rng, min: usize, spread: usize) -> String {
    let bytes = min + rng.below(spread);
    let mut out = String::with_capacity(bytes + 64);
    let mut line = 1;
    while out.len() < bytes {
        let (a, b) = (rng.pick(WORDS), rng.pick(WORDS));
        out.push_str(&format!("{line:>5}\tfn {a}_{b}(input: &str) -> Result<(), Error> {{ {a}(input)?; Ok(()) }}\n"));
        line += 1;
    }
    out
}

struct Turn {
    session: String,
    prompt_id: String,
    cwd: String,
}

fn base(turn: &Turn, event: &str) -> Value {
    json!({
        "session_id": turn.session,
        "prompt_id": turn.prompt_id,
        "transcript_path": format!("/home/dev/.claude/projects/app/{}.jsonl", turn.session),
        "cwd": turn.cwd,
        "permission_mode": "default",
        "hook_event_name": event,
    })
}

fn merge(mut payload: Value, extra: Value) -> Value {
    if let (Some(target), Value::Object(extra)) = (payload.as_object_mut(), extra) {
        target.extend(extra);
    }
    payload
}

/// One `PostToolUse`, 0.3–6 KB depending on the tool.
fn tool_use(rng: &mut Rng, turn: &Turn, n: usize) -> Value {
    let tool = rng.pick(TOOLS);
    let file = format!("{}/src/{}_{}.rs", turn.cwd, rng.pick(WORDS), rng.pick(WORDS));
    let (input, response) = match tool {
        "Read" => (json!({"file_path": file}), json!({"type": "text", "file": {"filePath": file, "content": code(rng, 1500, 4500)}})),
        "Edit" => {
            let (old, new) = (code(rng, 200, 800), code(rng, 200, 800));
            (json!({"file_path": file, "old_string": old, "new_string": new}), json!({"filePath": file, "success": true}))
        }
        "Write" => (json!({"file_path": file, "content": code(rng, 800, 2500)}), json!({"filePath": file, "success": true})),
        "Bash" => (
            json!({"command": format!("cargo test -p {} --quiet", rng.pick(WORDS)), "description": "Run the tests"}),
            json!({"stdout": text(rng, 300, 3000), "stderr": "", "interrupted": false}),
        ),
        "Grep" => (json!({"pattern": rng.pick(WORDS), "path": turn.cwd}), json!({"mode": "content", "content": code(rng, 200, 1500)})),
        _ => {
            (json!({"pattern": "**/*.rs"}), json!({"filenames": (0..20).map(|i| format!("{}/src/f{i}.rs", turn.cwd)).collect::<Vec<_>>()}))
        }
    };
    merge(
        base(turn, "PostToolUse"),
        json!({"tool_name": tool, "tool_input": input, "tool_response": response, "tool_use_id": format!("{}-t{n}", turn.prompt_id)}),
    )
}

/// The payloads of turn `i`: prompt, `events` tool uses, stop.
pub(crate) fn turn_payloads(rng: &mut Rng, i: usize, events: usize) -> Vec<Value> {
    let session_no = i / PROMPTS_PER_SESSION;
    let turn = Turn {
        session: format!("bench-session-{session_no:05}"),
        prompt_id: format!("bench-prompt-{i:07}"),
        cwd: format!("/work/project-{}", session_no % PROJECTS),
    };
    let mut out = Vec::with_capacity(events + 2);
    let prompt = text(rng, 80, 1200);
    out.push(merge(base(&turn, "UserPromptSubmit"), json!({"prompt": prompt})));
    out.extend((0..events).map(|n| tool_use(rng, &turn, n)));
    let answer = text(rng, 200, 1500);
    out.push(merge(base(&turn, "Stop"), json!({"stop_hook_active": false, "last_assistant_message": answer})));
    out
}
