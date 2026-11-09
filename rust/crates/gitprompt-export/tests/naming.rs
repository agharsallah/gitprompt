//! Export file names (`_filename` in api/routes/export.py): `gitprompt-<kind>[-<title>]-<day>.<ext>`,
//! ASCII only so it is safe in a Content-Disposition header.

mod common;

use jiff::civil::{Date, date};
use pretty_assertions::assert_eq;

use common::*;
use gitprompt_export::naming::{filename, slug};
use gitprompt_export::{Format, Kind, Report};

fn titled(kind: Kind, title: &str) -> Report {
    Report { kind, title: title.into(), ..report(vec![]) }
}

fn day() -> Date {
    date(2026, 3, 4)
}

#[test]
fn a_file_is_named_by_kind_title_and_day() {
    assert_eq!(filename(&titled(Kind::Session, TITLE), Format::Markdown, day()), "gitprompt-session-wire-the-shop-2026-03-04.md");
}

#[test]
fn a_project_file_starts_with_its_project_name() {
    assert_eq!(filename(&titled(Kind::Project, "app"), Format::Markdown, day()), "gitprompt-project-app-2026-03-04.md");
}

#[test]
fn a_title_starting_with_the_kind_does_not_repeat_it() {
    assert_eq!(filename(&titled(Kind::Session, "Session s-0001-0"), Format::Html, day()), "gitprompt-session-s-0001-0-2026-03-04.html");
}

#[test]
fn a_title_equal_to_the_kind_is_left_out() {
    let names = [
        filename(&titled(Kind::Overview, "Overview"), Format::Csv, day()),
        filename(&titled(Kind::Prompts, "Prompts"), Format::Json, day()),
    ];
    assert_eq!(names, ["gitprompt-overview-2026-03-04.csv", "gitprompt-prompts-2026-03-04.json"]);
}

#[test]
fn a_title_without_ascii_letters_or_digits_is_left_out() {
    assert_eq!(filename(&titled(Kind::Session, "☕ — ☕"), Format::Svg, day()), "gitprompt-session-2026-03-04.svg");
}

#[test]
fn a_chart_image_gets_the_png_extension() {
    assert_eq!(filename(&titled(Kind::Overview, "Overview"), Format::Png, day()), "gitprompt-overview-2026-03-04.png");
}

#[test]
fn a_slug_keeps_only_lowercase_ascii_letters_and_digits_joined_by_single_dashes() {
    assert_eq!(slug(&titled(Kind::Project, "  Café — déjà vu! v2 ")), "caf-d-j-vu-v2");
}

#[test]
fn a_slug_is_cut_to_forty_characters() {
    assert_eq!(slug(&titled(Kind::Project, &"a".repeat(50))), "a".repeat(40));
}
