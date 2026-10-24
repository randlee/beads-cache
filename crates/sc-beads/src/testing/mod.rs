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

use crate::cache::BeadId;
use crate::row::Bead;

/// The `created_at` / `updated_at` of rows made by [`bead`].
pub const BEAD_TIME: &str = "2026-10-23T09:00:00Z";

/// A minimal valid open bead: title = id, priority 2, type `task`, no labels.
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
pub fn bead(id: &str) -> Bead {
    Bead {
        id: BeadId::from(id),
        title: id.to_owned(),
        status: Some("open".into()),
        priority: 2,
        issue_type: Some("task".into()),
        created_at: BEAD_TIME.into(),
        updated_at: BEAD_TIME.into(),
        ..Bead::default()
    }
}

/// Recovers the data of a poisoned mutex: a panicking test thread must not
/// cascade into unrelated assertions.
pub(crate) fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
