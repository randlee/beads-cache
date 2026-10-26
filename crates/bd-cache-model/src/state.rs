//! Store-side state: freshness, sync state, outage, fingerprint outcome, reports and status.

use std::collections::BTreeMap;

use sc_beads::{BeadId, BlockedRow, InvalidValue, Timestamp};
use serde::Serialize;

use crate::journal::JournalEntry;
use crate::scope::{ScopeFilter, ScopeId};

/// Whether a stored row is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    /// Stored from a successful read.
    Fresh,
    /// A read-back failed (`stale = 1`); the next sync re-reads it.
    Stale,
}

/// One scope's sync cursor and last outcome (`sync_state`).
///
/// `failing_since` and `notified_since` are the ADR-STORE-0006 columns of
/// REQ-BDC-0728; they are not part of the 0.1 status dict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncState {
    /// The scope.
    #[serde(rename = "phase")]
    pub scope: ScopeId,
    /// Start of the latest sync.
    pub last_started_at: Option<Timestamp>,
    /// Start of the latest successful sync (the cursor basis).
    pub last_ok_at: Option<Timestamp>,
    /// The latest failure, `CODE: detail`.
    pub last_error: Option<String>,
    /// Since when the scope's results have not been ok ([`FailingSince`]).
    #[serde(skip)]
    pub failing_since: Option<FailingSince>,
    /// The `failing_since` a hook last marked ([`SyncState::notified`]).
    #[serde(skip)]
    pub notified_since: Option<FailingSince>,
}

impl SyncState {
    /// A scope that never synced.
    #[must_use]
    pub fn new(scope: ScopeId) -> Self {
        Self {
            scope,
            last_started_at: None,
            last_ok_at: None,
            last_error: None,
            failing_since: None,
            notified_since: None,
        }
    }

    /// True when the current failure has been marked: `notified_since`
    /// equals `failing_since` (REQ-BDC-0728).
    #[must_use]
    pub fn notified(&self) -> bool {
        self.failing_since.is_some() && self.notified_since == self.failing_since
    }

    /// Applies one sync result to the failure state (REQ-BDC-0728): an ok
    /// result clears `failing_since` (and the mark); a non-ok one (a failure
    /// or an outage) keeps a `failing_since` already set, and otherwise sets
    /// it to `at`, the instant of this first non-ok result.
    ///
    /// # Examples
    ///
    /// ```
    /// use bd_cache_model::{ScopeId, SyncState};
    /// use sc_beads::Timestamp;
    /// let t1 = Timestamp::parse("2026-10-23T10:00:00Z").unwrap();
    /// let t2 = Timestamp::parse("2026-10-23T10:01:00Z").unwrap();
    /// let mut st = SyncState::new(ScopeId::new("x").unwrap());
    /// st.record_result(false, &t1);
    /// let since = st.failing_since.clone().unwrap();
    /// st.record_result(false, &t2);
    /// assert_eq!(st.failing_since, Some(since));
    /// st.record_result(true, &t2);
    /// assert_eq!(st.failing_since, None);
    /// ```
    pub fn record_result(&mut self, ok: bool, at: &Timestamp) {
        if ok {
            self.failing_since = None;
            self.notified_since = None;
        } else if self.failing_since.is_none() {
            self.failing_since = Some(FailingSince::at(at));
        }
    }
}

/// One sync's result, as [`CacheStore::record_sync_result`](crate::CacheStore::record_sync_result)
/// applies it to the scope's sync state (REQ-BDC-0728, ADR-CORE-0202 as amended).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncOutcome {
    /// The sync stored the scope: `last_ok_at` is its start; the failure state clears.
    Ok,
    /// The sync failed, or bd could not be reached: `last_error` is this
    /// text (`CODE: detail`); `failing_since` is set on the first one.
    Failed(String),
    /// The deadline cut the sync: only `last_started_at` is set.
    Deferred,
}

/// When a scope's results stopped being ok: an opaque token (REQ-BDC-0728).
///
/// Callers round-trip it (to `scope_announce`) and never parse it; equality
/// is the only operation they rely on. The store keeps it as text.
///
/// # Examples
///
/// ```
/// use bd_cache_model::FailingSince;
/// use sc_beads::Timestamp;
/// let at = Timestamp::parse("2026-10-23T10:00:00Z").unwrap();
/// let token = FailingSince::at(&at);
/// assert_eq!(FailingSince::parse(token.as_str()).unwrap(), token);
/// assert!(FailingSince::parse("").is_err());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct FailingSince(String);

impl FailingSince {
    /// The token of a failure that started at `at`.
    #[must_use]
    pub fn at(at: &Timestamp) -> Self {
        Self(at.as_str().to_owned())
    }

    /// A token a caller hands back (`--since <token>`, a stored value).
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty or blank token.
    pub fn parse(token: impl Into<String>) -> Result<Self, InvalidValue> {
        let token = token.into();
        if token.trim().is_empty() {
            return Err(InvalidValue {
                kind: "failing_since token",
                value: token,
                reason: "empty",
            });
        }
        Ok(Self(token))
    }

    /// The token's text, to hand back unchanged.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FailingSince {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for FailingSince {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// `Hash` and `Eq` are the token's text's, so a map keyed by it is looked up by `&str`.
impl std::borrow::Borrow<str> for FailingSince {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// One run of a sync row: the id a lease holder, a `TICK_RUN_FAILED` and a
/// lease release name it by (REQ-BDC-0722, 0729).
///
/// Opaque to the store and the core: they compare it and never parse it.
///
/// # Examples
///
/// ```
/// use bd_cache_model::RunId;
/// let id = RunId::new("run-1")?;
/// assert_eq!(id.as_str(), "run-1");
/// assert!(RunId::new(" ").is_err());
/// # Ok::<(), sc_beads::InvalidValue>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct RunId(String);

impl RunId {
    /// A run id.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] for an empty or blank id.
    pub fn new(id: impl Into<String>) -> Result<Self, InvalidValue> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(InvalidValue {
                kind: "run id",
                value: id,
                reason: "empty",
            });
        }
        Ok(Self(id))
    }

    /// The id's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for RunId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Who holds the one sync lease (`tick_lease`, REQ-BDC-0722).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LeaseHolder {
    /// The run holding it.
    pub run_id: RunId,
    /// The holder's process id.
    pub pid: u32,
    /// The holder's host name.
    pub host: String,
    /// When the lease expires: an expired lease may be taken over.
    pub expires_at: Timestamp,
}

/// What a lease take found (REQ-BDC-0722).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "lease", rename_all = "snake_case")]
pub enum LeaseTake {
    /// The lease was free and is now held by the caller.
    Taken,
    /// An expired lease was taken over; its last holder.
    TookOver(LeaseHolder),
    /// A live lease is held by another run: nothing changed.
    Held(LeaseHolder),
}

/// The most bytes of a failed hook's error the hook state keeps (2 KiB).
///
/// The text is a consumer's hook output, rewritten on every failure and read
/// back into `status` and replies; 2 KiB keeps the cause readable, as the
/// tick's announcement text limit does.
pub const MAX_HOOK_ERROR_BYTES: usize = 2048;

/// `err` clipped to at most [`MAX_HOOK_ERROR_BYTES`] at a character
/// boundary: what both stores keep as `hook_state.last_error`.
///
/// # Examples
///
/// ```
/// use bd_cache_model::{MAX_HOOK_ERROR_BYTES, clip_hook_error};
/// assert_eq!(clip_hook_error("short"), "short");
/// let long = "é".repeat(MAX_HOOK_ERROR_BYTES);
/// assert!(clip_hook_error(&long).len() <= MAX_HOOK_ERROR_BYTES);
/// ```
#[must_use]
pub fn clip_hook_error(err: &str) -> &str {
    if err.len() <= MAX_HOOK_ERROR_BYTES {
        return err;
    }
    let mut end = MAX_HOOK_ERROR_BYTES;
    while !err.is_char_boundary(end) {
        end -= 1;
    }
    &err[..end]
}

/// The built-in row's post-sync hook backoff state (`hook_state`, REQ-BDC-0727).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct HookState {
    /// Consecutive failed hook calls; 0 after a success.
    pub consecutive_failures: u32,
    /// When the first of those failures happened.
    pub first_failure_at: Option<Timestamp>,
    /// The latest failure's error.
    pub last_error: Option<String>,
    /// The hook is not called before this instant (the backoff).
    pub next_attempt_at: Option<Timestamp>,
}

/// The journal's pending entries in brief: how many, and when the oldest was
/// journaled (the post-sync hook request's `pending`, REQ-BDC-0727).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct PendingSummary {
    /// Pending entries.
    pub count: u64,
    /// The oldest pending entry's `created_at`; `None` with none pending.
    pub oldest_created_at: Option<Timestamp>,
}

/// The first unannounced escalations, oldest first, and how many there are
/// in all (the post-sync hook request's `escalated`, REQ-BDC-0727).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EscalationPage {
    /// At most the page size, oldest first.
    pub entries: Vec<crate::journal::JournalEntry>,
    /// Every unannounced escalation.
    pub total: u64,
}

/// How a sync row's run ended, for its run record ([`SyncRun`], REQ-BDC-0731).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncRunEnd {
    /// It ran and succeeded: the failure count is reset.
    Ok,
    /// It failed: counted as one more consecutive failure.
    Failed,
    /// It did nothing (another run held the lease, or its stop check said
    /// to skip it): the failure count is left as it is.
    Skipped,
}

impl SyncRunEnd {
    /// The stored text: `ok`, `failed` or `skipped`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    /// The end named by [`SyncRunEnd::as_str`]; `None` for any other text.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "ok" => Some(Self::Ok),
            "failed" => Some(Self::Failed),
            "skipped" => Some(Self::Skipped),
            _ => None,
        }
    }
}

/// The run record of one sync row (`sync_run`, REQ-BDC-0731, ADR-STORE-0006
/// as amended by sprint b-8).
///
/// sc-cron keeps one last outcome per row and no run id or counter, so the
/// sync keeps its own record per row name: the latest run's id and times,
/// and the consecutive failures with the first one's time. A run is started
/// ([`CacheStore::sync_run_started`](crate::CacheStore::sync_run_started))
/// before it does anything and finished
/// ([`CacheStore::sync_run_finished`](crate::CacheStore::sync_run_finished))
/// when it returns; a run that never finished (timed out, aborted or
/// killed) is counted as a failure when the next run starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncRun {
    /// The latest run, finished or not.
    pub run_id: RunId,
    /// When it started.
    pub started_at: Timestamp,
    /// When it finished; `None` while it runs, or when it never finished.
    pub finished_at: Option<Timestamp>,
    /// How it ended, once it finished.
    pub end: Option<SyncRunEnd>,
    /// Consecutive failed runs up to the latest finished one; 0 after a success.
    pub consecutive_failures: u32,
    /// When the first of those failures happened.
    pub first_failure_at: Option<Timestamp>,
}

impl SyncRun {
    /// The record after a run `run_id` starts at `at`, from the `previous`
    /// record: a previous run that never finished counts as one failure,
    /// from its start. Both stores apply this one rule.
    #[must_use]
    pub fn started(previous: Option<&Self>, run_id: &RunId, at: &Timestamp) -> Self {
        let (mut failures, mut first) = previous.map_or((0, None), |p| {
            (p.consecutive_failures, p.first_failure_at.clone())
        });
        if let Some(p) = previous.filter(|p| p.finished_at.is_none()) {
            failures = failures.saturating_add(1);
            first = first.or_else(|| Some(p.started_at.clone()));
        }
        Self {
            run_id: run_id.clone(),
            started_at: at.clone(),
            finished_at: None,
            end: None,
            consecutive_failures: failures,
            first_failure_at: first,
        }
    }

    /// The record after the run it names finished at `at` with `end`.
    /// Both stores apply this one rule.
    #[must_use]
    pub fn finished(&self, at: &Timestamp, end: SyncRunEnd) -> Self {
        let (failures, first) = match end {
            SyncRunEnd::Ok => (0, None),
            SyncRunEnd::Failed => (
                self.consecutive_failures.saturating_add(1),
                self.first_failure_at.clone().or_else(|| Some(at.clone())),
            ),
            SyncRunEnd::Skipped => (self.consecutive_failures, self.first_failure_at.clone()),
        };
        Self {
            run_id: self.run_id.clone(),
            started_at: self.started_at.clone(),
            finished_at: Some(at.clone()),
            end: Some(end),
            consecutive_failures: failures,
            first_failure_at: first,
        }
    }
}

/// The backend outage record (ADR-BDC-0005).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Outage {
    /// When the outage started; kept across later failures.
    pub since: Timestamp,
    /// The latest error, `CODE: detail`.
    pub last_error: String,
}

/// What recording a fingerprint did (REQ-BDC-0105).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FingerprintOutcome {
    /// First check: recorded, nothing dropped.
    Recorded,
    /// Same schema sha as recorded.
    Unchanged,
    /// A different sha: cached rows dropped, journal kept, new sha recorded.
    Rebuilt,
}

/// What one `upsert_rows` call stored.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct UpsertReport {
    /// Rows stored.
    pub stored: Vec<BeadId>,
    /// Rows not stored because the stored row is newer (unforced upsert).
    pub skipped_newer: Vec<BeadId>,
    /// Existing rows whose status text changed (forced or not); new rows excluded.
    pub status_changed: Vec<BeadId>,
}

/// A scope's stored ready base (ADR-BDC-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReadyBase {
    /// `bd blocked -l <label>` rows.
    pub blocked: Vec<BlockedRow>,
    /// `bd ready -l <label> -n 0` ids.
    pub ready: Vec<BeadId>,
    /// The instant taken before the sync's first blocked/ready call.
    pub fetched_at: Timestamp,
    /// True when a later refresh failed or was cut.
    pub stale: bool,
}

/// One bead's entry in a scope's stored ready base, read without loading the
/// whole base ([`CacheStore::base_entry`](crate::CacheStore::base_entry)).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BaseEntry {
    /// The base's fetch instant.
    pub fetched_at: Timestamp,
    /// The base's stale mark.
    pub stale: bool,
    /// The bead's blockers in the base, in bd's order without repeats;
    /// empty when the base lists it as not blocked.
    pub blocked_by: Vec<BeadId>,
}

impl BaseEntry {
    /// The entry of `id` in a whole base: the blockers of every blocked row of
    /// that id, in order, without repeats.
    #[must_use]
    pub fn of(base: &ReadyBase, id: &BeadId) -> Self {
        let mut blocked_by: Vec<BeadId> = Vec::new();
        for row in base.blocked.iter().filter(|r| r.id == *id) {
            for b in &row.blocked_by {
                if !blocked_by.contains(b) {
                    blocked_by.push(b.clone());
                }
            }
        }
        Self {
            fetched_at: base.fetched_at.clone(),
            stale: base.stale,
            blocked_by,
        }
    }
}

/// Open work: pending entries and unannounced escalations (`cache health`
/// `open_work`, the tick's report).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct OpenWork {
    /// Pending entries.
    pub pending: u64,
    /// Escalated entries not yet announced.
    pub escalated_unannounced: u64,
}

impl OpenWork {
    /// True when any work is open.
    #[must_use]
    pub fn any(&self) -> bool {
        self.pending > 0 || self.escalated_unannounced > 0
    }
}

/// One scope in [`StoreStatus`]: its name (in `sync`), its filter, its sync
/// state and its bead count (REQ-BDC-0603 as amended by ADR-BDC-0013).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScopeStatus {
    /// Sync state.
    #[serde(flatten)]
    pub sync: SyncState,
    /// The scope's filter: `None` for a phase-a scope (its label is the
    /// legacy prefix followed by its name) or a name with sync state but no
    /// registry row. Not part of the 0.1 status dict, whose shape is frozen.
    #[serde(skip)]
    pub filter: Option<ScopeFilter>,
    /// Beads held by the scope.
    pub beads: u64,
}

/// What `status` reports from the store (REQ-BDC-0603).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct StoreStatus {
    /// The `meta` table.
    pub meta: BTreeMap<String, String>,
    /// Journal counts keyed `<phase>:<state>`; entries with no phase use `None`,
    /// as the Python implementation prints it.
    pub journal: BTreeMap<String, u64>,
    /// Per-scope sync state and bead counts, by scope.
    #[serde(rename = "phases")]
    pub scopes: Vec<ScopeStatus>,
    /// Every escalated entry until it is resolved, by seq.
    pub escalated: Vec<JournalEntry>,
    /// The outage, while one holds.
    pub outage: Option<Outage>,
    /// Rows whose read-back failed and that no store has refreshed since
    /// (the same set as `CacheStore::stale_rows`), by id (REQ-BDC-0603; a-1 amendment 1).
    pub stale: Vec<BeadId>,
}
