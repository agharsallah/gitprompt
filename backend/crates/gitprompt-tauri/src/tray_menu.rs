//! The tray's live part as data: whether GitPrompt records, today's prompts and cost, the sessions
//! to pick up and the last prompts you typed. A pure function of the daemon's status and its
//! [`Glance`]; the desktop app only draws it (`apps/desktop/src/tray.rs`) and runs the actions.
//!
//! Labels are one line and short; a prompt's label hides what looks like a secret (the copied text
//! is the prompt as stored: the menu bar is on screen, the clipboard is yours).

use gitprompt_core::numbers::{money, thousands};
use gitprompt_core::secrets;
use gitprompt_server::model::{PickUp, PickUpPresence, Target};
use gitprompt_server::service::{Glance, Today};
use gitprompt_store::filter::Outcome;

use crate::route::route;
use crate::state::DaemonStatus;

/// Characters a label keeps before it ends with `…`.
pub const LABEL_CHARS: usize = 48;

#[derive(Clone, Debug, PartialEq)]
pub struct TrayMenu {
    pub items: Vec<TrayItem>,
    /// The Dock badge: the possible leaks not handled yet, `None` for no badge (none open, or the
    /// `leak_badge` preference is off).
    pub badge: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TrayItem {
    /// A line to read, not to click.
    Text(String),
    Separator,
    /// `id` is unique in the menu: a click reports it.
    Action {
        id: String,
        label: String,
        action: TrayAction,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAction {
    /// Put this text on the clipboard.
    Copy(String),
    /// Show the window on this dashboard path.
    Open(String),
}

impl TrayMenu {
    /// The action of the item `id`.
    pub fn action(&self, id: &str) -> Option<&TrayAction> {
        self.items.iter().find_map(|item| match item {
            TrayItem::Action { id: own, action, .. } if own == id => Some(action),
            _ => None,
        })
    }
}

/// The menu for `status`, with what the daemon read (`None` when it could not be read: not
/// running, or another process serves it); `badge` is the `leak_badge` preference.
pub fn tray_menu(status: &DaemonStatus, glance: Option<&Glance>, badge: bool) -> TrayMenu {
    let mut items = vec![TrayItem::Text(recording(status))];
    let Some(glance) = glance else { return TrayMenu { items, badge: None } };
    items.push(TrayItem::Text(today(&glance.today)));
    if glance.open_leaks > 0 {
        items.push(leaks_item(glance.open_leaks));
    }
    items.push(TrayItem::Separator);
    items.push(TrayItem::Text("Pick up".into()));
    if glance.pick_up.is_empty() {
        items.push(TrayItem::Text("No session to pick up".into()));
    }
    items.extend(glance.pick_up.iter().enumerate().map(|(i, row)| pick_up_item(i, row)));
    if !glance.recent.is_empty() {
        items.push(TrayItem::Separator);
        items.push(TrayItem::Text("Last prompts (click copies)".into()));
        items.extend(glance.recent.iter().enumerate().map(|(i, prompt)| TrayItem::Action {
            id: format!("prompt:{i}"),
            label: label(&secrets::hide(&prompt.text)),
            action: TrayAction::Copy(prompt.text.clone()),
        }));
    }
    let open = i64::try_from(glance.open_leaks).unwrap_or(i64::MAX);
    TrayMenu { items, badge: (badge && open > 0).then_some(open) }
}

fn recording(status: &DaemonStatus) -> String {
    match status {
        DaemonStatus::Running => "Recording".into(),
        DaemonStatus::Starting => "Starting…".into(),
        DaemonStatus::External => "Recording in another GitPrompt (gitprompt serve)".into(),
        DaemonStatus::Failed { detail } => label(&format!("Not recording: {detail}")),
        DaemonStatus::Stopped => "Not recording".into(),
    }
}

fn today(today: &Today) -> String {
    let prompts = match today.prompts {
        0 => "no prompts yet".to_owned(),
        1 => "1 prompt".to_owned(),
        n => format!("{} prompts", thousands(n)),
    };
    match today.cost_usd {
        Some(usd) => format!("Today: {prompts} · {}", money(usd)),
        None => format!("Today: {prompts}"),
    }
}

fn leaks_item(open: usize) -> TrayItem {
    let what = if open == 1 { "1 possible leak to look at".to_owned() } else { format!("{open} possible leaks to look at") };
    TrayItem::Action { id: "leaks".into(), label: what, action: TrayAction::Open(route(&Target::Leaks)) }
}

/// A session: copy its resume command when its tool has one, else open its story.
fn pick_up_item(index: usize, row: &PickUp) -> TrayItem {
    let name = row.name.as_deref().or(row.label.as_deref()).unwrap_or(&row.session);
    let state = match (row.presence, row.ended) {
        (PickUpPresence::Running, _) => "Running",
        (PickUpPresence::Left, Outcome::Failed) => "Failed",
        (PickUpPresence::Left, Outcome::Interrupted) => "Stopped",
        (PickUpPresence::Left, _) => "Left",
    };
    let place = row.project.as_deref().map(|p| format!(" · {p}")).unwrap_or_default();
    let (verb, action) = match &row.resume {
        Some(resume) => ("copy resume", TrayAction::Copy(resume.paste.clone())),
        None => ("open", TrayAction::Open(route(&Target::Session { id: row.session.clone() }))),
    };
    TrayItem::Action { id: format!("session:{index}"), label: format!("{state}: {} ({verb})", label(&format!("{name}{place}"))), action }
}

/// `text` on one line, at most [`LABEL_CHARS`] characters.
fn label(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() <= LABEL_CHARS {
        return line;
    }
    let kept: String = line.chars().take(LABEL_CHARS - 1).collect();
    format!("{}…", kept.trim_end())
}

#[cfg(test)]
mod tests;
