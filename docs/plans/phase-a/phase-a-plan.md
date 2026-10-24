# Phase A — cache core, shared beads backend and cron prototype

Status: draft, not yet hardened. P-1 is done; P-2 and P-3 are open.

Phase id: `a`. Plan branch: `plan/phase-a` (PR to `develop`). Phase branch:
`integrate/phase-a`, cut from `develop`. Sprint branches
`sprint/a-<n>-<slug>`, cut from `integrate/phase-a` or the top of their
track's stack.

## Intent

Build every bd-cache crate that does not depend on an sc-runtime crate
(sc-config, sc-transport, sc-command, sc-runtime are being built in
parallel), so the most deliverables are done before integration:

1. the cache types (`bd-cache-model`);
2. the shared beads backend crates, seeded from beads-task-issue-tracker's
   btit crates and published to crates.io for btit to use (ADR-BDC-0007):
   `sc-beads` (traits, types, errors, test double) and `sc-bd-cli` (the
   bd CLI implementation); `sc-bd-http` follows when bd ships the filters it
   needs;
3. the SQLite store (`bd-cache-store`);
4. the core library (`bd-cache`): get, write, drain, sync, resolve, prune,
   status, the improved ready/blocked refresh and outage handling;
5. a SQLite-backed cron prototype `sc-cron`, extracted from atm-core's
   atm-cron, meant to move to sc-runtime (ADR-BDC-0006);
6. the sync task on `sc-cron` (`bd-cache-sync`).

Baseline contract: atm-core `BD-CACHE.md` (PR randlee/atm-core#2219). The
Python acceptance suite (`test_bd_cache.py`, 41 tests; `test_bd_cache_cron.py`,
9 tests) is ported to Rust and run against the fake backend.

## Non-goals

- The `bdc` / `bd-cache` binary. It needs sc-command and sc-config; it comes
  after sc-runtime integration. Phase A exercises the CLI contract's JSON
  replies and error codes through the library.
- The MCP server, the `bd serve` HTTP adapter, DoltHub-specific adapters.
- Bead creation and comments in the cache journal (out of BD-CACHE v1).
- Moving `sc-cron` to sc-runtime, and publishing `sc-beads` / `sc-bd-cli` to
  crates.io. Phase A makes them publish-ready (`cargo publish --dry-run`);
  the release and btit's switch follow the phase.

## Requirements deferred from phase A

| Id | Disposition |
|---|---|
| REQ-BDC-0801, REQ-BDC-0805, NFR-BDC-0002 | The `bdc` / `bd-cache` binary waits for sc-command and sc-config. Phase A meets their JSON and error-code content through the library (REQ-BDC-0802, 0803). |

## Prerequisites before dispatch

| Id | Prerequisite | Owner |
|---|---|---|
| P-1 | Done: `docs/requirements.md` holds `REQ-BDC-*` / `NFR-BDC-*` drawn from BD-CACHE.md (with section provenance) and from planning (source "user"); every sprint lists its ids. | team-lead |
| P-2 | Agreement on #557 (randlee/atm-bd-orchestration#557) for the additive v3 extensions in ADR-BDC-0001, 0002, 0003 and 0005, or a recorded decision to diverge. Blocks a-4's schema freeze, not a-1. | owner |
| P-3 | beads-task-issue-tracker agreement on the shared `sc-beads` API: synchronous calls, `BeadsError` as the shared error type, tolerant and strict row modes (ADR-BDC-0007). Blocks a-1's trait freeze. | owner |
| P-4 | sc-runtime team informed of `sc-cron` (name, eventual home). Does not block dispatch. | owner |
| P-5 | The existing beads-release trigger (today it drives the hermes team's fork maintenance) also sends `repository_dispatch` type `beads-release` with the bd version to randlee/beads-cache, with a token allowed to dispatch there. Needed before a-2's workflow can be proven by a real release; a-2 proves it with `workflow_dispatch`. | owner |

## Boundary map

No crate exists yet. Each crate is one boundary; no boundary manifests are
declared (quality policy: boundaries are reviewed against
`docs/architecture.md`).

| Boundary | Contract | Depends on | Consumed by |
|---|---|---|---|
| `bd-cache-model` | `Op`, `Patch`, `Guards`, `Refusal`, `CacheError` and codes, `Edge`, `JournalEntry`, overlay and `achieved` (pure) | serde, `sc-beads` row types | store, core, sync |
| `sc-beads` | `BeadsBackend` trait (btit) plus the cache extensions, strict `Bead` row, tolerant `BdRawIssue`, `BeadsError`, `Fingerprint`, `FakeBackend` test double | serde, serde_json, tracing | model, store, core, sync, `sc-bd-cli`, btit |
| `sc-bd-cli` | `BdCli` implementing the `sc-beads` traits over the `bd` binary | `sc-beads` | sync composition, btit |
| `bd-cache-store` | SQLite file location, schema v3 plus extensions, meta and rebuild rule, journal, scopes and membership, edges index, transactional API used by core | model, `sc-beads` rows, rusqlite (bundled) | core |
| `bd-cache` | `Cache` facade: get, list, write, drain, sync, resolve, prune, status, ready/blocked refresh, outage state | model, store, `sc-beads` trait | sync, later `bdc` and MCP |
| `sc-cron` | scheduler, schedule store port and rusqlite store, `Clock`, jitter, generation fencing | cron, chrono, tokio, rusqlite, tracing | sync |
| `bd-cache-sync` | sync task: tick (drain then sync per active scope), stop rule, escalation sender port | core, `sc-cron`, `sc-bd-cli` | later `bdc` and daemon |

Workspace `Cargo.toml` uses `members = ["crates/*"]`, so adding a crate edits
no shared file. `Cargo.lock` is regenerated by every sprint and is the one
shared file; conflicts are resolved by rebasing onto the parent layer, never
by hand-editing another sprint's dependencies.

## Contract handoff

a-1 freezes these signatures (sketch; a-1's doc carries the normative
version). Later sprints reference them and do not restate them.

```rust
// sc-beads (btit trait kept; cache extensions added)
pub trait BeadsBackend: Send + Sync {
    fn fingerprint(&self, project: &ProjectRef, deadline: Deadline) -> Result<Fingerprint, BeadsError>;
    fn list_by_ids(&self, project: &ProjectRef, ids: &[BeadId], deadline: Deadline) -> Result<Vec<Bead>, BeadsError>;
    fn list_since(&self, project: &ProjectRef, filter: &ScopeFilter, cursor: Option<&SyncCursor>, deadline: Deadline) -> Result<(Vec<Bead>, SyncCursor), BeadsError>;
    fn close(&self, project: &ProjectRef, id: &BeadId, reason: Option<&str>, guards: &Guards, actor: &Actor, deadline: Deadline) -> Result<(), BeadsError>;
    fn update(&self, project: &ProjectRef, id: &BeadId, patch: &Patch, guards: &Guards, actor: &Actor, deadline: Deadline) -> Result<(), BeadsError>;
    fn dep_add(&self, project: &ProjectRef, id: &BeadId, depends_on: &BeadId, kind: DepType, actor: &Actor, deadline: Deadline) -> Result<(), BeadsError>;
    // ... btit's existing list/show/create/search/label/comment/dep_remove/sync methods
}

pub enum BeadsError { Unavailable { .. }, Refused(Refusal), Contract { bead: Option<BeadId>, field: String, .. }, Failed { code: i32, stderr: String }, /* btit variants */ }
pub enum Refusal { Precondition, NotClosable, AlreadyClaimed, Other(String) } // bd exit 13 -> Precondition

// bd-cache-model
pub enum Op { Close { reason: Option<String> }, Update(Patch), DepAdd { depends_on: BeadId, kind: DepType } }
impl Op {
    pub fn overlay(&self, bead: &Bead) -> Bead;
    pub fn achieved(&self, bead: &Bead, edges: &[Edge]) -> bool;
}
pub enum CacheError { /* exactly the BD_CACHE.* codes */ }
impl CacheError { pub fn code(&self) -> &'static str; pub fn fix(&self) -> &'static str; }
```

## Sprints and waves

| Sprint | Closure | Boundary | Wave | Track | must_follow |
|---|---|---|---|---|---|
| [a-1](sprint-a-1-contracts.md) | contract | `bd-cache-model` + `sc-beads` | 1 | cache | — |
| [a-2](sprint-a-2-sc-bd-cli.md) | boundary (implementer) | `sc-bd-cli` | 2 | backend | a-1 (`BeadsBackend` extensions) |
| [a-3](sprint-a-3-sc-cron.md) | boundary | `sc-cron` | 1 | cron | — |
| [a-4](sprint-a-4-store.md) | boundary | `bd-cache-store` | 2 | cache | a-1 (model and row types) |
| [a-5](sprint-a-5-core.md) | boundary (consumer) | `bd-cache` | 3 | cache | a-4 (store transactional API) |
| [a-6](sprint-a-6-sync-integration.md) | integration | `bd-cache-sync` | 4 | cache | a-2, a-3, a-5 |

- Critical path: 4 sprints (a-1 → a-4 → a-5 → a-6). It exceeds three because
  the core consumes the store's transactional API (journal insert with
  precondition check, read-back store plus applied mark in one transaction),
  which cannot be hoisted into a-1 without a speculative store trait.
- Width: 2 (wave 1: a-1, a-3; wave 2: a-2, a-4; a-3 may run into wave 2).
- Sprint count: 6.

## Acceptance ownership

- Ported Python cases are split by the crate that implements the behaviour:
  row validation and contract fixtures in a-1; file location, metadata,
  project id, schema version and rebuild rule in a-4; overlay, write,
  drain, sync, resolve, prune and status in a-5; the 9 cron cases and the
  end-to-end composition in a-6. a-1's doc carries the case-to-sprint table.
- Feature-level behaviour (a hook-style read and journaled write applied by
  a scheduled drain through the real bd CLI adapter) closes only in a-6.

## Dependency edges

- a-2 → a-1: `sc-bd-cli` implements the `BeadsBackend` extensions and strict row parsing
  a-1 defines.
- a-4 → a-1: stores `Bead` rows and journal entries typed in a-1.
- a-5 → a-4: calls the store's transactional API.
- a-6 → a-2, a-3, a-5: composes the real adapter, the scheduler and the core.
- a-3 is parallel_safe with every other sprint until a-6.

## bd release compatibility

beads-cache is more sensitive to bd changes than btit: it validates rows
strictly and keeps cache files across bd upgrades. a-2 adds a CI workflow
that runs on every beads release (P-5) and checks the new bd against the
pinned fingerprint, the contract tests and the live probes, opening an issue
on any break. The shared crates make the fix a single upgrade, released as a
semver-major version that every consumer takes (ADR-BDC-0007).

## QA consumption

Each sprint doc lists `requirements`, `adrs`, deliverables, acceptance criteria with roots, and
`owned_paths`. Reviewers per `.claude/project/quality-policy.md`.
