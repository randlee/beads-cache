---
id: a-3
phase: a
status: planned
closure_type: boundary
target_boundary: sc-cron
track: cron
wave: 1
branch: sprint/a-3-sc-cron
worktree: sprint/a-3-sc-cron
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [a-1, a-2, a-4, a-5]
baseline_sections:
  - "BD-CACHE.md: Cron module (schedule, timeouts, stop)"
adrs: [ADR-BDC-0006]
owned_paths:
  - "crates/sc-cron/**"
---

# Sprint a-3 — sc-cron prototype

## Outcome and closure

A SQLite-backed cron crate with no ATM or bd-cache types, extracted from
atm-core's `atm-cron` (`crates/atm-cron`, store port
`crates/atm-storage/src/cron_schedule_store.rs`, rusqlite store
`crates/atm-storage-rusqlite/src/cron_schedule_store.rs`; ADR-069 there).
Closes when the scheduler, store port and rusqlite store pass their tests
with a manual clock.

## Decision prerequisites

- Generic job payload (`serde_json::Value` or a type parameter) and an
  `sc-cron` error type in place of `HookRequest`, `TeamName`, `PhaseId`,
  `AtmError`. Record the choice in ADR-BDC-0006.
- Runtime: atm-cron is tokio-based; keep it, and keep blocking job work out
  of the scheduler (callers use `spawn_blocking`).

## Deliverables

1. Scheduler: expression parse before write, next-run computation, jitter,
   missed-tick replay, stop check before every run, per-run timeout.
2. Schedule store port (stores read no clock; timestamps are arguments) with
   `(id, generation)` fencing so a stale run never writes over a restarted
   schedule.
3. rusqlite store implementation (bundled SQLite).
4. `Clock` with `SystemClock` and `ManualClock`, `JitterSource` with a
   deterministic test source.
5. Tests ported from atm-cron's `tests/expr.rs`, `tests/worker.rs` and the
   store tests, where they apply without ATM types.

## Acceptance criteria

| Criterion | Root |
|---|---|
| Invalid expressions are refused before anything is stored | ADR-BDC-0006 |
| Missed ticks are replayed per the policy; stop is checked before every run | ADR-BDC-0006 |
| A run from an older generation cannot record its result | ADR-BDC-0006 |
| No dependency on any bd-cache or ATM crate (`cargo tree -p sc-cron`) | ADR-BDC-0006 |

## This sprint does not close

- The bd-cache sync schedule (a-6).

## Required validation

Crate tests and lints; `cargo build --workspace`.
