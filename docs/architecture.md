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
 backend port:            sc-beads traits: sc-bd-cli (phase a)
                                     sc-bd-http (later)
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
- Python bindings (maturin / PyO3), planned after sc-runtime integration,
  are a third front end: a `bd-cache-py` crate wrapping the concrete
  composition (`SqliteStore`, its drain lock, `BdCli`) and exposing the same
  operations as the CLI, so Python callers such as atm-core's hooks run in
  process instead of spawning `bdc`. Phase A keeps this cheap: the replies
  and `CacheError` already serialize to the CLI's JSON, and the core takes
  plain config values rather than sc-runtime types.
- All writes go through `bd` (see the research docs: writes through the
  DoltHub SQL API bypass bd's guards and events journal).

### Repository configuration

bdc is set up from `.beads/bd-cache.toml`, next to bd's own files, since the
cache is an extension of bd. It names the sync scopes, the timeouts
in `CacheConfig`, and the schema packages to load by path, for example
`.claude/skills/atm-beads/schemas`, each with an optional enforcement
override (ADR-BDC-0008). Loading it goes through sc-config after sc-runtime
integration; phase a's core takes the resulting `CacheConfig` as plain
values. Format details (keys, and how it relates to BD-CACHE's
`.atm-bd/phase-*.toml` phase files) are settled then.

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
  its own cursor, then replaces the scope's blocked set from one
  `bd blocked -l <label>` call (ADR-BDC-0002).

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
| `bd-cache-model` | Pure cache types on top of `sc-beads` rows: `Op` (parse, args JSON, overlay, achieved), journal entries, `replay`, the `CacheStore` and `DrainLock` ports with `FakeStore`, the `EscalationSender` port, `CacheConfig`, `DrainBudget`, reply and report types, `CacheError` with every `BD_CACHE.*` code and the backend-error mapping (ADR-BDC-0003) | `sc-beads`, serde, serde_json | a |
| `sc-beads` (shared, published) | Beads backend contract only: btit's `BeadsBackend`, `CliBackend` and `DoltOperations` traits plus the cache extensions (batched `--id` and `--updated-after` lists, guarded update and close with reason, per-call deadline, `/v0`-aligned patch, backend fingerprint), strict and tolerant rows, `BeadsError` and `Refusal`, `Clock` and `Deadline`, a `FakeBackend` test double (ADR-BDC-0007) | serde, serde_json, sha2, tracing | a |
| `sc-bd-cli` (shared, published) | The bd CLI implementation of the `sc-beads` traits, ported from btit-cli and btit-bd: invoker with a recording test double, probes and version gates, dolt-mode detection, per-project locks; exit 13 as a typed refusal | `sc-beads` | a |
| `sc-bd-http` (shared, published) | The `bd serve` HTTP implementation of the same traits | `sc-beads` | when bd releases the `id` and `updated_after` filters |
| `bd-cache-store` | Implements `CacheStore` and `DrainLock` over SQLite: BD-CACHE v3 tables unchanged plus additive tables and nullable columns, file location from `.beads/metadata.json`, meta and rebuild rule, journal, scopes and membership, edges index, outage record, cross-process drain lock (`flock`) | `bd-cache-model`, `sc-beads`, rusqlite (bundled), fs4 | a |
| `bd-cache` (core library) | `Cache`: get, list, write, drain, sync, resolve, prune, status, the targeted blocked refresh (ADR-BDC-0002) and outage handling (ADR-BDC-0005); shared `view`, `backend_check` and `miss` helpers; takes a `CacheConfig` and a `Clock` | `bd-cache-model`, `sc-beads` | a |
| `bdc` binary (`bd-cache`) | JSON-first CLI, mapped to `/v0` vocabulary where it makes sense; BD-CACHE CLI contract | core, sc-command / sc-config | after sc-runtime |
| `sc-cron` (prototype, moves to sc-runtime) | SQLite-backed cron scheduler extracted from atm-core's `atm-cron`: expression parse, next-run computation, jitter, missed-tick replay, stop check before every run, generation fencing of overlapping runs, schedule store with a rusqlite implementation. Generic job payload and error type; no ATM or bd-cache types (ADR-BDC-0006) | cron, chrono, tokio, rusqlite | a |
| `bd-cache-sync` | The sync task on `sc-cron`: tick (drain then sync), stop rule, announcements through `EscalationSender` (the `atm send` sender is one implementation) | `bd-cache`, `bd-cache-store`, `sc-bd-cli`, `sc-cron`, tokio | a |
| MCP server | HTTPS MCP front end over the core | core, sc-runtime, sc-transport | later |

Phase a acceptance runs through the core library API with the fake
backend: the ported cases from `test_bd_cache.py` (41), the 9 cases from
`test_bd_cache_cron.py` against the sync task with an injected sender, and
the bd-1.3.1 contract fixtures. `sc-cron` carries its own tests, ported from
atm-cron where they apply.

## Boundaries and Dependencies

Normative; CI checks it with `cargo metadata` (a-1). Allowed edges between
workspace crates (third-party edges are in the Crate Layout table):

| Crate | May depend on |
|---|---|
| `sc-beads` | none |
| `sc-bd-cli` | `sc-beads` |
| `sc-cron` | none |
| `bd-cache-model` | `sc-beads` |
| `bd-cache-store` | `bd-cache-model`, `sc-beads` |
| `bd-cache` | `bd-cache-model`, `sc-beads` (never the store or `sc-bd-cli`) |
| `bd-cache-sync` | `bd-cache`, `bd-cache-store`, `bd-cache-model`, `sc-bd-cli`, `sc-cron`, `sc-beads` |

`bd-cache-model` does no I/O. `sc-beads`, `sc-bd-cli` and `sc-cron` depend on
no bd-cache crate, so they can move or be published on their own.

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

Consequences: extends the BD-CACHE v3 file additively at `user_version` 3;
the Python implementation reads the extended file, and the Rust store
tolerates rows Python leaves in the extension tables. No version bump. Needs agreement on
https://github.com/randlee/atm-bd-orchestration/issues/557.

## ADR-BDC-0002: Blocked state from bd, one query per scope

**Status:** Draft

Context: a filtered cache must answer `ready` and blocked state correctly,
including when a blocker is outside every scope, and when a blocker is closed
or reopened. The probe in
[`research/bd-blocked-state-probe.md`](research/bd-blocked-state-probe.md)
(P-6) found:

- `bd list --json` and `bd show --json` never emit `is_blocked`, so cached
  rows carry no blocked state at all (`bd schema` declares it optional).
- bd settles blocked state without bumping the depender's `updated_at`
  (`BlockedStateInvariant`, beads `issueops/blockedstate.go`; fenix confirmed
  18 of 1279 live `blocks` edges), so `--updated-after` sync cannot see it
  change either.
- `blocks` edges block; a child inherits its parent's blocked state through
  `parent-child`; `related` does not block. Other types were not probed.
- `bd blocked -l <label> --json` returns bd's own blocked set for a label,
  each row with `blocked_by`, including blockers outside the label and
  inherited parent-child blocking.

The live Python cache (BD-CACHE v3) is not affected: it never stores or
serves blocked state, and readiness there comes from live `bd ready`.

Decision: the cache does not compute blocked state and does not read it from
rows. Each sync of a scope ends with one `bd blocked -l <scope label> --json`
call and replaces that scope's stored blocked set (bead id plus
`blocked_by`) with the result. `ready` for a scope is its cached rows with
status `open`, minus the stored blocked set, with the journal overlay
applied. A bead whose blocked state is unknown (its scope has never
completed this call) is reported as such, never as ready.

Between syncs the blocked set can lag a journaled close or reopen by up to
one tick; the cron tick drains then syncs, so a drained close is reflected
in the same tick. This is the same lag BD-CACHE documents for agents'
direct `bd` reads.

Consequences: one extra `bd` call per scope per sync, returning only the
blocked beads; no depender re-reads, no changed-blocker or outside-blocker
bookkeeping, and no copy of bd's blocking rules to drift. Out-of-scope
blockers and parent-child inheritance are handled by bd. If the call fails,
the previous set is kept and marked stale with the scope's sync error.

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

Decision: all backend access goes through the `sc-beads` traits
(ADR-BDC-0007). The cache uses `CacheBackend: BeadsBackend`, whose methods
all take a `Deadline`: `fingerprint`, `list_by_ids`, `list_scope(filter,
since)`, `list_blocked(filter)`, `close_guarded`, `update_guarded`,
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

**Status:** Draft

Context: two deployments are expected: a dolt server on a remote machine over
an unreliable network, and a local server on a host so loaded that `bd`
calls take seconds (the BD-CACHE motivation). BD-CACHE v3 escalates a write
after `MAX_ATTEMPTS` (5) transient failures, so a backend outage of a few
cron ticks escalates every pending write to the phase lead.

Decision:
- Reads never wait long on the backend. Every read reply carries the scope's
  `last_ok_at` and `stale` per row, so callers and MCP tools can see how
  fresh the data is. A cache miss fetches with a short deadline
  (`CacheConfig::miss_fill_timeout`, default 2 s) and returns
  `BD_CACHE.BD_UNAVAILABLE` when it expires.
- Backend unreachable (timeout, connection refused, cannot start) is a
  backend state, recorded in the store as the outage (since when, last
  error) and cleared by the next successful call. While it holds, the drain
  stops at the first such failure without counting an attempt, and a sync
  that meets it records the outage instead of an announced `SYNC_FAILED`.
  Only failures from a reachable backend (`BD_WRITE_FAILED`, guard refusal)
  count towards `MAX_ATTEMPTS` or escalate.
- No extra backoff: the cron tick (30 s) is the retry interval. `status`
  reports the outage. An outage is announced through each active scope's
  sync alert under the stable cause key `backend_unavailable`, so each lead
  hears it once; its end resets the alert without a message.
- The drain has a start gate and a hard deadline (REQ-BDC-0402): each `bd`
  call is capped at the time left before the hard deadline; a call cut short
  by it is neither an outage nor an attempt.

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

## ADR-BDC-0007: Shared beads backend crates, seeded from btit

**Status:** Draft

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

## ADR-BDC-0008: User metadata extensions as consumer-owned schema packages

**Status:** Draft (later phase; not in phase a)

Context: teams extend beads with their own fields in bd's free-form
`metadata`, and may require some of them. atm-beads already does this: its
Pydantic models (`bead_schema.py`) are exported as JSON Schemas in
randlee/atm-core `.claude/skills/atm-beads/schemas/` (dev, fix, sanity, qa,
finding, gate, root, sprint and stack beads), with role-based fields
stamped at pour and required at close (`close_fields.py`), and structured
values such as the QA bead's list of required review agents. bd accepts any
metadata and enforces none of it. The schemas belong to the skill that
defines them and change when that skill is upgraded, so they cannot be
compiled into, or released with, the shared crates.

Decision (proposed):
- **Schema packages ship with their consumer.** A package is a directory the
  skill owns (for atm-beads, its `schemas/`), plus a small manifest: package
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
  metadata (for example `schema: "atm-beads/qa-bead@3"`), so a bead always
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
- Structured values. `bd update --set-metadata` stores only flat scalars
  (REQ-BDC-0303), and BD-CACHE v1 keeps structured metadata out of the
  journal, so arrays like the QA reviewer list can be read and validated but
  not written through the cache today. `bd update --metadata <json>` and the
  `/v0` `metadata: {merge, set, unset}` patch can carry them; whether
  `--metadata` merges or replaces must be probed first. Where these values
  live and how they are written is being confirmed with fenix.
- The manifest format and the version-stamp key.
- Long term, schema packages may live in dolt as an extension table, so a
  database carries its own schemas and versions. Out of scope for this
  work; the package format should not rule it out.

Consequences: adds schema-package loading, version selection and a validator
to `sc-beads`, a new error code, and a pour-time version stamp in each
consuming skill.

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
