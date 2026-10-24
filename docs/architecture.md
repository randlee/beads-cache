# beads-cache Architecture

Status: draft. No architecture has been approved yet.

## Overview

beads-cache extracts the beads SQLite cache from atm-core, where it runs
today as part of the `atm-bd-orchestration` skill alongside an integrated TUI
and other orchestration features. The design is baselined on that in-use
implementation, limited to the cache functionality and delivered first as an
MVP; where beads-cache departs from the atm-core baseline, an ADR here
records why.

beads-cache is built against sc-runtime, which is being specified in
parallel. sc-runtime is an upstream dependency: its interfaces and
consumer contract constrain this architecture, and gaps go back to the
sc-runtime team as requests rather than local workarounds.

- Baseline (the contract): `.claude/skills/atm-bd-orchestration/scripts/BD-CACHE.md`
  in atm-core, introduced by https://github.com/randlee/atm-core/pull/2219.
  Reference implementation: `bd_cache.py`, `bd_models.py` and `bd_cache_cron.py`
  in the same directory. Acceptance suite: `tests/test_bd_cache.py` (41 tests)
  and `tests/test_bd_cache_cron.py` (9), with fixtures in
  `tests/fixtures/bd-1.3.1/`. Design tracker:
  https://github.com/randlee/atm-bd-orchestration/issues/557. Where BD-CACHE.md
  and the Python code disagree, report it on that issue.
- Upstream dependency: sc-runtime (https://github.com/randlee/sc-runtime).

CLI: the primary command is `bd-cache`, with the alias `bdc`.

Backend research: [bd serve HTTP API](research/bd-serve-http.md) and
[DoltHub sync without a local server](research/dolthub-sync.md).

## Target Design

Status: draft direction, agreed in discussion 2026-10-23; the ADRs below carry
the decisions.

beads-cache is a **filtered local replica** of a beads database. bd databases
grow large (sc-obs is already over 1 GB), so the cache never copies a whole
database: it holds only the beads in its configured sync scopes, kept in step
with a local or remote dolt server through `bd`. That filtered store is the
point of the product: fast local reads for local work, and a local data store
for an HTTPS MCP server.

The backend is allowed to be slow or unreachable. A dolt server on another
machine behind flaky wifi, or a host at 100% CPU where every `bd` call takes
seconds, must not slow or fail local work. Reads come from the cache with
their staleness reported, writes are journaled, and the drain and sync catch
up when the backend responds (ADR-BDC-0005).

### Layers

```
 front ends:   bdc / bd-cache CLI      HTTPS MCP server      (later) read-only /v0 facade
                 (BD-CACHE contract)    (streamable HTTP + auth)
                        \                   |                    /
 core library:            bd-cache: SQLite store, sync scopes, journal, drain
                                          |
 backend port:            sc-beads: bd CLI adapter (phase a)
                                     bd serve HTTP adapter (later)
                                          |
 backend:                 bd  ->  dolt sql-server (local, shared or Hosted Dolt)
                                  or embedded dolt + dolthub:// remote via bd sync
 runtime:                 sc-runtime (process model, sync task scheduling)
```

- The core library knows nothing about its front ends. `bdc` keeps the
  BD-CACHE CLI contract so atm-bd's hooks and cron keep working.
- The MCP server is a second front end over the same core. No upstream beads
  project offers MCP over HTTP today (`beads-mcp` is stdio only, and
  `bd serve` is HTTP but not MCP), so beads-cache provides it.
- All writes go through `bd` (see the research docs: writes through the
  DoltHub SQL API bypass bd's guards and events journal).

### Data flow

- **Read:** a front end reads the cached row with that bead's pending journal
  entries replayed on top. A `get` outside every scope may make one `bd`
  fetch. A `list` or `ready` outside every scope is refused with an error
  that names the configured scopes; it is never answered from a partial
  cache.
- **Write:** validated, checked against its precondition, and journaled. The
  reply says the write is journaled (with its `seq`), not that it has
  landed. The MCP write tools say this explicitly and may offer an optional
  wait for the drain.
- **Drain:** applies journaled writes through the backend port in `seq`
  order, with achieved checks, preconditions and read-back (BD-CACHE.md).
- **Sync:** the sync task (on sc-runtime) refreshes each active scope from
  its own cursor, then re-reads only the cached dependers of blockers whose
  status changed, inside or outside the cache (ADR-BDC-0002).

### Open questions

- Which scope kinds besides `phase-<x>` come first (label set, assignee,
  status).
- Whether rows are stored in full or as `--brief` rows with full text fetched
  on demand. Needs measurement; full rows are the BD-CACHE baseline.
- MCP auth model (bearer token file like `bd serve`, or OAuth) and which MCP
  tools ship first. Create and comments are out of BD-CACHE v1 but a
  general-purpose MCP will need them.
- Automatic prune of closed scopes (an open item in BD-CACHE.md).

## Crate Layout

Status: proposed for phase a. Phase a builds only the crates that do not
depend on sc-runtime crates (sc-config, sc-transport, sc-command,
sc-runtime), which are being built in parallel. sc-runtime has no cron or
scheduler crate today, so phase a also prototypes one here (`sc-cron`) and
builds the sync task on it, to finish more of the deliverables before
integrating with sc-runtime.

| Crate | Responsibility | May depend on | Phase |
|---|---|---|---|
| `bd-cache-model` | Cache types on top of `sc-beads` rows: `Edge`, `Op`, `Patch` (`/v0`-aligned), guards, `Refusal`, `CacheError` with every `BD_CACHE.*` code and its fix text, overlay and `achieved` logic | serde | aa |
| `sc-beads` (shared, seeded from btit) | Beads backend contract and the bd CLI adapter, shared with beads-task-issue-tracker: transport-neutral `BeadsBackend` trait, CLI invoker with a recording test double, probe and version gates, dolt-mode detection, per-project locks. Adds what bd-cache needs: batched `--id` and `--updated-after` lists, guarded update (`--if-status`/`--if-assignee`, exit 13 as a typed refusal), close with reason, a per-call deadline, `/v0`-aligned patch, backend fingerprint, and a strict row mode (ADR-BDC-0007) | serde | a |
| `bd-cache-store` | SQLite file: BD-CACHE v3 schema plus the additive ADR-BDC extensions, file location from `.beads/metadata.json`, meta and rebuild rule, journal, scopes and membership, edges index | model, rusqlite (bundled) | aa |
| `bd-cache` (core library) | `get`, `write`, `drain`, `sync`, `resolve`, `prune`, `status`, `ready`/blocked refresh (ADR-BDC-0002), outage handling (ADR-BDC-0005); takes plain config values and a clock, no runtime | model, `sc-beads`, store | a |
| `bdc` binary (`bd-cache`) | JSON-first CLI, mapped to `/v0` vocabulary where it makes sense; BD-CACHE CLI contract | core, sc-command / sc-config | after sc-runtime |
| `sc-cron` (prototype, moves to sc-runtime) | SQLite-backed cron scheduler extracted from atm-core's `atm-cron`: expression parse, next-run computation, jitter, missed-tick replay, stop check before every run, generation fencing of overlapping runs, schedule store with a rusqlite implementation. Generic job payload and error type; no ATM or bd-cache types (ADR-BDC-0006) | cron, chrono, tokio, rusqlite | a |
| sync task | scheduled drain and sync per BD-CACHE's cron module, on `sc-cron`; escalation delivery behind an injected sender (the ATM `atm send` sender is one implementation) | core, `sc-cron` | a |
| MCP server | HTTPS MCP front end over the core | core, sc-runtime, sc-transport | later |

Phase a acceptance runs through the core library API with the fake
backend: the ported cases from `test_bd_cache.py` (41), the 9 cases from
`test_bd_cache_cron.py` against the sync task with an injected sender, and
the bd-1.3.1 contract fixtures. `sc-cron` carries its own tests, ported from
atm-cron where they apply.

## Boundaries and Dependencies

To be filled: allowed dependency directions and public interfaces between
crates.

## Architecture Decision Records

Each ADR is its own section, `## ADR-<DOMAIN>-nnnn: Title`, where `<DOMAIN>`
is the product's domain code, or the crate's domain code in
per-crate docs. ADR template: `**Status:**` (Draft, Proposed, Approved,
Active, Superseded), context, decision, and consequences. An ADR changes only
through a later ADR that names the one it amends or supersedes. Domain
code: `BDC`.

## ADR-BDC-0001: Sync scopes with per-scope cursors and bead membership

**Status:** Draft

Context: BD-CACHE v3 syncs per `phase-<x>` label, one `sync_state` row per
phase, and stores one `phase` column per bead (its first `phase-*` label).
bdc must hold several phases at once today, and as a filtered cache for
local work and MCP it will need other filters later. Whole-database sync is
not viable at current database sizes.

Decision: a sync scope is a named filter with its own cursor. The first and
only kind in phase a is `phase-<x>`, which maps one to one onto today's
`sync_state` rows. Bead membership is recorded per (bead, scope), so a bead
can belong to several scopes; the `phase` column is kept for v3
compatibility. Pruning a scope deletes only beads that no other scope holds.

Consequences: extends the BD-CACHE v3 file additively; a v3 file must stay
readable, and the Python implementation must accept the extended file or the
schema version moves. Needs agreement on
https://github.com/randlee/atm-bd-orchestration/issues/557.

## ADR-BDC-0002: Blocked state for dependencies outside the cache

**Status:** Draft

Context: a filtered cache holds edges whose targets are in no scope. bd
stores blocked state on each row (`is_blocked` in `bd schema` `types.issue`)
and settles it inside the transaction of every committing write, including
the dependers of a closed blocker and parent-child descendants. So `ready`
does not need the targets' status: it can read `is_blocked` from the cached
row. But bd settles `is_blocked` **without bumping `updated_at`**
(`BlockedStateInvariant` in beads `issueops/blockedstate.go`), so
`bd list --updated-after` never returns a row whose only change is a
blocked-state flip. When a blocker outside the cache closes, the cached
depender keeps `is_blocked = true` until something else touches it. After a
pull or merge, bd recomputes blocked state in a scoped pass that can be
skipped or fail. This affects the BD-CACHE v3 design and the live Python
implementation today.

Reopening works the same way: reopening a closed bead re-blocks its dependers
(for example a downstream sanity bead) without changing their `updated_at`.

Edges: `bd list --json` rows carry only outgoing edges (`dependencies`) plus a
`dependent_count`; incoming edges are not listed. The cache's own `edges`
table, indexed by `depends_on_id`, gives the incoming edges from cached
beads, which are the only dependers the cache needs to refresh.

Decision: re-read only the dependers of blockers whose status changed, never
every blocked bead.

1. **Blocker in the cache.** Its own close or reopen bumps its `updated_at`,
   so scope sync (or the drain's read-back) sees the status change. The cache
   then looks up that bead's cached dependers through the `edges` index and
   re-reads just those in one batched `bd list --id`. Repeat to a fixpoint for
   parent-child descendants.
2. **Blocker outside the cache.** The cache keeps the set of outside blocking
   targets (edge targets that are not cached rows). Each sync asks for only
   the ones that changed: `bd list --id <outside targets> --updated-after
   <cursor>`, chunked to keep the argument list bounded. Each changed target's
   cached dependers are re-read as in 1.
3. **Safety net.** A merge or pull can leave blocked state stale until bd's
   scoped recompute runs. A full re-read of cached open beads with blocking
   edges runs only on demand (`doctor`, a manual refresh, or after a backend
   rebuild), not on every sync.

Reads per sync are then bounded by the number of changed blockers' dependers,
plus one small query for outside targets.

To verify in phase a: that `--id` and `--updated-after` combine as a filter
intersection, and that `bd close`/`bd reopen` bump the blocker's own
`updated_at`.

Consequences: one extra small `bd` call per sync for outside targets; no
reference rows are stored. Report the `updated_at` gap on
https://github.com/randlee/atm-bd-orchestration/issues/557.

## ADR-BDC-0003: Align data vocabulary with the bd serve /v0 API

**Status:** Draft

Context: the CLI, the MCP tools and a later HTTP backend adapter should share
one set of types. bd serve's OpenAPI v0 spec and `bd list --json` use the
same Go structs for rows.

Decision: align the data vocabulary, not the command structure. Patch shape
follows `/v0` (`add_labels`, `remove_labels`, `append_notes`, and `metadata:
{set, unset}` accepted alongside BD-CACHE's `set_metadata`/`unset_metadata`);
guards accept `expected_status`/`expected_assignee` alongside BD-CACHE's
precondition keys, with room for `expected_version`; refusals keep the
`BD_CACHE.*` codes and add a `reason` using `/v0` names
(`precondition_failed`, `not_closable`, `already_claimed`). The CLI keeps its
cache verbs (`get`, `list`, `write`, `drain`, `sync`, `resolve`, `prune`,
`status`), because a journaled write is not an applied write.

Consequences: every change is additive to the BD-CACHE CLI contract and
needs agreement on #557.

## ADR-BDC-0004: Backend port with the bd CLI adapter first

**Status:** Draft

Context: released bd (1.3.0/1.3.1) has no `id` or `updated_after` filter on
`bd serve`, and bd serve needs a dolt sql-server. The bd CLI works with every
backend bd supports, including Hosted Dolt and embedded dolt with a remote.

Decision: all backend access goes through one port, the `sc-beads`
`BeadsBackend` trait (ADR-BDC-0007) (`fingerprint`,
`list_by_ids`, `list_since(scope, cursor)`, `get`, `close`,
`update(patch, guards)`, `dep_add`, optional `sync_remote`). Phase a builds
only the bd CLI adapter. The sync cursor is opaque (a timestamp today; a dolt
commit hash or events `seq` later). Nullable `revision`, `backend_kind`,
`endpoint`, `replica_id` and `journal_seq` columns leave room for the HTTP
adapter without a schema change.

Consequences: an HTTP adapter can be added when a bd release ships the
missing filters; beads-task-issue-tracker already plans one for the same
trait, so bd-cache gets it through `sc-beads`. The unverified gap that `--updated-after` may miss deletes
and edge changes is tracked for a later commit-hash cursor.

## ADR-BDC-0005: The backend may be slow or unreachable

**Status:** Draft

Context: two deployments are expected: a dolt server on a remote machine over
an unreliable network, and a local server on a host so loaded that `bd`
calls take seconds (the BD-CACHE motivation). BD-CACHE v3 escalates a write
after `MAX_ATTEMPTS` (5) transient failures, so a backend outage of a few
cron ticks escalates every pending write to the phase lead.

Decision:
- Reads never wait on the backend. Every read reply carries the scope's
  `last_ok_at` (and `stale` per row), so callers and MCP tools can see how
  fresh the data is.
- Backend unreachable (`BD_CACHE.BD_UNAVAILABLE`: timeout, connection
  refused, cannot start) is a backend state, not an entry failure. While it
  holds, the drain stops at the first such failure, the entry's `attempts`
  is not increased, and later entries are not tried. Only failures from a
  reachable backend (`BD_WRITE_FAILED`, guard refusal) count towards
  `MAX_ATTEMPTS` or escalate.
- The drain and sync retry with backoff while the backend is unreachable, and
  `status` reports the outage (since when, last error). A sustained outage is
  announced once, like a sync failure, not once per entry.
- Each `bd` call is capped at the time left before the caller's deadline.

Consequences: a long outage leaves writes pending rather than escalated; when
the backend returns, preconditions are re-checked against fresh rows, so
conflicting changes made elsewhere still escalate. Changes BD-CACHE's
retry-cap behaviour; needs agreement on #557.

## ADR-BDC-0006: Prototype a SQLite-backed cron crate here for sc-runtime

**Status:** Draft

Context: the sync task needs a scheduler (BD-CACHE's cron drains and syncs
every 30 s). sc-runtime has no cron or scheduler crate. atm-core's `atm-cron`
(ADR-069 there, phase bk) is a SQLite-backed cron already in use, but it is
bound to ATM: it implements `atm-core`'s sealed `CronScheduler` boundary,
uses `AtmError`, `TeamName`, `PhaseId` and `HookRequest`, keeps its store
port in `atm-storage` and its rusqlite store in `atm-storage-rusqlite`, and
is composed by the ATM daemon.

Decision: build `sc-cron` in this workspace by extracting atm-cron's
scheduler behaviour (expression parse before write, next-run computation,
jitter, missed-tick replay, stop check before every run, `(id, generation)`
fencing so a stale run never writes over a restarted schedule, stores that
read no clock) with a generic job payload and its own error type. The store
port and a rusqlite store live with it. `sc-cron` depends on no bd-cache or
ATM crate, so it can move to sc-runtime unchanged once the sc-runtime team
accepts it; bd-cache then switches its dependency.

Consequences: phase a can deliver the sync task before sc-runtime
integration. Needs agreement with the sc-runtime team on the crate name and
its place in sc-runtime, and a note to the atm-core team, since atm-cron
could later adopt it.

## ADR-BDC-0007: Share the beads backend as `sc-beads`, seeded from btit-bd

**Status:** Draft

Context: beads-task-issue-tracker already has a Rust bd backend:
`btit-beads` (transport-neutral `BeadsBackend`, `CliBackend` and
`DoltOperations` traits, `BeadsError`, pure detection, version-gate and
parsing logic), `btit-cli` (the `CliInvoker` process runner, a
`RecordingInvoker` test double, probes, per-project locks, the issue ops) and
`btit-bd` (`BdCli`, dolt-mode detection), with issue types in `btit-types`
(https://github.com/randlee/beads-task-issue-tracker/tree/develop/crates/btit-bd/src).
It covers more of bd than bd-cache does (create, search, labels, delete,
comments, dep add/remove, sync), which the MCP front end will need. It does
not yet cover what the cache drain and sync rely on: `ListQuery` has no id,
label or `updated_after` filters, `update` takes no guards, `close` takes no
reason, there is no per-call deadline, and rows are parsed tolerantly into
`BdRawIssue`.

Decision: build one shared crate, `sc-beads`, seeded from btit-beads,
btit-cli and btit-bd, in place of a bd-cache-only backend crate. It keeps the
transport-neutral trait so a `bd serve` HTTP adapter fits later (ADR-BDC-0004)
and adds bd-cache's needs: batched `--id` and `--updated-after` listing,
guarded update with exit 13 mapped to a typed refusal, close with reason, a
per-call deadline, the `/v0`-aligned patch (ADR-BDC-0003), and the backend
fingerprint (`bd version` plus `bd schema` sha). Rows come in two modes: the
tolerant mode btit's UI uses today, and a strict mode for bd-cache, generated
from `bd schema` with unknown fields denied and gated on the fingerprint.
Logging moves from btit's global macros to `tracing`. Like `sc-cron`, it is
built here first and moves to sc-runtime (or its own repository) once agreed;
beads-task-issue-tracker then switches to it.

Consequences: phase a starts by porting the btit crates rather than writing a
backend from scratch, and btit's parity and API-freeze tests come with them.
Needs agreement from the beads-task-issue-tracker side on the shared API
(sync calls, error type, tolerant vs strict rows) and from the sc-runtime team
on where the crate lives.

## Per-Crate Architecture

When the workspace declares per-crate docs (see
`.claude/project/quality-policy.md`), each crate's architecture lives in
`docs/<crate>/architecture.md` and is indexed here.
