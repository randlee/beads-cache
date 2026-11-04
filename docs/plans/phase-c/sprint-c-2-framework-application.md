---
phase: c
status: in_progress
recommended_agent: rust-developer
recommended_model: gpt-6.1-sol
id: c-2
closure_type: integration
target_boundary: generated application assembly
track: application
wave: 2
branch: sprint/c-2-framework-application
worktree: sprint/c-2-framework-application
vertical_rationale: Creates the new service/daemon application boundary from one upstream scaffold and wires its
  only production consumer; splitting the initial assembly would defer lifecycle integration without independent
  existing boundaries.
must_follow:
- c-1
- c-3
parallel_safe: []
requirements:
  owns:
  - REQ-BDC-1001
  - REQ-BDC-1003
  - REQ-SERVICE-0001
  - REQ-SERVICE-0002
  - REQ-SERVICE-0003
  - REQ-DAEMON-0001
  - REQ-DAEMON-0002
  - REQ-DAEMON-0003
  - REQ-DAEMON-0004
  constrained_by:
  - REQ-BDC-0001
  - REQ-BDC-0006
  - REQ-BDC-0101
  - REQ-BDC-0103
  - REQ-BDC-0720
  - REQ-BDC-0722
  - REQ-BDC-0723
  - REQ-BDC-0725
  - REQ-BDC-0727
  - REQ-BDC-0730
  - REQ-BDC-0732
  - REQ-BDC-0802
  - REQ-BDC-0804
  - REQ-BDC-0903
  - REQ-BDC-0805
  - REQ-BDC-0810
  - REQ-BDC-0812
  - REQ-BDC-0910
  - NFR-BDC-0001
  - NFR-BDC-0002
  - NFR-BDC-0003
  - NFR-BDC-0004
  - NFR-BDC-0005
  - NFR-BDC-0006
  - REQ-PY-0006
  - REQ-BDC-0007
  - REQ-BDC-0008
  - REQ-BDC-0502
  - NFR-BDC-0007
adrs:
- ADR-BDC-0009
- ADR-BDC-0010
- ADR-BDC-0011
- ADR-BDC-0012
- ADR-BDC-0014
- ADR-BDC-0015
- ADR-SYNC-0002
- ADR-SYNC-0008
- ADR-CLI-0003
- ADR-CLI-0004
- ADR-CLI-0005
- ADR-PY-0006
- ADR-BDC-0005
- ADR-BDC-0013
- ADR-STORE-0003
- ADR-CORE-0001
- ADR-BDC-0016
owned_paths:
- Cargo.toml
- Cargo.lock
- crates/bd-cache-service/**
- crates/bd-cache-daemon/**
- config/**
- scripts/consumer_checks.py
- .github/workflows/ci.yml
- .github/workflows/python.yml
- docs/architecture.md
- docs/development/runtime.md
- docs/getting-started.md
- docs/plans/phase-c/evidence/**
- runtime/adoption.toml
- docs/bd-cache-service/**
- docs/bd-cache-daemon/**
- .claude/project/quality-policy.md
inherited_bounds:
- ANC-001
- ANC-002
- ANC-003
- ANC-004
- ANC-008
- ANC-009
- ADR-BDC-0009
- ADR-STORE-0001
- REQ-BDC-0001
- REQ-BDC-0006
- REQ-BDC-0007
- REQ-BDC-0008
- REQ-BDC-0502
- REQ-BDC-0812
- NFR-BDC-0004
- NFR-BDC-0005
- ADR-BDC-0005
---

# Sprint c-2 — Generated application, ongoing alignment and consumer proof

## Outcome

The installed sc-runtime scaffold hosts real cache sync and readiness. Existing
bdc/Python behavior remains local, and the existing CI gates validate the new workspace without adopting sc-lint.
This sprint closes application integration and actual consumer acceptance, not a
placeholder generated example.

## Recorded owner approval

The project owner's explicit 2026-11-02 instruction to execute all approved
Phase C sprints satisfies this sprint's dispatch prerequisite. See the phase
plan's [recorded approval](phase-c-plan.md#implementation-dispatch-gate) and
ADR-BDC-0016 for provenance and the unchanged bounds. ADR-BDC-0016 is now in the
binding `adrs` list. c-1 applies the approved CLI/API/CRON amendments in its owned
files; no other existing rule is waived. Independent implementation QA, applicable
anchor gates and green CI remain required before merge.

## Deliverables

1. REQ-BDC-1001: render the immutable template with the phase-plan answers and
   release-mode dependencies. Adapt new service/daemon crates to one existing
   Composed cache and migrated SystemSchedules. Register new workspace members
   literally, update the architecture edge table, add their
   per-crate requirements/architecture docs, and add the new crate/domain rows to
   quality-policy.md. The new per-crate REQs refine REQ-BDC-1001 and stay within
   the operations/lifecycle contract already stated here.
   Implement the phase plan's foreground daemon and /ready contract; reuse
   sc-runtime's /health, lock, endpoint validation, logging and cleanup lifecycle.
   The daemon hosts the cache-file schedules, not a second scheduler database.
2. REQ-BDC-1001: preserve existing cached-read and Python execution without a
   daemon; use existing execute_api/health and cache error codes for readiness.
   Distinguish process liveness from degraded cache readiness. Keep cache TOML and
   daemon config separate; unknown cache keys still fail. Shut down the worker and
   supervised child tree before releasing instance ownership. No widget/demo
   implementation or generated SQL store remains reachable.
3. REQ-BDC-1003: consume c-3's pin/answers/baseline helper, record the actual
   adopted-path mapping in runtime/adoption.toml using the sole format definition in
   [c-3, Adopted-path mapping contract](sprint-c-3-template-upkeep.md#adopted-path-mapping-contract),
   and document application-owned
   adaptations. Prove that regenerating old/new baselines exposes an adopted
   scaffold change without overwriting domain code. just upgrade remains a lint
   upgrade; it is not the framework update procedure.
4. REQ-BDC-0812, REQ-BDC-0910: preserve existing CI and its OS/Python gates. Extend the current
   docs/architecture.md edge table and inline graph checks for the new crates;
   do not install sc-lint, create its profiles or migrate to its boundary manifests
   before sc-runtime 0.2.0 lands. Keep independently built local artifacts isolated
   from server dependencies, while accounting for workspace feature unification.
5. REQ-BDC-1001: execute and document one actual consumer workflow on the designated
   scratch testbed: schedule built-in sync, read locally, journal a supported
   write, drain/read-back, observe outage/recovery, switch to Python sync, observe
   post-sync results/errors, and stop/restart the daemon. Record exact versions,
   commands, timings and observed outcomes under evidence/. Unit tests still use
   FakeBd; live bd is testbed-only. If the consumer needs #104, report the missing
   capability rather than replacing the consumer proof with synthetic tests.

## Inherited preservation bounds

ANC-002: the host runs bd sync at times stored in the cache file's SQLite cron
table. ANC-003: a consumer-registered Python sync method replaces the built-in bd
sync when selected. ANC-008: each repository has at most one built-in bd sync
schedule row; a second built-in row is rejected, not added by the new host.
ANC-009: existing sync scopes remain bd filters such as labels or --all, never
phase definitions. ANC-004: preserve the optional consumer-registered post-sync hook, which
receives every sync result and error. ADR-BDC-0005: cached reads continue during
bd outages and affected scopes record their outage. NFR-BDC-0004: every bd call,
Python sync method and post-sync hook is killed at its timeout, capped by the
remaining run deadline; shutdown grace does not extend execution deadlines.
REQ-BDC-0007/0008/0502: preserve the strict bd-schema-generated row decoder on all
stored rows including consumer sync, all-or-nothing rejection of a result with
one bad row, and the updated_at guard that retains a newer stored row.
REQ-BDC-0812: preserve every Python 0.1 verb, argument and returned dict unchanged,
with no DeprecationWarning. New assembly does not change those binding methods.

## Composition contract

The existing `compose_with_config`, `execute_api`, public Composed fields,
`SystemSchedules::system` and `SystemSchedules::cron_run` are preserved by c-1.
The service uses the existing production sharing constructor, not the hidden
`cache_for_tests` accessor or a second open:

```rust
let health_reads = composed.health_reads.clone();
let facts = composed.facts();
let files = composed.files();
let table = composed.schedules.clone();
let schedule_store = composed.schedule_store()?;
let cache = Arc::new(Mutex::new(composed.cache));
let schedules = SystemSchedules::system(schedule_store, Arc::clone(&cache), files);
// Service state owns health_reads, facts, table, cache and schedules.
// One admitted blocking closure uses cache.try_lock(), then:
// health_reads.within(deadline, ||
//     execute_api(&table, &mut cache_guard, &request, &facts))
// No guard is held across await; the budget covers the schedule view too.
```

The service owns these values; daemon/runtime owns lifecycle, listener and envelope
adaptation. No domain rule moves into route handlers.

`bd-cache-daemon --repo <absolute-path> --config <daemon-config.json>` runs in the
foreground with a loopback listener. Upstream run_with_cleanup invokes application cleanup after HTTP drain while
still holding instance ownership. That cleanup sets StopFlag and awaits the
existing bounded worker stop. Test a total bound of the pinned upstream HTTP-drain
maximum (30 seconds plus a one-second backstop) plus the existing worker bound (at most twice its shutdown timeout),
including child reap before ownership release plus at most one readiness evaluation (1-second read budget) and documented scheduler tolerance
in process tests. Cleanup joins both worker and admitted readiness work before release. v0.1.1 does not expose a configurable production HTTP-drain
budget; do not invent a config key for it. Do not assume upstream supplies
an immediate application cancellation token; this phase does not add a second
signal handler. /ready delegates to
existing cache-health evaluation over shared state and returns the phase-plan
200/503 envelope contract. Neither readiness nor cached reads may run bd.

## Daemon startup and readiness failure contract

Reuse the pinned template's main/agent_error formatting and typed library causes.
Daemon failures print one bounded, secret-free stderr line
`<code> <cause> — <recovery>`, leave stdout empty, and exit 1; argv misuse uses
CLI.USAGE/exit 2 and help exits 0. This applies to the new daemon only: bdc's
JSON/0-2-3 and Python exception contracts do not change. Preserve a runtime
error's code/message/cause/remediation and nested TransportError as typed sources;
format code and recovery once, never flatten/relabel every failure as INTERNAL.
The explicit --config file adapter maps file read, JSON and typed decoding errors
to public sc_config::ConfigError variants; do not invent an upstream arbitrary-file
Loader API. Keep secrets out of diagnostics as in the generated adapter.

| Failure | Stable typed code / cause | Recovery and observable |
|---|---|---|
| Missing/unreadable, malformed JSON or wrong typed daemon config | ConfigError::Read / Parse / Deserialize: CFG.READ / CFG.PARSE / CFG.DESERIALIZE with path and safe location/type cause | preserve ConfigError::recovery(): fix file access, reported JSON location or typed fields; stderr/exit 1, no cache open |
| Invalid endpoint syntax/resolution | RuntimeError::EndpointFailed: RUNTIME.ENDPOINT_FAILED with structured TransportError | preserve RuntimeError::remediation() and transport cause; correct uds/tcp syntax or required port; stderr/exit 1, no cache open |
| Syntactically valid non-loopback listener | application ListenerPolicyError: DAEMON.LISTENER_POLICY, cause 'listener must use a loopback TCP address' | 'use tcp:127.0.0.1:<port> or tcp:[::1]:<port>'; stderr/exit 1 before stores; do not silently bind a public address |
| Existing instance owner | RuntimeError::LockHeld: RUNTIME.LOCK_HELD, preserving its cause | preserve library remediation: stop the other instance or choose a different instance root; stderr/exit 1, no store/worker start |
| Listener cannot bind | RuntimeError::BindFailed: RUNTIME.BIND_FAILED with structured bind/endpoint-busy TransportError | preserve library remediation: free the endpoint or select another permitted one; stderr/exit 1, no worker survives failed startup |

Keep generated logger ownership/finish precedence: drop application users before
consuming observability.shutdown(); preserve a primary startup/runtime error and
report a secondary logger-shutdown error separately. A sole logging init/shutdown
failure retains DAEMON.OBSERVABILITY / DAEMON.OBSERVABILITY_SHUTDOWN, generated
recovery and exit 1.

Readiness admission is finite: one process-wide permit, acquired with
try_acquire_owned before submitting to the one owned blocking worker; never
await a permit or queue a second probe. Saturation or a busy shared-cache try_lock returns HTTP 503 with an
sc-command unavailable envelope, code DAEMON.READINESS_BUSY, cause 'readiness
capacity or cache is busy', recovery 'retry the readiness probe shortly'. A
closing service similarly refuses admission with that same code and recovery
'retry after the daemon restarts'. These are daemon adapter errors, not new
BD_CACHE codes or changed domain health rules.

Move the permit into the actual blocking job. The service owns the dedicated
worker thread JoinHandle before HTTP serving begins and tracks whether that
worker is executing the admitted job independently of its response receiver.
Completion returns the existing
cache-health data/200-or-503 contract. Evaluation uses c-1's HealthReadControl with
a 1-second absolute monotonic deadline and probe=false. A store budget failure
keeps BD_CACHE.STORE_FAILED/cause and HTTP 503 with recovery to retry and inspect
cache contention if persistent. If a request disconnects or upstream times it out,
only its response wait is dropped: the service retains the job and permit until
actual completion; no retry spawns another job while capacity is occupied. Reuse
the upstream generic request timeout/drain responses for that canceled HTTP path.

After HTTP drain, cleanup closes admission, sets the worker StopFlag, and joins
all admitted readiness work and the worker while instance ownership is still held.
The worker is one dedicated std::thread, not one spawn_blocking task per probe.
Submission uses a zero-capacity rendezvous channel with try_send: if the worker
is not idle/receiving, release the unsubmitted permit and return READINESS_BUSY;
there is no accepted-but-queued job. Start it before serving and retain its
JoinHandle; after HTTP drain close its submission channel, let the one admitted
job finish/expire under c-1's budget, and join the thread before releasing instance
ownership. Do not detach the thread or claim dropping a response receiver cancels
its job. An already-expired budget returns immediately when the closure starts.
This worker executes the same synchronous health closure; it is not a new scheduler,
extra cache connection or backend process. Existing scheduler jobs retain their
approved spawn_blocking execution path. Process-test tolerance covers thread
scheduling, not unbounded admission or SQLite/mutex waits.

## Acceptance criteria

| Root | Positive observable | Planted negative |
|---|---|---|
| req:REQ-BDC-1001 | generated daemon runs a real scheduled sync against FakeBd; a pre-existing cache retains file/version/rows and schedule state | leave the demo job or open an independent schedule DB; scenario detects no cache sync |
| req:REQ-BDC-1001 | /health stays live during backend outage; /ready reports degraded cached health without bd, then recovers from recorded state | probe bd for readiness or report liveness as cache health; call-log/response test fails |
| req:REQ-BDC-0730 | duplicate instance is refused; stop during a blocked hook is bounded, reaps children and then releases ownership; restart succeeds | release the instance lock before shutdown completes; second-instance/process test fails |
| req:REQ-BDC-0001 | original bdc JSON/0-2-3 contract, alias identity, latency and Python corpus pass with daemon absent and present | route a cached get through HTTP; daemon-off and latency tests fail |
| req:REQ-BDC-1003 | recorded pin/answers regenerate a baseline; a changed upstream template path appears in the diff without overwriting working files | suppress a changed adopted path; update-helper test fails |
| req:REQ-BDC-1001 | malformed config, non-loopback listener, duplicate owner and bind failure produce the defined typed code/cause/recovery, stdout and exit behavior, and leave no worker on failed startup | flatten errors or start stores before refusal; startup process tests fail |
| req:REQ-BDC-1001 / NFR-BDC-0004 | concurrent readiness probes admit one job, return typed 503 on saturation/cache contention, keep ownership after timeout/disconnect, and join real work within the read/cleanup bounds before releasing the instance | release the permit when the request future drops or leave queued blocking work; concurrency/shutdown process test fails |
| req:REQ-BDC-1001 | actual consumer testbed completes the supported workflow with recorded outcomes and bounded timing | mark an undrained write as applied or lose pending data on outage; consumer procedure fails |

## Required validation

Full existing formatting, Clippy, docs, boundary/script/advisory checks and Rust/Python CI on
supported OSes; new daemon process/HTTP/lifecycle tests with FakeBd; original CLI
latency gate and neutral Python corpus; template helper integration smoke; actual scratch
consumer procedure. Linux/macOS run the process/signal paths and Windows covers
job-object and shutdown behavior. Evidence names the exact integration commit.

## Dependencies and limits

For both c-1 and c-3, a parent development push triggers merge-forward into c-2
before every child development/fix round; do not wait for parent QA to begin that
work. Each parent PR must merge before c-2's PR can complete. These execution
triggers apply to the named artifacts below, not shared-file sequencing.

c-1 supplies the upstream-backed store/worker, shutdown implementation and bounded HealthReadControl seam; c-3
supplies the pinned template baseline/answers and regeneration helper. These production artifacts, not shared
filenames, justify both dependencies. c-4 consumes this sprint's built daemon and
consumer procedure. Public operation HTTP parity, MCP, auto-start and create are
outside the fence; the optional daemon does not change the local cache contract.

## Implementation handoff

The application implementation and earlier consumer qualification are delivered;
QA1 has identified independent corrections and unresolved upstream/gate findings. Application/local process
qualification is recorded in [evidence/c-2-application.md](evidence/c-2-application.md).
The actual scratch consumer workflow passed against release artifacts from source
`623218021d27ef247b389233650314896b69cbc1`; see the exact source/artifact/bridge/
cleanup [receipt](evidence/c-2-consumer-qualification.md). The later receipt commit
changes documentation only and does not claim a different artifact build source.

`status: in_progress` includes QA corrections and open upstream/CI/merge gates.
The earlier implementation-only completion label is superseded by this status. Parent QA and PR merges remain required. The advisory gate remains
unsuppressed and externally blocked under the immutable sc-runtime 0.1.1 pin. The
owner instructed deferral until sc-runtime 0.2.0 is available and an explicit
baseline adoption/revalidation is approved. The bead remains in progress until
the required gates are satisfied; no QA waiver, self-merge or publication occurred.


## QA1 correction delivery and open gates

QA1 reviewed `bf7f8aad17f1081716b6d37adb44873f25293ee7` and published FAIL:
36 unique IDs (12 Blocking, 19 Important, 5 Minor). The authoritative report is
[PR #109 QA1](https://github.com/randlee/beads-cache/pull/109#issuecomment-6594892195),
with local full occurrences and ceremony screen at
`/tmp/phase-c-execution/c-2/qa-1/screen-input.json` and `screen-result.md`.
Every ID below requests independent verification; no row declares QA closure.

C1 published corrections were consumed from `73ced71`, followed by its test-only
FTQ-001 correction `96ce326fca99a2919a6462bd4249b9931281eadd`. The sole merge
conflict in ci.yml retained both approved graph invocations, C1's graph unit
proofs, and exactly one jsonschema prerequisite; C1 remains that workflow
correction's owner. Shared project-plan corrections come only from C1.

| Finding | Correction / remaining disposition |
|---|---|
| FTQ-001 | Consumed C1 96ce326: whole-child watchdog bounds the held-mutex scenario and its deliberately stalled-reader failure branch. Independent C1 QA2 verification remains required. |
| FTQ-002 | Consumed C1 deterministic injected row-decode expiration, replacing machine-throughput assumptions. |
| FTQ-003 | Consumed C1 causal active-health-session / normal-writer contention handshakes and restoration proof. |
| FTQ-004 | Consumed C1 first-snapshot acknowledgements and owned observer cleanup for store and both sync pollers. |
| FTQ-005 | Consumed C1 actual contended mode-lock acquisition handshake with bounded completion. |
| FTQ-006 | Consumed C1 bounded real watchdog and owned abort/join for hook and manual-clock run completion. |
| FTQ-007 | Consumed C1 bounded Cargo metadata capture and actual sleeping-child timeout/reap regression. |
| FTQ-008 | Consumed C1 bounded holder/readiness/release/task completion and unwind-safe release ownership. |
| C2-FTQ-001 | Readiness thread test now independently awaits actual evaluator entry, aborts/joins response task, checks held capacity, observes join attempt, and releases/joins under finite deadlines. The complete scenario runs in an owned child watchdog. |
| C2-FTQ-002 | Application test uses a finite startup/readiness deadline with yielding rather than sleep, a cleanup deadline, and a whole-child watchdog owning all service threads. |
| C2-FTQ-003 | Both real-logger lifecycle cases now run entirely inside the existing owned child watchdog; real flush/shutdown/event assertions remain. |
| C2-FTQ-004 | Removed the scheduling race that launched a new process after stop inside a finite grace window. Duplicate-owner refusal before stop and real blocked-tree reap/restart remain. Causal HTTP fixture separately checks cleanup retains the cache claim while evaluation is held. Exact runtime instance-lock retention through this phase remains OPEN with C2-REQ-009; no expanded grace or replacement proof is claimed. |
| C2-FTQ-005 | QA1 added Windows Job ownership and initial POSIX group cleanup. QA2 correctly found that actual upstream hooks lead a separate group within the session, so the original POSIX cleanup/proof was incomplete. The verified-session correction and actual-topology regressions are recorded below; this historical row does not claim QA2 closure. |
| QA-C2-001 | OPEN/BLOCKED: published sc-runtime 0.1.1 registers Windows CtrlC only; the isolated process-group acceptance requires CTRL_BREAK. Existing zero-exit/cleanup assertions and Windows acceptance remain intact. Needs approved public upstream graceful stop support; no competing handler/workaround is added. |
| QA-C2-002 | run_product captures bytes and strictly decodes both streams on its calling thread, so Windows pipe-reader threads cannot swallow UnicodeDecodeError. Exact UTF-8 diagnostic and malformed-byte negative regressions remain. |
| ARCH-001 | OPEN/BLOCKED: unauthorized direct sync -> sc-command logging receiver edge requires the upstream public seam recorded by C1. No boundary waiver or dropped canonical events. |
| RSH-001 | OPEN/BLOCKED: pinned WorkerHandle can await uncancelled blocking store work beyond the stop deadline. Existing ownership joins remain; no detach/timeout/drop substitute claims to enforce 2T. |
| RBQA-F001 | OPEN/BLOCKED, distinct correlated logging dependency finding; consume only an approved published upstream receiver seam through C1. |
| RBQA-F002 | Consumed C1 canonical root-reachable normal/build provenance/introducer checks and planted negatives; C2 delegates to the same checker, retaining only the CLI's already-approved direct contract and clap dependencies. Actual unauthorized edge is intentionally rejected. |
| RBQA-F003 | Consumed C1 testing-only ModuleHook export and crate-private invoke; no normal consumer process API remains. |
| C2-RBQA-F001 | Removed unused direct service -> store normal dependency and both architecture table edges. Real storage stays behind sync composition. |
| C2-RBQA-F002 | Daemon routes module is private; router is crate-visible. Binary-facing composition/config/observability APIs remain. |
| C2-RBP-F001 | ShutdownError::source exposes the actual Worker JoinError and no source for source-free variants. Test downcasts the retained cause from an actually canceled Tokio task. |
| C1-RBP-F001 | Consumed C1 infallible local unrunnable constructor correction while preserving fallible resolver errors. |
| C2-REQ-001 | Single sprint list now includes all seven local refinements and retained 0804/0903/NFR0003 constraints. Per-crate records have titled IDs, approved status, statement/refines/evidence, and both baseline indices include service/daemon. |
| C2-REQ-002 | Consumed C1-owned shared lifecycle correction; owning C2 status is in_progress and the implementation-only complete label is superseded. Root metadata aa192fa now updates the central C2/C3/C4 rows from C1's exact shared-file baseline. All QA/CI/parent merge gates remain open. |
| C2-REQ-003 | OPEN: exact new-head OS/Python qualification, independent QA and C1 merge are required. Windows graceful stop, actual unauthorized graph, unbounded upstream store join and unsuppressed paste advisory block green completion. Owner defers paste effort until published 0.2.0 and explicit adoption/revalidation. |
| C2-REQ-004 | OPEN/BLOCKED with RSH-001: no approved upstream bounded store-operation shutdown contract is available; happy-path stop timings cannot qualify contended store work. |
| C2-REQ-005 | Consumed C1 absolute scheduled deadline propagation through record claims, preparation, mutex wait, backend, hook and completion; causal expired-before-acquisition regression checks no later backend call. |
| C2-REQ-006 | Consumed C1 256-byte module validation for Python sync and hooks, with positive 256/negative 257 and unchanged-row assertions. |
| C2-REQ-007 | Partially corrected: C2 delegates to canonical graph provenance/introducer rule; a planted unauthorized direct contract edge is rejected even for CLI, while its own existing direct contract remains permitted. OPEN until upstream logging seam permits C1 to remove the actual direct sync introducer. |
| C2-REQ-008 | Consumed C1 migrated production scheduled 1000-bead incremental qualification (6.184458 ms release measurement), including incremental cursor and changed cached row assertions. Cached-get latency is not used as this evidence. |
| C2-REQ-009 | Partially corrected, OPEN for exact runtime instance-lock lifecycle: owned real HTTP fixture uses the production router, public runtime request-timeout instrumentation and actual Application health evaluation. It proves causal admission, typed concurrent BUSY, actual HTTP timeout cancellation, capacity retention, HTTP drain, cleanup/cache-claim retention, actual reader join and reopen/restart. A testing-feature observer gates one real evaluation inside its original one-second budget; no production config bypass exists. Published runtime test-util lacks public run_with_cleanup stop injection/instance control, and Windows production stop is blocked by QA-C2-001. Cache-claim retention is not relabeled as runtime instance-lock proof. |
| RSH-002 | Consumed C1 scheduled absolute deadline propagation and causal contention/expiry proof; shutdown grace does not renew execution budgets. |
| RSH-003 | Consumed C1 health deadline checks at all scope/filter row and status assembly boundaries, with deterministic expiration/restoration tests. |
| RSH-004 | Consumed C1 continuous-worker context with no detailed retained failure collection; finite run-once retains exact details and drain-once proof. |


The retained actual consumer receipt qualifies only source
`623218021d27ef247b389233650314896b69cbc1`. C1 production corrections now change
the native candidate; old live proof is not proof for this new head. Future
exact-candidate scratch qualification is required after the remaining approved
corrections/baseline decisions. No new remote action is performed in this round.
Final source validation and exact push SHA follow separately; overall disposition
remains FAIL/blocked, not merge readiness or a waived acceptance criterion.


### QA1 correction validation receipt

Final correction source: `3a6bef362ab4eaa9b76a2d7534b6ad122704ecea`.
Native Rust/manifests/lock content is unchanged from `070ab583af093475941405c3b67c164f1a07d414`;
`c8fa58a` adds test-supervisor Windows kill-on-close ownership and `3a6bef3`
preserves the CLI's approved direct contract while retaining actual sync-edge
rejection. Merge `56ccd25` consumes root lifecycle metadata `aa192fa` by taking
that exact project-plan blob; only C2/C3/C4 row conflicts were reconciled.
This validation receipt changes documentation only.

- PASS: formatting, all-target workspace Clippy, all-feature non-Python Clippy,
  default library Clippy and warnings-denied docs. Logs:
  `/tmp/c2-qa1-fix-final-clippy.log`, `c2-qa1-fix-clippy-all.log`,
  `c2-qa1-fix-clippy-default.log`, `c2-qa1-fix-docs.log` (all under `/tmp`).
- PASS: all-feature non-Python workspace tests/doctests: 1153 passed, 0 failed,
  21 intentionally ignored across 114 top-level result groups (nested watchdog
  child summaries are not counted twice). `/tmp/c2-qa1-fix-workspace.log`.
  This includes C1 96ce326 and all new service/logger/real HTTP fixture cases;
  the owned HTTP case observes the public runtime's actual
  503/RUNTIME.REQUEST_TIMEOUT response, not an invented 504 contract.
- PASS: final pushed script suite 102 run, 101 passed, 1 existing explicitly
  opt-in pinned-generation smoke skip. No new platform skip was added.
  `/tmp/c2-qa1-fix-scripts-final.log`. Focused graph/encoding/owned-tree proofs:
  `/tmp/c2-qa1-fix-graph-followup.log`; canonical four graph proofs:
  `/tmp/c2-qa1-fix-graph-unit.log`.
- PASS: freshly rebuilt debug CLI, fake binaries and daemon process acceptance:
  scheduled sync, cached v3 identity/state, outage recovery, typed startup errors,
  duplicate/bind cleanup, blocked hook child/grandchild reap and restart.
  Stop samples 0.002901/0.001937/1.005516 seconds at the existing configurations.
  `/tmp/c2-qa1-fix-build-cli.log`, `c2-qa1-fix-build-daemon.log`,
  `c2-qa1-fix-process.log` under `/tmp`. No test invokes live bd or Dolt.
- PASS: retained release CLI latency gate: median 5.4 ms (50 ms bound), p95
  6.5 ms; pending overlay reported median 6.0 ms/p95 7.0 ms.
  `/tmp/c2-qa1-fix-latency.log`. Incremental 1000-bead sync evidence remains
  the explicitly consumed C1 production-composition measurement above.
- PASS: independent API graph; strict consumer neutrality; no backend client.
  The no-backend PATH check passes under the controlled cargo/system-only PATH;
  the normal developer PATH contains installed tools and is not claimed isolated.
  `/tmp/c2-qa1-fix-graph-api.log`, `c2-qa1-fix-neutrality.log`,
  `c2-qa1-fix-backend-graph.log`, `c2-qa1-fix-backend-path.log` under `/tmp`.
- EXPECTED REAL FAIL, not waived: independent Python and CLI graphs both reject
  `bd-cache-sync -> sc-command`; the CLI's own approved edge is not rejected.
  `/tmp/c2-qa1-fix-graph-python.log`, `c2-qa1-fix-graph-cli.log` under `/tmp`.
- REAL FAIL, unsuppressed: RUSTSEC-2024-0436 for paste1.0.15 through
  utoipa-axum0.2.0 / sc-runtime0.1.1. `/tmp/c2-qa1-fix-advisories.log`.
  No additional workaround investigation or baseline change occurred.

Local execution is macOS arm64/Python3.11. Windows Job Object teardown and the
fixed strict-decoding regression still require hosted Windows confirmation;
production CTRL_BREAK acceptance remains blocked upstream and unchanged.
The existing native Python corpus receipt is C1's changed-source qualification,
not a newly built C2 release wheel or replacement for the new-head OS/Python CI.
No full unchanged suite was repeated for the metadata-only merge.

Overall remains FAIL/blocked: upstream logging and bounded store shutdown,
Windows graceful stop, exact canceled-readiness/runtime-instance-lock proof,
unsuppressed advisory, exact-head CI and required parent completion/merges.
All 36 IDs remain available to the next independent QA pass; implementation
corrections above request verification and do not close the bead or authorize merge.


## QA2 fixture ownership correction

QA2 reviewed `380f87f3a9634982fb1b992acf96b5e18f89f304` and published FAIL with
nine IDs (7 Blocking, 2 Important), while owning reviewers resolved 27 original
IDs. Authoritative report:
[PR #109 QA2](https://github.com/randlee/beads-cache/pull/109#issuecomment-6595342965);
full findings and ceremony screen are under `/tmp/phase-c-execution/c-2/qa-2/`.
This round changes only the two acceptance scripts and this owning receipt;
integration merge-forward contributes orchestration metadata only. Native Rust,
manifest/lock content and the previously qualified local debug artifacts remain
unchanged. No Rust/wheel suite or remote consumer qualification is repeated.

| QA2 finding | Disposition |
|---|---|
| C2-FTQ-005 | Correction requests verification: POSIX starts an owned session-leader keeper, which reports the acceptance worker's original exit code while staying alive and unreaped. Cleanup freezes that anchor, inventories PIDs and verifies each matching SID before signaling, stops/kills members across every owned hook PGID under one absolute 10-second deadline, then kills/reaps the anchor last. Caller SID is refused before any signal. Scratch removal occurs only after successful tree cleanup; cleanup failure retains scratch and preserves the primary failure/timeout diagnostics. Windows Job admission/accounting/kill-on-close is unchanged. |
| QA-C2-001 | OPEN: published runtime Windows CtrlC-only handling does not satisfy isolated-group CTRL_BREAK graceful stop. Original Windows acceptance and zero-exit/cleanup assertions remain. No handler or runtime-control workaround. |
| C2-REQ-003 | OPEN: exact-head OS/Python CI, current-native/wheel consumer qualification and parent completion/merge gates remain. Historical live6232180 proof is not retargeted to current native source. The graph/advisory failures are real and unsuppressed. |
| C2-REQ-004 | OPEN: approved upstream bounded store-operation stop/join contract is unavailable; safe actual joins remain, with no timeout/drop substitute or bound padding. |
| C2-REQ-007 | OPEN: repaired canonical graph check correctly rejects the actual direct sync logging edge; removal requires the approved published upstream logging seam through C1. |
| C2-REQ-009 | OPEN: real HTTP/service cancellation fixture proves retained capacity/cache claim, but does not establish actual runtime instance-lock ordering through run_with_cleanup. No cache-claim or keeper test is mislabeled as that proof. |
| RBQA-F001 | OPEN: unauthorized sync -> sc-command dependency requires the compliant upstream public receiver seam; no boundary expansion or dropped events. |
| RSH-001 | OPEN: published worker shutdown can await uncancelled blocking store work beyond the existing bound. Preserve ownership and await the approved upstream correction. |
| ARCH-001 | OPEN: distinct correlated direct logging receiver dependency finding remains, despite correct fail-closed graph enforcement. |

Focused regressions now establish the actual topology causally: a helper calls
setpgid(0,0), its grandchild shares that separate PGID, and both retain the owned
SID. Both failed-worker and watchdog-timeout cases assert distinct PGID/same SID,
verify every actual signal targets the owned session, and observe both helpers
terminated and the direct leader reaped before scratch removal. An unrelated
session sentinel must remain alive and is explicitly terminated/reaped by the
test afterward. The original worker exit23 and TimeoutExpired command/timeout
are preserved. No test or platform acceptance is skipped.

Pre-push focused ownership/graph/encoding validation: 6 tests passed on local
macOS arm64/Python3.11; log `/tmp/c2-qa2-ownership-focused.log`. Full scripts and
unchanged-native FakeBd acceptance run against the pushed source next; the parent
will preserve final logs, without another evidence-only commit. Overall remains
blocked by the eight OPEN IDs above; no QA PASS, completion or merge claim.


## Strict-resume lifecycle proof and remaining eight IDs

The owner requires the developer/QA loop to continue until zero findings and
all required gates pass. This round consumes QA3 FAIL at
https://github.com/randlee/beads-cache/pull/109#issuecomment-6595583718 and the full
assignment `/tmp/phase-c-execution/c-2/qa-4/fix.xml`. The eight reviewer-derived
`.triage/c/findings/*.ttl` records are preserved byte-for-byte with status OPEN;
no developer disposition marks a finding closed.

### Concrete public-runtime assessment

Pinned public run_with_cleanup can exercise the actual instance lock on Unix
without a new runtime API: the exactly filtered owned test child installs the
published runtime through its builder, serves real readiness, and sends real
SIGTERM only to itself after serving/admission handshakes. A concurrent public
Daemon startup must fail LockHeld after the actual runtime enters cleanup and
closes admission, while the same admitted read remains held following its HTTP
request timeout. Releasing the read allows actual application/runtime joins;
reacquiring that same instance root and endpoint then proves restart. The existing
public request-timeout test middleware and testing-only health observer are used;
no signal handler, production config escape or private runtime control is added.
The original one-second read budget is not extended. A whole-child watchdog and
bounded signal-sender kill/reap path own failures as well as successful cleanup.

Portable proof still needs an upstream test-util DaemonFixture start_with_cleanup
variant that invokes actual run_with_cleanup while using its existing private
controlled shutdown, preserving serving/endpoint/root and join ownership. Current
start variants call run; with_control and RunControl are private. Exposing a
fixture lifecycle hook is sufficient; no public InstanceLock internals are needed.
This request is separate from Windows production signals: 0.1.1 Signals/install/
wait only registers and polls ctrl_c. Upstream must register/poll ctrl_break too
before lock/bind and enter the same drain/cleanup path for either event. Existing
isolated-group CTRL_BREAK, zero-exit, actual reap, ownership and restart assertions
remain unchanged. No local handler or baseline workaround is authorized.

| Finding / TTL | Current developer disposition (all OPEN pending independent QA) |
|---|---|
| QA-C2-001 | `.triage/c/findings/QA-C2-001.ttl`: concrete CtrlBreak production gap sent to parent for upstream coordination. No Windows acceptance change or successful stop claim. |
| C2-REQ-003 | `.triage/c/findings/C2-REQ-003.ttl`: exact-head OS/Python/advisory and parent gates remain. Prepared fresh qualification inputs/provenance without selecting, building, transferring or running obsolete artifacts. Historical6232180 proof stays historical. |
| C2-REQ-004 | `.triage/c/findings/C2-REQ-004.ttl`: C1 owns published bounded worker/store compatibility; no duplicate research or local cancellation/drop substitute. |
| C2-REQ-007 | `.triage/c/findings/C2-REQ-007.ttl`: actual direct sync logging edge still fails the repaired gate; consume only C1's approved published seam correction. |
| C2-REQ-009 | `.triage/c/findings/C2-REQ-009.ttl`: new Unix-only actual run_with_cleanup/real-signal/LockHeld/read-join/restart test requests independent verification. Portable lifecycle proof, Windows graceful stop and contended-store bound remain open; no partial fixture is completion. |
| ARCH-001 | `.triage/c/findings/ARCH-001.ttl`: C1/parent owns public logging seam; no dependency-policy or ADR change. |
| RSH-001 | `.triage/c/findings/RSH-001.ttl`: C1/parent owns bounded store-operation shutdown; no timeout padding or detached work. |
| RBQA-F001 | `.triage/c/findings/RBQA-F001.ttl`: distinct logging dependency finding retained independently, not collapsed into ARCH-001. |

Local-only `/tmp/c2-next-candidate-qualification-inputs.json` records the seven
immutable consumer source hashes and three approved bounded driver hashes, all
reverified. It selects zero current artifacts: final candidate_source is null
until approved compatible production sources are frozen/pushed. It prescribes
fresh same-source release CLI/daemon/wheel builds and hashes, native install,
pinned-bd/toolchain verification and the existing scratch workflow under parent
transport ownership. No consumer identifiers or source are copied into this
product repository, and the prior qualified manifest is not retargeted.

Initial Unix public-lifecycle experiment passed; targeted daemon Clippy passes.
Final pushed-source tests and exact SHA are reported to parent separately. The
unsuppressed paste gate and deferred sc-lint/0.2 baseline remain unchanged. No
upstream repository mutation/publication, remote qualification, self-merge or
completion claim occurs in this round.

## Cross-sprint QA4 correction

Complete C4 QA1 added two C2-owned findings. The public observability startup
error now retains its concrete upstream validation/init source, original code
and remediation; four real-error/public-process regressions request independent
verification. The lifecycle test's endpoint-reservation handoff remains racy:
published0.1.1 exposes the actual bound endpoint only through a fixture that
cannot retain this required cleanup/signal path. All prior eight IDs plus
C4-RBP-F001 and C4-FTQ-001 remain OPEN. The exact public API blocker, minimal
upstream requirement and full ten-ID disposition are recorded in
[the QA4 correction receipt](evidence/c-2-cross-sprint-qa4.md). Real signal,
run_with_cleanup, LockHeld, actual joins and restart assertions are preserved;
the endpoint finding is not closed by a passing sample or weakened proof.

### Completed QA4 and qualified C3 parent consumption

[C2 QA4](https://github.com/randlee/beads-cache/pull/109#issuecomment-6608719324)
verified the typed startup error correction and retained nine OPEN findings
(7 Blocking, 2 Important, 0 Minor). Parent-authored triage closure/review records
are preserved. The qualified C3 PR111 merge `91467d1`, including its QA4/anchor
PASS and 15 green CI checks, is now consumed through integration `77a5eea`.
Only the qualified template helper/tests and inherited administrative evidence
change; all native source and typed-error tests remain identical to `3282502`.
The [updated receipt](evidence/c-2-cross-sprint-qa4.md#qualified-c3-merge-forward-after-completed-c2-qa4)
lists every remaining blocker. C1, exact-candidate qualification, current C2
QA/CI/anchor and published compatibility gates remain; C2 is not merge-ready.


### Qualified C1 bounded-shutdown component consumption

Exact integration `d4af63d2a81bebd6b5b967ef5032731152bb5f7c` and independently
qualified C1 source `5255610a16ef4500f0af508e29a533707a4a3e0a` are consumed by
ordinary merges `74536fb` and `ddc0027`. Store/sync trees match qualified source
exactly; C2 owning triage/history and integration orchestration are preserved.
The existing Application uses the bounded SystemSchedules::system path and
retains real worker/readiness joins. Two actual Application contention/zero-
timeout regressions request independent QA5; no production interface change.
See [the scoped receipt](evidence/c-2-qualified-shutdown-consumption.md) for all
nine dispositions and source provenance. RSH-001/C2-REQ-004 are implemented
pending C2 QA; endpoint/Windows/logging/paste/current qualification gates remain.
Whole C1 is FAIL3B1I, PR108 remains unmerged, and C2 is not merge-ready.
