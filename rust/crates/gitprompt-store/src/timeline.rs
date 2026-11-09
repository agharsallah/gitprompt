//! Reads for stories and search (ADR-0016): prompts as the index sees them, in time order, with
//! their typed text and outcome, plus the few turn events a story shows.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use gitprompt_core::spec::ToolKind;
use gitprompt_core::{Json, Result};

use crate::Store;
use crate::filter::{Outcome, PromptFilter};
use crate::index::IndexRow;

/// A prompt as stories and search see it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IndexedPrompt {
    #[serde(flatten)]
    pub row: IndexRow,
    pub outcome: Outcome,
}

/// A finished tool call or a compaction of a turn, with only what a story shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TurnEvent {
    pub prompt_id: String,
    pub event: String,
    pub received_at: String,
    pub tool: Option<String>,
    /// The tool input when it is an object.
    pub tool_input: Option<Json>,
    /// `tool_input.description`, else `tool_input.task_name`.
    pub description: Option<String>,
    pub subagent_type: Option<String>,
    /// `tool_input.prompt`, else `tool_input.message`, clipped to `clip` characters.
    pub prompt: Option<String>,
}

impl Store {
    /// The prompts the filter keeps, oldest first unless `newest_first` (by received_at, then id).
    pub fn timeline(&self, _filter: &PromptFilter, _limit: Option<usize>, _newest_first: bool) -> Result<Vec<IndexedPrompt>> {
        todo!()
    }

    pub fn timeline_one(&self, _id: i64) -> Result<Option<IndexedPrompt>> {
        todo!()
    }

    pub fn count(&self, _filter: &PromptFilter) -> Result<i64> {
        todo!()
    }

    /// Tool calls per kind of each prompt's turn.
    pub fn tool_kinds(&self, _ids: &[i64]) -> Result<BTreeMap<i64, BTreeMap<ToolKind, i64>>> {
        todo!()
    }

    /// `PostToolUse` and `PreCompact` events of these turns, oldest first.
    pub fn turn_events(&self, _prompt_ids: &[String], _clip: usize) -> Result<Vec<TurnEvent>> {
        todo!()
    }
}
