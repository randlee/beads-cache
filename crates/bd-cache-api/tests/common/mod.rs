//! The shared-fixture world and runner (format: `tests/fixtures/README.md`).
//!
//! A fixture seeds a `FakeBackend`, syncs scopes into a `FakeStore`, then
//! runs its steps: each step decodes a request for a command, executes it
//! and matches the result against `expect`. b-3 replays the same files
//! through `bd_cache_cli::run`, and b-4 through Python.

#![allow(
    dead_code,
    unreachable_pub,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap is a test failure; each test file uses a subset"
)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bd_cache::Cache;
use bd_cache_api::{ApiError, CacheFacts, Command, decode, execute};
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use bd_cache_model::{CacheConfig, LEGACY_SCOPE_PREFIX, Scope, ScopeId};
use sc_beads::testing::{FakeBackend, ManualClock, bead};
use sc_beads::{Clock, Deadline, DepType, ProjectRef};
use serde_json::{Map, Value, json};

/// The cache under test.
pub type TestCache = Cache<FakeStore, FakeDrainLock, FakeBackend>;

/// Where every world's clock starts.
pub const START: &str = "2026-10-23T10:00:00Z";

/// The facts every fixture's `context` and `cache status` see, unless it
/// gives its own `facts`.
#[must_use]
pub fn default_facts() -> CacheFacts {
    CacheFacts {
        cache_path: "/repo/.bd-cache/beads.bd-cache.sqlite3".into(),
        store_schema_version: 3,
        database: "beads".into(),
        project_id: "proj-1".into(),
        beads_dir: Some("/repo/.beads".into()),
        repo_root: Some("/repo".into()),
        sync: None,
    }
}

/// A world: bd, store, lock and clock, as shared handles.
pub struct World {
    /// The clock every part reads.
    pub clock: Arc<ManualClock>,
    /// bd.
    pub bd: FakeBackend,
    /// The store.
    pub store: FakeStore,
    /// The drain lock.
    pub lock: FakeDrainLock,
    /// The configured scopes.
    pub scopes: Vec<ScopeId>,
}

impl World {
    /// An empty world configured with scope `x`.
    #[must_use]
    pub fn new() -> Self {
        let clock = Arc::new(ManualClock::new(START));
        Self {
            bd: FakeBackend::new(Arc::clone(&clock)),
            clock,
            store: FakeStore::new(),
            lock: FakeDrainLock::new(),
            scopes: vec![ScopeId::new("x").unwrap()],
        }
    }

    /// Adds a bead to bd: `fields` merged over `sc_beads::testing::bead(id)`,
    /// labelled `phase-x` unless `fields` gives `labels`.
    pub fn add(&self, id: &str, fields: &Map<String, Value>) {
        let mut row = serde_json::to_value(bead(id)).unwrap();
        let obj = row.as_object_mut().unwrap();
        obj.insert("labels".into(), json!([format!("{LEGACY_SCOPE_PREFIX}x")]));
        for (k, v) in fields {
            obj.insert(k.clone(), v.clone());
        }
        let b = sc_beads::parse_bd_row(&row).unwrap_or_else(|e| panic!("bead {id}: {e}"));
        self.bd.insert(b);
    }

    /// A cache over this world's parts.
    #[must_use]
    pub fn cache(&self) -> TestCache {
        let clock: Arc<dyn Clock> = Arc::clone(&self.clock) as Arc<dyn Clock>;
        let scopes = self.scopes.iter().cloned().map(Scope::legacy).collect();
        let config = CacheConfig::new(ProjectRef::local(None), scopes);
        Cache::new(
            config,
            self.store.clone(),
            self.lock.clone(),
            self.bd.clone(),
            clock,
        )
        .unwrap()
    }

    /// Syncs a scope into the store.
    pub fn sync(&self, cache: &mut TestCache, scope: &str) {
        // The fixture's beads carry legacy-prefixed labels: phase-a scopes.
        let s = bd_cache_model::legacy_scope(scope).unwrap();
        let d = Deadline::after(&*self.clock, Duration::from_secs(30));
        cache
            .sync_legacy(&s, d)
            .unwrap_or_else(|e| panic!("sync {scope}: {e}"));
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}

/// Decodes and runs one request; the result as the envelope's `ok` / `data`
/// / `error` members.
pub fn run(cache: &mut TestCache, command: &str, request: &Value, facts: &CacheFacts) -> Value {
    let command = Command::parse(command).unwrap_or_else(|| panic!("no command {command:?}"));
    let result: Result<Value, ApiError> = decode(command, request)
        .and_then(|req| execute(cache, &req, facts))
        .and_then(|data| data.to_json());
    match result {
        Ok(data) => json!({"ok": true, "data": data}),
        Err(e) => json!({"ok": false, "error": e.to_json()}),
    }
}

/// Builds the world of a fixture and returns it with its cache.
pub fn world_of(fixture: &Value) -> (World, TestCache) {
    let w = World::new();
    let spec = &fixture["world"];
    if let Some(beads) = spec["beads"].as_array() {
        for b in beads {
            let obj = b.as_object().unwrap();
            let id = obj["id"].as_str().unwrap();
            w.add(id, obj);
        }
    }
    if let Some(edges) = spec["edges"].as_array() {
        for e in edges {
            let kind = DepType::parse(e[2].as_str().unwrap()).unwrap();
            assert!(
                w.bd.add_edge(e[0].as_str().unwrap(), e[1].as_str().unwrap(), kind),
                "edge {e}"
            );
        }
    }
    let mut cache = w.cache();
    if let Some(scopes) = spec["sync"].as_array() {
        for s in scopes {
            w.sync(&mut cache, s.as_str().unwrap());
        }
    }
    (w, cache)
}

/// Matches `got` against the pattern `want`; returns the mismatches.
///
/// An object matches when every key of `want` matches (`"$absent"` requires
/// the key to be missing); an array when it has the same length and each
/// element matches; `"$any"` matches anything present; any other value by
/// equality.
pub fn mismatches(path: &str, want: &Value, got: Option<&Value>, out: &mut Vec<String>) {
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

/// Runs one fixture file; returns its failures, each naming the step.
pub fn run_fixture(path: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(path).unwrap();
    let fixture: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: not JSON: {e}", path.display()));
    let (_w, mut cache) = world_of(&fixture);
    let facts = default_facts();
    let mut failures = Vec::new();
    for (i, step) in fixture["steps"].as_array().unwrap().iter().enumerate() {
        let got = run(
            &mut cache,
            step["command"].as_str().unwrap(),
            &step["request"],
            &facts,
        );
        let mut out = Vec::new();
        mismatches("", &step["expect"], Some(&got), &mut out);
        let name = step["name"].as_str().unwrap_or("");
        failures.extend(
            out.into_iter()
                .map(|m| format!("step {i} ({name}){m}\n  got: {got}")),
        );
    }
    failures
}

/// The fixture directory.
#[must_use]
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
