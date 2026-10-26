//! bd-cache's contract vocabulary: requests, data, the one decoder, the operations and the errors.
//!
//! Every front end (the `bdc` CLI, the Python bindings and, later, the MCP
//! server) goes through this crate (contract §1 rule 6, §9;
//! `docs/bd-cache-cli/contract.md`), so the same input gives the same
//! `data` or the same error code everywhere:
//!
//! 1. [`decode`] turns a JSON request for a [`Command`] into a typed
//!    [`Request`]. Validation lives here: an unknown member is `BAD_ARGS`
//!    with `details.member`, a `/v0` name the cache does not serve is
//!    `UNSUPPORTED`, and a write without `idempotency_key` is refused
//!    (REQ-BDC-0807, REQ-BDC-0004).
//! 2. [`execute`] runs it over a [`Cache`](bd_cache::Cache) and returns the
//!    command's `data` ([`Data`]) or an [`ApiError`].
//!
//! **Decode, then execute, is the only path** (REQ-API-0001). A [`Request`]
//! and its per-command structs cannot be built or changed outside this
//! crate: their fields are private (read them through accessors), and the
//! per-command operations are crate-private. b-3 (the CLI), b-4 (Python)
//! and the later MCP server each turn their input into JSON, call
//! [`decode`], then [`execute`]:
//!
//! ```
//! # use std::sync::Arc;
//! # use bd_cache::Cache;
//! # use bd_cache_model::{CacheConfig, ScopeId, testing::{FakeDrainLock, FakeStore}};
//! # use sc_beads::testing::{FakeBackend, ManualClock};
//! use bd_cache_api::{CacheFacts, Command, decode, execute};
//! # let clock = Arc::new(ManualClock::new("2026-10-23T10:00:00Z"));
//! # let config = CacheConfig::new(sc_beads::ProjectRef::local(None), vec![bd_cache_model::Scope::legacy(ScopeId::new("x").unwrap())]);
//! # let mut cache = Cache::new(config, FakeStore::new(), FakeDrainLock::new(),
//! #     FakeBackend::new(Arc::clone(&clock)), clock).unwrap();
//! let req = decode(Command::IssuesList, &serde_json::json!({"scope": "x"})).unwrap();
//! // Never synced: the scope is not served (lead ruling S67).
//! let err = execute(&mut cache, &req, &CacheFacts::default()).unwrap_err();
//! assert_eq!(err.code(), "BD_CACHE.SCOPE_UNKNOWN");
//! ```
//!
//! A typed request cannot be written by hand, so it cannot skip the
//! decoder's checks (here, `status: "closed"` in a patch):
//!
//! ```compile_fail
//! use bd_cache_api::request::{IssuePatch, IssuesUpdate};
//! let _ = IssuesUpdate {
//!     id: sc_beads::BeadId::new("b1").unwrap(),
//!     actor: sc_beads::Actor::new("san").unwrap(),
//!     idempotency_key: bd_cache_model::IdemKey::new("k").unwrap(),
//!     patch: IssuePatch { status: Some("closed".into()), ..IssuePatch::default() },
//!     expected_status: None,
//!     expected_assignee: None,
//! };
//! ```
//!
//! nor changed after decoding:
//!
//! ```compile_fail
//! use bd_cache_api::{Command, Request, decode};
//! let req = serde_json::json!({"id": "b1", "actor": "san", "idempotency_key": "k",
//!     "patch": {"status": "open"}});
//! let Ok(Request::IssuesUpdate(mut u)) = decode(Command::IssuesUpdate, &req) else { panic!() };
//! u.patch.status = Some("closed".into());
//! ```
//!
//! and the per-command operations are not public:
//!
//! ```compile_fail
//! use bd_cache_api::ops::issues_update;
//! ```
//! 3. The front end wraps the result in its own envelope. This crate has no
//!    sc-command and no clap: [`ApiError`] is `{kind, code, message,
//!    details, suggested_action}`, with kind, exit and retryable from
//!    [`ERROR_TABLE`] (REQ-BDC-0803).
//!
//! The filters, paging and orders of `issues list` and `ready` run here, on
//! top of `Cache::list(scope)` and `Cache::ready` (REQ-BDC-0808). The api
//! never opens a file: `context` and `cache status` take the facts of the
//! cache file and `metadata.json` as a [`CacheFacts`], and `cache health`
//! runs through [`execute_health`] with the sync rows' state ([`SyncFacts`]).
//!
//! # Example
//!
//! ```
//! use bd_cache_api::{Command, decode};
//! let req = serde_json::json!({"id": "b1", "actor": "san", "patch": {"status": "in_progress"}});
//! let err = decode(Command::IssuesUpdate, &req).unwrap_err();
//! assert_eq!(err.code(), "BD_CACHE.BAD_ARGS");
//! assert_eq!(err.details()["member"], "idempotency_key");
//! ```

#![deny(missing_docs)]

pub mod data;
mod decode;
mod error;
mod filter;
mod ops;
mod page;
pub mod request;
mod v0;

#[doc(inline)]
pub use data::Data;
#[doc(inline)]
pub use decode::decode;
#[doc(inline)]
pub use error::{ApiError, CODE_PREFIX, ERROR_TABLE, ErrorKind, ErrorSpec, Unsupported, spec};
#[doc(inline)]
pub use ops::{CacheFacts, SyncFacts, SyncRowFacts, SyncRows, execute, execute_health};
#[doc(inline)]
pub use page::{Cursor, ListSort};
#[doc(inline)]
pub use request::{Command, Request};
#[doc(inline)]
pub use v0::{entry_args, patch_args};

/// The version of `docs/bd-cache-cli/contract.md` this crate implements
/// (REQ-BDC-0811); `context` and `cache status` report it.
pub const CONTRACT_VERSION: &str = "1.0.0";

/// The `/v0` capability tokens the cache serves (`context.capabilities`, contract §5.2).
pub const CAPABILITIES: [&str; 7] = [
    "issues.get",
    "issues.list",
    "ready.list",
    "dependencies.list",
    "issues.update",
    "issues.close",
    "dependencies.add",
];
