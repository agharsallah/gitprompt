//! A regular expression from a spec.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Error, Result};

/// A regular expression from a spec, with Python `re` features specs rely on (look-around).
/// Compares and serializes as its source text.
#[derive(Clone, Debug)]
pub struct Pattern(pub fancy_regex::Regex);

impl Pattern {
    /// Compile `source`; an invalid pattern is an `Error::Spec`.
    pub fn new(source: &str) -> Result<Self> {
        fancy_regex::Regex::new(source).map(Pattern).map_err(|e| Error::Spec(format!("invalid regular expression {source:?}: {e}")))
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
    /// Whether the pattern matches at the very start of `text` (Python `re.match`).
    /// A regex runtime error counts as no match.
    pub fn matches_start(&self, text: &str) -> bool {
        // The leftmost match starts at 0 whenever any match does.
        match self.0.find(text) {
            Ok(found) => found.is_some_and(|m| m.start() == 0),
            Err(e) => self.failed(&e),
        }
    }
    /// The first capture group of every match (`tool_files`).
    /// Matches without a first group are skipped; a regex runtime error ends the list.
    pub fn first_groups(&self, text: &str) -> Vec<String> {
        self.0
            .captures_iter(text)
            .map_while(|captures| captures.map_err(|e| self.failed(&e)).ok())
            .filter_map(|captures| captures.get(1).map(|group| group.as_str().to_owned()))
            .collect()
    }
    /// Whether the pattern matches anywhere in `text` (Python `re.search`), e.g. a [`Spec::marker`]
    /// against a hook command line. A regex runtime error (backtrack limit) counts as no match.
    pub fn is_match(&self, text: &str) -> bool {
        self.0.is_match(text).unwrap_or_else(|e| self.failed(&e))
    }

    /// Log a regex runtime error; the search counts as finding nothing.
    fn failed(&self, error: &fancy_regex::Error) -> bool {
        tracing::warn!(pattern = self.as_str(), %error, "pattern failed at run time; treated as no match");
        false
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Serialize for Pattern {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Pattern {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let source = String::deserialize(d)?;
        Pattern::new(&source).map_err(serde::de::Error::custom)
    }
}
