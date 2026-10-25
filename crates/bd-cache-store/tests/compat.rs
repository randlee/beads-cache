//! Compatibility both ways with the reference `bd_cache.py` (REQ-BDC-0804).
//!
//! The reference implementation, its model and the bd-1.3.1 fixtures are
//! vendored at atm-core `1695220` under `tests/compat/` (see
//! `tests/compat/PROVENANCE.md`) and run with `uv`, as CI installs it.
//! `tests/compat/driver.py` drives them with a deterministic bd; no bd runs.
//! A missing `uv` fails these tests: they are never skipped. The first test to
//! need `uv` resolves its environment alone (`warm_uv`), so parallel tests never
//! race on a cold uv cache.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use bd_cache_model::{
    CacheStore, FingerprintOutcome, Freshness, IdemKey, JournalState, NewEntry, Op, Outage,
    ScopeId, SyncState, normalize,
};
use bd_cache_store::SqliteStore;
use common::{CHILD_TIMEOUT, TempDir, identity, run_bounded, workspace};
use sc_beads::testing::{actor, edge, id};
use sc_beads::{Bead, BlockedRow, DepType, Fingerprint, Guards, Timestamp};
use serde_json::{Value, json};

const ROWS: &str = include_str!("compat/fixtures/bd-1.3.1/list-rows.json");
const SCHEMA: &str = include_str!("compat/fixtures/bd-1.3.1/bd-schema.json");
/// The driver's clock (`CLOCK` in driver.py).
const T0: &str = "2026-10-23T10:00:00Z";

fn compat_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("compat")
}

/// The requirement every `uv run` of the reference implementation resolves.
const PYDANTIC: &str = "pydantic>=2,<3";

/// `uv run --no-project --quiet --with pydantic python`, run in the compat
/// directory, after the one shared warm-up ([`warm_uv`]).
fn uv_python() -> Command {
    warm_uv();
    let mut cmd = Command::new("uv");
    cmd.args([
        "run",
        "--no-project",
        "--quiet",
        "--with",
        PYDANTIC,
        "python",
    ])
    .current_dir(compat_dir());
    cmd
}

/// Resolves and installs the `uv` environment once, before any test's driver
/// runs (PE-FTQ-004).
///
/// The tests run in parallel, and each `uv run --with` resolves against the
/// shared uv cache: on a cold cache several would download and build the same
/// environment at once and race. The first caller does it alone, under the
/// same 180 s bound ([`CHILD_TIMEOUT`]) as every child; the others wait for it.
/// A failed warm-up fails every test that needs it, with its output, instead
/// of each racing on its own.
fn warm_uv() {
    static WARM: OnceLock<Result<(), String>> = OnceLock::new();
    let outcome = WARM.get_or_init(|| {
        let mut cmd = Command::new("uv");
        cmd.args([
            "run",
            "--no-project",
            "--quiet",
            "--with",
            PYDANTIC,
            "python",
            "-c",
            "import pydantic",
        ])
        .current_dir(compat_dir());
        let out = run_bounded(&mut cmd, CHILD_TIMEOUT);
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "uv warm-up failed ({}):\n{}\n{}",
                out.status,
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    });
    if let Err(why) = outcome {
        panic!("{why}");
    }
}

/// `uv run ... python driver.py <args>`, bounded; returns stdout parsed as JSON.
fn driver(args: &[&OsStr]) -> Value {
    let mut cmd = uv_python();
    cmd.arg(compat_dir().join("driver.py")).args(args);
    let out = run_bounded(&mut cmd, CHILD_TIMEOUT);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "driver {args:?} failed ({}):\n{stdout}\n{stderr}",
        out.status
    );
    let last = stdout.lines().last().unwrap_or_default();
    serde_json::from_str(last).unwrap_or_else(|e| panic!("driver printed {stdout:?}: {e}"))
}

fn ct() -> ScopeId {
    ScopeId::phase("ct").unwrap()
}

fn zz() -> ScopeId {
    ScopeId::phase("zz").unwrap()
}

fn ts(s: &str) -> Timestamp {
    Timestamp::parse(s).unwrap()
}

fn fixture_rows() -> Vec<Bead> {
    sc_beads::parse_bd_rows(ROWS).unwrap()
}

fn fixture(name: &str) -> Bead {
    fixture_rows()
        .into_iter()
        .find(|b| b.id.as_str() == name)
        .unwrap()
}

fn ids(rows: Vec<(Bead, Freshness)>) -> Vec<String> {
    rows.into_iter().map(|(b, _)| b.id.into_string()).collect()
}

struct Shared {
    _tmp: TempDir,
    root: PathBuf,
    db: PathBuf,
}

impl Shared {
    fn new(tag: &str) -> Self {
        let tmp = TempDir::new(tag);
        let root = tmp.path().to_path_buf();
        workspace(&root, "testdb", "p-1");
        let db = root.join(".atm-bd").join("testdb.bd-cache.sqlite3");
        Self {
            _tmp: tmp,
            root,
            db,
        }
    }

    fn run(&self, command: &str, rest: &[&str]) -> Value {
        let mut args: Vec<&OsStr> = vec![
            OsStr::new(command),
            self.root.as_os_str(),
            self.db.as_os_str(),
        ];
        args.extend(rest.iter().map(OsStr::new));
        driver(&args)
    }

    fn open(&self) -> SqliteStore {
        SqliteStore::open_at(&self.db, &identity()).unwrap()
    }
}

fn new_entry(key: &str, bead: &str, scope: &ScopeId, op: Op) -> NewEntry {
    NewEntry {
        idem_key: IdemKey::new(key).unwrap(),
        scope: Some(scope.clone()),
        bead: id(bead),
        op,
        precondition: Guards::default(),
        actor: actor("dev"),
        changed: Some(true),
    }
}

#[test]
fn a_python_written_v3_file_opens_and_reads_back_equal() {
    let f = Shared::new("py-wrote");
    let wrote = f.run("write", &[]);
    assert_eq!(wrote["stored"], 3);

    let store = f.open();
    for row in fixture_rows() {
        assert_eq!(
            store.get_row(&row.id).unwrap(),
            Some((normalize(&row), Freshness::Fresh)),
            "{}",
            row.id
        );
    }
    assert_eq!(
        ids(store.rows_in_scope(&ct()).unwrap()),
        ["ct-1", "ct-2", "ct-3"]
    );
    assert_eq!(
        store.dependents_of(&id("ct-1")).unwrap(),
        [edge("ct-2", "ct-1", DepType::Blocks)]
    );

    let pending = store.pending_entries().unwrap();
    assert_eq!(pending.len(), 2);
    let (first, second) = (&pending[0], &pending[1]);
    assert_eq!(first.seq.get(), 1);
    assert_eq!(first.idem_key.as_str(), "py-1");
    assert_eq!(first.scope, Some(ct()));
    assert_eq!(first.bead, id("ct-3"));
    assert_eq!(
        first.op,
        Op::parse("update", r#"{"add_labels": ["reviewed"]}"#).unwrap()
    );
    assert_eq!(first.actor, actor("dev"));
    assert_eq!(first.state, JournalState::Pending);
    assert_eq!(first.created_at, ts(T0));
    assert_eq!(first.precondition, Guards::default());
    assert_eq!(
        second.op,
        Op::Close {
            reason: Some("done".into())
        }
    );
    assert_eq!(second.precondition.status.as_deref(), Some("in_progress"));

    let sync = store.sync_state(&ct()).unwrap();
    assert_eq!(
        (sync.last_started_at, sync.last_ok_at, sync.last_error),
        (Some(ts(T0)), Some(ts(T0)), None)
    );
    assert_eq!(
        store.sync_alert(&ct()).unwrap().as_deref(),
        Some("BD_CACHE.SYNC_FAILED: boom")
    );
    assert_eq!(
        store.outage().unwrap(),
        Some(Outage {
            since: ts(T0),
            last_error: "BD_CACHE.BD_UNAVAILABLE: refused".into()
        })
    );
    let status = store.status().unwrap();
    assert_eq!(status.journal.get("ct:pending"), Some(&2));
    assert_eq!(
        status
            .scopes
            .iter()
            .find(|s| s.sync.scope == ct())
            .map(|s| s.beads),
        Some(3)
    );
    let journal_text: Vec<(String, String)> = {
        drop(store);
        let c = rusqlite::Connection::open(&f.db).unwrap();
        let mut s = c
            .prepare("SELECT args_json, precondition_json FROM journal ORDER BY seq")
            .unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    };
    // The Rust renderings of the parsed entries are byte-identical to Python's text.
    assert_eq!(
        journal_text,
        [
            (
                first.op.to_args_json(),
                bd_cache_model::precondition_json(&first.precondition)
            ),
            (
                second.op.to_args_json(),
                bd_cache_model::precondition_json(&second.precondition)
            ),
        ]
    );
}

#[test]
fn the_python_get_and_status_read_a_file_the_rust_store_wrote() {
    let f = Shared::new("rs-wrote");
    let mut store = f.open();
    store.ensure_scope(&ct()).unwrap();
    store
        .upsert_rows(Some(&ct()), &fixture_rows(), false)
        .unwrap();
    let pending = store
        .insert_entry(
            new_entry(
                "rs-1",
                "ct-2",
                &ct(),
                Op::Close {
                    reason: Some("landed".into()),
                },
            ),
            &ts(T0),
        )
        .unwrap();
    let applied = store
        .insert_entry(
            new_entry(
                "rs-2",
                "ct-3",
                &ct(),
                Op::parse("update", r#"{"add_labels": ["x"]}"#).unwrap(),
            ),
            &ts(T0),
        )
        .unwrap();
    store
        .apply_with_readback(applied, None, None, &ts(T0))
        .unwrap();
    let blocked = [BlockedRow {
        id: id("ct-2"),
        blocked_by: vec![id("ct-1")],
    }];
    store
        .replace_ready_base(&ct(), &blocked, &[id("ct-1")], &ts(T0))
        .unwrap();
    let mut sync = SyncState::new(ct());
    sync.last_ok_at = Some(ts(T0));
    store.record_sync(&sync).unwrap();

    // Python reads while the Rust store is open, as two processes share the file.
    let got = f.run("cli", &["get", "ct-1"]);
    assert_eq!(got["ok"], true);
    assert_eq!(
        got["bead"],
        serde_json::to_value(normalize(&fixture("ct-1"))).unwrap()
    );
    let got = f.run("cli", &["get", "ct-2"]);
    assert_eq!(
        (
            got["bead"]["status"].clone(),
            got["bead"]["close_reason"].clone()
        ),
        (json!("closed"), json!("landed")),
        "Python replays the journal text the Rust store wrote"
    );
    let status = f.run("cli", &["status"]);
    assert_eq!(status["meta"]["project_id"], "p-1");
    assert_eq!(status["journal"], json!({"ct:pending": 1, "ct:applied": 1}));
    let phase = &status["phases"][0];
    assert_eq!(
        (phase["phase"].clone(), phase["beads"].clone()),
        (json!("ct"), json!(3))
    );
    assert!(
        phase.get("journal_seq").is_some(),
        "Python reads the extended sync_state: {phase}"
    );
    assert_eq!(phase["last_ok_at"], T0);

    // The extension tables survive Python's open, for this store and the next one.
    check_extensions(&store, pending);
    assert_eq!(
        SqliteStore::open_at(&f.db, &identity()).unwrap_err().code(),
        "BD_CACHE.STORE_IN_USE",
        "still one connection per process"
    );
    drop(store);
    check_extensions(&f.open(), pending);
}

fn check_extensions(store: &SqliteStore, pending: bd_cache_model::Seq) {
    let base = store.ready_base(&ct()).unwrap().unwrap();
    assert_eq!(base.ready, [id("ct-1")]);
    assert_eq!(base.blocked[0].blocked_by, [id("ct-1")]);
    assert_eq!(ids(store.rows_in_scope(&ct()).unwrap()).len(), 3);
    assert_eq!(store.applied_since(&ts(T0)).unwrap().len(), 1);
    assert_eq!(store.scopes().unwrap(), [ct()]);
    assert_eq!(store.pending_entries().unwrap()[0].seq, pending);
}

#[test]
fn after_python_prune_and_rebuild_the_rust_store_reads_consistent_rows() {
    let f = Shared::new("py-prune");
    let row = |name: &str, labels: &[&str]| {
        let mut b = sc_beads::testing::bead(name);
        b.labels = labels.iter().map(|l| (*l).to_owned()).collect();
        b
    };
    let both = row("both", &["phase-ct", "phase-zz"]);
    {
        let mut store = f.open();
        store.ensure_scope(&ct()).unwrap();
        store.ensure_scope(&zz()).unwrap();
        store
            .record_fingerprint(&Fingerprint {
                bd_version: "1.3.1".into(),
                schema_sha256: sc_beads::schema_sha256(SCHEMA),
                schema_version: "1".into(),
            })
            .unwrap();
        store
            .upsert_rows(Some(&ct()), &[row("a", &["phase-ct"]), both.clone()], false)
            .unwrap();
        store
            .upsert_rows(Some(&zz()), &[both.clone(), row("z", &["phase-zz"])], false)
            .unwrap();
        store
            .insert_entry(
                new_entry("open-z", "z", &zz(), Op::Close { reason: None }),
                &ts(T0),
            )
            .unwrap();
        for s in [ct(), zz()] {
            let mut st = SyncState::new(s.clone());
            st.last_ok_at = Some(ts(T0));
            store.record_sync(&st).unwrap();
        }
    }

    // bd_cache.py prunes by the v3 phase column: `both` (phase ct) goes too.
    let pruned = f.run("cli", &["prune", "--phase", "ct"]);
    assert_eq!(pruned["pruned_beads"], 2);
    {
        let mut store = f.open();
        assert!(store.get_row(&id("a")).unwrap().is_none());
        assert!(store.get_row(&id("both")).unwrap().is_none());
        assert!(ids(store.rows_in_scope(&ct()).unwrap()).is_empty());
        assert_eq!(ids(store.rows_in_scope(&zz()).unwrap()), ["z"]);
        let counts: Vec<(ScopeId, u64)> = store
            .status()
            .unwrap()
            .scopes
            .into_iter()
            .map(|s| (s.sync.scope, s.beads))
            .collect();
        assert_eq!(counts, [(ct(), 0), (zz(), 1)]);
        assert_eq!(store.sync_state(&ct()).unwrap(), SyncState::new(ct()));
        assert_eq!(store.sync_state(&zz()).unwrap().last_ok_at, Some(ts(T0)));
        // Stored again unscoped, `both` is a new out-of-scope row: no stale membership revives.
        store
            .upsert_rows(None, std::slice::from_ref(&both), false)
            .unwrap();
        assert_eq!(ids(store.rows_in_scope(&zz()).unwrap()), ["z"]);
    }

    // A changed bd schema: bd_cache.py's backend check rebuilds.
    assert_eq!(f.run("rebuild", &[])["rebuilt"], true);
    let mut store = f.open();
    for name in ["a", "both", "z"] {
        assert!(store.get_row(&id(name)).unwrap().is_none(), "{name}");
    }
    assert!(store.rows_in_scope(&zz()).unwrap().is_empty());
    assert!(store.status().unwrap().scopes.iter().all(|s| s.beads == 0));
    assert_eq!(
        store.scopes().unwrap(),
        [ct(), zz()],
        "the scope registry stays"
    );
    assert_eq!(
        store.pending_entries().unwrap().len(),
        1,
        "the journal stays"
    );
    assert_eq!(store.sync_state(&zz()).unwrap(), SyncState::new(zz()));
    assert_eq!(
        store
            .record_fingerprint(&Fingerprint {
                bd_version: "1.3.1".into(),
                schema_sha256: sc_beads::schema_sha256(r#"{"schema_version": "changed"}"#),
                schema_version: "changed".into(),
            })
            .unwrap(),
        FingerprintOutcome::Unchanged,
        "the sha Python recorded is the one the store sees"
    );
}

#[cfg(unix)]
#[test]
fn the_python_and_rust_drain_locks_exclude_each_other() {
    use bd_cache_model::DrainLock;
    use bd_cache_store::FlockDrainLock;
    use common::Holder;

    let f = Shared::new("py-lock");
    drop(f.open());
    let lock = FlockDrainLock::for_cache_file(&f.db);
    let python = |f: &Shared| {
        let mut cmd = uv_python();
        cmd.arg(compat_dir().join("driver.py"))
            .arg("hold")
            .arg(&f.root)
            .arg(&f.db);
        Holder::spawn(&mut cmd, "\"held\"", CHILD_TIMEOUT)
    };
    let (held, line) = python(&f);
    assert!(line.contains("\"held\": true"), "{line}");
    assert!(lock.try_acquire().unwrap().is_none(), "Python holds it");
    held.release(CHILD_TIMEOUT);
    let guard = lock.try_acquire().unwrap();
    assert!(guard.is_some());
    let (held, line) = python(&f);
    assert!(line.contains("\"held\": false"), "Rust holds it: {line}");
    held.release(CHILD_TIMEOUT);
    drop(guard);
}

/// REQ-BDC-0804, sprint b-1: the columns `last_error_code`, `note` and
/// `changed` are additive and nullable at `user_version` 3, both ways.
///
/// `bd_cache.py` opens a file the Rust store wrote with the new columns,
/// escalates (as its drain does) and resolves entries. The Rust store reads
/// them back with `note` split out of `last_error` and `last_error_code`
/// taken from the `CODE: ` prefix where `bd_cache.py` left the column NULL or
/// stale. An entry the Rust store resolved has its note in `note`, and
/// `last_error` left as it was.
#[test]
fn python_resolves_a_rust_written_entry_and_the_rust_store_splits_it_back() {
    let f = Shared::new("b1-columns");
    let [
        rust_escalated,
        python_escalated,
        python_resolved,
        rust_resolved,
    ] = seed_rust_entries(&f);

    // bd_cache.py opens the file with the new columns and writes to it.
    let seq = |s: bd_cache_model::Seq| s.get().to_string();
    assert_eq!(
        f.run(
            "escalate",
            &[&seq(python_escalated), "BD_CACHE.NOT_FOUND: gone"]
        )["ok"],
        true
    );
    assert_eq!(
        f.run(
            "escalate",
            &[&seq(python_resolved), "BD_CACHE.PRECONDITION: x"]
        )["ok"],
        true
    );
    for (s, note) in [(rust_escalated, "set by hand"), (python_resolved, "again")] {
        let out = f.run(
            "cli",
            &["resolve", &seq(s), "--note", note, "--actor", "lead-1"],
        );
        assert_eq!(out["state"], "resolved", "{out}");
    }
    let status = f.run("cli", &["status"]);
    assert_eq!(
        status["journal"],
        json!({"ct:resolved": 3, "ct:escalated": 1}),
        "{status}"
    );

    let store = f.open();
    let read = |s| store.entry_by_seq(s).unwrap().unwrap();
    let e = read(rust_escalated);
    assert_eq!(e.state, JournalState::Resolved);
    assert_eq!(
        e.last_error.as_deref(),
        Some("BD_CACHE.PRECONDITION: status is closed")
    );
    assert_eq!(e.note.as_deref(), Some("resolved by lead-1: set by hand"));
    assert_eq!(e.last_error_code.as_deref(), Some("BD_CACHE.PRECONDITION"));
    assert_eq!(e.changed, Some(true));
    assert_eq!(
        e.legacy_last_error().as_deref(),
        Some("BD_CACHE.PRECONDITION: status is closed | resolved by lead-1: set by hand"),
        "the 0.1 rendering is the text bd_cache.py stored"
    );

    let e = read(python_escalated);
    assert_eq!(e.state, JournalState::Escalated);
    assert_eq!(e.last_error.as_deref(), Some("BD_CACHE.NOT_FOUND: gone"));
    assert_eq!(
        e.last_error_code.as_deref(),
        Some("BD_CACHE.NOT_FOUND"),
        "the stale column is not the code of the text bd_cache.py wrote"
    );
    assert_eq!((e.note, e.changed), (None, Some(false)));

    let e = read(python_resolved);
    assert_eq!(e.last_error.as_deref(), Some("BD_CACHE.PRECONDITION: x"));
    assert_eq!(e.note.as_deref(), Some("resolved by lead-1: again"));
    assert_eq!(e.last_error_code.as_deref(), Some("BD_CACHE.PRECONDITION"));
    assert_eq!(e.changed, None, "journaled without changed");

    let e = read(rust_resolved);
    assert_eq!(
        (e.last_error.as_deref(), e.note.as_deref()),
        (
            Some("BD_CACHE.PRECONDITION: assignee"),
            Some("resolved by lead: by hand")
        )
    );
    drop(store);

    check_raw_columns(&f, rust_resolved, python_resolved);
}

/// The mixed cutover (I9): the Rust store journals two writes, `bd_cache.py`'s
/// drain applies them through a fake bd that accepts writes, and the Rust store
/// reads the result back.
///
/// Asserted on both sides: the bd commands the drain ran (the precondition
/// became `--if-status`), the bead rows after the read-back, the entries'
/// state as Python and as Rust read them, and that nothing is left pending.
#[test]
fn python_drains_entries_the_rust_store_wrote_and_the_rust_store_reads_the_result() {
    let f = Shared::new("mixed-drain");
    let (close, update) = seed_close_and_update(&f);

    let out = f.run("drain", &[]);
    assert_eq!(
        out["report"]["applied"],
        json!([close.get(), update.get()]),
        "{out}"
    );
    assert_eq!(out["report"]["escalated"], json!([]), "{out}");
    assert_eq!(out["report"]["retrying"], json!([]), "{out}");
    assert_eq!(
        out["commands"],
        json!([
            [
                "bd", "close", "ct-2", "--reason", "landed", "--actor", "dev"
            ],
            [
                "bd",
                "update",
                "ct-1",
                "--add-label",
                "reviewed",
                "--if-status",
                "in_progress",
                "--actor",
                "dev"
            ]
        ]),
        "{out}"
    );
    for seq in [close, update] {
        assert_eq!(out["entries"][seq.get().to_string()]["state"], "applied");
    }
    assert_eq!(out["beads"]["ct-2"]["status"], "closed", "{out}");
    assert_eq!(out["beads"]["ct-2"]["close_reason"], "landed", "{out}");

    let store = f.open();
    assert!(store.pending_entries().unwrap().is_empty());
    for seq in [close, update] {
        let e = store.entry_by_seq(seq).unwrap().unwrap();
        assert_eq!(e.state, JournalState::Applied, "seq {seq:?}");
        assert_eq!(e.attempts, 0, "seq {seq:?}");
        assert_eq!(e.last_error, None, "seq {seq:?}");
    }
    let row = |name: &str| {
        let (bead, freshness) = store.get_row(&id(name)).unwrap().unwrap();
        (serde_json::to_value(&bead).unwrap(), freshness)
    };
    let (closed, freshness) = row("ct-2");
    assert_eq!(
        (closed["status"].as_str(), closed["close_reason"].as_str()),
        (Some("closed"), Some("landed"))
    );
    assert_eq!(freshness, Freshness::Fresh);
    let (updated, _) = row("ct-1");
    assert_eq!(updated["status"], "in_progress");
    assert!(
        updated["labels"]
            .as_array()
            .unwrap()
            .contains(&json!("reviewed")),
        "{updated}"
    );
    assert_eq!(
        row("ct-3").0["status"],
        "closed",
        "an untouched bead is unchanged"
    );
}

/// The two entries of the mixed-cutover test, as the Rust store writes them:
/// a close with a reason, and an update guarded by `status`.
fn seed_close_and_update(f: &Shared) -> (bd_cache_model::Seq, bd_cache_model::Seq) {
    let mut store = f.open();
    store.ensure_scope(&ct()).unwrap();
    store
        .upsert_rows(Some(&ct()), &fixture_rows(), false)
        .unwrap();
    let close = store
        .insert_entry(
            new_entry(
                "rs-close",
                "ct-2",
                &ct(),
                Op::Close {
                    reason: Some("landed".into()),
                },
            ),
            &ts(T0),
        )
        .unwrap();
    let update = store
        .insert_entry(
            NewEntry {
                precondition: Guards {
                    status: Some("in_progress".into()),
                    assignee: None,
                },
                ..new_entry(
                    "rs-update",
                    "ct-1",
                    &ct(),
                    Op::parse("update", r#"{"add_labels": ["reviewed"]}"#).unwrap(),
                )
            },
            &ts(T0),
        )
        .unwrap();
    (close, update)
}

/// The four entries of
/// [`python_resolves_a_rust_written_entry_and_the_rust_store_splits_it_back`],
/// as the Rust store writes them, in seq order.
fn seed_rust_entries(f: &Shared) -> [bd_cache_model::Seq; 4] {
    let mut store = f.open();
    store.ensure_scope(&ct()).unwrap();
    store
        .upsert_rows(Some(&ct()), &fixture_rows(), false)
        .unwrap();
    let close = || Op::Close { reason: None };
    let journal = |store: &mut SqliteStore, key: &str, bead: &str, changed| {
        store
            .insert_entry(
                NewEntry {
                    changed,
                    ..new_entry(key, bead, &ct(), close())
                },
                &ts(T0),
            )
            .unwrap()
    };
    // rs-1: escalated by Rust (code column set), resolved by Python.
    let rust_escalated = journal(&mut store, "rs-1", "ct-2", Some(true));
    store
        .escalate(
            rust_escalated,
            "BD_CACHE.PRECONDITION: status is closed",
            &ts(T0),
        )
        .unwrap();
    // rs-2: an attempt recorded by Rust, escalated by Python (stale code column).
    let python_escalated = journal(&mut store, "rs-2", "ct-3", Some(false));
    store
        .record_attempt(
            python_escalated,
            "BD_CACHE.BD_WRITE_FAILED: exit 1",
            &ts(T0),
        )
        .unwrap();
    // rs-3: escalated and resolved by Python (code column NULL).
    let python_resolved = journal(&mut store, "rs-3", "ct-1", None);
    // rs-4: escalated and resolved by Rust.
    let rust_resolved = journal(&mut store, "rs-4", "ct-1", Some(true));
    store
        .escalate(rust_resolved, "BD_CACHE.PRECONDITION: assignee", &ts(T0))
        .unwrap();
    store
        .resolve(rust_resolved, "by hand", &actor("lead"), &ts(T0))
        .unwrap();
    drop(store);
    [
        rust_escalated,
        python_escalated,
        python_resolved,
        rust_resolved,
    ]
}

/// The stored columns: `user_version` 3, the Rust resolve's `note`, and
/// `bd_cache.py`'s text as it wrote it.
fn check_raw_columns(
    f: &Shared,
    rust_resolved: bd_cache_model::Seq,
    python_resolved: bd_cache_model::Seq,
) {
    let c = rusqlite::Connection::open(&f.db).unwrap();
    let version: i32 = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 3, "user_version is unchanged");
    let raw = |s: bd_cache_model::Seq| -> (Option<String>, Option<String>, Option<String>) {
        c.query_row(
            "SELECT last_error, last_error_code, note FROM journal WHERE seq = ?1",
            [s.get()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    };
    assert_eq!(
        raw(rust_resolved),
        (
            Some("BD_CACHE.PRECONDITION: assignee".into()),
            Some("BD_CACHE.PRECONDITION".into()),
            Some("resolved by lead: by hand".into())
        ),
        "the Rust resolve writes note and leaves last_error as it was"
    );
    assert_eq!(
        raw(python_resolved),
        (
            Some("BD_CACHE.PRECONDITION: x | resolved by lead-1: again".into()),
            None,
            None
        ),
        "bd_cache.py's columns as it wrote them: the split is on read only"
    );
}
