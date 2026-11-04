# Daemon requirements

Approved refinements of the owner-approved Phase C application contract.

## REQ-DAEMON-0001: Explicit foreground host

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

The daemon runs explicitly in the foreground with
`--repo <absolute-path> --config <daemon-json>`. Validate listener policy before
cache opening, reuse published sc-runtime lock, liveness, endpoint resolution,
logging and cleanup. Accept loopback TCP only. Host existing persisted sync rows.

**Refines:** REQ-BDC-1001
**Acceptance evidence:** FakeBd process acceptance: configured foreground startup, duplicate owner, loopback policy and persisted schedules.

## REQ-DAEMON-0002: Read-only HTTP health

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

GET /ready wraps existing cache-health data
in sc-command's envelope, HTTP200 for ok and503 for degraded/down/errors.
GET /health remains runtime process liveness. Expose no operation HTTP or MCP.

**Refines:** REQ-BDC-1001
**Acceptance evidence:** FakeBd outage/liveness/readiness acceptance and `canceled_http_keeps_actual_read_and_cache_claim_until_cleanup`; only `/ready` is application-owned.

## REQ-DAEMON-0003: Typed startup failures

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

Config errors retain public
CFG.READ/PARSE/DESERIALIZE; runtime errors retain typed RuntimeError and nested
TransportError. Format one bounded secret-free stderr line and empty stdout,
exit1; usage CLI.USAGE/exit2, help exit0. Retain logger finish precedence.

**Refines:** REQ-BDC-1001/REQ-BDC-0730
**Acceptance evidence:** FakeBd config/policy/endpoint/duplicate/bind refusals and real logger adapter child fixtures; `ProductEncodingTest` preserves strict diagnostic decoding.

## REQ-DAEMON-0004: Owned bounded shutdown

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

The daemon keeps instance ownership through
runtime HTTP drain (30 seconds plus1-second backstop), existing worker shutdown
(at most2T), admitted read budget (at most1second) and child reap; no configurable
HTTP drain budget or additional signal handler. Busy admission returns503 with
DAEMON.READINESS_BUSY and the documented retry recovery.


**Refines:** REQ-BDC-1001/NFR-BDC-0004
**Acceptance evidence:** Blocked-hook process stop/reap/restart and readiness HTTP cleanup fixture. The Unix-only public runtime lifecycle fixture additionally requires LockHeld through actual signal-driven cleanup and verifies restart. Portable runtime instance-lock ordering with canceled readiness remains OPEN (C2-REQ-009), as do upstream bounded-store shutdown (RSH-001) and Windows graceful stop (QA-C2-001).
