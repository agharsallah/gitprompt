//! The format table (export/formats.py and each format's registration): names, labels, media
//! types, extensions, and which formats draw a single chart.

use pretty_assertions::assert_eq;

use gitprompt_export::Format;

#[test]
fn formats_are_named_by_their_query_values() {
    assert_eq!(Format::ALL.map(Format::name), ["md", "html", "csv", "json", "svg", "png"]);
}

#[test]
fn every_query_value_parses_back_to_its_format() {
    let parsed = ["md", "html", "csv", "json", "svg", "png"].map(Format::parse);
    assert_eq!(parsed, Format::ALL.map(Some));
}

#[test]
fn an_unknown_or_differently_cased_format_does_not_parse() {
    assert_eq!(["pdf", "MD", "", "markdown"].map(Format::parse), [None, None, None, None]);
}

#[test]
fn formats_are_labelled_as_the_dashboard_lists_them() {
    assert_eq!(Format::ALL.map(Format::label), ["Markdown", "HTML", "CSV", "JSON", "Chart (SVG)", "Chart (PNG)"]);
}

#[test]
fn formats_carry_their_media_types() {
    assert_eq!(
        Format::ALL.map(Format::media_type),
        [
            "text/markdown; charset=utf-8",
            "text/html; charset=utf-8",
            "text/csv; charset=utf-8",
            "application/json",
            "image/svg+xml",
            "image/png",
        ]
    );
}

#[test]
fn formats_carry_their_file_extensions() {
    assert_eq!(Format::ALL.map(Format::extension), [".md", ".html", ".csv", ".json", ".svg", ".png"]);
}

#[test]
fn only_svg_and_png_draw_a_single_chart() {
    assert_eq!(Format::ALL.map(Format::single_chart), [false, false, false, false, true, true]);
}

#[test]
fn every_format_but_png_is_text() {
    assert_eq!(Format::ALL.map(Format::is_text), [true, true, true, true, true, false]);
}

#[test]
fn a_format_serializes_as_its_query_value() {
    assert_eq!(serde_json::to_string(&Format::ALL).unwrap(), r#"["md","html","csv","json","svg","png"]"#);
}
