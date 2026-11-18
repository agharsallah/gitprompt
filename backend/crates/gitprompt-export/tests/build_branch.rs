//! The prompts behind a branch: a summary line, each prompt folded with its commits, and the
//! redaction defaults (secrets masked, paths relative) on a block meant for a pull request.
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

use pretty_assertions::assert_eq;

use common::*;
use gitprompt_export::build::{self, BRANCH_OPTIONS, EMPTY_BRANCH};
use gitprompt_export::input::{BranchExport, ExportInput, ProjectPrompt};
use gitprompt_export::report::{Block, Report};
use gitprompt_export::{Format, Kind, prepare, render_text};

fn block_of(data: BranchExport) -> String {
    let export = prepare(&ExportInput::Branch(data), &BRANCH_OPTIONS, &context()).expect("prepare");
    render_text(&export, Format::Markdown, None).expect("render")
}

fn report_of(data: &BranchExport) -> Report {
    build::branch(data, &BRANCH_OPTIONS, utc()).0
}

#[test]
fn a_key_in_a_prompt_is_masked_and_absolute_paths_become_relative() {
    let block = block_of(branch_export());
    assert!(block.contains(PLANTED_REDACTED), "{block}");
    assert_absent(&block, &never_with_text());
}

#[test]
fn the_summary_line_counts_prompts_commits_agent_time_and_rewordings() {
    let report = report_of(&branch_export());
    assert_eq!((report.kind, report.title.as_str()), (Kind::Branch, "Prompts behind timing-ledger"));
    assert_eq!(report.subtitle, "2 prompts · 3 commits · 51 min of agent time · 1 reworded");
}

#[test]
fn figures_the_data_cannot_back_are_left_out_of_the_summary() {
    let mut data = branch_export();
    for prompt in &mut data.prompts {
        prompt.response_s = None;
        prompt.retry_of = None;
    }
    assert_eq!(report_of(&data).subtitle, "2 prompts · 3 commits");
}

#[test]
fn a_commit_shared_by_two_prompts_is_counted_once() {
    let mut data = branch_export();
    let shared = data.prompts[0].commits[0].clone();
    data.prompts[1].commits.push(shared);
    assert!(report_of(&data).subtitle.starts_with("2 prompts · 3 commits"));
}

#[test]
fn an_empty_branch_says_so_and_folds_nothing() {
    let data = BranchExport { prompts: vec![], ..branch_export() };
    let report = report_of(&data);
    assert_eq!((report.subtitle.as_str(), report.sections.len()), (EMPTY_BRANCH, 0));
    let block = block_of(data);
    assert!(block.contains(EMPTY_BRANCH) && !block.contains("<details>"), "{block}");
}

#[test]
fn each_prompt_is_folded_under_its_number_first_line_and_short_hashes() {
    let block = block_of(branch_export());
    let summaries: Vec<&str> = block.lines().filter(|l| l.starts_with("<summary>")).collect();
    assert_eq!(summaries.len(), 2, "{block}");
    assert!(summaries[0].starts_with("<summary>1. use [hidden Anthropic API key] and write to src/app.py"), "{block}");
    assert!(summaries[0].ends_with("… · abc1234</summary>"), "{block}");
    assert_eq!(summaries[1], "<summary>2. Block the flaky kinds in CI · d3b1663 +1</summary>");
    assert_eq!(block.matches("<details>").count(), block.matches("</details>").count());
}

#[test]
fn a_prompt_lists_its_commits_without_their_author() {
    let block = block_of(branch_export());
    assert!(block.contains("- `d3b1663 Add the ledger (+3 −0)`\n- `a08db20 Block the flaky kinds (+3 −0)`"), "{block}");
    assert_absent(&block, &[AUTHOR.to_string()]);
}

#[test]
fn a_prompt_says_how_long_it_ran_and_that_it_rewords() {
    let report = report_of(&branch_export());
    let Block::Paragraph(facts) = &report.sections[1].blocks[0] else { panic!("facts first") };
    assert_eq!(facts.text, "worked 50 min · rewords an earlier prompt");
}

#[test]
fn a_prompt_without_known_facts_opens_with_its_text() {
    let mut data = branch_export();
    data.prompts[0].response_s = None;
    assert!(matches!(report_of(&data).sections[0].blocks[0], Block::Quote(_)));
}

#[test]
fn html_in_a_prompt_stays_text_in_its_summary() {
    let mut data = branch_export();
    data.prompts.truncate(1);
    data.prompts[0] = ProjectPrompt { text: Some("<script>alert(1)</script> & more".into()), ..data.prompts[0].clone() };
    let block = block_of(data);
    assert!(block.contains("<summary>1. &lt;script&gt;alert(1)&lt;/script&gt; &amp; more · abc1234</summary>"), "{block}");
}

#[test]
fn the_footer_says_what_was_masked_and_where_it_was_made() {
    let block = block_of(branch_export());
    assert!(block.trim_end().ends_with("_Secrets masked · paths relative · made by GitPrompt, on this machine_"), "{block}");
}
