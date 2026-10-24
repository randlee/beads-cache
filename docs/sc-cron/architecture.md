# sc-cron Architecture

Status: draft. Domain code: `CRON`. Requirements:
[requirements.md](requirements.md). Governing repo-level decision:
[ADR-BDC-0006](../architecture.md#adr-bdc-0006-prototype-a-sqlite-backed-cron-crate-here-for-sc-runtime).

## Purpose

`sc-cron` is a generic SQLite-backed cron scheduler, extracted from
atm-core's `atm-cron` (`crates/atm-cron`, store port
`crates/atm-storage/src/cron_schedule_store.rs`, rusqlite store
`crates/atm-storage-rusqlite/src/cron_schedule_store.rs`, ADR-069 there). It
carries over atm-cron's scheduling behaviour and leaves out everything that
is ATM's: hooks, teams, phases, the sealed `CronScheduler` boundary and
`AtmError`. It depends on no workspace crate, so it can move to sc-runtime
unchanged. `bd-cache-sync` (a-7) builds the bd-cache sync schedule on it.

## Module layout

| Module | Contents |
|---|---|
| `clock` | `Clock`, `SystemClock`; `ManualClock` (feature `test-utils`) |
| `jitter` | `JitterSource`, `SystemJitter`; `SeqJitter` (feature `test-utils`); the jitter bound |
| `expr` | expression parse (refuses five fields and no future occurrence) and `next_run` |
| `types` | `ScheduleId`, `Generation`, `ScheduleSpec`, `NewSchedule`, `Schedule`, `RunOutcome`, `StopDecision` |
| `error` | `CronError` |
| `store` | the `ScheduleStore` port |
| `sqlite` | `SqliteScheduleStore` |
| `job` | the `Job` trait |
| `scheduler` | `Scheduler`, `MissedTickPolicy`, `WorkerHandle`, `MAX_TIMEOUT` |
| `worker` | the worker task (crate-private) |
| `timestamp` | the fixed-width RFC 3339 text form (crate-private) |

Third-party dependencies: `async-trait`, `chrono`, `cron` 0.17, `getrandom`,
`rusqlite` (bundled), `serde_json`, `tokio`, `tracing`.

## Public contract

The sprint a-3 contract
([sprint-a-3-sc-cron.md](../plans/phase-a/sprint-a-3-sc-cron.md#public-contract))
is normative. ADR-CRON-0002 and ADR-CRON-0006 record where the code extends
it and why.

```rust
#[async_trait]
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn monotonic_now(&self) -> Instant { Instant::now() }
    async fn sleep_until(&self, target: DateTime<Utc>);
}
pub trait JitterSource: Send + Sync { fn jitter(&self, max: Duration) -> Duration; }

pub trait ScheduleStore: Send + Sync {
    fn create(&self, schedule: &NewSchedule, created_at: &str) -> Result<Schedule, CronError>;
    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError>;
    fn list_active(&self) -> Result<Vec<Schedule>, CronError>;
    fn record_run(&self, id: ScheduleId, generation: Generation, at: &str, outcome: &RunOutcome) -> Result<bool, CronError>;
    fn set_next(&self, id: ScheduleId, generation: Generation, next_run_at: &str) -> Result<bool, CronError>;
    fn stop(&self, id: ScheduleId, generation: Generation, at: &str, reason: &str) -> Result<bool, CronError>;
}

#[async_trait]
pub trait Job: Send + Sync {
    async fn run(&self, payload: &serde_json::Value, deadline: Instant) -> RunOutcome;
    async fn should_stop(&self, payload: &serde_json::Value) -> StopDecision;
}

impl Scheduler<S, C, J> {
    pub fn new(store: Arc<S>, clock: Arc<C>, jitter: Arc<J>) -> Self;
    pub fn with_missed_tick_policy(self, policy: MissedTickPolicy) -> Self;
    pub fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError>;
    pub fn stop(&self, id: ScheduleId, generation: Generation, reason: &str) -> Result<(), CronError>;
    pub fn spawn_worker(&self, job: Arc<dyn Job>) -> WorkerHandle;
    pub fn store(&self) -> &Arc<S>;
}
impl WorkerHandle { pub async fn shutdown(self, deadline: tokio::time::Instant) -> bool; }

#[non_exhaustive]
pub enum CronError { BadExpression(String), InvalidSpec(String), AlreadyActive(String), Store(String), Fenced }
```

## Data model

One table, `sc_cron_schedule`, created on open if missing:

| Column | Meaning |
|---|---|
| `id` | stable id, kept when an ended name is restarted |
| `generation` | fencing generation, 1..=u32::MAX |
| `name` | unique |
| `expr`, `timeout_ms`, `jitter_ms`, `payload` (JSON text) | the spec and its stored jitter |
| `created_at` | when (re)started |
| `next_run_at` | next run; null once ended (CHECK) |
| `last_run_at`, `last_outcome` | last recorded run (`ok`, `refused: m`, `failed: m`, `timed_out`) |
| `ended_at`, `stop_reason` | null exactly while active (CHECK) |

All timestamps are RFC 3339 UTC text with a fixed six-digit fraction, so
they sort in time order. Run-state updates are guarded by
`id = ? AND generation = ? AND ended_at IS NULL`.

## Worker

`spawn_worker` starts one tokio task. It holds one slot per active schedule:
row, parsed expression, next run, at most one run in flight. Each pass:

1. Reload from `list_active` if signalled (by `start`, `stop` or a fenced
   write) or a retry is due. A slot whose `(id, generation)` is unchanged is
   kept with its run in flight. Any other old slot is dropped, which cancels
   its run. A failed reload is retried after 1 s, doubling to 60 s.
2. Cancel every run past its deadline and record `TimedOut`.
3. For each due slot: compute and store the next run (a fenced `set_next`
   skips the launch and reloads), then skip it if a run is in flight, skip it
   if missed under `Skip`, or launch it.
4. Sleep on `Clock::sleep_until` until the earliest next run or run
   deadline, unless a reload signal, a finished run or shutdown comes first.

A launched run is its own task: `should_stop`, then `run` with the
deadline. The worker records its result. The worker is the only caller of
`sleep_until`, so `ManualClock`'s park count tells a test that the worker
has settled. Store calls run on tokio's blocking pool.

## Architecture Decision Records

## ADR-CRON-0001: Generic job payload and an sc-cron error type

**Status:** Proposed

Context: atm-cron runs ATM hooks: a schedule carries `TeamName`, `PhaseId`,
module path, cwd, args and `custom`, a tick is a `HookRequest`, a reply's
actions go to a `HookActionDispatcher`, and every failure is an `AtmError`.
ADR-BDC-0006 requires `sc-cron` to carry none of that.

Decision: a schedule's input is one opaque `serde_json::Value` payload,
handed to a caller-supplied `Job` (`run` with a deadline, `should_stop`)
on every tick. atm-cron's two stop conditions (`Until` and the stop hook)
become `Job::should_stop`, so a time limit is a payload field the job
checks. Outcomes are `RunOutcome { Ok, Refused, Failed, TimedOut }`.
atm-cron's `Actions`, `Malformed`, `ActionErr` and `Exit` are hook-specific
and fold into `Ok` or `Failed`. Failures are one
`#[non_exhaustive] enum CronError` with string context, so no backend's
error type leaks through the port.

Consequences: bd-cache-sync puts its scope and budget in the payload and
implements `Job`. Callers that need richer outcomes encode them in
`Failed`/`Refused` text. `Store(String)` drops the source error chain, which
is acceptable for a prototype; it can become a structured variant later
without breaking callers, since the enum is non-exhaustive.

## ADR-CRON-0002: Clock carries monotonic time and sleeping

**Status:** Proposed

Context: the sprint contract gives `Clock` only `now()`. A worker that can
only read the time cannot wait for a manual clock to advance, and a job's
`Instant` deadline needs a monotonic timeline that a manual clock also
controls.

Decision: `Clock` also has `monotonic_now()` (default `Instant::now()`) and
`async fn sleep_until(target)`, as atm-cron's `Clock` does. `SystemClock`
sleeps in slices of at most 30 s and re-reads the wall clock between them.
`ManualClock` moves both timelines together, never moves monotonic time
back, and counts parks so tests can wait for the worker to settle.

Consequences: the trait uses `async-trait`. A job compares its deadline with
`Clock::monotonic_now`; under `SystemClock` that is `Instant::now`.

## ADR-CRON-0003: Persisted next run; missed ticks coalesce by default

**Status:** Proposed

Context: atm-cron recomputes every next run from now at startup, so a tick
missed across a restart is never replayed (ADR-069 D2), while a tick missed
during a suspend fires once. ADR-BDC-0006 asks for missed-tick replay.

Decision: the store keeps `next_run_at`, written before each launch. An
occurrence is missed when the worker sees it only after the following
occurrence is also due. `MissedTickPolicy::Coalesce` (default) replays all
missed occurrences as one run. `MissedTickPolicy::Skip` drops them. A late
but not missed run always runs. The policy is set on the `Scheduler` and
applies the same way across restarts and suspends.

Consequences: a sync schedule catches up once after downtime instead of
waiting a full period. A run interrupted by a crash is not replayed, since
its next run was stored before launch.

## ADR-CRON-0004: One fixed jitter per schedule

**Status:** Proposed

Context: atm-cron stores a whole-second jitter per schedule and spreads it
over offsets not held by other schedules with the same expression (an ATM
product ruling). The sprint contract's `JitterSource::jitter(max)` returns a
`Duration`.

Decision: `start` draws one jitter in `[0, max)` with
`max = min(period / 2, 30 min)`, clamps an out-of-range draw to `max`, and
stores it in milliseconds. Every occurrence fires at
`occurrence + jitter`. The held-offset spreading is not carried over. Random
offsets spread load statistically, and intervals stay regular.

Consequences: two schedules with one expression may draw the same offset.
That is harmless, and a caller that cares can pick distinct names and
expressions.

## ADR-CRON-0005: The worker enforces run deadlines on the injected clock

**Status:** Proposed

Context: a run must not delay the next tick, and a timed-out run must stop
working. With tokio timers inside the run task, a manual clock could not
drive timeouts, and the worker would not be the clock's only sleeper.

Decision: each run's wall deadline (`launch + timeout`) is part of the
worker's wake time. At the deadline the worker aborts the run task and
records `TimedOut`. Dropping an in-flight run (a reload, a stop, a restart
or an aborted worker) also aborts it, and an aborted run records nothing.
The job receives the deadline on the monotonic timeline, and passes it into
any `spawn_blocking` work, which an abort cannot cancel. Timeouts are
limited to 1 ms..=24 h (`MAX_TIMEOUT`).

Consequences: timeouts are testable under `ManualClock`. A job that blocks
a runtime thread without yielding cannot be cancelled. This is documented
on `Job`.

## ADR-CRON-0006: Store contract additions

**Status:** Proposed

Context: the sprint contract's `create(spec, created_at)` and `Schedule`
fields leave nowhere to keep the jitter or first run, to read a run's
outcome back, or to read an ended schedule. Edition 2024 reserves `gen`.

Decision: `create` takes a `NewSchedule { spec, jitter, next_run_at }`, so
the store stays clock-free. `create` on an ended name restarts it in place,
with the same id and generation + 1, which is what makes fencing reachable.
`Schedule` adds `jitter`, `created_at`, `last_outcome` and `stop_reason`.
The port adds `get(id)`. `CronError` adds `InvalidSpec` and `AlreadyActive`.
The generation parameter is named `generation`. `Scheduler` adds `new`,
`with_missed_tick_policy`, `stop` and `store`. `WorkerHandle::shutdown`
bounds the drain.

Consequences: a-7 codes against these signatures, not the sketch in the
sprint doc. The additions are needed for the sprint's acceptance criteria,
and none removes anything from the sketch.

## Build note

Until sprint a-1's workspace root lands, `crates/sc-cron/Cargo.toml` is
self-contained (explicit versions, no `workspace = true`), and is built with
`cargo test --manifest-path crates/sc-cron/Cargo.toml --all-features`. Once
a-1 merges, an amendment moves its dependencies to
`[workspace.dependencies]` and switches the crate to workspace inheritance.
