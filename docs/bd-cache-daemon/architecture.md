# Daemon architecture

The source mapping is runtime/adoption.toml, generated at runtime/pin.toml with
runtime/answers.json. Production uses published sc-runtime0.1.1. Config, main,
canonical logger bridge, request IDs, typed failure rendering and stores/routes/
run_with_cleanup follow the rendered scaffold. Read docs/development/runtime.md
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

The daemon depends on the service and composition/clock types. The service may
not depend on daemon or runtime. Existing independently resolved CLI/API/Python
consumers do not inherit the daemon's axum/runtime/MCP implementation dependencies.
An all-workspace build unifies features; isolated consumer gates enforce this.


Route assembly is private to this crate. The owned readiness HTTP fixture uses
that real router and public runtime request-timeout middleware, then drains HTTP
and joins the actual application. Its cache-file claim proof is distinct from
runtime daemon.lock ordering: published test-util lacks a public controlled-stop
run_with_cleanup fixture. Exact canceled-readiness/instance-lock coverage remains
open, together with Windows CTRL_BREAK compatibility and upstream contended-store
shutdown. No test fixture substitutes for the unchanged Windows acceptance.
