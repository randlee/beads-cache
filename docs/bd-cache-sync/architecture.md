# bd-cache-sync Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24), with the corrections of QA round 1 (fix-a-7-1). Crate-local decisions (domain code `SYNC`).
Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-7.

`bd-cache-sync` is the baseline design's cron module (the baseline Python
implementation's cron script and the first consumer's ADR-069) in Rust, scheduled on `sc-cron` (ADR-BDC-0006). It composes the
core (`bd-cache`), the store (`bd-cache-store`), the `bd` CLI backend
(`sc-bd-cli`) and the scheduler (`sc-cron`). Its only inbound contract is
`EscalationSender` and `RepoConfig.tick`, both defined in a-1 and amended only by lead ruling (the ports by ADR-MODEL-0003's a-7 amendment).

| Module | Contents |
|---|---|
| `tick` | `tick` (drain, roots, per-phase sync, announcements), `stop_reason`, `TICK_BUDGET` |
| `phases` | `TickSettings` (the validated `[tick]` table), `MemberName`, `ConfigRefusal`, `Phase`, `PhaseFileProblem`, `load_phases` |
| `active` | which phases are active: roots from the cache or one `bd list --id` |
| `announce` | sync-failure, outage and escalation messages, `AlertCause` (the `backend_unavailable` alert and the sync causes) |
| `reply` | `TickReply`, `TickOutcome`, `Announcement`, `Refusal` and `RefusalCode` (a stable code, the detail and the fix) |
| `control` | `StopFlag` and the tick's time budget for messages |
| `job` | `SyncJob` (the `sc-cron` `Job`; `with_recompose`, `join_idle`), `TickRun`, `CacheLost`, `JobCacheError`, `schedule_spec`, `TICK_EXPR` |
| messaging module (the `src/` file whose path the lint flags) | the consumer's message sender: the consumer's send command with the body on stdin, bounded, the whole process tree killed on timeout or stop |
| `compose` | `compose` / `compose_with`: the real `SqliteStore` + `FlockDrainLock` + `BdCli` cache of a repository; `compose` builds every `BdCli` over one process-wide `ProjectLocks` (a `OnceLock` static, as `bd-cache-py` keeps its own; S54, PE-RSH-003) and `compose_with` takes the lock set as an `Arc<ProjectLocks>` and the backend as a function of it, so a host shares its own set; a repository root that is not UTF-8 is `ComposeError::RepoRootNotUtf8` (`sync_repo_root_not_utf8`), never a lossy `ProjectRef`; `compose_with` takes the `$BEADS_DIR` value explicitly (`None` = unset; S36, over `SqliteStore::open_with`), rebinds the backend to its clock and gives every `bd` child that same `$BEADS_DIR` (S40, `BdCli::with_child_var`), normalized once for both into one `bd_cache_store::BeadsDirOverride` (empty is unset, relative is absolute against the process's working directory; S44, S49), failing closed with `ComposeError::BeadsDir(BeadsDirError)` (`sync_beads_dir_unresolvable`; the store's error whole, its `Display` and `source`, the `io::Error`, delegated: RBP-F050) before the store opens when a relative value cannot be made absolute (S48), and `ComposeError::code` maps each `ConfigLoadError` variant to this crate's stable `sync_config_unreadable` / `sync_config_invalid_toml` / `sync_config_bad_scope` (S34) in an exhaustive match with no wildcard arm (S38; the store's own codes are `store_config_*`, S37), the fix text still the loader's |

Dependencies: ADR-SYNC-0006.

## ADR-SYNC-0001: The tick is a scheduler-independent library function

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: the first consumer keeps its own scheduling (its cron script under its
own cron scheduler, through the Python bindings), and the later `bdc tick` will
be driven by that cron scheduler; `sc-cron` is the prototype scheduler of phase a.

Decision: the work of a tick is `tick(&mut Cache, &TickConfig, &dyn
EscalationSender, end: Deadline)`, as the sprint's deliverable 2 gives it;
`SyncJob` is only the adapter that gives `sc-cron` a `Job`. The tick reads
time from the cache's own clock, `Cache::clock()` (an accessor added to
`bd-cache` by lead ruling S2, bc-lead, 2026-10-24, an a-1/a-6 amendment), so
the deadlines, the budget margins and every backend call are on one
timeline; there is no second clock to pass in or to disagree. `tick_with` and
`stop_reason_with` take already validated `TickSettings` and a `StopFlag` for
hosts that validate once (`SyncJob` does).

Consequences: `tick` and `stop_reason` are plain functions, testable with
`FakeStore`, `FakeBackend` and a `ManualClock`; one ticker per cache file is
the caller's rule (the store also refuses a second connection in a process).

## ADR-SYNC-0002: The cache lives in an `Arc<Mutex<_>>` and runs under `spawn_blocking`

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: ADR-CORE-0001: a `Cache` is `Send`, `!Sync`, its verbs block, and
`sc-cron` times a run out by dropping its future, which never stops a
blocking thread.

Decision: `SyncJob` holds `Mutex<Cache>` in an `Arc`. Each run and each stop
check is a `spawn_blocking` task that `try_lock`s the mutex: if it is held
(by the previous tick's task that outlived its timeout, or by the host's own
`with_cache` call) the run is `Refused("busy ...")` and the stop check
answers `Continue`. A `Poisoned` mutex is recovered with `into_inner` and its
poison cleared, and a shared flag remembers that nobody has reported it;
whichever of the stop check and the run recovers it, the next tick's reply
carries `poisoned` once, also when that tick fails (the failed outcome names
it). The run's deadline `Instant` becomes `T` through `Deadline::at`, so every
`bd` call inside the tick is capped at the time left (NFR-BDC-0004), which
bounds how long a left-over task can hold the mutex. The stop check has its
own small bound (`STOP_CHECK_BUDGET`, 3 s) because it runs before every run
and shares the schedule's budget with it. A cooperative `StopFlag` ends a
running tick between phases and before each message and, given to the
consumer's message sender, kills its child; the shutdown order is: set the flag, await the
worker's shutdown, drop the job. `SyncJob::new` validates the `[tick]` table,
so a job that could never run is not built.

Consequences: a tick never waits for a mutex, so ticks cannot queue up
behind a stuck task; the host's own reads and writes use `with_cache` (which
waits, so not from async code) or `try_with_cache` (which does not).

Amendment (phase-end fix, S55, PE-RBP-F007): a poisoned mutex is no longer
trusted. A panic may leave the `Cache` between two steps of a verb, so
whoever meets the poison (the stop check, a run, or a host call) drops the
cache, clears the poison and sets the unreported flag; the mutex holds
`Option<Cache>`. The next tick, or host call, rebuilds it with the recompose
function given to `SyncJob::with_recompose` (the host passes `compose` over
the same inputs, so the same lock set and `$BEADS_DIR`) and its reply carries
`poisoned` once, as before. The stop check never recomposes: it answers
`Continue`. A job built with `new` / `with_stop` has no recompose function:
it reports `TickRun::Lost` (`sync_cache_lost`) and `with_cache` /
`try_with_cache` return `CacheLost`; a recompose that fails is
`TickRun::Lost` with the `ComposeError` as its cause, and is tried again by the
next tick. Evidence: `a_poisoned_mutex_drops_the_cache_and_the_next_tick_recomposes`,
`a_tick_that_panics_in_the_backend_is_not_reused_and_the_next_run_recomposes`
(a panicking backend under `spawn_blocking`; the panicked cache is shown
dropped by the backend's handle count),
`a_poison_met_by_the_stop_check_is_still_reported_by_the_next_tick`,
`a_job_without_a_recompose_function_reports_the_cache_lost_after_a_panic`,
`a_recompose_that_fails_is_tried_again_by_the_next_tick`
(`tests/job_threading.rs`).

Amendment (phase-end fix, PE-RSH-004): the shutdown order waits for the tick
in flight. `sc-cron` drops a timed-out run's future but never its
`spawn_blocking` thread, so after `WorkerHandle::shutdown` a tick may still
hold the cache. The order is now: (1) set the stop flag; (2) await
`WorkerHandle::shutdown`; (3) `SyncJob::join_idle(deadline)` from a blocking
thread, which takes the cache mutex with a timeout (a `try_lock` poll every
5 ms up to the deadline) and returns `true` once no tick holds it, `false` at
the deadline; (4) drop the job. `job.rs` documents the same steps. Evidence:
`join_idle_waits_for_the_tick_in_flight_up_to_its_deadline`
(`tests/job_threading.rs`: a tick held in its `bd list` keeps `join_idle`
waiting until its deadline, and a released tick ends it early).

Note (fix-a-7-3, lead ruling S20, QA3-005): the drain keeps its own budget and
does not look at the stop flag; a stop that arrives while it runs is seen when
it returns. The worst-case shutdown delay while a drain runs is therefore the
drain's hard deadline, `T - 2 s`: the start gate (no new entry starts after
`T - 5 s`) plus one `bd` call (the entry in flight is capped at `T - 2 s`).
`control.rs` implements this and `tests/tick_budget.rs` tests it.

Note (fix-a-7-4, QA4-006, extends the S20 note; **Status:** Approved (lead ruling S24, bc-lead, 2026-10-24)): the flag is also checked after the cached root reads and before
the one `bd list --id` of the uncached roots (ADR-SYNC-0005). A stop that is
set by then does not start that call; the tick replies `stopped` with no sync
and no message, so a stop that arrives after the drain waits at most for the
store reads of the roots. A stop that arrives while that call runs is seen
when it returns, within the miss-fill timeout. `active.rs` implements this and
`a_set_stop_flag_skips_the_root_refresh` tests it.

## ADR-SYNC-0003: Announcements record their marker only after delivery, inside the tick's budget

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: REQ-BDC-0706 requires an undelivered message to be retried; a tick
has a 25 s budget.

Decision: three markers decide what is sent again: `announced_at` on an
escalated entry, and the scope's `sync_alert` for both a sync-failure cause
and an outage (`AlertCause`: `backend_unavailable`, ADR-BDC-0005, or the
failure's underlying code). Each is written only after the sender returns
`Ok`; a failed send adds a `Refusal` (stable code, detail, fix) to
`TickReply::refused` and leaves the marker as it was. The delivery is
at-least-once: a crash between the send and the marker sends again. Every
send goes through `EscalationSender::send_within` with the time left before
`T` less the 1.5 s send reserve (REQ-SYNC-0011, S27); with less than half a second left, or the stop flag set, the message is
not started, is counted in `TickReply::unsent`, stays unmarked and is sent by
the next tick (a deferral, not a refusal). Text embedded from errors is
clipped at 2 KiB. One message goes to each distinct recipient for an outage,
covering that recipient's scopes; the outage targets are the active phases and the
phases whose root could not be read, never a phase known to be closed. A
phase without a recipient uses the fallback recipient setting for alerts about
the backend (REQ-BDC-0704); an escalated entry whose phase has no recipient is
refused (REQ-BDC-0706), and one with no scope goes to the fallback recipient
(requirements
amended by lead ruling S3, bc-lead, 2026-10-24).

Consequences: a recipient hears of an outage once per outage, however many ticks
it lasts; a sender that is down for hours is retried every tick without
losing anything; a slow sender cannot push a tick past `T`.

Amendment (fix-a-7-8, RSH-029; **Status:** Approved (lead ruling S33, bc-lead, 2026-10-24)):
the marker written after a delivery (`announced_at`, a sync-failure alert, an
outage alert) waits for the store's write lock at most the time left before
`T`: the tick passes `Budget::write_wait` (`T` less now) to
`Cache::mark_announced_within` and `Cache::set_sync_alert_within`, and the
store waits `min(BUSY_TIMEOUT, that)` (`CacheStore::mark_announced_within`,
`set_sync_alert_within`; ADR-STORE-0002). Before, the marker took the store's
full 5 s busy timeout, so a write lock another process held could carry a tick
some 3.5 s past `T` against the 1.5 s reserve. A marker that cannot be written
in time returns the store's usual error (`STORE_FAILED`, `database is locked`):
the tick fails with it, the message stays unmarked, and the next tick sends it
again, the at-least-once delivery above. Evidence: `tests/marker_wait.rs` (each of the three markers is
given exactly the time left, on the `ManualClock`),
`a_marker_write_may_wait_the_time_left_before_t` (`src/control.rs`), and in
`bd-cache-store` `a_bounded_marker_write_gives_up_on_a_held_lock_within_its_wait`
(a lock held by another connection) and
`a_bounded_marker_write_waits_at_most_its_wait_and_restores_the_timeout`
(the busy timeout sampled inside the write).

Amendment (fix-a-7-10, S35, RSH-030, RSH-031, QA7-002, QA7-003; **Status:** Approved (lead ruling S35, bc-lead, 2026-10-24); restated for QA8-001/RBP-F045 and lead ruling S41 (fix-a-7-12)):
the alert clears without a message (after a passing sync, and at the end of an
outage) are bounded writes too: they wait at most `Budget::write_wait`, and a
clear that fails is logged (`tracing::warn`, scope, reason, code) and does not
fail the tick; the alert stays set and the next tick clears it. The end of an
outage clears each scope once (a stored scope and a phase file of the same
phase are one scope), so a failed clear is the next tick's to retry
(fix-a-7-12, QA8-001). A marker write that fails after a delivered send logs a
structured warning (announcement kind, recipient, `seq` or scope, error code) and
then fails the tick as above, keeping the at-least-once delivery. The
announcement and its marked item are one value, `Unmarked` (`SyncFailure`,
`Outage` with the scope, `Escalation` with the `seq`), and the reason of a
clear is `ClearReason` (`SyncSucceeded`, `OutageOver`), so the logged kind
and the marked item cannot disagree (fix-a-7-12, RBP-F045). The bound applies
to the bounded writes only; the tick's other store writes keep the store's own
busy timeout; what it is in time is the S39 amendment below. Evidence
(`tests/marker_wait.rs`, one failing-write test per marker and per clear; a
wrapper store fails one bounded write with `STORE_FAILED`):
`a_failed_marker_write_fails_the_tick_and_the_next_tick_sends_again` (the
tick returns `Err`, the message was sent once; the next tick sends it again
and records the marker),
`a_failed_sync_failure_alert_write_fails_the_tick_and_the_next_tick_sends_again`,
`a_failed_outage_alert_write_fails_the_tick_and_the_next_tick_sends_again`,
`a_failed_alert_clear_does_not_fail_the_tick_and_the_next_tick_clears_it` and
`a_failed_outage_over_clear_does_not_fail_the_tick_and_the_next_tick_clears_it`.
The warnings themselves are not asserted: the workspace has no tracing
subscriber, and no test name claims the logging (lead ruling S41, QA8-002).

Amendment (fix-a-7-12, S39, RSH-032; **Status:** Approved (lead ruling S39, bc-lead, 2026-10-24); corrected by lead ruling S46 (fix-a-7-14, RSH-034)):
the bound of every bounded write (the markers after a send and the alert
clears) is `T` plus the bounded write's own run time (statement, commit,
fsync, any automatic checkpoint) plus scheduler jitter. The lock wait is
capped at the time left before `T` (`write_wait` is the time left and the
store waits `min(BUSY_TIMEOUT, that)`) and may overshoot it by the busy
handler's sleep slack, which counts as scheduler jitter (S46). It replaces
S35's "`T` plus the busy handler's sleep slack (tens of ms)", which bounded
the lock wait only.

Amendment (fix-a-7-14, S46, RSH-034; **Status:** Approved (lead ruling S46, bc-lead, 2026-10-24)):
the S39 wording "the lock wait itself never extends past `T`" is withdrawn,
here, in ADR-SYNC-0007, in REQ-SYNC-0011 and in `control.rs`
(`SEND_RESERVE`, `Budget::write_wait`): SQLite's busy handler sleeps in
steps, so a wait capped at the time left can end a step after `T` (22-46 ms
measured by RSH-034). That slack counts as scheduler jitter; the overall
bound of S39 is unchanged.

Amendment (fix-a-7-14, S45, RSH-037; **Status:** Approved (lead ruling S45, bc-lead, 2026-10-24)):
the end-of-outage clears stop once `Budget::write_wait` is zero (`T` has
passed) or the stop flag is set (the reply then says `stopped`); the alerts
not yet cleared stay set and the next tick clears them. No clear is started
with no time left. Evidence:
`an_outage_over_clear_is_not_started_after_t_and_the_next_tick_clears_it`
(`tests/marker_wait.rs`: no bounded write is issued after `T`, the next tick
clears) and `a_stopped_tick_clears_no_outage_alert_and_the_next_tick_does`
(`src/announce.rs`).

Note (fix-a-7-3, lead ruling S21): the baseline Python implementation's cron
stores `sync_alert` as `"<code>: <detail>"` (verified in the baseline at
`05831df`), where this crate stores
the bare key. The tick reads the code before the first `": "` with
`AlertCause::from_key` (through `AlertCause::from_stored`), so an alert written
in the Python form does not make it announce the same cause again. Python
records the generic `BD_CACHE.SYNC_FAILED` for every sync failure, and that
covers any sync cause; any other stored code covers only itself, so a different
cause is announced. `tests/announce.rs` pins both cases.

## ADR-SYNC-0004: The stop rule requires positive knowledge

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: BD-CACHE's `cron.stop` ends the schedule when no phase is active and
no entry is open; the cron treats an uncached root that cannot be read during
an outage as inactive.

Decision: the tick keeps that rule for syncing (an unreadable root is inactive
for the tick and reported). `stop_reason` (requirements amended by lead
ruling S5, bc-lead, 2026-10-24) stops only when every root was read
(from the cache or from bd) and found closed, the phase files were all usable,
the configuration is sound and `open_work` is zero. Any uncertainty answers
`Continue`.

Consequences: an outage or a typo in `phases_dir` cannot end the schedule; an
unreadable phase file keeps it going until fixed. A registration of the
schedule is removed only by a clean `Stop` or by the operator.

Amendment (fix-a-7-4, ARCH-305; **Status:** Approved (lead ruling S23,
bc-lead, 2026-10-24)): a phase root that bd does not return from the
`bd list --id` of the uncached roots is unknown, not closed: the rule is "stop
only on positive knowledge". The tick counts the phase inactive (it is not
synced), logs a warning, and reports it in `TickReply::unreadable_roots`
with the roots that could not be read; it is also an outage target, since it
is never known to be closed (ADR-SYNC-0003). `stop_reason` answers `Continue`
while any such root remains; a root counts towards `Stop` only once it was read
and found closed. `tests/stop_rule.rs`
(`a_root_bd_does_not_return_never_ends_the_schedule`) and `tests/tick_phases.rs`
(`a_root_that_bd_does_not_know_is_no_open_root_and_is_reported`) test it.

Note (fix-a-7-5, RBP-F039; S23 semantics unchanged): a root bd answered
without is also refused with its own `Refusal`, code `phase_root_not_returned`,
fix "check that the phase file's root names an existing bead", since its `root`
likely names no bead. A read of the roots that failed is transient and is not
refused; it is only reported in `TickReply::unreadable_roots`. Either way the
root stays unknown and the schedule continues. `tests/tick_phases.rs` (same
test) and `tests/outage.rs`
(`test_outage_with_uncached_roots_still_tells_every_configured_lead`) test both.

## ADR-SYNC-0005: Phases are discovered from files, roots from the cache first

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Decision: phase files are read on every tick (they are small and operators
edit them); a root the cache holds in the phase's scope comes from
`Cache::list`, which a sync keeps current, and the remaining roots from one
`Cache::refresh` (one `bd list --id`, all statuses, forced store, scope
membership unchanged). Reading the scope's rows costs time proportional to
the scope; it is paid once per active phase per tick, not per read.

Consequences: a root that carries no `phase-<x>` label is read from bd every
tick; the phase is no worse off than the baseline's uncached-root rule.

Amendment (fix-a-7-3, lead ruling S19, ARCH-202; **Status:** Approved,
bc-lead, 2026-10-24): phase roots are read with `Cache::cached`, not
`Cache::list`: one bead, no `bd` call and no scan of the scope. The answer is
trusted only when the phase's own label (`phase-<x>`) holds the bead and the
scope has synced (`scope_last_ok_at` is set), because a sync is what keeps the
row current; otherwise the root is unknown and is read with the one
`Cache::refresh` above. The cost stated above ("proportional to the scope") is
superseded: it is constant while no write is in flight, and when a write is in
flight (pending, or applied since the scope's base was fetched) the scope's
base is adjusted, as `get` does, until the next sync (QA3-006; ADR-CORE-0206).

Note (fix-a-7-4, ARCH-304, clarifies the S19 amendment; **Status:** Approved (lead ruling S24, bc-lead, 2026-10-24)): "the scope has synced" means the phase's own scope
(`phase-<x>`), not the scope `Cache::cached` reports the bead under. A root
that carries several phase labels is reported under the first known one, and
that scope's `scope_last_ok_at` says nothing about the phase whose root it is.
The phase's own `sync_state.last_ok_at` is read instead, with
`Cache::scope_synced` (amendment fix-a-7-5, QA5-004; **Status:** Approved
(lead ruling S30, bc-lead, 2026-10-24)): one `sync_state` row through the
existing `CacheStore::sync_state` port method, no new port method, no `bd`
call and no scan of the journal, read only for a root its phase's label
holds. This replaces the one `Cache::status` per `active_phases` call that S24
accepted. `tests/tick_phases.rs`
(`a_root_with_two_phase_labels_is_trusted_only_after_its_own_phase_syncs`)
tests it.

## ADR-SYNC-0006: Third-party and dev dependencies of bd-cache-sync

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: ADR-BDC-0006 lists `sc-cron`'s third-party crates; this crate has
its own. Every one is already in `[workspace.dependencies]` (a-1), and the
sibling crates it composes are there too (lead ruling S7, bc-lead,
2026-10-24: `bd-cache-store`, `sc-bd-cli`, `sc-cron` were added).

Decision: normal dependencies: `bd-cache`, `bd-cache-model`, `bd-cache-store`,
`sc-bd-cli`, `sc-cron`, `sc-beads` (the Boundaries table of
`docs/architecture.md`); `tokio` (`spawn_blocking`), `async-trait` (`sc-cron`'s
`Job` is an `async_trait`), `serde` and `serde_json` (the reply shapes and the
schedule payload), `toml` (the phase files; the `[tick]` table is parsed by
`bd-cache-model`, ADR-MODEL-0004), `tracing`. Dev dependencies: `tempfile`,
`criterion` (the informational bench), `chrono` (`sc-cron`'s `ManualClock`
takes a `chrono::Duration`), `rusqlite` (tests seed and inspect the cache file),
`tokio`'s `test-util` feature (the scheduler tests), and the `testing`, `test-support` and
`test-utils` features of the crates above. The process tree of the consumer's send command
is killed with the platform's `kill`/`taskkill` programs rather than a `windows-sys`
dependency, so the crate has no `unsafe`.

Amendment (fix-a-7-3, RSH-024; **Status:** Approved (lead ruling S22, bc-lead, 2026-10-24)):
`libc` is a normal dependency on Unix only
(`[target.'cfg(unix)'.dependencies]` in the crate, `libc = "0.2"` in the root
`[workspace.dependencies]`). It supplies the `O_NONBLOCK` constant alone, used
to open a phase file so that one that is a FIFO cannot block the tick while it
holds the cache mutex; the file's type is then checked on the opened handle.
No `libc` function is called, so the crate still has no `unsafe`. The
Crate Layout row of `docs/architecture.md` lists it (ARCH-303). This closes
ARCH-301.

Consequences: a new dependency is a change to this ADR.

## ADR-SYNC-0007: The phase-file schema and the consumer's send contract

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: both are defined by the baseline Python implementation's cron script
at `05831dfe866d389feed138add3c29cb93cd6c82b` (the baseline); the Rust
side must read the same files and talk to the same program.

Decision (confirmed against that commit with `gh api`; two ported
phase-file fixtures under `tests/fixtures/`, see their `PROVENANCE.md`):

- **Phase file.** One TOML file per phase in the configured phases directory,
  by default the first consumer's own directory under `<repo>`. A string `root`
  (the phase's root bead) and a string naming the phase's recipient; every other key
  (`plan`, `integration_branch`, `[templates]`) is ignored. The name is the
  file name after `phase-`; a configured phase list names phases without the
  prefix (the `phase-` prefix is accepted too). Differences from the reference,
  each deliberate: the reference silently skips a file without `root` or
  a recipient, and a missing directory is no phases; this crate keeps a file
  without a recipient (its alerts go to the fallback recipient, lead ruling S3), reports an
  unusable file as a typed `PhaseFileProblem`, treats an unreadable
  directory as "do not stop" and refuses an empty phase list. A phase file is
  read up to 64 KiB, and at most 256 are read. With `[tick] phases` set the
  directory is not scanned: each configured phase's file is looked up by
  name, so no cap can hide one; without it the scan visits at most
  `MAX_PHASES_DIR_ENTRIES` (1024) entries of any kind (`TooManyEntries`,
  `phase_files_too_many`; PE-RSH-010).
- **The consumer's send command.** The first consumer's send command, addressed
  to the recipient under the consumer's cron principal and sender identity (set
  in the arguments and the environment), with the message on stdin, as the
  reference runs it (the exact command line is in git history); the reference's time limit is 10 s and so is
  this crate's, reduced to the time the tick has left. The recipient's names
  are `MemberName`s: nonempty, no whitespace, no `@`, no leading `-`. The message
  texts differ (the resolve command is `bd-cache resolve`, the Rust CLI's
  name); everything else a recipient needs is the same (the entry, the error, the
  recovery).

Amendment (fix-a-7-3, RSH-023; **Status:** Approved (lead ruling S22, bc-lead, 2026-10-24)):
the output of the consumer's send command is read by threads
bounded as the messaging module documents. Each send starts at most three threads (the
message writer and one reader per output stream), named and started with
`thread::Builder` (PE-RSH-011): a thread that cannot be started is a
`SendError` (`other`) and the child's tree is killed and reaped, never left
running (`a_helper_thread_that_cannot_start_fails_the_send_and_kills_the_child`,
the messaging module). A reader keeps at most
`CAPTURE_LIMIT` (4096 bytes) and drops the rest, and the error says so; the
threads are waited for no longer than the time left in the tick's budget. They
end when the pipes close, so a descendant of the send command that outlives a failed send
and holds a pipe keeps its reader thread until it exits; the process tree is
killed on a timeout or a stop, but a descendant that escapes the group is not
the tick's to reap. After a successful send nothing is killed or waited for.
This closes ARCH-302.

Amendment (fix-a-7-5, RSH-027, REQ-QA-501; **Status:** Approved (lead ruling S27, bc-lead, 2026-10-24)):
the consumer's send stays inside the tick's send reserve. The tree kill and the
reaping of the child share one kill deadline, `KILL_BOUND` (1 s) from the start
of the kill; the tick keeps `SEND_RESERVE` (1.5 s) before `T`, which covers
`READ_GRACE` (250 ms) plus `KILL_BOUND`. Const assertions in the messaging module and
`control.rs` tie the constants together and to `HARD_MARGIN`.
Two tests split the evidence (fix-a-7-10, FTQ-025):
`a_child_that_ignores_term_and_its_descendant_are_killed_by_the_real_tree_kill`
(the messaging module) runs the real group kill on a child and descendant that ignore
TERM, under a generous deadline, and shows the child reaped and the descendant
gone; `a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` gives a tree
kill that only hangs an injected deadline and shows it given up and reaped. No
test races a short deadline against a real kill; each asserts the outcome, not
the clock (FTQ-021): that `KILL_BOUND` fits the reserve is the const
assertions'. A descendant is
dead once it is a zombie: reparented to PID 1, its reaping is not this
crate's (FTQ-024); the child and the tree kill, which this crate owns, must be
reaped.

Amendment (fix-a-7-8, RSH-029; **Status:** Approved (lead ruling S33, bc-lead, 2026-10-24)):
the reserve covers the send's overrun, not the marker write after it. That
write waits for the store's write lock at most the time left before `T`
(ADR-SYNC-0003, S33 amendment). A marker not written in time is sent again
next tick.

Amendment (fix-a-7-10, S35, ARCH-705, QA7-002; **Status:** Approved (lead ruling S35, bc-lead, 2026-10-24); restated by lead ruling S39 (fix-a-7-12) and corrected by lead ruling S46 (fix-a-7-14)):
the same bound holds for the alert clears without a message, and for the
bounded writes only (the marker writes after a send and the alert clears),
not for the tick's other store writes (ADR-SYNC-0003, S35 amendment). As
restated by lead ruling S39 (fix-a-7-12, RSH-032; **Status:** Approved (lead
ruling S39, bc-lead, 2026-10-24)), a contended store bounds such a write by
`T` plus the bounded write's own run time (statement, commit, fsync, any
automatic checkpoint) plus scheduler jitter. As corrected by lead ruling S46
(fix-a-7-14, RSH-034; **Status:** Approved (lead ruling S46, bc-lead,
2026-10-24)), the lock wait is capped at the time left before `T` and may
overshoot it by the busy handler's sleep slack, which counts as scheduler
jitter. The send reserve is not sized for that run time.

Amendment (fix-a-7-5, RSH-028; **Status:** Approved (lead ruling S28, bc-lead, 2026-10-24)):
a child or tree-kill program still running at the kill deadline is killed and
handed to one detached reaper thread (named after the messaging module,
started on first use, for the life of the host). It polls what it holds every 200 ms and drops
each process once it has exited, so a given-up process is not left a zombie;
one that never exits is held without blocking the others.
`a_child_that_is_not_reaped_in_time_is_reaped_by_the_reaper` and
`a_stuck_tree_kill_is_given_up_at_the_deadline_and_reaped` (the messaging module)
check the pid is gone.

Consequences: the Python cron module and this crate can read the same phase
files; a change of either contract changes this ADR.

## ADR-SYNC-0008: The sync on sc-cron rows, the switch, the post-sync hook and its trust model

**Status:** Proposed (approved when the phase-B planning PR merges; implemented by sprint b-8).
Supersedes ADR-SYNC-0007 with its fix-a-7-3 and fix-a-7-5 amendments, and ADR-SYNC-0003,
ADR-SYNC-0004 and ADR-SYNC-0005, whose consumer announcements, stop rule and phase discovery
REQ-BDC-0704..0707 and 0702 no longer require.

Context: ADR-BDC-0010 removes every consumer concern from the sync; ADR-BDC-0011 places the sync
on the sc-cron table of the cache file, with a consumer's Python sync replacing it when needed
and a post-sync hook. Rand ruled (2026-10-26) that the switch is two cron rows with a CLI
convenience, and that sc-cron stays a general-purpose cron assembly.

Decision: the design of `docs/plans/phase-b/phase-b-boundary.md` §Scheduling, §Single flight,
§One tick, §Calling a Python method and §Request and result shapes, with its Amendments A2–A5
and A7:
- at most one built-in sync row (a Rust job) and one Python sync row (the consumer's script, an
  sc-cron hook row) per repository, at most one enabled; `bdc cache sync use-python` /
  `use-builtin` switch them, and `use-python` runs the Python row at once and reports its exit
  status and stderr, restoring the built-in row if that run fails;
- the built-in row's run takes the `tick_lease`, drains, syncs every configured scope, then runs
  the post-sync hook named in its payload with at least `hook_reserve_ms` left, and marks
  nothing itself; `bdc cache cron run`, `cron run --once`, `bdc cache tick` and `Cache.tick()`
  run that same code;
- bd-cache-sync runs the post-sync hook with its own process supervision, as
  `<absolute interpreter> -I -m bd_cache_rs.run post_sync <module:function>`, from a private
  temp cwd with an allowlisted environment and capped output, killed with everything it started
  at its deadline; no row's or hook's code runs from a cache file that is not the current user's
  or is group- or world-writable (Unix; the Windows ACL check is a documented gap);
- sc-cron gets no beads-cache-shaped API; its hook runner is used only for the Python sync row,
  as any sc-cron row.

Rejected alternatives: an in-process Python call (it would hold the GIL and the store across
user code); code named in `.beads/bd-cache.toml` (it may be checked in); r3's per-scope methods
with a registered `sync` method called inside the tick (superseded by Rand's two-row ruling).

Consequences: the sync reaches the backend only through bd (REQ-BDC-0006); every bd call and
hook run is bounded by the deadline (NFR-BDC-0004).
