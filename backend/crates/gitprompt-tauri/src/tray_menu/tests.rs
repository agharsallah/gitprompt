use pretty_assertions::assert_eq;

use gitprompt_core::spec::Resume;
use gitprompt_server::service::RecentPrompt;

use super::*;

fn session(id: &str, presence: PickUpPresence, resume: Option<&str>) -> PickUp {
    PickUp {
        session: id.into(),
        name: None,
        label: Some(format!("work on {id}")),
        project: Some("acme/app".into()),
        presence,
        ended: if presence == PickUpPresence::Running { Outcome::Running } else { Outcome::Done },
        last_event_at: "2026-11-18T12:00:00Z".into(),
        latest_prompt: None,
        resume: resume.map(|cmd| Resume::new(cmd.into(), Some("/r/app"))),
    }
}

fn glance(pick_up: Vec<PickUp>, cost_usd: Option<f64>) -> Glance {
    Glance {
        today: Today { prompts: 12, cost_usd },
        pick_up,
        recent: vec![RecentPrompt { id: 7, text: "add a retry loop\nto the uploader".into() }],
        open_leaks: 0,
    }
}

fn texts(menu: &TrayMenu) -> Vec<String> {
    menu.items
        .iter()
        .map(|item| match item {
            TrayItem::Text(text) => text.clone(),
            TrayItem::Separator => "---".into(),
            TrayItem::Action { label, .. } => format!("[{label}]"),
        })
        .collect()
}

#[test]
fn nothing_running_says_so_and_still_lists_the_last_prompts() {
    let menu = tray_menu(&DaemonStatus::Running, Some(&glance(vec![], Some(3.4))), true);
    assert_eq!(
        texts(&menu),
        [
            "Recording",
            "Today: 12 prompts · $3.40",
            "---",
            "Pick up",
            "No session to pick up",
            "---",
            "Last prompts (click copies)",
            "[add a retry loop to the uploader]"
        ]
    );
    assert_eq!(menu.action("prompt:0"), Some(&TrayAction::Copy("add a retry loop\nto the uploader".into())));
}

#[test]
fn three_running_sessions_each_copy_their_resume_command() {
    let rows = ["a", "b", "c"].map(|id| session(id, PickUpPresence::Running, Some(&format!("claude --resume {id}")))).to_vec();
    let menu = tray_menu(&DaemonStatus::Running, Some(&glance(rows, Some(1.0))), true);
    let labels: Vec<String> = texts(&menu).into_iter().filter(|t| t.starts_with("[Running")).collect();
    assert_eq!(labels, ["a", "b", "c"].map(|id| format!("[Running: work on {id} · acme/app (copy resume)]")));
    assert_eq!(menu.action("session:2"), Some(&TrayAction::Copy("cd /r/app && claude --resume c".into())));
}

#[test]
fn no_cost_data_shows_the_count_alone_never_zero_dollars() {
    let menu = tray_menu(&DaemonStatus::Running, Some(&glance(vec![], None)), true);
    assert_eq!(texts(&menu)[1], "Today: 12 prompts");
    assert_eq!(menu.badge, None, "no open leak: no badge");
}

#[test]
fn a_session_without_a_resume_command_opens_its_story() {
    let mut left = session("s/1", PickUpPresence::Left, None);
    left.ended = Outcome::Failed;
    let menu = tray_menu(&DaemonStatus::Running, Some(&glance(vec![left], None)), true);
    assert!(texts(&menu).contains(&"[Failed: work on s/1 · acme/app (open)]".to_owned()));
    assert_eq!(menu.action("session:0"), Some(&TrayAction::Open("/sessions/s%2F1".into())));
}

#[test]
fn open_leaks_lead_to_the_panel_and_set_the_badge() {
    let mut seen = glance(vec![], None);
    seen.open_leaks = 2;
    let menu = tray_menu(&DaemonStatus::Running, Some(&seen), true);
    assert_eq!(menu.badge, Some(2));
    assert_eq!(tray_menu(&DaemonStatus::Running, Some(&seen), false).badge, None, "the switch in Settings is off");
    assert_eq!(texts(&menu)[2], "[2 possible leaks to look at]");
    assert_eq!(menu.action("leaks"), Some(&TrayAction::Open("/#leaks".into())));
}

#[test]
fn a_prompt_label_hides_a_secret_and_stays_short() {
    let key = "sk-proj-".to_owned() + "q8Zt2Lw9XkP4vR7mN3cBy6Hd";
    let mut seen = glance(vec![], None);
    seen.recent = vec![RecentPrompt { id: 1, text: format!("use {key} for the upload job and then retry everything twice") }];
    let menu = tray_menu(&DaemonStatus::Running, Some(&seen), true);
    let TrayItem::Action { label, action, .. } = menu.items.last().unwrap() else { panic!("an action") };
    assert!(!label.contains("q8Zt2Lw9") && label.chars().count() <= LABEL_CHARS && label.ends_with('…'), "{label}");
    assert_eq!(action, &TrayAction::Copy(format!("use {key} for the upload job and then retry everything twice")));
}

#[test]
fn without_a_daemon_of_its_own_the_menu_says_why_and_offers_nothing_else() {
    let external = tray_menu(&DaemonStatus::External, None, true);
    assert_eq!(texts(&external), ["Recording in another GitPrompt (gitprompt serve)"]);
    let failed = tray_menu(&DaemonStatus::Failed { detail: "port 7878 in use".into() }, None, true);
    assert_eq!(texts(&failed), ["Not recording: port 7878 in use"]);
    assert_eq!(failed.badge, None);
}
