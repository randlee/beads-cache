# Phase C — Adopt the sc-runtime framework and qualify a consumer release

Status: **approved for implementation, 2026-11-02**. Written 2026-11-02.
Approval is the explicit project-owner execution instruction recorded below; no sprint is complete yet.
Plan branch/worktree: `plan/phase-c`. Integration branch: `integrate/phase-c`, cut from
`develop` only after the plan PR merges. Sprint PRs target the integration branch;
only the plan PR and final phase PR target `develop`.

## Intent and authority

Phase B landed in PR #82 at `5196acd`. Its CLI and Python CI passed on that commit.
Phase C installs and adapts sc-runtime's application templates, adopts its published
scheduler, and qualifies the resulting application through the existing CI and release path.
The user rejected a crates-only integration because it would drift from sc-runtime.
The user's later upstream guidance selects rusqlite **0.39**, matching sc-runtime.

The sprint docs below are authoritative for scope, requirements, owned paths and
acceptance. The project owner approved REQ-BDC-1001..1003, REQ-BDC-1005 and
ADR-BDC-0016 for implementation on 2026-11-02. All other binding requirements
and ADRs remain in force. This approval record makes no product implementation
or dependency change; c-1 applies its narrowly owned approved amendments.
The tested rusqlite downgrade remains uncommitted in the original checkout; c-1
lands that change on its sprint branch rather than committing on `develop`.

Template source baseline: sc-runtime tag `v0.1.1`, commit
`5af24c862fe7ed8dae8710b7bd64a84a29506b51`. Read the tagged files, not an unqualified
checkout. Its template is application-owned scaffold, not an in-place installer:
`scripts/new_project.py` refuses an existing destination. Its `install.py` installs
sc-publish; `just upgrade` upgrades sc-lint, not generated Rust code.

C1 library source baseline: published sc-cron/sc-command exactly registry
`=0.2.0`, default features off, under ADR-BDC-0016. Publication supersedes the
temporary qualified prerelease Git permission. This changes library provenance
only, not the independent template tag, answers, domain bounds or QA/anchor/CI
gates. The earlier conditional-pin authorization remains historical in the ADR.

## Implementation dispatch gate

The project owner explicitly approved implementation on 2026-11-02 in this Codex chat: “pr105 approved” and “complete all phase-c sprints and merge to develop when qa+ci both green”, with a direction to start cursor-orchestration and assign cursor-quality-mgr to QA.

This direct execution instruction satisfies the owner-approval prerequisite for
all four bounded sprints, including ADR-BDC-0016, REQ-BDC-1001..1003 and
REQ-BDC-1005, the narrow REQ-CLI-0001 dependency-source/version and
REQ-API-0008 transitive-contract amendments, and the prototype ownership
disposition already described in the approved plan. It is distinct from a
reviewer PASS, CI success or planning-PR merge alone. c-1 applies its owned
CLI/API/CRON amendments before their dependent code changes. No broader
behavior change, sc-lint adoption or QA waiver is approved. Each sprint and the
phase still require independent cursor-quality-mgr QA, passing applicable anchor
gates and green CI before merge.

## Approved application shape

Render the tagged template into an absent temporary destination using the documented
plain cargo-generate path, with registry dependencies and no checkout patches,
using explicit answers:

```json
{"project_name":"bd-cache","db":"none","mcp":false,"licence":"MIT"}
```

The new_project.py driver always runs setup/lint recipes, so do not invoke it
while sc-lint work is deferred. c-3 uses the supported lower-level renderer and
required schema/document rendering without running those recipes. Generated lint
assets are not installed into beads-cache.

`db=none` suppresses generated SQL stores and migrations. It does not replace the
cache with the template's in-memory example. The existing BD-CACHE v3 SQLite file,
cache operations and repository configuration remain authoritative.

Adopt generated lifecycle/configuration/observability structure into
`crates/bd-cache-service` and `crates/bd-cache-daemon`; retain the existing CLI and
Python bindings as local front ends. The generated example CLI's HTTP-only
operations, text default and exit 1 are incompatible with the existing bdc contract.
Port the applicable generated conventions, not its example behavior. Remove widget
examples and unused generated store/schema crates; do not ship inert scaffolding.

The proposed daemon is an explicit foreground scheduler host:
`bd-cache-daemon --repo <absolute-path> --config <daemon-config.json>`. It uses
sc-runtime's instance lock, endpoint validation, canonical logging and cleanup,
and runs the same built-in/Python schedule rows used by `bdc cache cron run`.
No daemon auto-start or new bdc command is introduced. The supported listener is
loopback only in this phase. Repository cache settings stay in
`.beads/bd-cache.toml`; daemon transport/logging settings are separate.

Runtime `/health` reports process liveness. Application `GET /ready` answers the
existing cache-health data in an sc-command envelope, using the same health
threshold defaults as bdc; degraded health has HTTP 503, healthy HTTP 200. A typed
health-evaluation error uses its existing error code and HTTP 503. Readiness starts
no bd process. No arbitrary operation route or MCP endpoint is added in this phase.
The daemon is operational because it hosts real scheduling, not because it can
serve a demo route. Application cleanup runs after the runtime HTTP drain, sets the existing
StopFlag and awaits bounded worker/child termination before releasing the instance
lock. The total bound includes the pinned upstream HTTP-drain maximum of 31 seconds
the existing worker cleanup bound, and at most one admitted readiness evaluation
with a 1-second store-read budget plus documented scheduling tolerance. This release has no public configurable
HTTP-drain budget.

The daemon needs an application composition without opening a second same-process handle
to the same cache file. HTTP readiness and the worker share that composition;
blocking store/backend work runs outside async request tasks. c-1 supplies the
store-owned bounded health-read seam; c-2 admits one readiness job with no waiter
queue, retains it through disconnect/timeout, and joins it during cleanup before
instance release. Saturation is a typed daemon-only 503; SQLite/mutex deadlines
are real store limits, not cancellation of a detached blocking future. Direct cached reads
through bdc and Python work whether the daemon is running or stopped.

## Boundary map before sprint cuts

There are no current boundary manifests. `docs/architecture.md` is today's allowed
workspace-edge table; CI checks it and forbids sc-command/clap transitively under
API/Python. Keep that enforcement; sc-lint adoption is deferred until sc-runtime
0.2.0 lands at the user's instruction. The following table
also records the proposed edges; it is not a claim that manifests already exist.

| Boundary | Contract or behavior affected | Allowed dependencies / dependents | Owner |
|---|---|---|---|
| Workspace/dependency registry | published sc-command/sc-cron exactly registry 0.2.0 under ADR-BDC-0016; rusqlite 0.39; retire local sc-cron | one libsqlite3-sys; no checkout patches or scheduler fork | c-1; c-2 registers its new app crates |
| bd-cache-store | CacheScheduleStore implements upstream ScheduleStore; legacy schedule rows and v3 identity preserved | model, sc-beads, upstream sc-cron; used by sync, CLI, Python, service | c-1 |
| bd-cache-sync | upstream target model, deadlines, worker and hook supervision | existing edges to cache/store/model/API/sc-bd-cli/sc-beads plus upstream sc-cron; used by CLI, Python, service | c-1 |
| bd-cache-api / bd-cache-model / bd-cache | request/data/error and domain rules unchanged | existing table edges; no runtime/server/framework assembly dependency | unchanged in this phase; compiler-required source edits require an explicit re-fence, not silent scope growth |
| bd-cache-cli / bd-cache-py | existing local operations and shutdown, tested against migrated composition | existing edges; approved-source sc-command in CLI; scheduler's contract-only sc-command may reach Python | c-1 compatibility; c-2 end-to-end qualification |
| bd-cache-service (new) | application state over one existing composition; readiness + schedule host | bd-cache-sync, model/API types, sc-config as needed; used by daemon only | c-2 |
| bd-cache-daemon (new) | generated runtime lifecycle, listener, logging, cleanup | service, sc-runtime/sc-command server, sc-transport, logging adapter; no SQL backend | c-2 |
| Template upkeep tooling | immutable pin/answers and pristine baseline comparison | upstream generator; c-2 consumes baseline and records application mapping | c-3 |
| Release tooling | no local sc-cron publication; binaries/wheels include the real app | canonical sc-publish installer; existing consumer extension point | c-4 |

The proposed graph amendment permits only the default-feature-off, contract/logging
`sc-command` dependency carried by upstream sc-cron beneath Python. It does not allow
clap, sc-runtime, sc-transport, axum, rmcp or daemon/service assembly in the API/Python
independently resolved normal/build consumer graphs. Qualify separately built
CLI/API/Python artifacts using temporary standalone consumer Cargo roots; the
all-workspace build may unify server features with the daemon. API still has no
direct sc-command dependency. Record this
amendment explicitly before changing the gate; do not silently delete the gate.

## Sprint cuts and wave table

| Wave | Track | Sprint | Closure / target | Owned-path summary |
|---|---|---|---|---|
| 1 | scheduler | [c-1 runtime migration](sprint-c-1-runtime-migration.md) | integration / scheduler adoption | root Cargo files; existing store/sync/compatibility code; local sc-cron deletion; existing ci.yml migration edits |
| 1 | tooling | [c-3 template upkeep](sprint-c-3-template-upkeep.md) | boundary / template upkeep tooling | runtime pin/answers, regeneration/diff helper and tests; no Cargo/workflow edits |
| 2 | application | [c-2 framework application](sprint-c-2-framework-application.md) | integration / generated application assembly | new service/daemon crates, actual template adoption mapping, application config/docs/consumer procedure; extends the existing CI graph checks |
| 3 | distribution | [c-4 release qualification](sprint-c-4-release-qualification.md) | integration / artifact distribution | release input/manifests/workflows through canonical kit installation; artifact/preflight checks and install docs |

**Sprint count: 4. Critical path: 3** (`c-1 → c-2 → c-4`, also
`c-3 → c-2 → c-4`). **Usable width: 2** in wave 1. c-1/c-3 are disjoint;
c-2 consumes the migrated SystemSchedules implementation and c-3's pinned template
baseline and executable regeneration helper; c-4 consumes the production daemon binary and qualification
procedure, not merely a shared version baseline. Shared root manifests and
ci.yml have one writer at a time, in their owning integration sprint.

No contract sprint is proposed: the scheduler adaptation preserves the existing
front-end contracts and consumes an already published upstream trait. Its store
and scheduler changes are one atomic compatibility migration. Splitting them
would add temporary dual scheduler types and bridge code. The new app boundary is
created with its implementation in c-2. These are the vertical exceptions, not a
rule that every feature crosses the entire crate stack.

Each track integrates when its own behavior is ready: c-1 proves existing CLI and
Python against the new scheduler, c-2 proves the generated daemon and consumer
workflow, c-4 proves packaged installation. There is no final test-only sprint.

## Feature closure ownership

| Phase behavior | Sole feature-level acceptance owner |
|---|---|
| Upstream scheduler operates on the existing cache file, with original CLI/Python behavior | c-1 |
| Generated application hosts actual sync and health with bounded lifecycle | c-2 |
| Template updates are reproducible and adopted differences reviewable | c-2 |
| Existing CLI/Python behavior survives scheduler migration | c-1; c-2 separately qualifies that preserved surface with its daemon present and absent |
| One actual consumer workflow passes against the built application | c-2 |
| Built archives/wheels can be installed and exercised, stable publish remains gated | c-4 |

## External prerequisites and release ruling

- sc-runtime v0.1.1 is published. Its sc-cron APIs differ from the local prototype;
  c-1 must preserve live rows or obtain an explicit upstream compatibility fix.
  No local scheduler fork is an accepted fallback.
- A real consumer repository/testbed is selected before c-2's final acceptance.
  If it requires journaled create (#104), report that requirement before dispatch;
  #104 is not silently folded into this migration.
- sc-publish #135 remains open at this draft's evidence baseline. The checked-in
  kit revision is `66d11b2e34d638f9e9239c23621744cb971cee3d`. c-4 qualifies a fixed
  immutable kit revision, or reports a blocked stable release. A prerelease uses
  the unaffected prerelease staging path. Never claim publication from a dry run.
- Channel credentials, immutable release settings and first WinGet submission are
  external setup; preflight reports their actual state without reading secrets.
- Workspace/wheel version stays 0.2.0 for planning. No 1.0 claim is authorized.
  If implementation changes the public contract incompatibly, stop and propose
  the semver/contract revision before changing outputs.

Phase closure requires all four sprints, green full CI, a passing actual consumer
procedure and installable candidate artifacts. Stable publication/channel activation
is a separate gate and may remain blocked by #135 or external setup; record the
actual outcome. Phase C does not close merely because all code is merged.

## Explicitly deferred

sc-lint installation/bootstrap/profile/boundary-manifest work until sc-runtime
0.2.0 lands; MCP, arbitrary HTTP operation parity, remote bdc mode/auto-start, alternative SQL
backends, Monty adoption, journaled create (#104), tracker migration, and a new
sc-runtime template updater product. The template-update helper is a local
regenerate-and-diff procedure, not an automatic merge engine.

## User steering: sc-lint deferral

User, 2026-11-02: “yes, sc-lint is a bit out of date on sc-runtime, I am having the
sc-runtime team bootstrap 0.8.x sc-lint now.  don't worry about sc-lint until
sc-runtime 0.2.0 lands”

No sprint installs/upgrades sc-lint or rewrites its profiles/boundary manifests in
this phase's current baseline. Existing Clippy and repository CI remain required.
When 0.2.0 lands, review its published template/tooling delta and propose a bounded
plan amendment or later sprint before taking that work. Do not interpret the
milestone alone as authorization to expand this plan. No monitoring is scheduled.

## Work bounds and plan readiness

Phase C reuses the relevant previously locked anchors documented in
[Inherited work bounds](anchors/index.md), with unchanged statements and original
provenance. These bound permissible changes to persistence, backend access, local
reads, sync behavior, front-end compatibility and deadlines. They are mapped to the
sprints they constrain, not replaced by conversation-derived planning notes.

Plan hardening and plan QA are complete as recorded in reviews/review-status.md.
The project owner has now approved the bounded implementation, including the
optional daemon. Implementation delivery remains subject to the sprint and phase
QA/CI gates; plan approval is not evidence that any sprint has shipped.
See [intent](intent.md) for the phase purpose and separation of proposal/context.

The PR's Rust CI passed. Python checks passed except Linux/Python 3.9, which failed
before tests at actions/setup-python: ubuntu-latest resolved to Ubuntu 26.04, for
which the official Python runner manifest offers no Python 3.9 build. That manifest
does list Python 3.9.25 builds for Ubuntu 22.04/24.04. The planning branch now
pins the Python Rust, wheel-build and wheel-test Linux matrices together to Ubuntu
24.04, preserving Python 3.9 support without a wheel/glibc mismatch. That narrow
CI repair is applied; validation of the repaired pipeline is a separate result,
and this note does not claim green checks before their completion. c-1 still
owns requalification of the preserved Python matrix after scheduler migration.
