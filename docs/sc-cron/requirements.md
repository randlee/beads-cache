# sc-cron Requirements

Status: REQ-CRON-0001..0012 are Approved (lead ruling 2026-10-24, a-3 QA-2).
REQ-CRON-0013..0015 are Approved (Rand's rulings, 2026-10-25); b-6 also amends REQ-CRON-0001,
0002, 0005, 0006, 0007, 0009 and 0011, marked in each.

Domain code: `CRON`. Owned by sprint a-3 (`requirements.owns` in
[sprint-a-3-sc-cron.md](../plans/phase-a/sprint-a-3-sc-cron.md)); amended and
extended by sprint b-6
([sprint-b-6-sc-cron.md](../plans/phase-b/sprint-b-6-sc-cron.md)).
`sc-cron` is the prototype cron scheduler of
[ADR-BDC-0006](../architecture.md#adr-bdc-0006-prototype-a-sqlite-backed-cron-crate-here-for-sc-runtime),
extracted from an earlier in-house scheduler. It owns no repo-level
requirement, and names no caller's requirement: how a caller meets its own
deadlines on top of REQ-CRON-0005 is in that caller's docs (for beads-cache,
`docs/bd-cache-sync/`). *Amended by ADR-CRON-0009 (phase-B planning PR, sc-cron generality review)*. It is constrained by
[NFR-BDC-0007](../requirements.md#nfr-bdc-0007-bundled-sqlite) (bundled
SQLite; its schedule store uses its own file and connection), which
REQ-CRON-0009 meets.

Architecture and crate-local ADRs: [architecture.md](architecture.md).

## REQ-CRON-0001: Parse before write

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6
(five-field expressions are accepted, REQ-CRON-0015)

Schedule input is validated before anything is stored, once, by type:
`ScheduleName` (1 to `MAX_NAME_LEN` = 128 bytes, no surrounding whitespace,
no control characters), `CronExpr` (at most `MAX_EXPR_LEN` = 256 bytes,
parsed), `RunTimeout` (1 ms to `MAX_TIMEOUT` = 24 h) and a payload of at
most `MAX_PAYLOAD_BYTES` = 64 KiB of compact JSON, checked by
`ScheduleSpec::new`, `Scheduler::start` and the store with one rule.
`Scheduler::start` also refuses an expression with no future occurrence.
An expression that does not parse or has no future occurrence is
`CronError::BadExpression`; any other invalid field or
exceeded limit is `CronError::InvalidSpec`. Nothing is written either way.
The number of active schedules is not capped: the caller creates every
schedule, and each row is bounded by the limits above.

**Refines:** ADR-BDC-0006 (expression parse before write)

## REQ-CRON-0002: Next-run computation in UTC

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6
(five-field expressions, REQ-CRON-0015)

The grammar is the `cron` crate's in UTC: six or seven fields, seconds
first, year optional, or `@yearly`, `@monthly`, `@weekly`, `@daily`,
`@hourly`; and, from b-6, standard five-field crontab (REQ-CRON-0015). A schedule's next run is the first `occurrence + jitter` strictly
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

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6 (the
three-way answer, ADR-CRON-0007)

Before every run the worker asks the stop check: `Job::should_stop`, or for a
Python target the `cron.stop` hook (REQ-CRON-0013). It answers
`StopDecision::Continue`, `Disable(reason)` or `Delete(reason)`, and for
`Disable` and `Delete` nothing runs. `Disable` keeps the schedule active but
not enabled (REQ-CRON-0014) and records the reason as the last outcome,
`Refused(reason)`. `Delete` ends the schedule with that reason, as `Stop`
did; its row is kept as the ended record and the name can be started again.
`StopDecision::Stop(reason)` remains as an alias of `Delete(reason)` for
callers written against the earlier contract. A stop check that cannot answer
skips that run and leaves the schedule active and enabled: a panic is recorded
`Failed`, a run past the timeout `TimedOut`, and for a Python hook a failure
to start, a non-zero exit or a reply that is not one of the three actions is
recorded `Failed`. Stop check and run share the
schedule's timeout. The job receives the deadline, on the clock's monotonic
timeline. A run still going at the deadline is cancelled and recorded as
`RunOutcome::TimedOut`, and the next tick fires on time. The worker measures
the deadline on the monotonic timeline, so a wall-clock jump never times a
run out early. Cancellation is cooperative, at the run's next `.await`; the
hard timeout guarantee needs a multi-thread runtime and a job that never
blocks a runtime thread (documented on `Job`).

**Refines:** ADR-BDC-0006 (stop check before every run), ADR-CRON-0007;
constrained by REQ-BDC-0703, NFR-BDC-0004

## REQ-CRON-0006: One run in flight per schedule

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6

A tick that comes due while the schedule's previous run is still in flight
is skipped and logged, not queued. A run that panics is recorded as
`RunOutcome::Failed` and the schedule keeps ticking. Across processes the
guarantee is REQ-CRON-0007's: a name is active in at most one place, and a run
that outlived its schedule cannot write (amended by b-6, which tests it with
two processes on one database file).

**Refines:** ADR-BDC-0006

## REQ-CRON-0007: Generation fencing

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6

Every schedule has an id that is stable for its name and a generation that
rises each time an ended name is started again. Every run-state write
(`record_run`, `set_next`, `stop`, `disable`) names `(id, generation)` and
changes nothing, returning `false`, unless that pair is the active schedule.
The store enforces this, so it holds between processes that share the
database file: a process that kept an older generation changes nothing
whichever process restarted the schedule. A run
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

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6
(`SCHEMA_VERSION` 2, the `enabled` column)

`SqliteScheduleStore` implements `ScheduleStore` over bundled SQLite, in one
`sc_cron_schedule` table on its own database file and its own connection.
It never opens a second connection to another store's file (NFR-BDC-0007 as
amended). On open it sets the busy timeout, then WAL, then checks
`PRAGMA user_version`. A fresh file gets the schema at `SCHEMA_VERSION` (2);
a file at version 1, which has no `enabled` column, gains it in place with
every schedule enabled (REQ-CRON-0014); a file at any other version, or one
holding other tables, is refused with `StoreErrorKind::SchemaMismatch`. Creating a schedule is one immediate
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

## REQ-CRON-0011: A general-purpose scheduler

**Status:** Approved (lead ruling 2026-10-24, a-3 QA-2); amended by b-6 (two
codes added; the Python hook protocol is generic)

`sc-cron` depends on no bd-cache or sc-beads crate, and no beads, bd or
bd-cache type appears in its API. *Amended by ADR-CRON-0009:* it depends on no
consumer crate at all, and no consumer type appears in its API. A schedule's input is an opaque `serde_json::Value`, and
every failure is a `CronError` with a stable `code()` (`BAD_EXPRESSION`,
`INVALID_SPEC`, `ALREADY_ACTIVE`, `NOT_FOUND`, `HOOK_FAILED`, `STORE_FAILED`,
`FENCED`) and `recovery()` text. The Python hook protocol (REQ-CRON-0013)
names no beads concept: its events are `cron.tick` and `cron.stop`, and the
payload is the caller's. A store failure is a `StoreError` with a `StoreErrorKind` (`Busy`,
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

## REQ-CRON-0013: Python hook target

**Status:** Approved (Rand's rulings, 2026-10-25)

A schedule's worker runs a `HookTarget`: a Rust `Job`, or a Python module
(`HookTarget::Python(PythonHook)`). A Python target runs as
`<interpreter> -I -m <module>` from a fresh temporary working directory, once
per event, with one JSON object on stdin:
`{"protocol": "sc-cron.hook.v1", "event": "cron.tick" | "cron.stop",
"schedule", "payload", "deadline"}`, where `schedule` is the schedule's name,
`payload` is its payload as stored and `deadline` is the run's deadline as a
UTC instant. The worker sends `cron.stop` before every run, and `cron.tick`
for the run if the stop check said to continue; both carry the same deadline.
A `cron.stop` hook answers on stdout with
`{"action": "continue" | "disable" | "delete", "reason": "..."}` (REQ-CRON-0005;
`reason` is required for `disable` and `delete`). A `cron.tick` hook that exits
0 is `RunOutcome::Ok`, any other exit is `Failed`.

Every hook a schedule starts, `cron.tick` and `cron.stop` alike, is killed,
with every process it started, at the schedule's timeout capped by the time
left before the run's deadline (a Unix process group or a Windows job object
carries the kill, and it also happens if the run is cancelled). A run whose
deadline has passed starts no hook. Each of stdout and stderr is kept up to
`MAX_HOOK_OUTPUT_BYTES` (64 KiB) and the rest is read and discarded, so a hook
that floods a pipe is neither blocked nor allowed to grow memory. The
interpreter is an absolute path or a bare command name, and the module a dotted
path of ASCII identifiers of at most 256 bytes, both checked once when
`PythonHook::new` builds the target. A hook that cannot be started is
`CronError::HookFailed` (`HOOK_FAILED`) from `PythonHook::invoke`, and a
`Failed` outcome from the worker.

This runner is also the one `bd-cache-sync` (b-7) uses for the consumer's
Python sync method and the post-sync hook, so the kill here is what enforces
NFR-BDC-0004 for both.

*Amended by ADR-CRON-0009:* `bd-cache-sync` (sprint b-8) runs the consumer's
Python sync row through `PythonHook` and its post-sync hook through this crate's
exported supervisor (REQ-CRON-0018); the kill described here still enforces
NFR-BDC-0004 for both.

**Refines:** ADR-CRON-0001, ADR-CRON-0008; **constrained by:** NFR-BDC-0004,
REQ-CRON-0010 (time is injected), REQ-CRON-0011

## REQ-CRON-0016: A hook target per schedule row

**Status:** Proposed (approved when the phase-B planning PR merges; implemented by sprint b-8)

A worker resolves each schedule row's hook target from the row, so rows in one table can run
different targets: a Rust `Job`, or a Python module (`<interpreter> -I -m <module>`, as
REQ-CRON-0013). The target stays Rust or Python (Rand, 2026-10-26); maturin / PyO3 bindings for
sc-cron itself come when it moves to sc-runtime, not in this phase.

**Refines:** ADR-CRON-0009; **constrained by:** REQ-CRON-0011

## REQ-CRON-0014: Enabled flag

**Status:** Approved (Rand's rulings, 2026-10-25)

Each schedule has an `enabled` boolean, stored in an `enabled` column that
defaults to 1. `Scheduler::set_enabled(name, enabled)` (and `set_enabled_async`)
updates only that column of the active schedule with that name, so its id,
generation, expression, payload, timeout, jitter and run state are unchanged;
a name with no active schedule is `CronError::NotFound` (`NOT_FOUND`). A
disabled schedule never fires: the worker keeps no slot for it, and its stored
next run is left as it was. A run in flight when the schedule is disabled is
cancelled and records nothing. Enabling resumes the same schedule; a run that
came due meanwhile counts as missed and is handled per the `MissedTickPolicy`
(REQ-CRON-0004). The change reaches this scheduler's worker at once. A worker
in another process, or on another scheduler, gets no signal, and a schedule it
holds no slot for (new, disabled, ended) wakes nothing. So the worker reloads
the active schedules at least once per reload-poll interval on the injected
clock, whatever the rows are (`Scheduler::with_reload_poll`, default
`DEFAULT_RELOAD_POLL`, 30 s; values under 1 s are raised to 1 s). A schedule
started, restarted, enabled, disabled or stopped elsewhere is therefore
noticed at most one interval after the change is stored: a started or
restarted or re-enabled schedule gets its slot (a run that came due
meanwhile is handled per the `MissedTickPolicy`), and a disabled, stopped or
restarted-over schedule loses its slot and its run in flight is cancelled. A
disable or stop is also noticed earlier if its next tick comes first, because
`set_next` refuses a disabled or ended schedule and a refused write makes the
worker reload before it launches (REQ-CRON-0007). The bound holds while the
store can be read; a failed reload is retried with a backoff of 1 s doubling to
60 s (REQ-CRON-0007). A reload is one `list_active` query that keeps every
unchanged slot, so the poll costs one query per interval. A new or restarted
schedule is enabled, and a stop check can disable a schedule (REQ-CRON-0005).

**Constrained by:** REQ-CRON-0008 (the store reads no clock), REQ-CRON-0009
(`SCHEMA_VERSION` 2)

**Refines:** ADR-BDC-0006, ADR-CRON-0007

## REQ-CRON-0015: Five-field expressions

**Status:** Approved (Rand's rulings, 2026-10-25)

A standard five-field crontab expression (minute, hour, day of month, month,
day of week) is accepted, in UTC, and fires at second 0 of each matching
minute. It is read with crontab's meaning, not the `cron` crate's: the day of
week counts Sunday as 0 or 7 and Monday as 1 (names `SUN`..`SAT` are accepted,
and a range or step may be used), where the `cron` crate counts Sunday as 1;
and when both the day of month and the day of week are restricted (neither
field starts with `*`), a day matches if either field matches. A day-of-week
field outside this grammar (for instance `5L` or `1#2`) is
`CronError::BadExpression`; the six- and seven-field forms keep the `cron`
crate's grammar and numbering, and the `@` macros are unchanged. Fewer than
five fields remain `BadExpression`. The expression is stored as given.

**Refines:** ADR-BDC-0006 (expression parse before write); amends
REQ-CRON-0001 and REQ-CRON-0002
