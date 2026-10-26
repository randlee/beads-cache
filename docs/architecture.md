# beads-cache Architecture

Status: approved. ADR-BDC-0001..0007 are Approved by lead ruling (2026-10-24); ADR-BDC-0008 is Draft for a later phase; ADR-BDC-0009 is Approved (lead ruling S60, 2026-10-25). ADR-BDC-0010 is Approved (lead ruling S82, 2026-10-25) and amended by ADR-BDC-0012. ADR-BDC-0011..0015 are Approved (Rand, 2026-10-26). Plan: `docs/plans/phase-b/phase-b-boundary.md`.

## Overview

beads-cache extracts the beads SQLite cache from its first consumer, where it
runs today alongside an integrated TUI and other orchestration features. The design is baselined on that in-use
implementation, limited to the cache functionality and delivered first as an
MVP; where beads-cache departs from the baseline design, an ADR here
records why.

beads-cache is built against sc-runtime, which is being specified in
parallel. sc-runtime is an upstream dependency: its interfaces and
consumer contract constrain this architecture, and gaps go back to the
sc-runtime team as requests rather than local workarounds.

- Baseline (the contract): the first consumer's cache design document (the
  baseline design) and its reference Python implementation (the baseline
  Python implementation: the cache, its models and its cron script). Acceptance
  suite: the baseline acceptance suite (44 tests) and the baseline cron suite
  (13), with bd-1.3.1 fixtures. Their locations and the design tracker are
  recorded in git history. Where the baseline design and the Python code
  disagree, report it to the first consumer.
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
 backend port:            sc-beads traits: sc-bd-cli (phase a)
                                     sc-bd-http (later)
                                          |
 backend:                 bd  ->  dolt sql-server (local, shared or Hosted Dolt)
                                  or embedded dolt + dolthub:// remote via bd sync
 runtime:                 sc-runtime (process model, sync task scheduling)
```

- The core library knows nothing about its front ends. `bdc` keeps the
  baseline design's CLI contract so the first consumer's hooks and cron keep
  working.
- The MCP server is a second front end over the same core. No upstream beads
  project offers MCP over HTTP today (`beads-mcp` is stdio only, and
  `bd serve` is HTTP but not MCP), so beads-cache provides it.
- Python bindings (maturin / PyO3) are a third front end, built in phase a
  (sprint a-8): a `bd-cache-py` crate wrapping the concrete composition
  (`SqliteStore`, its drain lock, `BdCli`) with the verbs the first consumer
  uses, so it replaces the baseline Python implementation in process. They
  need nothing from sc-runtime; the first consumer keeps its own scheduling
  through its own cron scheduler.
- All writes go through `bd` (see the research docs: writes through the
  DoltHub SQL API bypass bd's guards and events journal).

### Repository configuration

bdc is set up from `.beads/bd-cache.toml`, next to bd's own files, since the
cache is an extension of bd. It sets `cache_path` (default `.bd-cache/<dolt_database>.bd-cache.sqlite3`,
with the drain lock beside it), so btit and other consumers find the cache
wherever it lands. Pointing `cache_path` at a different file switches caches
in one line; that makes separate caches with unrelated schema packages
possible (different teams, different skills), though it is not a feature to
encourage. It also names the sync scopes, the timeouts
in `CacheConfig`, and the schema packages to load by path, for example
a skill's `schemas/` directory, each with an optional enforcement
override (ADR-BDC-0008). Loading it goes through sc-config after sc-runtime
integration; phase a's core takes the resulting `CacheConfig` as plain
values. Phase a loads it with `bd-cache-store`'s `load_repo_config`
(`RepoConfig`, including the `[tick]` table the sync task reads). Its relation
to the first consumer's phase configuration files is settled in ADR-SYNC-0007
(`bd-cache-sync/architecture.md`): the phase files stay the consumer's, and the
sync task reads their root and recipient. *Superseded by ADR-SYNC-0008 (ADR-SYNC-0007 is
Superseded):* the sync reads no consumer file; its scopes come from `.beads/bd-cache.toml` and
the store's registry (ADR-BDC-0013). Moving the loader to sc-config is the later step.

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
  order, with achieved checks, preconditions and read-back (baseline design).
- **Sync:** the sync task (`bd-cache-sync` on `sc-cron` in phase a, on sc-runtime later) refreshes each active scope from
  its own cursor, then refreshes the scope's ready/blocked base from
  `bd blocked -l` and `bd ready -l`; reads adjust that base for our journaled
  writes without a bd call (ADR-BDC-0002).

### Open questions

- ~~Which scope kinds besides `phase-<x>` come first (label set, assignee,
  status).~~ *Answered by ADR-BDC-0013:* a scope is a named bd filter.
- Whether rows are stored in full or as `--brief` rows with full text fetched
  on demand. Needs measurement; full rows are the BD-CACHE baseline.
- MCP auth model (bearer token file like `bd serve`, or OAuth) and which MCP
  tools ship first. Create and comments are out of BD-CACHE v1 but a
  general-purpose MCP will need them.
- Automatic prune of closed scopes (an open item in the baseline design).

## Crate Layout

Status: as shipped by sprints a-1 to a-8 for phase a. Phase a builds only the
crates that do not depend on sc-runtime crates (sc-config, sc-transport,
sc-command, sc-runtime), which are being built in parallel. sc-runtime has no
cron or scheduler crate today, so phase a also prototypes one here (`sc-cron`)
and builds the sync task on it, to finish more of the deliverables before
integrating with sc-runtime.

| Crate | Responsibility | May depend on | Phase |
|---|---|---|---|
| `bd-cache-model` | Pure cache types on top of `sc-beads` rows: `Op` (parse, args JSON, overlay, achieved), journal entries, `replay`, the `CacheStore` and `DrainLock` ports with `FakeStore`, the `EscalationSender` port, `CacheConfig`, `DrainBudget`, reply and report types, `CacheError` with every `BD_CACHE.*` code and the backend-error mapping (ADR-BDC-0003). Phase B (sprint b-7): scopes as named bd filters, `ScopeName`, `ScopeId(ScopeName)`, `ScopeFilter` (`All` / `Criteria`, validated once, with the one conversion to `sc_beads::ScopeFilter`) and `Scope { id, filter }` (ADR-BDC-0013); the 0.1 rule in the compatibility module (`LEGACY_SCOPE_PREFIX`, ADR-BDC-0012); `IngestSnapshot`, `FailingSince`, `LeaseHolder`, `HookState` and `TickBudget`; the codes `CONFIG_INVALID`, `INGEST_INVALID`, `SCHEDULE_INVALID`, `TICK_RUN_FAILED`, `TICK_IN_FLIGHT` and `CacheError::outage` (REQ-BDC-0729) | `sc-beads`, serde, serde_json, toml (private: parses `RepoConfig`, no toml type in the API; ADR-MODEL-0004) | a; b (b-7) |
| `sc-beads` (shared, published) | Beads backend contract only: btit's `BeadsBackend`, `CliBackend` and `DoltOperations` traits plus the cache extensions (batched `--id` and `--updated-after` lists, guarded update and close with reason, per-call deadline, `/v0`-aligned patch, backend fingerprint), strict and tolerant rows, btit's `BeadsError` unchanged plus `BeadsCacheError` and `Refusal` for the cache calls, `Clock` and `Deadline`, a `FakeBackend` test double (ADR-BDC-0007). Sprint b-7: `ScopeFilter` gains `All` and `Criteria { labels_all, labels_any, issue_type, assignee }` (no status), with `validate` refusing an empty `Criteria` before any bd call (`BeadsCacheError::Invalid`); `FakeBackend` answers every variant, and the contract suite checks each against the bd 1.3.1 scope-filter recording (ADR-BDC-0013) | serde, serde_json, sha2, tracing | a |
| `sc-bd-cli` (shared, published) | The bd CLI implementation of the `sc-beads` traits, ported from btit-cli and btit-bd: invoker with a recording test double, probes and version gates, dolt-mode detection, per-project locks; exit 13 as a typed refusal. Sprint b-7: the `bd list`, `bd blocked` and `bd ready` argv of each `ScopeFilter` variant (ADR-BDC-0013's table, never a status), `FakeBd` and the `fake-bd` binary answering each, with bd 1.3.1 fixtures | `sc-beads`, serde_json, tracing | a |
| `sc-bd-http` (shared, published) | The `bd serve` HTTP implementation of the same traits | `sc-beads` | when bd releases the `id` and `updated_after` filters |
| `bd-cache-store` | Implements `CacheStore` and `DrainLock` over SQLite: BD-CACHE v3 tables unchanged plus additive tables and nullable columns, file location from `.beads/metadata.json`, meta and rebuild rule, journal, scopes and membership, edges index, outage record, cross-process drain lock (`flock`). Sprint b-7: the ADR-STORE-0006 extensions (pinned by the format golden), the scope registry with filters, `ingest_scope`, failure state and marking, lease and hook state, and `CacheScheduleStore`, sc-cron's `ScheduleStore` over the cache's one connection (ADR-BDC-0014) | `bd-cache-model`, `sc-beads` (logging through `sc_beads::logging::tracing`), `sc-cron` (ADR-BDC-0014), rusqlite (bundled), fs4, serde_json (the list and the test-only features are ADR-STORE-0005, amended by ADR-BDC-0014) | a; b (b-7) |
| `bd-cache` (core library) | `Cache`: get, list, write, drain, sync, resolve, prune, status, the ready base and local adjustment (ADR-BDC-0002) and outage handling (ADR-BDC-0005); shared `view`, `backend_check` and `miss` helpers; takes a `CacheConfig` and a `Clock` | `bd-cache-model`, `sc-beads` | a |
| `bd-cache-api` (ADR-BDC-0009; `docs/bd-cache-api/`) | Request and data models, the shared request decoder, the operation layer (filters, paging, order, `/v0` mapping), the error table (`ERROR_TABLE`, `ApiError`), schemars request schemas, the shared fixtures; no sc-command, no clap (CI-checked) | `bd-cache`, `bd-cache-model`, `sc-beads`, serde, serde_json, schemars | b |
| `bd-cache-cli` (binaries `bd-cache`, `bdc`; ADR-BDC-0009; `docs/bd-cache-cli/`) | JSON-first CLI per `docs/bd-cache-cli/contract.md` (sprint b-3): clap argv to the JSON request, `bd_cache_api::decode` then `execute`, `cache tick` over `bd_cache_sync::tick`, one sc-command envelope on stdout, `--format text`, `--help` (text on a terminal), the stderr log. Config: `.beads/bd-cache.toml` read by the store's rules (a `bd` key ignored) plus `BD_CACHE__*` env overrides and flags, opened through `bd_cache_sync::compose_with_config`; no sc-config dependency until it offers a missing-allowed file source. The `fake-bd` harness behind feature `fake-bd` (ADR-CLI-0001, 0002) | `bd-cache-api`, `bd-cache-store`, `sc-bd-cli`, `bd-cache-sync`, `bd-cache-model`, `sc-beads`, sc-command, sc-observability-types, clap, serde, serde_json, toml | b |
| `sc-cron` (prototype, moves to sc-runtime) | SQLite-backed cron scheduler extracted from the first consumer's cron scheduler: expression parse, next-run computation, jitter, missed-tick replay, stop check before every run, generation fencing of overlapping runs, schedule store with a rusqlite implementation. Generic job payload and error type; no consumer or bd-cache types (ADR-BDC-0006) | cron, chrono, tokio (with process, io-util), rusqlite (bundled), async-trait, serde_json, getrandom, process-wrap (hook process group and job object), tempfile (a hook's working directory), tracing | a |
| `bd-cache-sync` | The sync task on `sc-cron`: `tick` (drain with `T - 5 s` / `T - 2 s`, then sync each active phase, deferring any not started by `T - 2 s`), `stop_reason`, announcements through `EscalationSender` (sync failures once per cause, outage once per scope as the alert `backend_unavailable`, escalations once with the `resolve` command; the consumer's message sender is one implementation), `SyncJob` (the `sc-cron` job: `Cache` in `Arc<Mutex<_>>` under `spawn_blocking`), `schedule_spec`, and `compose` (the real `SqliteStore` + `FlockDrainLock` + `BdCli` cache of a repository). Crate-local decisions: `ADR-SYNC-*` | `bd-cache`, `bd-cache-model`, `bd-cache-store`, `sc-bd-cli`, `sc-cron`, `sc-beads`, tokio, async-trait, serde, serde_json, toml (phase files), tracing, libc (Unix only: O_NONBLOCK) (the list is ADR-SYNC-0006) | a |
| `bd-cache-py` | Python module (maturin / PyO3) over the concrete composition with the first consumer's verbs; replies as dicts, `CacheError` as one exception class | `bd-cache`, `bd-cache-store`, `sc-bd-cli`, `bd-cache-model`, `sc-beads`, pyo3, serde, serde_json, dunce | a |
| MCP server | HTTPS MCP front end over `bd-cache-api` (ADR-BDC-0009) | `bd-cache-api`, `bd-cache-sync`, sc-command (server), sc-runtime, sc-transport | later |

Phase a acceptance runs through the core library API with the fake
backend: the ported cases from the baseline acceptance suite (44), the 13 cases from
the baseline cron suite against the sync task with an injected sender, and
the bd-1.3.1 contract fixtures. `sc-cron` carries its own tests, ported from
the first consumer's cron scheduler where they apply. Sprint a-7 adds the end-to-end flow through the
real composition (`SqliteStore`, `FlockDrainLock`, `BdCli` over a recording
fake `bd`, `SyncJob` on a scheduler) in CI, and the same flow against a
scratch `bd` workspace on the testbed
(`docs/plans/phase-a/evidence/a-7-testbed.md`).

## Boundaries and Dependencies

Normative; CI checks it with `cargo metadata` (a-1). Allowed edges between
workspace crates (third-party edges are in the Crate Layout table):

| Crate | May depend on |
|---|---|
| `sc-beads` | none |
| `sc-bd-cli` | `sc-beads` |
| `sc-cron` | none |
| `bd-cache-model` | `sc-beads` |
| `bd-cache-store` | `bd-cache-model`, `sc-beads`, `sc-cron` (ADR-BDC-0014, sprint b-7) |
| `bd-cache` | `bd-cache-model`, `sc-beads` (never the store or `sc-bd-cli`) |
| `bd-cache-sync` | `bd-cache`, `bd-cache-store`, `bd-cache-model`, `sc-bd-cli`, `sc-cron`, `sc-beads` |
| `bd-cache-py` | `bd-cache`, `bd-cache-store`, `bd-cache-model`, `sc-bd-cli`, `sc-beads`, `bd-cache-api` (ADR-BDC-0009) |
| `bd-cache-api` | `bd-cache`, `bd-cache-model`, `sc-beads` (ADR-BDC-0009) |
| `bd-cache-cli` | `bd-cache-api`, `bd-cache-store`, `sc-bd-cli`, `bd-cache-sync`, `bd-cache-model`, `sc-beads` (ADR-BDC-0009) |

`bd-cache-model` does no I/O. `sc-beads`, `sc-bd-cli` and `sc-cron` depend on
no bd-cache crate, so they can move or be published on their own; the edge
`bd-cache-store` → `sc-cron` (ADR-BDC-0014) points from the cache to the
scheduler, never back.

## Architecture Decision Records

Each ADR is its own section, `## ADR-<DOMAIN>-nnnn: Title`, where `<DOMAIN>`
is the product's domain code, or the crate's domain code in
per-crate docs. ADR template: `**Status:**` (Draft, Proposed, Approved,
Active, Superseded), context, decision, and consequences. An ADR changes only
through a later ADR that names the one it amends or supersedes. Domain
code: `BDC`.

### Implementation of the repo-level ADRs (phase a)

Where each ADR is implemented and decided in detail, from the crate docs
(a-7 keeps this table current):

| ADR | Status | Implemented in |
|---|---|---|
| ADR-BDC-0001 | Approved | `bd-cache-store` (ADR-STORE-0001, 0002: scope registry, membership), `bd-cache` (ADR-CORE-0204, 0205: scoped reads, prune) |
| ADR-BDC-0002 | Approved | `bd-cache` (ADR-CORE-0201, 0203: the ready adjustment, bounded fresh ready; ADR-CORE-0206: `get` reads the bead's own entry), `bd-cache-store` (ready base; ADR-STORE-0004: its per-bead index), `bd-cache-sync` (the base is refreshed by each sync; end-to-end in `tests/e2e.rs`) |
| ADR-BDC-0003 | Approved | `bd-cache-model` (ADR-MODEL-0001, 0002: ops, patch shapes, error reasons) |
| ADR-BDC-0004 | Approved | `sc-beads` (`CacheBackend`, ADR-BEADS-0002), `sc-bd-cli` (ADR-BDCLI-0001, 0002, amended by 0004) |
| ADR-BDC-0005 | Approved | `bd-cache` (REQ-CORE-0001, 0002; ADR-CORE-0101, 0202), `bd-cache-sync` (ADR-SYNC-0003: the outage announcement, *superseded by ADR-SYNC-0008: the outage is a scope result given to the post-sync hook*; ADR-SYNC-0002: the threading it runs under) |
| ADR-BDC-0006 | Approved | `sc-cron` (ADR-CRON-0001..0006), `bd-cache-sync` (dependencies listed in the Crate Layout) |
| ADR-BDC-0007 | Approved | `sc-beads` (ADR-BEADS-0001..0004), `sc-bd-cli` (ADR-BDCLI-0001..0004) |
| ADR-BDC-0008 | Draft | not in phase a |
| ADR-BDC-0009 | Approved | `bd-cache-api` (sprint b-2: the request vocabulary, decoder, operations and error table; ADR-API-0001..0003 in `docs/bd-cache-api/architecture.md`); `bd-cache-cli` (sprint b-3: both command groups of `docs/bd-cache-cli/contract.md` §1–§8, the exit codes, configuration and help; ADR-CLI-0001..0004 in `docs/bd-cache-cli/architecture.md`) |
| ADR-BDC-0010 | Approved; amended by ADR-BDC-0012 | `scripts/lint_no_atm.py` and its tests, run per sprint with `--paths`; removals in sprints b-7 and b-8; the exemption change and the CI lint gate in sprint b-9 (ADR-BDC-0012) |
| ADR-BDC-0011 | Approved | `bd-cache-store` (ADR-STORE-0006), `bd-cache-sync` (ADR-SYNC-0008), `sc-cron` (ADR-CRON-0009); sprints b-7 and b-8 |
| ADR-BDC-0012 | Approved | `scripts/lint_no_atm.py` exemptions and the CI step, sprint b-9 |
| ADR-BDC-0013 | Approved | `bd-cache-model`, `sc-beads`, `sc-bd-cli`, `bd-cache-store`, the FakeBd harnesses and the callers, sprint b-7 |
| ADR-BDC-0014 | Approved | `sc-cron` store trait and `bd-cache-store`'s `CacheScheduleStore`, sprint b-7 |
| ADR-BDC-0015 | Approved | `bd-cache-api` models and `bd-cache-cli` / `bd-cache-py` execution of the new verbs, sprint b-8 |

## ADR-BDC-0001: Sync scopes with per-scope cursors and bead membership

**Status:** Approved (lead ruling 2026-10-24: approved with the phase-a plan, merged by the owner in PR #1); amended by ADR-BDC-0013 (scopes are named bd filters).

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

Consequences: extends the BD-CACHE v3 file additively at `user_version` 3;
the Python implementation reads the extended file, and the Rust store
tolerates rows Python leaves in the extension tables. No version bump. Needs agreement with
the first consumer.

## ADR-BDC-0002: Blocked state from bd, one query per scope

**Status:** Approved (lead ruling 2026-10-24: approved with the phase-a plan, merged by the owner in PR #1); amended by ADR-BDC-0013 (the base of a filter scope).

Context: a filtered cache must answer `ready` and blocked state correctly,
including when a blocker is outside every scope, and when a blocker is closed
or reopened. The probe in
[`research/bd-blocked-state-probe.md`](research/bd-blocked-state-probe.md)
(P-6) found:

- `bd list --json` and `bd show --json` never emit `is_blocked`, so cached
  rows carry no blocked state at all (`bd schema` declares it optional).
- bd settles blocked state without bumping the depender's `updated_at`
  (`BlockedStateInvariant`, beads `issueops/blockedstate.go`; the first consumer's
  maintainer confirmed 18 of 1279 live `blocks` edges), so `--updated-after` sync cannot see it
  change either.
- `blocks` edges block; a child inherits its parent's blocked state through
  `parent-child`; `related` does not block. Other types were not probed.
- `bd blocked -l <label> --json` returns bd's own blocked set for a label,
  each row with `blocked_by`, including blockers outside the label and
  inherited parent-child blocking.

The live Python cache (BD-CACHE v3) is not affected: it never stores or
serves blocked state, and readiness there comes from live `bd ready`.

Readiness changes in the workflows this cache serves come from our own
writes: a dev close releases its sanity bead, which is then assigned
(by the first consumer's task-assignment command); the sanity agent's
task-open hook checks
`ready` through the cache (Python bindings) one or two turns later and
escalates if the bead is not ready. Under host load bd cannot be asked in
time (calls time out even at 30 s), so `ready` must reflect our writes
without a bd call, and a stale "not ready" after our own close would raise a
false escalation.

Decision: bd's answer is the base; our journaled writes adjust it locally.

1. **Base.** Each scope's sync ends with `bd blocked -l <label> --json`
   (blocked beads with `blocked_by`) and `bd ready -l <label> -n 0 --json`,
   stored as the scope's base with its fetch time. bd stays the authority for
   everything the cache does not model (custom statuses, issue types,
   deferral, wisps). A failed or cut fetch keeps the previous base, marked
   stale.
2. **Local adjustment for our ops.** Reads apply to the base, in `seq`
   order, every Pending entry plus every entry applied at or after the base's
   `fetched_at` (the instant before that sync's first `bd blocked`/`bd ready`
   call); Escalated and Resolved entries are skipped. The rules (full table in
   `docs/bd-cache/read-path.md`, owned by a-6):
   - close of X: remove X from every stored `blocked_by`; a bead whose list
     becomes empty, and whose own status is `open`, is ready. Repeat through
     parent-child (a child is listed as blocked by its blocked parent).
   - A close is a `Close` op or an `Update` whose status is `closed`. A
     reopen is an `Update` to any status other than `closed` of a bead whose
     view was `closed`: add X back to `blocked_by` of beads with a blocking,
     non-parent-child edge to X (cached edges, `DepType::is_blocking`), and
     recompute X's own `blocked_by` from its cached edges. Its parent-child
     children are blocked only if X itself ends up blocked (next rule).
     (Clarified by lead ruling, bc-lead, 2026-10-24, after a-6.)
   - Parent-child, from stored edges: bd lists a child of a blocked parent as
     blocked by the parent (probe), so unblocking a parent clears it from its
     children's `blocked_by`, and blocking a parent adds it.
   - dep_add `blocks` onto a target that is not closed, or whose status is
     unknown (no cached row): add the target to the bead's `blocked_by`.
   - close or status change of the bead itself: it leaves `ready`.
3. **Bounded fresh answer.** A caller may pass a time limit (for example
   500 ms). `ready` then makes the scope's `bd ready -l` and `bd blocked -l`
   calls under that limit; if they answer, the reply is bd's answer (adjusted
   for entries still pending) and the base is refreshed; if not, the reply is
   the cached, adjusted answer at the limit, naming the error as
   `read_fallback`. Without a limit, `ready` makes no bd call.
4. **No base yet** (cold cache): `ready` reports `unknown`; the caller's own
   checks (for example the `task open` hook's escalation) cover it.

Readers must go through the cache (bindings, `bdc`, later MCP); raw `bd`
reads see the backend only after the drain lands. Every `ready` reply says
its basis (bd fetch time, locally adjusted).

Consequences: two small `bd` calls per scope per sync (`bd blocked` measured
0.07-0.08 s on a loaded dev host); a ready transition caused by our own
write is visible immediately even with bd unreachable; changes made outside
the cache arrive only when a sync gets through. The live Python cache is
not affected (it never serves blocked state).

## ADR-BDC-0003: Align data vocabulary with the bd serve /v0 API

**Status:** Approved (lead ruling 2026-10-24: approved with the phase-a plan, merged by the owner in PR #1)

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
`status`), because a journaled write is not an applied write. *(This
command-structure clause is superseded by ADR-BDC-0009; the rest of this ADR
stands.)*

Consequences: every change is additive to the BD-CACHE CLI contract and
needs agreement on #557.

## ADR-BDC-0004: Backend port with the bd CLI adapter first

**Status:** Approved (lead ruling 2026-10-24: approved with the phase-a plan, merged by the owner in PR #1)

Context: released bd (1.3.0/1.3.1) has no `id` or `updated_after` filter on
`bd serve`, and bd serve needs a dolt sql-server. The bd CLI works with every
backend bd supports, including Hosted Dolt and embedded dolt with a remote.

Decision: all backend access goes through the `sc-beads` traits
(ADR-BDC-0007). The cache uses `CacheBackend: BeadsBackend`, whose methods
all take a `Deadline`: `fingerprint`, `list_by_ids`, `list_scope(filter,
since)`, `list_blocked(filter)`, `list_ready(filter)`, `close_guarded`, `update_guarded`,
`dep_add_typed`. Phase a builds only the bd CLI implementation (`sc-bd-cli`).
The cursor is an RFC 3339 string minted by the cache from a sync's start time;
the backend treats it as opaque, so a commit hash or events `seq` can replace
it later. Nullable `revision`, `backend_kind`, `endpoint`, `replica_id` and
`journal_seq` columns leave room for the HTTP implementation without a schema
change.

Consequences: an HTTP adapter can be added when a bd release ships the
missing filters; beads-task-issue-tracker already plans one for the same
trait, so bd-cache gets it through `sc-beads`. The unverified gap that `--updated-after` may miss deletes
and edge changes is tracked for a later commit-hash cursor.

## ADR-BDC-0005: The backend may be slow or unreachable

**Status:** Approved (lead ruling 2026-10-24: approved with the phase-a plan, merged by the owner in PR #1)

Context: two deployments are expected: a dolt server on a remote machine over
an unreliable network, and a local server on a host so loaded that `bd`
calls take seconds (the BD-CACHE motivation). BD-CACHE v3 escalates a write
after `MAX_ATTEMPTS` (5) transient failures, so a backend outage of a few
cron ticks escalates every pending write to the phase's recipient.

Decision:
- Reads never wait long on the backend. Every read reply carries the scope's
  `last_ok_at` and `stale` per row, so callers and MCP tools can see how
  fresh the data is. A cache miss fetches with a short deadline
  (`CacheConfig::miss_fill_timeout`, default 2 s) and returns
  `BD_CACHE.BD_UNAVAILABLE` when it expires.
- Backend unreachable (timeout, connection refused, cannot start) at the
  backend check, the drain pre-read or a sync or refresh list is a backend
  state, recorded in the store as the outage (since when, latest error). It
  is cleared only by a `bd list` that answers: `bd version` can pass while
  lists time out on a loaded host. A write that fails or times out counts an
  attempt, as in v3, since it may have reached the backend. While it holds, the drain
  stops without counting an attempt, and a sync that meets it records the
  outage instead of an announced `SYNC_FAILED`. These are the rules v3 adopted
  in the baseline at 05831df (Rand approved); the two implementations must apply the
  same rules because they share a journal, and only one may drain it.
  Only failures from a reachable backend (`BD_WRITE_FAILED`, guard refusal)
  count towards `MAX_ATTEMPTS` or escalate.
- No extra backoff: the cron tick (30 s) is the retry interval. `status`
  reports the outage. An outage is announced through each active scope's
  sync alert under the stable cause key `backend_unavailable`, so each recipient
  hears it once; its end resets the alert without a message.
- The drain has a start gate and a hard deadline (REQ-BDC-0402): each `bd`
  call is capped at the time left before the hard deadline; a call cut short
  by it is neither an outage nor an attempt.

Consequences: a long outage leaves writes pending rather than escalated; when
the backend returns, preconditions are re-checked against fresh rows, so
conflicting changes made elsewhere still escalate. Changes BD-CACHE's
retry-cap behaviour; needs agreement on #557.

## ADR-BDC-0006: Prototype a SQLite-backed cron crate here for sc-runtime

**Status:** Approved (lead ruling 2026-10-24, after a-3 QA-2: covers the decision text as amended in integrate/phase-a 3f87332, including the third-party dependency list cron, chrono, tokio, rusqlite (bundled), async-trait, serde_json, getrandom, tracing; extended by lead ruling 2026-10-23 to cover the integrate/phase-a 4dbc1ad amendment adding the dev-dependency tempfile). Amended by ADR-BDC-0011 (the general hook target and the dependency list) and ADR-BDC-0014 (a caller-supplied schedule store). Provenance restated neutrally under ADR-BDC-0010, with no change of meaning.

Context: the sync task needs a scheduler (BD-CACHE's cron drains and syncs
every 30 s). sc-runtime has no cron or scheduler crate. The first consumer's
cron scheduler (an ADR in that project) is a SQLite-backed cron already in
use, but it is bound to its host: it implements the host's sealed scheduler
boundary, uses the host's error, team, phase and hook-request types, keeps
its store port and its rusqlite store in the host's storage crates, and is
composed by the host's daemon.

Decision: build `sc-cron` in this workspace by extracting that scheduler's
behaviour (expression parse before write, next-run computation,
jitter, missed-tick replay, stop check before every run, `(id, generation)`
fencing so a stale run never writes over a restarted schedule, stores that
read no clock) with a generic job payload and its own error type. The store
port and a rusqlite store live with it. `sc-cron` depends on no bd-cache or
consumer crate, so it can move to sc-runtime unchanged once the sc-runtime team
accepts it; bd-cache then switches its dependency.

Third-party dependencies of `sc-cron`: cron, chrono, tokio, rusqlite
(bundled), async-trait, serde_json, getrandom, tracing (amended after a-3
QA-1); dev-dependency tempfile for its store tests (lead amendment,
2026-10-23). Its schedule store uses its own SQLite file and connection
(NFR-BDC-0007).

*Amended by ADR-BDC-0011:* the dependency list adds tokio's `process` and
`io-util` features, process-wrap, and tempfile as a normal dependency (a
Python hook's working directory). *Amended by ADR-BDC-0014:* the own-file
store (`SqliteScheduleStore`) is sc-cron's standalone default; a caller may
supply any `ScheduleStore`, and bd-cache supplies one inside the cache file
over the cache's single connection.

Consequences: phase a can deliver the sync task before sc-runtime
integration. Needs agreement with the sc-runtime team on the crate name and
its place in sc-runtime, and a note to the first consumer's team, since its
scheduler could later adopt it.

## ADR-BDC-0007: Shared beads backend crates, seeded from btit

**Status:** Approved (lead ruling 2026-10-24: approved with the phase-a plan, merged by the owner in PR #1); amended by ADR-BDC-0013 (`ScopeFilter` gains `All` and `Criteria`).

Context: beads-task-issue-tracker already has a Rust bd backend:
`btit-beads` (transport-neutral `BeadsBackend`, `CliBackend` and
`DoltOperations` traits, `BeadsError`, pure detection, version-gate and
parsing logic), `btit-cli` (the `CliInvoker` process runner, a
`RecordingInvoker` test double, probes, per-project locks, the issue ops) and
`btit-bd` (`BdCli`, dolt-mode detection), with issue types in `btit-types`
(https://github.com/randlee/beads-task-issue-tracker/tree/develop/crates/btit-bd/src).
Its trait already makes the backend swappable, and an HTTP backend is planned
for when bd releases it. It covers more of bd than bd-cache does (create,
search, labels, delete, comments, dep add/remove, sync), which the MCP front
end will need. It does not yet cover what the cache drain and sync rely on:
`ListQuery` has no id, label or `updated_after` filters, `update` takes no
guards, `close` takes no reason, there is no per-call deadline, and rows are
parsed tolerantly into `BdRawIssue`.

Decision: three shared crates, built in this repository and published to
crates.io. btit's `BeadsBackend`, `CliBackend` and `DoltOperations` traits are
kept unchanged; the cache's methods go in a separate extension trait,
`CacheBackend: BeadsBackend`, which `sc-bd-cli` (and later `sc-bd-http`)
implements alongside btit's traits:

- `sc-beads`: the backend contract only (traits, row types, errors,
  `FakeBackend`), seeded from btit-beads and btit-types.
- `sc-bd-cli`: the bd CLI implementation, seeded from btit-cli and btit-bd.
- `sc-bd-http`: the `bd serve` implementation, once bd ships the filters the
  cache needs (ADR-BDC-0004).

Every trait and feature btit uses today is kept with its signature, so btit
switches to the crates.io releases without code changes beyond imports. The
cache's needs are additions: batched `--id` and `--updated-after` listing,
guarded update with exit 13 mapped to a typed refusal, close with reason, a
per-call deadline, the `/v0`-aligned patch (ADR-BDC-0003), the backend
fingerprint, and a strict row mode (generated from `bd schema`, unknown
fields denied, gated on the fingerprint) beside btit's tolerant rows.
Logging moves from btit's global macros to `tracing`.

The point is one source of truth for bd's interface. When bd makes an
incompatible change, the upgrade happens once, in these crates, and every
consumer (bd-cache, btit, later the MCP server) takes it as a semver-major
release instead of patching its own copy.

Consequences: phase a ports the btit crates rather than writing a backend
from scratch, and btit's parity and API-freeze tests come with them.
bd-incompatible changes become breaking releases of `sc-beads` and
`sc-bd-cli`. Needs agreement from the btit side on the shared API (sync
calls, error type, tolerant vs strict rows).

P-3 outcome (sprint a-1, owner amendment by Rand): btit's `BeadsError` is
unchanged; cache errors are `BeadsCacheError` in `sc-beads`; this resolves
P-3's error-type question. `BeadsCacheError` carries `Unavailable { reason,
timed_out }`, `Refused(Refusal)`, `Contract { bead, field, detail }` and
`Failed { code, stderr }`, plus `Btit(BeadsError)` with
`From<BeadsError>`; every `CacheBackend` method returns it, and btit's
traits keep returning `BeadsError`. The calls stay synchronous, and both row
modes ship: btit's tolerant `BdRawIssue` unchanged, and the strict `Bead`
generated from `bd schema` (ADR-BEADS-0002 and ADR-BEADS-0003 in
[`sc-beads/architecture.md`](sc-beads/architecture.md)). Logging moved to
`tracing` behind btit's unchanged `log_*!` macro names (ADR-BEADS-0001).
The additional `BeadsCacheError::CheckFailed` variant (typed check step) is
recorded in ADR-BEADS-0002, accepted by lead ruling (bc-lead, 2026-10-23).

## ADR-BDC-0008: User metadata extensions as consumer-owned schema packages

**Status:** Draft (later phase; not in phase a)

Context: teams extend beads with their own fields in bd's free-form
`metadata`, and may require some of them. The first consumer's bead skill already does
this: its Pydantic models (`bead_schema.py`) are exported as JSON Schemas in
the skill's own `schemas/` directory (dev, fix, sanity, qa, finding, gate,
root, sprint and stack beads), with role-based fields
stamped at pour and required at close (`close_fields.py`), and structured
values such as the QA bead's list of required review agents. bd accepts any
metadata and enforces none of it. The schemas belong to the skill that
defines them and change when that skill is upgraded, so they cannot be
compiled into, or released with, the shared crates.

Decision (proposed):
- **Schema packages ship with their consumer.** A package is a directory the
  skill owns (for the first consumer's bead skill, its `schemas/`), plus a small manifest: package
  name, version, and for each schema the beads it selects (`issue_type`, a
  label, or a `metadata.role` value) and its lifecycle rules (required at
  pour, required at close). The skill's own models stay the source and
  export the package.
- **bdc loads packages at runtime** from the paths in the repository's bdc
  config (`.beads/bd-cache.toml`), with no bdc
  release needed to add or change one, and validates with a JSON Schema
  validator in `sc-beads` that btit and the later PyO3 bindings share.
- **Configurable enforcement.** Each package sets a default mode (`off`,
  `warn` or `enforce`), and a repository's bdc configuration can override it
  per package. `off` loads nothing; `warn` reports violations; `enforce`
  also refuses violating writes.
- **Versions.** A pour stamps the package and version into the bead's
  metadata (for example `schema: "example-beads/qa-bead@3"`), so a bead always
  says which schema it was made under. The first implementation loads one
  version per package and reports beads stamped with another version
  instead of validating them. Loading several versions at once is the
  extension for long-lived viewers such as btit, where schemas change over
  a database's life; the stamp is what makes it possible later.
- **Enforced on writes in bdc:** the view after the write is checked against
  the bead's schema; a violation is a typed refusal
  (`BD_CACHE.EXTENSION_INVALID` with the failing fields), or a warning when
  the package marks it `warn`.
- **Reported, not rejected, on sync:** agents still write with raw `bd`, so
  sync records violations and `get` / `status` show them. bd's own row
  contract stays strict and separate.
- **btit** uses the same packages to render typed fields, build edit forms
  and show violations.
- Rules that JSON Schema cannot express (for example "ids, or exactly
  `["NONE"]`", or checks across beads) are either written as JSON Schema
  where possible (`if`/`then`, `dependentRequired`) or left to the skill's
  own Python validation in CI or `doctor`. Running Pydantic in-process is not
  an option (Pydantic's Monty interpreter cannot import third-party packages);
  sandboxed plain-Python rule scripts are a possible later addition.

Open:
- Structured values (the first consumer's maintainer, 2026-10-24). Lists such as the QA bead's
  `reviewers_round1` and the dev bead's `requirements` / `adrs` are written
  at pour by `bd import` and stored as real lists. Later per-key writes use
  `--set-metadata k=<json text>`, which bd stores as a string that readers
  decode; the journal already accepts such strings when they round-trip.
  The one structured rewrite after pour is the stack bead's `layers` list,
  a read-modify-write through `bd update --metadata @file`, which the first
  consumer treats as replacing the whole object (not yet probed). The cache needs a
  whole-metadata replace op guarded by a precondition on the previous value
  for that case.
- The manifest format and the version-stamp key.
- Long term, schema packages may live in dolt as an extension table, so a
  database carries its own schemas and versions. Out of scope for this
  work; the package format should not rule it out.

Consequences: adds schema-package loading, version selection and a validator
to `sc-beads`, a new error code, and a pour-time version stamp in each
consuming skill.

## ADR-BDC-0009: CLI and MCP command structure: a /v0-shaped beads surface and a cache surface

**Status:** Approved (lead ruling S60, bc-lead, 2026-10-25). Amends ADR-BDC-0003. Amended by ADR-BDC-0015 (where the phase-B verbs are modelled and run).

Context: ADR-BDC-0003 aligned the data vocabulary with bd serve `/v0` but kept
BD-CACHE's command structure ("align the data vocabulary, not the command
structure"; the CLI keeps `get`, `list`, `write`, `drain`, `sync`, `resolve`,
`prune`, `status`). Rand's direction (2026-10-24) is that an agent using the
cache over MCP should see the same operations as talking to beads directly,
and that cache management should use conventional cache terms. Lead ruling
S59 superseded REQ-BDC-0806 (CLI drop-in), so no hook depends on the old
command lines.

Decision:
- **Supersedes ADR-BDC-0003's command-structure clause only.** Its data
  vocabulary (patch keys, `expected_status` / `expected_assignee`, `/v0`
  refusal names, `BD_CACHE.*` codes) stands.
- The CLI and MCP tools expose two surfaces (`docs/bd-cache-cli/contract.md`):
  a beads surface shaped like `/v0` at a pinned beads release (`bdc issues
  get|list|update|close`, `bdc ready`, `bdc dependencies list|add`, `bdc
  context`), whose bodies are the `/v0` bodies plus one `x_cache` member,
  with a published divergence list; and a cache surface (`bdc cache status|
  health|scopes|refresh|flush|evict|journal|tick`).
- A journaled write is still not an applied write: every write reply names
  its journal `seq` and `state`.
- Every reply is an sc-command envelope; the request, data and error models
  live in a new `bd-cache-api` crate shared by the CLI, the MCP server and
  `bd-cache-py`.

Consequences: REQ-BDC-0801, 0802 and 0803 are amended as in the contract's
Appendix A, applied in `docs/requirements.md` (S60); new crates `bd-cache-api` and `bd-cache-cli`
in the Crate Layout and Boundaries tables; the Python binding gains the two
surfaces additively (wheel 0.2.0) and keeps its 0.1 verbs.

## ADR-BDC-0010: No consumer-specific concerns in beads-cache

**Status:** Approved (lead ruling S82, 2026-10-25); amended by ADR-BDC-0012 (exemptions, prohibited list, the two-row sync, the sprints)

Context: phases a and b ported the first consumer's cache cron into
`bd-cache-sync`: the tick sends notifications on the consumer's behalf under
its cron principal and sender identity (through the consumer's send command,
in the messaging module), reads the consumer's message-routing settings from
the `[tick]` table, discovers the consumer's phase configuration files (each
with a root and a recipient), implements the consumer's cron stop rule and tick
protocol (`refused` / `pass-through` replies), syncs only scopes named by the
consumer's phase naming, and carries compat tests that vendor the baseline
Python implementation (`crates/bd-cache-store/tests/compat/`) and its call log
(the `crates/bd-cache-py/tests/` files whose paths the lint flags); the exact
names are in git history. Rand ruled this out of scope (2026-10-25): the
boundary is strict, with no concern or pattern of the consumer in beads-cache,
which is a cache with a CLI for configuration and a Python API through PyO3
for the consumer to use (Rand, 2026-10-25, paraphrased). He also ruled
(2026-10-25) that the beads-cache cron must support running a registered
Python script that ships in the consumer's own package; the consumer depends on
beads-cache for that plug-in and uses the cache both from its Python hooks and
by direct reads from its TUI (Rand, 2026-10-25, paraphrased).

Decision:
- beads-cache is a cache (store, journal, drain, sync and refresh of
  configured scopes, idempotent writes), a CLI for configuration and
  management, and a PyO3 API. Nothing else.
- It sends no messages, reads no consumer's configuration, files or paths,
  implements no consumer-shaped protocol, and names no consumer concept in
  source, tests, scripts, workflows or docs. Scopes are configured in
  beads-cache's own configuration, never discovered from a consumer's files.
- Notification state is neutral data the caller reads and marks (for example
  an escalated entry not yet notified, an outage or alert per scope); the
  cache does not decide who is told or how.
- Scheduling is `sc-cron`, which stays general purpose. A schedule row has a
  cron string, a timeout, a JSON payload, an `enabled` flag and a hook target:
  `Rust` (an in-process `Job`) or `Python` (an interpreter and a module, run
  as a subprocess with one JSON object on stdin, for the `cron.tick` and
  `cron.stop` events). The bd sync is one row. Its target switches between
  the built-in sync and a consumer's Python replacement, which feeds the cache
  through `ingest_scope`. The built-in sync calls an optional post-sync Python
  hook with the sync results, errors and unnotified escalations. The hook marks
  what it handled through the cache API. The cache sends nothing and knows
  nothing about what the hook does (plan:
  `docs/plans/phase-b/phase-b-boundary.md`). *Amended by ADR-BDC-0012:* the
  bd sync is two rows, a built-in row and a consumer's Python row, at most one
  enabled, switched with `bdc cache sync use-python` / `use-builtin`
  (ADR-BDC-0011, ADR-SYNC-0008).
- Consumers use the PyO3 API from their hooks and for direct reads (a TUI).
- Enforcement: `scripts/lint_no_atm.py`, a CI gate in the `ci` lint job
  from sprint b-6 (*amended by ADR-BDC-0012:* from sprint b-9), with no
  inline suppression and no allowlist file. Until
  then each sprint runs it on its own files with `--paths <prefix>...`. It
  scans the product: the tracked files under `crates/`, `docs/` and
  `scripts/`, plus `Cargo.toml`, `README.md`, `rust-toolchain.toml` and the
  repo's own workflows `ci.yml`, `python.yml` and `bd-compat.yml`. It matches
  each in-scope file's contents line by line, and its tracked path as well
  (a path hit is reported at line 0). Within that scope its only exemptions
  are the lint script and its tests (`scripts/lint_no_atm.py`,
  `scripts/test_lint_no_atm.py`), `docs/team-protocol.md` (the protocol of
  the agent team working on this repo), `docs/plans/**`, and this section of
  this file (*amended by ADR-BDC-0012:* these last two exemptions are
  removed). Out of scope by construction, as dev
  tooling and not product: the agent tooling (`.claude/**`, `.cursor/**`,
  `.codex/**`, `.agents/**`, `CLAUDE.md`, `AGENTS.md`), which the repo's own
  agents use to coordinate; the sc-publish kit, frozen upstream
  (`.github/scripts/**`, `.github/actions/**`, the `*-publish.yml`,
  `release*.yml` and `prerelease-archive.yml` workflows, `release/**`,
  `install.py`, `README.sc-publish.md`, and the kit docs
  `docs/getting-started.md`, `docs/npm-channel-compatibility.md`,
  `docs/consumer-preflight-and-docli.md` and
  `docs/sprints/fix-homebrew-formula-bundled-paths-tojson.md`);
  `CHANGELOG.md`; and the generated `Cargo.lock`. Besides the names of the
  first consumer and its components, it flags the consumer's messaging
  principals, its role names and its file names, and hard-coded scope
  prefixes (a string literal or a format string that builds a scope name from
  the consumer's phase prefix). If a legitimate neutral term hits, the rule is
  narrowed and a test added; no exemption is added. The scope and the
  exemptions change only through a later ADR.
- Strings built at runtime (for example joined from parts or read from
  input) cannot be caught by the lint; review guards against them.

Consequences:
- Consumers own messaging, scheduling policy, migration of their own files and
  their own compat tests against the published wheel. The first consumer ships
  its script and depends on the beads-cache wheel.
- To be superseded by the removal sprint b-6 (not edited here). *Amended by
  ADR-BDC-0012:* superseded in `docs/requirements.md` by the phase-B planning
  PR, with the removals in sprints b-7 and b-8:
  REQ-BDC-0701 (the consumer's message-routing settings in the `[tick]`
  table), REQ-BDC-0702 (phases discovered from the consumer's phase
  configuration files, each with a root and a recipient), REQ-BDC-0703 (only
  its per-phase sync wording; the budget itself is neutral), REQ-BDC-0704
  (failures and outages sent to a phase's recipient or the fallback
  recipient), REQ-BDC-0705 (escalations sent to the phase's recipient),
  REQ-BDC-0706 (the `refused` / `pass-through` tick replies and message
  retry) and REQ-BDC-0707 (the stop rule keyed on active phases), together
  with their refinements in `docs/bd-cache-sync/requirements.md`; and
  ADR-SYNC-0007 in `docs/bd-cache-sync/architecture.md` (the phase-file
  schema and the consumer's send contract, with its fix-a-7-3 and fix-a-7-5
  amendments).
- ADR-BDC-0006 states `sc-cron`'s provenance without naming the first
  consumer.
- Until the removal sprints land, the lint fails on the current tree; its
  output is the removal worklist.

## ADR-BDC-0011: The sync on sc-cron rows, Python targets and the store extensions

**Status:** Approved (Rand, 2026-10-26). Amends ADR-BDC-0006 (sc-cron's general hook target
and its dependency list).

Context: ADR-BDC-0010 removes every consumer concern from the sync. Rand ruled (2026-10-25) that
beads-cache syncs bd itself on a tick scheduled in the sqlite cron table, that a consumer can
replace the built-in sync with its own Python method, and that a post-sync hook gets the
results and errors; that sc-cron is a general tool with an `enabled` flag and a stop hook
returning continue, disable or delete; and (2026-10-26) that the switch is two cron rows, with a
CLI command as an app-level convenience, while sc-cron stays a general-purpose cron assembly
whose rows run a Rust or a Python target (PyO3 bindings for sc-cron itself come with
sc-runtime, not in this phase).

Decision:
- The bd sync is two sc-cron rows in the cache file: at most one built-in row (a Rust job) and
  at most one Python row (the consumer's script), at most one of them enabled, switched with
  `bdc cache sync use-python` / `use-builtin`. The rows, the switch, the lease, the post-sync
  hook, the trust check and the shutdown of `cron run` are ADR-SYNC-0008
  (`docs/bd-cache-sync/architecture.md`).
- The store extensions are ADR-STORE-0006 (`docs/bd-cache-store/architecture.md`): v3 plus
  additive columns and tables, `user_version` 3 (ADR-STORE-0001). The schedule rows live in
  the cache file through sc-cron's `ScheduleStore` trait (ADR-BDC-0014).
- Scopes are named bd filters (ADR-BDC-0013).
- The new verbs are modelled in `bd-cache-api` and executed by `bd-cache-cli` and
  `bd-cache-py` over `bd-cache-sync` (ADR-BDC-0015, ADR-PY-0006).
- sc-cron's general changes are ADR-CRON-0009: a hook target per row, several rows' enabled
  flags in one transaction, its child-process supervisor exported with an environment policy,
  and the `Stop` alias removed. Amends ADR-BDC-0006: sc-cron runs a Rust `Job` or a Python hook
  (`<interpreter> -I -m <module>`, JSON on stdin, process-group or job-object kill, capped
  output), chosen per schedule row; its dependency list adds tokio's `process` and `io-util`,
  process-wrap and tempfile (as a normal dependency). sc-cron still depends on no bd-cache or
  consumer crate.

Consequences: REQ-BDC-0701, 0702 and 0704..0707 are superseded by REQ-BDC-0720..0732, and
REQ-BDC-0703 and 0804 are amended. Sprints b-7 and b-8 implement this; b-9 writes the contract
text from this register and lands the lint gate.

## ADR-BDC-0012: The boundary lint after the planning PR

**Status:** Approved (Rand, 2026-10-26). Amends ADR-BDC-0010 (its exemptions, its prohibited
list, the shape of the bd sync and the sprints that remove and gate).

Context: ADR-BDC-0010 exempts `docs/plans/**` and its own section from the lint, describes the bd
sync as one row whose target switches, and lands the CI gate and the removals in sprint b-6.
Rand then ruled (2026-10-26) that no consumer wording appears anywhere in the repository except
in the lint's prohibited list, that the switch is two cron rows (ADR-BDC-0011), and that the
first consumer's maintainer handle is a prohibited word; b-6 was spent on sc-cron (Amendment A2).

Decision:
- **Exemptions.** The lint's only exemptions are the lint script, its tests and
  `docs/team-protocol.md`. The `docs/plans/` directory exemption (`EXEMPT_DIRS`) and the
  `## ADR-BDC-0010` section exemption (`EXEMPT_SECTIONS`) are removed, with their tests and
  docstring text. The plans, the anchor record and ADR-BDC-0010 itself name the words only as
  "a prohibited word"; ADR-BDC-0010's own text is restated that way with no change of meaning.
- **Prohibited list.** The list gains the first consumer's maintainer handle, as a word rule
  with a hit test and a non-hit test for ordinary words that contain it. The prohibited list in
  the lint is the one place every prohibited word is written.
- **Compatibility constant.** The 0.1 bare-name rule of ADR-BDC-0013 writes the scope prefix
  once, as the constant `LEGACY_SCOPE_PREFIX` in `bd-cache-model`'s compatibility module. The
  `scope-prefix` rule is narrowed so that this one declaration (`pub const
  LEGACY_SCOPE_PREFIX: &str = ` followed by the literal) does not match, with a hit test for any
  other literal or format string and a non-hit test for that declaration. Every use refers to
  the constant.
- **The bd sync** is two rows, at most one enabled (ADR-BDC-0011, ADR-SYNC-0008), not one row
  with a switched target.
- **Sprints.** The register supersessions are made in the phase-B planning PR; the removals are
  sprints b-7 (the store's comments) and b-8 (the sync's messaging, phase files, stop rule and
  reply protocol, the `[tick]` keys, the legacy-directory refusal); the exemption change, the
  prohibited-list change and the CI step "Strict consumer boundary (ADR-BDC-0010)" in the `ci`
  lint job are sprint b-9, which merges last. Until b-9, each sprint runs the lint on its own
  fence with `--paths`.

Consequences: the whole tree, `docs/plans/` included, must pass the lint before b-9 merges. A
neutral term that hits is handled by narrowing a rule with a test, never by an exemption.

## ADR-BDC-0013: Scopes are named bd filters

**Status:** Approved (Rand, 2026-10-26). Amends ADR-BDC-0001 (the scope kinds), ADR-BDC-0002
(how the ready and blocked base is fetched), ADR-BDC-0007 (`sc-beads`' `ScopeFilter`),
ADR-BDCLI-0001 (the scope argv), ADR-MODEL-0003 (its fix-a-1-1 amendment's `ScopeId::Phase(PhaseName)`
and its a-7 amendment's `PhaseName::RULE`), ADR-CORE-0204 (`ScopeId::from_labels`), ADR-STORE-0003's
a-7 amendment (S34: a `scopes` entry that is not a phase name) and REQ-STORE-0007 (the same
rule in the loader); referenced from ADR-STORE-0006. Sprint b-7 makes each of these changes.

Context: Rand ruled (2026-10-25, Amendment A5) that a scope is defined by a filter, not by the
first consumer's phase naming, and that `--all` is a scope. Today `ScopeId` is
`enum { Phase(PhaseName) }`, `sc-beads`' `ScopeFilter` is only `Label(String)`, `sc-bd-cli`
builds only `-l <label>` argv for `bd list`, `bd blocked` and `bd ready`, the `scope` table is
`(name, kind)`, and the fakes answer only a label.

Decision:
- **Types, validated once in `bd-cache-model`.** `ScopeName` keeps `PhaseName`'s rule: a
  non-empty string of ASCII letters, digits, `.`, `_` and `-`, with no length cap. That rule
  admitted every name a v3 file holds, so every stored scope stays valid and none is refused or
  renamed; no stricter rule is added, because a scope's name is never passed to bd (its filter's
  labels are). `ScopeName::RULE` replaces `PhaseName::RULE` as the one text of the rule.
  `ScopeFilter`: `All`, or `Criteria { labels_all, labels_any, status, issue_type, assignee }`,
  where `labels_*` are sets of labels, `status` a set of bd statuses, `issue_type` and
  `assignee` optional strings. A `Criteria` with every field empty and any request that sets
  `all` together with a criterion are refused at construction (`InvalidValue`; `BAD_ARGS` in
  `bd-cache-api`, `CONFIG_INVALID` in the configuration). `ScopeId` becomes the scope's name
  (`ScopeId(ScopeName)`, with `name()`); `ScopeId::Phase`, `PhaseName` as a scope kind and
  `ScopeId::label()` go. A `Scope` is `{ id: ScopeId, filter: Option<ScopeFilter> }`, where
  `None` is a phase-a scope (below). `bd-cache-api`, `bd-cache-cli` and `bd-cache-py` take these
  types and never re-validate.
- **Two filter types, one conversion.** `bd_cache_model::ScopeFilter` is the validated scope
  filter, `status` included; `sc_beads::ScopeFilter` is the backend's and has no `status`,
  because bd is never sent one. They are distinct types joined by one conversion,
  `bd_cache_model::ScopeFilter::backend(&self) -> sc_beads::ScopeFilter`, which drops `status`
  and turns a `Criteria` whose only criterion is `status` into `All`; `bd-cache-model` no longer
  uses the sc-beads type as its own. The two refusals above are made once, in the model's
  constructor; sc-beads takes its variants as given, and `sc-bd-cli` and `FakeBackend` refuse
  an empty sc-beads `Criteria` (`InvalidValue`, before any bd call), which the conversion never
  produces.
- **`sc-beads`.** `ScopeFilter` (already `#[non_exhaustive]`) gains `All` and
  `Criteria { labels_all, labels_any, issue_type, assignee }`; `Label(l)` stays and means
  `labels_all = [l]`. `CacheBackend`'s scope calls keep their signatures. `FakeBackend` answers
  every variant.
- **`sc-bd-cli` argv, per variant** (`<all seven>` is the existing status list):

  | Call | `All` | `Criteria` |
  |---|---|---|
  | list | `bd list --all --status <all seven> -n 0 [--updated-after c] --json` | `bd list [-l a,b] [--label-any c,d] [-t type] [-a who] --status <all seven> -n 0 [--updated-after c] --json` |
  | blocked | `bd blocked --json` | `bd blocked [-l a,b] [--label-any c,d] --json` |
  | ready | `bd ready -n 0 --json` | `bd ready [-l a,b] [--label-any c,d] [-t type] [-a who] -n 0 --json` |

  `status` is never passed to bd: the sync lists all seven statuses (REQ-BDC-0501) and the store
  applies the status criterion to membership, so a bead that leaves the status set leaves the
  scope on the next sync that lists it. `bd blocked` takes no type or assignee flags, so the
  blocked and ready answers are intersected with the scope's membership. A bead that stops
  matching a criterion bd applies (a label, type or assignee) keeps its membership until a sync
  lists it again, as a removed label does today. Verified: the flags above are those `bd list
  --help`, `bd blocked --help` and `bd ready --help` print for bd 1.3.0 on the development host
  (`bd list`: `-l` all, `--label-any` any, `-s`, `-t`, `-a`, `--all`, `-n`; `bd blocked`: `-l`,
  `--label-any`, `--exclude-label`, `--parent`; `bd ready`: `-l`, `--label-any`, `-t`, `-a`,
  `-n`); the repository's bd 1.3.1 fixtures record only `-l`. Sprint b-7 records bd 1.3.1
  fixtures for each variant, `FakeBd` and the `fake-bd` binary answer them, and the bd-compat
  workflow checks them against bd 1.3.1.
- **Base (amends ADR-BDC-0002 item 1).** A scope's ready and blocked base is the `bd blocked`
  and `bd ready` answer of the table above, intersected with the scope's members, stored with
  its fetch time; the local adjustment and the bounded fresh answer are unchanged.
- **Store.** Stored scope names stay as written: phase a stores the bare name (`scope(name, kind
  = 'phase')`, `sync_state.phase`, `scope_member.scope`, `journal.phase`), and nothing is
  renamed. A nullable `scope.filter` column holds the filter as JSON. NULL, on every
  `kind = 'phase'` row, keeps the phase-a meaning: a label scope whose label is
  `LEGACY_SCOPE_PREFIX` followed by the name (a stored `x` syncs the label `<prefix>x`). The JSON
  is written only for a scope created in phase B with a filter. The v3 `beads.phase` column
  keeps its phase-a meaning, the first legacy-prefixed label of the row, and is NULL for a row
  with no such label: a filter scope never writes its name there, so a row only a filter scope
  holds has `phase` NULL. The column is in ADR-STORE-0006's list and the format golden.
- **Source of truth.** `.beads/bd-cache.toml` defines scopes with `[[scope]]` tables (`name`
  plus the filter keys `labels_all`, `labels_any`, `status`, `type`, `assignee`, or `all =
  true`) and with the existing `scopes` string list (each string a phase-a scope, by the 0.1
  rule below). A scope the
  file defines is created or updated in the store's registry when the cache opens, and
  `scopes set` or `scopes remove` of it is `CONFIG_INVALID` (`details.source:
  "config_scope"`): the file wins. Any other scope lives in the registry only and is managed
  with `bdc cache scopes add|set|remove`. Syncing a name that is neither registers it on demand
  as a label scope with that one label (REQ-BDC-0909). Only a sync registers on demand (`cache
  refresh scopes <name>` and the 0.1 `sync(phase)`); every other verb that names a scope refuses
  an unknown name with `SCOPE_UNKNOWN` and registers nothing.
- **0.1 compatibility (Rand, 2026-10-26).** Where a 0.1 interface took a phase (the `scopes`
  string list and the 0.1 Python verbs), `x` and `<prefix>x` both mean the phase-a scope stored
  as `x`, whose label is `<prefix>x`: the 0.1 meaning, with the prefix the one constant
  `LEGACY_SCOPE_PREFIX` (ADR-BDC-0012). Every other interface (the `[[scope]]` tables, the CLI,
  the 0.2 Python namespaces and the new methods) takes a scope name as written: `x` names the
  scope stored as `x`, and `<prefix>x` names a scope stored under that longer name. The prefix
  rule is applied on the 0.1 paths only.
- **Callers.** The `ScopeId` change ripples into `bd-cache`, `bd-cache-store`, `bd-cache-api`,
  `bd-cache-cli` (with the `fake-bd` binary), `bd-cache-py` and `bd-cache-sync`; sprint b-7
  makes those edits mechanically, so the workspace builds green, and the semantic removals stay
  with b-8.
- **Register text this changes.** ADR-MODEL-0003's fix-a-1-1 amendment (scopes are
  `ScopeId::Phase(PhaseName)`) now reads `ScopeId(ScopeName)`, and `PhaseName::RULE` in its a-7
  amendment is `ScopeName::RULE`. ADR-CORE-0204's `ScopeId::from_labels` finds the first
  legacy-prefixed label through `LEGACY_SCOPE_PREFIX` and returns the phase-a scope it names; the
  rest of that ADR holds. ADR-STORE-0003's S34 amendment and REQ-STORE-0007: `store_config_bad_scope`
  is a `scopes` entry or a `[[scope]]` name that is not a valid `ScopeName`, or a `[[scope]]`
  filter refused above, and its fix states `ScopeName::RULE`; a valid name in the `scopes` list
  is a phase-a scope, never a `ConfigLoadError`.

Consequences: REQ-BDC-0501, 0602, 0603, 0901, 0909 and 0721 are amended; the contract's scope
text (§5, §6, §12) is rewritten by b-9 from this ADR. Tests `sync_label_filter_scope` and
`sync_all_scope` pin the two new variants end to end against the fakes.

## ADR-BDC-0014: The schedule rows live in the cache file through sc-cron's store trait

**Status:** Approved (Rand, 2026-10-26). Amends ADR-BDC-0006 (the schedule store), ADR-STORE-0003
(one connection per process), ADR-STORE-0006 (the schedule table), ADR-CRON-0006 (the store a
caller supplies), NFR-BDC-0007 and REQ-CRON-0009.

Context: the sync rows must live in the cache file (S83, ADR-BDC-0011), but sc-cron's
`SqliteScheduleStore` owns its own file and connection, refuses a file that holds other tables
("sc-cron needs its own database file") and owns `PRAGMA user_version` (its `SCHEMA_VERSION`
is 2), while the cache file is frozen at `user_version` 3 and a process holds one connection to
it (NFR-BDC-0007, ADR-STORE-0003). sc-cron's `Scheduler::new` already takes any
`Arc<S: ScheduleStore>`, so a caller can supply the store.

Decision (Rand's ruling, 2026-10-26):
- `sc_cron::ScheduleStore` is the seam. `SqliteScheduleStore` (its own file, `user_version` 2)
  stays sc-cron's standalone default, unchanged.
- `bd-cache-store` implements `sc_cron::ScheduleStore` as `CacheScheduleStore`, over the cache's
  single SQLite connection: `SqliteStore`'s connection moves behind a mutex shared by the store
  handle and `SqliteStore::schedule_store()`, so the process opens no second connection. The
  shared, mutex-guarded object owns both the connection and the per-process claim (`Registration`,
  `STORE_IN_USE`), the claim after the connection so the connection closes first, as today: the
  claim is released only when the last holder of the connection drops. A `CacheScheduleStore`
  that sc-cron's `Scheduler` holds after the `SqliteStore` handle is dropped therefore keeps the
  claim, and a second `SqliteStore` open in the process is still `STORE_IN_USE`. The schedule
  table, `sc_cron_schedule`, with sc-cron's version-2 columns and checks, is an additive table in the cache file, created
  through the extension mechanism (ADR-STORE-0006).
- The cache file stays `PRAGMA user_version` 3. The version of sc-cron's columns in the cache
  file is the `meta` row `sc_cron_schedule_version` (2), written and checked by
  `CacheScheduleStore`, never `user_version`; a cache file whose row names another version is
  refused with `StoreErrorKind::SchemaMismatch` before any schedule is served.
- A general store change sc-cron needs (several rows' enabled flags in one transaction, and a
  row written with its flag together with other rows' flags, REQ-CRON-0017) is a required
  `ScheduleStore` method that both implementations provide in one `BEGIN IMMEDIATE`.
- `bd-cache-store` gains the workspace edge `bd-cache-store` → `sc-cron`; `sc-cron` still depends
  on no bd-cache crate.

Consequences: `schedule set` and `cron run` open the schedule store at startup and fail with a
typed error (`STORE_FAILED` with `details.source` naming the check) before serving any work.
Sprint b-7 implements the trait change and `CacheScheduleStore`, with an acceptance test that a
v3 cache file with scopes gains the schedule table and stays `user_version` 3, that the process
holds one connection to the cache file, and that the claim outlives a dropped `SqliteStore`
while the schedule store is alive.

## ADR-BDC-0015: The new verbs are modelled in bd-cache-api and run by the front ends

**Status:** Approved (Rand, 2026-10-26). Amends ADR-BDC-0009 (where the new verbs execute),
ADR-CLI-0003 (the command table), ADR-CLI-0004 (the legacy-directory refusal is removed) and
ADR-PY-0005 (through ADR-PY-0006).

Context: phase B adds `bdc cache scopes add|set|remove|list|ingest|announce`, `cache schedule
set|show|enable|disable|clear`, `cache cron run [--once]`, `cache sync use-python|use-builtin`
and their Python methods, with the codes `INGEST_INVALID`, `SCHEDULE_INVALID`,
`TICK_RUN_FAILED` and `TICK_IN_FLIGHT`. ADR-BDC-0009, ADR-CLI-0003, REQ-BDC-0807 and
REQ-CLI-0006 require one `bd-cache-api` decoder and one set of models for the CLI, Python and
MCP; `bd-cache-api` must not depend on `bd-cache-sync` or `sc-cron`. The new `scopes list`
would collide with `cache scopes`, and `scopes remove` with `cache evict`.

Decision:
- **Models in `bd-cache-api`.** Every new verb's request, its reply data and its errors are
  `bd-cache-api` types (pure data: no `bd-cache-sync`, no `sc-cron`), decoded by the one decoder
  (`src/decode.rs`) and listed in `Command::ALL`; `cache tick` becomes a `Command` too, so the
  `request/mod.rs` doctest that `Command::parse("cache tick")` is `None` is reversed, and
  `src/ops/mod.rs` declares the new ops module and its `execute` arms.
  The store-only verbs (`scopes add|set|remove|list|ingest|announce`) also execute in
  `bd-cache-api` over `bd-cache`, like the other management verbs.
- **Execution in the front ends.** `schedule *`, `cron run`, `tick` and `sync use-python|use-builtin`
  need `bd-cache-sync`, so `bd-cache-api` decodes them into a request the front end executes:
  `bd-cache-cli` (`src/request.rs` and its `commands/`) and `bd-cache-py` (ADR-PY-0006) call
  `bd-cache-sync`. The MCP server, later, does the same.
- **Codes.** The four new codes are rows of `ERROR_TABLE`, with the kinds and exits of the code
  table in REQ-BDC-0729. The `TICK_<REFUSAL>` family (`ApiError::tick_refusal` and its kind
  table) is removed with the refusals it reported (b-8).
- **Verb names.** `cache scopes` stays the read verb and is `scopes list`'s alias: both return
  the same reply, now with each scope's filter. `cache evict <scope>` stays and empties a
  scope's beads as today; `scopes remove <scope>` is `evict` followed by removing the scope from
  the registry, refused (`PRUNE_BLOCKED`) under the same rule as `evict`. Neither old verb
  changes its arguments or reply members; new members are additive.
- **Configuration (amends ADR-CLI-0004).** The refusal of the legacy cache file in the first
  consumer's own directory is removed: the cache file is found by the store's rules or named by
  `cache_path`, and no consumer path is known to beads-cache.

Consequences: sprint b-8 owns the `bd-cache-api` request, data and error files for these verbs
and `bd-cache-cli/src/request.rs`; b-9 writes the contract text from this register. REQ-BDC-0810,
REQ-CLI-0006 and REQ-CLI-0008 are amended.

## Per-Crate Architecture

Each crate's architecture and crate-local ADRs live in
`docs/<crate>/architecture.md` (see `.claude/project/quality-policy.md`):

- [`sc-beads`](sc-beads/architecture.md) (`ADR-BEADS-*`)
- [`bd-cache-model`](bd-cache-model/architecture.md) (`ADR-MODEL-*`)
- [`bd-cache`](bd-cache/architecture.md) (`ADR-CORE-*`)
- [`sc-bd-cli`](sc-bd-cli/architecture.md) (`ADR-BDCLI-*`)
- [`sc-cron`](sc-cron/architecture.md) (`ADR-CRON-*`)
- [`bd-cache-store`](bd-cache-store/architecture.md) (`ADR-STORE-*`)
- [`bd-cache-sync`](bd-cache-sync/architecture.md) (`ADR-SYNC-*`)
- [`bd-cache-py`](bd-cache-py/architecture.md) (`ADR-PY-*`)
