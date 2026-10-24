//! [`FakeBd`]: an invoker that behaves like `bd` for the cache's command forms.
//!
//! It receives the argv a [`BdCli`](crate::BdCli) built, parses it the way `bd` does
//! (flags with values, an unknown flag is a usage error, `--set-metadata` values are
//! coerced as bd coerces them), applies it to a `sc_beads::testing::FakeBackend`, and
//! answers with bd's output and exit status. Because the argv really is parsed, a
//! wrong flag, a missing `--actor` or a metadata value bd would store differently
//! shows up as a failing test, not as a passing one that only echoed its input.
//!
//! The exit statuses follow bd 1.3.0: `0` success, `13` a refused `--if-*` guard, `1`
//! any other failure, `2` a usage error. An unreachable server (an injected
//! `Unavailable`) is exit 1 with bd's "Dolt server unreachable ... connection refused"
//! wording; a timeout is [`BoundedError::TimedOut`].
//!
//! `FakeBd` serves the cache's calls only; btit's `run_json` and `run_raw` refuse with
//! [`BeadsError::Unsupported`].

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use sc_beads::error::BeadsError;
use sc_beads::testing::{FakeBackend, ManualClock};
use sc_beads::{
    Actor, Assignee, Bead, BeadId, BeadsCacheError, CacheBackend, CliClient, CliOutput, CliProbe,
    Deadline, DepType, Guards, Patch, ProjectRef, Refusal, ScopeFilter, SyncCursor, BD_VERSION,
};
use serde_json::{json, Value};

use crate::bounded::BoundedError;
use crate::runner::CliInvoker;

/// bd's status list when `--status` is not given: closed beads are omitted (P-6).
const DEFAULT_STATUSES: &[&str] = &["open", "in_progress", "blocked", "deferred"];

/// The `bd schema` output of bd 1.3.1: the fake's default schema.
///
/// A byte-for-byte copy of `sc-beads/tests/fixtures/bd-1.3.1/bd-schema.json`; the test
/// `schema_copy_is_the_pinned_schema` fails if it drifts from `BD_SCHEMA_SHA256`.
const PINNED_SCHEMA: &str = include_str!("../../tests/fixtures/bd-schema-1.3.1.json");

struct State {
    schema: String,
    version: String,
    calls: Vec<Vec<String>>,
}

/// A fake `bd` over a [`FakeBackend`]; clones share one bd and one call log.
#[derive(Clone)]
pub struct FakeBd {
    backend: FakeBackend,
    state: Arc<Mutex<State>>,
}

impl std::fmt::Debug for FakeBd {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeBd")
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

impl FakeBd {
    /// An empty bd on the pinned schema, sharing `clock`.
    #[must_use]
    pub fn new(clock: Arc<ManualClock>) -> Self {
        Self::over(FakeBackend::new(clock))
    }

    /// A fake bd over an existing backend, so a test can seed and inspect its rows.
    #[must_use]
    pub fn over(backend: FakeBackend) -> Self {
        Self {
            backend,
            state: Arc::new(Mutex::new(State {
                schema: PINNED_SCHEMA.to_owned(),
                version: BD_VERSION.to_owned(),
                calls: Vec::new(),
            })),
        }
    }

    /// The backend holding the rows: seed it, edit it as another writer would, inject
    /// failures into it (they reach the caller as the exit status they stand for).
    #[must_use]
    pub fn backend(&self) -> &FakeBackend {
        &self.backend
    }

    /// Replaces the text `bd schema` prints (a `bd` upgrade that changed the schema).
    pub fn set_schema(&self, schema: impl Into<String>) {
        self.state().schema = schema.into();
    }

    /// Replaces the version `bd version` prints.
    pub fn set_bd_version(&self, version: impl Into<String>) {
        self.state().version = version.into();
    }

    /// The argv of every call so far, in call order.
    #[must_use]
    pub fn calls(&self) -> Vec<Vec<String>> {
        self.state().calls.clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        // Test double: a panic in another test thread must not cascade.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// A parsed command line: positional words and flags in order.
struct Args {
    positional: Vec<String>,
    flags: Vec<(String, String)>,
}

impl Args {
    /// Splits `argv` using the flags a command knows. A flag in `valued` takes the next
    /// word (even an empty one); a flag in `switches` takes none; any other word that
    /// starts with `-` is a usage error, as in cobra.
    fn parse(argv: &[String], valued: &[&str], switches: &[&str]) -> Result<Self, String> {
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut words = argv.iter();
        while let Some(word) = words.next() {
            if valued.contains(&word.as_str()) {
                let value = words
                    .next()
                    .ok_or_else(|| format!("flag needs an argument: {word}"))?;
                flags.push((word.clone(), value.clone()));
            } else if switches.contains(&word.as_str()) {
                flags.push((word.clone(), String::new()));
            } else if word.starts_with('-') && word.len() > 1 {
                return Err(format!("unknown flag: {word}"));
            } else {
                positional.push(word.clone());
            }
        }
        Ok(Self { positional, flags })
    }

    fn one(&self, name: &str) -> Option<&str> {
        self.flags
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn all(&self, name: &str) -> Vec<String> {
        self.flags
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
            .collect()
    }
}

fn success(stdout: impl Into<String>) -> CliOutput {
    CliOutput {
        status: Some(0),
        success: true,
        stdout: stdout.into(),
        stderr: String::new(),
    }
}

fn failure(status: i32, stderr: impl Into<String>) -> CliOutput {
    CliOutput {
        status: Some(status),
        success: false,
        stdout: String::new(),
        stderr: stderr.into(),
    }
}

fn usage(message: &str) -> CliOutput {
    failure(2, format!("Error: {message}"))
}

/// What `bd --set-metadata key=value` stores for `value`.
///
/// bd parses the text as JSON: a number, `true`/`false` or `null` is stored as that
/// type (a whole number as an integer, whatever its spelling: `5`, `5.0`, `1e3`,
/// ` 7`); anything else, including text that is JSON of another kind (`"5"`, `[1]`,
/// `{"x":1}`) and `True`, is stored as the string it was given (BD-CACHE baseline,
/// probed live against bd 1.3.1).
#[must_use]
pub fn coerce_metadata(value: &str) -> Value {
    match serde_json::from_str::<Value>(value) {
        Ok(Value::Number(n)) => {
            let whole = n
                .as_i64()
                .or_else(|| n.as_f64().filter(|f| f.fract() == 0.0).and_then(whole_i64));
            whole.map_or(Value::Number(n), Value::from)
        }
        Ok(typed @ (Value::Bool(_) | Value::Null)) => typed,
        _ => Value::String(value.to_owned()),
    }
}

/// `f` as an integer when it is in range (the caller has checked it is whole).
#[expect(
    clippy::cast_possible_truncation,
    reason = "the range check makes the cast exact"
)]
fn whole_i64(f: f64) -> Option<i64> {
    // 2^63 is exactly representable; every whole f64 below it fits an i64.
    (f.abs() < 9_223_372_036_854_775_808.0).then_some(f as i64)
}

impl FakeBd {
    fn dispatch(&self, argv: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let rest = argv.get(1..).unwrap_or_default();
        match argv.first().map(String::as_str) {
            Some("version") => Ok(success(format!(
                "bd version {} (dev)\n",
                self.state().version
            ))),
            Some("schema") => Ok(success(self.state().schema.clone())),
            Some("list") => self.list(rest, d),
            Some("blocked") => self.blocked(rest, d),
            Some("ready") => self.ready(rest, d),
            Some("close") => self.close(rest, d),
            Some("update") => self.update(rest, d),
            Some("dep") if argv.get(1).map(String::as_str) == Some("add") => {
                self.dep_add(argv.get(2..).unwrap_or_default(), d)
            }
            other => Ok(usage(&format!("unknown command {other:?}"))),
        }
    }

    fn list(&self, rest: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let args = match Args::parse(
            rest,
            &["--id", "-l", "--status", "-n", "--updated-after"],
            &["--json"],
        ) {
            Ok(a) => a,
            Err(m) => return Ok(usage(&m)),
        };
        let statuses: Vec<&str> = args
            .one("--status")
            .map_or_else(|| DEFAULT_STATUSES.to_vec(), |s| s.split(',').collect());
        let p = ProjectRef::local(None);
        let rows = if let Some(ids) = args.one("--id") {
            let ids: Result<Vec<BeadId>, _> = ids.split(',').map(BeadId::new).collect();
            let Ok(ids) = ids else {
                return Ok(failure(1, "Error: invalid --id list"));
            };
            self.backend.list_by_ids(&p, &ids, d)
        } else if let Some(label) = args.one("-l") {
            let cursor = args.one("--updated-after").map(SyncCursor::new);
            self.backend.list_scope(
                &p,
                &ScopeFilter::Label(label.to_owned()),
                cursor.as_ref(),
                d,
            )
        } else {
            return Ok(usage("list needs --id or -l in this fake"));
        };
        let rows = match rows {
            Ok(rows) => rows,
            Err(e) => return failed(e),
        };
        let rows: Vec<Bead> = rows
            .into_iter()
            .filter(|r| statuses.contains(&r.status.as_deref().unwrap_or("open")))
            .collect();
        Ok(success(beads_json(&rows)))
    }

    fn blocked(&self, rest: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let args = match Args::parse(rest, &["-l"], &["--json"]) {
            Ok(a) => a,
            Err(m) => return Ok(usage(&m)),
        };
        let Some(label) = args.one("-l") else {
            return Ok(usage("blocked needs -l in this fake"));
        };
        let blocked = match self.backend.list_blocked(
            &ProjectRef::local(None),
            &ScopeFilter::Label(label.to_owned()),
            d,
        ) {
            Ok(rows) => rows,
            Err(e) => return failed(e),
        };
        let rows: Vec<Value> = blocked
            .iter()
            .filter_map(|b| {
                let mut row = bd_row(&self.backend.row(b.id.as_str())?);
                let map = row.as_object_mut()?;
                map.insert("blocked_by".into(), json!(b.blocked_by));
                map.insert("blocked_by_count".into(), json!(b.blocked_by.len()));
                Some(row)
            })
            .collect();
        Ok(success(values_json(&rows)))
    }

    fn ready(&self, rest: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let args = match Args::parse(rest, &["-l", "-n"], &["--json"]) {
            Ok(a) => a,
            Err(m) => return Ok(usage(&m)),
        };
        let Some(label) = args.one("-l") else {
            return Ok(usage("ready needs -l in this fake"));
        };
        let ids = match self.backend.list_ready(
            &ProjectRef::local(None),
            &ScopeFilter::Label(label.to_owned()),
            d,
        ) {
            Ok(ids) => ids,
            Err(e) => return failed(e),
        };
        let rows: Vec<Bead> = ids
            .iter()
            .filter_map(|id| self.backend.row(id.as_str()))
            .collect();
        Ok(success(beads_json(&rows)))
    }

    fn close(&self, rest: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let args = match Args::parse(rest, &["--reason", "--actor"], &[]) {
            Ok(a) => a,
            Err(m) => return Ok(usage(&m)),
        };
        let (ids, actor) = match ids_and_actor(&args, 1) {
            Ok(v) => v,
            Err(out) => return Ok(out),
        };
        let Some(id) = ids.first() else {
            return Ok(usage("a bead id is required"));
        };
        let r = self.backend.close_guarded(
            &ProjectRef::local(None),
            id,
            args.one("--reason"),
            &Guards::default(),
            &actor,
            d,
        );
        written(r, "closed")
    }

    fn update(&self, rest: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let args = match Args::parse(
            rest,
            &[
                "--status",
                "--assignee",
                "--add-label",
                "--remove-label",
                "--set-metadata",
                "--unset-metadata",
                "--append-notes",
                "--if-status",
                "--if-assignee",
                "--actor",
            ],
            &[],
        ) {
            Ok(a) => a,
            Err(m) => return Ok(usage(&m)),
        };
        let (ids, actor) = match ids_and_actor(&args, 1) {
            Ok(v) => v,
            Err(out) => return Ok(out),
        };
        let Some(id) = ids.first() else {
            return Ok(usage("a bead id is required"));
        };
        let patch = Patch {
            status: args.one("--status").map(str::to_owned),
            assignee: args.one("--assignee").map(Assignee::from_wire),
            add_labels: args.all("--add-label"),
            remove_labels: args.all("--remove-label"),
            set_metadata: BTreeMap::new(),
            unset_metadata: args.all("--unset-metadata"),
            append_notes: args.one("--append-notes").map(str::to_owned),
        };
        let guards = Guards {
            status: args.one("--if-status").map(str::to_owned),
            assignee: args.one("--if-assignee").map(Assignee::from_wire),
        };
        let mut stored = Vec::new();
        for pair in args.all("--set-metadata") {
            let Some((key, value)) = pair.split_once('=') else {
                return Ok(usage("--set-metadata needs key=value"));
            };
            stored.push((key.to_owned(), coerce_metadata(value)));
        }
        let r =
            self.backend
                .update_guarded(&ProjectRef::local(None), id, &patch, &guards, &actor, d);
        if r.is_ok() && !stored.is_empty() {
            // `MetaValue` cannot carry what bd's coercion may produce (null, a float),
            // so the coerced values are stored on the row directly.
            self.backend.edit(id.as_str(), |row| {
                for (key, value) in stored {
                    row.metadata.insert(key, value);
                }
            });
        }
        written(r, "updated")
    }

    fn dep_add(&self, rest: &[String], d: Deadline) -> Result<CliOutput, BoundedError> {
        let args = match Args::parse(rest, &["--type", "--actor"], &[]) {
            Ok(a) => a,
            Err(m) => return Ok(usage(&m)),
        };
        let (ids, actor) = match ids_and_actor(&args, 2) {
            Ok(v) => v,
            Err(out) => return Ok(out),
        };
        let [id, on] = ids.as_slice() else {
            return Ok(usage("two bead ids are required"));
        };
        let wanted = args.one("--type").unwrap_or("blocks");
        let Some(kind) = DepType::ALL.iter().copied().find(|t| t.as_str() == wanted) else {
            return Ok(failure(
                1,
                format!("Error: invalid dependency type {wanted:?}"),
            ));
        };
        let r = self
            .backend
            .dep_add_typed(&ProjectRef::local(None), id, on, kind, &actor, d);
        written(r, "added")
    }
}

/// The `want` positional bead ids and the required `--actor`.
fn ids_and_actor(args: &Args, want: usize) -> Result<(Vec<BeadId>, Actor), CliOutput> {
    if args.positional.len() != want {
        return Err(usage(&format!(
            "expected {want} bead id(s), got {}",
            args.positional.len()
        )));
    }
    let ids: Result<Vec<BeadId>, _> = args.positional.iter().map(BeadId::new).collect();
    let ids = ids.map_err(|e| failure(1, format!("Error: {e}")))?;
    let actor = args
        .one("--actor")
        .and_then(|a| Actor::new(a).ok())
        .ok_or_else(|| usage("--actor is required by the cache's writes"))?;
    Ok((ids, actor))
}

/// Rows as the JSON array bd prints.
fn beads_json(rows: &[Bead]) -> String {
    values_json(&rows.iter().map(bd_row).collect::<Vec<_>>())
}

/// One row as bd prints it.
///
/// `Bead` keeps only the edge triple of each dependency (the store rebuilds the rest),
/// but bd prints every dependency with its `created_at` (required by `bd schema`) and
/// `metadata`; both are filled in so the strict parser sees what a real bd sends.
fn bd_row(bead: &Bead) -> Value {
    let mut row = serde_json::to_value(bead).unwrap_or(Value::Null);
    if let Some(deps) = row.get_mut("dependencies").and_then(Value::as_array_mut) {
        for dep in deps {
            if let Some(map) = dep.as_object_mut() {
                map.entry("created_at")
                    .or_insert_with(|| json!(bead.created_at));
                map.entry("metadata").or_insert_with(|| json!("{}"));
            }
        }
    }
    row
}

fn values_json(rows: &[Value]) -> String {
    serde_json::to_string_pretty(rows).unwrap_or_else(|_| "[]".to_owned())
}

/// A write's result as bd prints it.
fn written(r: Result<(), BeadsCacheError>, verb: &str) -> Result<CliOutput, BoundedError> {
    match r {
        Ok(()) => Ok(success(format!("{verb}\n"))),
        Err(e) => failed(e),
    }
}

/// A backend failure as the process outcome it stands for.
fn failed(e: BeadsCacheError) -> Result<CliOutput, BoundedError> {
    match e {
        BeadsCacheError::Unavailable {
            timed_out: true, ..
        } => Err(BoundedError::TimedOut),
        BeadsCacheError::Unavailable { reason, .. } => Ok(failure(
            1,
            format!("Error: failed to open database: Dolt server unreachable: {reason}"),
        )),
        BeadsCacheError::Refused(Refusal::Precondition) => Ok(failure(
            13,
            "Error updating: precondition mismatch (nothing was written)",
        )),
        BeadsCacheError::Failed { code, stderr }
        | BeadsCacheError::CheckFailed { code, stderr, .. } => Ok(failure(code, stderr)),
        other => Ok(failure(1, format!("Error: {other}"))),
    }
}

impl CliInvoker for FakeBd {
    fn binary(&self) -> String {
        "bd".to_owned()
    }

    fn client_info(&self) -> Option<CliProbe> {
        self.probe()
    }

    fn probe(&self) -> Option<CliProbe> {
        Some(CliProbe {
            client: CliClient::Bd,
            version: Some((1, 3, 1).into()),
            raw: format!("bd version {BD_VERSION} (dev)"),
        })
    }

    fn run_json(
        &self,
        _project: &ProjectRef,
        _command: &str,
        _args: &[String],
    ) -> Result<String, BeadsError> {
        Err(BeadsError::Unsupported {
            operation: "FakeBd serves the cache calls only",
            client: CliClient::Bd,
        })
    }

    fn run_raw(&self, _project: &ProjectRef, _args: &[&str]) -> Result<CliOutput, BeadsError> {
        Err(BeadsError::Unsupported {
            operation: "FakeBd serves the cache calls only",
            client: CliClient::Bd,
        })
    }

    fn run_bounded(
        &self,
        _project: &ProjectRef,
        argv: &[String],
        timeout: Duration,
    ) -> Result<CliOutput, BoundedError> {
        self.state().calls.push(argv.to_vec());
        let d = Deadline::after(self.backend.clock().as_ref(), timeout);
        self.dispatch(argv, d)
    }
}

#[cfg(test)]
mod tests {
    use sc_beads::{schema_sha256, BD_SCHEMA_SHA256};

    use super::*;

    #[test]
    fn schema_copy_is_the_pinned_schema() {
        assert_eq!(schema_sha256(PINNED_SCHEMA).as_str(), BD_SCHEMA_SHA256);
    }

    #[test]
    fn coercion_matches_what_bd_stores() {
        // The cases of the BD-CACHE baseline ("Metadata values must survive bd's coercion").
        assert_eq!(coerce_metadata("5"), json!(5));
        assert_eq!(coerce_metadata("5.0"), json!(5));
        assert_eq!(coerce_metadata("1e3"), json!(1000));
        assert_eq!(coerce_metadata(" 7"), json!(7));
        assert_eq!(coerce_metadata("true"), json!(true));
        assert_eq!(coerce_metadata("false"), json!(false));
        assert_eq!(coerce_metadata("null"), Value::Null);
        assert_eq!(coerce_metadata("[1]"), json!("[1]"));
        assert_eq!(coerce_metadata(r#"{"x":1}"#), json!(r#"{"x":1}"#));
        assert_eq!(coerce_metadata(r#""5""#), json!(r#""5""#));
        assert_eq!(coerce_metadata("True"), json!("True"));
        assert_eq!(coerce_metadata("abc123"), json!("abc123"));
    }

    #[test]
    fn unknown_flags_and_missing_actor_are_usage_errors() {
        let fake = FakeBd::new(Arc::new(ManualClock::default()));
        let p = ProjectRef::local(None);
        let run = |argv: &[&str]| {
            let argv: Vec<String> = argv.iter().map(|w| (*w).to_owned()).collect();
            fake.run_bounded(&p, &argv, Duration::from_secs(30))
                .unwrap()
        };
        assert_eq!(run(&["close", "b-1", "--bogus"]).status, Some(2));
        assert_eq!(run(&["close", "b-1"]).status, Some(2));
        assert_eq!(run(&["frobnicate"]).status, Some(2));
        assert_eq!(fake.calls().len(), 3);
    }
}
