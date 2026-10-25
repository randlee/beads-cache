//! Acceptance 1 of sprint b-3 (REQ-BDC-0801): the shared fixtures replay through `run()`.
//!
//! b-2's fixtures (`crates/bd-cache-api/tests/fixtures/*.json`) run with each
//! step's request given whole through `--input`, so the CLI hands the decoder
//! exactly what the bindings hand it. This crate's own fixture
//! (`tests/fixtures/cli_commands.json`) types every command of contract §5.2
//! and §6 as argv. Both run in process: `run()` over a real SQLite cache file
//! in a temporary repository, with sc-bd-cli's `FakeBd` over the fixture's
//! world as the `bd` invoker.
//!
//! The world, the facts and the matcher are b-2's
//! (`crates/bd-cache-api/tests/fixtures/README.md`). The facts are real
//! here: `/repo` in an expected value stands for the temporary repository.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use sc_bd_cli::testing::FakeBd;
use sc_beads::testing::{FakeBackend, ManualClock, bead};
use sc_beads::{Clock, DepType};
use serde_json::{Map, Value, json};
use support::{DATABASE, PROJECT, metadata, parse_envelope};
use tempfile::TempDir;

/// Where every world's clock starts (b-2's fixture README).
const START: &str = "2026-10-23T10:00:00Z";

/// b-2's fixture files, each with a test below.
const API_FIXTURES: &[&str] = &[
    "decode_refusals",
    "idempotency_key_required",
    "issues_list_filters",
    "reads",
    "writes",
];

fn api_fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../bd-cache-api/tests/fixtures")
}

fn cli_fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// One fixture's world: the fake bd, its clock, and a repository on disk.
struct World {
    bd: FakeBackend,
    clock: Arc<ManualClock>,
    dir: TempDir,
}

impl World {
    fn new(fixture: &Value) -> Self {
        let clock = Arc::new(ManualClock::new(START));
        let bd = FakeBackend::new(Arc::clone(&clock));
        let dir = TempDir::new().unwrap();
        let beads = dir.path().join("repo").join(".beads");
        std::fs::create_dir_all(&beads).unwrap();
        std::fs::write(beads.join("metadata.json"), metadata(DATABASE, PROJECT)).unwrap();
        std::fs::write(beads.join("bd-cache.toml"), "scopes = [\"x\"]\n").unwrap();
        let w = Self { bd, clock, dir };
        let spec = &fixture["world"];
        for b in spec["beads"].as_array().into_iter().flatten() {
            let obj = b.as_object().unwrap();
            w.put(obj["id"].as_str().unwrap(), obj, true);
        }
        for e in spec["edges"].as_array().into_iter().flatten() {
            let kind = DepType::parse(e[2].as_str().unwrap()).unwrap();
            assert!(
                w.bd.add_edge(e[0].as_str().unwrap(), e[1].as_str().unwrap(), kind),
                "edge {e}"
            );
        }
        for s in spec["sync"].as_array().into_iter().flatten() {
            let (exit, out) = w.run(&["cache", "refresh", "scopes", s.as_str().unwrap()]);
            assert_eq!(exit, 0, "sync {s}: {out}");
            assert_eq!(
                serde_json::from_str::<Value>(&out).unwrap()["data"]["failed"],
                0,
                "{out}"
            );
        }
        w
    }

    fn root(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    /// Stores `fields` merged over the bead's current row (or over
    /// `sc_beads::testing::bead(id)`, labelled `phase-x`, for a new one).
    fn put(&self, id: &str, fields: &Map<String, Value>, new: bool) {
        let base = if new {
            let mut b = serde_json::to_value(bead(id)).unwrap();
            b["labels"] = json!(["phase-x"]);
            b
        } else {
            let row = self.bd.rows().into_iter().find(|b| b.id.as_str() == id);
            serde_json::to_value(row.unwrap_or_else(|| panic!("no bead {id}"))).unwrap()
        };
        let mut row = base;
        for (k, v) in fields {
            row[k] = v.clone();
        }
        self.bd
            .insert(sc_beads::parse_bd_row(&row).unwrap_or_else(|e| panic!("bead {id}: {e}")));
    }

    /// `run(["--repo", <repo>, args...])` with the fake bd as the invoker.
    fn run(&self, args: &[&str]) -> (i32, String) {
        let root = self.root();
        let mut full = vec!["--repo".to_owned(), root.to_str().unwrap().to_owned()];
        full.extend(args.iter().map(|a| (*a).to_owned()));
        let clock: Arc<dyn Clock> = Arc::clone(&self.clock) as Arc<dyn Clock>;
        bd_cache_cli::run(
            &full,
            &BTreeMap::new(),
            Some(Box::new(FakeBd::over(self.bd.clone()))),
            clock,
        )
    }

    /// As [`World::run`], with `env` as the environment.
    fn run_env(&self, args: &[&str], env: &[(&str, &str)]) -> (i32, String) {
        let root = self.root();
        let mut full = vec!["--repo".to_owned(), root.to_str().unwrap().to_owned()];
        full.extend(args.iter().map(|a| (*a).to_owned()));
        let env: BTreeMap<String, String> = env
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        let clock: Arc<dyn Clock> = Arc::clone(&self.clock) as Arc<dyn Clock>;
        bd_cache_cli::run(
            &full,
            &env,
            Some(Box::new(FakeBd::over(self.bd.clone()))),
            clock,
        )
    }

    /// `/repo` in an expected string is this world's repository root.
    fn localize(&self, want: &Value) -> Value {
        match want {
            Value::String(s) => match s.strip_prefix("/repo") {
                Some(rest) => {
                    let mut p = self.root();
                    for part in rest.split('/').filter(|p| !p.is_empty()) {
                        p.push(part);
                    }
                    Value::String(p.display().to_string())
                }
                None => want.clone(),
            },
            Value::Array(items) => Value::Array(items.iter().map(|v| self.localize(v)).collect()),
            Value::Object(map) => Value::Object(
                map.iter()
                    .map(|(k, v)| (k.clone(), self.localize(v)))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
}

/// b-2's matcher: an object matches when each of its keys matches, an array
/// when it has the same length and each element matches, `"$any"` matches any
/// present value, `"$absent"` requires the key to be missing.
fn mismatches(path: &str, want: &Value, got: Option<&Value>, out: &mut Vec<String>) {
    match (want, got) {
        (Value::String(s), None) if s == "$absent" => {}
        (Value::String(s), Some(g)) if s == "$absent" => {
            out.push(format!("{path}: expected absent, got {g}"));
        }
        (_, None) => out.push(format!("{path}: missing, expected {want}")),
        (Value::String(s), Some(_)) if s == "$any" => {}
        (Value::Object(w), Some(Value::Object(g))) => {
            for (k, wv) in w {
                mismatches(&format!("{path}.{k}"), wv, g.get(k), out);
            }
        }
        (Value::Array(w), Some(Value::Array(g))) => {
            if w.len() == g.len() {
                for (i, (wv, gv)) in w.iter().zip(g).enumerate() {
                    mismatches(&format!("{path}[{i}]"), wv, Some(gv), out);
                }
            } else {
                out.push(format!(
                    "{path}: expected {} items, got {}: {}",
                    w.len(),
                    g.len(),
                    Value::Array(g.clone())
                ));
            }
        }
        (w, Some(g)) => {
            if w != g {
                out.push(format!("{path}: expected {w}, got {g}"));
            }
        }
    }
}

/// The envelope as the fixtures' `{ok, data}` / `{ok, error}`.
fn outcome(exit: i32, stdout: &str) -> Value {
    let envelope = parse_envelope(stdout).unwrap_or_else(|e| panic!("{e}"));
    let v: Value = serde_json::from_str(stdout.trim_end()).unwrap();
    if envelope.is_ok() {
        assert_eq!(exit, 0, "ok: true exits 0: {stdout}");
        json!({"ok": true, "data": v["data"]})
    } else {
        assert_ne!(exit, 0, "ok: false never exits 0: {stdout}");
        json!({"ok": false, "error": v["error"]})
    }
}

/// Runs one fixture file through `run()`; returns its failures, each naming
/// the step.
fn replay(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap();
    let fixture: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: not JSON: {e}", path.display()));
    let world = World::new(&fixture);
    let mut failures = Vec::new();
    for (i, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
        let name = step["name"].as_str().unwrap_or("");
        if let Some(update) = step.get("bd_update") {
            let obj = update.as_object().unwrap();
            world.put(obj["id"].as_str().unwrap(), obj, false);
            continue;
        }
        let (exit, stdout) = if let Some(argv) = step.get("argv") {
            {
                let argv: Vec<&str> = argv
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|a| a.as_str().unwrap())
                    .collect();
                world.run(&argv)
            }
        } else {
            {
                let file = world.dir.path().join(format!("request-{i}.json"));
                std::fs::write(&file, step["request"].to_string()).unwrap();
                let mut argv: Vec<&str> = step["command"].as_str().unwrap().split(' ').collect();
                argv.push("--input");
                argv.push(file.to_str().unwrap());
                world.run(&argv)
            }
        };
        let got = outcome(exit, &stdout);
        let mut out = Vec::new();
        mismatches("", &world.localize(&step["expect"]), Some(&got), &mut out);
        failures.extend(
            out.into_iter()
                .map(|m| format!("step {i} ({name}){m}\n  got: {got}")),
        );
    }
    failures
}

fn check(dir: &Path, name: &str) {
    let failures = replay(&dir.join(format!("{name}.json")));
    assert!(failures.is_empty(), "{name}.json:\n{}", failures.join("\n"));
}

/// REQ-BDC-0807 through the CLI: unknown members, unserved `/v0` names.
#[test]
fn decode_refusals() {
    check(&api_fixtures_dir(), "decode_refusals");
}

/// REQ-BDC-0004 through the CLI: a write without an idempotency key.
#[test]
fn idempotency_key_required() {
    check(&api_fixtures_dir(), "idempotency_key_required");
}

/// Contract §5.3 `issues list` through the CLI.
#[test]
fn issues_list_filters() {
    check(&api_fixtures_dir(), "issues_list_filters");
}

/// Contract §5.3 reads through the CLI.
#[test]
fn reads() {
    check(&api_fixtures_dir(), "reads");
}

/// REQ-BDC-0809, §5.1 writes through the CLI.
#[test]
fn writes() {
    check(&api_fixtures_dir(), "writes");
}

/// Every command of contract §5.2 and §6, typed as argv.
#[test]
fn cli_commands() {
    check(&cli_fixtures_dir(), "cli_commands");
}

/// Every b-2 fixture file has a test here, so a new shared fixture is replayed too.
#[test]
fn every_shared_fixture_file_is_replayed() {
    let mut files: Vec<String> = std::fs::read_dir(api_fixtures_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter_map(|e| {
            e.file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".json"))
                .map(str::to_owned)
        })
        .collect();
    files.sort();
    assert_eq!(
        files, API_FIXTURES,
        "add a test for each shared fixture file"
    );
}

/// Every command of the contract (§1 table, §5.2, §6) has an argv step in
/// `cli_commands.json`.
#[test]
fn every_contract_command_has_an_argv_step() {
    let text = std::fs::read_to_string(cli_fixtures_dir().join("cli_commands.json")).unwrap();
    let fixture: Value = serde_json::from_str(&text).unwrap();
    let typed: Vec<&str> = fixture["steps"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|s| s.get("argv").is_some())
        .filter_map(|s| s["command"].as_str())
        .collect();
    let contract = [
        "issues get",
        "issues list",
        "issues update",
        "issues close",
        "ready",
        "dependencies list",
        "dependencies add",
        "context",
        "cache status",
        "cache health",
        "cache scopes",
        "cache refresh scopes",
        "cache refresh issues",
        "cache flush",
        "cache evict",
        "cache journal get",
        "cache journal list",
        "cache journal resolve",
        "cache journal announce",
        "cache tick",
    ];
    for c in contract {
        assert!(typed.contains(&c), "no argv step for {c}");
    }
    // The argv of each step really starts with its command's words.
    for s in fixture["steps"].as_array().unwrap() {
        if let (Some(argv), Some(command)) = (s.get("argv"), s["command"].as_str()) {
            let words: Vec<&str> = argv
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .take(command.split(' ').count())
                .collect();
            assert_eq!(words.join(" "), command, "{s}");
        }
    }
}

/// Lead ruling S72: with an injected invoker, env and flag overrides apply
/// exactly as with the configured bd. An env `CACHE_PATH` and then a
/// `--cache-path` flag each put the cache file where they name, and the file
/// `.beads/bd-cache.toml` would give is never created.
#[test]
fn overrides_apply_through_an_injected_invoker() {
    let w = World::new(&json!({"world": {"beads": [{"id": "bd-1"}]}}));
    let refresh = ["cache", "refresh", "scopes", "x"];

    let (exit, out) = w.run_env(
        &refresh,
        &[("BD_CACHE__CACHE_PATH", ".bd-cache/from-env.sqlite3")],
    );
    assert_eq!(exit, 0, "{out}");
    assert!(
        w.root().join(".bd-cache/from-env.sqlite3").exists(),
        "{out}"
    );

    let mut args = vec!["--cache-path", ".bd-cache/from-flag.sqlite3"];
    args.extend(refresh);
    let (exit, out) = w.run_env(
        &args,
        &[("BD_CACHE__CACHE_PATH", ".bd-cache/from-env.sqlite3")],
    );
    assert_eq!(exit, 0, "{out}");
    assert!(
        w.root().join(".bd-cache/from-flag.sqlite3").exists(),
        "{out}"
    );

    let mut entries: Vec<String> = std::fs::read_dir(w.root().join(".bd-cache"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".sqlite3"))
        .collect();
    entries.sort();
    assert_eq!(
        entries,
        ["from-env.sqlite3", "from-flag.sqlite3"],
        "only the overridden cache files exist"
    );
}
