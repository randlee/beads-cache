//! Pure bd-cache model: journal ops, replay, ports, configuration, replies and typed errors.
//!
//! This crate does no I/O (`docs/architecture.md`, Boundaries). It holds what
//! the store (`bd-cache-store`, a-4), the core (`bd-cache`, a-5 and a-6) and
//! the sync task (a-7) share:
//!
//! - [`Op`] (parse, canonical args JSON, overlay, achieved) and [`replay`],
//!   with [`normalize`], [`parse_precondition`] and [`unmet`]
//!   (REQ-BDC-0301, 0304, 0305);
//! - the journal types ([`JournalEntry`], [`NewEntry`], [`Seq`], [`IdemKey`]),
//!   the size limits on what is journaled ([`MAX_WRITE_ARGS_BYTES`],
//!   [`check_write_size`]) and store-side state;
//! - scopes as named bd filters ([`ScopeName`], [`ScopeId`], [`ScopeFilter`],
//!   [`Scope`]; ADR-BDC-0013) and the 0.1 compatibility rule
//!   ([`LEGACY_SCOPE_PREFIX`], [`legacy_scope`]);
//! - the phase-B sync's store-side types (ADR-STORE-0006): [`IngestSnapshot`],
//!   [`FailingSince`], [`LeaseHolder`], [`HookState`] and [`TickBudget`];
//! - the ports: [`CacheStore`] with [`DrainLock`], and [`EscalationSender`];
//! - [`RepoConfig`] (`.beads/bd-cache.toml`), [`CacheConfig`], [`DrainBudget`];
//! - reply and report shapes, [`CallKind`] / [`CallFailure`];
//! - [`CacheError`] with every `BD_CACHE.*` code and the backend mapping
//!   (REQ-BDC-0802, 0803).
//!
//! With the `testing` feature: `testing::FakeStore`, `testing::FakeDrainLock`
//! and the store contract suite every `CacheStore` must pass.
//!
//! # Example
//!
//! ```
//! use bd_cache_model::{Op, replay};
//! let op = Op::parse("close", r#"{"reason": "PASS"}"#).unwrap();
//! let row = sc_beads::testing::bead("b1");
//! let view = op.overlay(&row);
//! assert_eq!(view.status.as_deref(), Some("closed"));
//! assert!(op.achieved(&view) && !op.achieved(&row));
//! assert_eq!(op.to_args_json(), r#"{"reason": "PASS"}"#);
//! ```

#![deny(missing_docs)]

mod budget;
mod call;
mod compat;
mod config;
mod error;
mod ingest;
mod journal;
mod limits;
mod op;
mod pyjson;
mod reply;
mod scope;
mod sender;
mod state;
mod store;

#[cfg(feature = "testing")]
pub mod testing;

#[doc(inline)]
pub use budget::{
    DEFAULT_BUDGET_MS, DEFAULT_HOOK_RESERVE_MS, MAX_LEASE_TTL, TickBudget, check_lease_expiry,
};
#[doc(inline)]
pub use call::{CallFailure, CallKind};
#[doc(inline)]
pub use compat::{LEGACY_SCOPE_PREFIX, legacy_label, legacy_scope};
#[doc(inline)]
pub use config::{
    CacheConfig, ConfigError, DEFAULT_CALL_TIMEOUT, DEFAULT_MAX_ATTEMPTS,
    DEFAULT_MISS_FILL_TIMEOUT, DrainBudget, HARD_MARGIN, MAX_CALL_TIMEOUT_S, MAX_CONFIGURED_SCOPES,
    MAX_MAX_ATTEMPTS, MAX_MISS_FILL_TIMEOUT_MS, RepoConfig, START_GATE_MARGIN, SchemaPackageRef,
    ScopeConfigError, ScopeTable, TickConfig,
};
#[doc(inline)]
pub use error::{
    BackendCause, CacheError, ConfigOrigin, ENVELOPE_KEYS, GuardKey, IngestReason,
    MAX_KNOWN_SCOPES_LISTED, PreconditionReason, ScheduleReason, StoreCause, SyncCause,
    TYPED_ERROR_EXIT, TickRunReason, UNREACHABLE_CODES, Unmet,
};
#[doc(inline)]
pub use ingest::{IngestSnapshot, MAX_INGEST_BYTES, MAX_INGEST_ROWS, encoded_rows_within_limit};
#[doc(inline)]
pub use journal::{
    IdemKey, JournalEntry, JournalState, NewEntry, RESOLVE_SEPARATOR, Seq, error_code_prefix,
    resolve_note, split_resolve_note,
};
#[doc(inline)]
pub use limits::{
    MAX_IDEM_KEY_BYTES, MAX_NOTE_BYTES, MAX_WRITE_ARGS_BYTES, check_note_size, check_write_size,
};
#[doc(inline)]
pub use op::{
    CLOSED_STATUS, OPS, Op, meta_value, normalize, parse_precondition, phase_of, precondition_json,
    replay, unmet,
};
#[doc(inline)]
pub use pyjson::python_json;
#[doc(inline)]
pub use reply::{
    DrainReport, EscalatedItem, GetReply, JournalCounts, ListReply, PruneReport, ReadyBasis,
    ReadyReply, RowView, StateCounts, StatusReply, SyncReport, WriteOutcome, WriteReply,
};
#[doc(inline)]
pub use scope::{
    Criteria, FilterParts, MAX_FILTER_WORD_BYTES, MAX_FILTER_WORDS, MAX_REGISTERED_SCOPES,
    MAX_SCOPE_NAME_LEN, Scope, ScopeFilter, ScopeId, ScopeName,
};
#[doc(inline)]
pub use sender::{EscalationSender, MEMBER_NAME_RULE, SendError, SendErrorKind};
#[doc(inline)]
pub use state::{
    BaseEntry, FailingSince, FingerprintOutcome, Freshness, HookState, LeaseHolder, LeaseTake,
    MAX_HOOK_ERROR_BYTES, OpenWork, Outage, ReadyBase, RunId, ScopeStatus, StoreStatus,
    SyncOutcome, SyncState, UpsertReport, clip_hook_error,
};
#[doc(inline)]
pub use store::{CacheStore, DrainGuard, DrainLock};
