# Daemon architecture

The source mapping is runtime/adoption.toml, generated at runtime/pin.toml with
runtime/answers.json. Production uses published sc-runtime0.2.0. Config, main,
canonical logger bridge, request IDs, typed failure rendering and stores/routes/
run_with_cleanup_and_bound follow the rendered scaffold. Read docs/development/runtime.md
for the application-owned adaptations and updater proof.

The runtime performs endpoint validation, signals, lock, stores, route assembly,
bind and serving. Our earlier loopback policy check freezes a valid resolved
endpoint. Invalid endpoint syntax remains the runtime's typed EndpointFailed;
policy rejection runs before any stores. Explicit-file config uses public
sc_config::ConfigError variants, not an invented Loader API. Diagnostics omit JSON
values and bound/control-escape rendered causes. Instance-root/logging setup can
precede runtime errors; cache and worker creation cannot precede instance lock.

The stores closure transfers Application to awaited cleanup and returns only
ReadyState. Even bind failure runs cleanup. The library retains RuntimeError until
main's final formatting boundary; nested TransportError sources are preserved.
The logger owner is consumed only after runtime/app handles drop. A primary
runtime failure wins over a secondary cleanup/logger failure.

The public bound callback emits one canonical `daemon.listener_bound` lifecycle
event with the actual endpoint and process identity. It reports binding, before
HTTP serving, without writing stdout or requiring another config/CLI option.
Installed process acceptance requests port zero and observes only its newly
spawned child's event in the configured log, then separately checks the live
child, competing bind/instance refusal and actual health/readiness. Its explicit
bind-failure control holds the competing socket throughout startup; no freed
reservation is treated as endpoint ownership. Restart retains the same instance
and cache; the owned runtime fixture separately checks same-endpoint restart.

The daemon depends on the service and composition/clock types. The service may
not depend on daemon or runtime. Existing independently resolved CLI/API/Python
consumers do not inherit the daemon's axum/runtime/MCP implementation dependencies.
An all-workspace build unifies features; isolated consumer gates enforce this.


Route assembly is private to this crate. The Unix real-signal lifecycle fixture
uses public `run_with_cleanup_and_bound`: the runtime reports its actual port-0
endpoint while retaining the same listener. A competing bind must fail with
AddrInUse. The real SIGTERM/cleanup/LockHeld/reader-join/restart assertions remain.
The portable cleanup fixture uses the released `DaemonFixture::start_with_cleanup`
and checks listener ownership, real Application joins and instance refusal.

SDK 0.2.0 removed the former public `testing::instrument_router` helper used by
these causal timeout tests. Restoring the existing public test-util timeout and
tracking surface is required to compile and qualify the unchanged one-second
admitted-read budget. The production 30-second timeout cannot substitute for it.
No application middleware, budget extension or test skip is adopted. Windows
CTRL_BREAK and exact-candidate platform acceptance remain required.
