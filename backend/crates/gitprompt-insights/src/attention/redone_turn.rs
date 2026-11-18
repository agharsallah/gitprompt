//! A source of Worth a look: the redone turn that cost the most in the range, to open and see why.

use gitprompt_core::numbers::{duration, money};

use super::{Action, Inputs, Item, Kind};
use crate::target::Target;

/// The costliest redone turn: its cost where reported, else its agent time.
pub(super) fn source(inputs: &Inputs) -> Option<Item> {
    let turn = inputs.redone.as_ref()?.costliest.as_ref()?;
    let cost = match turn.cost_usd {
        Some(usd) if usd > 0.0 => money(usd),
        _ => format!("{} of agent time", duration(turn.agent_time_s)),
    };
    Some(Item {
        kind: Kind::RedoneTurn,
        text: format!("Costliest redone turn: {cost}"),
        action: Action::OpenTurn { prompt_id: turn.id },
        target: Target::Prompt { id: turn.id },
    })
}
