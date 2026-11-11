//! SQL built from small fragments. A [`Sql`] carries its text and the values of its `?`
//! placeholders in textual order, so fragments compose without counting parameters.

pub(crate) mod filter;

use rusqlite::types::Value;
use serde::Serialize;

use crate::error::DbResult;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Sql {
    text: String,
    params: Vec<Value>,
}

impl Sql {
    pub(crate) fn new(text: impl Into<String>) -> Self {
        Sql { text: text.into(), params: Vec::new() }
    }

    /// Append text, or another fragment with its parameters.
    pub(crate) fn push(mut self, other: impl Into<Sql>) -> Self {
        let other = other.into();
        self.text.push_str(&other.text);
        self.params.extend(other.params);
        self
    }

    /// Append a `?` bound to `value`.
    pub(crate) fn bind(mut self, value: impl Into<Value>) -> Self {
        self.text.push('?');
        self.params.push(value.into());
        self
    }

    /// The fragments joined by `separator`.
    pub(crate) fn join(parts: impl IntoIterator<Item = Sql>, separator: &str) -> Sql {
        parts.into_iter().enumerate().fold(Sql::default(), |acc, (i, part)| if i == 0 { part } else { acc.push(separator).push(part) })
    }

    /// Every condition must hold; no condition is `1=1`.
    pub(crate) fn all(conditions: impl IntoIterator<Item = Sql>) -> Sql {
        let joined = Sql::join(conditions, " and ");
        if joined.text.is_empty() { Sql::new("1=1") } else { joined }
    }

    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn params(&self) -> rusqlite::ParamsFromIter<std::slice::Iter<'_, Value>> {
        rusqlite::params_from_iter(self.params.iter())
    }
}

impl From<&str> for Sql {
    fn from(text: &str) -> Self {
        Sql::new(text)
    }
}

impl From<String> for Sql {
    fn from(text: String) -> Self {
        Sql::new(text)
    }
}

/// A list bound as one JSON array, read in SQL with `json_each(?)`: one placeholder whatever the
/// length (no variable limit, and the statement text stays the same for the statement cache).
pub(crate) fn json_list<T: Serialize>(values: &[T]) -> DbResult<Value> {
    Ok(Value::Text(serde_json::to_string(values)?))
}

/// `expr in (<the values>)`.
pub(crate) fn in_list<T: Serialize>(expr: impl Into<Sql>, values: &[T]) -> DbResult<Sql> {
    Ok(expr.into().push(" in (select value from json_each(").bind(json_list(values)?).push("))"))
}

/// A count or size as SQLite wants it (`i64`), saturating instead of wrapping.
pub(crate) fn int(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_keep_their_parameters_in_text_order() {
        let sql = Sql::new("a = ").bind(1).push(Sql::new(" and b = ").bind("x".to_owned())).push(" and c = ").bind(2.5);

        assert_eq!(sql.text(), "a = ? and b = ? and c = ?");
        assert_eq!(sql.params, vec![Value::Integer(1), Value::Text("x".into()), Value::Real(2.5)]);
    }

    #[test]
    fn no_condition_holds_always() {
        assert_eq!(Sql::all([]).text(), "1=1");
        assert_eq!(Sql::all([Sql::new("a"), Sql::new("b")]).text(), "a and b");
    }

    #[test]
    fn a_list_is_one_json_parameter() {
        let sql = in_list("i.origin", &["typed", "subagent"]).unwrap();

        assert_eq!(sql.text(), "i.origin in (select value from json_each(?))");
        assert_eq!(sql.params, vec![Value::Text(r#"["typed","subagent"]"#.into())]);
    }
}
