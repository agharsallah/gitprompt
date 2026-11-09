//! Markdown (export/markdown.py): GitHub-flavoured, charts as tables, inert prompt text.

mod common;

use pretty_assertions::assert_eq;

use common::*;
use gitprompt_export::markdown::{cell, fence, inline, markdown};
use gitprompt_export::report::{Bar, Block, ChartKind, Figure, Figures, Items, Level, Paragraph, Quote, Section, Table, Voice};

fn one_block(block: Block) -> String {
    markdown(&report(vec![section("S", vec![block])]))
}

/// The lines between the section heading and the footer.
fn body_of(block: Block) -> String {
    let md = one_block(block);
    let start = md.find("## S\n\n").expect("section") + "## S\n\n".len();
    let end = md.find("---\n").expect("footer");
    md[start..end].to_string()
}

// --- the document --------------------------------------------------------------------------------

#[test]
fn a_minimal_report_is_title_subtitle_section_and_footer() {
    assert_eq!(
        one_block(Block::Paragraph(Paragraph { text: "Hello.".into(), muted: false })),
        "# A title\n\nA subtitle\n\n## S\n\nHello.\n\n---\n\n_Exported from GitPrompt on 2026-03-03 10:00._\n\n- First note.\n- Second note.\n"
    );
}

#[test]
fn a_report_without_subtitle_goes_from_title_to_first_section() {
    let mut r = report(vec![section("S", vec![])]);
    r.subtitle = String::new();
    assert!(markdown(&r).starts_with("# A title\n\n## S\n\n---\n"), "{}", markdown(&r));
}

#[test]
fn a_level_three_section_with_subtitle_is_a_smaller_heading_and_an_italic_line() {
    let s = Section { title: "Prompt 1".into(), blocks: vec![], subtitle: "09:00 · Done".into(), level: Level::Three };
    assert!(markdown(&report(vec![s])).contains("\n### Prompt 1\n\n_09:00 · Done_\n\n"));
}

#[test]
fn titles_and_subtitles_are_escaped() {
    let mut r = report(vec![]);
    r.title = "Fix [link](x) *now*".into();
    assert!(markdown(&r).starts_with("# Fix \\[link\\](x) \\*now\\*\n"));
}

#[test]
fn the_same_report_renders_the_same_markdown() {
    let r = report(vec![section("S", vec![Block::Chart(chart("c", ChartKind::Bars, vec![bar("a", 1.0)]))])]);
    assert_eq!(markdown(&r), markdown(&r.clone()));
}

// --- blocks --------------------------------------------------------------------------------------

#[test]
fn figures_are_a_list_of_bold_labels() {
    let figures = Figures {
        items: vec![Figure { label: "Prompts".into(), value: "12".into() }, Figure { label: "Cost".into(), value: "$1.20".into() }],
    };
    assert_eq!(body_of(Block::Figures(figures)), "- **Prompts:** 12\n- **Cost:** $1.20\n\n");
}

#[test]
fn a_chart_is_a_table_of_its_values_under_its_title() {
    let c = gitprompt_export::report::Chart {
        items: vec![
            Bar { label: "Done".into(), value: 1.0, display: "1".into() },
            Bar { label: "a|b".into(), value: 2.0, display: "$2.00".into() },
        ],
        ..chart("c", ChartKind::Bars, vec![])
    };
    assert_eq!(body_of(Block::Chart(c)), "**Chart c**\n\n|   | Value |\n| --- | ---: |\n| Done | 1 |\n| a\\|b | $2.00 |\n\n");
}

#[test]
fn a_chart_without_items_is_left_out() {
    assert_eq!(body_of(Block::Chart(chart("c", ChartKind::Bars, vec![]))), "");
}

#[test]
fn a_muted_paragraph_is_italic() {
    assert_eq!(body_of(Block::Paragraph(Paragraph { text: "quiet".into(), muted: true })), "_quiet_\n\n");
}

#[test]
fn what_you_typed_is_fenced_under_you_typed() {
    let q = Quote { text: "hello *world*".into(), voice: Voice::You, caption: String::new() };
    assert_eq!(body_of(Block::Quote(q)), "You typed:\n\n```text\nhello *world*\n```\n\n");
}

#[test]
fn a_caption_on_what_you_typed_goes_in_parentheses() {
    let q = Quote { text: "hel…".into(), voice: Voice::You, caption: "clipped".into() };
    assert!(body_of(Block::Quote(q)).starts_with("You typed (clipped):\n\n"));
}

#[test]
fn system_text_is_never_labelled_as_typed() {
    let plain = Quote { text: "x".into(), voice: Voice::System, caption: String::new() };
    let named = Quote { text: "x".into(), voice: Voice::System, caption: "Task notification".into() };
    assert_eq!(
        [body_of(Block::Quote(plain)), body_of(Block::Quote(named))],
        ["System text:\n\n```text\nx\n```\n\n", "Task notification:\n\n```text\nx\n```\n\n"]
    );
}

#[test]
fn prompt_markdown_stays_inert_in_a_longer_fence() {
    let q = Quote { text: "![x](https://tracker.invalid/p.png) ```code```".into(), voice: Voice::You, caption: String::new() };
    assert!(body_of(Block::Quote(q)).contains("````text\n![x](https://tracker.invalid/p.png) ```code```\n````"));
}

#[test]
fn mono_items_are_inline_code_unless_they_hold_a_backtick() {
    let items = Items { items: vec!["src/app.py".into(), "a`b".into()], mono: true };
    assert_eq!(body_of(Block::Items(items)), "- `src/app.py`\n- a\\`b\n\n");
}

#[test]
fn plain_items_are_escaped() {
    let items = Items { items: vec!["see [x](y)".into()], mono: false };
    assert_eq!(body_of(Block::Items(items)), "- see \\[x\\](y)\n\n");
}

#[test]
fn a_table_aligns_numeric_columns_right_and_cleans_its_cells() {
    let t = Table {
        columns: vec!["When".into(), "Cost".into(), "Prompt".into()],
        rows: vec![vec!["09:00".into(), "$1.00".into(), "a | b\nnext   line".into()], vec!["".into(), "".into(), "".into()]],
        numeric: vec![false, true],
    };
    assert_eq!(
        body_of(Block::Table(t)),
        "| When | Cost | Prompt |\n| --- | ---: | --- |\n| 09:00 | $1.00 | a \\| b next line |\n|   |   |   |\n\n"
    );
}

#[test]
fn a_table_without_rows_is_left_out() {
    let t = Table { columns: vec!["A".into()], rows: vec![], numeric: vec![] };
    assert_eq!(body_of(Block::Table(t)), "");
}

// --- helpers -------------------------------------------------------------------------------------

#[test]
fn inline_text_escapes_everything_that_could_become_markup() {
    assert_eq!(inline(r"\ ` * _ [ ] < > ! # plain"), r"\\ \` \* \_ \[ \] \< \> \! \# plain");
}

#[test]
fn a_cell_collapses_whitespace_escapes_pipes_and_is_never_empty() {
    assert_eq!([cell("  a \t\n b|c  "), cell(""), cell(" \n ")], ["a b\\|c", " ", " "]);
}

#[test]
fn a_fence_is_one_backtick_longer_than_the_longest_run_inside() {
    assert_eq!(
        [fence("plain"), fence("a ``` b"), fence("a ````` b")],
        ["```text\nplain\n```", "````text\na ``` b\n````", "``````text\na ````` b\n``````"]
    );
}
