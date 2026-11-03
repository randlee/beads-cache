# Daemon requirements

REQ-DAEMON-0001 refines REQ-BDC-1001: run explicitly in the foreground with
`--repo <absolute-path> --config <daemon-json>`. Validate listener policy before
cache opening, reuse published sc-runtime lock, liveness, endpoint resolution,
logging and cleanup. Accept loopback TCP only. Host existing persisted sync rows.

REQ-DAEMON-0002 refines REQ-BDC-1001: GET /ready wraps existing cache-health data
in sc-command's envelope, HTTP200 for ok and503 for degraded/down/errors.
GET /health remains runtime process liveness. Expose no operation HTTP or MCP.

REQ-DAEMON-0003 refines REQ-BDC-1001/REQ-BDC-0730: config errors retain public
CFG.READ/PARSE/DESERIALIZE; runtime errors retain typed RuntimeError and nested
TransportError. Format one bounded secret-free stderr line and empty stdout,
exit1; usage CLI.USAGE/exit2, help exit0. Retain logger finish precedence.

REQ-DAEMON-0004 refines REQ-BDC-1001/NFR-BDC-0004: keep instance ownership through
runtime HTTP drain (30 seconds plus1-second backstop), existing worker shutdown
(at most2T), admitted read budget (at most1second) and child reap; no configurable
HTTP drain budget or additional signal handler. Busy admission returns503 with
DAEMON.READINESS_BUSY and the documented retry recovery.
