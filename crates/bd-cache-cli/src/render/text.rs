//! `--format text`: the same `data` for a person (contract §2).
//!
//! Objects are `key: value` lines, nested values are indented by two spaces,
//! and arrays are `- ` items; an error is its code, message and suggested
//! action. The shape is not covered by semver (contract §11), so a program
//! reads the JSON envelope instead.

use std::fmt::Write as _;

use bd_cache_api::ApiError;
use serde_json::Value;

/// The text of a result.
#[must_use]
pub(crate) fn render(result: &Result<Value, ApiError>) -> String {
    match result {
        Ok(data) => {
            let mut out = String::new();
            value(&mut out, data, 0);
            if out.is_empty() {
                out.push_str("ok\n");
            }
            out
        }
        Err(e) => error(e),
    }
}

/// An error as text: `error: CODE: message`, then the action and details.
#[must_use]
pub(crate) fn error(e: &ApiError) -> String {
    let mut out = format!("error: {}: {}\n", e.code(), e.message());
    if let Some(action) = e.suggested_action() {
        let _ = writeln!(out, "suggested action: {action}");
    }
    if !e.details().is_empty() {
        out.push_str("details:\n");
        value(&mut out, &Value::Object(e.details().clone()), 1);
    }
    out
}

fn scalar(v: &Value) -> String {
    match v {
        Value::Null => "-".to_owned(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn is_scalar(v: &Value) -> bool {
    !matches!(v, Value::Object(_) | Value::Array(_))
        || matches!(v, Value::Object(m) if m.is_empty())
        || matches!(v, Value::Array(a) if a.is_empty())
}

fn inline(v: &Value) -> String {
    match v {
        Value::Object(_) => "{}".to_owned(),
        Value::Array(_) => "[]".to_owned(),
        other => scalar(other),
    }
}

fn value(out: &mut String, v: &Value, depth: usize) {
    let pad = "  ".repeat(depth);
    match v {
        Value::Object(map) => {
            for (k, item) in map {
                if is_scalar(item) {
                    let _ = writeln!(out, "{pad}{k}: {}", inline(item));
                } else {
                    let _ = writeln!(out, "{pad}{k}:");
                    value(out, item, depth + 1);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                if is_scalar(item) {
                    let _ = writeln!(out, "{pad}- {}", inline(item));
                } else {
                    let _ = writeln!(out, "{pad}-");
                    value(out, item, depth + 1);
                }
            }
        }
        other => {
            let _ = writeln!(out, "{pad}{}", scalar(other));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn data_renders_as_indented_lines() {
        let text = render(&Ok(json!({"items": [{"id": "b1"}, "x"]})));
        assert_eq!(text, "items:\n  -\n    id: b1\n  - x\n");
        assert_eq!(render(&Ok(json!({"outage": null}))), "outage: -\n");
    }

    #[test]
    fn an_error_renders_its_code_and_action() {
        let text = render(&Err(ApiError::not_cached("b1")));
        assert!(text.starts_with("error: BD_CACHE.NOT_CACHED: "), "{text}");
        assert!(text.contains("suggested action: retry"), "{text}");
    }
}
