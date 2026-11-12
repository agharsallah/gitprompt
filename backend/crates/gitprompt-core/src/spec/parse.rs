//! One spec file's YAML, on top of the shared defaults, into a checked [`Spec`].

use std::sync::LazyLock;

use serde_norway::{Mapping as YamlMap, Value as Yaml};

use super::validate::validate;
use super::{BUNDLED, DEFAULTS, Spec};
use crate::{Error, Result};

/// One spec file on top of the shared defaults. Merge rules: `map`, `tool_kinds` replace defaults per
/// key; `origin`, `surface` = the spec's own rules then the defaults'; `injected` = defaults then own,
/// deduplicated; `usage` and `answer` replace the defaults whole; `shown`, `ignored`, `surfaces` add to
/// the defaults' keys. Every surface named by a `surface` rule or `captures` must be in `surfaces`.
/// Errors are `Error::Spec` naming the file ("is not valid YAML", "must be a YAML mapping",
/// "is invalid").
pub fn parse_spec(name: &str, text: &str, defaults: &serde_norway::Mapping) -> Result<Spec> {
    let mut data = parse_mapping(name, text)?;
    merge_defaults(&mut data, defaults);
    let invalid = |why: String| Error::Spec(format!("source spec {name} is invalid: {why}"));
    let spec: Spec = serde_norway::from_value(Yaml::Mapping(data)).map_err(|e| invalid(e.to_string()))?;
    validate(&spec).map_err(invalid)?;
    Ok(spec)
}

/// The bundled `_defaults.yaml`, parsed. It is compiled in and checked by the tests; were it ever
/// broken, specs load without shared defaults (and the error is logged) rather than not at all.
pub fn bundled_defaults() -> serde_norway::Mapping {
    static DEFAULTS_MAP: LazyLock<YamlMap> = LazyLock::new(|| {
        parse_mapping(DEFAULTS, bundled_text(DEFAULTS).unwrap_or_default()).unwrap_or_else(|e| {
            tracing::error!(error = %e, "the bundled source spec defaults can't be read");
            YamlMap::new()
        })
    });
    DEFAULTS_MAP.clone()
}

pub(super) fn bundled_text(name: &str) -> Option<&'static str> {
    BUNDLED.iter().find(|(bundled, _)| *bundled == name).map(|(_, text)| *text)
}

/// YAML text that must hold a mapping (anchors resolved, `<<` merge keys applied).
fn parse_mapping(name: &str, text: &str) -> Result<YamlMap> {
    let not_yaml = |e: serde_norway::Error| Error::Spec(format!("source spec {name} is not valid YAML: {e}"));
    let mut value: Yaml = serde_norway::from_str(text).map_err(not_yaml)?;
    value.apply_merge().map_err(not_yaml)?;
    match value {
        Yaml::Mapping(map) => Ok(map),
        _ => Err(Error::Spec(format!("source spec {name} must be a YAML mapping"))),
    }
}

/// The merge rules of [`parse_spec`]. A key whose value is "empty" (null, `{}`, `[]`, `""`, `false`,
/// 0) counts as absent. A value of the wrong type is kept as is, so deserializing reports it.
fn merge_defaults(data: &mut YamlMap, defaults: &YamlMap) {
    for key in ["map", "tool_kinds", "shown", "ignored", "surfaces"] {
        let merged = merge_mappings(given(defaults, key), given(data, key));
        data.insert(key.into(), merged);
    }
    for key in ["origin", "surface"] {
        let merged = concat(given(data, key), given(defaults, key), false);
        data.insert(key.into(), merged);
    }
    let injected = concat(given(defaults, "injected"), given(data, "injected"), true);
    data.insert("injected".into(), injected);
    for key in ["usage", "answer"] {
        let whole = given(data, key).or_else(|| given(defaults, key)).cloned().unwrap_or_else(|| Yaml::Mapping(YamlMap::new()));
        data.insert(key.into(), whole);
    }
}

/// The value at `key`, unless it is empty (Python truthiness, as the original port relied on).
fn given<'a>(map: &'a YamlMap, key: &str) -> Option<&'a Yaml> {
    map.get(key).filter(|value| match value {
        Yaml::Null => false,
        Yaml::Bool(b) => *b,
        Yaml::Number(n) => n.as_f64() != Some(0.0),
        Yaml::String(s) => !s.is_empty(),
        Yaml::Sequence(items) => !items.is_empty(),
        Yaml::Mapping(map) => !map.is_empty(),
        Yaml::Tagged(_) => true,
    })
}

/// `base` with `own`'s keys replacing or added (a key keeps its position in `base`).
fn merge_mappings(base: Option<&Yaml>, own: Option<&Yaml>) -> Yaml {
    let mut merged = match base {
        Some(Yaml::Mapping(base)) => base.clone(),
        _ => YamlMap::new(),
    };
    match own {
        Some(Yaml::Mapping(own)) => merged.extend(own.iter().map(|(k, v)| (k.clone(), v.clone()))),
        Some(other) => return other.clone(),
        None => {}
    }
    Yaml::Mapping(merged)
}

/// `first`'s items then `second`'s (without repeats when `dedup`).
fn concat(first: Option<&Yaml>, second: Option<&Yaml>, dedup: bool) -> Yaml {
    let mut items: Vec<Yaml> = Vec::new();
    for part in [first, second].into_iter().flatten() {
        let Yaml::Sequence(part) = part else { return part.clone() };
        for item in part {
            if !(dedup && items.contains(item)) {
                items.push(item.clone());
            }
        }
    }
    Yaml::Sequence(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_yaml_values_count_as_absent() {
        let map: YamlMap = serde_norway::from_str("a: ~\nb: {}\nc: []\nd: ''\ne: 0\nf: false\ng: [1]\n").unwrap();
        let present: Vec<&str> = ["a", "b", "c", "d", "e", "f", "g"].into_iter().filter(|k| given(&map, k).is_some()).collect();
        assert_eq!(present, ["g"]);
    }
}
