//! `gitprompt why`: what made the code, one module per question. `<commit>`: the prompt behind a
//! commit ([`commit`]); `--branch`: the prompts behind a branch ([`branch`]).

mod branch;
mod commit;
mod verdict;

use gitprompt_core::Settings;

use crate::commands::Outcome as Exit;
use crate::{Io, WhyArgs};

/// Clap lets exactly one of the commit and `--branch` through.
pub(crate) fn why(settings: &Settings, args: &WhyArgs, io: &mut Io<'_>) -> Exit {
    match (&args.commit, &args.branch) {
        (Some(rev), _) => commit::print(settings, rev, args.json, io),
        (None, Some(branch)) => branch::print(settings, branch, args.project.as_deref(), io),
        (None, None) => Ok(2),
    }
}
