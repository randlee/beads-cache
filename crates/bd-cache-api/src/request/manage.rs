//! Surface (b) requests: the `cache` management commands (contract §6).

use bd_cache_model::{IdemKey, ScopeId, Seq};
use sc_beads::{Actor, BeadId};
use schemars::JsonSchema;

/// `cache status`: no members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CacheStatus;

/// `cache health [--probe]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CacheHealth {
    /// Also run the backend check (fingerprint and the rebuild rule).
    pub probe: bool,
}

/// `cache scopes`: no members.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CacheScopes;

/// `cache refresh scopes [S…]`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct CacheRefreshScopes {
    /// The scopes to sync, in request order; empty syncs every known scope.
    #[schemars(with = "Vec<String>")]
    pub scopes: Vec<ScopeId>,
}

/// `cache refresh issues ID…`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct CacheRefreshIssues {
    /// The beads to re-read from bd (nonempty).
    #[schemars(with = "Vec<String>")]
    pub ids: Vec<BeadId>,
    /// The read's timeout; absent takes the configured miss-fill timeout.
    pub timeout_ms: Option<u64>,
}

/// `cache flush [--start-gate-ms N] [--deadline-ms M]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct CacheFlush {
    /// No entry starts after this many milliseconds (default 25,000).
    pub start_gate_ms: u64,
    /// No bd call runs past this many milliseconds (default 28,000; above `start_gate_ms`).
    pub deadline_ms: u64,
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
    pub scope: ScopeId,
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
    pub key: JournalKey,
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
    pub state: JournalListState,
    /// Only the escalations not yet announced.
    pub unannounced: bool,
}

/// `cache journal resolve <seq> --note T --actor A`.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
pub struct JournalResolve {
    /// The escalated entry.
    #[schemars(with = "i64")]
    pub seq: Seq,
    /// The lead's note.
    pub note: String,
    /// Who resolves.
    #[schemars(with = "String")]
    pub actor: Actor,
}

/// `cache journal announce <seq>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, JsonSchema)]
pub struct JournalAnnounce {
    /// The escalated entry.
    #[schemars(with = "i64")]
    pub seq: Seq,
}
