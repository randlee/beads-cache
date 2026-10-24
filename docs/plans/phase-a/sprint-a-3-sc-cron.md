---
id: a-3
phase: a
status: planned
closure_type: boundary
target_boundary: sc-cron
track: cron
wave: 1-2 (no inbound edge)
branch: sprint/a-3-sc-cron
worktree: sprint/a-3-sc-cron
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [a-1, a-2, a-4, a-5, a-6]
requirements:
  owns: []
  constrained_by: [REQ-BDC-0703, NFR-BDC-0004]
adrs: [ADR-BDC-0006]
owned_paths:
  - "crates/sc-cron/**"
  - "docs/plans/phase-a/notes/a-3.md"
---

# Sprint a-3 — sc-cron prototype

## Outcome and closure

A SQLite-backed cron crate with no ATM or bd-cache types, extracted from
atm-core's `atm-cron` (`crates/atm-cron`; store port
`crates/atm-storage/src/cron_schedule_store.rs`; rusqlite store
`crates/atm-storage-rusqlite/src/cron_schedule_store.rs`; ADR-069 there).
It owns no bd-cache requirement: it is a generic scheduler governed by
ADR-BDC-0006, and a-7 meets the tick-budget requirements on top of it.
Closes when the scheduler, store port and rusqlite store pass their tests
with a manual clock. a-3 depends on nothing in phase A and may start in
wave 1.

## Decision prerequisites

- Generic job payload and an `sc-cron` error type in place of atm-cron's
  `HookRequest`, `TeamName`, `PhaseId`, `AtmError`. The choice goes to
  `docs/plans/phase-a/notes/a-3.md`; a-7 folds it into ADR-BDC-0006.
- Runtime: tokio, as atm-cron. Blocking job work runs outside the scheduler
  (callers use `spawn_blocking`).

## Public contract

```rust
pub trait Clock: Send + Sync { fn now(&self) -> chrono::DateTime<chrono::Utc>; }
pub struct SystemClock; pub struct ManualClock;          // ManualClock behind `test-utils`
pub trait JitterSource: Send + Sync { fn jitter(&self, max: std::time::Duration) -> std::time::Duration; }
pub struct SystemJitter; pub struct SeqJitter;           // SeqJitter behind `test-utils`

pub struct ScheduleId(u64);
pub struct Generation(std::num::NonZeroU32);             // fencing: a stale run cannot record
pub struct ScheduleSpec { pub name: String, pub expr: String, pub timeout: std::time::Duration,
    pub payload: serde_json::Value }
pub struct Schedule { pub id: ScheduleId, pub generation: Generation, pub spec: ScheduleSpec,
    pub next_run_at: Option<String>, pub last_run_at: Option<String>, pub ended_at: Option<String> }
pub enum RunOutcome { Ok, Refused(String), Failed(String), TimedOut }
pub enum StopDecision { Continue, Stop(String) }

/// Stores read no clock: every timestamp is an argument.
pub trait ScheduleStore: Send + Sync {
    fn create(&self, spec: &ScheduleSpec, created_at: &str) -> Result<Schedule, CronError>;
    fn list_active(&self) -> Result<Vec<Schedule>, CronError>;
    fn record_run(&self, id: ScheduleId, gen: Generation, at: &str, outcome: &RunOutcome) -> Result<bool, CronError>; // false = fenced
    fn set_next(&self, id: ScheduleId, gen: Generation, next_run_at: &str) -> Result<bool, CronError>;
    fn stop(&self, id: ScheduleId, gen: Generation, at: &str, reason: &str) -> Result<bool, CronError>;
}
pub struct SqliteScheduleStore; // rusqlite, bundled

#[async_trait::async_trait]
pub trait Job: Send + Sync {
    async fn run(&self, payload: &serde_json::Value, deadline: std::time::Instant) -> RunOutcome;
    async fn should_stop(&self, payload: &serde_json::Value) -> StopDecision;
}
pub struct Scheduler<S: ScheduleStore, C: Clock, J: JitterSource> { /* ... */ }
impl<S: ScheduleStore, C: Clock, J: JitterSource> Scheduler<S, C, J> {
    pub fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError>; // parses expr before any write
    pub fn spawn_worker(&self, job: std::sync::Arc<dyn Job>) -> WorkerHandle;
}
pub enum CronError { BadExpression(String), Store(String), Fenced }
```

## Deliverables

1. Scheduler: parse before write, next-run computation, jitter, missed-tick
   replay, stop check before every run, per-run timeout.
2. `ScheduleStore` port with `(id, generation)` fencing; stores read no
   clock.
3. `SqliteScheduleStore` (bundled SQLite).
4. `Clock` and `JitterSource` with system and test implementations.
5. Tests ported from atm-cron's `tests/expr.rs`, `tests/worker.rs` and the
   store tests, where they apply without ATM types.

## Acceptance criteria

| Criterion | Root |
|---|---|
| An invalid expression is refused before anything is stored | ADR-BDC-0006 |
| Missed ticks are replayed per policy; the stop check runs before every run | ADR-BDC-0006 |
| A run from an older generation cannot record its result (`record_run` returns false) | ADR-BDC-0006 |
| A run that exceeds its timeout is recorded as `TimedOut` and does not delay the next tick | ADR-BDC-0006 |
| `cargo tree -p sc-cron` shows no bd-cache, sc-beads or ATM crate | ADR-BDC-0006 |

## This sprint does not close

The bd-cache sync schedule and its tick budget (a-7).

## Required validation

Crate tests and lints; `cargo build --workspace`.
