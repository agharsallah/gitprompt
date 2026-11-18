//! `gitprompt why`: what made the code, one module per question. `--branch`: the prompts behind
//! a branch ([`branch`]).

mod branch;

use gitprompt_core::Settings;

use crate::commands::Outcome as Exit;
use crate::{Io, WhyArgs};

pub(crate) fn why(settings: &Settings, args: &WhyArgs, io: &mut Io<'_>) -> Exit {
    branch::print(settings, &args.branch, args.project.as_deref(), io)
}
