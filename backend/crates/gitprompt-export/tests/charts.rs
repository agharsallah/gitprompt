//! Charts (export/charts.py): inline and standalone SVG geometry, themes, picking the chart a
//! single-chart format draws, and the PNG drawn from the SVG.

mod common;

use pretty_assertions::assert_eq;

use common::*;
use gitprompt_export::ExportError;
use gitprompt_export::charts::{CHART_STYLE, ChartInfo, DARK, FONT, LIGHT, chart_list, pick, png, svg, svg_file, theme_vars};
use gitprompt_export::report::{Bar, Block, Chart, ChartKind, Paragraph};

fn three_bars() -> Chart {
    chart("x", ChartKind::Bars, vec![bar("a", 10.0), bar("b", 5.0), bar("c", 0.01)])
}

fn four_columns() -> Chart {
    chart("d", ChartKind::Columns, vec![bar("d1", 1.0), bar("d2", 2.0), bar("d3", 4.0), bar("d4", 0.0)])
}

fn columns(n: usize) -> Chart {
    chart("d", ChartKind::Columns, (1..=n).map(|i| bar(&format!("d{i}"), i as f64)).collect())
}

// --- themes --------------------------------------------------------------------------------------

#[test]
fn a_palette_reads_as_css_custom_properties() {
    assert_eq!(
        theme_vars(&LIGHT),
        "--gp-bg:#fcfdfd;--gp-text:#151c1b;--gp-muted:#5f6c69;--gp-grid:#e6ebea;--gp-axis:#c9d2d0;--gp-chart:#1a9384"
    );
}

// --- bars ----------------------------------------------------------------------------------------

#[test]
fn a_chart_without_items_draws_nothing() {
    assert_eq!([svg(&chart("e", ChartKind::Bars, vec![]), false), svg(&chart("e", ChartKind::Columns, vec![]), true)], ["", ""]);
}

#[test]
fn an_inline_bar_chart_opens_with_its_box_label_and_description() {
    assert!(svg(&three_bars(), false).starts_with(&format!(
        r#"<svg class="gp-bars" viewBox="0 0 360 102" role="img" font-family="{FONT}" aria-label="Chart x"><desc>a: 10; b: 5; c: 0.01</desc><g>"#
    )));
}

#[test]
fn a_bar_row_has_its_label_value_track_and_bar() {
    assert!(svg(&three_bars(), false).contains(concat!(
        r#"<g><title>a: 10</title><text class="gp-text" x="0" y="13" font-size="13">a</text>"#,
        r#"<text class="gp-muted gp-num" x="360" y="13" font-size="13" text-anchor="end">10</text>"#,
        r#"<rect class="gp-track" x="0" y="19" width="360" height="8" rx="4"/>"#,
        r#"<rect class="gp-bar" x="0" y="19" width="360.0" height="8" rx="4"/></g>"#,
    )));
}

#[test]
fn bars_are_as_long_as_their_share_of_the_peak_but_never_under_two() {
    let drawn = svg(&three_bars(), false);
    assert!(drawn.contains(r#"<rect class="gp-bar" x="0" y="53" width="180.0" height="8" rx="4"/>"#), "{drawn}");
    assert!(drawn.contains(r#"<rect class="gp-bar" x="0" y="87" width="2.0" height="8" rx="4"/>"#), "{drawn}");
}

#[test]
fn bars_of_all_zero_values_are_drawn_at_the_minimum_width() {
    let drawn = svg(&chart("z", ChartKind::Bars, vec![bar("a", 0.0)]), false);
    assert!(drawn.contains(r#"width="2.0""#), "{drawn}");
}

#[test]
fn a_long_bar_label_is_clipped_but_its_tooltip_keeps_it_whole() {
    let label = format!("{} bcdef", "a".repeat(34));
    let drawn = svg(&chart("l", ChartKind::Bars, vec![Bar { label: label.clone(), value: 1.0, display: "1".into() }]), false);
    assert!(drawn.contains(&format!(r#"font-size="13">{}…</text>"#, "a".repeat(34))), "{drawn}");
    assert!(drawn.contains(&format!("<title>{label}: 1</title>")));
}

#[test]
fn a_bar_label_of_exactly_the_limit_is_not_clipped() {
    let label = "b".repeat(36);
    assert!(svg(&chart("l", ChartKind::Bars, vec![bar(&label, 1.0)]), false).contains(&format!(r#"font-size="13">{label}</text>"#)));
}

#[test]
fn chart_text_is_escaped_everywhere() {
    let mut c = chart("h", ChartKind::Bars, vec![Bar { label: "<b>&".into(), value: 1.0, display: "\"1\"".into() }]);
    c.title = "<t>".into();
    let drawn = svg(&c, true);
    assert_absent(&drawn, &["<b>".to_string(), "<t>".to_string()]);
    assert!(drawn.contains(r#"aria-label="&lt;t&gt;""#) && drawn.contains("<desc>&lt;b&gt;&amp;: &quot;1&quot;</desc>"), "{drawn}");
}

// --- standalone ----------------------------------------------------------------------------------

#[test]
fn a_standalone_chart_has_its_namespace_size_and_padding() {
    assert!(svg(&three_bars(), true).starts_with(
        r#"<svg class="gp-bars" viewBox="-16 -16 392 170" xmlns="http://www.w3.org/2000/svg" width="392" height="170" role="img""#
    ));
}

#[test]
fn a_standalone_chart_carries_both_themes_its_background_and_title() {
    let drawn = svg(&three_bars(), true);
    let style =
        format!(":root{{{}}}@media (prefers-color-scheme: dark){{:root{{{}}}}}{CHART_STYLE}", theme_vars(&LIGHT), theme_vars(&DARK));
    assert!(drawn.contains(&format!(
        r#"<style>{style}</style><rect x="-16" y="-16" width="392" height="170" fill="var(--gp-bg)"/><text class="gp-text" x="0" y="14" font-size="16" font-weight="600">Chart x</text>"#
    )), "{drawn}");
}

#[test]
fn a_standalone_chart_draws_its_rows_below_the_title() {
    assert!(svg(&three_bars(), true).contains(r#"<text class="gp-text" x="0" y="49" font-size="13">a</text>"#));
}

#[test]
fn an_inline_chart_has_no_namespace_style_or_title_text() {
    let drawn = svg(&three_bars(), false);
    assert_absent(&drawn, &["xmlns".to_string(), "<style>".to_string(), "font-weight".to_string()]);
}

// --- columns -------------------------------------------------------------------------------------

#[test]
fn a_column_chart_shows_its_peak_axis_and_box() {
    let drawn = svg(&four_columns(), false);
    assert!(drawn.starts_with(r#"<svg class="gp-columns" viewBox="0 0 480 190""#), "{drawn}");
    assert!(drawn.contains(
        r#"<text class="gp-muted gp-num" x="0" y="12" font-size="15">4</text><line class="gp-axis" x1="0" x2="480" y1="160.5" y2="160.5"/>"#
    ));
}

#[test]
fn columns_are_as_tall_as_their_share_of_the_peak() {
    let drawn = svg(&four_columns(), false);
    assert!(
        drawn.contains(r#"<rect class="gp-bar" x="16.8" y="125.0" width="86.4" height="35.0" rx="2"><title>d1: 1</title></rect>"#),
        "{drawn}"
    );
    assert!(drawn.contains(r#"<rect class="gp-bar" x="256.8" y="20.0" width="86.4" height="140.0" rx="2"><title>d3: 4</title></rect>"#));
    assert!(drawn.contains(r#"<rect class="gp-bar" x="376.8" y="160.0" width="86.4" height="0.0" rx="2"><title>d4: 0</title></rect>"#));
}

#[test]
fn up_to_four_columns_are_all_labelled_first_start_last_end() {
    let drawn = svg(&four_columns(), false);
    assert!(
        drawn.contains(concat!(
            r#"<text class="gp-muted" x="0.0" y="182" font-size="15" text-anchor="start">d1</text>"#,
            r#"<text class="gp-muted" x="180.0" y="182" font-size="15" text-anchor="middle">d2</text>"#,
            r#"<text class="gp-muted" x="300.0" y="182" font-size="15" text-anchor="middle">d3</text>"#,
            r#"<text class="gp-muted" x="480.0" y="182" font-size="15" text-anchor="end">d4</text></svg>"#,
        )),
        "{drawn}"
    );
}

#[test]
fn more_than_four_columns_label_only_the_first_middle_and_last() {
    let drawn = svg(&columns(7), false);
    assert!(drawn.contains(">d1</text>") && drawn.contains(">d4</text>") && drawn.contains(">d7</text>"), "{drawn}");
    assert_absent(&drawn, &[">d2</text>", ">d3</text>", ">d5</text>", ">d6</text>"].map(String::from));
}

#[test]
fn a_single_column_is_labelled_in_the_middle() {
    assert!(svg(&columns(1), false).contains(r#"x="240.0" y="182" font-size="15" text-anchor="middle">d1</text>"#));
}

#[test]
fn a_column_chart_of_zeros_shows_a_peak_of_one() {
    // Pinned from export/charts.py: with no value above zero the scale's peak is 1, and no item
    // displays it, so the peak label falls back to "1".
    let drawn = svg(&chart("z", ChartKind::Columns, vec![bar("a", 0.0), bar("b", 0.0)]), false);
    assert!(drawn.contains(r#"font-size="15">1</text><line"#), "{drawn}");
}

// --- the chart a single-chart format draws -------------------------------------------------------

fn charted() -> gitprompt_export::Report {
    report(vec![
        section(
            "S",
            vec![
                Block::Chart(chart("empty", ChartKind::Bars, vec![])),
                Block::Paragraph(Paragraph { text: "p".into(), muted: false }),
                Block::Chart(chart("a", ChartKind::Bars, vec![bar("x", 1.0)])),
            ],
        ),
        section("T", vec![Block::Chart(chart("b", ChartKind::Columns, vec![bar("y", 2.0)]))]),
    ])
}

#[test]
fn a_chart_is_picked_by_its_id() {
    assert_eq!(pick(&charted(), Some("b")).map(|c| c.id.clone()).ok(), Some("b".to_string()));
}

#[test]
fn without_an_id_or_with_an_unknown_one_the_first_chart_with_data_is_picked() {
    let r = charted();
    let picked = [pick(&r, None), pick(&r, Some("nope"))].map(|p| p.map(|c| c.id.clone()).ok());
    assert_eq!(picked, [Some("a".to_string()), Some("a".to_string())]);
}

#[test]
fn picking_a_chart_without_data_is_an_error() {
    assert!(matches!(pick(&charted(), Some("empty")), Err(ExportError::NoChart)));
}

#[test]
fn a_report_without_charts_has_nothing_to_pick() {
    assert!(matches!(pick(&report(vec![section("S", vec![])]), None), Err(ExportError::NoChart)));
}

#[test]
fn the_svg_file_is_an_xml_declaration_then_the_standalone_chart() {
    let file = svg_file(&charted(), Some("b")).expect("a chart");
    let c = chart("b", ChartKind::Columns, vec![bar("y", 2.0)]);
    assert_eq!(file, format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{}\n", svg(&c, true)));
}

#[test]
fn the_svg_file_of_a_report_without_chart_data_is_an_error() {
    let err = svg_file(&report(vec![]), None).unwrap_err();
    assert_eq!(err.to_string(), "this export has no chart with data");
}

#[test]
fn the_chart_list_names_the_charts_with_data_in_order() {
    assert_eq!(
        chart_list(&charted()),
        vec![ChartInfo { id: "a".into(), title: "Chart a".into() }, ChartInfo { id: "b".into(), title: "Chart b".into() }]
    );
}

// --- png -----------------------------------------------------------------------------------------

#[test]
fn a_png_chart_is_a_png_twice_the_size_of_its_svg() {
    let bytes = png(&three_bars(), 2).expect("a png");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
    assert_eq!((width, height), (784, 340));
}

#[test]
fn a_png_of_a_chart_without_items_is_an_error() {
    assert!(matches!(png(&chart("e", ChartKind::Bars, vec![]), 2), Err(ExportError::NoChart)));
}

#[test]
fn a_png_column_chart_is_twice_the_size_of_its_svg() {
    let bytes = png(&four_columns(), 2).expect("a png");
    let size = (u32::from_be_bytes(bytes[16..20].try_into().unwrap()), u32::from_be_bytes(bytes[20..24].try_into().unwrap()));
    assert_eq!(size, (1024, 516));
}

#[test]
fn a_png_at_scale_zero_is_a_raster_error() {
    assert!(matches!(png(&three_bars(), 0), Err(ExportError::Raster(_))));
}
