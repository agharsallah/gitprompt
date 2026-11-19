//! Where a [`Target`] opens in the dashboard, as a path on the daemon's origin: the Rust twin of
//! `targetHref` in `web/src/lib/shared/links.ts` (the tray and native notifications have no web
//! view to ask). Keep the two in step; the cases below are the ones `links.test.ts` checks.

use gitprompt_server::model::Target;

/// The anchor of the Overview's possible leaks panel (`LEAKS_ANCHOR` in links.ts).
const LEAKS_ANCHOR: &str = "leaks";

/// The dashboard path of `target`, e.g. `/prompts?id=42`.
pub fn route(target: &Target) -> String {
    match target {
        Target::Overview { range: Some(range) } => format!("/?range={}", encode(range_id(*range))),
        Target::Overview { range: None } => "/".to_owned(),
        Target::Prompts { q } => format!("/prompts?q={}", encode(q)),
        Target::Prompt { id } => format!("/prompts?id={id}"),
        Target::Session { id } => format!("/sessions/{}", encode(id)),
        Target::Leaks => format!("/#{LEAKS_ANCHOR}"),
        Target::Source { id } => format!("/sources#tool-{}", encode(id)),
    }
}

/// A range as the dashboard's query names it (`24h`, `7d`...): its serde name.
fn range_id(range: gitprompt_core::preferences::RangeId) -> &'static str {
    use gitprompt_core::preferences::RangeId;
    match range {
        RangeId::Day => "24h",
        RangeId::Week => "7d",
        RangeId::Month => "30d",
        RangeId::All => "all",
    }
}

/// Percent-encoding of everything but the unreserved characters (RFC 3986), byte by byte: safe in
/// a path segment, a query value and a `data:` URL alike.
pub fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gitprompt_core::preferences::RangeId;
    use pretty_assertions::assert_eq;

    #[test]
    fn every_target_opens_where_the_dashboard_links_it() {
        let cases = [
            (Target::Overview { range: None }, "/"),
            (Target::Overview { range: Some(RangeId::Day) }, "/?range=24h"),
            (Target::Prompts { q: "fix the parser & lexer".into() }, "/prompts?q=fix%20the%20parser%20%26%20lexer"),
            (Target::Prompt { id: 42 }, "/prompts?id=42"),
            (Target::Session { id: "a/b c".into() }, "/sessions/a%2Fb%20c"),
            (Target::Leaks, "/#leaks"),
            (Target::Source { id: "claude-code".into() }, "/sources#tool-claude-code"),
        ];
        for (target, path) in cases {
            assert_eq!(route(&target), path, "{target:?}");
        }
    }

    #[test]
    fn a_range_is_named_as_on_the_wire() {
        for range in [RangeId::Day, RangeId::Week, RangeId::Month, RangeId::All] {
            assert_eq!(serde_json::to_value(range).unwrap(), range_id(range));
        }
    }
}
