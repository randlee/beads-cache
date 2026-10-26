//! Generator of `src/row/generated.rs` from the pinned `bd schema` fixture, and its freshness check.
//!
//! `cargo test` fails when the committed file differs from what this generator
//! produces from `tests/fixtures/bd-1.3.1/bd-schema.json`. To regenerate after
//! re-vendoring the fixture for a new bd release:
//!
//! ```text
//! SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated
//! ```
//!
//! then update `PINNED_BD_VERSION` below if bd's version changed.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::Value;

/// bd release that printed the fixture (`bd schema` does not carry it).
const PINNED_BD_VERSION: &str = "1.3.1";
const FIXTURE: &str = "tests/fixtures/bd-1.3.1/bd-schema.json";
const OUTPUT: &str = "src/row/generated.rs";

/// The keys `bd list --json` emits that `bd schema` does not declare (the v3 bead shape).
const JSON_EXTRAS: &[(&str, &str)] = &[
    ("comment_count", "integer"),
    ("dependency_count", "integer"),
    ("dependent_count", "integer"),
    ("parent", "string"),
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn camel(s: &str) -> String {
    s.split(['-', '_'])
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn required(obj: &Value) -> Vec<String> {
    obj["required"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn props(obj: &Value) -> Vec<(String, Value)> {
    // serde_json's map is ordered by key, so fields come out alphabetically.
    obj["properties"]
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

fn enum_values(p: &Value) -> Option<Vec<String>> {
    p["enum"].as_array().map(|a| {
        a.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    })
}

/// The `Kind` expression for one property.
fn kind(owner: &str, name: &str, p: &Value, out_consts: &mut Vec<(String, Vec<String>)>) -> String {
    // Bead ids are checked as `BeadId`s (no whitespace, control characters or commas).
    if matches!(
        (owner, name),
        ("issue", "id") | ("dependency", "issue_id" | "depends_on_id")
    ) {
        return "Kind::Id".into();
    }
    if p == &Value::Bool(true) {
        return "Kind::Any".into();
    }
    match p["type"].as_str() {
        Some("string") if p["format"] == "date-time" => "Kind::DateTime".into(),
        Some("string") => match enum_values(p) {
            Some(values) => {
                let konst = format!("{}_{}", owner.to_uppercase(), name.to_uppercase());
                if !out_consts.iter().any(|(k, _)| *k == konst) {
                    out_consts.push((konst.clone(), values));
                }
                format!("Kind::Enum({konst})")
            }
            None => "Kind::Str".into(),
        },
        Some("integer") => "Kind::Int".into(),
        Some("boolean") => "Kind::Bool".into(),
        Some("array") => {
            let items = &p["items"];
            if items["type"] == "object" {
                format!("Kind::Objects({}_FIELDS)", nested_name(name).to_uppercase())
            } else {
                "Kind::StrArray".into()
            }
        }
        other => panic!("unsupported schema type {other:?} for {owner}.{name}"),
    }
}

fn nested_name(field: &str) -> &'static str {
    match field {
        "dependencies" => "dependency",
        "comments" => "comment",
        "bonded_from" => "bonded_from",
        other => panic!("no nested type name for {other}"),
    }
}

fn spec_table(
    out: &mut String,
    table: &str,
    owner: &str,
    obj: &Value,
    extras: &[(&str, &str)],
    consts: &mut Vec<(String, Vec<String>)>,
) {
    let req = required(obj);
    writeln!(out, "pub(crate) static {table}: &[FieldSpec] = &[").unwrap();
    for (name, p) in props(obj) {
        let k = kind(owner, &name, &p, consts);
        let r = req.contains(&name);
        writeln!(
            out,
            "    FieldSpec {{ name: \"{name}\", required: {r}, kind: {k} }},"
        )
        .unwrap();
    }
    for (name, ty) in extras {
        let k = if *ty == "integer" {
            "Kind::Int"
        } else {
            "Kind::Str"
        };
        writeln!(
            out,
            "    FieldSpec {{ name: \"{name}\", required: false, kind: {k} }}, // --json extra"
        )
        .unwrap();
    }
    writeln!(out, "];\n").unwrap();
}

fn rust_type(owner: &str, name: &str, p: &Value, req: bool) -> (String, &'static str) {
    // (type, serde attribute)
    if owner == "issue" {
        match name {
            "id" => return ("BeadId".into(), ""),
            "labels" => return ("Vec<String>".into(), "#[serde(default)]"),
            "dependencies" => return ("Vec<Edge>".into(), "#[serde(default)]"),
            "metadata" => {
                return (
                    "serde_json::Map<String, serde_json::Value>".into(),
                    "#[serde(default)]",
                );
            }
            _ => {}
        }
    }
    let base = match p["type"].as_str() {
        Some("string") => "String".to_string(),
        Some("integer") => "i64".to_string(),
        Some("boolean") => "bool".to_string(),
        Some("array") if p["items"]["type"] == "object" => {
            format!("Vec<{}>", struct_name(nested_name(name)))
        }
        Some("array") => "Vec<String>".to_string(),
        other => panic!("unsupported type {other:?} for {owner}.{name}"),
    };
    if req {
        (base, "")
    } else {
        (
            format!("Option<{base}>"),
            "#[serde(default, skip_serializing_if = \"Option::is_none\")]",
        )
    }
}

fn struct_name(nested: &str) -> &'static str {
    match nested {
        "comment" => "BeadComment",
        "bonded_from" => "BondedFrom",
        other => panic!("no struct for {other}"),
    }
}

fn describe(p: &Value) -> String {
    if p == &Value::Bool(true) {
        return "any JSON value; stored as an object (non-objects become `{}`)".into();
    }
    let ty = p["type"].as_str().unwrap_or("?");
    match (ty, p["format"].as_str(), enum_values(p)) {
        ("string", Some("date-time"), _) => "RFC 3339 date-time".into(),
        ("string", _, Some(v)) => format!("one of `{}`", v.join("`, `")),
        ("array", _, _) if p["items"]["type"] == "object" => "array of objects".into(),
        ("array", _, _) => "array of strings".into(),
        (t, _, _) => t.into(),
    }
}

fn structure(
    out: &mut String,
    owner: &str,
    name: &str,
    doc: &str,
    obj: &Value,
    extras: &[(&str, &str)],
) {
    let req = required(obj);
    writeln!(out, "/// {doc}").unwrap();
    // `Bead` has no `Default`: an empty `BeadId` is not a valid id.
    let default = if name == "Bead" { "" } else { "Default, " };
    writeln!(
        out,
        "#[derive(Debug, Clone, {default}PartialEq, Serialize, Deserialize)]\n#[serde(deny_unknown_fields)]\npub struct {name} {{"
    )
    .unwrap();
    for (field, p) in props(obj) {
        let r = req.contains(&field);
        let (ty, attr) = rust_type(owner, &field, &p, r);
        let need = if r { "required" } else { "optional" };
        writeln!(out, "    /// `{field}`: {}, {need}.", describe(&p)).unwrap();
        if !attr.is_empty() {
            writeln!(out, "    {attr}").unwrap();
        }
        writeln!(out, "    pub {field}: {ty},").unwrap();
    }
    for (field, ty) in extras {
        let t = if *ty == "integer" { "i64" } else { "String" };
        writeln!(
            out,
            "    /// `{field}`: {ty}, optional; emitted by `--json`, not declared by `bd schema`."
        )
        .unwrap();
        writeln!(
            out,
            "    #[serde(default, skip_serializing_if = \"Option::is_none\")]"
        )
        .unwrap();
        writeln!(out, "    pub {field}: Option<{t}>,").unwrap();
    }
    writeln!(out, "}}\n").unwrap();
}

#[expect(
    clippy::too_many_lines,
    reason = "the generator emits the file top to bottom; splitting it hides the output order"
)]
fn generate(schema_text: &str) -> String {
    let schema: Value = serde_json::from_str(schema_text).unwrap();
    let sha = sc_beads::schema_sha256(schema_text);
    let schema_version = schema["schema_version"].to_string();
    let issue = &schema["types"]["issue"];
    let dependency = &schema["types"]["dependency"];
    let issue_props: serde_json::Map<String, Value> =
        issue["properties"].as_object().unwrap().clone();
    let comment = &issue_props["comments"]["items"];
    let bonded = &issue_props["bonded_from"]["items"];
    let dep_types = enum_values(&dependency["properties"]["type"]).unwrap();

    let mut out = String::new();
    write!(
        out,
        "// @generated by crates/sc-beads/tests/strict_rows_generated.rs from\n\
         // tests/fixtures/bd-1.3.1/bd-schema.json (`bd schema`, bd {PINNED_BD_VERSION}). Do not edit.\n\
         // Regenerate: SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated\n\
         // Pinned schema sha256: {sha}\n\n\
         use serde::{{Deserialize, Serialize}};\n\n\
         use super::{{FieldSpec, Kind}};\n\
         use crate::cache::{{BeadId, Edge}};\n\n\
         /// bd release the strict model was generated from.\n\
         pub const BD_VERSION: &str = \"{PINNED_BD_VERSION}\";\n\
         /// `schema_version` of the pinned `bd schema` output.\n\
         pub const BD_SCHEMA_VERSION: &str = \"{schema_version}\";\n\
         /// sha256 of the pinned `bd schema` output; a backend whose schema hashes the same is `model_current`.\n\
         pub const BD_SCHEMA_SHA256: &str = \"{sha}\";\n\n"
    )
    .unwrap();

    // DepType
    out.push_str("/// bd's dependency types (`types.dependency.type` in `bd schema`).\n");
    out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]\npub enum DepType {\n");
    for v in &dep_types {
        writeln!(
            out,
            "    /// `{v}`\n    #[serde(rename = \"{v}\")]\n    {},",
            camel(v)
        )
        .unwrap();
    }
    out.push_str("}\n\nimpl DepType {\n    /// Every dependency type, in `bd schema` order.\n    pub const ALL: &[DepType] = &[\n");
    for v in &dep_types {
        writeln!(out, "        DepType::{},", camel(v)).unwrap();
    }
    out.push_str("    ];\n\n    /// The wire name, e.g. `parent-child`.\n    #[must_use]\n    pub fn as_str(self) -> &'static str {\n        match self {\n");
    for v in &dep_types {
        writeln!(out, "            DepType::{} => \"{v}\",", camel(v)).unwrap();
    }
    out.push_str("        }\n    }\n\n    /// Parses a wire name; `None` for a type bd does not know.\n    #[must_use]\n    pub fn parse(s: &str) -> Option<Self> {\n        match s {\n");
    for v in &dep_types {
        writeln!(out, "            \"{v}\" => Some(DepType::{}),", camel(v)).unwrap();
    }
    out.push_str("            _ => None,\n        }\n    }\n}\n\n");

    let mut consts = Vec::new();
    let mut tables = String::new();
    spec_table(
        &mut tables,
        "DEPENDENCY_FIELDS",
        "dependency",
        dependency,
        &[],
        &mut consts,
    );
    spec_table(
        &mut tables,
        "COMMENT_FIELDS",
        "comment",
        comment,
        &[],
        &mut consts,
    );
    spec_table(
        &mut tables,
        "BONDED_FROM_FIELDS",
        "bonded_from",
        bonded,
        &[],
        &mut consts,
    );
    spec_table(
        &mut tables,
        "ISSUE_FIELDS",
        "issue",
        issue,
        JSON_EXTRAS,
        &mut consts,
    );
    for (name, values) in &consts {
        let list = values
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(out, "pub(crate) const {name}: &[&str] = &[{list}];").unwrap();
    }
    out.push('\n');
    out.push_str(&tables);

    structure(
        &mut out,
        "comment",
        "BeadComment",
        "A comment embedded in a bead row (`types.issue.comments`).",
        comment,
        &[],
    );
    structure(
        &mut out,
        "bonded_from",
        "BondedFrom",
        "A bond source embedded in a bead row (`types.issue.bonded_from`).",
        bonded,
        &[],
    );
    structure(
        &mut out,
        "issue",
        "Bead",
        "Strict bead row: `types.issue` of `bd schema` plus the four `--json` extras.\n///\n/// \
         Build one from bd output with [`parse_bd_row`](crate::parse_bd_row), which\n/// \
         validates every key first. `dependencies` holds the outgoing edges only\n/// \
         (`issue_id`, `depends_on_id`, `type`); `labels` and `dependencies` default\n/// \
         to `[]` and `metadata` to `{}`. `is_blocked` is declared by the schema but\n/// \
         bd never emits it (ADR-BDC-0002).",
        issue,
        JSON_EXTRAS,
    );
    out.trim_end().to_string() + "\n"
}

#[test]
fn generated_strict_rows_are_current() {
    let schema = std::fs::read_to_string(root().join(FIXTURE)).unwrap();
    // Keep the committed file rustfmt-clean: format the candidate the same way.
    let expected = rustfmt(&generate(&schema));
    let path = root().join(OUTPUT);
    if std::env::var_os("SC_BEADS_REGENERATE").is_some() {
        std::fs::write(&path, &expected).unwrap();
    }
    let actual = std::fs::read_to_string(&path).unwrap_or_default();
    assert!(
        actual == expected,
        "{OUTPUT} is stale; run SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated"
    );
}

/// How long the rustfmt subprocess may run before the test kills it and fails.
const RUSTFMT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// The edition `cargo fmt` uses for this crate: its own `edition`, else the workspace's.
fn crate_edition() -> String {
    let edition_in = |path: PathBuf| -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        text.lines().find_map(|l| {
            let v = l
                .trim()
                .strip_prefix("edition")?
                .trim_start()
                .strip_prefix('=')?;
            Some(v.trim().trim_matches('"').to_owned())
        })
    };
    edition_in(root().join("Cargo.toml"))
        .or_else(|| edition_in(root().join("../../Cargo.toml")))
        .unwrap_or_else(|| panic!("no edition in crates/sc-beads/Cargo.toml or the workspace"))
}

/// Formats `src` with rustfmt at the crate's edition, as `cargo fmt` formats it.
///
/// Fails the test, never silently falls back: a missing rustfmt, a rustfmt
/// error, or a run longer than [`RUSTFMT_TIMEOUT`] all panic with the reason.
/// The deadline starts before the input is written; stdin is written and
/// stdout/stderr drained on their own threads, so a stuck child cannot block
/// the test. On timeout the child is killed and reaped, then the threads joined.
fn rustfmt(src: &str) -> String {
    use std::io::{Read as _, Write as _};
    use std::process::{Command, Stdio};
    use std::time::Instant;

    let started = Instant::now();
    let edition = crate_edition();
    let mut child = Command::new("rustfmt")
        .args(["--edition", edition.as_str(), "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            panic!("rustfmt could not start ({e}); install the rustfmt component listed in rust-toolchain.toml")
        });
    let mut stdin = child.stdin.take().unwrap();
    let input = src.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut buf = String::new();
        stdout.read_to_string(&mut buf).map(|_| buf)
    });
    let err = std::thread::spawn(move || {
        let mut buf = String::new();
        stderr.read_to_string(&mut buf).map(|_| buf)
    });
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > RUSTFMT_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let _ = writer.join();
            panic!("rustfmt did not finish within {RUSTFMT_TIMEOUT:?}; killed and reaped it");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let written = writer.join().unwrap();
    let formatted = out.join().unwrap().unwrap();
    let diagnostics = err.join().unwrap().unwrap_or_default();
    assert!(status.success(), "rustfmt failed ({status}): {diagnostics}");
    written.unwrap_or_else(|e| panic!("writing rustfmt's input failed: {e}"));
    formatted
}
