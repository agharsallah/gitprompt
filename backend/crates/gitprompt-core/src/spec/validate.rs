//! What deserializing a spec can't check: rule conditions, surface ids and names, injected tags.

use super::expand::{Fill, fill};
use super::{Spec, When};
use crate::Json;

/// What deserializing can't check: rule conditions, surface ids and names, injected tags.
pub(super) fn validate(spec: &Spec) -> std::result::Result<(), String> {
    let whens = spec.origin.iter().map(|r| &r.when).chain(spec.surface.iter().map(|r| &r.when));
    for when in whens {
        check_when(when)?;
    }
    for rule in &spec.event_names {
        if rule.equals.as_ref().is_some_and(|v| !is_scalar(v)) {
            return Err(format!("event_names {:?}: `equals` must be a string, number or boolean", rule.from));
        }
    }
    let named: Vec<&str> = spec.surface.iter().map(|r| r.surface.as_str()).chain(spec.captures.keys().map(String::as_str)).collect();
    for id in spec.surfaces.keys().map(String::as_str).chain(named.iter().copied()) {
        if !is_surface_id(id) {
            return Err(format!("surface id {id:?} must match ^[a-z][a-z0-9-]*$"));
        }
    }
    let mut unknown: Vec<&str> = named.into_iter().filter(|id| !spec.surfaces.contains_key(*id)).collect();
    unknown.sort_unstable();
    unknown.dedup();
    if !unknown.is_empty() {
        return Err(format!("surfaces without a name in `surfaces`: {unknown:?}"));
    }
    if let Some(tag) = spec.injected.iter().find(|tag| !is_tag(tag)) {
        return Err(format!("injected tag {tag:?} must match ^[A-Za-z][\\w:-]*$"));
    }
    check_resume(spec)
}

/// The `resume.command` template fills with a value for every known placeholder, so what remains
/// is an unknown placeholder or a stray brace.
fn check_resume(spec: &Spec) -> std::result::Result<(), String> {
    let Some(resume) = &spec.resume else { return Ok(()) };
    match fill(&resume.command, |_| Some("x".into())) {
        Err(Fill::Invalid(why)) => Err(why),
        _ if resume.command.trim().is_empty() => Err("resume `command` is empty".into()),
        _ => Ok(()),
    }
}

fn check_when(when: &When) -> std::result::Result<(), String> {
    let has_condition = when.event.is_some()
        || when.field.is_some()
        || when.path.is_some()
        || when.text_prefix.is_some()
        || when.text_regex.is_some()
        || when.text_block.is_some();
    if !has_condition {
        return Err("a rule needs at least one condition".into());
    }
    match when.text_block.as_deref() {
        Some([]) => return Err("`text_block` needs at least one tag".into()),
        Some(tags) => {
            if let Some(tag) = tags.iter().find(|tag| !is_tag(tag)) {
                return Err(format!("text_block tag {tag:?} must match ^[A-Za-z][\\w:-]*$"));
            }
        }
        None => {}
    }
    match &when.equals {
        Some(_) if when.path.is_none() => Err("`equals` needs a `path`".into()),
        Some(value) if !is_scalar(value) => Err("`equals` must be a string, number or boolean".into()),
        _ => Ok(()),
    }
}

fn is_scalar(value: &Json) -> bool {
    matches!(value, Json::String(_) | Json::Number(_) | Json::Bool(_))
}

/// `^[a-z][a-z0-9-]*$`
fn is_surface_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|c| c.is_ascii_lowercase()) && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `^[A-Za-z][\w:-]*$`
fn is_tag(tag: &str) -> bool {
    let mut chars = tag.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic()) && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | ':' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_ids_and_tags_follow_their_patterns() {
        assert!(is_surface_id("my-app2") && !is_surface_id("App") && !is_surface_id("1app") && !is_surface_id(""));
        assert!(is_tag("ns:tag") && is_tag("A_b") && !is_tag("1abc") && !is_tag("a b") && !is_tag(""));
    }
}
