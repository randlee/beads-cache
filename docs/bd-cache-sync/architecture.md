# bd-cache-sync Architecture

Status: Draft (sprint a-7, awaiting lead ruling). Crate-local decisions (domain code `SYNC`).
Repo-level context: [`../architecture.md`](../architecture.md). Requirements:
[`requirements.md`](requirements.md). Created by sprint a-7.

`bd-cache-sync` is the BD-CACHE cron module (`bd_cache_cron.py`, ADR-069 in
atm-core) in Rust, scheduled on `sc-cron` (ADR-BDC-0006). It composes the
core (`bd-cache`), the store (`bd-cache-store`), the `bd` CLI backend
(`sc-bd-cli`) and the scheduler (`sc-cron`). Its only inbound contract is
`EscalationSender` and `RepoConfig.tick`, both frozen in a-1.

| Module | Contents |
|---|---|
| `tick` | `tick` (drain, roots, per-phase sync, announcements), `stop_reason`, `TICK_BUDGET` |
| `phases` | `TickSettings` (the validated `[tick]` table), `ConfigRefusal`, `Phase`, `load_phases` |
| `active` | which phases are active: roots from the cache or one `bd list --id` |
| `announce` | sync-failure, outage and escalation messages, the `backend_unavailable` alert |
| `reply` | `TickReply`, `TickOutcome`, `Announcement` |
| `job` | `SyncJob` (the `sc-cron` `Job`), `TickRun`, `schedule_spec`, `TICK_EXPR` |
| `atm` | `AtmSender`: `atm send` with the body on stdin, bounded |
| `compose` | `compose` / `compose_with`: the real `SqliteStore` + `FlockDrainLock` + `BdCli` cache of a repository |

Dependencies (the Crate Layout of [`../architecture.md`](../architecture.md)):
`bd-cache`, `bd-cache-model`, `bd-cache-store`, `sc-bd-cli`, `sc-cron`,
`sc-beads`; third party `tokio` (`spawn_blocking`), `async-trait` (sc-cron's
`Job`), `serde`, `serde_json`, `toml` (phase files), `tracing`. Dev
dependencies: `tempfile`, `criterion`, `chrono` (sc-cron's `ManualClock`).
`bd-cache-store`, `sc-bd-cli` and `sc-cron` are path dependencies: the
workspace dependency table lists only the crates sprints 1 to 6 consume.

## ADR-SYNC-0001: The tick is a scheduler-independent library function

**Status:** Draft

Context: atm-bd keeps its own scheduling (`bd_cache_cron.py` under ATM cron,
through the Python bindings), and the later `bdc tick` will be driven by the
ATM cron module; `sc-cron` is the prototype scheduler of phase a.

Decision: the work of a tick is `tick(&mut Cache, &TickConfig, &dyn
EscalationSender, &dyn Clock, Deadline)`; `SyncJob` is only the adapter that
gives `sc-cron` a `Job`. The sprint doc's signature has four parameters; the
fifth, the clock, is the one the cache and its backend share. `Cache` does not
expose its clock, and the budget margins (deferring a sync not started by `T -
2 s`) must be measured on it, so it is passed in. (A `Cache::clock()` accessor
would remove the parameter; it is an a-1 amendment the lead can make.)

Consequences: `tick` and `stop_reason` are plain functions, testable with
`FakeStore`, `FakeBackend` and a `ManualClock`; one ticker per cache file is
the caller's rule (the store also refuses a second connection in a process).

## ADR-SYNC-0002: The cache lives in an `Arc<Mutex<_>>` and runs under `spawn_blocking`

**Status:** Draft

Context: ADR-CORE-0001: a `Cache` is `Send`, `!Sync`, its verbs block, and
`sc-cron` times a run out by dropping its future, which never stops a
blocking thread.

Decision: `SyncJob` holds `Mutex<Cache>` in an `Arc`. Each run and each stop
check is a `spawn_blocking` task that `try_lock`s the mutex: if it is held
(by the previous tick's task that outlived its timeout, or by the host's own
`with_cache` call) the run is `Refused("busy ...")` and the stop check
answers `Continue`; a `Poisoned` mutex is recovered with `into_inner`, its
poison cleared, and the tick reply says so once (`poisoned`, outcome
`refused`). The run's deadline `Instant` becomes `T` through `Deadline::at`,
so every `bd` call inside the tick is capped at the time left (NFR-BDC-0004),
which bounds how long a left-over task can hold the mutex.

Consequences: a tick never waits for a mutex, so ticks cannot queue up
behind a stuck task; the host's own reads and writes use `with_cache`, which
waits.

## ADR-SYNC-0003: Announcements record their marker only after delivery

**Status:** Draft

Context: REQ-BDC-0706 requires an undelivered message to be retried.

Decision: three markers decide what is sent again: `announced_at` on an
escalated entry, and the scope's `sync_alert` for both a sync-failure cause
and an outage (`backend_unavailable`, ADR-BDC-0005). Each is written only
after `EscalationSender::send` returns `Ok`; a failed send adds a line to
`TickReply::refused` and leaves the marker as it was. The delivery is
at-least-once: a crash between the send and the marker sends again. One
message goes to each distinct lead for an outage, covering that lead's
scopes. A phase without a lead uses `fallback_lead` for alerts about the
backend (REQ-BDC-0704, per the sprint's acceptance criteria); an escalated
entry whose phase has no lead is refused (REQ-BDC-0706), and one with no
scope goes to the fallback lead.

Consequences: a lead hears of an outage once per outage, however many ticks
it lasts; a sender that is down for hours is retried every tick without
losing anything.

## ADR-SYNC-0004: The stop rule requires positive knowledge

**Status:** Draft

Context: BD-CACHE's `cron.stop` ends the schedule when no phase is active and
no entry is open; the cron treats an uncached root that cannot be read during
an outage as inactive.

Decision: the tick keeps that rule for syncing (an unreadable root is inactive
for the tick and reported). `stop_reason` stops only when every root was read
(from the cache or from bd) and found closed, the phase files were all usable,
the configuration is sound and `open_work` is zero. Any uncertainty answers
`Continue`.

Consequences: an outage or a typo in `phases_dir` cannot end the schedule; an
unreadable phase file keeps it going until fixed. A registration of the
schedule is removed only by a clean `Stop` or by the operator.

## ADR-SYNC-0005: Phases are discovered from files, roots from the cache first

**Status:** Draft

Decision: phase files are read on every tick (they are small and operators
edit them); a root the cache holds in the phase's scope comes from
`Cache::list`, which a sync keeps current, and the remaining roots from one
`Cache::refresh` (one `bd list --id`, all statuses, forced store, scope
membership unchanged). Reading the scope's rows costs time proportional to
the scope; it is paid once per active phase per tick, not per read.

Consequences: a root that carries no `phase-<x>` label is read from bd every
tick; the phase is no worse off than the baseline's uncached-root rule.
