//! Partial-equality matcher for assertion expectations.
//!
//! `expected` is a `toml::Value` taken straight from the spec. `actual` is the
//! engine event/snapshot serialized to `serde_json::Value`. We allow:
//! - missing keys in `expected` (only specified keys must match)
//! - exact string/numeric/bool match
//! - nested objects recursed by key
//! - arrays compared element-by-element when both sides are arrays

use serde_json::Value as Json;
use toml::Value as Toml;

pub fn partial_eq(expected: &Toml, actual: &Json) -> bool {
    match (expected, actual) {
        (Toml::String(a), Json::String(b)) => a == b,
        (Toml::Integer(a), Json::Number(b)) => b.as_i64() == Some(*a),
        (Toml::Float(a), Json::Number(b)) => b.as_f64().is_some_and(|bf| (bf - a).abs() < 1e-6),
        (Toml::Boolean(a), Json::Bool(b)) => a == b,
        (Toml::Array(a), Json::Array(b)) => {
            a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| partial_eq(x, y))
        }
        (Toml::Table(a), Json::Object(b)) => a
            .iter()
            .all(|(k, v)| b.get(k).is_some_and(|bv| partial_eq(v, bv))),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_partial_object() {
        let expected: Toml = toml::from_str(r#"x = 50"#).unwrap();
        let actual: Json = serde_json::json!({"x": 50, "y": 60});
        assert!(partial_eq(&expected, &actual));
    }

    #[test]
    fn rejects_mismatched_value() {
        let expected: Toml = toml::from_str(r#"x = 50"#).unwrap();
        let actual: Json = serde_json::json!({"x": 51});
        assert!(!partial_eq(&expected, &actual));
    }

    #[test]
    fn matches_nested_object() {
        let expected: Toml = toml::from_str(
            r#"
[selection]
x = 10
"#,
        )
        .unwrap();
        let actual: Json = serde_json::json!({"selection": {"x": 10, "y": 20}});
        assert!(partial_eq(&expected, &actual));
    }
}
