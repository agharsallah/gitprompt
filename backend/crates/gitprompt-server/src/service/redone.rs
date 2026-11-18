//! Redone work over the prompts a filter keeps: the store reads, insights decides.

use gitprompt_core::spec::{Capability, SpecSet};
use gitprompt_insights::redone::{RedoneWork, redone};
use gitprompt_store::filter::PromptFilter;

use super::Inner;
use crate::error::ApiResult;

impl Inner {
    /// What the turns the filter keeps that you had to redo cost; cost only for tools that report
    /// it, themselves or through a layer.
    pub(super) fn redone_work(&self, set: &SpecSet, filter: &PromptFilter) -> ApiResult<RedoneWork> {
        let prompts = self.store.timeline(filter, None, false)?;
        Ok(redone(&prompts, |source| set.tool_has(source, Capability::Cost)))
    }
}
