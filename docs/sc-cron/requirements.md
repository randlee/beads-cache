# sc-cron Requirements

Status: approved. REQ-CRON-0001..0012 are Approved (lead ruling 2026-10-24, a-3 QA-2).

Domain code: `CRON`. Owned by sprint a-3 (`requirements.owns` in
[sprint-a-3-sc-cron.md](../plans/phase-a/sprint-a-3-sc-cron.md)).
`sc-cron` is the prototype cron scheduler of
[ADR-BDC-0006](../architecture.md#adr-bdc-0006-prototype-a-sqlite-backed-cron-crate-here-for-sc-runtime),
extracted from atm-core's `atm-cron` (ADR-069 there). It owns no repo-level
requirement. It is constrained by
[REQ-BDC-0703](../requirements.md#req-bdc-0703-tick-budget) and
[NFR-BDC-0004](../requirements.md#nfr-bdc-0004-bd-calls-bounded-by-the-deadline),
which a-7 (`bd-cache-sync`) meets on top of REQ-CRON-0005: the sync
schedule's timeout is the tick budget, and the deadline the job receives is
the one its `bd` calls are capped at. It is also constrained by
[NFR-BDC-0007](../requirements.md#nfr-bdc-0007-bundled-sqlite) (bundled
SQLite; its schedule store uses its own file and connection), which
REQ-CRON-0009 meets.

Architecture and crate-local ADRs: [architecture.md](architecture.md).

## REQ-CRON-0001: Parse before write

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

Schedule input is validated before anything is stored, once, by type:
`ScheduleName` (1 to `MAX_NAME_LEN` = 128 bytes, no surrounding whitespace,
no control characters), `CronExpr` (at most `MAX_EXPR_LEN` = 256 bytes,
parsed), `RunTimeout` (1 ms to `MAX_TIMEOUT` = 24 h) and a payload of at
most `MAX_PAYLOAD_BYTES` = 64 KiB of compact JSON, checked by
`ScheduleSpec::new`, `Scheduler::start` and the store with one rule.
`Scheduler::start` also refuses an expression with no future occurrence.
An expression that does not parse, has five fields or has no future
occurrence is `CronError::BadExpression`; any other invalid field or
exceeded limit is `CronError::InvalidSpec`. Nothing is written either way.
The number of active schedules is not capped: the caller creates every
schedule, and each row is bounded by the limits above.

**Refines:** ADR-BDC-0006 (expression parse before write)

## REQ-CRON-0002: Next-run computation in UTC

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

The grammar is the `cron` crate's in UTC: six or seven fields, seconds
first, year optional, or `@yearly`, `@monthly`, `@weekly`, `@daily`,
`@hourly`. A schedule's next run is the first `occurrence + jitter` strictly
after now, so a restart inside an open jitter window still fires that
occurrence. The next run is stored after every tick. An expression with no
further occurrence ends the schedule with reason `no future occurrence`.

**Refines:** ADR-BDC-0006 (next-run computation)

## REQ-CRON-0003: Jitter

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

`start` draws one jitter per schedule from the injected `JitterSource`,
in `[0, min(period / 2, 30 min))`, where the period is the gap between the
expression's next two occurrences. The jitter is stored and kept for the
life of the schedule, and is drawn again only when the name is restarted.

**Refines:** ADR-BDC-0006 (jitter)

## REQ-CRON-0004: Missed-tick replay per policy

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

An occurrence is missed when the worker sees it only after the occurrence
following it is also due: the process was down or the host was suspended.
Under `MissedTickPolicy::Coalesce` (the default) all missed occurrences are
replayed as one run as soon as the worker sees them. Under
`MissedTickPolicy::Skip` they are dropped. A run that is late but not missed
always runs. A backlog is never run one by one. The schedule then continues
from its next future occurrence.

**Refines:** ADR-BDC-0006 (missed-tick replay)

## REQ-CRON-0005: Stop check and per-run timeout

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

Before every run the worker asks `Job::should_stop`. `Stop(reason)` ends the
schedule with that reason and nothing runs. Stop check and run share the
schedule's timeout. The job receives the deadline, on the clock's monotonic
timeline. A run still going at the deadline is cancelled and recorded as
`RunOutcome::TimedOut`, and the next tick fires on time. The worker measures
the deadline on the monotonic timeline, so a wall-clock jump never times a
run out early. Cancellation is cooperative, at the run's next `.await`; the
hard timeout guarantee needs a multi-thread runtime and a job that never
blocks a runtime thread (documented on `Job`).

**Refines:** ADR-BDC-0006 (stop check before every run); constrained by
REQ-BDC-0703, NFR-BDC-0004

## REQ-CRON-0006: One run in flight per schedule

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

A tick that comes due while the schedule's previous run is still in flight
is skipped and logged, not queued. A run that panics is recorded as
`RunOutcome::Failed` and the schedule keeps ticking.

**Refines:** ADR-BDC-0006

## REQ-CRON-0007: Generation fencing

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

Every schedule has an id that is stable for its name and a generation that
rises each time an ended name is started again. Every run-state write
(`record_run`, `set_next`, `stop`) names `(id, generation)` and changes
nothing, returning `false`, unless that pair is the active schedule. A run
from an older generation therefore cannot record its result. A fenced write
makes the worker reload at once.

**Refines:** ADR-BDC-0006 (`(id, generation)` fencing)

## REQ-CRON-0008: Stores read no clock

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

`ScheduleStore` methods take every instant as a `Timestamp` argument (UTC,
microsecond precision, valid by construction) and keep it as given; text
exists only at the SQL boundary. No store reads a clock. A stored timestamp
that does not parse is a `Corrupt` store error, never silently replaced.

**Refines:** ADR-BDC-0006 (stores that read no clock)

## REQ-CRON-0009: SQLite store

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

`SqliteScheduleStore` implements `ScheduleStore` over bundled SQLite, in one
`sc_cron_schedule` table on its own database file and its own connection.
It never opens a second connection to another store's file (NFR-BDC-0007 as
amended). On open it sets the busy timeout, then WAL, then checks
`PRAGMA user_version`. A fresh file gets the schema at `SCHEMA_VERSION`; a
file at another version, or one holding other tables, is refused with
`StoreErrorKind::SchemaMismatch`. Creating a schedule is one immediate
transaction. Table CHECKs refuse rows whose end state contradicts itself or
that exceed the REQ-CRON-0001 limits. Schedules survive reopening the file.

**Constrained by:** NFR-BDC-0007

**Refines:** ADR-BDC-0006 (rusqlite store)

## REQ-CRON-0010: Injected time and randomness

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

The scheduler reads time only through `Clock` and randomness only through
`JitterSource`. `SystemClock` and `SystemJitter` are the real
implementations. `ManualClock` and `SeqJitter`, behind feature
`test-utils`, make every behaviour testable without real waits.
`ManualClock` records the time each park observed, so a test waits for the
worker to settle at the new time, and `jump_wall` steps wall time without
moving monotonic time.

**Refines:** ADR-BDC-0006

## REQ-CRON-0011: No ATM or bd-cache types

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

`sc-cron` depends on no bd-cache, sc-beads or ATM crate, and no such type
appears in its API. A schedule's input is an opaque `serde_json::Value`, and
every failure is a `CronError` with a stable `code()` (`BAD_EXPRESSION`,
`INVALID_SPEC`, `ALREADY_ACTIVE`, `STORE_FAILED`, `FENCED`) and `recovery()`
text. A store failure is a `StoreError` with a `StoreErrorKind` (`Busy`,
`Corrupt`, `SchemaMismatch`, `Unavailable`, `Other`) and its underlying
error as `Error::source`. `Fenced` names the id and generation.
`cargo tree -p sc-cron` shows only third-party crates.

**Refines:** ADR-BDC-0006; [architecture.md](../architecture.md) Boundaries
and Dependencies (`sc-cron` may depend on none)

## REQ-CRON-0012: Bounded shutdown

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2)

`WorkerHandle::shutdown(deadline)` stops ticking and waits for each in-flight
run until that run's own deadline, recording its outcome. If the shutdown
deadline passes first, the worker and its runs are aborted and record
nothing. The worker checks for shutdown between slots, so no new run starts
after shutdown is requested; a shutdown that arrives during a reload still
keeps every unchanged in-flight run for the drain. Dropping the handle also stops the worker, but
the drain then runs detached; callers await `shutdown` to know outcomes were
recorded. A scheduler runs at most one worker at a time. `Scheduler::start`
and `stop` are synchronous store calls; `start_async` and `stop_async` run
them on tokio's blocking pool.

**Refines:** ADR-BDC-0006
