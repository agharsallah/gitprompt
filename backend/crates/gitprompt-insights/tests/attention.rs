//! The attention feed: which sessions to pick up, in which order, and what is worth a look.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    unreachable_pub,
    reason = "integration test: a panic is a test failure, and helpers are crate-private anyway"
)]

use jiff::{SignedDuration, Timestamp};
use pretty_assertions::assert_eq;

use gitprompt_insights::attention::{
    Action, Inputs, Item, Kind, PICK_UP_WITHIN, Presence, RUNNING_WINDOW, RepeatedText, SOURCES, SessionState, Source, attention,
    attention_from, command_name, pick_up,
};
use gitprompt_insights::target::Target;
use gitprompt_store::filter::Outcome;

const NOW: &str = "2026-11-18T09:00:00Z";

fn now() -> Timestamp {
    NOW.parse().unwrap()
}

/// A session whose last event was `ago` before [`NOW`], whose latest turn ended as `ended`.
fn session(id: &str, ago: SignedDuration, ended: Outcome) -> SessionState {
    let at = now().checked_sub(ago).unwrap();
    SessionState { id: id.into(), last_event_at: Some(at.to_string()), ended: Some(ended), ..SessionState::default() }
}

fn ids(sessions: &[SessionState]) -> Vec<(String, Presence)> {
    pick_up(sessions, now()).into_iter().map(|row| (row.session, row.presence)).collect()
}

const MIN: SignedDuration = SignedDuration::from_mins(1);
const HOUR: SignedDuration = SignedDuration::from_hours(1);

// --- pick up ---

#[test]
fn running_sessions_come_first_then_the_ones_left_newest_first() {
    let sessions = [
        session("left-old", 20 * HOUR, Outcome::Done),
        session("running", 3 * MIN, Outcome::Running),
        session("left-new", 2 * HOUR, Outcome::Failed),
    ];
    assert_eq!(
        ids(&sessions),
        [("running".into(), Presence::Running), ("left-new".into(), Presence::Left), ("left-old".into(), Presence::Left)]
    );
}

#[test]
fn a_turn_with_an_end_or_an_old_last_event_is_left() {
    let sessions = [session("waiting", MIN, Outcome::Done), session("quiet", RUNNING_WINDOW + MIN, Outcome::Open)];
    assert!(ids(&sessions).iter().all(|(_, presence)| *presence == Presence::Left));
}

#[test]
fn a_session_flips_from_running_to_left_as_time_passes() {
    let session = session("s", 9 * MIN, Outcome::Running);
    assert_eq!(pick_up(std::slice::from_ref(&session), now())[0].presence, Presence::Running);
    let later = now().checked_add(2 * MIN).unwrap();
    assert_eq!(pick_up(&[session], later)[0].presence, Presence::Left);
}

#[test]
fn at_most_three_and_none_left_over_a_week_ago() {
    let sessions: Vec<SessionState> = (1..=5).map(|h| session(&format!("s{h}"), h * HOUR, Outcome::Done)).collect();
    assert_eq!(pick_up(&sessions, now()).len(), 3);
    let stale = session("stale", PICK_UP_WITHIN + MIN, Outcome::Done);
    let unknown = SessionState { id: "no time".into(), ..SessionState::default() };
    assert_eq!(pick_up(&[stale, unknown], now()), []);
}

#[test]
fn a_row_carries_how_it_ended_and_its_resume_command() {
    let mut s = session("s", 2 * HOUR, Outcome::Interrupted);
    s.name = Some("Rust switch".into());
    s.project = Some("acme/app".into());
    s.latest_prompt = Some(42);
    let row = &pick_up(&[s], now())[0];
    assert_eq!(
        (row.ended, row.name.as_deref(), row.project.as_deref(), row.latest_prompt),
        (Outcome::Interrupted, Some("Rust switch"), Some("acme/app"), Some(42))
    );
}

// --- worth a look ---

fn repeated(text: &str, prompts: i64, sessions: i64) -> RepeatedText {
    RepeatedText { text: text.into(), prompts, sessions }
}

#[test]
fn a_prompt_sent_often_in_several_sessions_becomes_a_command_to_copy() {
    let inputs = Inputs { repeated: vec![repeated("  run just verify and fix\n", 14, 6)], ..Inputs::default() };
    let feed = attention(&inputs, now());
    assert_eq!(
        feed.worth_a_look,
        [Item {
            kind: Kind::RepeatedPrompt,
            text: "“run just verify and fix” sent 14×".into(),
            action: Action::CopyCommand { name: "run-just-verify-and".into(), text: "run just verify and fix".into() },
            target: Target::Prompts { q: "run just verify and fix".into() },
        }]
    );
    assert!(!feed.nothing_needs_you);
}

#[test]
fn short_rare_single_session_and_slash_texts_are_not_commands() {
    let inputs = Inputs {
        repeated: vec![
            repeated("continue", 40, 9),
            repeated("run just verify and fix", 4, 4),
            repeated("run just verify and fix", 9, 1),
            repeated("/review the parser please", 9, 4),
        ],
        ..Inputs::default()
    };
    assert_eq!(attention(&inputs, now()).worth_a_look, []);
}

#[test]
fn a_long_text_is_quoted_on_one_line_cut_on_a_word() {
    let text = "please read the whole architecture document\nand then explain every rule in it to me";
    let item = &attention(&Inputs { repeated: vec![repeated(text, 5, 2)], ..Inputs::default() }, now()).worth_a_look[0];
    assert_eq!(item.text, "“please read the whole architecture document and…” sent 5×");
}

#[test]
fn command_names_are_short_lowercase_words() {
    assert_eq!(command_name("Run `just verify` & fix it, NOW"), "run-just-verify-fix");
    assert_eq!(command_name("¿¡ — !?"), "repeat");
}

/// A kind added later, as a test stand-in: one function, one line in the list.
#[expect(clippy::unnecessary_wraps, reason = "a source's signature: it may find nothing")]
fn fake(_: &Inputs) -> Option<Item> {
    Some(Item {
        kind: Kind::RepeatedPrompt,
        text: "fake".into(),
        action: Action::CopyCommand { name: "fake".into(), text: "fake".into() },
        target: Target::Overview,
    })
}

#[test]
fn a_new_source_needs_only_its_function_and_a_place_in_the_list() {
    let inputs = Inputs { repeated: vec![repeated("run just verify and fix", 14, 6)], ..Inputs::default() };
    let first: Vec<Source> = [&[fake as Source], SOURCES].concat();
    let texts = |sources: &[Source]| attention_from(&inputs, now(), sources).worth_a_look.into_iter().map(|i| i.text).collect::<Vec<_>>();
    assert_eq!(texts(&first), ["fake", "“run just verify and fix” sent 14×"], "the list's order is the order on screen");
    assert_eq!(texts(&[fake, fake, fake, fake]).len(), 3, "at most three");
}

#[test]
fn an_empty_feed_says_nothing_needs_you() {
    let feed = attention(&Inputs::default(), now());
    assert!(feed.nothing_needs_you);
    let busy = Inputs { sessions: vec![session("s", HOUR, Outcome::Done)], ..Inputs::default() };
    assert!(!attention(&busy, now()).nothing_needs_you);
}

#[test]
fn the_costliest_redone_turn_comes_before_a_repeated_prompt_and_opens_the_turn() {
    use gitprompt_insights::redone::{CostliestTurn, Reason, RedoneWork};
    let costliest = |cost_usd| CostliestTurn { id: 7, reason: Reason::Failed, agent_time_s: 4320.0, cost_usd, text: None };
    let inputs = |cost| Inputs {
        repeated: vec![repeated("run just verify and fix", 14, 6)],
        redone: Some(RedoneWork { costliest: Some(costliest(cost)), ..RedoneWork::default() }),
        ..Inputs::default()
    };
    let feed = attention(&inputs(Some(9.2)), now());
    assert_eq!(feed.worth_a_look[0].text, "Costliest redone turn: $9.20");
    assert_eq!(feed.worth_a_look[0].action, Action::OpenTurn { prompt_id: 7 });
    assert_eq!(feed.worth_a_look[0].target, Target::Prompt { id: 7 });
    assert_eq!(feed.worth_a_look[1].kind, Kind::RepeatedPrompt);
    assert_eq!(attention(&inputs(None), now()).worth_a_look[0].text, "Costliest redone turn: 1 h 12 min of agent time");
}
