//! Test doubles and the `CacheBackend` contract suite (feature `testing`).
//!
//! - [`ManualClock`]: a [`Clock`](crate::Clock) tests advance by hand.
//! - [`FakeBackend`]: a deterministic in-memory bd implementing
//!   [`CacheBackend`](crate::CacheBackend), with failure injection, per-call
//!   cost and a recorded command plan.
//! - [`contract`]: the suite every `CacheBackend` implementation must pass,
//!   with [`backend_contract_tests!`](crate::backend_contract_tests) to
//!   instantiate it as `#[test]` functions.
//! - [`bead`]: a minimal valid row to start a test world from.
//!
//! These types are for tests only: they hold their state behind mutexes and
//! treat a poisoned lock as recoverable.

mod clock;
pub mod contract;
mod fake;

#[doc(inline)]
pub use clock::ManualClock;
#[doc(inline)]
pub use fake::{FakeBackend, FakeCall, FakeOp, Injection};

use crate::cache::{BeadId, Edge};
use crate::row::{Bead, DepType};

/// The `created_at` / `updated_at` of rows made by [`bead`].
pub const BEAD_TIME: &str = "2026-10-23T09:00:00Z";

/// A minimal valid open bead: title = id, priority 2, type `task`, no labels.
///
/// # Panics
///
/// When `id` is not a valid [`BeadId`] (a test bug).
///
/// # Examples
///
/// ```
/// let b = sc_beads::testing::bead("b1");
/// assert_eq!(b.status.as_deref(), Some("open"));
/// let json = serde_json::to_value(&b).unwrap();
/// assert_eq!(sc_beads::parse_bd_row(&json).unwrap(), b);
/// ```
#[must_use]
#[expect(clippy::panic, reason = "an invalid fixture id is a test bug")]
pub fn bead(id: &str) -> Bead {
    let row = serde_json::json!({
        "id": id,
        "title": id,
        "status": "open",
        "priority": 2,
        "issue_type": "task",
        "created_at": BEAD_TIME,
        "updated_at": BEAD_TIME,
    });
    crate::row::parse_bd_row(&row).unwrap_or_else(|e| panic!("testing::bead({id:?}): {e}"))
}

/// A [`BeadId`] from a fixture string.
///
/// # Panics
///
/// When `s` is not a valid bead id (a test bug).
#[must_use]
#[expect(clippy::panic, reason = "an invalid fixture id is a test bug")]
pub fn id(s: &str) -> BeadId {
    BeadId::new(s).unwrap_or_else(|e| panic!("testing::id: {e}"))
}

/// An [`Edge`] `from -> to` from fixture strings.
///
/// # Panics
///
/// When either id is invalid (a test bug).
#[must_use]
pub fn edge(from: &str, to: &str, kind: DepType) -> Edge {
    Edge::new(id(from), id(to), kind)
}

/// Recovers the data of a poisoned mutex: a panicking test thread must not
/// cascade into unrelated assertions.
pub(crate) fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
