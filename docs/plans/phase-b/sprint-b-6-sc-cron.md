---
id: b-6
phase: b
status: complete
closure_type: boundary
target_boundary: sc-cron
track: core
wave: 4
branch: sprint/b-6-sc-cron
worktree: sprint/b-6-sc-cron
recommended_agent: rust-developer
recommended_model: opus
must_follow: []
parallel_safe: [b-8]
requirements:
  owns: [REQ-CRON-0005, REQ-CRON-0009, REQ-CRON-0011, "REQ-CRON-0013 (new: Python hook target)", "REQ-CRON-0014 (new: enabled flag)", "REQ-CRON-0015 (new: five-field expressions)"]
  constrained_by: [REQ-CRON-0006, REQ-CRON-0007, REQ-CRON-0010, NFR-BDC-0004, NFR-BDC-0005]
adrs: [ADR-BDC-0006, ADR-CRON-0001, ADR-CRON-0005, ADR-CRON-0006, ADR-BDC-0010]
owned_paths:
  - "crates/sc-cron/**"
  - "docs/sc-cron/**"
---

# Sprint b-6 — sc-cron: Python hook target, enabled flag, stop-hook action

## Outcome

`sc-cron` stays a general-purpose job scheduler that sc-runtime can copy unchanged
(ADR-BDC-0006, REQ-CRON-0011). A schedule row can be pointed at a Rust `Job` or at a
Python module. It can be turned on or off without rewriting the row, and its stop hook
can continue, disable or delete it.

## Deliverables

1. **Python hook target** (REQ-CRON-0013, ADR-CRON-0001).
   - `HookTarget = Rust(Job) | Python { interpreter, module }`.
   - A Python target runs as `<interpreter> -I -m <module>` from a temp cwd, with one JSON
     object on stdin: `{"protocol": "sc-cron.hook.v1", "event": "cron.tick" | "cron.stop",
     "schedule", "payload", "deadline"}`.
   - Every hook a schedule starts, `cron.tick` and `cron.stop` alike, is killed at the row's
     timeout capped by the time left before the run's deadline (the same deadline passed on
     stdin). The kill uses a Unix process group or a Windows job object. Its stdout and stderr
     are capped (REQ-CRON-0005, NFR-BDC-0004).
   - This runner is the process b-7 uses to run the consumer's Python sync method and the
     post-sync hook, so this kill enforces NFR-BDC-0004 for both.
2. **`enabled` column** (REQ-CRON-0014, REQ-CRON-0009). The schedule table gets an
   `enabled` boolean column, default 1.
   - `Scheduler::set_enabled(name, bool)` updates only that column.
   - A disabled row never fires, and keeps its generation, expression and payload.
3. **Stop-hook action** (REQ-CRON-0005). The stop hook runs before each run and returns
   `continue`, `disable` or `delete` with a reason.
   - Rust `StopDecision::Stop(reason)` becomes `Continue`, `Disable(reason)` and
     `Delete(reason)`.
   - Python returns `{"action": "continue" | "disable" | "delete", "reason": "..."}`.
   - A failed, malformed or timed-out stop check skips that run and leaves the row active.
4. **Five-field expressions** (REQ-CRON-0015). Standard five-field crontab expressions
   (minute first) are accepted, as well as the existing six- and seven-field forms.
5. **Single flight across processes** (REQ-CRON-0006, REQ-CRON-0007). Generation fencing stays
   the guarantee, and is tested across two processes.

## Tests

All tests use injected time and the stub interpreter. No test needs bd or Dolt, since sc-cron
has no bd dependency (REQ-CRON-0010, NFR-BDC-0005):
- A Python hook through a stub interpreter: argv and stdin shape, timeout kill, output caps.
- `continue`, `disable` and `delete`, for a Rust and for a Python stop hook.
- `set_enabled` leaves the expression, payload and generation unchanged; a disabled row does
  not fire.
- Five-field parsing.
- Two-process fencing.

## Does not close

The bd sync job (b-7). sc-cron gains no beads, bd or beads-cache dependency or type
(REQ-CRON-0011).
