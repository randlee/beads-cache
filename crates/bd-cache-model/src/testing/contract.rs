//! The contract suite every [`CacheStore`] + [`DrainLock`] implementation must pass (NFR-BDC-0005).
//!
//! One function per rule of the [`CacheStore`] doc comment. An implementation
//! provides a [`StoreFixture`] and instantiates the suite with
//! [`store_contract_tests!`](crate::store_contract_tests):
//!
//! ```ignore
//! bd_cache_model::store_contract_tests!(MySqliteFixture::new());
//! ```
//!
//! `FakeStore` + `FakeDrainLock` pass it in this crate; a-4 runs it against
//! `SqliteStore` and its `flock` drain lock.
//!
//! # Panics
//!
//! Every rule panics (through `assert!`) when the store breaks it.
#![expect(
    clippy::missing_panics_doc,
    reason = "every rule panics on a violation by design; documented once above"
)]
#![expect(
    clippy::indexing_slicing,
    reason = "contract rules index results they have just asserted the shape of"
)]

use sc_beads::testing::{edge, id};
use sc_beads::{Bead, BlockedRow, DepType, Fingerprint, Guards, Sha256Hex, Timestamp};

use super::{FakeDrainLock, FakeStore};
use crate::error::CacheError;
use crate::journal::{IdemKey, JournalState, NewEntry, Seq};
use crate::op::{Op, normalize};
use crate::scope::ScopeId;
use crate::state::{FingerprintOutcome, Freshness, Outage, SyncState};
use crate::store::{CacheStore, DrainLock};

/// Opens the store under test.
pub trait StoreFixture {
    /// The store.
    type Store: CacheStore;
    /// Its drain lock.
    type Lock: DrainLock;

    /// A fresh, empty store.
    fn open(&mut self) -> Self::Store;
    /// Closes `store` and opens the same backing store again (a restart).
    fn reopen(&mut self, store: Self::Store) -> Self::Store;
    /// Two handles on one drain lock (two processes on one cache file).
    fn locks(&mut self) -> (Self::Lock, Self::Lock);
}

/// The fixture for [`FakeStore`] and [`FakeDrainLock`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FakeStoreFixture;

impl StoreFixture for FakeStoreFixture {
    type Store = FakeStore;
    type Lock = FakeDrainLock;

    fn open(&mut self) -> FakeStore {
        FakeStore::new()
    }

    fn reopen(&mut self, store: FakeStore) -> FakeStore {
        let again = store.clone();
        drop(store);
        again
    }

    fn locks(&mut self) -> (FakeDrainLock, FakeDrainLock) {
        let l = FakeDrainLock::new();
        (l.clone(), l)
    }
}

const T0: &str = "2026-10-23T10:00:00Z";
const T1: &str = "2026-10-23T10:00:01Z";
const T2: &str = "2026-10-23T10:00:02Z";

fn x() -> ScopeId {
    scope("x")
}

fn y() -> ScopeId {
    scope("y")
}

#[expect(clippy::panic, reason = "an invalid fixture is a contract-suite bug")]
fn scope(name: &str) -> ScopeId {
    ScopeId::phase(name).unwrap_or_else(|e| panic!("{e}"))
}

#[expect(clippy::panic, reason = "an invalid fixture is a contract-suite bug")]
fn ts(s: &str) -> Timestamp {
    Timestamp::parse(s).unwrap_or_else(|e| panic!("{e}"))
}

#[expect(clippy::panic, reason = "an invalid fixture is a contract-suite bug")]
fn key(s: &str) -> IdemKey {
    IdemKey::new(s).unwrap_or_else(|e| panic!("{e}"))
}

/// A valid row: labels as given, `updated_at` as given.
fn row(name: &str, labels: &[&str], updated_at: &str) -> Bead {
    let mut b = sc_beads::testing::bead(name);
    b.labels = labels.iter().map(|l| (*l).to_owned()).collect();
    updated_at.clone_into(&mut b.updated_at);
    b
}

#[expect(
    clippy::panic,
    reason = "a contract rule reports a failure by panicking, as assert! does"
)]
fn ok<T>(r: Result<T, CacheError>, what: &str) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("{what}: unexpected error {e}"),
    }
}

#[expect(
    clippy::panic,
    reason = "a contract rule reports a failure by panicking, as assert! does"
)]
fn missing<T>() -> T {
    panic!("expected a journal entry or ready base")
}

fn get<S: CacheStore>(s: &S, name: &str) -> Option<(Bead, Freshness)> {
    ok(s.get_row(&id(name)), "get_row")
}

fn ids_in<S: CacheStore>(s: &S, scope: &ScopeId) -> Vec<String> {
    ok(s.rows_in_scope(scope), "rows_in_scope")
        .into_iter()
        .map(|(b, _)| b.id.into_string())
        .collect()
}

fn entry(key: &str, bead: &str, scope: Option<ScopeId>) -> NewEntry {
    NewEntry {
        idem_key: self::key(key),
        scope,
        bead: id(bead),
        op: Op::Close { reason: None },
        precondition: Guards::default(),
        actor: sc_beads::testing::actor("san"),
    }
}

/// A fingerprint whose sha is the digit repeated 64 times.
#[expect(clippy::panic, reason = "an invalid fixture is a contract-suite bug")]
fn fingerprint(digit: char) -> Fingerprint {
    Fingerprint {
        bd_version: "1.3.1".into(),
        schema_sha256: Sha256Hex::new(digit.to_string().repeat(64))
            .unwrap_or_else(|e| panic!("{e}")),
        schema_version: "1".into(),
    }
}

/// `get_row(upsert(b)) == normalize(b)`: counts absent, edges rebuilt (sorted, outgoing only).
pub fn round_trip_is_normalize(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let mut b = row("b1", &["phase-x", "r1"], T0);
    b.dependency_count = Some(2);
    b.dependent_count = Some(0);
    b.comment_count = Some(0);
    b.metadata.insert("n".into(), serde_json::json!(5));
    b.dependencies = vec![
        edge("b1", "z9", DepType::Related),
        edge("b1", "a1", DepType::Blocks),
    ];
    ok(
        s.upsert_rows(Some(&x()), std::slice::from_ref(&b), false),
        "upsert",
    );
    let (got, fresh) = get(&s, "b1").unwrap_or_default_pair();
    assert_eq!(got, normalize(&b));
    assert_eq!(got.dependency_count, None);
    assert_eq!(got.dependencies[0].depends_on_id.as_str(), "a1");
    assert_eq!(fresh, Freshness::Fresh);
}

/// Upsert replaces a bead's outgoing edges; the edges index follows.
pub fn upsert_replaces_outgoing_edges(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let mut b = row("b1", &["phase-x"], T0);
    b.dependencies = vec![
        edge("b1", "a1", DepType::Blocks),
        edge("b1", "a2", DepType::Blocks),
    ];
    ok(s.upsert_rows(None, &[b.clone()], false), "upsert 1");
    b.dependencies = vec![
        edge("b1", "a2", DepType::Blocks),
        edge("b1", "a3", DepType::ParentChild),
    ];
    b.updated_at = T1.into();
    ok(s.upsert_rows(None, &[b.clone()], false), "upsert 2");
    assert_eq!(
        get(&s, "b1").unwrap_or_default_pair().0.dependencies,
        normalize(&b).dependencies
    );
    assert!(ok(s.dependents_of(&id("a1")), "dependents a1").is_empty());
    assert_eq!(
        ok(s.dependents_of(&id("a3")), "dependents a3"),
        vec![edge("b1", "a3", DepType::ParentChild)]
    );
}

/// Any successful store clears `stale`.
pub fn a_store_clears_stale(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let b = row("b1", &["phase-x"], T0);
    ok(
        s.upsert_rows(Some(&x()), std::slice::from_ref(&b), false),
        "upsert",
    );
    for force in [false, true] {
        let seq = ok(
            s.insert_entry(entry(&format!("k{force}"), "b1", Some(x())), &ts(T0)),
            "insert",
        );
        ok(
            s.apply_with_readback(seq, None, Some("read-back failed"), &ts(T1)),
            "apply none",
        );
        assert_eq!(get(&s, "b1").unwrap_or_default_pair().1, Freshness::Stale);
        assert_eq!(ok(s.stale_rows(), "stale_rows"), vec![id("b1")]);
        ok(
            s.upsert_rows(Some(&x()), std::slice::from_ref(&b), force),
            "upsert again",
        );
        assert_eq!(
            get(&s, "b1").unwrap_or_default_pair().1,
            Freshness::Fresh,
            "force={force}"
        );
        assert!(ok(s.stale_rows(), "stale_rows").is_empty());
    }
    let seq = ok(
        s.insert_entry(entry("k3", "b1", Some(x())), &ts(T0)),
        "insert",
    );
    ok(
        s.apply_with_readback(seq, None, Some("failed"), &ts(T1)),
        "apply none",
    );
    ok(
        s.apply_with_readback(seq, Some(&b), None, &ts(T2)),
        "apply row",
    );
    assert_eq!(get(&s, "b1").unwrap_or_default_pair().1, Freshness::Fresh);
}

/// Scoped upserts set membership from labels; unscoped upserts never change it.
pub fn membership_follows_labels_on_scoped_upserts_only(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(
        s.upsert_rows(Some(&x()), &[row("b1", &["phase-x"], T0)], false),
        "scoped",
    );
    assert_eq!(ids_in(&s, &x()), ["b1"]);
    ok(
        s.upsert_rows(None, &[row("b1", &[], T1)], false),
        "unscoped without label",
    );
    assert_eq!(ids_in(&s, &x()), ["b1"], "None never changes membership");
    ok(
        s.upsert_rows(Some(&x()), &[row("b1", &["other"], T2)], false),
        "scoped without label",
    );
    assert!(
        ids_in(&s, &x()).is_empty(),
        "scoped upsert without the label removes membership"
    );
    ok(
        s.upsert_rows(None, &[row("o1", &["phase-x"], T0)], false),
        "new out-of-scope row",
    );
    assert!(
        ids_in(&s, &x()).is_empty(),
        "a new row stored with None has no membership"
    );
    assert!(get(&s, "o1").is_some());
    ok(
        s.upsert_rows(Some(&y()), &[row("b2", &["phase-x", "phase-y"], T0)], false),
        "y",
    );
    ok(
        s.upsert_rows(Some(&x()), &[row("b2", &["phase-x", "phase-y"], T0)], false),
        "x",
    );
    assert_eq!(ids_in(&s, &x()), ["b2"]);
    assert_eq!(ids_in(&s, &y()), ["b2"]);
}

/// Unforced upserts never replace a newer row; forced ones do.
pub fn newer_rows_are_kept_unless_forced(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let mut newer = row("b1", &["phase-x"], T2);
    newer.status = Some("blocked".into());
    ok(s.upsert_rows(Some(&x()), &[newer], false), "newer");
    let older = row("b1", &["phase-x"], T1);
    let r = ok(
        s.upsert_rows(Some(&x()), std::slice::from_ref(&older), false),
        "older",
    );
    assert_eq!(r.skipped_newer, vec![id("b1")]);
    assert!(r.stored.is_empty());
    assert_eq!(
        get(&s, "b1").unwrap_or_default_pair().0.status.as_deref(),
        Some("blocked")
    );
    let r = ok(s.upsert_rows(Some(&x()), &[older], true), "forced");
    assert_eq!(r.stored, vec![id("b1")]);
    assert_eq!(
        get(&s, "b1").unwrap_or_default_pair().0.status.as_deref(),
        Some("open")
    );
}

/// The newer-row guard compares `updated_at` by instant, not text.
pub fn newer_row_guard_compares_instants(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    // Stored 10:00:00Z written with a +02:00 offset; as text it sorts after 10:00:01Z.
    let mut stored = row("b1", &["phase-x"], "2026-10-23T12:00:00+02:00");
    stored.status = Some("blocked".into());
    ok(s.upsert_rows(Some(&x()), &[stored], false), "stored");
    let later = row("b1", &["phase-x"], "2026-10-23T10:00:01Z");
    let r = ok(s.upsert_rows(Some(&x()), &[later], false), "later instant");
    assert_eq!(
        r.stored,
        vec![id("b1")],
        "a later instant is stored even when its text sorts first"
    );
    // Stored 10:00:01.9Z; incoming 09:00:01-01:00 is the same second: not older, stored.
    let mut frac = row("b2", &["phase-x"], "2026-10-23T10:00:01.9Z");
    frac.status = Some("blocked".into());
    ok(s.upsert_rows(Some(&x()), &[frac], false), "fractional");
    let same = row("b2", &["phase-x"], "2026-10-23T09:00:01-01:00");
    let r = ok(s.upsert_rows(Some(&x()), &[same], false), "same second");
    assert_eq!(r.stored, vec![id("b2")]);
    let earlier = row("b2", &["phase-x"], "2026-10-23T11:00:00+01:01");
    let r = ok(
        s.upsert_rows(Some(&x()), &[earlier], false),
        "earlier instant",
    );
    assert_eq!(
        r.skipped_newer,
        vec![id("b2")],
        "09:59:00Z is older than the stored 10:00:01Z"
    );
    // A stored updated_at that does not parse is never newer: the incoming row is stored.
    let mut odd = row("b3", &["phase-x"], T2);
    odd.updated_at = "not-a-time".into();
    ok(
        s.upsert_rows(Some(&x()), &[odd], false),
        "unparseable stored",
    );
    let r = ok(
        s.upsert_rows(Some(&x()), &[row("b3", &["phase-x"], T0)], false),
        "after unparseable",
    );
    assert_eq!(
        r.stored,
        vec![id("b3")],
        "unparseable is not newer; no text order"
    );
    // An incoming updated_at that does not parse: the stored row is not newer, so it is stored.
    ok(
        s.upsert_rows(Some(&x()), &[row("b4", &["phase-x"], T2)], false),
        "valid stored",
    );
    let mut odd_in = row("b4", &["phase-x"], T0);
    odd_in.updated_at = "not-a-time".into();
    let r = ok(
        s.upsert_rows(Some(&x()), &[odd_in], false),
        "unparseable incoming",
    );
    assert_eq!(
        r.stored,
        vec![id("b4")],
        "an unparseable incoming row is stored"
    );
}

/// `status_changed` lists existing rows whose status changed, forced or not; new rows excluded.
pub fn status_changes_are_reported(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let r = ok(
        s.upsert_rows(
            Some(&x()),
            &[row("b1", &["phase-x"], T0), row("b2", &["phase-x"], T0)],
            false,
        ),
        "new",
    );
    assert!(
        r.status_changed.is_empty(),
        "new rows are not status changes"
    );
    let mut closed = row("b1", &["phase-x"], T1);
    closed.status = Some("closed".into());
    let r = ok(
        s.upsert_rows(Some(&x()), &[closed, row("b2", &["phase-x"], T1)], false),
        "unforced",
    );
    assert_eq!(r.status_changed, vec![id("b1")]);
    let r = ok(
        s.upsert_rows(None, &[row("b1", &["phase-x"], T0)], true),
        "forced reopen",
    );
    assert_eq!(r.status_changed, vec![id("b1")]);
}

/// `apply_with_readback` marks applied (with the error, or storing the row) and sets `applied_at`.
pub fn apply_with_readback_marks_applied(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(
        s.upsert_rows(Some(&x()), &[row("b1", &["phase-x"], T0)], false),
        "upsert",
    );
    let a = ok(
        s.insert_entry(entry("a", "b1", Some(x())), &ts(T0)),
        "insert a",
    );
    let b = ok(
        s.insert_entry(entry("b", "b1", Some(x())), &ts(T0)),
        "insert b",
    );
    ok(
        s.apply_with_readback(a, None, Some("read-back failed"), &ts(T1)),
        "apply a",
    );
    let ea = ok(s.entry_by_seq(a), "entry a").unwrap_or_else(missing);
    assert_eq!(ea.state, JournalState::Applied);
    assert_eq!(ea.last_error.as_deref(), Some("read-back failed"));
    assert_eq!(ea.applied_at.as_ref().map(Timestamp::as_str), Some(T1));
    assert_eq!(get(&s, "b1").unwrap_or_default_pair().1, Freshness::Stale);
    let mut fresh = row("b1", &["phase-x"], T2);
    fresh.status = Some("closed".into());
    ok(
        s.apply_with_readback(b, Some(&fresh), None, &ts(T2)),
        "apply b",
    );
    let eb = ok(s.entry_by_seq(b), "entry b").unwrap_or_else(missing);
    assert_eq!(
        (
            eb.state,
            eb.last_error,
            eb.applied_at.as_ref().map(Timestamp::as_str)
        ),
        (JournalState::Applied, None, Some(T2))
    );
    assert_eq!(
        get(&s, "b1").unwrap_or_default_pair(),
        (normalize(&fresh), Freshness::Fresh)
    );
    assert_eq!(ids_in(&s, &x()), ["b1"], "read-back keeps membership");
    assert!(ok(s.pending_entries(), "pending").is_empty());
}

/// First fingerprint records and keeps rows; same sha is unchanged; a new sha
/// rebuilds (rows, edges, memberships, sync state, ready bases gone) and keeps the journal.
pub fn fingerprint_rebuild_rule(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(s.ensure_scope(&x()), "ensure");
    let mut b = row("b1", &["phase-x"], T0);
    b.dependencies = vec![edge("b1", "a1", DepType::Blocks)];
    ok(s.upsert_rows(Some(&x()), &[b], false), "upsert");
    let seq = ok(
        s.insert_entry(entry("k1", "b1", Some(x())), &ts(T0)),
        "insert",
    );
    ok(
        s.replace_ready_base(&x(), &[], &[id("b1")], &ts(T0)),
        "ready base",
    );
    let mut st = SyncState::new(x());
    st.last_ok_at = Some(ts(T0));
    ok(s.record_sync(&st), "record sync");

    assert_eq!(
        ok(s.record_fingerprint(&fingerprint('a')), "first"),
        FingerprintOutcome::Recorded
    );
    assert!(get(&s, "b1").is_some(), "the first check drops nothing");
    assert_eq!(
        ok(s.record_fingerprint(&fingerprint('a')), "same"),
        FingerprintOutcome::Unchanged
    );
    assert!(get(&s, "b1").is_some());
    assert_eq!(
        ok(s.record_fingerprint(&fingerprint('b')), "changed"),
        FingerprintOutcome::Rebuilt
    );
    assert!(get(&s, "b1").is_none(), "beads deleted");
    assert!(
        ok(s.dependents_of(&id("a1")), "dependents").is_empty(),
        "edges deleted"
    );
    assert!(ids_in(&s, &x()).is_empty(), "memberships deleted");
    assert_eq!(
        ok(s.sync_state(&x()), "sync"),
        SyncState::new(x()),
        "sync_state deleted"
    );
    assert!(
        ok(s.ready_base(&x()), "ready").is_none(),
        "ready bases deleted"
    );
    assert_eq!(ok(s.pending_entries(), "pending").len(), 1, "journal kept");
    assert!(ok(s.entry_by_seq(seq), "entry").is_some());
    assert_eq!(ok(s.scopes(), "scopes"), vec![x()], "scope registry kept");
    assert_eq!(
        ok(s.record_fingerprint(&fingerprint('b')), "after"),
        FingerprintOutcome::Unchanged
    );
}

/// `ensure_scope` persists: configured and on-demand scopes survive a restart.
pub fn scopes_persist_across_restarts(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(s.ensure_scope(&x()), "configured");
    ok(s.ensure_scope(&y()), "on demand");
    ok(s.ensure_scope(&x()), "idempotent");
    let s = fx.reopen(s);
    let mut got = ok(s.scopes(), "scopes");
    got.sort();
    assert_eq!(got, vec![x(), y()]);
}

/// Ready base: `None` until stored; replace is whole and clears stale; stale marks; prune deletes it.
pub fn ready_base_rules(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(s.ensure_scope(&x()), "ensure");
    assert!(ok(s.ready_base(&x()), "none").is_none());
    ok(s.mark_ready_base_stale(&x()), "stale on missing is a no-op");
    assert!(ok(s.ready_base(&x()), "still none").is_none());
    let blocked = [BlockedRow {
        id: id("b2"),
        blocked_by: vec![id("b1")],
    }];
    ok(
        s.replace_ready_base(&x(), &blocked, &[id("b1"), id("b3")], &ts(T0)),
        "replace",
    );
    let base = ok(s.ready_base(&x()), "base").unwrap_or_else(missing);
    assert_eq!(
        (
            base.blocked.as_slice(),
            base.ready.len(),
            base.fetched_at.as_str(),
            base.stale
        ),
        (&blocked[..], 2, T0, false)
    );
    ok(s.mark_ready_base_stale(&x()), "mark");
    assert!(ok(s.ready_base(&x()), "stale").is_some_and(|b| b.stale));
    ok(
        s.replace_ready_base(&x(), &[], &[id("b2")], &ts(T1)),
        "replace again",
    );
    let base = ok(s.ready_base(&x()), "base").unwrap_or_else(missing);
    assert_eq!(
        (
            base.blocked.len(),
            base.ready,
            base.fetched_at.as_str(),
            base.stale
        ),
        (0, vec![id("b2")], T1, false)
    );
    assert!(ok(s.ready_base(&y()), "other scope").is_none());
    ok(s.prune_scope(&x()), "prune");
    assert!(ok(s.ready_base(&x()), "pruned").is_none());
}

/// `applied_since(at)` returns entries applied at or after `at`, in seq order.
pub fn applied_since_is_inclusive_and_ordered(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let seqs: Vec<Seq> = (0..4)
        .map(|i| {
            ok(
                s.insert_entry(entry(&format!("k{i}"), "b1", None), &ts(T0)),
                "insert",
            )
        })
        .collect();
    ok(
        s.apply_with_readback(seqs[2], None, None, &ts(T1)),
        "apply 3rd at T1",
    );
    ok(
        s.apply_with_readback(seqs[0], None, None, &ts(T0)),
        "apply 1st at T0",
    );
    ok(
        s.apply_with_readback(seqs[1], None, None, &ts(T2)),
        "apply 2nd at T2",
    );
    let got: Vec<Seq> = ok(s.applied_since(&ts(T1)), "since")
        .into_iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(got, vec![seqs[1], seqs[2]]);
    assert_eq!(ok(s.applied_since(&ts(T0)), "all").len(), 3);
}

/// Journal basics: seqs increase, lookups by key and seq in any state, ordered pending reads,
/// attempts, escalation, resolve, announcement and open work.
#[expect(
    clippy::many_single_char_names,
    reason = "a, b, c name the three entries"
)]
pub fn journal_lifecycle(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let a = ok(s.insert_entry(entry("a", "b2", Some(x())), &ts(T0)), "a");
    let b = ok(s.insert_entry(entry("b", "b1", Some(x())), &ts(T0)), "b");
    let c = ok(s.insert_entry(entry("c", "b2", None), &ts(T1)), "c");
    assert!(a < b && b < c);
    let dup = s.insert_entry(entry("a", "b9", None), &ts(T1));
    assert!(
        matches!(dup, Err(CacheError::IdemConflict { seq, .. }) if seq == a),
        "{dup:?}"
    );
    let pend: Vec<Seq> = ok(s.pending_entries(), "pending")
        .into_iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(pend, vec![a, b, c]);
    let mine: Vec<Seq> = ok(s.pending_for(&id("b2")), "pending_for")
        .into_iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(mine, vec![a, c]);
    let e = ok(s.entry_by_key(&key("c")), "by key").unwrap_or_else(missing);
    assert_eq!(
        (e.seq, e.state, e.attempts, e.created_at.as_str()),
        (c, JournalState::Pending, 0, T1)
    );
    assert_eq!(e.op, Op::Close { reason: None });
    assert_eq!(
        ok(
            s.record_attempt(a, "BD_CACHE.BD_WRITE_FAILED: x", &ts(T1)),
            "attempt"
        ),
        1
    );
    assert_eq!(
        ok(
            s.record_attempt(a, "BD_CACHE.BD_WRITE_FAILED: y", &ts(T1)),
            "attempt"
        ),
        2
    );
    let r = s.resolve(a, "n/a", &sc_beads::testing::actor("lead"), &ts(T1));
    assert!(matches!(r, Err(CacheError::NotEscalated { .. })), "{r:?}");
    assert!(matches!(
        s.resolve(
            Seq::new(999),
            "n/a",
            &sc_beads::testing::actor("lead"),
            &ts(T1)
        ),
        Err(CacheError::NotEscalated { .. })
    ));
    ok(
        s.escalate(a, "precondition no longer holds", &ts(T1)),
        "escalate",
    );
    let ea = ok(s.entry_by_seq(a), "a").unwrap_or_else(missing);
    assert_eq!(
        (ea.state, ea.attempts),
        (JournalState::Escalated, 2),
        "escalate keeps attempts"
    );
    assert_eq!(ok(s.open_work(), "open").pending, 2);
    assert_eq!(ok(s.open_work(), "open").escalated_unannounced, 1);
    let un: Vec<Seq> = ok(s.unannounced_escalations(), "unannounced")
        .into_iter()
        .map(|e| e.seq)
        .collect();
    assert_eq!(un, vec![a]);
    ok(s.mark_announced(a, &ts(T2)), "announce");
    assert!(ok(s.unannounced_escalations(), "unannounced").is_empty());
    assert_eq!(ok(s.open_work(), "open").escalated_unannounced, 0);
    assert_eq!(
        ok(s.status(), "status").escalated.len(),
        1,
        "listed until resolved"
    );
    ok(
        s.resolve(a, "set by hand", &sc_beads::testing::actor("lead"), &ts(T2)),
        "resolve",
    );
    let ea = ok(s.entry_by_key(&key("a")), "a").unwrap_or_else(missing);
    assert_eq!(ea.state, JournalState::Resolved);
    assert_eq!(
        ea.last_error.as_deref(),
        Some("precondition no longer holds | resolved by lead: set by hand")
    );
    assert_eq!(ea.announced_at.as_ref().map(Timestamp::as_str), Some(T2));
    assert!(ok(s.status(), "status").escalated.is_empty());
    assert!(matches!(
        s.escalate(Seq::new(999), "x", &ts(T2)),
        Err(CacheError::NotFound { .. })
    ));
}

/// Resolving an entry with no error keeps the note (`test_resolve_keeps_the_note_when_the_entry_had_no_error`, store half).
pub fn resolve_note_without_prior_error(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let a = ok(s.insert_entry(entry("a", "b1", None), &ts(T0)), "a");
    ok(s.escalate(a, "", &ts(T0)), "escalate");
    ok(
        s.resolve(a, "set by hand", &sc_beads::testing::actor("lead"), &ts(T1)),
        "resolve",
    );
    let e = ok(s.entry_by_seq(a), "a").unwrap_or_else(missing);
    assert_eq!(
        e.last_error.as_deref(),
        Some(" | resolved by lead: set by hand")
    );
}

/// Sync state, sync alerts and the outage round-trip.
pub fn sync_state_alerts_and_outage_round_trip(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    assert_eq!(ok(s.sync_state(&x()), "empty"), SyncState::new(x()));
    let st = SyncState {
        scope: x(),
        last_started_at: Some(ts(T1)),
        last_ok_at: Some(ts(T0)),
        last_error: Some("BD_CACHE.BD_LIST_FAILED: boom".into()),
    };
    ok(s.record_sync(&st), "record");
    assert_eq!(ok(s.sync_state(&x()), "read"), st);
    assert_eq!(ok(s.sync_state(&y()), "other"), SyncState::new(y()));
    assert_eq!(ok(s.sync_alert(&x()), "alert"), None);
    ok(
        s.set_sync_alert(&x(), Some("BD_CACHE.SYNC_FAILED: boom")),
        "set",
    );
    assert_eq!(
        ok(s.sync_alert(&x()), "alert").as_deref(),
        Some("BD_CACHE.SYNC_FAILED: boom")
    );
    assert_eq!(ok(s.sync_alert(&y()), "other alert"), None);
    ok(s.set_sync_alert(&x(), None), "clear");
    assert_eq!(ok(s.sync_alert(&x()), "cleared"), None);
    assert_eq!(ok(s.outage(), "outage"), None);
    let o = Outage {
        since: ts(T0),
        last_error: "BD_CACHE.BD_UNAVAILABLE: refused".into(),
    };
    ok(s.set_outage(Some(&o)), "set outage");
    assert_eq!(ok(s.outage(), "outage"), Some(o.clone()));
    assert_eq!(ok(s.status(), "status").outage, Some(o));
    ok(s.set_outage(None), "clear outage");
    assert_eq!(ok(s.outage(), "cleared"), None);
}

/// Prune: refused with open entries; keeps beads another scope holds; sweeps out-of-scope rows
/// with no open entry; deletes the scope's journal and sync state.
pub fn prune_scope_rules(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(s.ensure_scope(&x()), "x");
    ok(
        s.upsert_rows(
            Some(&x()),
            &[
                row("only-x", &["phase-x"], T0),
                row("both", &["phase-x", "phase-y"], T0),
            ],
            false,
        ),
        "x rows",
    );
    ok(
        s.upsert_rows(
            Some(&y()),
            &[row("both", &["phase-x", "phase-y"], T0)],
            false,
        ),
        "y rows",
    );
    ok(
        s.upsert_rows(
            None,
            &[row("loose", &[], T0), row("loose-open", &[], T0)],
            false,
        ),
        "loose rows",
    );
    let p = ok(
        s.insert_entry(entry("p", "only-x", Some(x())), &ts(T0)),
        "pending in x",
    );
    let r = s.prune_scope(&x());
    assert!(
        matches!(r, Err(CacheError::PruneBlocked { .. })),
        "pending blocks: {r:?}"
    );
    ok(s.escalate(p, "x", &ts(T0)), "escalate");
    assert!(
        matches!(s.prune_scope(&x()), Err(CacheError::PruneBlocked { .. })),
        "escalated blocks"
    );
    ok(
        s.resolve(p, "done", &sc_beads::testing::actor("lead"), &ts(T1)),
        "resolve",
    );
    ok(
        s.insert_entry(entry("lo", "loose-open", None), &ts(T0)),
        "open entry on a loose row",
    );
    let mut st = SyncState::new(x());
    st.last_ok_at = Some(ts(T0));
    ok(s.record_sync(&st), "sync state");
    let report = ok(s.prune_scope(&x()), "prune");
    assert_eq!(
        (
            report.scope,
            report.beads_deleted,
            report.out_of_scope_swept
        ),
        (x(), 1, 1)
    );
    assert!(get(&s, "only-x").is_none());
    assert!(
        get(&s, "both").is_some() && ids_in(&s, &y()) == ["both"],
        "kept for y"
    );
    assert!(get(&s, "loose").is_none(), "swept");
    assert!(
        get(&s, "loose-open").is_some(),
        "an open entry keeps a loose row"
    );
    assert!(
        ok(s.entry_by_seq(p), "journal").is_none(),
        "the scope's journal is deleted"
    );
    assert_eq!(ok(s.sync_state(&x()), "sync"), SyncState::new(x()));
    assert!(
        ok(s.scopes(), "scopes").contains(&x()),
        "the scope stays registered"
    );
}

/// `status().stale` lists a row whose read-back failed, and drops it once
/// the row is re-read (stored again) (a-1 amendment 1, REQ-BDC-0603).
pub fn status_lists_stale_rows_until_reread(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    let b = row("b1", &["phase-x"], T0);
    ok(
        s.upsert_rows(Some(&x()), &[b.clone(), row("b2", &["phase-x"], T0)], false),
        "rows",
    );
    assert!(ok(s.status(), "status").stale.is_empty());
    let seq = ok(
        s.insert_entry(entry("k1", "b1", Some(x())), &ts(T0)),
        "insert",
    );
    ok(
        s.apply_with_readback(seq, None, Some("read-back failed"), &ts(T1)),
        "failed read-back",
    );
    let st = ok(s.status(), "status");
    assert_eq!(st.stale, vec![id("b1")], "the failed read-back is listed");
    assert_eq!(
        st.stale,
        ok(s.stale_rows(), "stale_rows"),
        "same set as stale_rows"
    );
    ok(s.upsert_rows(Some(&x()), &[b], false), "re-read");
    assert!(
        ok(s.status(), "status").stale.is_empty(),
        "a re-read clears it"
    );
}

/// Status: journal counts per `phase:state`, per-scope bead counts.
pub fn status_counts(fx: &mut impl StoreFixture) {
    let mut s = fx.open();
    ok(s.ensure_scope(&x()), "x");
    ok(
        s.upsert_rows(
            Some(&x()),
            &[row("b1", &["phase-x"], T0), row("b2", &["phase-x"], T0)],
            false,
        ),
        "rows",
    );
    ok(s.insert_entry(entry("a", "b1", Some(x())), &ts(T0)), "a");
    let b = ok(s.insert_entry(entry("b", "b2", Some(x())), &ts(T0)), "b");
    ok(s.insert_entry(entry("c", "b9", None), &ts(T0)), "c");
    ok(s.escalate(b, "why", &ts(T0)), "escalate");
    let st = ok(s.status(), "status");
    assert_eq!(st.journal.get("x:pending"), Some(&1));
    assert_eq!(st.journal.get("x:escalated"), Some(&1));
    assert_eq!(st.journal.get("None:pending"), Some(&1));
    let xs = st
        .scopes
        .iter()
        .find(|p| p.sync.scope == x())
        .map(|p| p.beads);
    assert_eq!(xs, Some(2));
    assert_eq!(
        st.escalated.iter().map(|e| e.seq).collect::<Vec<_>>(),
        vec![b]
    );
}

/// The drain lock admits one holder; dropping the guard releases it; handles contend.
pub fn drain_lock_is_exclusive(fx: &mut impl StoreFixture) {
    let (one, two) = fx.locks();
    let guard = ok(one.try_acquire(), "acquire");
    assert!(guard.is_some());
    assert!(
        ok(two.try_acquire(), "contend").is_none(),
        "second handle must not acquire"
    );
    assert!(
        ok(one.try_acquire(), "same handle").is_none(),
        "the lock is not reentrant"
    );
    drop(guard);
    let again = ok(two.try_acquire(), "after release");
    assert!(again.is_some(), "dropping the guard releases the lock");
}

trait OrDefaultPair {
    fn unwrap_or_default_pair(self) -> (Bead, Freshness);
}

impl OrDefaultPair for Option<(Bead, Freshness)> {
    fn unwrap_or_default_pair(self) -> (Bead, Freshness) {
        assert!(self.is_some(), "expected a stored row");
        match self {
            Some(pair) => pair,
            None => missing(),
        }
    }
}

/// Instantiates the store contract suite as `#[test]` functions.
///
/// The argument builds a fresh
/// [`StoreFixture`](crate::testing::contract::StoreFixture); it is
/// evaluated once per test.
///
/// ```ignore
/// bd_cache_model::store_contract_tests!(bd_cache_model::testing::contract::FakeStoreFixture);
/// ```
#[macro_export]
macro_rules! store_contract_tests {
    ($fixture:expr) => {
        $crate::store_contract_tests!(@rules $fixture;
            round_trip_is_normalize,
            upsert_replaces_outgoing_edges,
            a_store_clears_stale,
            membership_follows_labels_on_scoped_upserts_only,
            newer_rows_are_kept_unless_forced,
            newer_row_guard_compares_instants,
            status_changes_are_reported,
            apply_with_readback_marks_applied,
            fingerprint_rebuild_rule,
            scopes_persist_across_restarts,
            ready_base_rules,
            applied_since_is_inclusive_and_ordered,
            journal_lifecycle,
            resolve_note_without_prior_error,
            sync_state_alerts_and_outage_round_trip,
            prune_scope_rules,
            status_counts,
            status_lists_stale_rows_until_reread,
            drain_lock_is_exclusive,
        );
    };
    (@rules $fixture:expr; $($rule:ident),* $(,)?) => {
        $(
            #[test]
            fn $rule() {
                let mut fixture = $fixture;
                $crate::testing::contract::$rule(&mut fixture);
            }
        )*
    };
}
