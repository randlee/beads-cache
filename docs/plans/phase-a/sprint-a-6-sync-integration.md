---
id: a-6
phase: a
status: planned
closure_type: integration
target_boundary: bd-cache-sync
track: cache
wave: 4
branch: sprint/a-6-sync-integration
worktree: sprint/a-6-sync-integration
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [a-2, a-3, a-5]
parallel_safe: []
requirements:
  - REQ-BDC-0003
  - REQ-BDC-0701
  - REQ-BDC-0702
  - REQ-BDC-0703
  - REQ-BDC-0704
  - REQ-BDC-0705
  - REQ-BDC-0706
  - REQ-BDC-0707
  - REQ-BDC-0903
  - NFR-BDC-0001
  - NFR-BDC-0003
  - NFR-BDC-0004
  - NFR-BDC-0006
adrs: [ADR-BDC-0002, ADR-BDC-0005, ADR-BDC-0006]
owned_paths:
  - "crates/bd-cache-sync/**"
  - "tests/e2e/**"
  - "docs/architecture.md"
---

# Sprint a-6 — sync task and phase integration

## Outcome and closure

Compose the core, the real `sc-bd-cli` adapter and `sc-cron` into the sync task,
and close every feature-level criterion of phase A. Closes when the cron
cases pass, the end-to-end flow passes through the real composition, and the
crate layout in `docs/architecture.md` matches what shipped.

## must_follow rationale

Composes `sc-bd-cli` (a-2), the scheduler (a-3) and the `Cache` facade (a-5).

## Deliverables

1. `bd-cache-sync`: tick within a 25 s budget (drain with a deadline 5 s
   before the end, then sync each active scope, deferring scopes not started
   2 s before the end), stop rule, escalation and sync-alert announcements
   through an injected `EscalationSender` port, scheduled on `sc-cron`.
2. An `atm send` sender implementation behind that port.
3. End-to-end tests: a journaled write applied by a scheduled drain through
   `sc-bd-cli`, with a recording invoker in CI and against a scratch `bd`
   workspace on the testbed.
4. Performance check against BD-CACHE.md targets: in-process `get` < 1 ms;
   incremental sync < 1 s for a 1000-bead scope (testbed).
5. `docs/architecture.md` crate layout updated to the shipped state.

## Acceptance criteria

| Criterion | Root |
|---|---|
| All 9 `test_bd_cache_cron.py` cases pass with an injected sender | BD-CACHE.md Cron module |
| A write journaled through the core lands in `bd` via the scheduled drain, verified by read-back, end to end | BD-CACHE.md How it works |
| With the backend unreachable for several ticks, reads keep answering with staleness reported and nothing escalates; on recovery the journal drains | ADR-BDC-0005 |
| Performance targets met on the testbed and recorded | BD-CACHE.md Performance |
| No contract from a-1 is left without a production consumer | phase plan |

## Required validation

Full workspace validation; the end-to-end tests; testbed live run recorded.
