//! What "compacted" means on the wire: null, `""`, `[]`, `{}`, `false` and zero are left out, so
//! summaries show what happened. One rule for JSON values ([`crate::json::compact`]) and for typed
//! wire structs, whose fields say `#[serde(default, skip_serializing_if = "Blank::is_blank")]`
//! (the `default` also makes the generated TypeScript field optional). JSON values implement it
//! next to their type, in `json.rs`.

use indexmap::IndexMap;

/// A value that compaction leaves out.
pub trait Blank {
    fn is_blank(&self) -> bool;
}

macro_rules! zero_is_blank {
    ($($t:ty),*) => {$(
        impl Blank for $t {
            fn is_blank(&self) -> bool {
                *self == 0
            }
        }
    )*};
}

zero_is_blank!(i64, u64, usize);

impl Blank for f64 {
    fn is_blank(&self) -> bool {
        *self == 0.0
    }
}

impl Blank for bool {
    fn is_blank(&self) -> bool {
        !*self
    }
}

impl Blank for String {
    fn is_blank(&self) -> bool {
        self.is_empty()
    }
}

impl<T: Blank> Blank for Option<T> {
    fn is_blank(&self) -> bool {
        self.as_ref().is_none_or(Blank::is_blank)
    }
}

impl<T> Blank for Vec<T> {
    fn is_blank(&self) -> bool {
        self.is_empty()
    }
}

impl<K, V> Blank for IndexMap<K, V> {
    fn is_blank(&self) -> bool {
        self.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Blank;

    #[test]
    fn blank_values_are_the_ones_compaction_drops() {
        for value in [json!(null), json!(false), json!(0), json!(0.0), json!(""), json!([]), json!({})] {
            assert!(value.is_blank(), "{value} is blank");
        }
        for value in [json!(true), json!(1), json!(-0.5), json!("a"), json!([0]), json!({"a": 0})] {
            assert!(!value.is_blank(), "{value} is not blank");
        }
        assert!(Some(String::new()).is_blank());
        assert!(Some(0.0_f64).is_blank());
        assert!(!Some(2_u64).is_blank());
        assert!(None::<String>.is_blank());
    }
}
