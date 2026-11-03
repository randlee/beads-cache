# Foreground cache daemon

Build with `cargo build -p bd-cache-daemon`. Copy `config/daemon.example.json`
outside the checkout, set an absolute instance root for this repository and a
free loopback TCP port, then run:

```sh
bd-cache-daemon --repo /absolute/path/to/repo --config /absolute/path/to/daemon.json
```

The daemon hosts the existing cache-file schedules. Configure them with local
`bdc cache schedule set '* * * * * *'` (or the preserved Python API). Existing
`bdc issues get` and Python cached reads work while the daemon is stopped or
running. Do not run another cron host for the same cache. Cache TOML stays in
`.beads/bd-cache.toml` and rejects unknown keys; transport, logging, executable and
worker shutdown settings belong only to daemon JSON.

GET `/health` is runtime liveness. GET `/ready` returns the existing cache-health
data and default thresholds in an sc-command envelope:200 for state `ok`,503 for
`degraded` or `down`; successful evaluation remains `ok:true` even when health is
degraded. Typed evaluation errors return503/`ok:false` preserving BD_CACHE codes.
Readiness never probes bd. Busy capacity/cache mutex returns503 with
`DAEMON.READINESS_BUSY`, cause `readiness capacity or cache is busy`, recovery
`retry the readiness probe shortly`; closing admission uses the same code and
`retry after the daemon restarts`. A store-read deadline preserves
`BD_CACHE.STORE_FAILED` and suggests retrying/inspecting persistent contention.

One readiness thread and zero-capacity rendezvous admit at most one actual job.
The permit moves into that job, and request cancellation does not cancel or
untrack it. The shared cache is acquired with try_lock. HealthReadControl applies
an absolute one-second budget to both schedule and cache reads on the existing
connection. Thread scheduling tolerance is50ms plus one row decode, as documented
by the store seam. Cleanup joins real work and the scheduler after runtime HTTP
drain, before instance unlock. HTTP drain is at most30seconds plus a1second
backstop. Scheduler shutdown is at most twice `shutdown_timeout_ms` (default5000);
the total bound is31seconds +2T + at most1second admitted read, with1second process
test tolerance. Configuring worker grace never extends job execution deadlines.
There is no production configurable HTTP drain key or second signal handler.

Startup failures print a bounded control-escaped stderr line
`<code> <cause> — <recovery>`, stdout empty, exit1. Usage is CLI.USAGE/exit2; help
exits0. Explicit-file loading preserves CFG.READ/PARSE/DESERIALIZE and safe parser
locations/type causes. Invalid endpoint syntax retains RUNTIME.ENDPOINT_FAILED;
valid non-loopback/UDS addresses return DAEMON.LISTENER_POLICY. Duplicate owners
retain RUNTIME.LOCK_HELD. Bind failure retains RUNTIME.BIND_FAILED and cleanup
joins any application work already started. Typed runtime and transport sources
remain intact until final process formatting. Runtime/app handles drop before the
logger owner shuts down; the original runtime failure wins over secondary
cleanup/logger shutdown errors.

## Scaffold ownership and upkeep

The actual rendered immutable baseline is sc-runtime v0.1.1 commit
`5af24c862fe7ed8dae8710b7bd64a84a29506b51`, cargo-generate0.25.1/sc-compose1.7.0,
with the recorded `runtime/answers.json` (`db=none`, `mcp=false`).
`runtime/adoption.toml` records exact source/destination paths in C3's sole format.
We adopt main/error/observability/config/router/composition conventions. We replace
the example service with the existing cache composition, remove all demo routes,
omit unused generated CLI/API-types/store crates, and do not install generated
lint configuration. The generated cron database/config/job is replaced by the
existing SystemSchedules over the cache-file schedule table. No fork or checkout
patch is used. Console events use the public stderr sink so stdout stays reserved.
The generated logger orphan-rule bridge and tests are retained
until the upstream public adapter exists.

Use [runtime-updates.md](runtime-updates.md) to regenerate old/new pristine
baselines, compare all mapped/unmapped paths, and port selected deltas explicitly.
`just upgrade` still upgrades lint tooling; it is not framework upkeep. The C2
[evidence](../plans/phase-c/evidence/c-2-application.md) records a controlled
upstream template change regenerated into a second baseline and detected without
changing application code.

## Qualification

CI runs the FakeBd process acceptance via `scripts/consumer_checks.py` and
independent normal/build dependency graphs via its `--graph` mode. The latter
checks CLI/API/Python separately, because workspace server feature unification
is expected. Existing OS, Python corpus and latency gates remain.

Actual native consumer acceptance runs only on the designated marker-qualified
scratch testbed. Require `.sc-bd-live-root` and `.beads/sc-bd-live-scratch`, use the
existing scripts/bd-compat setup/teardown, a checksum-verified bd1.3.1 install,
and installed same-commit binaries/wheel. The host's bd1.3.2dev is not the pinned
fingerprint. External consumer source/config stays outside this product repo.
Supply native Cache through an existing provider field and execute the unchanged
supported write/drain/read-back method, disclosing that substitution. Check exact
call signatures first: the selected existing consumer requires an external
provider adapter to supply native precondition/drain-budget arguments and translate
exception classes. That adaptation must be reviewed and its execution recorded;
direct native substitution is not currently compatible. Exercise
built-in scheduling, local reads, journal/drain/read-back, outage/pending-data
preservation/recovery, Python replacement sync, post-sync results/errors and
stop/restart. Do not select create-dependent behavior from issue104. Never run
live bd/Dolt against a primary user repository or consumer data.
