# sc-cron Architecture

Status: ADR-CRON-0001..0006 are Approved (lead ruling 2026-10-24, a-3 QA-1).
ADR-CRON-0007 and 0008 are Approved (Rand's rulings, 2026-10-25), which also amends
ADR-CRON-0006 (its amendment below). Domain code: `CRON`. Requirements:
[requirements.md](requirements.md). Governing repo-level decision:
[ADR-BDC-0006](../architecture.md#adr-bdc-0006-prototype-a-sqlite-backed-cron-crate-here-for-sc-runtime).
Created by sprint a-3, which lists ADR-CRON-0001..0006 in its `adrs`; extended
by sprint b-6 (ADR-CRON-0007, ADR-CRON-0008).

## Purpose

`sc-cron` is a generic SQLite-backed cron scheduler, extracted from an earlier
in-house scheduler. It carries over that scheduler's scheduling behaviour and
leaves out everything specific to its host: hooks, teams, phases, its sealed
scheduler boundary and its error type. It depends on no workspace crate, so it
can move to sc-runtime unchanged (ANC-005, REQ-CRON-0011). `bd-cache-sync` (a-7) builds the bd-cache
sync schedule on it, and from b-7 also runs the consumer's Python sync method
and post-sync hook through its Python hook runner (REQ-CRON-0013).

## Module layout

| Module | Contents |
|---|---|
| `clock` | `Clock`, `SystemClock`; `ManualClock` (feature `test-utils`); monotonic sleep helper |
| `jitter` | `JitterSource`, `SystemJitter`; `SeqJitter` (feature `test-utils`); the jitter bound |
| `expr` | `CronExpr`: parse (five-field crontab, six/seven fields, `@` macros), future-occurrence check, `next_run` |
| `types` | `ScheduleId`, `Generation`, `ScheduleName`, `RunTimeout`, `ScheduleSpec`, `NewSchedule`, `Schedule` (with `enabled`), `RunOutcome`, `StopDecision` |
| `timestamp` | `Timestamp`, the validated instant on the store port |
| `limits` | `MAX_NAME_LEN`, `MAX_EXPR_LEN`, `MAX_PAYLOAD_BYTES`, `MAX_TIMEOUT`; the payload check |
| `error` | `CronError`, `StoreError`, `StoreErrorKind` |
| `store` | the `ScheduleStore` port |
| `sqlite` | `SqliteScheduleStore`, `SCHEMA_VERSION` |
| `job` | the `Job` trait |
| `hook` | `HookTarget`, `PythonHook`, `HookEvent`, `HookCall`, `HookOutput`, `HookStatus`, `kill_budget`: the Python hook runner and its protocol |
| `process` | the supervised child (crate-private): process group or job object, capped output |
| `scheduler` | `Scheduler`, `MissedTickPolicy`, `WorkerHandle` |
| `worker` | the worker task (crate-private) |

Third-party dependencies: `async-trait`, `chrono`, `cron` 0.17, `getrandom`,
`process-wrap` (hook process group and job object, ADR-CRON-0008),
`rusqlite` (bundled), `serde_json`, `tempfile` (a hook's working directory),
`tokio` (with `process` and `io-util`), `tracing`. The `sc-cron` row of
[docs/architecture.md](../architecture.md) lists the same set.

## Public contract

This is the implemented contract, and it supersedes the sketch the sprint
doc first carried (ADR-CRON-0006). The sprint doc's Public contract section
points here. The doc comments in the crate are the full reference.

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
pub struct CronExpr;      // parse / FromStr, as_str; 5 fields (crontab), 6-7 fields, @macros
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
    pub ended_at: Option<Timestamp>, pub stop_reason: Option<String>,
    pub enabled: bool }                             // is_active()
pub enum RunOutcome { Ok, Refused(String), Failed(String), TimedOut }
pub enum StopDecision { Continue, Disable(String), Delete(String),
    Stop(String) /* alias of Delete, ADR-CRON-0007 */ }

// Store port: stores read no clock
pub trait ScheduleStore: Send + Sync {
    fn create(&self, schedule: &NewSchedule, created_at: Timestamp) -> Result<Schedule, CronError>;
    fn get(&self, id: ScheduleId) -> Result<Option<Schedule>, CronError>;
    fn list_active(&self) -> Result<Vec<Schedule>, CronError>;
    fn record_run(&self, id: ScheduleId, generation: Generation, at: Timestamp, outcome: &RunOutcome) -> Result<bool, CronError>;
    fn set_next(&self, id: ScheduleId, generation: Generation, next_run_at: Timestamp) -> Result<bool, CronError>;
    fn stop(&self, id: ScheduleId, generation: Generation, at: Timestamp, reason: &str) -> Result<bool, CronError>;
    // set_next also returns false for a disabled schedule (REQ-CRON-0014)
    fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<bool, CronError>; // only that column
    fn disable(&self, id: ScheduleId, generation: Generation, at: Timestamp, reason: &str) -> Result<bool, CronError>;
}
pub struct SqliteScheduleStore;                     // open(path), open_in_memory()
pub const SCHEMA_VERSION: i64 = 2;                  // a version 1 file gains `enabled` on open

#[async_trait]
pub trait Job: Send + Sync {
    async fn run(&self, payload: &serde_json::Value, deadline: Instant) -> RunOutcome;
    async fn should_stop(&self, payload: &serde_json::Value) -> StopDecision;
}

// Hook targets (REQ-CRON-0013, ADR-CRON-0008)
pub enum HookTarget { Rust(Arc<dyn Job>), Python(PythonHook) }
pub struct PythonHook;    // new(interpreter, module) -> Result, interpreter(), module(),
                          // async invoke(&clock, &HookCall) -> Result<HookOutput, CronError>
pub enum HookEvent { Tick, Stop }                   // as_str: "cron.tick", "cron.stop"
pub struct HookCall<'a> { pub event: HookEvent, pub schedule: &'a ScheduleName,
    pub payload: &'a serde_json::Value, pub timeout: RunTimeout, pub deadline: Instant }
pub struct HookOutput { pub status: HookStatus, pub stdout: String, pub stderr: String,
    pub stdout_truncated: bool, pub stderr_truncated: bool }
pub enum HookStatus { Exited(ExitStatus), TimedOut }
pub fn kill_budget(timeout: Duration, deadline: Instant, now: Instant) -> Duration;
pub const HOOK_PROTOCOL: &str = "sc-cron.hook.v1";
pub const MAX_HOOK_OUTPUT_BYTES: usize = 65536;

pub enum MissedTickPolicy { Coalesce, Skip }        // default Coalesce
impl<S: ScheduleStore, C: Clock, J: JitterSource> Scheduler<S, C, J> {
    pub fn new(store: Arc<S>, clock: Arc<C>, jitter: Arc<J>) -> Self;
    pub fn with_missed_tick_policy(self, policy: MissedTickPolicy) -> Self;
    pub fn store(&self) -> &Arc<S>;
    pub fn start(&self, spec: ScheduleSpec) -> Result<Schedule, CronError>;
    pub async fn start_async(&self, spec: ScheduleSpec) -> Result<Schedule, CronError>;
    pub fn stop(&self, id: ScheduleId, generation: Generation, reason: &str) -> Result<(), CronError>;
    pub async fn stop_async(&self, id: ScheduleId, generation: Generation, reason: &str) -> Result<(), CronError>;
    pub fn set_enabled(&self, name: &ScheduleName, enabled: bool) -> Result<(), CronError>;
    pub async fn set_enabled_async(&self, name: &ScheduleName, enabled: bool) -> Result<(), CronError>;
    pub fn spawn_worker(&self, job: Arc<dyn Job>) -> WorkerHandle;       // HookTarget::Rust(job)
    pub fn spawn_worker_for(&self, target: HookTarget) -> WorkerHandle;
}
impl WorkerHandle {
    pub async fn shutdown(self, deadline: tokio::time::Instant) -> bool;
    pub fn is_finished(&self) -> bool;
}

// Errors
#[non_exhaustive]
pub enum CronError { BadExpression(String), InvalidSpec(String), AlreadyActive(ScheduleName),
    NotFound(ScheduleName), HookFailed(String),
    Store(StoreError), Fenced { id: ScheduleId, generation: Generation } }   // code(), recovery(), source()
pub struct StoreError;    // new(kind, context), with_source(e), kind(), context()
#[non_exhaustive]
pub enum StoreErrorKind { Busy, Corrupt, SchemaMismatch, Unavailable, Other }   // as_str(), recovery()
```

## Data model

One table, `sc_cron_schedule`, in a database file of its own
(`PRAGMA user_version` = `SCHEMA_VERSION`):

| Column | Meaning |
|---|---|
| `id` | stable id, kept when an ended name is restarted |
| `generation` | fencing generation, 1..=u32::MAX |
| `name` | unique, 1..=128 bytes (CHECK) |
| `expr`, `timeout_ms`, `jitter_ms`, `payload` (JSON text, <= 64 KiB) | the spec and its stored jitter |
| `created_at` | when (re)started |
| `next_run_at` | set exactly while active (CHECK) |
| `last_run_at`, `last_outcome` | last recorded run (`ok`, `refused: m`, `failed: m`, `timed_out`) |
| `ended_at`, `stop_reason` | null exactly while active (CHECK) |
| `enabled` | 1 or 0 (CHECK), default 1; a disabled active schedule never fires (REQ-CRON-0014) |

`SCHEMA_VERSION` is 2. A version 1 file has no `enabled` column; opening it
runs `ALTER TABLE ... ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1 CHECK
(enabled IN (0, 1))` and sets version 2 in the same immediate transaction, so
every existing schedule stays enabled.

Timestamps are written as RFC 3339 UTC text with a fixed six-digit fraction
(`Timestamp`'s `Display`), so they sort in time order. Run-state updates are
guarded by `id = ? AND generation = ? AND ended_at IS NULL`; `set_next` adds
`AND enabled = 1`, so a disabled row is refused its next run, which makes a
worker that has not reloaded notice before it launches. `set_enabled` is
guarded by `name = ? AND ended_at IS NULL` and sets only `enabled`; `disable`
(the stop check's answer) is fenced like a run-state write and also stores the
reason as the last outcome, `Refused(reason)`. A value that
does not decode is a `StoreErrorKind::Corrupt` error naming the row.

## Worker

`spawn_worker` starts one tokio task. It holds one slot per active schedule:
the row (with its parsed `CronExpr`), the next run, and at most one run in
flight with its monotonic deadline. Each pass, checking for shutdown
between slots:

1. Reload from `list_active` if signalled (by `start`, `stop`,
   `set_enabled` or a fenced write), a retry is due, or the disabled-row poll
   is due. A disabled row gets no
   slot, so it never fires and a run in flight for it is cancelled. A slot
   whose `(id, generation)` is unchanged and still enabled is kept with its
   run in flight. Any other old slot is dropped, which cancels
   its run. If shutdown is requested mid-reload, no new slot is built but
   unchanged slots are still kept, so their runs reach the drain. A failed
   reload (including a corrupt row) is logged with its code and retried
   after 1 s, doubling to 60 s (the shift is capped, so it never
   overflows). The same retry rebuilds a slot lost to a store error: when the
   `stop` or `disable` write a stop check asked for returns an error (not
   `false`), the slot is already dropped but the row is still active and
   enabled, so the worker schedules a reload and the row's slot comes back.
   The disabled-row poll: a reload that finds an active, disabled row sets the
   next reload due one interval later (`Scheduler::with_disabled_poll`, default
   30 s, minimum 1 s, on the injected clock), as does a successful stop-check
   `disable`; a reload that finds none clears it. Another process's
   `set_enabled(true)` sends no signal and a disabled row has no slot, so
   this poll is the only way the worker notices it, at most one interval after
   the write. The poll adds the cost of one `list_active` per interval, and
   only while a disabled row exists.
2. Cancel every run whose deadline has passed on `Clock::monotonic_now`,
   and record `TimedOut`.
3. For each due slot: compute and store the next run (a fenced `set_next`
   skips the launch and reloads at once), then skip it if a run is in
   flight, skip it if missed under `Skip`, or launch it.
4. Sleep on `Clock::sleep_until` until the earliest next run, run
   deadline (converted from monotonic to wall time), reload retry or
   disabled-row poll, unless a reload signal, a finished run or shutdown comes
   first.

A launched run is its own task: the stop check, then `run` with the
deadline, for a Rust target; for a Python target the `cron.stop` hook, then the
`cron.tick` hook, each started with the run's deadline and killed at
`kill_budget` (ADR-CRON-0008). The stop check's answer is acted on by the
worker: `Continue` runs, `Disable` stores the flag and reason (fenced) and drops
the slot, `Delete` ends the schedule (fenced) and drops the slot. A stop check
that panics, times out or, for Python, fails or answers malformed, skips the run
and records `Failed` or `TimedOut`; the slot stays. The worker records its
result. The worker is the only caller of
`sleep_until`, so `ManualClock`'s parks, each tagged with the time it saw,
tell a test that the worker has settled at a given time. Store calls run on
tokio's blocking pool.

## Architecture Decision Records

## ADR-CRON-0001: Generic job payload and an sc-cron error type

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-1)

Context: the scheduler `sc-cron` was extracted from ran its host's hooks: a
schedule carried a team name, a phase id, a module path, cwd, args and
`custom` data, a tick was a hook request, a reply's actions went to an action
dispatcher, and every failure was the host's error type. ADR-BDC-0006 requires
`sc-cron` to carry none of that.

Decision: a schedule's input is one opaque `serde_json::Value` payload
(at most `MAX_PAYLOAD_BYTES`), handed to a caller-supplied `Job` (`run`
with a deadline, `should_stop`) on every tick. The earlier scheduler's two
stop conditions (`Until` and the stop hook) become `Job::should_stop`, so a
time limit is a payload field the job checks. Outcomes are
`RunOutcome { Ok, Refused, Failed, TimedOut }`. Its hook-specific `Actions`,
`Malformed`, `ActionErr` and `Exit` fold into `Ok` or `Failed`. Failures are one `#[non_exhaustive] enum CronError` with a stable
`code()` (`BAD_EXPRESSION`, `INVALID_SPEC`, `ALREADY_ACTIVE`,
`STORE_FAILED`, `FENCED`) and `recovery()` text. A store failure is a
`StoreError`: a `StoreErrorKind` (`Busy`, `Corrupt`, `SchemaMismatch`,
`Unavailable`, `Other`), a context naming the operation and row, and the
backend's error kept as `Error::source`. No backend type appears in the
port. `Fenced` carries the id and generation it was refused for.

Amendment (sprint b-6): `CronError` gains `NotFound` (`NOT_FOUND`) and
`HookFailed` (`HOOK_FAILED`), and the payload can also be handed to a Python
module (ADR-CRON-0008).

Consequences: bd-cache-sync puts its scope and budget in the payload and
implements `Job`. A caller can retry on `Busy` and alert on `Corrupt`
without parsing text. A store outside this crate builds its errors with
`StoreError::new(..).with_source(..)`.

## ADR-CRON-0002: Clock carries monotonic time and sleeping

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-1)

Context: the sprint contract sketch gave `Clock` only `now()`. A worker that
can only read the time cannot wait for a manual clock to advance, and a
job's `Instant` deadline needs a monotonic timeline that a manual clock also
controls.

Decision: `Clock` also has `monotonic_now()` (default `Instant::now()`) and
`async fn sleep_until(target)`, as the earlier scheduler's `Clock` does. `SystemClock`
sleeps in slices of at most 30 s and re-reads the wall clock between them.
`ManualClock` moves both timelines together and never moves monotonic time
back. `jump_wall` steps wall time alone, to model an NTP step. Each park is
recorded with the time the sleeper saw, and `advance_settled` waits for a
park at the new time, so an earlier park cannot satisfy it.

Consequences: the trait uses `async-trait`. A job compares its deadline with
`Clock::monotonic_now`; under `SystemClock` that is `Instant::now`.

## ADR-CRON-0003: Persisted next run; missed ticks coalesce by default

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-1)

Context: the earlier scheduler recomputed every next run from now at startup,
so a tick missed across a restart was never replayed, while a tick missed
during a suspend fired once. ADR-BDC-0006 asks for missed-tick replay.

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

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-1)

Context: the earlier scheduler stored a whole-second jitter per schedule and
spread it over offsets not held by other schedules with the same expression (a
product ruling there). The contract's `JitterSource::jitter(max)` returns a
`Duration`.

Decision: `start` draws one jitter in `[0, max)` with
`max = min(period / 2, 30 min)`, clamps an out-of-range draw to `max`,
truncates it to whole milliseconds and stores it. Every occurrence fires at
`occurrence + jitter`. The held-offset spreading is not carried over.
Random offsets spread load statistically, and intervals stay regular.

Consequences: two schedules with one expression may draw the same offset.
That is harmless, and a caller that cares can pick distinct names and
expressions.

## ADR-CRON-0005: The worker enforces run deadlines on the monotonic clock

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-1)

Context: a run must not delay the next tick, and a timed-out run must stop
working. With tokio timers inside the run task, a manual clock could not
drive timeouts, and the worker would not be the clock's only sleeper. A
deadline measured on the wall clock would be cut short by a forward
wall-clock jump.

Decision: each run's deadline is `monotonic_now() + timeout`, the same
`Instant` the job receives. The worker converts the remaining span to a
wall-clock wake time for `sleep_until`, then compares against
`monotonic_now()` before timing anything out. At the deadline the worker
aborts the run task and records `TimedOut`. Dropping an in-flight run (a
reload, a stop, a restart or an aborted worker) also aborts it, and an
aborted run records nothing. Cancellation is cooperative: the job passes
the deadline into any `spawn_blocking` work, which an abort cannot cancel.
A job must not block a runtime thread, and the hard timeout guarantee needs
a multi-thread runtime (documented on `Job` and in the crate docs).
Timeouts are `RunTimeout`: 1 ms to 24 h, in whole milliseconds.

Consequences: timeouts are testable under `ManualClock`, including a wall
jump. A job that blocks a runtime thread without yielding cannot be
cancelled; that limit is documented rather than worked around.

## ADR-CRON-0006: Implemented contract and store

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-1)

Context: the contract sketch's `create(spec, created_at)` and `Schedule`
fields left nowhere to keep the jitter or first run, to read a run's outcome
back, or to read an ended schedule. Instants and names crossed the port as
raw strings, and were re-validated at each layer. Edition 2024 reserves
`gen`. NFR-BDC-0007 gives the cache file one connection per process.

Decision:

- Validated values: `Timestamp` (UTC, microsecond precision) for every
  instant on the port and in `Schedule`, rendered to text only at the SQL
  boundary. `ScheduleName`, `CronExpr` and `RunTimeout` are the one
  definition of a valid name, expression and timeout. The payload size has
  one check, shared by `ScheduleSpec::new`, `Scheduler::start` and the
  store. The limits are named constants. The active schedule count is not
  capped: callers create every schedule and each row is bounded.
- `create` takes a `NewSchedule { spec, jitter, next_run_at }`, so the store
  stays clock-free. `create` on an ended name restarts it in place (same
  id, generation + 1), which is what makes fencing reachable. `Schedule`
  adds `jitter`, `created_at`, `last_outcome` and `stop_reason`. The port
  adds `get(id)`. The generation parameter is named `generation`.
- `Scheduler` adds `new`, `with_missed_tick_policy`, `stop`, `store`, and
  `start_async`/`stop_async`, which run the synchronous store calls on the
  blocking pool. `WorkerHandle::shutdown` bounds the drain.
- `SqliteScheduleStore` uses its own database file and connection, never a
  second connection to another store's file (NFR-BDC-0007 as amended). It
  sets the busy timeout before WAL and checks `PRAGMA user_version`
  (`SCHEMA_VERSION`), refusing an unknown version or a file that already
  holds other tables.

Consequences: a-7 codes against the signatures above. The sprint doc's
Public contract points here.

Amendment (sprint b-6): the contract gains the `enabled` column and flag
(`Schedule::enabled`, `ScheduleStore::set_enabled` and `disable`,
`Scheduler::set_enabled`), `SCHEMA_VERSION` becomes 2 with an in-place upgrade
of a version 1 file, and `CronError` gains `NotFound` and `HookFailed`. See
REQ-CRON-0014, ADR-CRON-0007 and ADR-CRON-0008.

Amendment (PE-RSH-007): `SqliteScheduleStore::list_active` decodes rows one
at a time. A row that fails to decode (`Corrupt`: bad expression, payload
that is not JSON, unknown outcome, bad timestamp) is skipped with a
`tracing::warn` naming its id, name and the error, once per row id per store
instance, and the other schedules are returned and keep running. The row is
left as is, not ended, so it can be repaired. `get(id)` still returns the
`Corrupt` error for that row. Busy, I/O and other non-corrupt failures still
fail the whole call.

## ADR-CRON-0007: Stop decision is continue, disable or delete

**Status:** Approved (Rand's rulings, 2026-10-25)

Context: the stop check could only end a schedule (`Stop(reason)`), and a
schedule could only be ended or left running. A consumer wants a check that
pauses a schedule without losing it, and ANC-006 asks for an enabled flag so a
task can be switched off without rewriting its row. ANC-007 asks for a stop
hook that answers `continue`, `disable` or `delete`.

Decision: `StopDecision` is `Continue`, `Disable(reason)` and `Delete(reason)`.
`Disable` sets `enabled = 0` through the fenced `ScheduleStore::disable`, keeps
the row active with its generation, expression and payload, and stores the
reason as the last outcome, `Refused(reason)`. `Delete` is the old `Stop`: it
ends the schedule with the reason (`ended_at`, `stop_reason`). The row is kept
as the ended record rather than removed, because the id and generation it
holds are what let a restart raise the generation and fence a stale run
(ADR-CRON-0006); a removed row would let a restart start over at generation 1.
A Python stop hook answers `{"action": "continue" | "disable" | "delete",
"reason": "..."}`; `reason` is required for the last two, so a reply that
lost its reason is refused, not acted on. A stop check that cannot answer
(panic, timeout, and for Python a failed start, non-zero exit or malformed
reply) is no decision: the run is skipped and the schedule left active and
enabled, with `Failed` or `TimedOut` recorded.

`StopDecision::Stop(reason)` is kept as an alias of `Delete(reason)`. Its one
caller outside this crate, `bd-cache-sync`'s stop rule, is owned by sprint b-7,
which removes that rule; keeping the alias lets the workspace build between b-6
and b-7. It is to be deleted when no caller is left.

Consequences: a disabled schedule is not asked its stop check again until it is
enabled. A job that wants a time limit still checks it in the payload
(ADR-CRON-0001) and answers `Delete`.

## ADR-CRON-0008: Python hooks run as supervised child processes

**Status:** Approved (Rand's rulings, 2026-10-25)

Context: sc-cron must stay general (ANC-005, REQ-CRON-0011), yet a schedule
should be able to run a Python module instead of a Rust `Job`, and b-7 needs
one runner for the consumer's Python sync method and the post-sync hook with
every call killed at its timeout (NFR-BDC-0004). `forbid(unsafe_code)` is set
for the crate, and the kill must reach processes the hook starts.

Decision:

- `HookTarget` is `Rust(Arc<dyn Job>)` or `Python(PythonHook)`, where
  `PythonHook` holds the interpreter and the module. `spawn_worker_for` takes a
  target; `spawn_worker(job)` is `HookTarget::Rust(job)`. The target is held by
  the worker, not stored in the row: a Rust closure cannot be stored, and the
  row stays the opaque spec of ADR-CRON-0001. A caller that wants a row per
  target keeps the choice in its payload.
- A Python target runs `<interpreter> -I -m <module>` (`-I` isolates it from the
  caller's environment, user site and the working directory) from a new
  temporary directory per call, with one JSON object on stdin
  (`protocol`, `event`, `schedule`, `payload`, `deadline`) and the events
  `cron.stop` and `cron.tick`. No name in the protocol is beads-specific. The
  interpreter is an absolute path or a bare command name and the module a
  dotted identifier path, validated once; a relative path with directories
  would be resolved against the temporary directory.
- Every hook is killed at `kill_budget(timeout, deadline, now)`, the row's timeout
  capped by the time left before the run's deadline, on the clock's monotonic
  timeline; the deadline on stdin is the same instant as a UTC time. The wait
  uses the tokio timer. The same kill happens when the run is cancelled, since
  the supervisor kills on drop. A deadline already passed starts nothing.
- The kill takes the whole tree: the child is the leader of a Unix process
  group (killed with `SIGKILL` to the group) or inside a Windows job object
  (terminated, and killed when the job handle closes). `process-wrap` provides
  both with the platform calls inside it, which keeps the crate free of
  `unsafe`; the alternative, calling `libc` and `windows-sys` directly, would
  need `forbid(unsafe_code)` relaxed.
- stdout and stderr are each read to the end and kept up to
  `MAX_HOOK_OUTPUT_BYTES`; stdin is written while they are read, so a hook that
  ignores its input or floods its output cannot stall the supervisor.
- A `cron.tick` hook succeeds by exit 0, otherwise `Failed`; a kill is
  `TimedOut`.

Consequences: a hook should write its answer to stdout and its diagnostics to
stderr. A grandchild that leaves the group or job on purpose (a daemon calling
`setsid`) escapes the kill; if it also keeps the hook's stdout open, the run
lasts until its timeout. The tests use a stub interpreter (a script) on
Unix; the Windows job-object path is checked by compiling it for
`x86_64-pc-windows-msvc`.

## Build note

`sc-cron` is a workspace member (`members = ["crates/*"]`). It inherits the
package fields, `[workspace.dependencies]` and `[workspace.lints]`, plus
crate-level `forbid(unsafe_code)` and `warn(unreachable_pub)`. A dev-dependency
on itself with `test-utils` means `cargo test --workspace` runs every test.
It is validated with the workspace commands in the sprint doc
(`docs/plans/phase-a/sprint-a-3-sc-cron.md`).
Test lint configuration (lead rulings R14 and R15, a-8 fix round 6 and QA-8): `clippy.toml`
`allow-*-in-tests`; the src test-module `#[allow]` blocks are removed; the integration
tests `tests/expr.rs`, `tests/sqlite_store.rs` and `tests/worker.rs` use crate-level
`#![expect(..., reason)]`.
