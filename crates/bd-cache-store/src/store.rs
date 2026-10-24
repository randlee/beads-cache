//! [`SqliteStore`]: the [`CacheStore`] over one SQLite file shared with `bd_cache.py`.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bd_cache_model::{
    BaseEntry, CacheError, CacheStore, FingerprintOutcome, Freshness, IdemKey, JournalEntry,
    JournalState, NewEntry, OpenWork, Outage, PruneReport, ReadyBase, RepoConfig, ScopeId,
    ScopeStatus, Seq, StoreStatus, SyncState, UpsertReport, phase_of, precondition_json,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp};

use crate::codec::{
    JOURNAL_COLUMNS, JournalRow, bead_id, decode_edge, decode_ready, decode_row, encode_ready,
    encode_row, index_blocked, is_newer, opt_timestamp, scope, timestamp,
};
use crate::error::{Step, corrupt, failed};
use crate::location::{CacheLocation, DatabaseIdentity};
use crate::lock::FlockDrainLock;
use crate::registry::Registration;
use crate::schema::{
    APPLIED_SINCE_SQL, APPLIED_SINCE_UNHINTED_SQL, BACKFILL_APPLIED_EPOCH,
    BACKFILL_APPLIED_EPOCH_UNHINTED, EXTENSION_COLUMNS, EXTENSION_INDEXES, EXTENSION_TABLES,
    HAS_DATEABLE_SQL, HAS_DATEABLE_UNHINTED_SQL, HAS_ENTRIES_SINCE_SQL,
    HAS_ENTRIES_SINCE_UNHINTED_SQL, IN_SCOPE, RECONCILE_MEMBERSHIP, SCHEMA_VERSION, UNDATED_SEQS,
    V3_SCHEMA,
};

/// SQLite's busy timeout on the shared file (REQ-BDC-0103).
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

const META_PROJECT_ID: &str = "project_id";
const META_DOLT_DATABASE: &str = "dolt_database";
const META_SHA: &str = "bd_schema_sha256";
const META_VERSION: &str = "bd_version";
const META_SCHEMA_VERSION: &str = "bd_schema_version";
const META_OUTAGE_SINCE: &str = "outage_since";
const META_OUTAGE_ERROR: &str = "outage_error";
/// Written by `bd_cache_cron.py` when it announces an outage; cleared with the outage.
const META_OUTAGE_ANNOUNCED: &str = "outage_announced_at";

/// `synced_at` (v3, NOT NULL, never read by either implementation): the
/// SQLite clock at the store, as `bd_cache.py` writes its own clock
/// (ADR-STORE-0002). No value the port returns depends on it.
const SYNCED_AT: &str = "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')";

fn alert_key(s: &ScopeId) -> String {
    format!("sync_alert:{}", s.name())
}

/// Maps SQLite errors to the store's failure, naming the step.
trait At<T> {
    fn at(self, step: Step<'_>) -> Result<T, CacheError>;
}

impl<T> At<T> for rusqlite::Result<T> {
    fn at(self, step: Step<'_>) -> Result<T, CacheError> {
        self.map_err(|e| failed(step, e))
    }
}

fn step<'a>(path: &'a Path, what: &'a str) -> Step<'a> {
    Step { what, path }
}

fn not_found(seq: Seq) -> CacheError {
    CacheError::NotFound {
        detail: format!("journal entry {seq} is absent"),
    }
}

fn count(n: i64) -> u64 {
    // COUNT(*) is never negative.
    u64::try_from(n).unwrap_or_default()
}

/// Is `e` SQLite's `no such index` (an `INDEXED BY` naming an index another
/// writer dropped)? A generic `SQLITE_ERROR` whose message names the case:
/// SQLite has no more specific code for it.
fn is_missing_index(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(f, Some(msg))
            if f.extended_code == rusqlite::ffi::SQLITE_ERROR && msg.starts_with("no such index")
    )
}

/// How many seqs the open's warning about undatable entries names.
const UNDATED_SEQS_LOGGED: usize = 20;

/// The BD-CACHE v3 SQLite store (ADR-STORE-0001), shared with `bd_cache.py`.
///
/// One `SqliteStore` is the process's one connection to its cache file
/// (NFR-BDC-0007): opening the same file again in the same process, while a
/// store on it is alive, is refused. The store is `Send` but not `Sync`, as
/// the `Cache` that owns it (ADR-CORE-0001). Every mutating method runs in one
/// `BEGIN IMMEDIATE` transaction, so a concurrent writer in another process
/// (a hook, the Python implementation) waits up to [`BUSY_TIMEOUT`]. The
/// journal reads `has_entries_since` and `applied_since` may also write: they
/// date the entries another writer applied without an epoch, never waiting
/// for the write lock (ADR-STORE-0004).
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use bd_cache_model::{CacheStore, RepoConfig, ScopeId};
/// use bd_cache_store::SqliteStore;
///
/// let root = std::env::temp_dir().join(format!("bdc-doc-store-{}", std::process::id()));
/// std::fs::create_dir_all(root.join(".beads"))?;
/// std::fs::write(
///     root.join(".beads").join("metadata.json"),
///     r#"{"dolt_database": "atm-dev", "project_id": "p-1"}"#,
/// )?;
/// let mut store = SqliteStore::open(&root, &RepoConfig::default())?;
/// let x = ScopeId::phase("x")?;
/// let mut b = sc_beads::testing::bead("b1");
/// b.labels = vec!["phase-x".into()];
/// store.upsert_rows(Some(&x), &[b], false)?;
/// assert_eq!(store.rows_in_scope(&x)?.len(), 1);
/// # drop(store);
/// # std::fs::remove_dir_all(&root)?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct SqliteStore {
    // Field order matters: the connection closes before the claim is released.
    conn: Connection,
    path: PathBuf,
    /// Set, with one warning, when a hinted journal index is found missing:
    /// from then on the journal reads ask their unhinted form (ADR-STORE-0004).
    hints_missing: Cell<bool>,
    _claim: Registration,
}

/// The stored columns an upsert decides on: `updated_at`, `status`,
/// `membership_known`, `phase`.
type Existing = (Option<String>, Option<String>, Option<i64>, Option<String>);

/// What [`SqliteStore::store_row`] did with one row.
enum Stored {
    /// Stored; `true` when an existing row's status text changed.
    Yes { status_changed: bool },
    /// Skipped: the stored row is a later instant.
    SkippedNewer,
}

impl SqliteStore {
    /// Opens (creating when absent) the cache file of `repo_root`.
    ///
    /// The file is located from `config` and `.beads/metadata.json`
    /// ([`CacheLocation::resolve`], `$BEADS_DIR` aware); no `bd` process starts.
    ///
    /// # Errors
    ///
    /// As [`CacheLocation::resolve`] and [`SqliteStore::open_at`].
    pub fn open(repo_root: &Path, config: &RepoConfig) -> Result<Self, CacheError> {
        let loc = CacheLocation::resolve(repo_root, config)?;
        Self::open_at(&loc.cache_path, &loc.identity)
    }

    /// Opens (creating when absent) the cache file at `path` for `identity`.
    ///
    /// In order (ADR-STORE-0002): sets a 5 s busy timeout; then, in one
    /// transaction, checks `user_version` (0 is initialized to 3, 3 is
    /// opened, anything else refused), creates the v3 tables and the additive
    /// extensions when absent, records `dolt_database` and `project_id` on
    /// first open and checks `project_id` on every later one, and takes over
    /// the membership of rows `bd_cache.py` wrote; then sets WAL mode. A
    /// refused file is left as it was, its journal mode included.
    /// `user_version` is never changed from 3.
    ///
    /// # Errors
    ///
    /// - `BD_CACHE.SCHEMA_VERSION`: the file has another nonzero `user_version` (REQ-BDC-0103).
    /// - `BD_CACHE.WRONG_DATABASE`: the file records another `project_id` (REQ-BDC-0104).
    /// - `BD_CACHE.STORE_FAILED`: the file cannot be created or opened, is not
    ///   SQLite, refuses WAL, or this process already has a connection to it
    ///   (NFR-BDC-0007; ADR-STORE-0003); the underlying error, when there is
    ///   one, is the source.
    pub fn open_at(path: &Path, identity: &DatabaseIdentity) -> Result<Self, CacheError> {
        let at = |what| step(path, what);
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
        if let Some(dir) = dir {
            std::fs::create_dir_all(dir)
                .map_err(|e| failed(at("create the cache directory"), e))?;
        }
        let conn = Connection::open(path).at(at("open"))?;
        // The file exists now: claim its canonical path (symlinks and `.`
        // segments resolved). On a refused claim the connection closes on
        // return, before it has read or written anything.
        let canonical = path
            .canonicalize()
            .map_err(|e| failed(at("resolve the cache file"), e))?;
        let claim = Registration::claim(&canonical).ok_or_else(|| {
            corrupt(
                at("open"),
                "this process already has a connection to the cache file (NFR-BDC-0007)",
            )
        })?;
        let mut conn = conn;
        conn.busy_timeout(BUSY_TIMEOUT)
            .at(at("set the busy timeout"))?;
        // Version and identity are checked (and the schema created) before
        // WAL is set, so a refused file is left exactly as it was, journal
        // mode included.
        initialize(&mut conn, path, identity)?;
        let mode: String = conn
            .pragma_update_and_check(None, "journal_mode", "WAL", |r| r.get(0))
            .at(at("set WAL mode"))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(corrupt(
                at("set WAL mode"),
                format_args!("SQLite kept journal_mode {mode}"),
            ));
        }
        Ok(Self {
            conn,
            path: path.to_path_buf(),
            hints_missing: Cell::new(false),
            _claim: claim,
        })
    }

    /// The cache file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The drain lock of this cache file (`<cache>.drain.lock`, REQ-BDC-0401).
    #[must_use]
    pub fn drain_lock(&self) -> FlockDrainLock {
        FlockDrainLock::for_cache_file(&self.path)
    }

    fn at<'a>(&'a self, what: &'a str) -> Step<'a> {
        step(&self.path, what)
    }

    /// Runs `run` with the `hinted` SQL (which names its indexes with
    /// `INDEXED BY`) unless a hinted index was found missing before; on
    /// `no such index` warns once, remembers it, and runs `unhinted`, as every
    /// later call does without trying the hints again (ADR-STORE-0004).
    fn hinted<T>(
        &self,
        hinted: &str,
        unhinted: &str,
        run: impl Fn(&str) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        if !self.hints_missing.get() {
            match run(hinted) {
                Err(e) if is_missing_index(&e) => {
                    self.hints_missing.set(true);
                    tracing::warn!(
                        path = %self.path.display(),
                        error = %e,
                        "a journal index is missing; asking without index hints until the next open"
                    );
                }
                other => return other,
            }
        }
        run(unhinted)
    }

    /// Dates the entries applied without an epoch since the last backfill
    /// (`bd_cache.py`'s, written while this store is open), so the journal
    /// reads stay index lookups (ADR-STORE-0004). Best effort and never
    /// waits: the `UPDATE` runs with no busy timeout, and when the write lock
    /// is held elsewhere, or the file is read-only, nothing is dated now and
    /// the reads answer the same from the undated entries, only slower.
    fn date_undated(&self) {
        let dateable = self.hinted(HAS_DATEABLE_SQL, HAS_DATEABLE_UNHINTED_SQL, |sql| {
            self.conn.prepare_cached(sql)?.query_row([], |r| r.get(0))
        });
        match dateable {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                tracing::debug!(error = %e, "could not look for undated journal entries");
                return;
            }
        }
        if let Err(e) = self.conn.busy_timeout(Duration::ZERO) {
            tracing::debug!(error = %e, "could not drop the busy timeout to date entries");
            return;
        }
        let dated = self.hinted(
            BACKFILL_APPLIED_EPOCH,
            BACKFILL_APPLIED_EPOCH_UNHINTED,
            |sql| self.conn.prepare_cached(sql)?.execute([]),
        );
        if let Err(e) = self.conn.busy_timeout(BUSY_TIMEOUT) {
            tracing::warn!(
                path = %self.path.display(),
                error = %e,
                "could not restore the busy timeout"
            );
        }
        match dated {
            Ok(n) => tracing::debug!(entries = n, "dated journal entries applied elsewhere"),
            Err(e) => tracing::debug!(error = %e, "could not date journal entries now"),
        }
    }

    /// Runs `f` in one `BEGIN IMMEDIATE` transaction, committed when `f` succeeds.
    fn write<T>(
        &mut self,
        what: &str,
        f: impl FnOnce(&Transaction<'_>, Step<'_>) -> Result<T, CacheError>,
    ) -> Result<T, CacheError> {
        let at = step(&self.path, what);
        let tx = begin(&mut self.conn, at)?;
        let out = f(&tx, at)?;
        tx.commit().at(at)?;
        Ok(out)
    }

    /// Stores one row inside `tx` (see the `CacheStore` rules).
    fn store_row(
        tx: &Transaction<'_>,
        at: Step<'_>,
        s: Option<&ScopeId>,
        row: &Bead,
        force: bool,
    ) -> Result<Stored, CacheError> {
        let id = row.id.as_str();
        let existing: Option<Existing> = tx
            .prepare_cached(
                "SELECT updated_at, status, membership_known, phase FROM beads WHERE id = ?1",
            )
            .at(at)?
            .query_row([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()
            .at(at)?;
        if let Some((updated_at, ..)) = &existing
            && !force
            && is_newer(updated_at.as_deref(), &row.updated_at)
        {
            return Ok(Stored::SkippedNewer);
        }
        let status_changed = existing
            .as_ref()
            .is_some_and(|(_, status, ..)| *status != row.status);
        match &existing {
            // A new row has no membership: drop rows left by a bead deleted elsewhere.
            None => {
                tx.prepare_cached("DELETE FROM scope_member WHERE bead = ?1")
                    .at(at)?
                    .execute([id])
                    .at(at)?;
            }
            // Written by bd_cache.py: its membership is its v3 phase (ADR-STORE-0002).
            Some((_, _, None, phase)) => {
                tx.prepare_cached("DELETE FROM scope_member WHERE bead = ?1")
                    .at(at)?
                    .execute([id])
                    .at(at)?;
                if let Some(phase) = phase {
                    tx.prepare_cached(
                        "INSERT OR IGNORE INTO scope_member(scope, bead) VALUES (?1, ?2)",
                    )
                    .at(at)?
                    .execute([phase.as_str(), id])
                    .at(at)?;
                }
            }
            Some(_) => {}
        }
        let (stored, row_json) = encode_row(at, row)?;
        let phase = phase_of(&stored);
        tx.prepare_cached(&format!(
            "INSERT OR REPLACE INTO beads(id, phase, status, updated_at, row_json, synced_at, \
             stale, membership_known) VALUES (?1, ?2, ?3, ?4, ?5, {SYNCED_AT}, 0, 1)"
        ))
        .at(at)?
        .execute(params![
            id,
            phase.as_ref().map(bd_cache_model::PhaseName::as_str),
            stored.status,
            stored.updated_at,
            row_json
        ])
        .at(at)?;
        tx.prepare_cached("DELETE FROM edges WHERE issue_id = ?1")
            .at(at)?
            .execute([id])
            .at(at)?;
        for e in &stored.dependencies {
            tx.prepare_cached("INSERT OR IGNORE INTO edges VALUES (?1, ?2, ?3)")
                .at(at)?
                .execute([
                    e.issue_id.as_str(),
                    e.depends_on_id.as_str(),
                    e.kind.as_str(),
                ])
                .at(at)?;
        }
        if let Some(s) = s {
            let sql = if s.holds(row) {
                "INSERT OR IGNORE INTO scope_member(scope, bead) VALUES (?1, ?2)"
            } else {
                "DELETE FROM scope_member WHERE scope = ?1 AND bead = ?2"
            };
            tx.prepare_cached(sql)
                .at(at)?
                .execute([s.name(), id])
                .at(at)?;
        }
        Ok(Stored::Yes { status_changed })
    }

    /// A row with its outgoing edges, from its `row_json` and `stale` columns.
    fn load_row(
        &self,
        at: Step<'_>,
        id: &str,
        row_json: &str,
        stale: i64,
    ) -> Result<(Bead, Freshness), CacheError> {
        let mut bead = decode_row(at, id, row_json)?;
        let mut stmt = self
            .conn
            .prepare_cached("SELECT issue_id, depends_on_id, type FROM edges WHERE issue_id = ?1")
            .at(at)?;
        let rows = stmt
            .query_map([id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .at(at)?;
        let mut edges = Vec::new();
        for r in rows {
            let (from, to, kind) = r.at(at)?;
            edges.push(decode_edge(at, &from, &to, &kind)?);
        }
        edges.sort();
        bead.dependencies = edges;
        let freshness = if stale == 0 {
            Freshness::Fresh
        } else {
            Freshness::Stale
        };
        Ok((bead, freshness))
    }

    /// Journal entries matching `filter` (SQL after `WHERE`), by seq.
    fn entries(
        &self,
        at: Step<'_>,
        filter: &str,
        args: impl rusqlite::Params,
    ) -> Result<Vec<JournalEntry>, CacheError> {
        let rows = self
            .conn
            .prepare_cached(&format!(
                "SELECT {JOURNAL_COLUMNS} FROM journal WHERE {filter} ORDER BY seq"
            ))
            .and_then(|mut stmt| Self::journal_rows(&mut stmt, args))
            .at(at)?;
        rows.into_iter().map(|r| r.decode(at)).collect()
    }

    /// The [`JOURNAL_COLUMNS`] rows a prepared journal query returns.
    fn journal_rows(
        stmt: &mut rusqlite::Statement<'_>,
        args: impl rusqlite::Params,
    ) -> rusqlite::Result<Vec<JournalRow>> {
        stmt.query_map(args, JournalRow::from_sql)?.collect()
    }

    fn meta(&self, at: Step<'_>, key: &str) -> Result<Option<String>, CacheError> {
        self.conn
            .prepare_cached("SELECT value FROM meta WHERE key = ?1")
            .at(at)?
            .query_row([key], |r| r.get(0))
            .optional()
            .at(at)
    }

    fn scope_beads(&self, at: Step<'_>, s: &ScopeId) -> Result<u64, CacheError> {
        let n: i64 = self
            .conn
            .prepare_cached(&format!("SELECT COUNT(*) FROM beads b WHERE {IN_SCOPE}"))
            .at(at)?
            .query_row([s.name()], |r| r.get(0))
            .at(at)?;
        Ok(count(n))
    }
}

/// `BEGIN IMMEDIATE`: writers in other processes wait, up to the busy timeout.
fn begin<'c>(conn: &'c mut Connection, at: Step<'_>) -> Result<Transaction<'c>, CacheError> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .at(at)
}

/// Schema, version and identity checks of [`SqliteStore::open_at`], in one transaction.
fn initialize(
    conn: &mut Connection,
    path: &Path,
    identity: &DatabaseIdentity,
) -> Result<(), CacheError> {
    let at = |what| step(path, what);
    let tx = begin(conn, at("initialize"))?;
    let version: i32 = tx
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .at(at("read user_version"))?;
    if version != 0 && version != SCHEMA_VERSION {
        return Err(CacheError::SchemaVersion {
            detail: format!(
                "{} is cache schema {version}, this code is {SCHEMA_VERSION}",
                path.display()
            ),
        });
    }
    tx.execute_batch(V3_SCHEMA).at(at("create the v3 tables"))?;
    tx.execute_batch(EXTENSION_TABLES)
        .at(at("create the extension tables"))?;
    for (table, column, kind) in EXTENSION_COLUMNS {
        let present = tx
            .prepare(&format!(
                "SELECT 1 FROM pragma_table_info('{table}') WHERE name = ?1"
            ))
            .at(at("read the table columns"))?
            .exists([column])
            .at(at("read the table columns"))?;
        if !present {
            tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {kind}"))
                .at(at("add an extension column"))?;
        }
    }
    tx.execute_batch(EXTENSION_INDEXES)
        .at(at("create the extension indexes"))?;
    tx.execute_batch(BACKFILL_APPLIED_EPOCH)
        .at(at("date the entries bd_cache.py applied"))?;
    warn_undatable(&tx, path)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)
        .at(at("set user_version"))?;
    let recorded: Option<String> = tx
        .query_row(
            "SELECT value FROM meta WHERE key = ?1",
            [META_PROJECT_ID],
            |r| r.get(0),
        )
        .optional()
        .at(at("read project_id"))?;
    match recorded {
        None => {
            tx.execute(
                "INSERT OR REPLACE INTO meta VALUES (?1, ?2), (?3, ?4)",
                [
                    META_DOLT_DATABASE,
                    identity.dolt_database.as_str(),
                    META_PROJECT_ID,
                    identity.project_id.as_str(),
                ],
            )
            .at(at("record the database identity"))?;
        }
        Some(p) if p != identity.project_id.as_str() => {
            return Err(CacheError::WrongDatabase {
                detail: format!(
                    "{} belongs to project {p}, this workspace is {}",
                    path.display(),
                    identity.project_id
                ),
            });
        }
        Some(_) => {}
    }
    tx.execute_batch(RECONCILE_MEMBERSHIP)
        .at(at("take over membership"))?;
    tx.commit().at(at("initialize"))
}

/// Warns about applied entries the backfill could not date: their time is
/// unreadable, so they count as applied since every instant and keep `get`
/// of their scope on the adjusted path until an operator resolves or deletes
/// them (ADR-STORE-0004). They still count: correctness comes first.
fn warn_undatable(tx: &Transaction<'_>, path: &Path) -> Result<(), CacheError> {
    let seqs = undatable_seqs(tx).at(step(path, "find undatable journal entries"))?;
    if !seqs.is_empty() {
        tracing::warn!(
            path = %path.display(),
            count = seqs.len(),
            seqs = ?seqs.iter().take(UNDATED_SEQS_LOGGED).collect::<Vec<_>>(),
            "applied journal entries have an unreadable time and count as applied since \
             every instant, so ready answers of their scope are always adjusted; resolve \
             or delete them (ADR-STORE-0004)"
        );
    }
    Ok(())
}

/// The seqs of applied entries still without an epoch, by seq.
fn undatable_seqs(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    conn.prepare(UNDATED_SEQS)?
        .query_map([], |r| r.get(0))?
        .collect()
}

/// Deletes a bead with its outgoing edges and memberships.
fn delete_bead(tx: &Transaction<'_>, at: Step<'_>, id: &str) -> Result<(), CacheError> {
    for sql in [
        "DELETE FROM edges WHERE issue_id = ?1",
        "DELETE FROM scope_member WHERE bead = ?1",
        "DELETE FROM beads WHERE id = ?1",
    ] {
        tx.prepare_cached(sql).at(at)?.execute([id]).at(at)?;
    }
    Ok(())
}

/// SQL predicate: bead `?1` has a pending or escalated entry.
const HAS_OPEN_ENTRY: &str =
    "EXISTS (SELECT 1 FROM journal j WHERE j.bead = ?1 AND j.state IN ('pending', 'escalated'))";

impl CacheStore for SqliteStore {
    fn ensure_scope(&mut self, s: &ScopeId) -> Result<(), CacheError> {
        self.write("ensure scope", |tx, at| {
            tx.prepare_cached("INSERT OR IGNORE INTO scope(name, kind) VALUES (?1, 'phase')")
                .at(at)?
                .execute([s.name()])
                .at(at)?;
            Ok(())
        })
    }

    fn scopes(&self) -> Result<Vec<ScopeId>, CacheError> {
        let at = self.at("read scopes");
        let mut stmt = self
            .conn
            .prepare_cached("SELECT name, kind FROM scope ORDER BY name")
            .at(at)?;
        let rows = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .at(at)?;
        let mut out = Vec::new();
        for r in rows {
            let (name, kind) = r.at(at)?;
            if kind != "phase" {
                return Err(corrupt(
                    at,
                    format_args!("scope {name} has unknown kind {kind:?}"),
                ));
            }
            out.push(scope(at, &name)?);
        }
        Ok(out)
    }

    fn upsert_rows(
        &mut self,
        s: Option<&ScopeId>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError> {
        let at = step(&self.path, "upsert rows");
        let tx = begin(&mut self.conn, at)?;
        let mut report = UpsertReport::default();
        for row in rows {
            match Self::store_row(&tx, at, s, row, force)? {
                Stored::SkippedNewer => report.skipped_newer.push(row.id.clone()),
                Stored::Yes { status_changed } => {
                    if status_changed {
                        report.status_changed.push(row.id.clone());
                    }
                    report.stored.push(row.id.clone());
                }
            }
        }
        tx.commit().at(at)?;
        Ok(report)
    }

    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError> {
        let at = self.at("read stale rows");
        let mut stmt = self
            .conn
            .prepare_cached("SELECT id FROM beads WHERE stale != 0 ORDER BY id")
            .at(at)?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0)).at(at)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(bead_id(at, &r.at(at)?)?);
        }
        Ok(out)
    }

    fn dependents_of(&self, id: &BeadId) -> Result<Vec<Edge>, CacheError> {
        let at = self.at("read dependents");
        // `edges_target` (depends_on_id, type) answers this without a scan (REQ-BDC-0109).
        let mut stmt = self
            .conn
            .prepare_cached(
                "SELECT issue_id, depends_on_id, type FROM edges WHERE depends_on_id = ?1",
            )
            .at(at)?;
        let rows = stmt
            .query_map([id.as_str()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .at(at)?;
        let mut out = Vec::new();
        for r in rows {
            let (from, to, kind) = r.at(at)?;
            out.push(decode_edge(at, &from, &to, &kind)?);
        }
        out.sort();
        Ok(out)
    }

    fn replace_ready_base(
        &mut self,
        s: &ScopeId,
        blocked: &[BlockedRow],
        ready: &[BeadId],
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        let step = self.at("replace a ready base");
        let (blocked_json, ready_json) = encode_ready(step, blocked, ready)?;
        let index = index_blocked(step, blocked)?;
        self.write("replace a ready base", |tx, st| {
            tx.prepare_cached(
                "INSERT OR REPLACE INTO ready_base(scope, blocked_json, ready_json, fetched_at, \
                 stale, indexed) VALUES (?1, ?2, ?3, ?4, 0, 1)",
            )
            .at(st)?
            .execute([s.name(), &blocked_json, &ready_json, at.as_str()])
            .at(st)?;
            tx.prepare_cached("DELETE FROM ready_base_blocked WHERE scope = ?1")
                .at(st)?
                .execute([s.name()])
                .at(st)?;
            let mut insert = tx
                .prepare_cached(
                    "INSERT INTO ready_base_blocked(scope, bead, blocked_by_json) VALUES (?1, ?2, ?3)",
                )
                .at(st)?;
            for (bead, json) in &index {
                insert.execute([s.name(), bead.as_str(), json.as_str()]).at(st)?;
            }
            Ok(())
        })
    }

    fn base_entry(&self, s: &ScopeId, id: &BeadId) -> Result<Option<BaseEntry>, CacheError> {
        // One read transaction: the head and the bead's row are one snapshot,
        // also while another process replaces the base (ADR-STORE-0004).
        self.read("read a ready base entry", || {
            let at = self.at("read a ready base entry");
            let head: Option<(String, i64, Option<i64>)> = self
                .conn
                .prepare_cached(
                    "SELECT fetched_at, stale, indexed FROM ready_base WHERE scope = ?1",
                )
                .at(at)?
                .query_row([s.name()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .optional()
                .at(at)?;
            let Some((fetched_at, stale, indexed)) = head else {
                return Ok(None);
            };
            if indexed != Some(1) {
                // A base another writer stored, or stored before the index
                // existed (ADR-STORE-0004): read it whole.
                return Ok(self.ready_base(s)?.map(|b| BaseEntry::of(&b, id)));
            }
            let json: Option<String> = self
                .conn
                .prepare_cached(
                    "SELECT blocked_by_json FROM ready_base_blocked \
                     WHERE scope = ?1 AND bead = ?2",
                )
                .at(at)?
                .query_row([s.name(), id.as_str()], |r| r.get(0))
                .optional()
                .at(at)?;
            let blocked_by = match json {
                Some(json) => serde_json::from_str(&json).map_err(|e| {
                    corrupt(
                        at,
                        format_args!(
                            "ready base blocked_by_json of {id} is not a list of ids: {e}"
                        ),
                    )
                })?,
                None => Vec::new(),
            };
            Ok(Some(BaseEntry {
                fetched_at: timestamp(at, "ready base fetched_at", &fetched_at)?,
                stale: stale != 0,
                blocked_by,
            }))
        })
    }

    fn has_entries_since(&self, at: &Timestamp) -> Result<bool, CacheError> {
        self.date_undated();
        self.hinted(
            HAS_ENTRIES_SINCE_SQL,
            HAS_ENTRIES_SINCE_UNHINTED_SQL,
            |sql| {
                self.conn
                    .prepare_cached(sql)?
                    .query_row([at.unix()], |r| r.get(0))
            },
        )
        .at(self.at("ask whether the journal has entries since"))
    }

    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError> {
        let at = self.at("read a ready base");
        let found: Option<(String, String, String, i64)> = self
            .conn
            .prepare_cached(
                "SELECT blocked_json, ready_json, fetched_at, stale FROM ready_base WHERE scope = ?1",
            )
            .at(at)?
            .query_row([s.name()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .optional()
            .at(at)?;
        found
            .map(|(blocked, ready, fetched_at, stale)| {
                let (blocked, ready) = decode_ready(at, &blocked, &ready)?;
                Ok(ReadyBase {
                    blocked,
                    ready,
                    fetched_at: timestamp(at, "ready base fetched_at", &fetched_at)?,
                    stale: stale != 0,
                })
            })
            .transpose()
    }

    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError> {
        self.write("mark a ready base stale", |tx, at| {
            tx.prepare_cached("UPDATE ready_base SET stale = 1 WHERE scope = ?1")
                .at(at)?
                .execute([s.name()])
                .at(at)?;
            Ok(())
        })
    }

    fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError> {
        Ok(self
            .entries(
                self.at("read a journal entry"),
                "idem_key = ?1",
                [k.as_str()],
            )?
            .pop())
    }

    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError> {
        Ok(self
            .entries(self.at("read a journal entry"), "seq = ?1", [seq.get()])?
            .pop())
    }

    fn insert_entry(&mut self, e: NewEntry, at: &Timestamp) -> Result<Seq, CacheError> {
        let st = step(&self.path, "journal a write");
        let tx = begin(&mut self.conn, st)?;
        let held: Option<i64> = tx
            .prepare_cached("SELECT seq FROM journal WHERE idem_key = ?1")
            .at(st)?
            .query_row([e.idem_key.as_str()], |r| r.get(0))
            .optional()
            .at(st)?;
        if let Some(held) = held {
            return Err(CacheError::IdemConflict {
                detail: format!("idem_key {} already journals entry {held}", e.idem_key),
                seq: Seq::new(held),
            });
        }
        tx.prepare_cached(
            "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, actor, \
             state, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?8)",
        )
        .at(st)?
        .execute(params![
            e.idem_key.as_str(),
            e.scope.as_ref().map(ScopeId::name),
            e.bead.as_str(),
            e.op.name(),
            e.op.to_args_json(),
            precondition_json(&e.precondition),
            e.actor.as_str(),
            at.as_str(),
        ])
        .at(st)?;
        let seq = Seq::new(tx.last_insert_rowid());
        tx.commit().at(st)?;
        Ok(seq)
    }

    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.entries(self.at("read pending entries"), "state = 'pending'", [])
    }

    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError> {
        self.entries(
            self.at("read pending entries"),
            "state = 'pending' AND bead = ?1",
            [id.as_str()],
        )
    }

    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError> {
        // An entry's time is `applied_epoch`; for one `bd_cache.py` applied
        // (no epoch, no `applied_at`) it is `updated_at`, which that writer
        // sets when it marks the entry; one whose time cannot be read counts
        // as applied since (ADR-STORE-0004). Such an entry has no `applied_at`
        // to decode, and is kept.
        self.date_undated();
        let st = self.at("read applied entries");
        let rows = self
            .hinted(APPLIED_SINCE_SQL, APPLIED_SINCE_UNHINTED_SQL, |sql| {
                Self::journal_rows(&mut *self.conn.prepare_cached(sql)?, [at.unix()])
            })
            .at(st)?;
        let mut out = rows
            .into_iter()
            .map(|r| r.decode(st))
            .collect::<Result<Vec<_>, _>>()?;
        out.retain(|e| e.applied_at.as_ref().is_none_or(|t| t.unix() >= at.unix()));
        Ok(out)
    }

    fn apply_with_readback(
        &mut self,
        seq: Seq,
        row: Option<&Bead>,
        err: Option<&str>,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        let st = step(&self.path, "apply a journal entry");
        let tx = begin(&mut self.conn, st)?;
        let bead: String = tx
            .prepare_cached("SELECT bead FROM journal WHERE seq = ?1")
            .at(st)?
            .query_row([seq.get()], |r| r.get(0))
            .optional()
            .at(st)?
            .ok_or_else(|| not_found(seq))?;
        match row {
            Some(row) if row.id.as_str() != bead => {
                return Err(CacheError::BdContract {
                    bead: Some(row.id.clone()),
                    field: "id".into(),
                    detail: format!(
                        "read-back row {} is not the bead of journal entry {seq} ({bead})",
                        row.id
                    ),
                    source: None,
                });
            }
            Some(row) => {
                Self::store_row(&tx, st, None, row, true)?;
            }
            None => {
                tx.prepare_cached("UPDATE beads SET stale = 1 WHERE id = ?1")
                    .at(st)?
                    .execute([&bead])
                    .at(st)?;
            }
        }
        tx.prepare_cached(
            "UPDATE journal SET state = 'applied', last_error = ?2, applied_at = ?3, \
             applied_epoch = ?4, updated_at = ?3 WHERE seq = ?1",
        )
        .at(st)?
        .execute(params![seq.get(), err, at.as_str(), at.unix()])
        .at(st)?;
        tx.commit().at(st)
    }

    fn record_attempt(&mut self, seq: Seq, err: &str, at: &Timestamp) -> Result<u32, CacheError> {
        self.write("record an attempt", |tx, st| {
            let attempts: i64 = tx
                .prepare_cached(
                    "UPDATE journal SET attempts = attempts + 1, last_error = ?2, updated_at = ?3 \
                     WHERE seq = ?1 RETURNING attempts",
                )
                .at(st)?
                .query_row(params![seq.get(), err, at.as_str()], |r| r.get(0))
                .optional()
                .at(st)?
                .ok_or_else(|| not_found(seq))?;
            // The store's own validation, as `JournalRow::decode` does it: no source.
            u32::try_from(attempts)
                .map_err(|e| corrupt(st, format_args!("journal entry {seq}: attempts: {e}")))
        })
    }

    fn escalate(&mut self, seq: Seq, reason: &str, at: &Timestamp) -> Result<(), CacheError> {
        self.write("escalate", |tx, st| {
            let n = tx
                .prepare_cached(
                    "UPDATE journal SET state = 'escalated', last_error = ?2, updated_at = ?3 \
                     WHERE seq = ?1",
                )
                .at(st)?
                .execute(params![seq.get(), reason, at.as_str()])
                .at(st)?;
            if n == 0 {
                return Err(not_found(seq));
            }
            Ok(())
        })
    }

    fn resolve(
        &mut self,
        seq: Seq,
        note: &str,
        actor: &Actor,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        let st = step(&self.path, "resolve");
        let tx = begin(&mut self.conn, st)?;
        let state: Option<String> = tx
            .prepare_cached("SELECT state FROM journal WHERE seq = ?1")
            .at(st)?
            .query_row([seq.get()], |r| r.get(0))
            .optional()
            .at(st)?;
        if state.as_deref() != Some(JournalState::Escalated.as_str()) {
            return Err(CacheError::NotEscalated {
                detail: format!(
                    "journal entry {seq} is {}",
                    state.as_deref().unwrap_or("absent")
                ),
            });
        }
        let suffix = format!(" | resolved by {actor}: {note}");
        tx.prepare_cached(
            "UPDATE journal SET state = 'resolved', last_error = COALESCE(last_error, '') || ?2, \
             updated_at = ?3 WHERE seq = ?1",
        )
        .at(st)?
        .execute(params![seq.get(), suffix, at.as_str()])
        .at(st)?;
        tx.commit().at(st)
    }

    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.entries(
            self.at("read unannounced escalations"),
            "state = 'escalated' AND announced_at IS NULL",
            [],
        )
    }

    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError> {
        self.write("mark announced", |tx, st| {
            let n = tx
                .prepare_cached("UPDATE journal SET announced_at = ?2 WHERE seq = ?1")
                .at(st)?
                .execute(params![seq.get(), at.as_str()])
                .at(st)?;
            if n == 0 {
                return Err(not_found(seq));
            }
            Ok(())
        })
    }

    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError> {
        let st = step(&self.path, "record the backend fingerprint");
        let tx = begin(&mut self.conn, st)?;
        let recorded: Option<String> = tx
            .prepare_cached("SELECT value FROM meta WHERE key = ?1")
            .at(st)?
            .query_row([META_SHA], |r| r.get(0))
            .optional()
            .at(st)?;
        let outcome = match recorded {
            None => FingerprintOutcome::Recorded,
            Some(sha) if sha == f.schema_sha256.as_str() => {
                return Ok(FingerprintOutcome::Unchanged);
            }
            Some(_) => FingerprintOutcome::Rebuilt,
        };
        if outcome == FingerprintOutcome::Rebuilt {
            // Rows and everything derived from them; the journal, the scope
            // registry, alerts and the outage stay (REQ-BDC-0105, ADR-MODEL-0003).
            tx.execute_batch(
                "DELETE FROM edges; DELETE FROM beads; DELETE FROM scope_member; \
                 DELETE FROM sync_state; DELETE FROM ready_base; \
                 DELETE FROM ready_base_blocked;",
            )
            .at(st)?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO meta VALUES (?1, ?2), (?3, ?4), (?5, ?6)",
            [
                META_SHA,
                f.schema_sha256.as_str(),
                META_VERSION,
                &f.bd_version,
                META_SCHEMA_VERSION,
                &f.schema_version,
            ],
        )
        .at(st)?;
        tx.commit().at(st)?;
        Ok(outcome)
    }

    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError> {
        let at = self.at("read sync state");
        let found: Option<(Option<String>, Option<String>, Option<String>)> = self
            .conn
            .prepare_cached(
                "SELECT last_started_at, last_ok_at, last_error FROM sync_state WHERE phase = ?1",
            )
            .at(at)?
            .query_row([s.name()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .at(at)?;
        let Some((started, ok, last_error)) = found else {
            return Ok(SyncState::new(s.clone()));
        };
        Ok(SyncState {
            scope: s.clone(),
            last_started_at: opt_timestamp(at, "last_started_at", started.as_deref())?,
            last_ok_at: opt_timestamp(at, "last_ok_at", ok.as_deref())?,
            last_error,
        })
    }

    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError> {
        self.write("record sync state", |tx, at| {
            tx.prepare_cached(
                "INSERT INTO sync_state(phase, last_started_at, last_ok_at, last_error) \
                 VALUES (?1, ?2, ?3, ?4) ON CONFLICT(phase) DO UPDATE SET \
                 last_started_at = excluded.last_started_at, last_ok_at = excluded.last_ok_at, \
                 last_error = excluded.last_error",
            )
            .at(at)?
            .execute(params![
                st.scope.name(),
                st.last_started_at.as_ref().map(Timestamp::as_str),
                st.last_ok_at.as_ref().map(Timestamp::as_str),
                st.last_error,
            ])
            .at(at)?;
            Ok(())
        })
    }

    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.meta(self.at("read a sync alert"), &alert_key(s))
    }

    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        let key = alert_key(s);
        self.write("set a sync alert", |tx, at| {
            match cause {
                Some(c) => tx
                    .prepare_cached("INSERT OR REPLACE INTO meta VALUES (?1, ?2)")
                    .at(at)?
                    .execute([key.as_str(), c]),
                None => tx
                    .prepare_cached("DELETE FROM meta WHERE key = ?1")
                    .at(at)?
                    .execute([key.as_str()]),
            }
            .at(at)?;
            Ok(())
        })
    }

    fn outage(&self) -> Result<Option<Outage>, CacheError> {
        let at = self.at("read the outage");
        // One statement, so both keys come from one snapshot: a concurrent
        // set or clear (one transaction in either implementation) is seen
        // whole or not at all.
        let (since, last_error): (Option<String>, Option<String>) = self
            .conn
            .prepare_cached(
                "SELECT (SELECT value FROM meta WHERE key = ?1), \
                 (SELECT value FROM meta WHERE key = ?2)",
            )
            .at(at)?
            .query_row([META_OUTAGE_SINCE, META_OUTAGE_ERROR], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .at(at)?;
        let Some(since) = since else {
            return Ok(None);
        };
        let last_error =
            last_error.ok_or_else(|| corrupt(at, "outage_since is set without outage_error"))?;
        Ok(Some(Outage {
            since: timestamp(at, "outage_since", &since)?,
            last_error,
        }))
    }

    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError> {
        let at = step(&self.path, "set the outage");
        let tx = begin(&mut self.conn, at)?;
        match o {
            Some(o) => tx.execute(
                "INSERT OR REPLACE INTO meta VALUES (?1, ?2), (?3, ?4)",
                [
                    META_OUTAGE_SINCE,
                    o.since.as_str(),
                    META_OUTAGE_ERROR,
                    &o.last_error,
                ],
            ),
            // The announcement mark goes with the outage, as in bd_cache.py.
            None => tx.execute(
                "DELETE FROM meta WHERE key IN (?1, ?2, ?3)",
                [META_OUTAGE_SINCE, META_OUTAGE_ERROR, META_OUTAGE_ANNOUNCED],
            ),
        }
        .at(at)?;
        tx.commit().at(at)
    }

    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        let at = step(&self.path, "prune a scope");
        let tx = begin(&mut self.conn, at)?;
        tx.execute_batch(RECONCILE_MEMBERSHIP).at(at)?;
        let open: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM journal WHERE phase = ?1 AND state IN ('pending', 'escalated')",
                [s.name()],
                |r| r.get(0),
            )
            .at(at)?;
        if open > 0 {
            return Err(CacheError::PruneBlocked {
                detail: format!("phase {} has {open} pending or escalated writes", s.name()),
            });
        }
        let mut report = PruneReport {
            scope: s.clone(),
            beads_deleted: 0,
            out_of_scope_swept: 0,
        };
        let held: Vec<String> = {
            let mut stmt = tx
                .prepare_cached("SELECT bead FROM scope_member WHERE scope = ?1 ORDER BY bead")
                .at(at)?;
            let rows = stmt.query_map([s.name()], |r| r.get(0)).at(at)?;
            rows.collect::<rusqlite::Result<_>>().at(at)?
        };
        tx.execute("DELETE FROM scope_member WHERE scope = ?1", [s.name()])
            .at(at)?;
        let kept = format!(
            "SELECT EXISTS (SELECT 1 FROM scope_member WHERE bead = ?1) OR {HAS_OPEN_ENTRY}"
        );
        for id in &held {
            let keep: bool = tx
                .prepare_cached(&kept)
                .at(at)?
                .query_row([id], |r| r.get(0))
                .at(at)?;
            if !keep {
                delete_bead(&tx, at, id)?;
                report.beads_deleted = report.beads_deleted.saturating_add(1);
            }
        }
        for sql in [
            "DELETE FROM journal WHERE phase = ?1",
            "DELETE FROM sync_state WHERE phase = ?1",
            "DELETE FROM ready_base WHERE scope = ?1",
            "DELETE FROM ready_base_blocked WHERE scope = ?1",
        ] {
            tx.execute(sql, [s.name()]).at(at)?;
        }
        let loose: Vec<String> = {
            let mut stmt = tx
                .prepare(
                    "SELECT b.id FROM beads b WHERE NOT EXISTS \
                     (SELECT 1 FROM scope_member m WHERE m.bead = b.id) AND NOT \
                     EXISTS (SELECT 1 FROM journal j WHERE j.bead = b.id AND \
                     j.state IN ('pending', 'escalated')) ORDER BY b.id",
                )
                .at(at)?;
            let rows = stmt.query_map([], |r| r.get(0)).at(at)?;
            rows.collect::<rusqlite::Result<_>>().at(at)?
        };
        for id in &loose {
            delete_bead(&tx, at, id)?;
            report.out_of_scope_swept = report.out_of_scope_swept.saturating_add(1);
        }
        tx.commit().at(at)?;
        Ok(report)
    }

    fn open_work(&self) -> Result<OpenWork, CacheError> {
        let at = self.at("count open work");
        // One statement: both counts from one snapshot.
        let (pending, escalated): (i64, i64) = self
            .conn
            .prepare_cached(
                "SELECT COALESCE(SUM(state = 'pending'), 0), \
                 COALESCE(SUM(state = 'escalated' AND announced_at IS NULL), 0) FROM journal",
            )
            .at(at)?
            .query_row([], |r| Ok((r.get(0)?, r.get(1)?)))
            .at(at)?;
        Ok(OpenWork {
            pending: count(pending),
            escalated_unannounced: count(escalated),
        })
    }

    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError> {
        self.read("read a row", || self.get_row_now(id))
    }

    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError> {
        self.read("read a scope's rows", || self.rows_in_scope_now(s))
    }

    fn status(&self) -> Result<StoreStatus, CacheError> {
        self.read("read status", || self.status_now())
    }
}

/// Reads that span several statements, each run on one snapshot.
impl SqliteStore {
    /// Runs `f` in one read transaction, so every statement in it sees one
    /// snapshot of the file (a concurrent writer commits before or after it).
    fn read<T>(
        &self,
        what: &str,
        f: impl FnOnce() -> Result<T, CacheError>,
    ) -> Result<T, CacheError> {
        let at = self.at(what);
        let tx = self.conn.unchecked_transaction().at(at)?;
        let out = f()?;
        tx.commit().at(at)?;
        Ok(out)
    }

    fn get_row_now(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError> {
        let at = self.at("read a row");
        let found: Option<(String, i64)> = self
            .conn
            .prepare_cached("SELECT row_json, stale FROM beads WHERE id = ?1")
            .at(at)?
            .query_row([id.as_str()], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .at(at)?;
        found
            .map(|(json, stale)| self.load_row(at, id.as_str(), &json, stale))
            .transpose()
    }

    fn rows_in_scope_now(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError> {
        let at = self.at("read a scope's rows");
        let mut stmt = self
            .conn
            .prepare_cached(&format!(
                "SELECT b.id, b.row_json, b.stale FROM beads b WHERE {IN_SCOPE} ORDER BY b.id"
            ))
            .at(at)?;
        let rows = stmt
            .query_map([s.name()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .at(at)?;
        let mut out = Vec::new();
        for r in rows {
            let (id, json, stale) = r.at(at)?;
            out.push(self.load_row(at, &id, &json, stale)?);
        }
        Ok(out)
    }

    fn status_now(&self) -> Result<StoreStatus, CacheError> {
        let at = self.at("read status");
        let mut meta = BTreeMap::new();
        {
            let mut stmt = self
                .conn
                .prepare_cached("SELECT key, value FROM meta")
                .at(at)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
                .at(at)?;
            for r in rows {
                let (k, v) = r.at(at)?;
                meta.insert(k, v);
            }
        }
        let mut journal = BTreeMap::new();
        {
            let mut stmt = self
                .conn
                .prepare_cached("SELECT phase, state, COUNT(*) FROM journal GROUP BY phase, state")
                .at(at)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, Option<String>>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                })
                .at(at)?;
            for r in rows {
                let (phase, state, n) = r.at(at)?;
                // v3 prints a missing phase as Python's None.
                let phase = phase.as_deref().unwrap_or("None");
                journal.insert(format!("{phase}:{state}"), count(n));
            }
        }
        let mut names: BTreeSet<ScopeId> = self.scopes()?.into_iter().collect();
        {
            let mut stmt = self
                .conn
                .prepare_cached("SELECT phase FROM sync_state")
                .at(at)?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).at(at)?;
            for r in rows {
                names.insert(scope(at, &r.at(at)?)?);
            }
        }
        let mut scopes = Vec::new();
        for s in names {
            scopes.push(ScopeStatus {
                beads: self.scope_beads(at, &s)?,
                sync: self.sync_state(&s)?,
            });
        }
        Ok(StoreStatus {
            meta,
            journal,
            scopes,
            escalated: self.entries(at, "state = 'escalated'", [])?,
            outage: self.outage()?,
            stale: self.stale_rows()?,
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "test code: a failed unwrap is a test failure"
    )]
    use super::*;
    use crate::location::{DoltDatabase, ProjectId};

    /// The open's warning names exactly the applied entries left undated
    /// (QA4-004): the unreadable one, not the dated one nor a pending one.
    #[test]
    fn the_open_finds_the_applied_entries_it_could_not_date() {
        let mut conn = Connection::open_in_memory().unwrap();
        let identity = DatabaseIdentity {
            dolt_database: DoltDatabase::new("p").unwrap(),
            project_id: ProjectId::new("p-p").unwrap(),
        };
        let path = Path::new(":memory:");
        initialize(&mut conn, path, &identity).unwrap();
        conn.execute_batch(
            "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
             actor, state, attempts, created_at, updated_at) VALUES \
             ('ok', 'x', 'b', 'close', '{}', '{}', 'a', 'applied', 0, 't', '2026-10-24T09:00:00Z'), \
             ('bad', 'x', 'b', 'close', '{}', '{}', 'a', 'applied', 0, 't', 'yesterday'), \
             ('wait', 'x', 'b', 'close', '{}', '{}', 'a', 'pending', 0, 't', 'yesterday');",
        )
        .unwrap();
        assert_eq!(
            undatable_seqs(&conn).unwrap(),
            [1, 2],
            "before the backfill"
        );
        initialize(&mut conn, path, &identity).unwrap();
        assert_eq!(
            undatable_seqs(&conn).unwrap(),
            [2],
            "only the unreadable one"
        );
    }
}
