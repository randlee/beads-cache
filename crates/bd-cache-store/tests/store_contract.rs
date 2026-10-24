//! a-1's store contract suite against `SqliteStore` + `FlockDrainLock` (ADR-BDC-0004, NFR-BDC-0005).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod common;

bd_cache_model::store_contract_tests!(common::SqliteFixture::new());
