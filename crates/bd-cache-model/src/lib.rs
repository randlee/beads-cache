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
//!   scopes ([`ScopeId`]) and store-side state;
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

mod call;
mod config;
mod error;
mod journal;
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
pub use call::{CallFailure, CallKind};
#[doc(inline)]
pub use config::{
    CacheConfig, ConfigError, DEFAULT_CALL_TIMEOUT, DEFAULT_MAX_ATTEMPTS,
    DEFAULT_MISS_FILL_TIMEOUT, DrainBudget, HARD_MARGIN, RepoConfig, START_GATE_MARGIN,
    SchemaPackageRef, TickConfig,
};
#[doc(inline)]
pub use error::{
    BackendCause, CacheError, GuardKey, PreconditionReason, StoreCause, SyncCause,
    TYPED_ERROR_EXIT, Unmet,
};
#[doc(inline)]
pub use journal::{IdemKey, JournalEntry, JournalState, NewEntry, Seq};
#[doc(inline)]
pub use op::{
    CLOSED_STATUS, OPS, Op, meta_value, normalize, parse_precondition, phase_of, precondition_json,
    replay, unmet,
};
#[doc(inline)]
pub use pyjson::python_json;
#[doc(inline)]
pub use reply::{
    DrainReport, EscalatedItem, GetReply, ListReply, PruneReport, ReadyBasis, ReadyReply, RowView,
    StatusReply, SyncReport, WriteReply,
};
#[doc(inline)]
pub use scope::{PhaseName, ScopeId};
#[doc(inline)]
pub use sender::{EscalationSender, MEMBER_NAME_RULE, SendError, SendErrorKind};
#[doc(inline)]
pub use state::{
    BaseEntry, FingerprintOutcome, Freshness, OpenWork, Outage, ReadyBase, ScopeStatus,
    StoreStatus, SyncState, UpsertReport,
};
#[doc(inline)]
pub use store::{CacheStore, DrainGuard, DrainLock};
