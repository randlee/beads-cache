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
//! 2. [`execute`], or one function per command in [`ops`], runs it over a
//!    [`Cache`](bd_cache::Cache) and returns the command's `data`
//!    ([`Data`]) or an [`ApiError`].
//! 3. The front end wraps the result in its own envelope. This crate has no
//!    sc-command and no clap: [`ApiError`] is `{kind, code, message,
//!    details, suggested_action}`, with kind, exit and retryable from
//!    [`ERROR_TABLE`] (REQ-BDC-0803).
//!
//! The filters, paging and orders of `issues list` and `ready` run here, on
//! top of `Cache::list(scope)` and `Cache::ready` (REQ-BDC-0808). The api
//! never opens a file: `context` and `cache status` take the facts of the
//! cache file and `metadata.json` as a [`CacheFacts`].
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

mod b1_stub;
pub mod data;
mod decode;
mod error;
mod filter;
pub mod ops;
mod page;
pub mod request;
mod v0;

#[doc(inline)]
pub use data::Data;
#[doc(inline)]
pub use decode::{NOT_SERVED_OPERATIONS, decode};
#[doc(inline)]
pub use error::{ApiError, CODE_PREFIX, ERROR_TABLE, ErrorKind, ErrorSpec, Unsupported, spec};
#[doc(inline)]
pub use ops::{CacheFacts, execute};
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
