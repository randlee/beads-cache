# bd-cache-sync Architecture

Status: Approved (lead ruling, bc-lead, 2026-10-24), with the corrections of QA round 1 (fix-a-7-1). Crate-local decisions (domain code `SYNC`).
Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-7.

`bd-cache-sync` is the BD-CACHE cron module (`bd_cache_cron.py`, ADR-069 in
atm-core) in Rust, scheduled on `sc-cron` (ADR-BDC-0006). It composes the
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
| `job` | `SyncJob` (the `sc-cron` `Job`), `TickRun`, `schedule_spec`, `TICK_EXPR` |
| `atm` | `AtmSender`: `atm send` with the body on stdin, bounded, the whole process tree killed on timeout or stop |
| `compose` | `compose` / `compose_with`: the real `SqliteStore` + `FlockDrainLock` + `BdCli` cache of a repository |

Dependencies: ADR-SYNC-0006.

## ADR-SYNC-0001: The tick is a scheduler-independent library function

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: atm-bd keeps its own scheduling (`bd_cache_cron.py` under ATM cron,
through the Python bindings), and the later `bdc tick` will be driven by the
ATM cron module; `sc-cron` is the prototype scheduler of phase a.

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
`AtmSender`, kills its child; the shutdown order is: set the flag, await the
worker's shutdown, drop the job. `SyncJob::new` validates the `[tick]` table,
so a job that could never run is not built.

Consequences: a tick never waits for a mutex, so ticks cannot queue up
behind a stuck task; the host's own reads and writes use `with_cache` (which
waits, so not from async code) or `try_with_cache` (which does not).

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
`T`; with less than half a second left, or the stop flag set, the message is
not started, is counted in `TickReply::unsent`, stays unmarked and is sent by
the next tick (a deferral, not a refusal). Text embedded from errors is
clipped at 2 KiB. One message goes to each distinct lead for an outage,
covering that lead's scopes; the outage targets are the active phases and the
phases whose root could not be read, never a phase known to be closed. A
phase without a lead uses `fallback_lead` for alerts about the backend
(REQ-BDC-0704); an escalated entry whose phase has no lead is refused
(REQ-BDC-0706), and one with no scope goes to the fallback lead (requirements
amended by lead ruling S3, bc-lead, 2026-10-24).

Consequences: a lead hears of an outage once per outage, however many ticks
it lasts; a sender that is down for hours is retried every tick without
losing anything; a slow sender cannot push a tick past `T`.

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
`test-utils` features of the crates above. The process tree of `atm` is killed
with the platform's `kill`/`taskkill` programs rather than a `libc` or
`windows-sys` dependency, so the crate has no `unsafe`.

Consequences: a new dependency is a change to this ADR.

## ADR-SYNC-0007: The phase-file schema and the `atm send` contract

**Status:** Approved (lead ruling, bc-lead, 2026-10-24)

Context: both are defined by atm-bd's `bd_cache_cron.py` at atm-core
`05831dfe866d389feed138add3c29cb93cd6c82b` (the BD-CACHE baseline); the Rust
side must read the same files and talk to the same program.

Decision (confirmed against that commit with `gh api`; ported fixtures
`tests/fixtures/phase-bd.toml` and `phase-bk.toml`, see their `PROVENANCE.md`):

- **Phase file.** `<phases_dir>/phase-<x>.toml`, default `<repo>/.atm-bd/`.
  A string `root` (the phase's root bead) and a string `lead`; every other key
  (`plan`, `integration_branch`, `[templates]`) is ignored. The name is the
  file name after `phase-`; a configured phase list names phases without the
  prefix (the `phase-` prefix is accepted too). Differences from the reference,
  each deliberate: the reference silently skips a file without `root` or
  `lead`, and a missing directory is no phases; this crate keeps a file
  without `lead` (its alerts go to `fallback_lead`, lead ruling S3), reports an
  unusable file as a typed `PhaseFileProblem`, treats an unreadable
  directory as "do not stop" and refuses an empty phase list. A phase file is
  read up to 64 KiB, and at most 256 are read.
- **`atm send`.** `atm send <lead>@<team> --team tool --stdin --json` with
  `ATM_IDENTITY=cron` and `ATM_TEAM=tool` in the environment and the message on
  stdin, as the reference runs it; the reference's time limit is 10 s and so is
  this crate's, reduced to the time the tick has left. `lead` and `team` are
  `MemberName`s: nonempty, no whitespace, no `@`, no leading `-`. The message
  texts differ (the resolve command is `bd-cache resolve`, the Rust CLI's
  name); everything else a lead needs is the same (the entry, the error, the
  recovery).

Consequences: the Python cron module and this crate can read the same phase
files; a change of either contract changes this ADR.
