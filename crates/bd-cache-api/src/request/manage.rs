//! Surface (b) requests: the `cache` management commands (contract §6).

use bd_cache_model::{IdemKey, ScopeId, Seq};
use sc_beads::{Actor, BeadId};
use schemars::JsonSchema;

/// `cache status`: no members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[non_exhaustive]
pub struct CacheStatus;

/// `cache health [--probe]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CacheHealth {
    /// Also run the backend check (fingerprint and the rebuild rule).
    pub(crate) probe: bool,
    /// The age, in seconds, past which the oldest pending entry makes health
    /// `degraded`; absent takes [`CacheHealth::DEFAULT_MAX_PENDING_AGE_S`].
    pub(crate) max_pending_age_s: Option<u64>,
}

impl CacheHealth {
    /// The default age limit of the oldest pending entry: 300 s (contract §6).
    ///
    /// The cron drains every minute, so a pending entry older than five
    /// ticks means the drainer is not running or cannot apply it.
    pub const DEFAULT_MAX_PENDING_AGE_S: u64 = 300;

    /// The age, in seconds, past which the oldest pending entry makes health
    /// `degraded` (`--max-pending-age-s`; default
    /// [`CacheHealth::DEFAULT_MAX_PENDING_AGE_S`]).
    #[must_use]
    pub fn max_pending_age_s(&self) -> u64 {
        self.max_pending_age_s
            .unwrap_or(Self::DEFAULT_MAX_PENDING_AGE_S)
    }

    /// Also run the backend check (fingerprint and the rebuild rule).
    #[must_use]
    pub fn probe(&self) -> bool {
        self.probe
    }
}

/// `cache scopes`: no members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
#[non_exhaustive]
pub struct CacheScopes;

/// `cache refresh scopes [S…]`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct CacheRefreshScopes {
    /// The scopes to sync, in request order; empty syncs every known scope.
    #[schemars(with = "Vec<String>")]
    pub(crate) scopes: Vec<ScopeId>,
    /// Each scope's sync deadline, from 1 to [`Self::MAX_TIMEOUT_MS`]; absent
    /// takes the per-call cap. It bounds one scope's whole sync (several bd
    /// calls) and restarts for the next scope, so the command takes at most
    /// N times it for N scopes. Each bd call stays capped by the per-call cap
    /// as well, so a value above the cap lengthens the sync as a whole, never
    /// a single bd call.
    pub(crate) timeout_ms: Option<u64>,
}

impl CacheRefreshScopes {
    /// The largest `timeout_ms`: ten minutes, far past any one scope's sync,
    /// and the bound of a request that would otherwise hold the command for
    /// any `u64` of milliseconds (contract §6).
    pub const MAX_TIMEOUT_MS: u64 = 600_000;

    /// The scopes to sync, in request order; empty syncs every known scope.
    #[must_use]
    pub fn scopes(&self) -> &[ScopeId] {
        &self.scopes
    }

    /// Each scope's sync deadline; absent takes the per-call cap.
    #[must_use]
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }
}

/// `cache refresh issues ID…`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct CacheRefreshIssues {
    /// The beads to re-read from bd (nonempty).
    #[schemars(with = "Vec<String>")]
    pub(crate) ids: Vec<BeadId>,
    /// The read's timeout, from 1 to [`Self::MAX_TIMEOUT_MS`]; absent takes
    /// the configured miss-fill timeout.
    pub(crate) timeout_ms: Option<u64>,
}

impl CacheRefreshIssues {
    /// The largest `timeout_ms`: ten minutes, the bound of one `bd list --id`
    /// read (contract §6).
    pub const MAX_TIMEOUT_MS: u64 = 600_000;

    /// The beads to re-read from bd (nonempty).
    #[must_use]
    pub fn ids(&self) -> &[BeadId] {
        &self.ids
    }

    /// The read's timeout; absent takes the configured miss-fill timeout.
    #[must_use]
    pub fn timeout_ms(&self) -> Option<u64> {
        self.timeout_ms
    }
}

/// `cache flush [--start-gate-ms N] [--deadline-ms M]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CacheFlush {
    /// No entry starts after this many milliseconds (default 25,000).
    pub(crate) start_gate_ms: u64,
    /// No bd call runs past this many milliseconds (default 28,000; above `start_gate_ms`).
    pub(crate) deadline_ms: u64,
}

impl CacheFlush {
    /// No entry starts after this many milliseconds (default 25,000).
    #[must_use]
    pub fn start_gate_ms(&self) -> u64 {
        self.start_gate_ms
    }

    /// No bd call runs past this many milliseconds (default 28,000; above `start_gate_ms`).
    #[must_use]
    pub fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}

impl CacheFlush {
    /// The default start gate (contract §6).
    pub const DEFAULT_START_GATE_MS: u64 = 25_000;
    /// The default deadline (contract §6).
    pub const DEFAULT_DEADLINE_MS: u64 = 28_000;
}

/// `cache evict --scope S`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct CacheEvict {
    /// The scope to prune.
    #[schemars(with = "String")]
    pub(crate) scope: ScopeId,
}

impl CacheEvict {
    /// The scope to prune.
    #[must_use]
    pub fn scope(&self) -> &ScopeId {
        &self.scope
    }
}

/// How `cache journal get` names its entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalKey {
    /// `<seq>`.
    Seq(Seq),
    /// `--idempotency-key K`, after a lost reply (contract §7).
    IdempotencyKey(IdemKey),
}

/// `cache journal get <seq> | --idempotency-key K`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct JournalGet {
    /// The entry (`seq` or `idempotency_key` on the wire; exactly one).
    #[schemars(with = "String")]
    pub(crate) key: JournalKey,
}

impl JournalGet {
    /// The entry (`seq` or `idempotency_key` on the wire; exactly one).
    #[must_use]
    pub fn key(&self) -> &JournalKey {
        &self.key
    }
}

/// The states `cache journal list` serves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum JournalListState {
    /// Waiting for the drain.
    Pending,
    /// Sent to the phase lead (the default).
    #[default]
    Escalated,
}

/// `cache journal list [--state pending|escalated] [--unannounced]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct JournalList {
    /// The state listed (`applied` and `resolved` are `UNSUPPORTED`).
    #[schemars(with = "Option<String>")]
    pub(crate) state: JournalListState,
    /// Only the escalations not yet announced.
    pub(crate) unannounced: bool,
}

impl JournalList {
    /// The state listed (`applied` and `resolved` are `UNSUPPORTED`).
    #[must_use]
    pub fn state(&self) -> JournalListState {
        self.state
    }

    /// Only the escalations not yet announced.
    #[must_use]
    pub fn unannounced(&self) -> bool {
        self.unannounced
    }
}

/// `cache journal resolve <seq> --note T --actor A`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct JournalResolve {
    /// The escalated entry.
    #[schemars(with = "i64")]
    pub(crate) seq: Seq,
    /// The lead's note.
    pub(crate) note: String,
    /// Who resolves.
    #[schemars(with = "String")]
    pub(crate) actor: Actor,
}

impl JournalResolve {
    /// The escalated entry.
    #[must_use]
    pub fn seq(&self) -> Seq {
        self.seq
    }

    /// The lead's note.
    #[must_use]
    pub fn note(&self) -> &str {
        &self.note
    }

    /// Who resolves.
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }
}

/// `cache journal announce <seq>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct JournalAnnounce {
    /// The escalated entry.
    #[schemars(with = "i64")]
    pub(crate) seq: Seq,
}

impl JournalAnnounce {
    /// The escalated entry.
    #[must_use]
    pub fn seq(&self) -> Seq {
        self.seq
    }
}
