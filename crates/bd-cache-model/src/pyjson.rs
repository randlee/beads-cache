//! Python-compatible JSON text: `json.dumps(value, sort_keys=True)`.
//!
//! The BD-CACHE v3 journal stores `args_json` and `precondition_json` as
//! written by Python's `json.dumps(..., sort_keys=True)`: keys sorted by code
//! point, `", "` and `": "` separators, and `ensure_ascii` escaping (every
//! non-ASCII character as a lower-case `\uXXXX`, astral characters as a
//! surrogate pair). Writing the same text from Rust keeps a file shared with
//! the Python implementation byte-identical (REQ-BDC-0804).

use serde_json::Value;

/// Renders `v` exactly as Python's `json.dumps(v, sort_keys=True)` does.
///
/// Floats render with `serde_json`'s shortest form, which can differ from
/// Python's `repr`; journal args never hold floats (they are `BAD_ARGS`).
///
/// # Examples
///
/// ```
/// let v = serde_json::json!({"b": [1, true], "a": "é"});
/// assert_eq!(bd_cache_model::python_json(&v), r#"{"a": "\u00e9", "b": [1, true]}"#);
/// ```
#[must_use]
pub fn python_json(v: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, v);
    out
}

fn write_value(out: &mut String, v: &Value) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_str(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, k) in keys.into_iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_str(out, k);
                out.push_str(": ");
                if let Some(item) = map.get(k) {
                    write_value(out, item);
                }
            }
            out.push('}');
        }
    }
}

/// Pushes `\uXXXX` for one UTF-16 code unit, four lowercase hex digits, as
/// Python's `json.dumps` writes it.
fn push_unicode_escape(out: &mut String, unit: u16) {
    out.push_str("\\u");
    for byte in unit.to_be_bytes() {
        out.push(hex_digit(byte >> 4));
        out.push(hex_digit(byte & 0x0f));
    }
}

/// The lowercase hex digit of the low nibble of `n` (the high nibble is
/// ignored, so the function is total).
#[expect(
    clippy::indexing_slicing,
    reason = "the index is masked to 0..16, the length of the 16-byte table"
)]
fn hex_digit(n: u8) -> char {
    char::from(b"0123456789abcdef"[usize::from(n & 0x0f)])
}

fn write_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            c => {
                let mut units = [0u16; 2];
                for u in c.encode_utf16(&mut units) {
                    push_unicode_escape(out, *u);
                }
            }
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_python_escaping_and_separators() {
        let v = serde_json::json!({
            "z": {}, "a": [], "q": "tab\tquote\"back\\nl\nbell\u{7}del\u{7f}",
            "emoji": "😀", "n": -3
        });
        assert_eq!(
            python_json(&v),
            r#"{"a": [], "emoji": "\ud83d\ude00", "n": -3, "q": "tab\tquote\"back\\nl\nbell\u0007del\u007f", "z": {}}"#
        );
    }

    #[test]
    fn hex_digit_is_total_and_reads_the_low_nibble() {
        let digits: String = (0u8..16).map(hex_digit).collect();
        assert_eq!(digits, "0123456789abcdef");
        for n in 0..=u8::MAX {
            assert_eq!(hex_digit(n), hex_digit(n & 0x0f), "{n}");
        }
    }
}
