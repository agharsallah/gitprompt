//! The hook forwarder: it delivers the payload with the token, queues it when GitPrompt is down, and
//! never disturbs the tool (only the configured reply on stdout). Port of `test_hook_forwarder.py`,
//! with a fake transport instead of curl and a socket.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_pass_by_value,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, fixtures use small known values, and helpers are crate-private anyway"
)]

mod common;

use std::cell::RefCell;
use std::time::Duration;

use common::*;
use gitprompt_capture::forwarder::{Forwarded, Forwarder, HookRequest, Outcome, Transport, Unreachable, build_request, reply};
use gitprompt_capture::spool::{Spool, Spooled};
use pretty_assertions::assert_eq;
use serde_json::json;

/// The receiver as the forwarder sees it: answers with a status, or can't be reached; records every
/// request that crossed the boundary.
struct FakeReceiver {
    answer: Result<u16, Unreachable>,
    requests: RefCell<Vec<HookRequest>>,
}

impl FakeReceiver {
    fn answering(status: u16) -> Self {
        FakeReceiver { answer: Ok(status), requests: RefCell::new(vec![]) }
    }
    fn down() -> Self {
        FakeReceiver { answer: Err(Unreachable("connection refused".into())), requests: RefCell::new(vec![]) }
    }
}

impl Transport for FakeReceiver {
    fn send(&self, request: &HookRequest) -> Result<u16, Unreachable> {
        self.requests.borrow_mut().push(request.clone());
        self.answer.clone()
    }
}

const PAYLOAD: &[u8] = br#"{"hook_event_name": "Stop", "session_id": "s1"}"#;

fn forwarder<'a>(env: &TestEnv, receiver: &'a FakeReceiver) -> Forwarder<'a> {
    Forwarder {
        receiver: "http://127.0.0.1:7878".into(),
        token: "t0k".into(),
        spool: Spool::new(env.path("gitprompt/spool")),
        transport: receiver,
    }
}

#[test]
fn the_request_posts_the_payload_unchanged_to_the_sources_ingest_url_with_the_token() {
    let request = build_request("http://127.0.0.1:7878", "codex", "t0k", PAYLOAD);
    assert_eq!(
        request,
        HookRequest {
            method: "POST",
            url: "http://127.0.0.1:7878/ingest/codex".into(),
            headers: vec![("Content-Type".into(), "application/json".into()), ("Authorization".into(), "Bearer t0k".into())],
            body: PAYLOAD.to_vec(),
            timeout: Duration::from_secs(2),
        }
    );
}

#[test]
fn a_delivered_payload_is_not_queued_and_nothing_is_printed() {
    let env = TestEnv::new();
    let receiver = FakeReceiver::answering(204);

    let result = forwarder(&env, &receiver).forward("claude-code", PAYLOAD, None);

    assert_eq!(result, Forwarded { stdout: String::new(), outcome: Outcome::Delivered { status: 204 } });
    assert_eq!(receiver.requests.borrow()[0], build_request("http://127.0.0.1:7878", "claude-code", "t0k", PAYLOAD));
    assert_eq!(Spool::new(env.path("gitprompt/spool")).sources().unwrap(), Vec::<String>::new());
}

#[test]
fn a_payload_is_queued_when_gitprompt_is_down() {
    let env = TestEnv::new();
    let receiver = FakeReceiver::down();

    let result = forwarder(&env, &receiver).forward("claude-code", PAYLOAD, None);

    assert_eq!(result, Forwarded { stdout: String::new(), outcome: Outcome::Spooled });
    let queued = Spool::new(env.path("gitprompt/spool")).pending("claude-code").unwrap();
    let spooled: Vec<Spooled> = queued.iter().map(|l| Spooled::parse(l).unwrap()).collect();
    assert_eq!(spooled.iter().map(|s| s.payload.clone()).collect::<Vec<_>>(), vec![json!({"hook_event_name": "Stop", "session_id": "s1"})]);
    assert!(spooled[0].spooled_at.is_some(), "queued with the time it was spooled");
}

#[test]
fn an_error_answer_is_not_queued_because_replaying_it_would_fail_the_same_way() {
    let env = TestEnv::new();
    let receiver = FakeReceiver::answering(500);

    let result = forwarder(&env, &receiver).forward("claude-code", PAYLOAD, None);

    assert_eq!(result.outcome, Outcome::Rejected { status: 500 });
    assert_eq!(receiver.requests.borrow().len(), 1);
    assert_eq!(Spool::new(env.path("gitprompt/spool")).sources().unwrap(), Vec::<String>::new());
}

#[test]
fn a_stale_token_is_rejected_and_not_queued() {
    let env = TestEnv::new();
    let receiver = FakeReceiver::answering(401);
    assert_eq!(forwarder(&env, &receiver).forward("claude-code", PAYLOAD, None).outcome, Outcome::Rejected { status: 401 });
}

#[test]
fn a_payload_that_is_not_a_json_object_is_dropped_rather_than_queued() {
    let env = TestEnv::new();
    let receiver = FakeReceiver::down();

    let result = forwarder(&env, &receiver).forward("claude-code", b"not json", None);

    assert!(matches!(result.outcome, Outcome::Dropped { .. }), "{result:?}");
    assert_eq!(Spool::new(env.path("gitprompt/spool")).sources().unwrap(), Vec::<String>::new());
}

#[test]
fn cursors_reply_comes_from_its_spec_by_event_with_a_fallback_for_any_other() {
    let env = TestEnv::new();
    let cursor = env.spec("cursor");
    assert_eq!(reply(&cursor, br#"{"hook_event_name": "beforeSubmitPrompt"}"#).as_deref(), Some(r#"{"continue":true}"#));
    assert_eq!(reply(&cursor, br#"{"hook_event_name": "stop", "status": "aborted"}"#).as_deref(), Some("{}"));
    assert_eq!(reply(&cursor, b"not json").as_deref(), Some("{}"));
}

#[test]
fn a_tool_whose_spec_names_no_reply_gets_nothing_printed() {
    let env = TestEnv::new();
    assert_eq!(reply(&env.spec("claude-code"), PAYLOAD), None);
    let receiver = FakeReceiver::answering(204);
    assert_eq!(forwarder(&env, &receiver).forward_hook(&env.spec("claude-code"), PAYLOAD).stdout, "");
}

#[test]
fn cursors_reply_is_printed_whether_the_payload_was_delivered_or_not() {
    let env = TestEnv::new();
    let cursor = env.spec("cursor");
    let up = FakeReceiver::answering(204);
    let down = FakeReceiver::down();
    let broken = FakeReceiver::answering(500);
    let payload = br#"{"hook_event_name": "beforeSubmitPrompt", "generation_id": "g1"}"#;

    let outputs: Vec<String> = [&up, &down, &broken].iter().map(|r| forwarder(&env, r).forward_hook(&cursor, payload).stdout).collect();

    assert_eq!(outputs, vec![r#"{"continue":true}"#; 3]);
    assert_eq!(up.requests.borrow()[0].url, "http://127.0.0.1:7878/ingest/cursor");
}
