//! argv to the JSON request that `bd-cache-api`'s decoder reads (contract §1 rule 6).
//!
//! The CLI validates nothing a request member means: each argument is put at
//! its member path as given ([`ArgSpec::member`]), integers as JSON numbers
//! when they parse and as text otherwise, so the decoder decides the code
//! (`BAD_ARGS`, `UNSUPPORTED`) exactly as for the bindings. Only the shape of
//! argv is the CLI's: two arguments that fill one member are `USAGE`, and so
//! is `--input` with any request argument. Invalid JSON in `--input`,
//! `--patch` or `--set-metadata` is `BAD_ARGS` (contract §3).

use std::io::Read as _;

use bd_cache_api::ApiError;
use bd_cache_model::MAX_INGEST_BYTES;
use clap::ArgMatches;
use clap::parser::ValueSource;
use serde_json::{Map, Value};

use crate::args::{ArgSpec, CommandSpec, Shape, Ty};

/// The request of `spec` from its argv matches.
///
/// # Errors
///
/// `USAGE` when two arguments fill the same member; `BAD_ARGS` for a JSON
/// argument that does not parse.
pub(crate) fn from_matches(spec: &CommandSpec, m: &ArgMatches) -> Result<Value, ApiError> {
    let mut request = Value::Object(Map::new());
    for a in spec.args {
        if let Some(value) = value_of(a, m)? {
            insert(&mut request, a, value)?;
        }
    }
    Ok(request)
}

/// True when any request argument of `spec` was given on the command line.
pub(crate) fn has_request_args(spec: &CommandSpec, m: &ArgMatches) -> bool {
    spec.args
        .iter()
        .any(|a| m.value_source(a.id) == Some(ValueSource::CommandLine))
}

/// The most `--input` reads: twice [`MAX_INGEST_BYTES`], the largest legal
/// payload a request carries (one ingest snapshot), with room for the
/// request's other members and for indentation.
///
/// Every other ingress is bounded (ingest at 64 MiB, write args at 256 KiB, a
/// `bd` stream at 64 MiB); a whole-request read with no cap would grow with
/// a multi-GB file or a stdin that never ends until the process is killed.
const MAX_REQUEST_BYTES: usize = 2 * MAX_INGEST_BYTES;

/// The request in `--input FILE|-` (`-` is stdin), as given.
///
/// # Errors
///
/// `BAD_ARGS` for a file that cannot be read, input over [`MAX_REQUEST_BYTES`],
/// or text that is not JSON.
pub(crate) fn from_input(source: &str) -> Result<Value, ApiError> {
    let text = if source == "-" {
        read_capped(std::io::stdin().lock(), MAX_REQUEST_BYTES)
            .map_err(|e| ApiError::bad_args(format!("--input -: {}", e.describe("stdin"))))?
    } else {
        let file = std::fs::File::open(source)
            .map_err(|e| ApiError::bad_args(format!("--input {source}: cannot read it: {e}")))?;
        read_capped(file, MAX_REQUEST_BYTES)
            .map_err(|e| ApiError::bad_args(format!("--input {source}: {}", e.describe("it"))))?
    };
    serde_json::from_str(&text)
        .map_err(|e| ApiError::bad_args(format!("--input {source}: invalid JSON: {e}")))
}

/// Why [`read_capped`] stopped.
#[derive(Debug)]
enum ReadFault {
    /// More than the cap was available.
    TooLarge(usize),
    /// The reader failed, or its bytes were not UTF-8.
    Io(std::io::Error),
}

impl ReadFault {
    fn describe(&self, what: &str) -> String {
        match self {
            Self::TooLarge(cap) => {
                format!("{what} is over {cap} bytes; a request is at most that")
            }
            Self::Io(e) => format!("cannot read {what}: {e}"),
        }
    }
}

/// Reads `reader` to the end as UTF-8 text, refusing more than `cap` bytes
/// (read through `Read::take(cap + 1)`, so memory stays bounded).
fn read_capped(reader: impl std::io::Read, cap: usize) -> Result<String, ReadFault> {
    let mut bytes = Vec::new();
    reader
        .take(u64::try_from(cap).unwrap_or(u64::MAX).saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(ReadFault::Io)?;
    if bytes.len() > cap {
        return Err(ReadFault::TooLarge(cap));
    }
    String::from_utf8(bytes)
        .map_err(|e| ReadFault::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
}

fn scalar(a: &ArgSpec, text: &str) -> Result<Value, ApiError> {
    Ok(match a.ty {
        Ty::Str => Value::String(text.to_owned()),
        // Only JSON's integer form is a number, so `+5` or `05` reaches the
        // decoder as text and is refused as it is through `--input` (QA minor).
        Ty::Int if !json_integer(text) => Value::String(text.to_owned()),
        Ty::Int => text
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| text.parse::<u64>().map(Value::from))
            .unwrap_or_else(|_| Value::String(text.to_owned())),
        Ty::Json => serde_json::from_str(text).map_err(|e| {
            ApiError::bad_member(a.member, format!("{}: invalid JSON: {e}", a.spelled()))
        })?,
    })
}

/// `text` is an integer as JSON writes one: an optional `-`, then `0` or
/// digits with no leading zero.
fn json_integer(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && (digits == "0" || !digits.starts_with('0'))
}

fn value_of(a: &ArgSpec, m: &ArgMatches) -> Result<Option<Value>, ApiError> {
    Ok(match a.shape {
        Shape::Switch => m.get_flag(a.id).then_some(Value::Bool(true)),
        Shape::One => match m.get_one::<String>(a.id) {
            Some(text) => Some(scalar(a, text)?),
            None => None,
        },
        Shape::Many | Shape::List => {
            match m.get_many::<String>(a.id) {
                Some(texts) => Some(Value::Array(texts.map(|t| scalar(a, t)).collect::<Result<
                    Vec<_>,
                    _,
                >>(
                )?)),
                None => None,
            }
        }
    })
}

fn conflict(a: &ArgSpec) -> ApiError {
    ApiError::usage(
        format!(
            "{} sets request member {} a second time",
            a.spelled(),
            a.member
        ),
        &[a.member],
    )
    .with_detail("member", a.member)
}

/// Puts `value` at `a.member` (dotted; a numeric segment is an array index).
/// Arrays at one member are appended; any other second value is `USAGE`.
fn insert(request: &mut Value, a: &ArgSpec, value: Value) -> Result<(), ApiError> {
    let mut node = request;
    let segments: Vec<&str> = a.member.split('.').collect();
    let Some((last, parents)) = segments.split_last() else {
        return Ok(());
    };
    for (i, seg) in parents.iter().enumerate() {
        let next_is_index = segments
            .get(i + 1)
            .is_some_and(|s| s.parse::<usize>().is_ok());
        node = child(node, seg, next_is_index).ok_or_else(|| conflict(a))?;
    }
    let slot = child_slot(node, last).ok_or_else(|| conflict(a))?;
    match (slot.take(), value) {
        (Value::Null, v) => *slot = v,
        (Value::Array(mut have), Value::Array(more)) => {
            have.extend(more);
            *slot = Value::Array(have);
        }
        (have, _) => {
            *slot = have;
            return Err(conflict(a));
        }
    }
    Ok(())
}

/// The child `seg` of `node`, created as an object (or a one-element array
/// when the next segment is an index) when absent; `None` when `node` holds a
/// value of another kind there.
fn child<'a>(node: &'a mut Value, seg: &str, next_is_index: bool) -> Option<&'a mut Value> {
    let slot = child_slot(node, seg)?;
    if slot.is_null() {
        *slot = if next_is_index {
            Value::Array(Vec::new())
        } else {
            Value::Object(Map::new())
        };
    }
    (slot.is_object() || slot.is_array()).then_some(slot)
}

fn child_slot<'a>(node: &'a mut Value, seg: &str) -> Option<&'a mut Value> {
    match node {
        Value::Object(map) => Some(map.entry(seg.to_owned()).or_insert(Value::Null)),
        Value::Array(items) => {
            let i: usize = seg.parse().ok()?;
            while items.len() <= i {
                items.push(Value::Null);
            }
            items.get_mut(i)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::args::{clap_tree, command};
    use serde_json::json;

    fn request(argv: &[&str]) -> Result<Value, ApiError> {
        let m = clap_tree(false)
            .try_get_matches_from(std::iter::once("bdc").chain(argv.iter().copied()))
            .map_err(|e| ApiError::usage(e.to_string(), &[]))?;
        let mut leaf = &m;
        let mut path = Vec::new();
        while let Some((name, sub)) = leaf.subcommand() {
            path.push(name);
            leaf = sub;
        }
        let spec = command(&path.join(" ")).ok_or_else(|| ApiError::internal("no spec"))?;
        from_matches(spec, leaf)
    }

    #[test]
    fn patch_flags_nest_under_patch_and_merge_with_patch_json() {
        let r = request(&[
            "issues",
            "update",
            "b1",
            "--actor",
            "a",
            "--idempotency-key",
            "k",
            "--patch",
            r#"{"assignee": "x"}"#,
            "--status",
            "open",
            "--set-metadata",
            r#"{"k": 1}"#,
            "--unset-metadata",
            "old",
        ]);
        assert_eq!(
            r.ok(),
            Some(json!({"id": "b1", "actor": "a", "idempotency_key": "k",
                "patch": {"assignee": "x", "status": "open",
                    "metadata": {"set": {"k": 1}, "unset": ["old"]}}}))
        );
    }

    /// An integer flag takes JSON's integer form only, so the CLI and
    /// `--input` hand the decoder the same value (QA minor on 254317c).
    #[test]
    fn an_integer_flag_is_a_number_only_in_json_form() {
        for (text, want) in [
            ("5", json!(5)),
            ("0", json!(0)),
            ("+5", json!("+5")),
            ("05", json!("05")),
            ("5x", json!("5x")),
        ] {
            let r = request(&["issues", "list", "--scope", "x", "--limit", text]).ok();
            let limit = r.as_ref().and_then(|r| r.get("limit"));
            assert_eq!(limit, Some(&want), "--limit {text}");
        }
        // clap reads a leading `-` as a flag; the rule itself still holds.
        assert!(json_integer("-3"));
        assert!(!json_integer("-"));
        assert!(!json_integer("-05"));
    }

    #[test]
    fn a_member_given_twice_is_usage() {
        let r = request(&[
            "issues",
            "update",
            "b1",
            "--actor",
            "a",
            "--idempotency-key",
            "k",
            "--patch",
            r#"{"status": "open"}"#,
            "--status",
            "open",
        ]);
        assert_eq!(r.err().as_ref().map(ApiError::code), Some("BD_CACHE.USAGE"));
    }

    #[test]
    fn dependencies_add_builds_one_edge_and_integers_stay_text_when_not_numbers() {
        let r = request(&[
            "dependencies",
            "add",
            "b2",
            "b1",
            "--type",
            "blocks",
            "--actor",
            "a",
            "--idempotency-key",
            "k",
        ]);
        assert_eq!(
            r.ok(),
            Some(
                json!({"edges": [{"issue_id": "b2", "depends_on_id": "b1", "type": "blocks"}],
                "actor": "a", "idempotency_key": "k"})
            )
        );
        let r = request(&["issues", "list", "--scope", "x", "--limit", "many"]);
        assert_eq!(r.ok(), Some(json!({"scope": "x", "limit": "many"})));
        let r = request(&["cache", "refresh", "scopes", "a", "--scope", "b"]);
        assert_eq!(r.ok(), Some(json!({"scopes": ["a", "b"]})));
    }

    #[test]
    fn invalid_json_in_a_json_flag_is_bad_args() {
        let r = request(&[
            "issues",
            "update",
            "b1",
            "--actor",
            "a",
            "--idempotency-key",
            "k",
            "--patch",
            "{",
        ]);
        assert_eq!(
            r.err().as_ref().map(ApiError::code),
            Some("BD_CACHE.BAD_ARGS")
        );
    }

    /// `--input` reads at most the cap and refuses more as `BAD_ARGS`, never
    /// reading past `cap + 1` bytes (PE1-RSH-005). Planted negative: reading
    /// without `take` would return the oversized text.
    #[test]
    fn input_over_the_cap_is_refused_and_not_read_whole() {
        assert_eq!(
            read_capped(&b"12345678"[..], 8).ok().as_deref(),
            Some("12345678")
        );
        assert!(
            matches!(
                read_capped(&b"123456789"[..], 8),
                Err(ReadFault::TooLarge(8))
            ),
            "one byte over the cap"
        );
        // An endless stream is cut at cap + 1 bytes instead of read forever.
        assert!(
            matches!(
                read_capped(std::io::repeat(b'x'), 8),
                Err(ReadFault::TooLarge(8))
            ),
            "an endless stream"
        );
        assert!(matches!(
            read_capped(&[0xff_u8, 0xfe][..], 8),
            Err(ReadFault::Io(_))
        ));
    }
}
