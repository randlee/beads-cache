//! a-1's `CacheBackend` contract suite against `BdCli` (REQ-BDC-0006, ADR-BDC-0004).
//!
//! `BdCli` runs over `FakeBd`, an invoker that parses the argv `BdCli` built, applies
//! it to sc-beads' `FakeBackend` and answers with bd's output and exit status. Every
//! rule of the suite therefore passes through the real argv builders, the real output
//! parsing and the real error mapping.

use std::sync::Arc;

use sc_bd_cli::testing::FakeBd;
use sc_bd_cli::BdCli;
use sc_beads::testing::contract::{BackendFixture, World};
use sc_beads::testing::ManualClock;

struct BdCliFixture;

impl BackendFixture for BdCliFixture {
    type Backend = BdCli;

    fn backend(&mut self, world: &World, clock: Arc<ManualClock>) -> BdCli {
        let fake = FakeBd::new(Arc::clone(&clock));
        fake.backend().insert_all(world.rows.iter().cloned());
        BdCli::with_invoker(Box::new(fake)).with_clock(clock)
    }
}

sc_beads::backend_contract_tests!(BdCliFixture);
