//! A source of Worth a look: the possible leaks not handled yet, first of all kinds, to hide.

use super::{Action, Inputs, Item, Kind};
use crate::target::Target;

/// Kinds named in the sentence at most.
const KINDS_NAMED: usize = 2;

/// The open findings of the range, if any.
pub(super) fn source(inputs: &Inputs) -> Option<Item> {
    let open = &inputs.leaks;
    let first = open.first()?;
    let mut kinds: Vec<&str> = Vec::new();
    for leak in open {
        if !kinds.contains(&leak.kind.as_str()) {
            kinds.push(&leak.kind);
        }
    }
    let named = kinds.iter().take(KINDS_NAMED).copied().collect::<Vec<_>>().join(", ");
    let more = if kinds.len() > KINDS_NAMED { ", …" } else { "" };
    let text = if open.len() == 1 {
        format!("A possible leak: {} {}", first.kind, first.masked)
    } else {
        format!("{} possible leaks: {named}{more}", open.len())
    };
    Some(Item {
        kind: Kind::PossibleLeak,
        text,
        action: Action::HideValues { findings: open.iter().map(|l| l.id.clone()).collect() },
        target: Target::Leaks,
    })
}
