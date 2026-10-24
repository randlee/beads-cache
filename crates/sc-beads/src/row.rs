//! Strict bd rows: [`Bead`] and [`BlockedRow`], validated key by key (REQ-BDC-0007).
//!
//! The row types and their field tables are generated from the pinned
//! `bd schema` output (bd 1.3.1, `tests/fixtures/bd-1.3.1/bd-schema.json`)
//! into `row/generated.rs`; the generator command and pinned sha are in that
//! file's header, and [`BD_SCHEMA_SHA256`] carries the sha.
//!
//! Validation mirrors BD-CACHE v3's pydantic models (`extra=forbid`): an
//! unknown key, a missing required key, `null` in a required key, a wrong
//! JSON type, a value outside a declared enum or a malformed date-time is a
//! [`BeadsCacheError::Contract`] naming the bead and the field path (for
//! example `dependencies[0].type`). Optional keys accept `null`. Integers must
//! be JSON integers and booleans JSON booleans (stricter than pydantic's lax
//! coercion).
//!
//! After validation the row is shaped for the cache: `dependencies` keep only
//! the edge triple, `labels` and `dependencies` default to `[]`, and a missing
//! or non-object `metadata` becomes `{}` (BD-CACHE.md, Bead shape). The
//! derived counts are kept; the store drops them when it normalizes a row.

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::cache::{BeadId, BeadsCacheError, BlockedRow, Sha256Hex};

mod generated;

#[doc(inline)]
pub use generated::{
    Bead, BeadComment, BondedFrom, DepType, BD_SCHEMA_SHA256, BD_SCHEMA_VERSION, BD_VERSION,
};

/// One key of a generated field table.
#[derive(Debug)]
pub(crate) struct FieldSpec {
    pub(crate) name: &'static str,
    pub(crate) required: bool,
    pub(crate) kind: Kind,
}

/// The JSON shape a key must have.
#[derive(Debug)]
pub(crate) enum Kind {
    Str,
    /// A string that is a valid [`BeadId`].
    Id,
    /// An array of valid [`BeadId`]s.
    IdArray,
    DateTime,
    Int,
    Bool,
    Any,
    Enum(&'static [&'static str]),
    StrArray,
    Objects(&'static [FieldSpec]),
}

/// Keys `bd blocked --json` adds to an issue row.
static BLOCKED_EXTRA_FIELDS: &[FieldSpec] = &[
    FieldSpec {
        name: "blocked_by",
        required: true,
        kind: Kind::IdArray,
    },
    FieldSpec {
        name: "blocked_by_count",
        required: false,
        kind: Kind::Int,
    },
];

impl DepType {
    /// True for types bd counts as blocking ready work.
    ///
    /// `blocks` and `parent-child` were probed (P-6; a child inherits its
    /// parent's blocked state); `conditional-blocks` and `waits-for` are the
    /// other types bd's `AffectsReadyWork` counts and are treated as blocking,
    /// which errs towards "not ready" (REQ-BDC-0904). `related` and the rest do
    /// not block.
    #[must_use]
    pub fn is_blocking(self) -> bool {
        matches!(
            self,
            DepType::Blocks | DepType::ParentChild | DepType::ConditionalBlocks | DepType::WaitsFor
        )
    }
}

impl std::fmt::Display for DepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Bead {
    /// True when the bead carries the label.
    #[must_use]
    pub fn has_label(&self, label: &str) -> bool {
        self.labels.iter().any(|l| l == label)
    }
}

/// Lower-case hex sha256 of a `bd schema` output, as recorded in a [`Fingerprint`](crate::Fingerprint).
///
/// # Examples
///
/// ```
/// assert_eq!(
///     sc_beads::schema_sha256("").as_str(),
///     "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
/// );
/// ```
#[must_use]
pub fn schema_sha256(schema_stdout: &str) -> Sha256Hex {
    let hex = Sha256::digest(schema_stdout.as_bytes()).iter().fold(
        String::with_capacity(64),
        |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
            s
        },
    );
    // A sha256 digest always renders as 64 lower-case hex digits.
    Sha256Hex::new(hex).unwrap_or_else(|e| unreachable_hex(&e))
}

#[expect(clippy::panic, reason = "a sha256 digest is always 64 hex digits")]
fn unreachable_hex(e: &crate::cache::InvalidValue) -> Sha256Hex {
    panic!("sha256 rendering produced {e}")
}

/// Validates one `bd list --json` row and returns it as a [`Bead`].
///
/// # Errors
///
/// [`BeadsCacheError::Contract`] naming the bead (when the row has a string
/// `id`) and the failing field path.
///
/// # Examples
///
/// ```
/// let row = serde_json::json!({
///     "id": "b1", "title": "t", "priority": 2, "status": "open",
///     "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z",
/// });
/// let bead = sc_beads::parse_bd_row(&row).unwrap();
/// assert_eq!(bead.id.as_str(), "b1");
/// assert!(bead.labels.is_empty() && bead.metadata.is_empty());
///
/// let bad = serde_json::json!({"id": "b2", "surprise": 1});
/// let err = sc_beads::parse_bd_row(&bad).unwrap_err();
/// assert_eq!(err.to_string(), "b2: surprise: unknown key");
/// ```
pub fn parse_bd_row(value: &Value) -> Result<Bead, BeadsCacheError> {
    let bead = row_id(value);
    validate_object(value, &[generated::ISSUE_FIELDS], bead.as_ref(), "")?;
    shape_bead(value, bead.as_ref())
}

/// Validates the stdout of a `bd list --json` call: a JSON array of rows, or `null` for none.
///
/// One bad row fails the whole result (REQ-BDC-0008).
///
/// # Errors
///
/// [`BeadsCacheError::Contract`] for invalid JSON, a top level that is not an
/// array, or any row that fails [`parse_bd_row`].
pub fn parse_bd_rows(stdout: &str) -> Result<Vec<Bead>, BeadsCacheError> {
    rows_of(stdout)?.iter().map(parse_bd_row).collect()
}

/// Validates the stdout of `bd blocked --json` and returns each row's id and blockers.
///
/// A row is an issue row (every key of [`Bead`]'s table) plus `blocked_by`
/// (required) and `blocked_by_count`.
///
/// # Errors
///
/// [`BeadsCacheError::Contract`] as for [`parse_bd_rows`].
pub fn parse_blocked_rows(stdout: &str) -> Result<Vec<BlockedRow>, BeadsCacheError> {
    rows_of(stdout)?
        .iter()
        .map(|value| {
            let bead = row_id(value);
            validate_object(
                value,
                &[generated::ISSUE_FIELDS, BLOCKED_EXTRA_FIELDS],
                bead.as_ref(),
                "",
            )?;
            let blocked_by = value
                .get("blocked_by")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(Value::as_str)
                        .filter_map(|s| BeadId::new(s).ok())
                        .collect()
                })
                .unwrap_or_default();
            // validate_object checked that `id` is present and valid.
            let id =
                bead.ok_or_else(|| BeadsCacheError::contract(None, "id", "missing bead id"))?;
            Ok(BlockedRow { id, blocked_by })
        })
        .collect()
}

fn rows_of(stdout: &str) -> Result<Vec<Value>, BeadsCacheError> {
    let text = if stdout.trim().is_empty() {
        "null"
    } else {
        stdout
    };
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Null) => Ok(Vec::new()),
        Ok(Value::Array(rows)) => Ok(rows),
        Ok(_) => Err(BeadsCacheError::contract(
            None,
            "",
            "expected a JSON array of rows",
        )),
        Err(e) => Err(BeadsCacheError::contract(
            None,
            "",
            format!("invalid JSON: {e}"),
        )),
    }
}

fn row_id(value: &Value) -> Option<BeadId> {
    value
        .get("id")
        .and_then(Value::as_str)
        .and_then(|s| BeadId::new(s).ok())
}

fn join(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}

fn validate_object(
    value: &Value,
    tables: &[&[FieldSpec]],
    bead: Option<&BeadId>,
    path: &str,
) -> Result<(), BeadsCacheError> {
    let field = |name: &str| join(path, name);
    let Some(obj) = value.as_object() else {
        let at = if path.is_empty() { "" } else { path };
        return Err(BeadsCacheError::contract(bead, at, "expected an object"));
    };
    let lookup = |key: &str| tables.iter().flat_map(|t| t.iter()).find(|s| s.name == key);
    for (key, v) in obj {
        let Some(spec) = lookup(key) else {
            return Err(BeadsCacheError::contract(bead, field(key), "unknown key"));
        };
        if v.is_null() {
            if spec.required {
                return Err(BeadsCacheError::contract(
                    bead,
                    field(key),
                    "required key is null",
                ));
            }
            continue;
        }
        check_kind(v, &spec.kind, bead, &field(key))?;
    }
    for spec in tables.iter().flat_map(|t| t.iter()) {
        if spec.required && !obj.contains_key(spec.name) {
            return Err(BeadsCacheError::contract(
                bead,
                field(spec.name),
                "missing required key",
            ));
        }
    }
    Ok(())
}

fn check_kind(
    v: &Value,
    kind: &Kind,
    bead: Option<&BeadId>,
    field: &str,
) -> Result<(), BeadsCacheError> {
    let wrong = |expected: &str| {
        Err(BeadsCacheError::contract(
            bead,
            field,
            format!("expected {expected}, got {}", type_name(v)),
        ))
    };
    match kind {
        Kind::Any => Ok(()),
        Kind::Str => {
            if v.is_string() {
                Ok(())
            } else {
                wrong("a string")
            }
        }
        Kind::Int => {
            if v.is_i64() || v.is_u64() {
                Ok(())
            } else {
                wrong("an integer")
            }
        }
        Kind::Bool => {
            if v.is_boolean() {
                Ok(())
            } else {
                wrong("a boolean")
            }
        }
        Kind::Id => match v.as_str() {
            Some(s) => BeadId::new(s)
                .map(drop)
                .map_err(|e| BeadsCacheError::contract(bead, field, e.to_string())),
            None => wrong("a bead id string"),
        },
        Kind::IdArray => match v.as_array() {
            Some(items) => items.iter().enumerate().try_for_each(|(i, item)| {
                check_kind(item, &Kind::Id, bead, &format!("{field}[{i}]"))
            }),
            None => wrong("an array"),
        },
        Kind::DateTime => match v.as_str() {
            Some(s) if crate::clock::rfc3339_to_unix(s).is_some() => Ok(()),
            Some(s) => Err(BeadsCacheError::contract(
                bead,
                field,
                format!("{s:?} is not an RFC 3339 date-time with a timezone"),
            )),
            None => wrong("a date-time string"),
        },
        Kind::Enum(values) => match v.as_str() {
            Some(s) if values.contains(&s) => Ok(()),
            Some(s) => Err(BeadsCacheError::contract(
                bead,
                field,
                format!("{s:?} is not one of {}", values.join(", ")),
            )),
            None => wrong("a string"),
        },
        Kind::StrArray => match v.as_array() {
            Some(items) => items.iter().enumerate().try_for_each(|(i, item)| {
                if item.is_string() {
                    Ok(())
                } else {
                    Err(BeadsCacheError::contract(
                        bead,
                        format!("{field}[{i}]"),
                        format!("expected a string, got {}", type_name(item)),
                    ))
                }
            }),
            None => wrong("an array"),
        },
        Kind::Objects(spec) => match v.as_array() {
            Some(items) => items.iter().enumerate().try_for_each(|(i, item)| {
                validate_object(item, &[spec], bead, &format!("{field}[{i}]"))
            }),
            None => wrong("an array"),
        },
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(n) if n.is_f64() => "a float",
        Value::Number(_) => "an integer",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

/// Reshapes a validated row into the stored form and deserializes it.
fn shape_bead(value: &Value, bead: Option<&BeadId>) -> Result<Bead, BeadsCacheError> {
    let mut obj = value.as_object().cloned().unwrap_or_default();
    // Drop explicit nulls so serde defaults apply uniformly.
    obj.retain(|_, v| !v.is_null());
    if let Some(Value::Array(deps)) = obj.get_mut("dependencies") {
        for dep in deps.iter_mut() {
            if let Value::Object(d) = dep {
                d.retain(|k, _| matches!(k.as_str(), "issue_id" | "depends_on_id" | "type"));
            }
        }
    }
    if !obj.get("metadata").is_some_and(Value::is_object) {
        obj.insert("metadata".into(), Value::Object(serde_json::Map::new()));
    }
    serde_json::from_value(Value::Object(obj)).map_err(|e| {
        BeadsCacheError::contract(
            bead,
            "",
            format!("row does not match the strict model: {e}"),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> Value {
        serde_json::json!({
            "id": "b1", "title": "t", "priority": 2, "status": "open", "issue_type": "task",
            "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:01Z",
            "labels": ["phase-x"], "metadata": {"n": 1},
            "dependencies": [{"issue_id": "b1", "depends_on_id": "b0", "type": "blocks",
                              "created_at": "2026-10-23T09:00:00Z", "metadata": "{}"}],
            "dependency_count": 1, "dependent_count": 0, "comment_count": 0
        })
    }

    fn field_of(v: &Value) -> (Option<String>, String) {
        match parse_bd_row(v) {
            Err(BeadsCacheError::Contract { bead, field, .. }) => {
                (bead.map(BeadId::into_string), field)
            }
            other => panic!("expected a contract error, got {other:?}"),
        }
    }

    #[test]
    fn a_good_row_keeps_edges_and_counts() {
        let b = parse_bd_row(&row()).unwrap();
        assert_eq!(b.dependencies.len(), 1);
        assert_eq!(b.dependencies[0].kind, DepType::Blocks);
        assert_eq!(b.dependency_count, Some(1));
        assert_eq!(b.is_blocked, None);
        assert!(b.has_label("phase-x"));
    }

    #[test]
    fn violations_name_bead_and_field() {
        let mut v = row();
        v["surprise"] = 1.into();
        assert_eq!(field_of(&v), (Some("b1".into()), "surprise".into()));

        let mut v = row();
        v.as_object_mut().unwrap().remove("title");
        assert_eq!(field_of(&v).1, "title");

        let mut v = row();
        v["priority"] = "2".into();
        assert_eq!(field_of(&v).1, "priority");

        let mut v = row();
        v["status"] = "nope".into();
        assert_eq!(field_of(&v).1, "status");

        let mut v = row();
        v["created_at"] = "yesterday".into();
        assert_eq!(field_of(&v).1, "created_at");

        let mut v = row();
        v["dependencies"][0]["type"] = "likes".into();
        assert_eq!(field_of(&v).1, "dependencies[0].type");

        let mut v = row();
        v["dependencies"][0]["extra"] = 1.into();
        assert_eq!(field_of(&v).1, "dependencies[0].extra");

        let mut v = row();
        v["labels"] = serde_json::json!(["ok", 3]);
        assert_eq!(field_of(&v).1, "labels[1]");

        let mut v = row();
        v["title"] = Value::Null;
        assert_eq!(field_of(&v).1, "title");
    }

    #[test]
    fn optional_null_and_non_object_metadata_are_accepted() {
        let mut v = row();
        v["assignee"] = Value::Null;
        v["metadata"] = "legacy".into();
        let b = parse_bd_row(&v).unwrap();
        assert!(b.assignee.is_none() && b.metadata.is_empty());
    }

    #[test]
    fn rows_of_handles_null_and_garbage() {
        assert!(parse_bd_rows("").unwrap().is_empty());
        assert!(parse_bd_rows("null").unwrap().is_empty());
        assert!(matches!(
            parse_bd_rows("{}"),
            Err(BeadsCacheError::Contract { .. })
        ));
        assert!(matches!(
            parse_bd_rows("not json"),
            Err(BeadsCacheError::Contract { .. })
        ));
    }

    #[test]
    fn blocked_rows_require_blocked_by() {
        let mut v = row();
        v.as_object_mut().unwrap().remove("dependencies");
        v["blocked_by"] = serde_json::json!(["b0"]);
        v["blocked_by_count"] = 1.into();
        let rows = parse_blocked_rows(&serde_json::to_string(&vec![v.clone()]).unwrap()).unwrap();
        assert_eq!(
            rows,
            vec![BlockedRow {
                id: BeadId::new("b1").unwrap(),
                blocked_by: vec![BeadId::new("b0").unwrap()]
            }]
        );
        v.as_object_mut().unwrap().remove("blocked_by");
        assert!(parse_blocked_rows(&serde_json::to_string(&vec![v]).unwrap()).is_err());
    }

    #[test]
    fn dep_type_round_trips_and_blocking() {
        for t in DepType::ALL {
            assert_eq!(DepType::parse(t.as_str()), Some(*t));
        }
        assert_eq!(DepType::parse("likes"), None);
        assert!(DepType::Blocks.is_blocking() && DepType::ParentChild.is_blocking());
        assert!(!DepType::Related.is_blocking());
    }
}
