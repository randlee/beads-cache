---
phase: c
status: complete
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
  - REQ-BDC-0805
  - REQ-BDC-0810
  - REQ-BDC-0812
  - REQ-BDC-0910
  - NFR-BDC-0001
  - NFR-BDC-0002
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

All five implementation deliverables are complete. Application/local process
qualification is recorded in [evidence/c-2-application.md](evidence/c-2-application.md).
The actual scratch consumer workflow passed against release artifacts from source
`623218021d27ef247b389233650314896b69cbc1`; see the exact source/artifact/bridge/
cleanup [receipt](evidence/c-2-consumer-qualification.md). The later receipt commit
changes documentation only and does not claim a different artifact build source.

`status: complete` records implementation delivery, not QA/CI completion or merge
eligibility. Parent QA and PR merges remain required. The advisory gate remains
unsuppressed and externally blocked under the immutable sc-runtime 0.1.1 pin. The
owner instructed deferral until sc-runtime 0.2.0 is available and an explicit
baseline adoption/revalidation is approved. The bead remains in progress until
the required gates are satisfied; no QA waiver, self-merge or publication occurred.
