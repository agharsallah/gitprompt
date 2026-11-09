//! The self-contained HTML file (export/html.py, ADR-0018 §5): escaped text, inline SVG, no script,
//! no request of any kind, light and dark from the reader's setting, printable.

mod common;

use pretty_assertions::assert_eq;

use common::*;
use gitprompt_export::Kind;
use gitprompt_export::charts::CHART_STYLE;
use gitprompt_export::html::{CSP, html, style};
use gitprompt_export::report::{Block, Chart, ChartKind, Figure, Figures, Items, Level, Paragraph, Quote, Report, Section, Table, Voice};

/// Markup that would run or load something if it were not escaped (no URL: see the URL test).
const HOSTILE: &str = "<script>alert(1)</script><img onerror=alert(2)><a onclick=x>x</a><style>@x{}</style> & 'q'";

fn page(blocks: Vec<Block>) -> String {
    html(&report(vec![section("S", blocks)]))
}

/// A report with the hostile string in every text slot a report has.
fn hostile_report() -> Report {
    let mut c: Chart =
        chart("c", ChartKind::Bars, vec![gitprompt_export::report::Bar { label: HOSTILE.into(), value: 1.0, display: HOSTILE.into() }]);
    c.title = HOSTILE.into();
    c.caption = HOSTILE.into();
    Report {
        kind: Kind::Session,
        title: HOSTILE.into(),
        subtitle: HOSTILE.into(),
        generated_at: HOSTILE.into(),
        sections: vec![Section {
            title: HOSTILE.into(),
            subtitle: HOSTILE.into(),
            level: Level::Two,
            blocks: vec![
                Block::Figures(Figures { items: vec![Figure { label: HOSTILE.into(), value: HOSTILE.into() }] }),
                Block::Chart(c),
                Block::Paragraph(Paragraph { text: HOSTILE.into(), muted: false }),
                Block::Quote(Quote { text: HOSTILE.into(), voice: Voice::System, caption: HOSTILE.into() }),
                Block::Items(Items { items: vec![HOSTILE.into()], mono: true }),
                Block::Table(Table { columns: vec![HOSTILE.into()], rows: vec![vec![HOSTILE.into()]], numeric: vec![] }),
            ],
        }],
        notes: vec![HOSTILE.into()],
    }
}

// --- self-contained, whatever the data -----------------------------------------------------------

#[test]
fn hostile_text_in_every_slot_is_escaped_and_never_becomes_markup() {
    let body = html(&hostile_report());
    let forbidden: Vec<String> = ["<script", "<img", "<a ", "<a>"].map(String::from).to_vec();
    assert_absent(&body, &forbidden);
    assert!(body.contains("&lt;script&gt;alert(1)&lt;/script&gt;&lt;img onerror=alert(2)&gt;"));
    assert_eq!(body.matches("<style>").count(), 1);
}

#[test]
fn a_url_in_the_data_stays_text_and_never_becomes_an_attribute() {
    let body = page(vec![Block::Paragraph(Paragraph { text: "see https://tracker.invalid/p.png".into(), muted: false })]);
    assert!(body.contains("<p>see https://tracker.invalid/p.png</p>"));
    let forbidden: Vec<String> = [" src=", " href=", "<img", "<a "].map(String::from).to_vec();
    assert_absent(&body, &forbidden);
}

#[test]
fn the_page_makes_no_request_of_any_kind() {
    let body = html(&hostile_report());
    let forbidden: Vec<String> = [
        "<link",
        "<iframe",
        "<object",
        "<embed",
        "<video",
        "<audio",
        "<source",
        "<base",
        " srcset=",
        " action=",
        " poster=",
        " data=\"",
        "xlink:href",
        "http://",
        "https://",
        "url(",
        "@import",
    ]
    .map(String::from)
    .to_vec();
    assert_absent(&body, &forbidden);
}

#[test]
fn the_page_carries_its_own_content_security_policy() {
    assert_eq!(CSP, "default-src 'none'; style-src 'unsafe-inline'; img-src data:; base-uri 'none'; form-action 'none'");
    assert!(page(vec![]).contains(&format!(r#"<meta http-equiv="Content-Security-Policy" content="{CSP}">"#)));
}

#[test]
fn the_page_sends_no_referrer_and_declares_utf8_and_a_mobile_viewport() {
    let body = page(vec![]);
    assert!(body.starts_with("<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">"), "{body}");
    assert!(body.contains(r#"<meta name="referrer" content="no-referrer">"#));
    assert!(body.contains(r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#));
    assert!(body.ends_with("</main></body></html>\n"));
}

#[test]
fn escaping_covers_ampersands_and_both_quotes() {
    assert!(page(vec![Block::Paragraph(Paragraph { text: "<b>&\"'".into(), muted: false })]).contains("<p>&lt;b&gt;&amp;&quot;&#x27;</p>"));
}

// --- light, dark and print -----------------------------------------------------------------------

#[test]
fn the_page_follows_the_readers_light_or_dark_setting() {
    let body = page(vec![]);
    assert!(body.contains(r#"<meta name="color-scheme" content="light dark">"#));
    assert!(body.contains(":root{color-scheme:light dark;--bg:#fcfdfd;"));
    assert!(body.contains("@media (prefers-color-scheme: dark){:root{--bg:#151c1b;"));
}

#[test]
fn printing_forces_the_light_palette_and_hides_the_number_tables() {
    let css = style();
    let print = &css[css.find("@media print{").expect("a print block")..];
    assert!(print.contains(":root{--bg:#fcfdfd;"), "{print}");
    assert!(print.contains("details{display:none}"), "{print}");
}

#[test]
fn the_stylesheet_defines_the_chart_classes_and_variables() {
    let css = style();
    assert!(css.contains(CHART_STYLE));
    assert!(css.contains("--gp-chart:#1a9384") && css.contains("--gp-chart:#2fa898"));
}

#[test]
fn the_stylesheet_is_inline_in_the_head() {
    assert!(page(vec![]).contains(&format!("<style>{}</style></head>", style())));
}

// --- the document --------------------------------------------------------------------------------

#[test]
fn the_head_title_names_the_report_and_gitprompt() {
    assert!(page(vec![]).contains("<title>A title · GitPrompt</title>"));
}

#[test]
fn the_body_opens_with_kind_title_and_subtitle() {
    assert!(
        page(vec![]).contains(
            r#"<body><main><p class="kicker">GitPrompt session report</p><h1>A title</h1><p class="sub">A subtitle</p><section>"#
        )
    );
}

#[test]
fn a_report_without_subtitle_has_no_subtitle_paragraph() {
    let mut r = report(vec![]);
    r.subtitle = String::new();
    assert!(!html(&r).contains(r#"class="sub""#));
}

#[test]
fn a_section_is_headed_at_its_level_with_its_subtitle() {
    let s = Section { title: "Prompt 1".into(), blocks: vec![], subtitle: "09:00".into(), level: Level::Three };
    assert!(html(&report(vec![s])).contains(r#"<section><h3>Prompt 1</h3><p class="section-sub">09:00</p></section>"#));
}

#[test]
fn a_section_without_subtitle_is_just_its_heading() {
    assert!(page(vec![]).contains("<section><h2>S</h2></section>"));
}

#[test]
fn the_footer_states_the_export_date_and_notes() {
    assert!(page(vec![]).contains(
        "<footer><p>Exported from GitPrompt on 2026-03-03 10:00.</p><ul><li>First note.</li><li>Second note.</li></ul></footer>"
    ));
}

#[test]
fn the_same_report_renders_the_same_page() {
    let r = hostile_report();
    assert_eq!(html(&r), html(&r.clone()));
}

// --- blocks --------------------------------------------------------------------------------------

#[test]
fn figures_are_a_definition_list() {
    let f = Figures { items: vec![Figure { label: "Prompts".into(), value: "12".into() }] };
    assert!(page(vec![Block::Figures(f)]).contains(r#"<dl class="figures"><div><dt>Prompts</dt><dd>12</dd></div></dl>"#));
}

#[test]
fn a_chart_is_a_figure_with_inline_svg_caption_and_a_numbers_table() {
    let mut c = chart("c", ChartKind::Bars, vec![bar("Done", 1.0)]);
    c.caption = "A caption".into();
    let body = page(vec![Block::Chart(c.clone())]);
    let expected = format!(
        r#"<div class="charts"><figure><figcaption>Chart c</figcaption>{}<p class="muted">A caption</p><details><summary>Numbers</summary><div class="table"><table><tr><td>Done</td><td class="num">1</td></tr></table></div></details></figure></div>"#,
        gitprompt_export::charts::svg(&c, false)
    );
    assert!(body.contains(&expected), "{body}");
}

#[test]
fn an_inline_chart_carries_no_namespace_url() {
    assert!(!page(vec![Block::Chart(chart("c", ChartKind::Columns, vec![bar("d", 1.0)]))]).contains("xmlns"));
}

#[test]
fn consecutive_charts_share_one_grid_and_separated_ones_do_not() {
    let a = Block::Chart(chart("a", ChartKind::Bars, vec![bar("x", 1.0)]));
    let b = Block::Chart(chart("b", ChartKind::Bars, vec![bar("y", 1.0)]));
    let p = Block::Paragraph(Paragraph { text: "between".into(), muted: false });
    let together = page(vec![a.clone(), b.clone()]);
    let apart = page(vec![a, p, b]);
    assert_eq!((together.matches(r#"<div class="charts">"#).count(), together.matches("<figure>").count()), (1, 2));
    assert_eq!(apart.matches(r#"<div class="charts">"#).count(), 2);
}

#[test]
fn charts_without_items_are_left_out_and_add_no_empty_grid() {
    let body = page(vec![Block::Chart(chart("a", ChartKind::Bars, vec![])), Block::Chart(chart("b", ChartKind::Columns, vec![]))]);
    assert!(body.contains("<section><h2>S</h2></section>"), "{body}");
}

#[test]
fn paragraphs_are_plain_or_muted() {
    let body = page(vec![
        Block::Paragraph(Paragraph { text: "loud".into(), muted: false }),
        Block::Paragraph(Paragraph { text: "quiet".into(), muted: true }),
    ]);
    assert!(body.contains(r#"<p>loud</p><p class="muted">quiet</p>"#));
}

#[test]
fn what_you_typed_is_a_blockquote_saying_so() {
    let q = Quote { text: "hi\nthere".into(), voice: Voice::You, caption: "clipped".into() };
    assert!(page(vec![Block::Quote(q)]).contains(
        r#"<blockquote><span class="who">You typed · clipped</span><p class="body">hi
there</p></blockquote>"#
    ));
}

#[test]
fn system_text_is_a_dashed_blockquote_named_by_its_caption_or_system_text() {
    let named = Quote { text: "x".into(), voice: Voice::System, caption: "Task notification".into() };
    let plain = Quote { text: "y".into(), voice: Voice::System, caption: String::new() };
    let body = page(vec![Block::Quote(named), Block::Quote(plain)]);
    assert!(body.contains(r#"<blockquote class="system"><span class="who">Task notification</span><p class="body">x</p></blockquote>"#));
    assert!(body.contains(r#"<blockquote class="system"><span class="who">System text</span><p class="body">y</p></blockquote>"#));
}

#[test]
fn a_quote_of_yours_without_caption_says_only_you_typed() {
    let q = Quote { text: "x".into(), voice: Voice::You, caption: String::new() };
    assert!(page(vec![Block::Quote(q)]).contains(r#"<span class="who">You typed</span>"#));
}

#[test]
fn items_are_a_list_monospaced_for_paths() {
    let body = page(vec![
        Block::Items(Items { items: vec!["a.py".into()], mono: true }),
        Block::Items(Items { items: vec!["plain".into()], mono: false }),
    ]);
    assert!(body.contains(r#"<ul class="mono"><li>a.py</li></ul><ul><li>plain</li></ul>"#));
}

#[test]
fn a_table_right_aligns_numbers_and_wraps_the_prompt_column() {
    let t = Table {
        columns: vec!["When".into(), "Cost".into(), "Prompt".into()],
        rows: vec![vec!["09:00".into(), "$1.00".into(), "fix it".into()]],
        numeric: vec![false, true, false],
    };
    assert!(page(vec![Block::Table(t)]).contains(
        r#"<div class="table"><table><thead><tr><th>When</th><th class="num">Cost</th><th>Prompt</th></tr></thead><tbody><tr><td>09:00</td><td class="num">$1.00</td><td class="text">fix it</td></tr></tbody></table></div>"#
    ));
}

#[test]
fn a_table_without_rows_is_left_out() {
    let t = Table { columns: vec!["A".into()], rows: vec![], numeric: vec![] };
    assert!(page(vec![Block::Table(t)]).contains("<section><h2>S</h2></section>"));
}
