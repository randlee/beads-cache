//! Beads backend contract shared by beads-cache and beads-task-issue-tracker.
//!
//! This crate is the one source of truth for how Rust code talks to `bd`
//! (ADR-BDC-0007). It holds contracts and pure logic only; it spawns no
//! process and opens no file. Implementations live in `sc-bd-cli` (the `bd`
//! CLI) and later `sc-bd-http`.
//!
//! # Contents
//!
//! - **btit's contract, unchanged** (ported from
//!   randlee/beads-task-issue-tracker at `8d8e82ae9f85`): the data types of
//!   `btit-types` at the crate root ([`ProjectRef`], [`BdRawIssue`],
//!   [`ListQuery`], ...), and the modules of `btit-beads`: [`backend`]
//!   ([`BeadsBackend`], [`CliBackend`], [`DoltOperations`],
//!   [`CloseSuggestions`]), [`error`] ([`BeadsError`]), [`compat`],
//!   [`detect`], [`gates`], [`issues`], [`parse`] and [`logging`]. A btit
//!   consumer switches by changing `btit_types::` and `btit_beads::` imports
//!   to `sc_beads::`. The public surface is pinned by
//!   `tests/fixtures/btit-api.txt` and the ported API-freeze tests. The one
//!   deliberate change is that the `log_*!` macros emit through `tracing`
//!   (`logging::tracing` replaces `logging::log`), per ADR-BDC-0007.
//! - **The cache extension** ([`cache`]): [`CacheBackend`], a subtrait of
//!   [`BeadsBackend`] whose calls all take a [`Deadline`] and return
//!   [`BeadsCacheError`], plus the value types they use ([`BeadId`],
//!   [`Actor`], [`Edge`], [`Patch`], [`Guards`], [`Fingerprint`], ...).
//! - **Strict rows** ([`row`]): [`Bead`] and [`BlockedRow`], generated from the
//!   bd 1.3.1 `bd schema` fixture, with validators that name the bead and
//!   field of any violation.
//! - **Time** ([`clock`]): [`Clock`], [`SystemClock`], [`Deadline`] and
//!   RFC 3339 helpers.
//! - **Test doubles** (feature `testing`): [`testing::ManualClock`],
//!   [`testing::FakeBackend`] and the `CacheBackend` contract suite every
//!   implementation must pass.
//!
//! # Example
//!
//! ```
//! use std::time::Duration;
//! use sc_beads::{Clock, Deadline, SystemClock};
//!
//! let clock = SystemClock;
//! let call_cap = Deadline::after(&clock, Duration::from_secs(30));
//! let caller = Deadline::after(&clock, Duration::from_millis(500));
//! // A call is handed the earlier of the two.
//! let d = caller.earliest(call_cap);
//! assert!(d.remaining(&clock) <= Duration::from_millis(500));
//! ```

#![deny(missing_docs)]

// Declared first so the exported `log_*!` macros are in textual scope for every
// module below, exactly as in btit-beads.
#[macro_use]
pub mod logging;

pub mod backend;
pub mod cache;
pub mod clock;
pub mod compat;
pub mod detect;
pub mod error;
pub mod gates;
pub mod issues;
pub mod parse;
pub mod row;

#[cfg(feature = "testing")]
pub mod testing;

/// The data types of btit-types, re-exported at the crate root.
mod types {
    pub(crate) mod backend;
    pub(crate) mod cli;
    pub(crate) mod fs;
    pub(crate) mod issue;
    pub(crate) mod payload;
}

#[cfg(test)]
#[expect(
    clippy::uninlined_format_args,
    reason = "fixtures moved verbatim from btit-beads"
)]
mod test_support;

#[doc(inline)]
pub use backend::{BeadsBackend, CliBackend, CloseSuggestions, DoltOperations};
#[doc(inline)]
pub use cache::{
    Actor, BeadId, BeadsCacheError, BlockedRow, CacheBackend, Edge, Fingerprint, Guards, MetaValue,
    Patch, Refusal, ScopeFilter, SyncCursor,
};
#[doc(inline)]
pub use clock::{rfc3339_to_unix, unix_to_rfc3339, Clock, Deadline, SystemClock};
#[doc(inline)]
pub use error::{BeadsError, ExpectedShape, ParseTarget};
#[doc(inline)]
pub use row::{
    parse_bd_row, parse_bd_rows, parse_blocked_rows, schema_sha256, Bead, BeadComment, BondedFrom,
    DepType, BD_SCHEMA_SHA256, BD_SCHEMA_VERSION, BD_VERSION,
};
#[doc(inline)]
pub use types::backend::{DoltOpResult, ProjectRef, RelationType};
#[doc(inline)]
pub use types::cli::{
    BackendCapabilities, CliClient, CliOutput, CliProbe, CliVersion, CompatibilityInfo,
    ReleaseSource,
};
#[doc(inline)]
pub use types::fs::{DirectoryEntry, FsListResult, PurgeResult};
#[doc(inline)]
pub use types::issue::{
    BdRawComment, BdRawDependency, BdRawDependent, BdRawIssue, ChildIssue, Comment, CountResult,
    Issue, ParentIssue, Relation,
};
#[doc(inline)]
pub use types::payload::{CreatePayload, CwdOptions, ListOptions, ListQuery, UpdatePayload};
