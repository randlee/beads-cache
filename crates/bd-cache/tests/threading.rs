//! ADR-CORE-0001: a `Cache` is `Send` but never `Sync`, whatever its store
//! is (B7Q1-025): the `FakeStore` here is `Send + Sync`, as `SqliteStore` is.

use bd_cache::Cache;
use bd_cache_model::testing::{FakeDrainLock, FakeStore};
use sc_beads::testing::FakeBackend;

type TestCache = Cache<FakeStore, FakeDrainLock, FakeBackend>;

fn assert_send<T: Send>() {}
fn assert_send_sync<T: Send + Sync>() {}

/// Resolves only when `T` is not `Sync`: for a `Sync` type both impls
/// apply and the call below is ambiguous, which fails to compile.
trait AmbiguousIfSync<A> {
    fn check() {}
}
impl<T: ?Sized> AmbiguousIfSync<()> for T {}
/// The second impl's marker.
struct IsSync;
impl<T: ?Sized + Sync> AmbiguousIfSync<IsSync> for T {}

#[test]
fn a_cache_is_send_and_not_sync_over_a_sync_store() {
    assert_send_sync::<FakeStore>();
    assert_send::<TestCache>();
    <TestCache as AmbiguousIfSync<_>>::check();
}
