---
id: a-3
phase: a
status: complete
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
  owns: [REQ-CRON-0001, REQ-CRON-0002, REQ-CRON-0003, REQ-CRON-0004, REQ-CRON-0005, REQ-CRON-0006, REQ-CRON-0007, REQ-CRON-0008, REQ-CRON-0009, REQ-CRON-0010, REQ-CRON-0011, REQ-CRON-0012]
  constrained_by: [REQ-BDC-0703, NFR-BDC-0004, NFR-BDC-0007]
adrs: [ADR-BDC-0006, ADR-CRON-0001, ADR-CRON-0002, ADR-CRON-0003, ADR-CRON-0004, ADR-CRON-0005, ADR-CRON-0006]
owned_paths:
  - "crates/sc-cron/**"
  - "docs/sc-cron/**"
---

# Sprint a-3 — sc-cron prototype

## Outcome and closure

A SQLite-backed cron crate with no consumer or bd-cache types, extracted from
the first consumer's cron scheduler (its scheduler crate, store port and
rusqlite store, and its ADR-069; the source paths are in git history).
It owns no repo-level bd-cache requirement: it is a generic scheduler
governed by ADR-BDC-0006, and a-7 meets the tick-budget requirements on top
of it. It owns the crate-level requirements REQ-CRON-0001..0012
([docs/sc-cron/requirements.md](../../sc-cron/requirements.md)) and the
crate ADRs ADR-CRON-0001..0006
([docs/sc-cron/architecture.md](../../sc-cron/architecture.md)).
Closes when the scheduler, store port and rusqlite store pass their tests
with a manual clock. a-3 depends on nothing in phase A and may start in
wave 1.

## Decision prerequisites

- Generic job payload and an `sc-cron` error type in place of the first consumer's
  scheduler types (`HookRequest`, `TeamName`, `PhaseId` and its error type). The choice is ADR-CRON-0001
  in `docs/sc-cron/architecture.md`.
- Runtime: tokio, as the first consumer's scheduler. A job receives its run deadline and must
  honour it: blocking work runs under `spawn_blocking` with the same deadline
  passed down, so a timed-out run never keeps working past it.

## Public contract

Normative: the implemented contract in
[docs/sc-cron/architecture.md, Public contract](../../sc-cron/architecture.md#public-contract),
recorded by ADR-CRON-0006. It supersedes the sketch this section first
carried. The current signatures are copied here:

```rust
// Time and randomness
#[async_trait]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn monotonic_now(&self) -> Instant { Instant::now() }
    async fn sleep_until(&self, target: DateTime<Utc>);
}
pub struct SystemClock;
pub struct ManualClock;   // test-utils: new, advance, set, jump_wall, parks,
                          // wait_parks_above, wait_parked_at, advance_settled
pub trait JitterSource: Send + Sync { fn jitter(&self, max: Duration) -> Duration; }
pub struct SystemJitter;
pub struct SeqJitter;     // test-utils: new, push, calls

// Validated values
pub struct Timestamp;     // from_datetime, as_datetime, FromStr, Display (fixed-width RFC 3339)
pub struct ScheduleName;  // new / FromStr, as_str
pub struct CronExpr;      // parse / FromStr, as_str
pub struct RunTimeout;    // new(Duration), get
pub const MAX_NAME_LEN: usize = 128;
pub const MAX_EXPR_LEN: usize = 256;
pub const MAX_PAYLOAD_BYTES: usize = 65536;
pub const MAX_TIMEOUT: Duration = 24 h;

pub struct ScheduleId(u64);                         // new, get
pub struct Generation(NonZeroU32);                  // FIRST, new, get, next
pub struct ScheduleSpec { pub name: ScheduleName, pub expr: CronExpr, pub timeout: RunTimeout,
    pub payload: serde_json::Value }                // ScheduleSpec::new(&str, &str, Duration, Value)
pub struct NewSchedule { pub spec: ScheduleSpec, pub jitter: Duration, pub next_run_at: Timestamp }
pub struct Schedule { pub id: ScheduleId, pub generation: Generation, pub spec: ScheduleSpec,
    pub jitter: Duration, pub created_at: Timestamp, pub next_run_at: Option<Timestamp>,
    pub last_run_at: Option<Timestamp>, pub last_outcome: Option<RunOutcome>,
    pub ended_at: Option<Timestamp>, pub stop_reason: Option<String> }   // is_active()
pub enum RunOutcome { Ok, Refused(String), Failed(String), TimedOut }
pub enum StopDecision { Continue, Stop(String) }

// Store port: stores read no clock
pub trait ScheduleStore: Send + Sync {
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError>;
    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError>;
    fn list_active(&self) -> Result<Vec<Schedule>, CronError>;
    fn record_run(&self, id: ScheduleId, generation: Generation, at: Timestamp, outcome: &RunOutcome) -> Result<bool, CronError>;
    fn set_next(&self, id: ScheduleId, generation: Generation, next_run_at: Timestamp) -> Result<bool, CronError>;
    fn stop(&self, id: ScheduleId, generation: Generation, at: Timestamp, reason: &str) -> Result<bool, CronError>;
}
pub struct SqliteScheduleStore;                     // open(path), open_in_memory()
pub const SCHEMA_VERSION: i64 = 1;

#[async_trait]
pub trait Job: Send + Sync {
    async fn run(&self, payload: &serde_json::Value, deadline: Instant) -> RunOutcome;
    async fn should_stop(&self, payload: &serde_json::Value) -> StopDecision;
}

pub enum MissedTickPolicy { Coalesce, Skip }        // default Coalesce
impl<S: ScheduleStore, C: Clock, J: JitterSource> Scheduler<S, C, J> {
    pub fn new(store: Arc<S>, clock: Arc<C>, jitter: Arc<J>) -> Self;
    pub fn with_missed_tick_policy(self, policy: MissedTickPolicy) -> Self;
    pub fn store(&self) -> &Arc<S>;
    pub fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError>;
    pub async fn start_async(&self, spec: ScheduleSpec) -> Result<Schedule, CronError>;
    pub fn stop(&self, id: ScheduleId, generation: Generation, reason: &str) -> Result<(), CronError>;
    pub async fn stop_async(&self, id: ScheduleId, generation: Generation, reason: &str) -> Result<(), CronError>;
    pub fn spawn_worker(&self, job: Arc<dyn Job>) -> WorkerHandle;
}
impl WorkerHandle {
    pub async fn shutdown(self, deadline: tokio::time::Instant) -> bool;
    pub fn is_finished(&self) -> bool;
}

// Errors
#[non_exhaustive]
pub enum CronError { BadExpression(String), InvalidSpec(String), AlreadyActive(ScheduleName),
    Store(StoreError), Fenced { id: ScheduleId, generation: Generation } }   // code(), recovery(), source()
pub struct StoreError;    // new(kind, context), with_source(e), kind(), context()
#[non_exhaustive]
pub enum StoreErrorKind { Busy, Corrupt, SchemaMismatch, Unavailable, Other }   // as_str(), recovery()
```

## Deliverables

1. Scheduler: validation and parse before write, next-run computation,
   jitter, missed-tick replay, stop check before every run, per-run timeout,
   one run in flight, bounded shutdown (REQ-CRON-0001..0006, 0012;
   ADR-CRON-0003, 0004, 0005).
2. `ScheduleStore` port with `(id, generation)` fencing; stores read no
   clock (REQ-CRON-0007, 0008; ADR-CRON-0006).
3. `SqliteScheduleStore` (bundled SQLite, own file and connection, schema
   version) (REQ-CRON-0009; NFR-BDC-0007; ADR-CRON-0006).
4. `Clock` and `JitterSource` with system and test implementations
   (REQ-CRON-0010; ADR-CRON-0002).
5. Generic payload and `CronError` with stable codes; no consumer or bd-cache
   types (REQ-CRON-0011; ADR-CRON-0001).
6. Per-crate docs `docs/sc-cron/requirements.md` (REQ-CRON-0001..0012) and
   `architecture.md` (ADR-CRON-0001..0006, approved by the lead).
7. Tests ported from the first consumer's scheduler (`tests/expr.rs`, `tests/worker.rs` and the
   store tests), where they apply without the consumer's types (REQ-CRON-0001..0012).

## Acceptance criteria

| Criterion | Requirement | Root |
|---|---|---|
| An invalid expression or spec is refused before anything is stored | REQ-CRON-0001 | ADR-BDC-0006 |
| Missed ticks are replayed per policy; the stop check runs before every run | REQ-CRON-0004, REQ-CRON-0005 | ADR-BDC-0006, ADR-CRON-0003 |
| A run from an older generation cannot record its result (`record_run` returns false) | REQ-CRON-0007 | ADR-BDC-0006 |
| A run that exceeds its timeout is recorded as `TimedOut`, the job saw the deadline, and the next tick is not delayed | REQ-CRON-0005 | ADR-BDC-0006, ADR-CRON-0005 |
| The schedule store uses its own SQLite file and connection | REQ-CRON-0009 | NFR-BDC-0007, ADR-CRON-0006 |
| `cargo tree -p sc-cron` shows no bd-cache, sc-beads or consumer crate | REQ-CRON-0011 | ADR-BDC-0006 |

## This sprint does not close

The bd-cache sync schedule and its tick budget (a-7).

## Required validation

`sc-cron` is a workspace member (a-1's workspace) and inherits edition,
license and dependencies with `workspace = true`. Validation runs workspace
wide:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo clippy --workspace --lib -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace --all-features
cargo build --workspace
cargo tree -p sc-cron -e normal
```
