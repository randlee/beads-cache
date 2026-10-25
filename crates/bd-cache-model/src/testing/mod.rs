//! Test doubles and the store contract suite (feature `testing`).
//!
//! - [`FakeStore`]: an in-memory [`CacheStore`](crate::CacheStore) that keeps
//!   every rule of the port. Clones share one store, so a "reopened" store and
//!   a second `Cache` on the same file see the same data.
//! - [`FakeDrainLock`]: clones share one lock, so two caches contend.
//! - [`contract`]: the suite every `CacheStore` + `DrainLock` must pass, with
//!   [`store_contract_tests!`](crate::store_contract_tests).

pub mod contract;
mod store;

#[doc(inline)]
pub use store::{FakeDrainLock, FakeStore};

pub(crate) fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
