//! Query-string parsing of the prompt filters (port of api/filters.py): repeated keys make lists,
//! empty values are ignored, 0 is a value, local times become UTC, invalid values are 422.

use gitprompt_core::spec::Origin;
use gitprompt_server::ApiError;
use gitprompt_server::model::ExportFormat;
use gitprompt_server::query::{dry_run, export_query, pairs, prompt_filter, prompt_query, stats_query};
use gitprompt_store::filter::{Has, Outcome, PromptFilter};
use gitprompt_store::stats::GroupBy;
use jiff::civil;
use jiff::tz::TimeZone;
use pretty_assertions::assert_eq;

fn parse(query: &str) -> Result<PromptFilter, ApiError> {
    prompt_filter(&pairs(Some(query)))
}

#[test]
fn pairs_are_percent_decoded_in_order() {
    let parsed = pairs(Some("q=fix%20the+bug&origin=typed&origin=slash_command"));
    let expected = [("q", "fix the bug"), ("origin", "typed"), ("origin", "slash_command")].map(|(k, v)| (k.to_string(), v.to_string()));
    assert_eq!(parsed, expected);
    assert_eq!(pairs(None), vec![]);
}

#[test]
fn repeated_keys_build_lists() {
    let filter = parse("origin=typed&origin=slash_command&session=s1&session=s2&has=code&has=no_follow_up&outcome=done").expect("valid");
    assert_eq!(filter.origin, [Origin::Typed, Origin::SlashCommand]);
    assert_eq!(filter.session, ["s1", "s2"]);
    assert_eq!(filter.has, [Has::Code, Has::NoFollowUp]);
    assert_eq!(filter.outcome, [Outcome::Done]);
}

#[test]
fn no_query_is_no_filter() {
    assert_eq!(parse("").expect("valid"), PromptFilter::default());
}

#[test]
fn empty_values_and_unknown_keys_are_ignored() {
    assert_eq!(parse("q=&source=&origin=&nope=1").expect("valid"), PromptFilter::default());
}

#[test]
fn hour_zero_is_a_value() {
    assert_eq!(parse("hour=0").expect("valid").hour, Some(0));
}

#[test]
fn numbers_out_of_range_or_malformed_are_422() {
    let queries = ["weekday=0", "weekday=8", "hour=24", "hour=-1", "before=x", "starred=maybe", "since=yesterday"];
    let accepted: Vec<&str> = queries.iter().copied().filter(|q| !matches!(parse(q), Err(ApiError::Unprocessable(_)))).collect();
    assert_eq!(accepted, Vec::<&str>::new());
}

#[test]
fn unknown_enum_values_are_422_naming_the_key() {
    let error = parse("origin=nope").expect_err("invalid origin");
    assert!(matches!(&error, ApiError::Unprocessable(m) if m.contains("origin")), "{error:?}");
    assert!(matches!(parse("has=nope"), Err(ApiError::Unprocessable(_))));
    assert!(matches!(parse("outcome=nope"), Err(ApiError::Unprocessable(_))));
}

#[test]
fn an_instant_with_an_offset_is_stored_as_utc_with_microseconds() {
    assert_eq!(parse("since=2026-11-03T01:00:00%2B02:00").expect("valid").since.as_deref(), Some("2026-11-02T23:00:00.000000+00:00"));
    assert_eq!(parse("until=2026-11-03T00:00:00Z").expect("valid").until.as_deref(), Some("2026-11-03T00:00:00.000000+00:00"));
}

#[test]
fn an_instant_without_an_offset_is_local_time() {
    let naive: civil::DateTime = "2026-11-03T00:00:00".parse().expect("naive");
    let utc = TimeZone::system().to_ambiguous_zoned(naive).unambiguous().expect("unambiguous").timestamp();
    let expected = utc.strftime("%Y-%m-%dT%H:%M:%S%.6f+00:00").to_string();
    assert_eq!(parse("until=2026-11-03T00:00:00").expect("valid").until, Some(expected));
}

#[test]
fn starred_and_the_paging_cursor_are_typed() {
    let filter = parse("starred=true&before=42&tag=release&repo=/r/app").expect("valid");
    assert_eq!(
        (filter.starred, filter.before, filter.tag.as_deref(), filter.repo.as_deref()),
        (Some(true), Some(42), Some("release"), Some("/r/app"))
    );
}

#[test]
fn the_prompt_limit_defaults_to_50_and_must_be_1_to_200() {
    assert_eq!(prompt_query(&pairs(Some("q=x"))).expect("valid").limit, 50);
    assert_eq!(prompt_query(&pairs(Some("limit=200"))).expect("valid").limit, 200);
    assert!(matches!(prompt_query(&pairs(Some("limit=0"))), Err(ApiError::Unprocessable(_))));
    assert!(matches!(prompt_query(&pairs(Some("limit=201"))), Err(ApiError::Unprocessable(_))));
}

#[test]
fn stats_queries_parse_grouping_and_refuse_split_without_group() {
    let query = stats_query(&pairs(Some("group_by=day_hour&split_by=tool_kind&limit=10&source=codex"))).expect("valid");
    assert_eq!(
        (query.group_by, query.split_by, query.limit, query.filter.source.as_deref()),
        (Some(GroupBy::DayHour), Some(GroupBy::ToolKind), 10, Some("codex"))
    );
    assert_eq!(stats_query(&pairs(None)).expect("valid").limit, 1000);
    assert!(matches!(stats_query(&pairs(Some("split_by=repo"))), Err(ApiError::Unprocessable(_))));
    assert!(matches!(stats_query(&pairs(Some("limit=10001"))), Err(ApiError::Unprocessable(_))));
}

#[test]
fn export_options_default_to_the_most_private_choice() {
    let query = export_query(&pairs(Some("id=s1"))).expect("valid");
    assert_eq!(
        (query.format, query.prompt_text, query.relative_paths, query.drop_identity, query.drop_raw, query.download),
        (ExportFormat::Html, false, true, true, true, false)
    );
    assert!(matches!(export_query(&pairs(Some("format=pdf"))), Err(ApiError::Unprocessable(_))));
}

#[test]
fn dry_run_takes_the_routes_default_unless_given() {
    assert_eq!((dry_run(&pairs(None), true).expect("valid"), dry_run(&pairs(None), false).expect("valid")), (true, false));
    assert_eq!(dry_run(&pairs(Some("dry_run=false")), true).expect("valid"), false);
    assert!(matches!(dry_run(&pairs(Some("dry_run=perhaps")), true), Err(ApiError::Unprocessable(_))));
}
