//! Environment variables as a value. Every component reads the environment through an [`Env`], never
//! through `std::env` directly, so tests (which run in parallel threads) inject their own and never
//! mutate the process environment.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Env(BTreeMap<String, String>);

impl Env {
    /// A snapshot of the process environment.
    pub fn process() -> Self {
        todo!()
    }

    /// An environment holding exactly these variables (tests).
    pub fn from_pairs<K: Into<String>, V: Into<String>>(pairs: impl IntoIterator<Item = (K, V)>) -> Self {
        Env(pairs.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
    }

    /// The variable's value; an unset or empty variable is `None`.
    pub fn get(&self, _name: &str) -> Option<&str> {
        todo!()
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0.insert(name.into(), value.into());
    }
}
