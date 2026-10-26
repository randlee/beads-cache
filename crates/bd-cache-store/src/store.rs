//! [`SqliteStore`]: the [`CacheStore`] over one SQLite file shared with the v3 Python writer.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use bd_cache_model::{
    BaseEntry, CacheError, CacheStore, EscalationPage, FailingSince, FingerprintOutcome, Freshness,
    HookState, IdemKey, IngestSnapshot, JournalCounts, JournalEntry, JournalState, LeaseHolder,
    LeaseTake, MAX_REGISTERED_SCOPES, NewEntry, OpenWork, Outage, PendingSummary,
    PreconditionReason, PruneReport, ReadyBase, RepoConfig, RunId, Scope, ScopeId, ScopeStatus,
    Seq, StoreStatus, SyncOutcome, SyncRun, SyncRunEnd, SyncState, UpsertReport,
    check_lease_expiry, clip_hook_error, error_code_prefix, phase_of, precondition_json,
    resolve_note,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sc_beads::logging::tracing;
use sc_beads::{Actor, Bead, BeadId, BlockedRow, Edge, Fingerprint, Timestamp};

use crate::codec::{
    JOURNAL_COLUMNS, JournalRow, KIND_PHASE, bead_id, decode_edge, decode_ready, decode_row,
    decode_scope, encode_ready, encode_row, encode_scope, index_blocked, is_newer, opt_timestamp,
    scope, timestamp,
};
use crate::error::{Step, corrupt, failed};
use crate::location::{BeadsDirOverride, CacheLocation, DatabaseIdentity, env_beads_dir};
use crate::lock::FlockDrainLock;
use crate::registry::Registration;
use crate::schedule::CacheScheduleStore;
use crate::schema::in_scope;
use crate::schema::{
    APPLIED_SINCE_SQL, APPLIED_SINCE_UNHINTED_SQL, BACKFILL_APPLIED_EPOCH,
    BACKFILL_APPLIED_EPOCH_UNHINTED, EXTENSION_COLUMNS, EXTENSION_INDEXES, EXTENSION_TABLES,
    FIRST_DATEABLE_ABOVE_SQL, FIRST_DATEABLE_ABOVE_UNHINTED_SQL, HAS_ENTRIES_SINCE_SQL,
    HAS_ENTRIES_SINCE_UNHINTED_SQL, LAST_UNDATED_SQL, LAST_UNDATED_UNHINTED_SQL,
    RECONCILE_MEMBERSHIP, SC_CRON_SCHEDULE_VERSION, SC_CRON_SCHEDULE_VERSION_KEY, SCHEMA_VERSION,
    UNDATABLE_BETWEEN_SQL, UNDATABLE_BETWEEN_UNHINTED_SQL, UNDATED_ABOVE_SQL,
    UNDATED_ABOVE_UNHINTED_SQL, UNDATED_SEQS, V3_SCHEMA, sc_cron_schedule_table,
};

/// SQLite's busy timeout on the shared file (REQ-BDC-0103).
pub const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// How many undated journal entries (applied by the v3 Python writer while the store
/// is open, so without an epoch) the journal reads walk before they date them
/// (ADR-STORE-0004, lead ruling S26).
///
/// At or below it, `has_entries_since` and `applied_since` write nothing and
/// answer by walking the undated entries: about 0.1 us each, so a cached `get`
/// stays well under NFR-BDC-0001's 1 ms with no WAL commit on its path. Above
/// it they date the entries first, without waiting for the write lock: a
/// one-off commit (a few ms per thousand entries) after which the walk is
/// empty again. The measurements behind the value are in ADR-STORE-0004.
pub const READ_DATING_THRESHOLD: u32 = 2_000;

const META_PROJECT_ID: &str = "project_id";
const META_DOLT_DATABASE: &str = "dolt_database";
const META_SHA: &str = "bd_schema_sha256";
const META_VERSION: &str = "bd_version";
const META_SCHEMA_VERSION: &str = "bd_schema_version";
const META_OUTAGE_SINCE: &str = "outage_since";
const META_OUTAGE_ERROR: &str = "outage_error";
/// Written by the v3 cron script when it announces an outage; cleared with the outage.
const META_OUTAGE_ANNOUNCED: &str = "outage_announced_at";

/// SQL predicate: bead `?1` has a pending or escalated entry.
macro_rules! has_open_entry {
    () => {
        "EXISTS (SELECT 1 FROM journal j WHERE j.bead = ?1 AND j.state IN ('pending', 'escalated'))"
    };
}

/// `synced_at` (v3, NOT NULL, never read by either implementation): the
/// SQLite clock at the store, as the v3 Python writer writes its own clock
/// (ADR-STORE-0002). No value the port returns depends on it.
macro_rules! synced_at {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%SZ', 'now')"
    };
}

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

/// How many seqs a warning about undatable entries names.
const UNDATED_SEQS_LOGGED: usize = 20;

/// The BD-CACHE v3 SQLite store (ADR-STORE-0001), shared with the v3 Python writer.
///
/// One `SqliteStore` is the process's one connection to its cache file
/// (NFR-BDC-0007): opening the same file again in the same process, while a
/// store on it is alive, is refused. The store is `Send + Sync`: its one
/// connection sits behind the mutex it shares with its schedule view
/// ([`CacheScheduleStore`](crate::CacheScheduleStore), ADR-BDC-0014), and each
/// call locks it for one statement or transaction. The `Cache` that owns it
/// stays `!Sync` by its own marker (ADR-CORE-0001), whatever the store is; a
/// test pins both. Every mutating method runs in one
/// `BEGIN IMMEDIATE` transaction, so a concurrent writer in another process
/// (a hook, the Python implementation) waits up to [`BUSY_TIMEOUT`]. The
/// journal reads `has_entries_since` and `applied_since` may also write: when
/// more than [`READ_DATING_THRESHOLD`] entries another writer applied wait to
/// be dated, they date them, never waiting for the write lock, and answer the
/// same either way (ADR-STORE-0004, lead rulings S25/S26). The bounded marker
/// writes (`mark_announced_within`, `set_sync_alert_within`) wait at most the
/// time they are given, never more than [`BUSY_TIMEOUT`] (lead ruling S33).
///
/// # Examples
///
/// ```
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use bd_cache_model::{CacheStore, RepoConfig, Scope, ScopeFilter, ScopeId};
/// use bd_cache_store::SqliteStore;
///
/// let dir = tempfile::TempDir::new()?;
/// let root = dir.path();
/// std::fs::create_dir_all(root.join(".beads"))?;
/// std::fs::write(
///     root.join(".beads").join("metadata.json"),
///     r#"{"dolt_database": "dev", "project_id": "p-1"}"#,
/// )?;
/// // `None`: no `$BEADS_DIR`, whatever the environment holds (S36).
/// let mut store = SqliteStore::open_with(root, &RepoConfig::default(), None)?;
/// let area = Scope::new(ScopeId::new("area")?, ScopeFilter::label("area")?);
/// let mut b = sc_beads::testing::bead("b1");
/// b.labels = vec!["area".into()];
/// store.upsert_rows(Some(&area), &[b], false)?;
/// assert_eq!(store.rows_in_scope(&area.id)?.len(), 1);
/// # drop(store);
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct SqliteStore {
    shared: Arc<Shared>,
}

/// The cache file's one connection and this process's claim on the file,
/// shared by the store and its schedule view (ADR-BDC-0014).
///
/// The claim is released only when the last holder drops: a
/// [`CacheScheduleStore`](crate::CacheScheduleStore) a scheduler still holds
/// keeps it after the [`SqliteStore`] is dropped, so a second open in the
/// process is still `STORE_IN_USE` (NFR-BDC-0007).
#[derive(Debug)]
pub(crate) struct Shared {
    // Field order matters: the connection closes before the claim is released.
    inner: Mutex<Inner>,
    path: PathBuf,
    _claim: Registration,
}

impl Shared {
    /// The connection and its state, for one call. A poisoned lock is
    /// recovered: rusqlite rolls an open transaction back on drop, so the
    /// connection is sound after a panic.
    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The cache file.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

/// The connection and the read path's dating state, behind [`Shared`]'s mutex.
///
/// The three dating fields are `Cell`s although the mutex already gives
/// exclusive access: the port's journal reads (`has_entries_since`,
/// `applied_since`) take `&self`, as every read of [`CacheStore`] does, yet
/// may date entries and raise the floor (ADR-STORE-0004). The `Session` those
/// reads run on hands out `&` to its helpers, so the state they update is
/// interior-mutable; `Cell` is enough because a `Session` lives inside one
/// lock of the mutex on one thread (`Inner` is never shared without it).
#[derive(Debug)]
pub(crate) struct Inner {
    pub(crate) conn: Connection,
    /// Set, with one warning, when a hinted journal index is found missing:
    /// from then on the journal reads ask their unhinted form (ADR-STORE-0004).
    hints_missing: Cell<bool>,
    /// The floor of known unreadable entries. The read path counts only
    /// undated entries above it for the threshold, so the entries no dating
    /// can date are not walked on every call (QA5-002).
    ///
    /// The invariant holds per rise (QA7-001): a rise from an old floor to a
    /// new one passes only the entries between the two (above the old, at or
    /// below the new), and every one of those that was applied and undated
    /// when the floor rose had an unreadable time (warned about); a rise never
    /// passes an applied entry the dating can date. Entries already below the
    /// old floor are not examined again by a rise, so the floor says nothing
    /// about them: an entry still pending when an earlier rise passed it,
    /// which another v3 writer applies later without an epoch, sits below the
    /// floor with a readable time, and stays there through later rises. Such
    /// an entry is answered correctly (the journal reads do not use the
    /// floor, and read its own time), is left out of the threshold count, and
    /// is dated by the next dating, which dates every readable entry whatever
    /// its seq, or at the next open (QA6-001).
    undatable_through: Cell<i64>,
    /// Every undated entry with a seq at or below this one has had its time
    /// examined; only higher seqs are examined (and warned about) again.
    examined_through: Cell<i64>,
}

/// One call's view of the store: the locked connection and its state.
struct Session<'a> {
    conn: &'a mut Connection,
    path: &'a Path,
    hints_missing: &'a Cell<bool>,
    undatable_through: &'a Cell<i64>,
    examined_through: &'a Cell<i64>,
}

/// The stored columns an upsert decides on: `updated_at`, `status`,
/// `membership_known`, `phase`.
type Existing = (Option<String>, Option<String>, Option<i64>, Option<String>);

/// The `sync_state` columns a read decodes.
type SyncColumns = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// What [`Session::store_row`] did with one row.
enum Stored {
    /// Stored; `true` when an existing row's status text changed.
    Yes { status_changed: bool },
    /// Skipped: the stored row is a later instant.
    SkippedNewer,
}

impl SqliteStore {
    /// Opens (creating when absent) the cache file of `repo_root`, reading
    /// `$BEADS_DIR` from the environment.
    ///
    /// The file is located from `config` and `.beads/metadata.json`
    /// ([`CacheLocation::resolve`], `$BEADS_DIR` aware); no `bd` process starts.
    /// The environment's value is validated as an explicit one is
    /// ([`BeadsDirOverride::from_env`]; lead ruling S49).
    ///
    /// # Errors
    ///
    /// As [`CacheLocation::resolve`] and [`SqliteStore::open_at`]: a
    /// `$BEADS_DIR` that cannot be made absolute is `BD_CACHE.NO_DATABASE`,
    /// its detail naming the value and the I/O error, before any file is read.
    pub fn open(repo_root: &Path, config: &RepoConfig) -> Result<Self, CacheError> {
        Self::open_with(repo_root, config, env_beads_dir()?.as_ref())
    }

    /// Opens (creating when absent) the cache file of `repo_root` with an
    /// explicit `$BEADS_DIR` override (`None` = unset), as
    /// [`CacheLocation::resolve_with`]: the environment is not read, so a
    /// test or a caller that has its own value is not machine-dependent
    /// (lead ruling S36). The override is already absolute
    /// ([`BeadsDirOverride`], lead ruling S49).
    ///
    /// # Errors
    ///
    /// As [`CacheLocation::resolve_with`] and [`SqliteStore::open_at`].
    pub fn open_with(
        repo_root: &Path,
        config: &RepoConfig,
        beads_dir: Option<&BeadsDirOverride>,
    ) -> Result<Self, CacheError> {
        let loc = CacheLocation::resolve_with(repo_root, config, beads_dir)?;
        Self::open_at(&loc.cache_path, &loc.identity)
    }

    /// Opens (creating when absent) the cache file at `path` for `identity`.
    ///
    /// In order (ADR-STORE-0002): sets a 5 s busy timeout; then, in one
    /// transaction, checks `user_version` (0 is initialized to 3, 3 is
    /// opened, anything else refused), creates the v3 tables and the additive
    /// extensions when absent, records `dolt_database` and `project_id` on
    /// first open and checks `project_id` on every later one, and takes over
    /// the membership of rows the v3 Python writer wrote; then sets WAL mode. A
    /// refused file is left as it was, its journal mode included.
    /// `user_version` is never changed from 3.
    ///
    /// # Errors
    ///
    /// - `BD_CACHE.SCHEMA_VERSION`: the file has another nonzero `user_version` (REQ-BDC-0103).
    /// - `BD_CACHE.WRONG_DATABASE`: the file records another `project_id` (REQ-BDC-0104).
    /// - `BD_CACHE.STORE_FAILED`: the file cannot be created or opened, is not
    ///   SQLite, or refuses WAL (ADR-STORE-0003); the underlying error, when
    ///   there is one, is the source.
    /// - `BD_CACHE.STORE_IN_USE`: this process already has a connection to it
    ///   (NFR-BDC-0007; PE-RBP-F008), through a handle it must close or reuse.
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
        let claim = Registration::claim(&canonical).ok_or_else(|| CacheError::StoreInUse {
            detail: format!(
                "{}: this process already has a connection to the cache file (NFR-BDC-0007)",
                at("open")
            ),
        })?;
        let mut conn = conn;
        conn.busy_timeout(BUSY_TIMEOUT)
            .at(at("set the busy timeout"))?;
        // Version and identity are checked (and the schema created) before
        // WAL is set, so a refused file is left exactly as it was, journal
        // mode included.
        let undatable = initialize(&mut conn, path, identity)?;
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
            shared: Arc::new(Shared {
                inner: Mutex::new(Inner {
                    conn,
                    hints_missing: Cell::new(false),
                    undatable_through: Cell::new(undatable),
                    examined_through: Cell::new(undatable),
                }),
                path: path.to_path_buf(),
                _claim: claim,
            }),
        })
    }

    /// The cache file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.shared.path
    }

    /// The drain lock of this cache file (`<cache>.drain.lock`, REQ-BDC-0401).
    #[must_use]
    pub fn drain_lock(&self) -> FlockDrainLock {
        FlockDrainLock::for_cache_file(&self.shared.path)
    }

    /// The schedule store over this cache file's one connection (ADR-BDC-0014).
    ///
    /// The view shares the store's connection and its claim on the file: the
    /// process opens no second connection, and the claim is released only
    /// when the store and every view are dropped. The `sc_cron_schedule`
    /// table is in the cache file (created when the store opened); the
    /// `meta` row `sc_cron_schedule_version` is checked here, before any
    /// schedule is served.
    ///
    /// # Errors
    ///
    /// [`sc_cron::CronError::Store`] with [`sc_cron::StoreErrorKind::SchemaMismatch`]
    /// when the file's `sc_cron_schedule_version` row names another version
    /// than `sc_cron::SCHEMA_VERSION`, or
    /// another store error when the row cannot be read.
    pub fn schedule_store(&self) -> Result<CacheScheduleStore, sc_cron::CronError> {
        CacheScheduleStore::over(Arc::clone(&self.shared))
    }

    /// One call on the locked connection.
    fn session<T>(&self, f: impl FnOnce(&mut Session<'_>) -> T) -> T {
        let mut guard = self.shared.lock();
        let inner = &mut *guard;
        let mut session = Session {
            conn: &mut inner.conn,
            path: &self.shared.path,
            hints_missing: &inner.hints_missing,
            undatable_through: &inner.undatable_through,
            examined_through: &inner.examined_through,
        };
        f(&mut session)
    }

    /// The locked connection and its state, for the unit tests.
    #[cfg(test)]
    pub(crate) fn inner(&self) -> MutexGuard<'_, Inner> {
        self.shared.lock()
    }
}

impl Session<'_> {
    fn at<'a>(&'a self, what: &'a str) -> Step<'a> {
        step(self.path, what)
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
    /// (the v3 Python writer's, written while this store is open) when there are
    /// more than [`READ_DATING_THRESHOLD`] of them, so the journal reads stay
    /// index lookups (ADR-STORE-0004, lead ruling S26). At or below the
    /// threshold nothing is written: the reads walk them, which is cheaper
    /// than a commit. Best effort and never waits: the `UPDATE` runs with no
    /// busy timeout ([`BusyWait`] of zero), and when the write lock is held elsewhere,
    /// or the file is read-only, nothing is dated now and the reads answer
    /// the same from the undated entries, only slower.
    ///
    /// Entries with an unreadable time are warned about once for each new
    /// highest such seq, and are not counted again (QA5-002).
    fn date_undated(&self) {
        let floor = self.undatable_through.get();
        let limit = i64::from(READ_DATING_THRESHOLD) + 1;
        let waiting: rusqlite::Result<i64> =
            self.hinted(UNDATED_ABOVE_SQL, UNDATED_ABOVE_UNHINTED_SQL, |sql| {
                self.conn
                    .prepare_cached(sql)?
                    .query_row([floor, limit], |r| r.get(0))
            });
        let dated = match waiting {
            Ok(0) => return,
            Ok(n) => n >= limit && self.date_now(),
            Err(e) => {
                tracing::debug!(error = %e, "could not count undated journal entries");
                return;
            }
        };
        let warned = self.note_undatable().unwrap_or_else(|e| {
            tracing::debug!(error = %e, "could not look for undatable journal entries");
            None
        });
        if (dated || warned.is_some())
            && let Err(e) = self.raise_floor()
        {
            tracing::debug!(error = %e, "could not skip undatable journal entries");
        }
    }

    /// The non-waiting `UPDATE` of [`SqliteStore::date_undated`]; `true` when
    /// it ran, so every entry with a readable time is dated.
    fn date_now(&self) -> bool {
        let _no_wait = match BusyWait::new(&*self.conn, self.path, Duration::ZERO) {
            Ok(guard) => guard,
            Err(e) => {
                tracing::debug!(error = %e, "could not drop the busy timeout to date entries");
                return false;
            }
        };
        let dated = self.hinted(
            BACKFILL_APPLIED_EPOCH,
            BACKFILL_APPLIED_EPOCH_UNHINTED,
            |sql| self.conn.prepare_cached(sql)?.execute([]),
        );
        match dated {
            Ok(n) => {
                tracing::debug!(entries = n, "dated journal entries applied elsewhere");
                true
            }
            Err(e) => {
                tracing::debug!(error = %e, "could not date journal entries now");
                false
            }
        }
    }

    /// Raises the floor of known unreadable entries through the undated
    /// entries above it with an unreadable time, up to (not past) the first
    /// one the dating can date. Asked after a dating or a new warning only.
    fn raise_floor(&self) -> rusqlite::Result<()> {
        let floor = self.undatable_through.get();
        let first: Option<i64> = self.hinted(
            FIRST_DATEABLE_ABOVE_SQL,
            FIRST_DATEABLE_ABOVE_UNHINTED_SQL,
            |sql| {
                self.conn
                    .prepare_cached(sql)?
                    .query_row([floor], |r| r.get(0))
            },
        )?;
        let raised = match first {
            Some(first) => first.saturating_sub(1),
            None => self
                .hinted(LAST_UNDATED_SQL, LAST_UNDATED_UNHINTED_SQL, |sql| {
                    self.conn
                        .prepare_cached(sql)?
                        .query_row([], |r| r.get::<_, Option<i64>>(0))
                })?
                .unwrap_or(floor),
        };
        self.undatable_through.set(floor.max(raised));
        Ok(())
    }

    /// Examines the undated entries above the last examined seq, and warns
    /// when some have an unreadable time; returns the new highest such seq
    /// when it warned. Each entry is examined once: the work is the entries
    /// that landed since the last call.
    fn note_undatable(&self) -> rusqlite::Result<Option<i64>> {
        let from = self.examined_through.get();
        let last: Option<i64> =
            self.hinted(LAST_UNDATED_SQL, LAST_UNDATED_UNHINTED_SQL, |sql| {
                self.conn.prepare_cached(sql)?.query_row([], |r| r.get(0))
            })?;
        let Some(to) = last.filter(|&to| to > from) else {
            return Ok(None);
        };
        let seqs: Vec<i64> = self.hinted(
            UNDATABLE_BETWEEN_SQL,
            UNDATABLE_BETWEEN_UNHINTED_SQL,
            |sql| {
                self.conn
                    .prepare_cached(sql)?
                    .query_map([from, to], |r| r.get(0))?
                    .collect()
            },
        )?;
        self.examined_through.set(to);
        let Some(&highest) = seqs.last() else {
            return Ok(None);
        };
        warn_undatable(self.path, &seqs);
        Ok(Some(highest))
    }

    /// Runs `f` in one `BEGIN IMMEDIATE` transaction, committed when `f` succeeds.
    fn write<T>(
        &mut self,
        what: &str,
        f: impl FnOnce(&Transaction<'_>, Step<'_>) -> Result<T, CacheError>,
    ) -> Result<T, CacheError> {
        write_on(self.conn, step(self.path, what), f)
    }

    /// [`SqliteStore::write`] waiting at most `wait` for a write lock held
    /// elsewhere, never more than [`BUSY_TIMEOUT`] (lead ruling S33): the busy
    /// timeout is `min(BUSY_TIMEOUT, wait)` for this one transaction
    /// ([`BusyWait`]) and back to [`BUSY_TIMEOUT`] after it. A lock held past
    /// `wait` is the usual `STORE_FAILED` (SQLite's `database is locked`).
    fn write_within<T>(
        &mut self,
        wait: Duration,
        what: &str,
        f: impl FnOnce(&Transaction<'_>, Step<'_>) -> Result<T, CacheError>,
    ) -> Result<T, CacheError> {
        let at = step(self.path, what);
        let mut conn = BusyWait::new(&mut *self.conn, self.path, wait.min(BUSY_TIMEOUT))
            .at(step(self.path, "set the busy timeout"))?;
        write_on(&mut conn, at, f)
    }

    /// Stores one row inside `tx` (see the `CacheStore` rules).
    fn store_row(
        tx: &Transaction<'_>,
        at: Step<'_>,
        s: Option<&Scope>,
        row: &Bead,
        force: bool,
    ) -> Result<Stored, CacheError> {
        let (stored, row_json) = encode_row(at, row)?;
        Self::store_encoded(tx, at, s, row, &stored, &row_json, force)
    }

    /// [`Session::store_row`] of a row already encoded ([`encode_row`]).
    fn store_encoded(
        tx: &Transaction<'_>,
        at: Step<'_>,
        s: Option<&Scope>,
        row: &Bead,
        stored: &Bead,
        row_json: &str,
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
            // Written by another v3 writer: its membership is its v3 phase
            // (ADR-STORE-0002), a phase-a scope's only (ADR-BDC-0013).
            Some((_, _, None, phase)) => {
                tx.prepare_cached("DELETE FROM scope_member WHERE bead = ?1")
                    .at(at)?
                    .execute([id])
                    .at(at)?;
                if let Some(phase) = phase {
                    tx.prepare_cached(
                        "INSERT OR IGNORE INTO scope_member(scope, bead) SELECT ?1, ?2 \
                         WHERE NOT EXISTS (SELECT 1 FROM scope s WHERE s.name = ?1 \
                         AND s.filter IS NOT NULL)",
                    )
                    .at(at)?
                    .execute([phase.as_str(), id])
                    .at(at)?;
                }
            }
            Some(_) => {}
        }
        let phase = phase_of(stored);
        tx.prepare_cached(concat!(
            "INSERT OR REPLACE INTO beads(id, phase, status, updated_at, row_json, synced_at, \
             stale, membership_known) VALUES (?1, ?2, ?3, ?4, ?5, ",
            synced_at!(),
            ", 0, 1)"
        ))
        .at(at)?
        .execute(params![
            id,
            phase.as_ref().map(ScopeId::name),
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
                .execute([s.id.name(), id])
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

    /// The first `limit` entries matching `filter`, by seq.
    fn entries_first(
        &self,
        at: Step<'_>,
        filter: &str,
        limit: usize,
    ) -> Result<Vec<JournalEntry>, CacheError> {
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let rows = self
            .conn
            .prepare_cached(&format!(
                "SELECT {JOURNAL_COLUMNS} FROM journal WHERE {filter} ORDER BY seq LIMIT ?1"
            ))
            .and_then(|mut stmt| Self::journal_rows(&mut stmt, [limit]))
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
            .prepare_cached(concat!("SELECT COUNT(*) FROM beads b WHERE ", in_scope!()))
            .at(at)?
            .query_row([s.name()], |r| r.get(0))
            .at(at)?;
        Ok(count(n))
    }
}

/// Sets the connection's busy timeout to `wait` while it lives, and back to
/// [`BUSY_TIMEOUT`] when dropped, also on unwind (QA5-003, lead ruling S33).
/// The read-path dating holds it at zero, so it never waits for the write
/// lock; a bounded write ([`SqliteStore::write_within`]) holds it at the time
/// the caller has left. Every later write waits as before.
///
/// `C` is `&Connection` for a guard over a read (the dating runs from
/// `&self`) and `&mut Connection` for one a transaction is begun on.
struct BusyWait<'p, C: Deref<Target = Connection>> {
    conn: C,
    path: &'p Path,
}

impl<'p, C: Deref<Target = Connection>> BusyWait<'p, C> {
    fn new(conn: C, path: &'p Path, wait: Duration) -> rusqlite::Result<Self> {
        conn.busy_timeout(wait)?;
        Ok(Self { conn, path })
    }
}

impl<C: Deref<Target = Connection>> Deref for BusyWait<'_, C> {
    type Target = Connection;

    fn deref(&self) -> &Connection {
        &self.conn
    }
}

impl<C: DerefMut<Target = Connection>> DerefMut for BusyWait<'_, C> {
    fn deref_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

impl<C: Deref<Target = Connection>> Drop for BusyWait<'_, C> {
    fn drop(&mut self) {
        if let Err(e) = self.conn.busy_timeout(BUSY_TIMEOUT) {
            tracing::warn!(
                path = %self.path.display(),
                error = %e,
                "could not restore the busy timeout"
            );
        }
    }
}

/// `BEGIN IMMEDIATE`: writers in other processes wait, up to the busy timeout.
fn begin<'c>(conn: &'c mut Connection, at: Step<'_>) -> Result<Transaction<'c>, CacheError> {
    conn.transaction_with_behavior(TransactionBehavior::Immediate)
        .at(at)
}

/// Runs `f` in one [`begin`] transaction on `conn`, committed when `f` succeeds.
fn write_on<T>(
    conn: &mut Connection,
    at: Step<'_>,
    f: impl FnOnce(&Transaction<'_>, Step<'_>) -> Result<T, CacheError>,
) -> Result<T, CacheError> {
    let tx = begin(conn, at)?;
    let out = f(&tx, at)?;
    tx.commit().at(at)?;
    Ok(out)
}

/// Schema, version and identity checks of [`SqliteStore::open_at`], in one
/// transaction. Returns the highest seq of an applied entry the backfill
/// could not date (warned about), or 0.
fn initialize(
    conn: &mut Connection,
    path: &Path,
    identity: &DatabaseIdentity,
) -> Result<i64, CacheError> {
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
    // The schedule rows live in the cache file (ADR-BDC-0014): sc-cron's
    // version-2 table, and the version of its columns as a meta row, written
    // only when absent so a file holding another version keeps it (and
    // `CacheScheduleStore` refuses it).
    tx.execute_batch(&sc_cron_schedule_table())
        .at(at("create the schedule table"))?;
    tx.execute(
        "INSERT OR IGNORE INTO meta VALUES (?1, ?2)",
        params![
            SC_CRON_SCHEDULE_VERSION_KEY,
            SC_CRON_SCHEDULE_VERSION.to_string()
        ],
    )
    .at(at("record the schedule table version"))?;
    tx.execute_batch(BACKFILL_APPLIED_EPOCH)
        .at(at("date the entries the v3 Python writer applied"))?;
    let undatable = undatable_seqs(&tx).at(at("find undatable journal entries"))?;
    warn_undatable(path, &undatable);
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
    tx.commit().at(at("initialize"))?;
    Ok(undatable.last().copied().unwrap_or_default())
}

/// Warns about applied entries the store could not date, `seqs` by seq: their
/// time is unreadable, so they count as applied since every instant and keep
/// `get` of their scope on the adjusted path until an operator resolves or
/// deletes them (ADR-STORE-0004). They still count: correctness comes first.
/// Called by the open for every such entry, and by the read path for the new
/// ones (QA5-002).
fn warn_undatable(path: &Path, seqs: &[i64]) {
    if let Some(&highest) = seqs.last() {
        tracing::warn!(
            path = %path.display(),
            count = seqs.len(),
            highest,
            seqs = ?seqs.iter().take(UNDATED_SEQS_LOGGED).collect::<Vec<_>>(),
            "applied journal entries have an unreadable time and count as applied since \
             every instant, so ready answers of their scope are always adjusted; resolve \
             or delete them (ADR-STORE-0004)"
        );
    }
}

/// The seqs of applied entries still without an epoch, by seq.
fn undatable_seqs(conn: &Connection) -> rusqlite::Result<Vec<i64>> {
    conn.prepare(UNDATED_SEQS)?
        .query_map([], |r| r.get(0))?
        .collect()
}

/// Prunes a scope inside `tx` (the `CacheStore` rules): refused while it has
/// open writes; deletes the beads no other scope holds, its memberships,
/// journal entries, sync state and ready base, and sweeps out-of-scope rows.
fn prune_in(tx: &Transaction<'_>, at: Step<'_>, s: &ScopeId) -> Result<PruneReport, CacheError> {
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
            detail: format!("scope {} has {open} pending or escalated writes", s.name()),
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
    for id in &held {
        let keep: bool = tx
            .prepare_cached(concat!(
                "SELECT EXISTS (SELECT 1 FROM scope_member WHERE bead = ?1) OR ",
                has_open_entry!()
            ))
            .at(at)?
            .query_row([id], |r| r.get(0))
            .at(at)?;
        if !keep {
            delete_bead(tx, at, id)?;
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
        delete_bead(tx, at, id)?;
        report.out_of_scope_swept = report.out_of_scope_swept.saturating_add(1);
    }
    Ok(report)
}

/// Writes a scope's ready base inside `tx`: the head row and its per-bead index.
fn write_ready_base(
    tx: &Transaction<'_>,
    st: Step<'_>,
    s: &ScopeId,
    blocked_json: &str,
    ready_json: &str,
    index: &[(BeadId, String)],
    at: &Timestamp,
) -> Result<(), CacheError> {
    tx.prepare_cached(
        "INSERT OR REPLACE INTO ready_base(scope, blocked_json, ready_json, fetched_at, \
         stale, indexed) VALUES (?1, ?2, ?3, ?4, 0, 1)",
    )
    .at(st)?
    .execute([s.name(), blocked_json, ready_json, at.as_str()])
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
    for (bead, json) in index {
        insert
            .execute([s.name(), bead.as_str(), json.as_str()])
            .at(st)?;
    }
    Ok(())
}

/// The registered scopes whose stored membership holds bead `?1`, by name:
/// the `scope_member` rows of a bead this store wrote, else the v3 `phase`
/// of a row another writer left, as a phase-a scope ([`in_scope!`]'s rule),
/// both through an index.
const SCOPES_OF_SQL: &str = "SELECT s.name, s.kind, s.filter FROM scope_member m \
    JOIN beads b ON b.id = m.bead JOIN scope s ON s.name = m.scope \
    WHERE m.bead = ?1 AND b.membership_known IS NOT NULL \
    UNION SELECT s.name, s.kind, s.filter FROM beads b JOIN scope s ON s.name = b.phase \
    WHERE b.id = ?1 AND b.membership_known IS NULL AND s.filter IS NULL \
    ORDER BY 1";

/// The registered scope of a name, read on `conn`.
fn registered(conn: &Connection, at: Step<'_>, s: &ScopeId) -> Result<Option<Scope>, CacheError> {
    let row: Option<(String, Option<String>)> = conn
        .prepare_cached("SELECT kind, filter FROM scope WHERE name = ?1")
        .at(at)?
        .query_row([s.name()], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()
        .at(at)?;
    row.map(|(kind, filter)| decode_scope(at, s.name(), &kind, filter.as_deref()))
        .transpose()
}

/// `SCOPE_UNKNOWN` for an unregistered name, listing (some of) the
/// registered ones ([`CacheError::scope_unknown`]).
fn unknown_scope(conn: &Connection, at: Step<'_>, s: &ScopeId) -> Result<CacheError, CacheError> {
    let mut stmt = conn
        .prepare_cached("SELECT name FROM scope ORDER BY name")
        .at(at)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).at(at)?;
    let mut known = Vec::new();
    for r in rows {
        known.push(scope(at, &r.at(at)?)?);
    }
    Ok(CacheError::scope_unknown(
        format!("scope {s} is not registered"),
        known,
    ))
}

/// How a registration treats a name that is (or is not) registered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Register {
    /// `ensure_scope`: register when absent; a registered scope is kept.
    IfAbsent,
    /// `put_scope`: add or set.
    Put,
    /// `scope_add`: refuse a registered name.
    Add,
    /// `scope_set`: refuse an absent name.
    Set,
}

/// The one registration body of the four registry calls, inside `tx`:
/// returns the scope registered afterwards (the `CacheStore` rules).
fn register(
    tx: &Transaction<'_>,
    at: Step<'_>,
    s: &Scope,
    mode: Register,
) -> Result<Scope, CacheError> {
    let known = registered(tx, at, &s.id)?;
    match (&known, mode) {
        (Some(k), Register::IfAbsent) => return Ok(k.clone()),
        (Some(_), Register::Add) => {
            return Err(CacheError::BadArgs {
                detail: format!("scope {} is already registered; use scopes set", s.id),
            });
        }
        (None, Register::Set) => return Err(unknown_scope(tx, at, &s.id)?),
        (None, _) => {
            let n: i64 = tx
                .prepare_cached("SELECT COUNT(*) FROM scope")
                .at(at)?
                .query_row([], |r| r.get(0))
                .at(at)?;
            if usize::try_from(n).unwrap_or(usize::MAX) >= MAX_REGISTERED_SCOPES {
                return Err(CacheError::registry_full(&s.id));
            }
        }
        (Some(_), Register::Put | Register::Set) => {}
    }
    write_scope(tx, at, s, known.as_ref())?;
    Ok(s.clone())
}

/// Creates or updates a scope inside `tx`. A changed filter drops what the
/// old one derived: the scope's memberships, sync state and ready base; the
/// rows stay (the `CacheStore` rules).
fn write_scope(
    tx: &Transaction<'_>,
    at: Step<'_>,
    s: &Scope,
    before: Option<&Scope>,
) -> Result<(), CacheError> {
    let filter = encode_scope(at, s)?;
    tx.prepare_cached(
        "INSERT INTO scope(name, kind, filter) VALUES (?1, ?2, ?3) \
         ON CONFLICT(name) DO UPDATE SET filter = excluded.filter",
    )
    .at(at)?
    .execute(params![s.id.name(), KIND_PHASE, filter])
    .at(at)?;
    if before.is_some_and(|b| b.filter != s.filter) {
        for sql in [
            "DELETE FROM scope_member WHERE scope = ?1",
            "DELETE FROM sync_state WHERE phase = ?1",
            "DELETE FROM ready_base WHERE scope = ?1",
            "DELETE FROM ready_base_blocked WHERE scope = ?1",
        ] {
            tx.execute(sql, [s.id.name()]).at(at)?;
        }
    }
    Ok(())
}

/// The `tick_lease` columns a read decodes: `run_id`, `pid`, `host`, `expires_at`.
type LeaseColumns = (Option<String>, Option<i64>, Option<String>, Option<String>);

/// The `hook_state` columns a read decodes.
type HookColumns = (i64, Option<String>, Option<String>, Option<String>);

/// The `sync_run` columns a read decodes: `run_id`, `started_at`,
/// `finished_at`, `run_end`, `consecutive_failures`, `first_failure_at`.
type SyncRunColumns = (
    String,
    String,
    Option<String>,
    Option<String>,
    i64,
    Option<String>,
);

/// One sync row's run record (ADR-STORE-0006 as amended by sprint b-8);
/// `None` before its first run.
fn read_sync_run(
    conn: &Connection,
    at: Step<'_>,
    row: &str,
) -> Result<Option<SyncRun>, CacheError> {
    let found: Option<SyncRunColumns> = conn
        .prepare_cached(
            "SELECT run_id, started_at, finished_at, run_end, consecutive_failures, \
             first_failure_at FROM sync_run WHERE row = ?1",
        )
        .at(at)?
        .query_row([row], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .optional()
        .at(at)?;
    let Some((run_id, started, finished, end, failures, first)) = found else {
        return Ok(None);
    };
    let end = end
        .map(|e| {
            SyncRunEnd::parse(&e).ok_or_else(|| corrupt(at, format_args!("sync_run run_end {e:?}")))
        })
        .transpose()?;
    Ok(Some(SyncRun {
        run_id: RunId::new(run_id)
            .map_err(|e| corrupt(at, format_args!("sync_run run_id: {e}")))?,
        started_at: timestamp(at, "sync_run started_at", &started)?,
        finished_at: opt_timestamp(at, "sync_run finished_at", finished.as_deref())?,
        end,
        consecutive_failures: u32::try_from(failures)
            .map_err(|e| corrupt(at, format_args!("sync_run consecutive_failures: {e}")))?,
        first_failure_at: opt_timestamp(at, "sync_run first_failure_at", first.as_deref())?,
    }))
}

/// Writes one sync row's run record inside `tx`.
fn write_sync_run(
    tx: &Transaction<'_>,
    at: Step<'_>,
    row: &str,
    run: &SyncRun,
) -> Result<(), CacheError> {
    tx.prepare_cached(
        "INSERT INTO sync_run(row, run_id, started_at, finished_at, run_end, \
         consecutive_failures, first_failure_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
         ON CONFLICT(row) DO UPDATE SET run_id = excluded.run_id, \
         started_at = excluded.started_at, finished_at = excluded.finished_at, \
         run_end = excluded.run_end, consecutive_failures = excluded.consecutive_failures, \
         first_failure_at = excluded.first_failure_at",
    )
    .at(at)?
    .execute(params![
        row,
        run.run_id.as_str(),
        run.started_at.as_str(),
        run.finished_at.as_ref().map(Timestamp::as_str),
        run.end.map(SyncRunEnd::as_str),
        i64::from(run.consecutive_failures),
        run.first_failure_at.as_ref().map(Timestamp::as_str),
    ])
    .at(at)?;
    Ok(())
}

/// The lease row, live or expired; `Corrupt` for a row that does not decode
/// (a NULL column, a bad time): `lease_take` replaces such a row.
fn read_lease(conn: &Connection, at: Step<'_>) -> Result<Option<LeaseHolder>, CacheError> {
    let row: Option<LeaseColumns> = conn
        .prepare_cached("SELECT run_id, pid, host, expires_at FROM tick_lease WHERE id = 1")
        .at(at)?
        .query_row([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()
        .at(at)?;
    let Some((run_id, pid, host, expires_at)) = row else {
        return Ok(None);
    };
    let (Some(run_id), Some(pid), Some(host), Some(expires_at)) = (run_id, pid, host, expires_at)
    else {
        return Err(corrupt(at, "tick_lease row has a NULL column"));
    };
    Ok(Some(LeaseHolder {
        run_id: RunId::new(run_id)
            .map_err(|e| corrupt(at, format_args!("tick_lease run_id: {e}")))?,
        pid: u32::try_from(pid).map_err(|e| corrupt(at, format_args!("tick_lease pid: {e}")))?,
        host,
        expires_at: timestamp(at, "tick_lease expires_at", &expires_at)?,
    }))
}

/// A scope's sync state, read on `conn` (all `None` before its first sync).
fn read_sync_state(conn: &Connection, at: Step<'_>, s: &ScopeId) -> Result<SyncState, CacheError> {
    let found: Option<SyncColumns> = conn
        .prepare_cached(
            "SELECT last_started_at, last_ok_at, last_error, failing_since, notified_since \
             FROM sync_state WHERE phase = ?1",
        )
        .at(at)?
        .query_row([s.name()], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .optional()
        .at(at)?;
    let Some((started, ok, last_error, failing, notified)) = found else {
        return Ok(SyncState::new(s.clone()));
    };
    let token = |what: &str, v: Option<String>| {
        v.map(FailingSince::parse)
            .transpose()
            .map_err(|e| corrupt(at, format_args!("stored {what}: {e}")))
    };
    Ok(SyncState {
        scope: s.clone(),
        last_started_at: opt_timestamp(at, "last_started_at", started.as_deref())?,
        last_ok_at: opt_timestamp(at, "last_ok_at", ok.as_deref())?,
        last_error,
        failing_since: token("failing_since", failing)?,
        notified_since: token("notified_since", notified)?,
    })
}

/// Writes a scope's sync state inside `tx`, every column but
/// `notified_since`, which only `scope_announce` sets (a new row has none).
fn write_sync_state(tx: &Transaction<'_>, at: Step<'_>, st: &SyncState) -> Result<(), CacheError> {
    tx.prepare_cached(
        "INSERT INTO sync_state(phase, last_started_at, last_ok_at, last_error, \
         failing_since, notified_since) VALUES (?1, ?2, ?3, ?4, ?5, NULL) \
         ON CONFLICT(phase) DO UPDATE SET \
         last_started_at = excluded.last_started_at, last_ok_at = excluded.last_ok_at, \
         last_error = excluded.last_error, failing_since = excluded.failing_since",
    )
    .at(at)?
    .execute(params![
        st.scope.name(),
        st.last_started_at.as_ref().map(Timestamp::as_str),
        st.last_ok_at.as_ref().map(Timestamp::as_str),
        st.last_error,
        st.failing_since.as_ref().map(FailingSince::as_str),
    ])
    .at(at)?;
    Ok(())
}

/// The hook backoff row; the default when there is none.
fn read_hook_state(conn: &Connection, at: Step<'_>) -> Result<HookState, CacheError> {
    let row: Option<HookColumns> = conn
        .prepare_cached(
            "SELECT consecutive_failures, first_failure_at, last_error, next_attempt_at \
             FROM hook_state WHERE id = 1",
        )
        .at(at)?
        .query_row([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()
        .at(at)?;
    let Some((failures, first, last_error, next)) = row else {
        return Ok(HookState::default());
    };
    Ok(HookState {
        consecutive_failures: u32::try_from(failures)
            .map_err(|e| corrupt(at, format_args!("hook_state consecutive_failures: {e}")))?,
        first_failure_at: opt_timestamp(at, "hook_state first_failure_at", first.as_deref())?,
        last_error,
        next_attempt_at: opt_timestamp(at, "hook_state next_attempt_at", next.as_deref())?,
    })
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

impl Session<'_> {
    fn register(&mut self, s: &Scope, mode: Register, what: &str) -> Result<Scope, CacheError> {
        self.write(what, |tx, at| register(tx, at, s, mode))
    }

    fn ensure_scope(&mut self, s: &Scope) -> Result<Scope, CacheError> {
        self.register(s, Register::IfAbsent, "ensure scope")
    }

    fn put_scope(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.register(s, Register::Put, "write a scope").map(drop)
    }

    fn scope_add(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.register(s, Register::Add, "add a scope").map(drop)
    }

    fn scope_set(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.register(s, Register::Set, "set a scope").map(drop)
    }

    /// The one body of `prune_scope` and `scope_remove`: a prune, then, for
    /// a removal, the registry row, in one transaction.
    fn prune(&mut self, s: &ScopeId, remove: bool) -> Result<PruneReport, CacheError> {
        let what = if remove {
            "remove a scope"
        } else {
            "prune a scope"
        };
        self.write(what, |tx, at| {
            if remove && registered(tx, at, s)?.is_none() {
                return Err(unknown_scope(tx, at, s)?);
            }
            let report = prune_in(tx, at, s)?;
            if remove {
                tx.execute("DELETE FROM scope WHERE name = ?1", [s.name()])
                    .at(at)?;
            }
            Ok(report)
        })
    }

    fn scope_remove(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        self.prune(s, true)
    }

    fn scope(&self, s: &ScopeId) -> Result<Option<Scope>, CacheError> {
        registered(self.conn, self.at("read a scope"), s)
    }

    fn scopes(&self) -> Result<Vec<Scope>, CacheError> {
        let at = self.at("read scopes");
        let mut stmt = self
            .conn
            .prepare_cached("SELECT name, kind, filter FROM scope ORDER BY name")
            .at(at)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .at(at)?;
        let mut out = Vec::new();
        for r in rows {
            let (name, kind, filter) = r.at(at)?;
            out.push(decode_scope(at, &name, &kind, filter.as_deref())?);
        }
        Ok(out)
    }

    fn scope_revision(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        let at = self.at("read a scope revision");
        Ok(self
            .conn
            .prepare_cached("SELECT revision FROM scope WHERE name = ?1")
            .at(at)?
            .query_row([s.name()], |r| r.get::<_, Option<String>>(0))
            .optional()
            .at(at)?
            .flatten())
    }

    fn members(&self, s: &ScopeId) -> Result<BTreeSet<BeadId>, CacheError> {
        let at = self.at("read a scope's members");
        let mut stmt = self
            .conn
            .prepare_cached(concat!("SELECT b.id FROM beads b WHERE ", in_scope!()))
            .at(at)?;
        let rows = stmt
            .query_map([s.name()], |r| r.get::<_, String>(0))
            .at(at)?;
        let mut out = BTreeSet::new();
        for r in rows {
            out.insert(bead_id(at, &r.at(at)?)?);
        }
        Ok(out)
    }

    fn scopes_of(&self, id: &BeadId) -> Result<Vec<Scope>, CacheError> {
        let at = self.at("read a bead's scopes");
        let mut stmt = self.conn.prepare_cached(SCOPES_OF_SQL).at(at)?;
        let rows = stmt
            .query_map([id.as_str()], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                ))
            })
            .at(at)?;
        let mut out = Vec::new();
        for r in rows {
            let (name, kind, filter) = r.at(at)?;
            out.push(decode_scope(at, &name, &kind, filter.as_deref())?);
        }
        Ok(out)
    }

    /// The snapshot's limits are its own ([`IngestSnapshot`]): the store
    /// trusts it. The scope is read, and refused when unregistered, inside
    /// the write transaction, so another process's `scopes remove` or
    /// `scopes set` between the core's call and this one is seen: an ingest
    /// never re-registers a scope or applies a stale status criterion.
    ///
    /// The transaction holds the connection's mutex and the file's write lock
    /// for the whole snapshot: up to [`MAX_INGEST_ROWS`](bd_cache_model::MAX_INGEST_ROWS) row upserts, their
    /// memberships and the ready base. `tests/phase_b_store.rs` measures a
    /// 50 000-row ingest: about 1.3 s on a debug build (macOS, Apple silicon,
    /// 2026), and it asserts a debug build stays under [`BUSY_TIMEOUT`] and a
    /// release build under half of it. That is the most other writers
    /// (another process, `CacheScheduleStore` on this connection) wait, so the
    /// default busy wait suffices and no longer one is set for this call.
    fn ingest_scope(
        &mut self,
        s: &ScopeId,
        snapshot: &IngestSnapshot,
        ready: &[BeadId],
        at: &Timestamp,
        revision: Option<&str>,
    ) -> Result<UpsertReport, CacheError> {
        let st = step(self.path, "ingest a scope");
        let rows = snapshot.rows();
        let encoded = rows
            .iter()
            .map(|row| encode_row(st, row))
            .collect::<Result<Vec<_>, _>>()?;
        let tx = begin(self.conn, st)?;
        let Some(s) = registered(&tx, st, s)? else {
            return Err(unknown_scope(&tx, st, s)?);
        };
        let (blocked_json, ready_json, index) = {
            let members: BTreeSet<&BeadId> =
                rows.iter().filter(|r| s.admits(r)).map(|r| &r.id).collect();
            let blocked: Vec<BlockedRow> = snapshot
                .blocked()
                .iter()
                .filter(|b| members.contains(&b.id))
                .cloned()
                .collect();
            let ready: Vec<BeadId> = ready
                .iter()
                .filter(|id| members.contains(id))
                .cloned()
                .collect();
            let (b, r) = encode_ready(st, &blocked, &ready)?;
            (b, r, index_blocked(st, &blocked)?)
        };
        tx.prepare_cached("DELETE FROM scope_member WHERE scope = ?1")
            .at(st)?
            .execute([s.id.name()])
            .at(st)?;
        let mut report = UpsertReport::default();
        for (row, (stored, json)) in rows.iter().zip(&encoded) {
            match Self::store_encoded(&tx, st, None, row, stored, json, false)? {
                Stored::SkippedNewer => report.skipped_newer.push(row.id.clone()),
                Stored::Yes { status_changed } => {
                    if status_changed {
                        report.status_changed.push(row.id.clone());
                    }
                    report.stored.push(row.id.clone());
                }
            }
            if s.admits(row) {
                tx.prepare_cached(
                    "INSERT OR IGNORE INTO scope_member(scope, bead) VALUES (?1, ?2)",
                )
                .at(st)?
                .execute([s.id.name(), row.id.as_str()])
                .at(st)?;
            }
        }
        write_ready_base(&tx, st, &s.id, &blocked_json, &ready_json, &index, at)?;
        tx.prepare_cached(
            "INSERT INTO sync_state(phase, last_started_at, last_ok_at, last_error, failing_since, \
             notified_since) VALUES (?1, ?2, ?2, NULL, NULL, NULL) ON CONFLICT(phase) DO UPDATE SET \
             last_started_at = ?2, last_ok_at = ?2, last_error = NULL, failing_since = NULL, \
             notified_since = NULL",
        )
        .at(st)?
        .execute([s.id.name(), at.as_str()])
        .at(st)?;
        tx.prepare_cached("UPDATE scope SET revision = ?2 WHERE name = ?1")
            .at(st)?
            .execute(params![s.id.name(), revision])
            .at(st)?;
        tx.commit().at(st)?;
        Ok(report)
    }

    fn scope_announce(&mut self, s: &ScopeId, since: &FailingSince) -> Result<(), CacheError> {
        self.write("announce a scope's failure", |tx, at| {
            let marked = tx
                .prepare_cached(
                    "UPDATE sync_state SET notified_since = failing_since \
                     WHERE phase = ?1 AND failing_since = ?2",
                )
                .at(at)?
                .execute([s.name(), since.as_str()])
                .at(at)?;
            if marked == 1 {
                return Ok(());
            }
            let current: Option<String> = tx
                .prepare_cached("SELECT failing_since FROM sync_state WHERE phase = ?1")
                .at(at)?
                .query_row([s.name()], |r| r.get(0))
                .optional()
                .at(at)?
                .flatten();
            Err(CacheError::Precondition {
                detail: format!(
                    "scope {s}: since {since} is not its current failing_since ({})",
                    current.as_deref().unwrap_or("none")
                ),
                unmet: BTreeMap::new(),
                reason: PreconditionReason::PreconditionFailed,
                source: None,
            })
        })
    }

    fn mark_offered(&mut self, seqs: &[Seq], at: &Timestamp) -> Result<(), CacheError> {
        self.write("mark entries offered", |tx, st| {
            for seq in seqs {
                let exists = tx
                    .prepare_cached("SELECT 1 FROM journal WHERE seq = ?1")
                    .at(st)?
                    .exists([seq.get()])
                    .at(st)?;
                if !exists {
                    return Err(not_found(*seq));
                }
                tx.prepare_cached(
                    "UPDATE journal SET first_offered_at = ?2 \
                     WHERE seq = ?1 AND first_offered_at IS NULL",
                )
                .at(st)?
                .execute(params![seq.get(), at.as_str()])
                .at(st)?;
            }
            Ok(())
        })
    }

    fn lease_take(
        &mut self,
        holder: &LeaseHolder,
        now: &Timestamp,
    ) -> Result<LeaseTake, CacheError> {
        check_lease_expiry(&holder.expires_at, now)?;
        self.write("take the sync lease", |tx, at| {
            let took = match read_lease(tx, at) {
                Ok(None) => LeaseTake::Taken,
                Ok(Some(live)) if live.expires_at.unix() > now.unix() => {
                    return Ok(LeaseTake::Held(live));
                }
                Ok(Some(expired)) => LeaseTake::TookOver(expired),
                // A row that does not decode holds no one: it is replaced,
                // so it cannot refuse every later run (B7Q1-048).
                Err(e) => {
                    tracing::warn!(
                        step = %at,
                        error = %e,
                        "tick_lease row does not decode; it is treated as expired and replaced"
                    );
                    LeaseTake::Taken
                }
            };
            tx.prepare_cached(
                "INSERT OR REPLACE INTO tick_lease(id, run_id, pid, host, expires_at) \
                 VALUES (1, ?1, ?2, ?3, ?4)",
            )
            .at(at)?
            .execute(params![
                holder.run_id.as_str(),
                holder.pid,
                holder.host,
                holder.expires_at.as_str()
            ])
            .at(at)?;
            Ok(took)
        })
    }

    fn lease_release(&mut self, run_id: &RunId) -> Result<bool, CacheError> {
        self.write("release the sync lease", |tx, at| {
            let n = tx
                .prepare_cached("DELETE FROM tick_lease WHERE id = 1 AND run_id = ?1")
                .at(at)?
                .execute([run_id.as_str()])
                .at(at)?;
            Ok(n == 1)
        })
    }

    fn lease(&self) -> Result<Option<LeaseHolder>, CacheError> {
        read_lease(self.conn, self.at("read the sync lease"))
    }

    fn hook_state(&self) -> Result<HookState, CacheError> {
        read_hook_state(self.conn, self.at("read the hook state"))
    }

    fn hook_failed(
        &mut self,
        err: &str,
        at: &Timestamp,
        next_attempt_at: Option<&Timestamp>,
    ) -> Result<HookState, CacheError> {
        self.write("count a hook failure", |tx, st| {
            tx.prepare_cached(
                "INSERT INTO hook_state(id, consecutive_failures, first_failure_at, last_error, \
                 next_attempt_at) VALUES (1, 1, ?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET \
                 consecutive_failures = consecutive_failures + 1, \
                 first_failure_at = COALESCE(first_failure_at, ?1), last_error = ?2, \
                 next_attempt_at = ?3",
            )
            .at(st)?
            .execute(params![
                at.as_str(),
                clip_hook_error(err),
                next_attempt_at.map(Timestamp::as_str)
            ])
            .at(st)?;
            read_hook_state(tx, st)
        })
    }

    fn hook_succeeded(&mut self) -> Result<(), CacheError> {
        self.write("reset the hook state", |tx, at| {
            tx.prepare_cached("DELETE FROM hook_state WHERE id = 1")
                .at(at)?
                .execute([])
                .at(at)?;
            Ok(())
        })
    }

    fn sync_run(&self, row: &str) -> Result<Option<SyncRun>, CacheError> {
        read_sync_run(self.conn, self.at("read a sync row's run record"), row)
    }

    fn sync_run_started(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
    ) -> Result<SyncRun, CacheError> {
        self.write("record a sync run's start", |tx, st| {
            let previous = read_sync_run(tx, st, row)?;
            let record = SyncRun::started(previous.as_ref(), run_id, at);
            write_sync_run(tx, st, row, &record)?;
            Ok(record)
        })
    }

    fn sync_run_finished(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
        end: SyncRunEnd,
    ) -> Result<bool, CacheError> {
        self.write("record a sync run's end", |tx, st| {
            match read_sync_run(tx, st, row)? {
                Some(record) if record.run_id == *run_id => {
                    write_sync_run(tx, st, row, &record.finished(at, end))?;
                    Ok(true)
                }
                _ => Ok(false),
            }
        })
    }

    fn upsert_rows(
        &mut self,
        s: Option<&Scope>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError> {
        let at = step(self.path, "upsert rows");
        let tx = begin(self.conn, at)?;
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
            write_ready_base(tx, st, s, &blocked_json, &ready_json, &index, at)
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

    fn insert_entry(&mut self, e: &NewEntry, at: &Timestamp) -> Result<Seq, CacheError> {
        let st = step(self.path, "journal a write");
        let tx = begin(self.conn, st)?;
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
             state, created_at, updated_at, changed) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'pending', ?8, ?8, ?9)",
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
            e.changed,
        ])
        .at(st)?;
        // The entry and its `changed` are one row of one statement, committed
        // once: no reader sees the entry without it (REQ-BDC-0809).
        let seq = Seq::new(tx.last_insert_rowid());
        tx.commit().at(st)?;
        Ok(seq)
    }

    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.entries(self.at("read pending entries"), "state = 'pending'", [])
    }

    fn pending_summary(&self) -> Result<PendingSummary, CacheError> {
        let at = self.at("summarize pending entries");
        let n: i64 = self
            .conn
            .prepare_cached("SELECT COUNT(*) FROM journal WHERE state = 'pending'")
            .at(at)?
            .query_row([], |r| r.get(0))
            .at(at)?;
        // The oldest by seq: entries are journaled in time order.
        let oldest = self.entries_first(at, "state = 'pending'", 1)?;
        Ok(PendingSummary {
            count: count(n),
            oldest_created_at: oldest.into_iter().next().map(|e| e.created_at),
        })
    }

    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError> {
        self.entries(
            self.at("read pending entries"),
            "state = 'pending' AND bead = ?1",
            [id.as_str()],
        )
    }

    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError> {
        // An entry's time is `applied_epoch`; for one the v3 Python writer applied
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
        let st = step(self.path, "apply a journal entry");
        let tx = begin(self.conn, st)?;
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
            "UPDATE journal SET state = 'applied', last_error = ?2, last_error_code = ?5, \
             applied_at = ?3, applied_epoch = ?4, updated_at = ?3 WHERE seq = ?1",
        )
        .at(st)?
        .execute(params![
            seq.get(),
            err,
            at.as_str(),
            at.unix(),
            err.and_then(error_code_prefix)
        ])
        .at(st)?;
        tx.commit().at(st)
    }

    fn record_attempt(&mut self, seq: Seq, err: &str, at: &Timestamp) -> Result<u32, CacheError> {
        self.write("record an attempt", |tx, st| {
            let attempts: i64 = tx
                .prepare_cached(
                    "UPDATE journal SET attempts = attempts + 1, last_error = ?2, \
                     last_error_code = ?4, updated_at = ?3 WHERE seq = ?1 RETURNING attempts",
                )
                .at(st)?
                .query_row(
                    params![seq.get(), err, at.as_str(), error_code_prefix(err)],
                    |r| r.get(0),
                )
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
                    "UPDATE journal SET state = 'escalated', last_error = ?2, \
                     last_error_code = ?4, updated_at = ?3 WHERE seq = ?1",
                )
                .at(st)?
                .execute(params![
                    seq.get(),
                    reason,
                    at.as_str(),
                    error_code_prefix(reason)
                ])
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
        let st = step(self.path, "resolve");
        let tx = begin(self.conn, st)?;
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
        // The note has its own column; `last_error` is left as it was. Its 0.1
        // rendering appends the note as the v3 Python writer does (ADR-STORE-0001).
        tx.prepare_cached(
            "UPDATE journal SET state = 'resolved', note = ?2, updated_at = ?3 WHERE seq = ?1",
        )
        .at(st)?
        .execute(params![seq.get(), resolve_note(actor, note), at.as_str()])
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

    fn unannounced_escalations_first(&self, limit: usize) -> Result<EscalationPage, CacheError> {
        let at = self.at("read unannounced escalations");
        let filter = "state = 'escalated' AND announced_at IS NULL";
        let n: i64 = self
            .conn
            .prepare_cached(&format!("SELECT COUNT(*) FROM journal WHERE {filter}"))
            .at(at)?
            .query_row([], |r| r.get(0))
            .at(at)?;
        Ok(EscalationPage {
            entries: self.entries_first(at, filter, limit)?,
            total: count(n),
        })
    }

    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError> {
        self.mark_announced_within(seq, at, BUSY_TIMEOUT)
    }

    fn mark_announced_within(
        &mut self,
        seq: Seq,
        at: &Timestamp,
        wait: Duration,
    ) -> Result<(), CacheError> {
        self.write_within(wait, "mark announced", |tx, st| {
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
        let st = step(self.path, "record the backend fingerprint");
        let tx = begin(self.conn, st)?;
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
        read_sync_state(self.conn, self.at("read sync state"), s)
    }

    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError> {
        self.write("record sync state", |tx, at| write_sync_state(tx, at, st))
    }

    fn record_sync_result(
        &mut self,
        s: &ScopeId,
        outcome: &SyncOutcome,
        started: &Timestamp,
    ) -> Result<SyncState, CacheError> {
        self.write("record a sync result", |tx, at| {
            let mut st = read_sync_state(tx, at, s)?;
            st.last_started_at = Some(started.clone());
            match outcome {
                SyncOutcome::Ok => {
                    st.last_ok_at = Some(started.clone());
                    st.last_error = None;
                    st.record_result(true, started);
                }
                SyncOutcome::Failed(err) => {
                    st.last_error = Some(err.clone());
                    st.record_result(false, started);
                }
                SyncOutcome::Deferred => {}
            }
            write_sync_state(tx, at, &st)?;
            // An ok result clears the mark with the failure; `record_sync`
            // never writes it.
            if st.notified_since.is_none() {
                tx.prepare_cached("UPDATE sync_state SET notified_since = NULL WHERE phase = ?1")
                    .at(at)?
                    .execute([s.name()])
                    .at(at)?;
            }
            Ok(st)
        })
    }

    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.meta(self.at("read a sync alert"), &alert_key(s))
    }

    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        self.set_sync_alert_within(s, cause, BUSY_TIMEOUT)
    }

    fn set_sync_alert_within(
        &mut self,
        s: &ScopeId,
        cause: Option<&str>,
        wait: Duration,
    ) -> Result<(), CacheError> {
        let key = alert_key(s);
        self.write_within(wait, "set a sync alert", |tx, at| {
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
        let at = step(self.path, "set the outage");
        let tx = begin(self.conn, at)?;
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
            // The announcement mark goes with the outage, as in the v3 Python writer.
            None => tx.execute(
                "DELETE FROM meta WHERE key IN (?1, ?2, ?3)",
                [META_OUTAGE_SINCE, META_OUTAGE_ERROR, META_OUTAGE_ANNOUNCED],
            ),
        }
        .at(at)?;
        tx.commit().at(at)
    }

    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        self.prune(s, false)
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

    fn journal_counts(&self) -> Result<JournalCounts, CacheError> {
        let at = self.at("count the journal");
        // One grouped statement: one snapshot, no read transaction needed.
        let mut stmt = self.conn.prepare_cached(JOURNAL_COUNTS_SQL).at(at)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, i64>(2)?,
                ))
            })
            .at(at)?;
        let mut counts = JournalCounts::default();
        for r in rows {
            let (phase, state, n) = r.at(at)?;
            let scope = phase.as_deref().map(|p| scope(at, p)).transpose()?;
            let state = JournalState::parse(&state)
                .ok_or_else(|| corrupt(at, format_args!("journal state {state:?} is unknown")))?;
            counts.add(scope.as_ref(), state, count(n));
        }
        Ok(counts)
    }
}

/// Each port method locks the shared connection for its one call
/// and runs on it.
impl CacheStore for SqliteStore {
    fn ensure_scope(&mut self, s: &Scope) -> Result<Scope, CacheError> {
        self.session(|x| x.ensure_scope(s))
    }
    fn put_scope(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.session(|x| x.put_scope(s))
    }
    fn scope_add(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.session(|x| x.scope_add(s))
    }
    fn scope_set(&mut self, s: &Scope) -> Result<(), CacheError> {
        self.session(|x| x.scope_set(s))
    }
    fn scope_remove(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        self.session(|x| x.scope_remove(s))
    }
    fn scope(&self, s: &ScopeId) -> Result<Option<Scope>, CacheError> {
        self.session(|x| x.scope(s))
    }
    fn scopes(&self) -> Result<Vec<Scope>, CacheError> {
        self.session(|x| x.scopes())
    }
    fn scope_revision(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.session(|x| x.scope_revision(s))
    }
    fn members(&self, s: &ScopeId) -> Result<BTreeSet<BeadId>, CacheError> {
        self.session(|x| x.members(s))
    }
    fn scopes_of(&self, id: &BeadId) -> Result<Vec<Scope>, CacheError> {
        self.session(|x| x.scopes_of(id))
    }
    fn ingest_scope(
        &mut self,
        s: &ScopeId,
        snapshot: &IngestSnapshot,
        ready: &[BeadId],
        at: &Timestamp,
        revision: Option<&str>,
    ) -> Result<UpsertReport, CacheError> {
        self.session(|x| x.ingest_scope(s, snapshot, ready, at, revision))
    }
    fn scope_announce(&mut self, s: &ScopeId, since: &FailingSince) -> Result<(), CacheError> {
        self.session(|x| x.scope_announce(s, since))
    }
    fn mark_offered(&mut self, seqs: &[Seq], at: &Timestamp) -> Result<(), CacheError> {
        self.session(|x| x.mark_offered(seqs, at))
    }
    fn lease_take(
        &mut self,
        holder: &LeaseHolder,
        now: &Timestamp,
    ) -> Result<LeaseTake, CacheError> {
        self.session(|x| x.lease_take(holder, now))
    }
    fn lease_release(&mut self, run_id: &RunId) -> Result<bool, CacheError> {
        self.session(|x| x.lease_release(run_id))
    }
    fn lease(&self) -> Result<Option<LeaseHolder>, CacheError> {
        self.session(|x| x.lease())
    }
    fn hook_state(&self) -> Result<HookState, CacheError> {
        self.session(|x| x.hook_state())
    }
    fn hook_failed(
        &mut self,
        err: &str,
        at: &Timestamp,
        next_attempt_at: Option<&Timestamp>,
    ) -> Result<HookState, CacheError> {
        self.session(|x| x.hook_failed(err, at, next_attempt_at))
    }
    fn hook_succeeded(&mut self) -> Result<(), CacheError> {
        // A closure, not the method path: `session` hands a `Session` of any lifetime.
        #[expect(
            clippy::redundant_closure_for_method_calls,
            reason = "the method path is not general over the session's lifetime"
        )]
        self.session(|x| x.hook_succeeded())
    }
    fn sync_run(&self, row: &str) -> Result<Option<SyncRun>, CacheError> {
        self.session(|x| x.sync_run(row))
    }
    fn sync_run_started(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
    ) -> Result<SyncRun, CacheError> {
        self.session(|x| x.sync_run_started(row, run_id, at))
    }
    fn sync_run_finished(
        &mut self,
        row: &str,
        run_id: &RunId,
        at: &Timestamp,
        end: SyncRunEnd,
    ) -> Result<bool, CacheError> {
        self.session(|x| x.sync_run_finished(row, run_id, at, end))
    }
    fn get_row(&self, id: &BeadId) -> Result<Option<(Bead, Freshness)>, CacheError> {
        self.session(|x| x.get_row(id))
    }
    fn rows_in_scope(&self, s: &ScopeId) -> Result<Vec<(Bead, Freshness)>, CacheError> {
        self.session(|x| x.rows_in_scope(s))
    }
    fn upsert_rows(
        &mut self,
        s: Option<&Scope>,
        rows: &[Bead],
        force: bool,
    ) -> Result<UpsertReport, CacheError> {
        self.session(|x| x.upsert_rows(s, rows, force))
    }
    fn stale_rows(&self) -> Result<Vec<BeadId>, CacheError> {
        self.session(|x| x.stale_rows())
    }
    fn dependents_of(&self, id: &BeadId) -> Result<Vec<Edge>, CacheError> {
        self.session(|x| x.dependents_of(id))
    }
    fn replace_ready_base(
        &mut self,
        s: &ScopeId,
        blocked: &[BlockedRow],
        ready: &[BeadId],
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        self.session(|x| x.replace_ready_base(s, blocked, ready, at))
    }
    fn ready_base(&self, s: &ScopeId) -> Result<Option<ReadyBase>, CacheError> {
        self.session(|x| x.ready_base(s))
    }
    fn base_entry(&self, s: &ScopeId, id: &BeadId) -> Result<Option<BaseEntry>, CacheError> {
        self.session(|x| x.base_entry(s, id))
    }
    fn has_entries_since(&self, at: &Timestamp) -> Result<bool, CacheError> {
        self.session(|x| x.has_entries_since(at))
    }
    fn mark_ready_base_stale(&mut self, s: &ScopeId) -> Result<(), CacheError> {
        self.session(|x| x.mark_ready_base_stale(s))
    }
    fn entry_by_key(&self, k: &IdemKey) -> Result<Option<JournalEntry>, CacheError> {
        self.session(|x| x.entry_by_key(k))
    }
    fn entry_by_seq(&self, seq: Seq) -> Result<Option<JournalEntry>, CacheError> {
        self.session(|x| x.entry_by_seq(seq))
    }
    fn insert_entry(&mut self, e: NewEntry, at: &Timestamp) -> Result<Seq, CacheError> {
        self.session(|x| x.insert_entry(&e, at))
    }
    fn pending_entries(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.session(|x| x.pending_entries())
    }
    fn pending_summary(&self) -> Result<PendingSummary, CacheError> {
        self.session(|x| x.pending_summary())
    }
    fn pending_for(&self, id: &BeadId) -> Result<Vec<JournalEntry>, CacheError> {
        self.session(|x| x.pending_for(id))
    }
    fn applied_since(&self, at: &Timestamp) -> Result<Vec<JournalEntry>, CacheError> {
        self.session(|x| x.applied_since(at))
    }
    fn apply_with_readback(
        &mut self,
        seq: Seq,
        row: Option<&Bead>,
        err: Option<&str>,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        self.session(|x| x.apply_with_readback(seq, row, err, at))
    }
    fn record_attempt(&mut self, seq: Seq, err: &str, at: &Timestamp) -> Result<u32, CacheError> {
        self.session(|x| x.record_attempt(seq, err, at))
    }
    fn escalate(&mut self, seq: Seq, reason: &str, at: &Timestamp) -> Result<(), CacheError> {
        self.session(|x| x.escalate(seq, reason, at))
    }
    fn resolve(
        &mut self,
        seq: Seq,
        note: &str,
        actor: &Actor,
        at: &Timestamp,
    ) -> Result<(), CacheError> {
        self.session(|x| x.resolve(seq, note, actor, at))
    }
    fn unannounced_escalations(&self) -> Result<Vec<JournalEntry>, CacheError> {
        self.session(|x| x.unannounced_escalations())
    }
    fn unannounced_escalations_first(&self, limit: usize) -> Result<EscalationPage, CacheError> {
        self.session(|x| x.unannounced_escalations_first(limit))
    }
    fn mark_announced(&mut self, seq: Seq, at: &Timestamp) -> Result<(), CacheError> {
        self.session(|x| x.mark_announced(seq, at))
    }
    fn mark_announced_within(
        &mut self,
        seq: Seq,
        at: &Timestamp,
        wait: Duration,
    ) -> Result<(), CacheError> {
        self.session(|x| x.mark_announced_within(seq, at, wait))
    }
    fn record_fingerprint(&mut self, f: &Fingerprint) -> Result<FingerprintOutcome, CacheError> {
        self.session(|x| x.record_fingerprint(f))
    }
    fn sync_state(&self, s: &ScopeId) -> Result<SyncState, CacheError> {
        self.session(|x| x.sync_state(s))
    }
    fn record_sync(&mut self, st: &SyncState) -> Result<(), CacheError> {
        self.session(|x| x.record_sync(st))
    }
    fn record_sync_result(
        &mut self,
        s: &ScopeId,
        outcome: &SyncOutcome,
        started: &Timestamp,
    ) -> Result<SyncState, CacheError> {
        self.session(|x| x.record_sync_result(s, outcome, started))
    }
    fn sync_alert(&self, s: &ScopeId) -> Result<Option<String>, CacheError> {
        self.session(|x| x.sync_alert(s))
    }
    fn set_sync_alert(&mut self, s: &ScopeId, cause: Option<&str>) -> Result<(), CacheError> {
        self.session(|x| x.set_sync_alert(s, cause))
    }
    fn set_sync_alert_within(
        &mut self,
        s: &ScopeId,
        cause: Option<&str>,
        wait: Duration,
    ) -> Result<(), CacheError> {
        self.session(|x| x.set_sync_alert_within(s, cause, wait))
    }
    fn outage(&self) -> Result<Option<Outage>, CacheError> {
        self.session(|x| x.outage())
    }
    fn set_outage(&mut self, o: Option<&Outage>) -> Result<(), CacheError> {
        self.session(|x| x.set_outage(o))
    }
    fn prune_scope(&mut self, s: &ScopeId) -> Result<PruneReport, CacheError> {
        self.session(|x| x.prune_scope(s))
    }
    fn open_work(&self) -> Result<OpenWork, CacheError> {
        self.session(|x| x.open_work())
    }
    fn status(&self) -> Result<StoreStatus, CacheError> {
        self.session(|x| x.status())
    }
    fn journal_counts(&self) -> Result<JournalCounts, CacheError> {
        self.session(|x| x.journal_counts())
    }
}

/// `CacheStore::journal_counts`: the journal grouped by phase and state.
const JOURNAL_COUNTS_SQL: &str = "SELECT phase, state, COUNT(*) FROM journal GROUP BY phase, state";

/// Reads that span several statements, each run on one snapshot.
impl Session<'_> {
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
            .prepare_cached(concat!(
                "SELECT b.id, b.row_json, b.stale FROM beads b WHERE ",
                in_scope!(),
                " ORDER BY b.id"
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
        let registry: BTreeMap<ScopeId, Scope> = self
            .scopes()?
            .into_iter()
            .map(|s| (s.id.clone(), s))
            .collect();
        let mut names: BTreeSet<ScopeId> = registry.keys().cloned().collect();
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
                filter: registry.get(&s).and_then(|r| r.filter.clone()),
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
    use std::sync::atomic::{AtomicI64, Ordering};

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

    fn seed_applied(path: &Path, tag: &str, n: usize, updated_at: &str) {
        let mut c = Connection::open(path).unwrap();
        let tx = c.transaction().unwrap();
        for i in 0..n {
            tx.execute(
                "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                 actor, state, attempts, created_at, updated_at) VALUES \
                 (?1, 'x', 'b', 'close', '{}', '{}', 'a', 'applied', 0, \
                 '2026-10-24T07:00:00Z', ?2)",
                [format!("{tag}-{i}"), updated_at.to_owned()],
            )
            .unwrap();
        }
        tx.commit().unwrap();
    }

    fn vm_steps(c: &Connection, sql: &str, args: impl rusqlite::Params) -> i32 {
        let mut stmt = c.prepare(sql).unwrap();
        let mut rows = stmt.query(args).unwrap();
        while rows.next().unwrap().is_some() {}
        drop(rows);
        stmt.get_status(rusqlite::StatementStatus::VmStep)
    }

    /// Entries with an unreadable time written while the store is open are
    /// warned about once for each new highest seq, and the read path stops
    /// walking them (QA5-002).
    #[test]
    fn undatable_entries_written_while_open_are_warned_once_and_not_walked_again() {
        // Removed on drop, after the stores below (declared later, dropped first).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("u.sqlite3");
        let identity = DatabaseIdentity {
            dolt_database: DoltDatabase::new("p").unwrap(),
            project_id: ProjectId::new("p-p").unwrap(),
        };
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();
        let store = SqliteStore::open_at(&path, &identity).unwrap();
        assert_eq!(store.inner().undatable_through.get(), 0);

        // Many unreadable ones, past the threshold, then one readable old one.
        let bad = usize::try_from(READ_DATING_THRESHOLD).unwrap() + 3_000;
        seed_applied(&path, "bad", bad, "yesterday");
        seed_applied(&path, "old", 1, "2026-10-24T08:00:00Z");
        let highest_bad = i64::try_from(bad).unwrap();
        assert!(store.has_entries_since(&at).unwrap(), "unreadable counts");
        assert_eq!(
            store.inner().undatable_through.get(),
            highest_bad,
            "found and warned"
        );
        // Past the threshold the readable one was dated: only the bad ones remain.
        assert_eq!(store.inner().examined_through.get(), highest_bad);
        assert_eq!(
            store.session(|s| s.note_undatable()).unwrap(),
            None,
            "no second warning"
        );

        // The read path's count starts above them: a probe, not a walk.
        let c = Connection::open(&path).unwrap();
        let limit = i64::from(READ_DATING_THRESHOLD) + 1;
        let above = vm_steps(&c, UNDATED_ABOVE_SQL, [highest_bad, limit]);
        let all = vm_steps(&c, UNDATED_ABOVE_SQL, [0, limit]);
        assert!(
            above < 50,
            "known unreadable entries are skipped: {above} steps"
        );
        assert!(all > 2_000, "control: counting from 0 walks them ({all})");

        // A new one is warned about, once.
        seed_applied(&path, "worse", 1, "garbage");
        assert!(store.has_entries_since(&at).unwrap());
        assert_eq!(store.inner().undatable_through.get(), highest_bad + 2);
        assert_eq!(store.session(|s| s.note_undatable()).unwrap(), None);
        seed_applied(&path, "worst", 1, "never");
        assert_eq!(
            store.session(|s| s.note_undatable()).unwrap(),
            Some(highest_bad + 3)
        );
        assert_eq!(store.session(|s| s.note_undatable()).unwrap(), None);
        drop((c, store));

        // The open warns about all of them and starts above the highest.
        let reopened = SqliteStore::open_at(&path, &identity).unwrap();
        assert_eq!(reopened.inner().undatable_through.get(), highest_bad + 3);
        assert_eq!(reopened.session(|s| s.note_undatable()).unwrap(), None);
        drop(reopened);
    }

    /// A store on a fresh file in a temp dir. Field order matters: the store
    /// closes before the directory is removed, also when a test fails (QA6-002).
    struct TempStore {
        store: SqliteStore,
        path: PathBuf,
        _dir: tempfile::TempDir,
    }

    fn temp_store() -> TempStore {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("t.sqlite3");
        let identity = DatabaseIdentity {
            dolt_database: DoltDatabase::new("p").unwrap(),
            project_id: ProjectId::new("p-p").unwrap(),
        };
        let store = SqliteStore::open_at(&path, &identity).unwrap();
        TempStore {
            store,
            path,
            _dir: dir,
        }
    }

    fn undated_count(path: &Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM journal WHERE state = 'applied' AND applied_epoch IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap()
    }

    /// The floor stops one below the first dateable entry, not on it
    /// (QA5-002). After a skipped dating, with a dateable entry at seq F
    /// above unreadable ones, `undatable_through` is exactly F - 1, so the
    /// next count includes F: it is dated with the rest once the threshold
    /// is passed. A floor of F would leave it out of the count and undated.
    #[test]
    fn the_floor_stops_one_below_the_first_dateable_entry() {
        let TempStore { store, path, _dir } = temp_store();
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();

        // Seqs 1..=3 unreadable, seq 4 (= F) readable: below the threshold,
        // so the dating is skipped and the floor rises through the bad ones.
        seed_applied(&path, "bad", 3, "garbage");
        seed_applied(&path, "first", 1, "2026-10-24T08:00:00Z");
        let first_dateable = 4;
        assert!(store.has_entries_since(&at).unwrap(), "unreadable counts");
        assert_eq!(
            store.inner().undatable_through.get(),
            first_dateable - 1,
            "the floor is F - 1: F stays in the count"
        );
        assert_eq!(undated_count(&path), 4, "nothing dated yet");

        // F plus THRESHOLD more readable ones: counted from F - 1 that is
        // THRESHOLD + 1, so the dating runs and includes F.
        seed_applied(
            &path,
            "more",
            usize::try_from(READ_DATING_THRESHOLD).unwrap(),
            "2026-10-24T08:00:00Z",
        );
        assert!(store.has_entries_since(&at).unwrap());
        assert_eq!(
            undated_count(&path),
            3,
            "F was dated with the rest: only the unreadable remain"
        );
        let epoch: Option<i64> = Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT applied_epoch FROM journal WHERE seq = ?1",
                [first_dateable],
                |r| r.get(0),
            )
            .unwrap();
        assert!(epoch.is_some(), "entry F itself is dated");
    }

    /// The busy timeout in effect inside a write, read by a temporary
    /// trigger `name`: `trigger` (`AFTER <event> ON main.<table>`) calls
    /// `probe_busy_timeout()`, which reads `PRAGMA busy_timeout` on the
    /// connection running the statement, once it holds the write lock, and
    /// stores it in the returned cell (-1 until the trigger fires). Causal, no
    /// timing.
    fn probe_busy_timeout(conn: &Connection, name: &str, trigger: &str) -> Arc<AtomicI64> {
        let seen = Arc::new(AtomicI64::new(-1));
        let sink = Arc::clone(&seen);
        conn.create_scalar_function(
            "probe_busy_timeout",
            0,
            rusqlite::functions::FunctionFlags::SQLITE_UTF8,
            move |ctx| {
                // SAFETY: rusqlite's contract for `get_connection` is that the
                // returned `ConnectionRef` is not sent to, or used from,
                // another thread (rusqlite issue 643). It is used only here,
                // for one read, on the thread running this statement, and is
                // dropped before the closure returns: it neither outlives the
                // call nor leaves this thread.
                let ms: i64 =
                    unsafe { ctx.get_connection() }?
                        .query_row("PRAGMA busy_timeout", [], |r| r.get(0))?;
                sink.store(ms, Ordering::SeqCst);
                Ok(0_i64)
            },
        )
        .unwrap();
        conn.execute_batch(&format!(
            "CREATE TEMP TRIGGER {name} {trigger} BEGIN SELECT probe_busy_timeout(); END;"
        ))
        .unwrap();
        seen
    }

    fn busy_timeout_ms(c: &Connection) -> i64 {
        c.query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap()
    }

    fn ms(d: Duration) -> i64 {
        i64::try_from(d.as_millis()).unwrap()
    }

    /// The dating `UPDATE` runs with a zero busy timeout, and the timeout is
    /// back afterwards (QA5-003). A timeout of any size would make the read
    /// wait for a held write lock, but that wait is bounded and ends in the
    /// same answer, so it cannot be told apart by what the read returns
    /// without a clock. Instead a temporary trigger samples
    /// `PRAGMA busy_timeout` from inside the `UPDATE`, once it has the lock:
    /// causal, no timing.
    #[test]
    fn the_dating_update_runs_with_no_busy_timeout_and_restores_it() {
        let TempStore { store, path, _dir } = temp_store();
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();
        let seen = probe_busy_timeout(
            &store.inner().conn,
            "probe_dating",
            "AFTER UPDATE ON main.journal",
        );
        let normal = ms(BUSY_TIMEOUT);
        assert_eq!(
            busy_timeout_ms(&store.inner().conn),
            normal,
            "control: the usual timeout"
        );

        seed_applied(
            &path,
            "old",
            usize::try_from(READ_DATING_THRESHOLD).unwrap() + 1,
            "2026-10-24T08:00:00Z",
        );
        assert!(!store.has_entries_since(&at).unwrap());
        assert_eq!(undated_count(&path), 0, "the dating ran");
        assert_eq!(seen.load(Ordering::SeqCst), 0, "no busy timeout inside it");
        assert_eq!(
            busy_timeout_ms(&store.inner().conn),
            normal,
            "restored afterwards"
        );
    }

    /// A bounded marker write runs with `min(BUSY_TIMEOUT, wait)` as its busy
    /// timeout, and the timeout is back afterwards (lead ruling S33). Sampled
    /// from inside each write by a temporary trigger, as the dating's is:
    /// causal, no timing. The contended case is `tests/store.rs`
    /// (`a_bounded_marker_write_gives_up_on_a_held_lock_within_its_wait`).
    #[test]
    fn a_bounded_marker_write_waits_at_most_its_wait_and_restores_the_timeout() {
        let TempStore {
            mut store, _dir, ..
        } = temp_store();
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();
        let x = ScopeId::new("x").unwrap();
        let normal = ms(BUSY_TIMEOUT);

        let alert = probe_busy_timeout(
            &store.inner().conn,
            "probe_alert",
            "AFTER INSERT ON main.meta",
        );
        let quarter = Duration::from_millis(250);
        store.set_sync_alert_within(&x, Some("c"), quarter).unwrap();
        assert_eq!(alert.load(Ordering::SeqCst), ms(quarter), "the time left");
        assert_eq!(
            busy_timeout_ms(&store.inner().conn),
            normal,
            "restored afterwards"
        );
        store
            .set_sync_alert_within(&x, Some("d"), Duration::ZERO)
            .unwrap();
        assert_eq!(alert.load(Ordering::SeqCst), 0, "no time left: no wait");
        // The unbounded form is the usual timeout.
        store.set_sync_alert(&x, Some("e")).unwrap();
        assert_eq!(alert.load(Ordering::SeqCst), normal);

        let seq = store
            .insert_entry(
                bd_cache_model::NewEntry {
                    idem_key: IdemKey::new("k").unwrap(),
                    scope: None,
                    bead: sc_beads::testing::id("b"),
                    op: bd_cache_model::Op::Close { reason: None },
                    precondition: sc_beads::Guards::default(),
                    actor: sc_beads::testing::actor("san"),
                    changed: Some(true),
                },
                &at,
            )
            .unwrap();
        store.escalate(seq, "gone", &at).unwrap();
        let mark = probe_busy_timeout(
            &store.inner().conn,
            "probe_mark",
            "AFTER UPDATE OF announced_at ON main.journal",
        );
        // Less time left than the store's own timeout: the time left, as the
        // alert's (the planted bug S33 gave the mark the full timeout).
        store.mark_announced_within(seq, &at, quarter).unwrap();
        assert_eq!(mark.load(Ordering::SeqCst), ms(quarter), "the time left");
        assert_eq!(
            busy_timeout_ms(&store.inner().conn),
            normal,
            "restored afterwards"
        );
        store
            .mark_announced_within(seq, &at, Duration::ZERO)
            .unwrap();
        assert_eq!(mark.load(Ordering::SeqCst), 0, "no time left: no wait");
        assert_eq!(busy_timeout_ms(&store.inner().conn), normal);
        // More time left than the store's own timeout: capped at it.
        store
            .mark_announced_within(seq, &at, Duration::from_secs(60))
            .unwrap();
        assert_eq!(
            mark.load(Ordering::SeqCst),
            normal,
            "capped at BUSY_TIMEOUT"
        );
        assert_eq!(busy_timeout_ms(&store.inner().conn), normal);
        assert!(store.unannounced_escalations().unwrap().is_empty());
    }

    /// QA6-001, the chosen behaviour (ADR-STORE-0004): an entry still pending
    /// when the floor rose above its seq, then applied by the v3 Python writer
    /// without an epoch, is below the floor. It is answered as every undated
    /// entry is, it is not in the threshold count, and the next dating dates it.
    /// A second rise, driven by a warning alone, passes only the entries above
    /// the old floor: the readable entry below it is not examined again and
    /// stays undated and answered until the next dating (QA7-001).
    #[test]
    fn a_pending_entry_applied_below_the_floor_is_answered_and_dated_by_the_next_dating() {
        let TempStore { store, path, _dir } = temp_store();
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();
        // Seq 1 pending (the v3 Python writer's), seqs 2..=4 applied with unreadable times.
        Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO journal(idem_key, phase, bead, op, args_json, precondition_json, \
                 actor, state, attempts, created_at, updated_at) VALUES \
                 ('low', 'x', 'b', 'close', '{}', '{}', 'a', 'pending', 0, \
                 '2026-10-24T07:00:00Z', '2026-10-24T07:00:00Z')",
                [],
            )
            .unwrap();
        seed_applied(&path, "bad", 3, "garbage");
        assert!(store.has_entries_since(&at).unwrap());
        assert_eq!(
            store.inner().undatable_through.get(),
            4,
            "the floor rose past seq 1"
        );

        // the v3 Python writer applies seq 1, with a readable time and no epoch.
        Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE journal SET state = 'applied', applied_at = '2026-10-24T08:30:00Z', \
                 updated_at = '2026-10-24T08:30:00Z' WHERE seq = 1",
                [],
            )
            .unwrap();
        let seqs = |since: &str| -> Vec<i64> {
            store
                .applied_since(&Timestamp::parse(since).unwrap())
                .unwrap()
                .iter()
                .map(|e| e.seq.get())
                .collect()
        };
        // Answered by its own time; the unreadable ones count as every instant.
        assert_eq!(seqs("2026-10-24T08:00:00Z"), [1, 2, 3, 4]);
        assert_eq!(seqs("2026-10-24T09:00:00Z"), [2, 3, 4]);
        assert_eq!(store.inner().undatable_through.get(), 4, "the floor stays");
        assert_eq!(undated_count(&path), 4, "not dated: below the threshold");

        // A second rise from a warning alone (QA7-001): seq 5 is unreadable,
        // below the threshold, so nothing is dated; the warning raises the
        // floor from 4 to 5. The rise passes only seq 5; seq 1, readable and
        // below the old floor, is not examined again and stays undated.
        seed_applied(&path, "worse", 1, "garbage");
        assert!(store.has_entries_since(&at).unwrap());
        assert_eq!(
            store.inner().undatable_through.get(),
            5,
            "the warning raised the floor"
        );
        let seq1_epoch = || -> Option<i64> {
            Connection::open(&path)
                .unwrap()
                .query_row("SELECT applied_epoch FROM journal WHERE seq = 1", [], |r| {
                    r.get(0)
                })
                .unwrap()
        };
        assert_eq!(
            seq1_epoch(),
            None,
            "below the old floor: not dated by a rise"
        );
        assert_eq!(undated_count(&path), 5, "nothing dated");
        // Still answered by its own time, below the new floor too.
        assert_eq!(seqs("2026-10-24T08:00:00Z"), [1, 2, 3, 4, 5]);
        assert_eq!(seqs("2026-10-24T09:00:00Z"), [2, 3, 4, 5]);

        // The next dating dates it, whatever its seq.
        seed_applied(
            &path,
            "more",
            usize::try_from(READ_DATING_THRESHOLD).unwrap() + 1,
            "2026-10-24T08:00:00Z",
        );
        assert!(store.has_entries_since(&at).unwrap());
        assert_eq!(
            seq1_epoch(),
            Some(at.unix() - 1_800),
            "seq 1 is dated by its time"
        );
        assert_eq!(undated_count(&path), 4, "only the unreadable ones remain");
    }

    fn new_entry(key: &str, scope: Option<&str>, changed: Option<bool>) -> NewEntry {
        NewEntry {
            idem_key: IdemKey::new(key).unwrap(),
            scope: scope.map(|s| ScopeId::new(s).unwrap()),
            bead: sc_beads::testing::id("b1"),
            op: bd_cache_model::Op::Close { reason: None },
            precondition: sc_beads::Guards::default(),
            actor: sc_beads::testing::actor("san"),
            changed,
        }
    }

    /// The committed journal as `(seq, changed)`.
    type Committed = Vec<(i64, Option<i64>)>;

    /// The committed journal as `(seq, changed)`, read on a connection of its
    /// own: what another process sees.
    fn committed_changed(path: &Path) -> Committed {
        let c =
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut stmt = c
            .prepare("SELECT seq, changed FROM journal ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// REQ-BDC-0809: the entry and its `changed` are committed together.
    ///
    /// Temporary triggers on every insert into and update of `journal` call
    /// `probe_committed()` from inside the store's write transaction, while it
    /// holds the write lock. The probe reads the committed journal on a second
    /// connection, as another process would, and records it. Every snapshot
    /// must show either no entry or the entry with its `changed`: an entry
    /// committed first and given its `changed` by a later transaction would be
    /// seen without it by the probe of that later write. Causal, no timing.
    #[test]
    fn a_reader_never_sees_an_entry_without_its_changed() {
        let TempStore { store, path, _dir } = temp_store();
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();
        let snapshots: Arc<std::sync::Mutex<Vec<Committed>>> = Arc::default();
        let sink = Arc::clone(&snapshots);
        let probe_path = path.clone();
        store
            .inner()
            .conn
            .create_scalar_function(
                "probe_committed",
                0,
                rusqlite::functions::FunctionFlags::SQLITE_UTF8,
                move |_| {
                    sink.lock().unwrap().push(committed_changed(&probe_path));
                    Ok(0_i64)
                },
            )
            .unwrap();
        store
            .inner()
            .conn
            .execute_batch(
                "CREATE TEMP TRIGGER probe_insert AFTER INSERT ON main.journal \
                 BEGIN SELECT probe_committed(); END; \
                 CREATE TEMP TRIGGER probe_update AFTER UPDATE ON main.journal \
                 BEGIN SELECT probe_committed(); END;",
            )
            .unwrap();
        let mut store = store;
        let a = store
            .insert_entry(new_entry("a", Some("x"), Some(true)), &at)
            .unwrap();
        let b = store
            .insert_entry(new_entry("b", None, Some(false)), &at)
            .unwrap();

        let seen = snapshots.lock().unwrap().clone();
        assert!(seen.len() >= 2, "the probe ran inside each write: {seen:?}");
        for snapshot in &seen {
            for (seq, changed) in snapshot {
                assert!(
                    changed.is_some(),
                    "entry {seq} was visible without its changed: {seen:?}"
                );
            }
        }
        assert_eq!(
            seen.first(),
            Some(&vec![]),
            "inside the first write nothing is committed yet"
        );
        assert_eq!(
            committed_changed(&path),
            [(a.get(), Some(1)), (b.get(), Some(0))],
            "both committed with their changed"
        );
        assert_eq!(store.entry_by_seq(a).unwrap().unwrap().changed, Some(true));
        assert_eq!(store.entry_by_seq(b).unwrap().unwrap().changed, Some(false));
    }

    thread_local! {
        /// The statements `count_statements` saw on this thread.
        static STATEMENTS: std::cell::RefCell<Vec<String>> = const {
            std::cell::RefCell::new(Vec::new())
        };
    }

    #[expect(
        clippy::needless_pass_by_value,
        reason = "the signature rusqlite's trace_v2 takes"
    )]
    fn count_statements(event: rusqlite::trace::TraceEvent<'_>) {
        if let rusqlite::trace::TraceEvent::Stmt(_, sql) = event {
            STATEMENTS.with(|s| s.borrow_mut().push(sql.to_owned()));
        }
    }

    /// Contract §6 `journal`: `journal_counts` is one grouped query, whatever
    /// the size of the journal, and its buckets sum to the total.
    ///
    /// The store's statements are counted with SQLite's statement trace on
    /// its own connection (the query counter): exactly one runs.
    #[test]
    fn journal_counts_is_one_query_and_its_buckets_sum_to_the_total() {
        let TempStore {
            mut store,
            path: _path,
            _dir,
        } = temp_store();
        let at = Timestamp::parse("2026-10-24T09:00:00Z").unwrap();
        let lead = sc_beads::testing::actor("lead");
        let mut n = 0;
        let mut journal = |store: &mut SqliteStore, scope: Option<&str>| {
            n += 1;
            store
                .insert_entry(new_entry(&format!("k{n}"), scope, Some(true)), &at)
                .unwrap()
        };
        // x: 3 pending, 2 applied, 1 escalated; y: 1 resolved, 1 pending;
        // unscoped: 2 pending, 1 escalated, 1 applied.
        for _ in 0..3 {
            journal(&mut store, Some("x"));
        }
        for _ in 0..2 {
            let s = journal(&mut store, Some("x"));
            store.apply_with_readback(s, None, None, &at).unwrap();
        }
        let s = journal(&mut store, Some("x"));
        store.escalate(s, "BD_CACHE.PRECONDITION: no", &at).unwrap();
        let s = journal(&mut store, Some("y"));
        store.escalate(s, "why", &at).unwrap();
        store.resolve(s, "by hand", &lead, &at).unwrap();
        journal(&mut store, Some("y"));
        for _ in 0..2 {
            journal(&mut store, None);
        }
        let s = journal(&mut store, None);
        store.escalate(s, "why", &at).unwrap();
        let s = journal(&mut store, None);
        store.apply_with_readback(s, None, None, &at).unwrap();

        STATEMENTS.with(|s| s.borrow_mut().clear());
        store.inner().conn.trace_v2(
            rusqlite::trace::TraceEventCodes::SQLITE_TRACE_STMT,
            Some(count_statements),
        );
        let counts = store.journal_counts().unwrap();
        store
            .inner()
            .conn
            .trace_v2(rusqlite::trace::TraceEventCodes::empty(), None);
        let ran = STATEMENTS.with(|s| s.borrow().clone());
        assert_eq!(ran.len(), 1, "one query: {ran:?}");

        let sc = |pending, applied, escalated, resolved| bd_cache_model::StateCounts {
            pending,
            applied,
            escalated,
            resolved,
        };
        let x = ScopeId::new("x").unwrap();
        let y = ScopeId::new("y").unwrap();
        assert_eq!(counts.by_scope.get(&x), Some(&sc(3, 2, 1, 0)));
        assert_eq!(counts.by_scope.get(&y), Some(&sc(1, 0, 0, 1)));
        assert_eq!(counts.by_scope.len(), 2);
        assert_eq!(counts.unscoped, sc(2, 1, 1, 0));
        assert_eq!(counts.total, sc(6, 3, 2, 1), "the hand count");
        let mut summed = counts.unscoped;
        for bucket in counts.by_scope.values() {
            for state in [
                JournalState::Pending,
                JournalState::Applied,
                JournalState::Escalated,
                JournalState::Resolved,
            ] {
                summed.add(state, bucket.get(state));
            }
        }
        assert_eq!(summed, counts.total, "total is the sum of the buckets");
        assert_eq!(counts.total.sum(), 12, "every entry counted once");
    }
}
